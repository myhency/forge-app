use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};

// How many lines of one transcript are searched for a `cwd`. Real transcripts carry one within
// their first few lines; the bound only stops a pathological file from being read in full.
pub(crate) const MAX_CWD_SCAN_LINES: usize = 60;

/// One transcript and the folder it belongs to.
pub(crate) struct Transcript {
    pub modified: SystemTime,
    pub path: PathBuf,
    /// The folder as written in the transcript (or inherited, see `assign_cwds`).
    pub cwd: PathBuf,
    /// `cwd` with symlinks resolved; two transcripts are of the same folder when their keys match.
    pub key: PathBuf,
}

// Every `*.jsonl` file in `dir` with its modification time, newest first (ties broken by path so
// the order is deterministic). Unreadable entries are skipped, never an error.
pub(crate) async fn transcripts_newest_first(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut transcripts = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        transcripts.push((modified, path));
    }
    transcripts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    transcripts
}

// The first non-empty `cwd` string in the first `MAX_CWD_SCAN_LINES` lines. A line that is not
// valid JSON (a partial final line of a transcript still being appended to, or invalid UTF-8) is
// skipped rather than failing the scan.
async fn read_cwd(path: &Path) -> Option<PathBuf> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut lines = BufReader::new(file).split(b'\n');
    for _ in 0..MAX_CWD_SCAN_LINES {
        let Ok(Some(line)) = lines.next_segment().await else {
            return None;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if let Some(cwd) = value
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|cwd| !cwd.is_empty())
        {
            return Some(PathBuf::from(cwd));
        }
    }
    None
}

/// The path with symlinks, `.` and a trailing `/` resolved; a path that cannot be resolved (a
/// folder that no longer exists) is used as written.
pub(crate) async fn folder_key(path: &Path) -> PathBuf {
    tokio::fs::canonicalize(path)
        .await
        .map(|path| dunce::simplified(&path).to_path_buf())
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Gives every transcript of one directory the folder it belongs to. `transcripts` must be newest
/// first; so is the result. A transcript that names no `cwd` takes the `cwd` of the newest
/// transcript that does. When no transcript names one, nothing is assigned (empty result).
pub(crate) async fn assign_cwds(transcripts: Vec<(SystemTime, PathBuf)>) -> Vec<Transcript> {
    let mut read = Vec::with_capacity(transcripts.len());
    for (modified, path) in transcripts {
        let cwd = read_cwd(&path).await;
        read.push((modified, path, cwd));
    }
    // Newest first, so this is the newest transcript that names a cwd.
    let Some(inherited) = read.iter().find_map(|(_, _, cwd)| cwd.clone()) else {
        return Vec::new();
    };
    let mut assigned = Vec::with_capacity(read.len());
    for (modified, path, cwd) in read {
        let cwd = cwd.unwrap_or_else(|| inherited.clone());
        let key = folder_key(&cwd).await;
        assigned.push(Transcript { modified, path, cwd, key });
    }
    assigned
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn cwd_line(cwd: &Path) -> String {
        serde_json::json!({ "type": "user", "cwd": cwd.to_string_lossy() }).to_string()
    }

    fn file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn each_transcript_gets_the_cwd_it_names() {
        let dir = tempfile::tempdir().unwrap();
        let a = file(dir.path(), "a.jsonl", &cwd_line(Path::new("/work/a")));
        let b = file(dir.path(), "b.jsonl", &cwd_line(Path::new("/work/b")));

        let assigned = assign_cwds(vec![(at(2), b.clone()), (at(1), a.clone())]).await;

        assert_eq!(assigned.len(), 2);
        assert_eq!((assigned[0].path.clone(), assigned[0].cwd.clone()), (b, PathBuf::from("/work/b")));
        assert_eq!((assigned[1].path.clone(), assigned[1].cwd.clone()), (a, PathBuf::from("/work/a")));
    }

    #[tokio::test]
    async fn a_transcript_with_no_cwd_takes_the_cwd_of_the_newest_transcript_that_has_one() {
        let dir = tempfile::tempdir().unwrap();
        let a = file(dir.path(), "a.jsonl", &cwd_line(Path::new("/work/a")));
        let b = file(dir.path(), "b.jsonl", &cwd_line(Path::new("/work/b")));
        let bare = file(dir.path(), "bare.jsonl", r#"{"type":"attachment"}"#);

        let assigned = assign_cwds(vec![(at(3), bare), (at(2), b), (at(1), a)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/b"));
        assert_eq!(assigned[2].cwd, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn nothing_is_assigned_when_no_transcript_names_a_cwd() {
        let dir = tempfile::tempdir().unwrap();
        let bare = file(dir.path(), "bare.jsonl", r#"{"type":"attachment"}"#);

        assert!(assign_cwds(vec![(at(1), bare)]).await.is_empty());
    }

    #[tokio::test]
    async fn two_spellings_of_one_folder_share_a_key() {
        let folder = tempfile::tempdir().unwrap();
        let real = folder.path().canonicalize().unwrap();
        std::fs::create_dir(real.join("sub")).unwrap();
        // `Path` already treats a `.` component as absent, so the second spelling goes through `..`.
        let roundabout = real.join("sub").join("..");
        let dir = tempfile::tempdir().unwrap();
        let plain = file(dir.path(), "plain.jsonl", &cwd_line(&real));
        let dotted = file(dir.path(), "dotted.jsonl", &cwd_line(&roundabout));

        let assigned = assign_cwds(vec![(at(2), dotted), (at(1), plain)]).await;

        assert_ne!(assigned[0].cwd, assigned[1].cwd, "the cwd keeps the spelling it was written in");
        assert_eq!(assigned[0].key, assigned[1].key);
    }

    #[tokio::test]
    async fn a_folder_that_no_longer_exists_is_keyed_as_written() {
        assert_eq!(folder_key(Path::new("/gone/for/good")).await, PathBuf::from("/gone/for/good"));
    }
}
