use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, BufReader};

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

// How many bytes at the end of a transcript are searched for a folder change. A `relocated` record
// further back is covered by the turns that followed it, whose `cwd` lines name the new folder. A
// window holding no whole line (one huge last line) is doubled, up to `MAX_TAIL_BYTES`.
const TAIL_BYTES: u64 = 16 * 1024;
const MAX_TAIL_BYTES: u64 = 1024 * 1024;

// The folder a transcript belongs to: the last move record (written when `폴더 변경…` moves the
// session) when the tail has one, else the first `cwd`. That first `cwd` gives way to the last
// `cwd` of the tail only when the first is of another directory and the last is of this one: a
// session moved long ago. Later `cwd` lines are not trusted otherwise, as a turn may `cd` elsewhere.
async fn read_cwd(path: &Path) -> Option<PathBuf> {
    let tail = read_tail(path).await;
    if let Some(relocated) = tail.relocated {
        return Some(relocated);
    }
    let first = read_first_cwd(path).await?;
    if let Some(last) = tail.last_cwd.filter(|last| *last != first) {
        if !in_own_directory(path, &first).await && in_own_directory(path, &last).await {
            return Some(last);
        }
    }
    Some(first)
}

#[derive(Default)]
struct Tail {
    relocated: Option<PathBuf>,
    last_cwd: Option<PathBuf>,
}

async fn read_tail(path: &Path) -> Tail {
    let Ok(mut file) = tokio::fs::File::open(path).await else { return Tail::default() };
    let Ok(len) = file.metadata().await.map(|metadata| metadata.len()) else { return Tail::default() };
    let mut window = TAIL_BYTES;
    loop {
        let start = len.saturating_sub(window);
        if file.seek(SeekFrom::Start(start)).await.is_err() {
            return Tail::default();
        }
        let mut bytes = Vec::new();
        if file.read_to_end(&mut bytes).await.is_err() {
            return Tail::default();
        }
        let mut lines = bytes.split(|byte| *byte == b'\n');
        if start > 0 {
            // The window starts inside a line.
            lines.next();
        }
        let mut lines = lines.filter(|line| !line.is_empty()).peekable();
        if lines.peek().is_none() && start > 0 && window < MAX_TAIL_BYTES {
            window *= 2;
            continue;
        }
        return scan_tail(lines);
    }
}

fn scan_tail<'a>(lines: impl Iterator<Item = &'a [u8]>) -> Tail {
    let mut tail = Tail::default();
    for line in lines {
        let relocation = contains(line, b"\"relocatedCwd\"");
        if !relocation && !contains(line, b"\"cwd\"") {
            continue;
        }
        let Ok(value) = serde_json::from_slice::<Value>(line) else { continue };
        let named = |key: &str| value.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(PathBuf::from);
        if value.get("type").and_then(Value::as_str) == Some("relocated") {
            // The Claude CLI writes its own `relocated` record on resume, with a `sessionId` and the
            // folder it last knew, which is stale after a second move. Only a move record (Forge's,
            // or the desktop app's, which has the same shape) carries no `sessionId`.
            if value.get("sessionId").is_none() {
                if let Some(folder) = named("relocatedCwd") {
                    tail.relocated = Some(folder);
                }
            }
        } else if let Some(cwd) = named("cwd") {
            tail.last_cwd = Some(cwd);
        }
    }
    tail
}

fn contains(line: &[u8], needle: &[u8]) -> bool {
    line.windows(needle.len()).any(|window| window == needle)
}

// Whether Claude Code would keep a session of `folder` in the directory `transcript` lies in.
async fn in_own_directory(transcript: &Path, folder: &Path) -> bool {
    let Some(dir) = transcript.parent().and_then(Path::file_name) else { return false };
    crate::sessions::escape_project_path(&folder_key(folder).await) == dir.to_string_lossy()
}

