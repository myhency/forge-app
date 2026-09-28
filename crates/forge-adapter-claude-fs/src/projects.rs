use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::ProjectSummary;
use forge_ports::{ProjectRepositoryError, ProjectRepositoryPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::sessions::ClaudeSessionAdapter;

// How many lines of one transcript are searched for a `cwd`. Real transcripts carry one within
// their first few lines; the bound only stops a pathological file from being read in full.
const MAX_CWD_SCAN_LINES: usize = 60;

pub struct ClaudeProjectAdapter {
    claude_home: PathBuf,
}

impl ClaudeProjectAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[async_trait]
impl ProjectRepositoryPort for ClaudeProjectAdapter {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError> {
        let projects_dir = self.claude_home.join("projects");
        let mut entries = match tokio::fs::read_dir(&projects_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(ProjectRepositoryError::ListFailed(format!(
                    "{}: {}",
                    projects_dir.display(),
                    e
                )))
            }
        };
        let sessions = ClaudeSessionAdapter::new(self.claude_home.clone());

        let mut found: Vec<(SystemTime, ProjectSummary)> = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ProjectRepositoryError::ListFailed(format!("{}: {}", projects_dir.display(), e))
        })? {
            let dir = entry.path();
            let Ok(metadata) = tokio::fs::metadata(&dir).await else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            let transcripts = transcripts_newest_first(&dir).await;
            let Some(&(newest, _)) = transcripts.first() else {
                continue;
            };
            let Ok(last_active) = time::OffsetDateTime::from(newest).format(&Rfc3339) else {
                continue;
            };
            let Some(path) = recover_cwd(&transcripts).await else {
                continue;
            };

            // Keep a directory only if `ClaudeSessionAdapter` finds it again from the recovered
            // path, so `list_sessions(project.path)` always lists exactly this directory.
            if sessions.project_dir(&path).await != dir {
                continue;
            }

            let exists = tokio::fs::metadata(&path)
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            found.push((
                newest,
                ProjectSummary {
                    path,
                    last_active,
                    session_count: transcripts.len(),
                    exists,
                },
            ));
        }

        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
        Ok(found.into_iter().map(|(_, project)| project).collect())
    }
}