// The first non-empty `cwd` string in the first `MAX_CWD_SCAN_LINES` lines. A line that is not
// valid JSON (a partial final line of a transcript still being appended to, or invalid UTF-8) is
// skipped rather than failing the scan.
async fn read_first_cwd(path: &Path) -> Option<PathBuf> {
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

    fn dir_of(root: &Path, project: &Path) -> PathBuf {
        let dir = root.join(crate::sessions::escape_project_path(project));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn the_last_relocated_record_decides_the_folder() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/c"));
        let relocated = |to: &str| serde_json::json!({ "type": "relocated", "relocatedCwd": to }).to_string();
        let content = [cwd_line(Path::new("/work/a")), relocated("/work/b"), relocated("/work/c")].join("\n");
        let moved = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), moved)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/c"));
    }

    #[tokio::test]
    async fn a_later_cwd_of_this_directory_wins_when_the_first_cwd_is_of_another() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/b"));
        let content = [cwd_line(Path::new("/work/a")), cwd_line(Path::new("/work/b"))].join("\n");
        let moved = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), moved)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/b"));
    }

    #[tokio::test]
    async fn without_a_relocated_record_the_first_cwd_stays_even_when_a_later_line_moved_on() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/a"));
        let content = [cwd_line(Path::new("/work/a")), cwd_line(Path::new("/work/a/sub"))].join("\n");
        let plain = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), plain)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/a"));
    }

    fn forge_relocated(to: &str) -> String {
        serde_json::json!({ "type": "relocated", "relocatedCwd": to }).to_string()
    }

    // The record the Claude CLI writes itself when it resumes; it names the folder it last knew.
    fn cli_relocated(to: &str) -> String {
        serde_json::json!({ "type": "relocated", "relocatedCwd": to, "sessionId": "8b1a56e1-f7b2-464b-8208-6bb9668a4b41" }).to_string()
    }

    #[tokio::test]
    async fn a_stale_cli_record_after_the_last_move_back_is_ignored() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/a"));
        let content = [
            cwd_line(Path::new("/work/a")),
            forge_relocated("/work/b"),
            cli_relocated("/work/b"),
            cwd_line(Path::new("/work/b")),
            forge_relocated("/work/a"),
            cli_relocated("/work/b"),
            cwd_line(Path::new("/work/a")),
        ]
        .join("\n");
        let moved = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), moved)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn a_cli_record_alone_leaves_the_first_cwd_in_charge() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/a"));
        let content = [cwd_line(Path::new("/work/a")), cli_relocated("/work/z")].join("\n");
        let plain = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), plain)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn once_the_move_record_scrolled_out_the_new_turns_cwd_still_decides() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/b"));
        let filler = "x".repeat(1024);
        let turn = serde_json::json!({ "type": "user", "cwd": "/work/b", "note": filler }).to_string();
        let mut lines = vec![cwd_line(Path::new("/work/a")), forge_relocated("/work/b"), cli_relocated("/work/a")];
        lines.extend(std::iter::repeat_n(turn, 40));
        let moved = file(&dir, "s.jsonl", &lines.join("\n"));

        let assigned = assign_cwds(vec![(at(1), moved)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/b"));
    }

    #[tokio::test]
    async fn a_huge_last_line_widens_the_tail_until_the_move_record_is_seen() {
        let root = tempfile::tempdir().unwrap();
        let dir = dir_of(root.path(), Path::new("/work/b"));
        let huge = serde_json::json!({ "type": "attachment", "content": "y".repeat(200 * 1024) }).to_string();
        let content = [cwd_line(Path::new("/work/a")), forge_relocated("/work/b"), huge].join("\n") + "\n";
        let moved = file(&dir, "s.jsonl", &content);

        let assigned = assign_cwds(vec![(at(1), moved)]).await;

        assert_eq!(assigned[0].cwd, PathBuf::from("/work/b"));
    }

    #[tokio::test]
    async fn a_folder_that_no_longer_exists_is_keyed_as_written() {
        assert_eq!(folder_key(Path::new("/gone/for/good")).await, PathBuf::from("/gone/for/good"));
    }
}