// Every `*.jsonl` file in `dir` with its modification time, newest first (ties broken by path so
// the order is deterministic). Unreadable entries are skipped, never an error.
async fn transcripts_newest_first(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
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

// The first `cwd` found, trying transcripts newest first.
async fn recover_cwd(transcripts: &[(SystemTime, PathBuf)]) -> Option<PathBuf> {
    for (_, path) in transcripts {
        if let Some(cwd) = read_cwd(path).await {
            return Some(cwd);
        }
    }
    None
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

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    use forge_domain::SessionId;
    use forge_ports::SessionRepositoryPort;

    use super::*;
    use crate::sessions::ClaudeSessionAdapter;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn cwd_line(cwd: &Path) -> String {
        serde_json::json!({
            "type": "user",
            "cwd": cwd.to_string_lossy(),
            "message": { "role": "user", "content": "hi" }
        })
        .to_string()
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    // Writes `<home>/projects/<dir_name>/<id>.jsonl`.
    fn write_transcript_in(home: &Path, dir_name: &str, id: &str, content: &[u8], modified: SystemTime) {
        let dir = home.join("projects").join(dir_name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(&path, content).unwrap();
        set_modified(&path, modified);
    }

    // Writes a transcript into the directory Claude Code would use for `project`.
    async fn add_transcript(home: &Path, project: &Path, id: &str, content: &[u8], modified: SystemTime) {
        let dir = ClaudeSessionAdapter::new(home.to_path_buf()).project_dir(project).await;
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        write_transcript_in(home, &name, id, content, modified);
    }

    async fn list(home: &Path) -> Result<Vec<ProjectSummary>, ProjectRepositoryError> {
        ClaudeProjectAdapter::new(home.to_path_buf()).list_projects().await
    }

    #[tokio::test]
    async fn list_projects_returns_empty_when_the_projects_directory_is_missing() {
        let home = tempfile::tempdir().unwrap();

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_recovers_the_real_path_from_the_cwd_not_the_lossy_directory_name() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/my_app one");
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(10)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/my_app one"));
        assert_eq!(projects[0].session_count, 1);
        assert!(!projects[0].exists);
    }

    #[tokio::test]
    async fn list_projects_formats_last_active_as_rfc3339_of_the_newest_transcript() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "old", cwd_line(project).as_bytes(), at(0)).await;
        add_transcript(home.path(), project, "new", cwd_line(project).as_bytes(), at(86_400)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].last_active, "1970-01-02T00:00:00Z");
        assert_eq!(projects[0].session_count, 2);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_with_no_jsonl_files_and_ignores_non_transcript_files() {
        let home = tempfile::tempdir().unwrap();
        let empty = home.path().join("projects").join("-work-empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("notes.txt"), "not a transcript").unwrap();
        std::fs::write(home.path().join("projects").join("stray-file"), "x").unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(1)).await;
        std::fs::write(
            ClaudeSessionAdapter::new(home.path().to_path_buf())
                .project_dir(project)
                .await
                .join("readme.md"),
            "ignored",
        )
        .unwrap();

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].session_count, 1);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_whose_transcripts_have_no_cwd() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(
            home.path(),
            project,
            "s-1",
            br#"{"type":"queue-operation","operation":"enqueue"}"#,
            at(1),
        )
        .await;

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_finds_a_cwd_that_is_not_on_the_first_line() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let content = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"queue-operation"}"#,
            r#"{"type":"attachment"}"#,
            cwd_line(project)
        );
        add_transcript(home.path(), project, "s-1", content.as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_uses_the_newest_transcripts_cwd_when_transcripts_disagree() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let dir_name = ClaudeSessionAdapter::new(home.path().to_path_buf())
            .project_dir(project)
            .await
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        write_transcript_in(home.path(), &dir_name, "older", cwd_line(Path::new("/work/b")).as_bytes(), at(100));
        write_transcript_in(home.path(), &dir_name, "newer", cwd_line(project).as_bytes(), at(200));

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_falls_back_to_an_older_transcript_when_the_newest_has_no_cwd() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "older", cwd_line(project).as_bytes(), at(100)).await;
        add_transcript(home.path(), project, "newer", br#"{"type":"attachment"}"#, at(200)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_tolerates_invalid_utf8_and_a_partial_final_line() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let mut content: Vec<u8> = vec![0xff, 0xfe, b'\n'];
        content.extend_from_slice(cwd_line(project).as_bytes());
        content.extend_from_slice(b"\n{\"type\":\"user\",\"cwd\":\"/wor");
        add_transcript(home.path(), project, "s-1", &content, at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_does_not_search_past_the_scan_bound() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let mut content = String::new();
        for _ in 0..MAX_CWD_SCAN_LINES {
            content.push_str("{\"type\":\"attachment\"}\n");
        }
        content.push_str(&cwd_line(project));
        add_transcript(home.path(), project, "s-1", content.as_bytes(), at(1)).await;

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_sorts_newest_first_and_breaks_ties_by_path() {
        let home = tempfile::tempdir().unwrap();
        for (name, when) in [("/work/b", 100), ("/work/a", 100), ("/work/c", 300)] {
            let project = Path::new(name);
            add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(when)).await;
        }

        let paths: Vec<PathBuf> = list(home.path()).await.unwrap().into_iter().map(|p| p.path).collect();

        assert_eq!(
            paths,
            vec![PathBuf::from("/work/c"), PathBuf::from("/work/a"), PathBuf::from("/work/b")]
        );
    }

    #[tokio::test]
    async fn list_projects_reports_exists_for_a_real_directory() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let cwd = project_dir.path().canonicalize().unwrap();
        add_transcript(home.path(), &cwd, "s-1", cwd_line(&cwd).as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, cwd);
        assert!(projects[0].exists);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_that_does_not_round_trip_to_its_recovered_path() {
        let home = tempfile::tempdir().unwrap();
        write_transcript_in(
            home.path(),
            "-some-other-name",
            "s-1",
            cwd_line(Path::new("/work/a")).as_bytes(),
            at(1),
        );

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_keeps_a_project_whose_directory_name_hit_the_200_character_cap() {
        let home = tempfile::tempdir().unwrap();
        let long = format!("/work/{}", "d".repeat(250));
        let project = Path::new(&long);
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from(&long));
    }

    #[tokio::test]
    async fn list_projects_round_trips_with_list_sessions_for_the_recovered_path() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let cwd = project_dir.path().canonicalize().unwrap();
        add_transcript(home.path(), &cwd, "s-1", cwd_line(&cwd).as_bytes(), at(1)).await;
        add_transcript(home.path(), &cwd, "s-2", cwd_line(&cwd).as_bytes(), at(2)).await;

        let projects = list(home.path()).await.unwrap();
        let sessions = ClaudeSessionAdapter::new(home.path().to_path_buf())
            .list_sessions(&projects[0].path)
            .await
            .unwrap();

        let ids: Vec<SessionId> = sessions.into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![SessionId("s-2".to_string()), SessionId("s-1".to_string())]);
        assert_eq!(projects[0].session_count, 2);
    }
}
