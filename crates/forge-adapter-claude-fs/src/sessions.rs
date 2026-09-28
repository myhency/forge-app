use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{Session, SessionId, SessionSummary};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct ClaudeSessionAdapter {
    claude_home: PathBuf,
}

impl ClaudeSessionAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }

    fn project_dir(&self, project_path: &Path) -> PathBuf {
        self.claude_home
            .join("projects")
            .join(escape_project_path(project_path))
    }
}

fn escape_project_path(project_path: &Path) -> String {
    project_path
        .to_string_lossy()
        .chars()
        .map(|c| if c == '/' || c == '.' { '-' } else { c })
        .collect()
}

fn title_from_value(value: &Value) -> Option<String> {
    let key = match value.get("type")?.as_str()? {
        "ai-title" => "aiTitle",
        "custom-title" => "customTitle",
        _ => return None,
    };
    value.get(key)?.as_str().map(str::to_string)
}

fn title_from_line(line: &str) -> Option<String> {
    if !line.contains("-title\"") {
        return None;
    }
    title_from_value(&serde_json::from_str::<Value>(line).ok()?)
}

async fn read_title(path: &Path) -> std::io::Result<Option<String>> {
    let file = tokio::fs::File::open(path).await?;
    let mut lines = BufReader::new(file).lines();
    let mut title = None;
    while let Some(line) = lines.next_line().await? {
        if let Some(found) = title_from_line(&line) {
            title = Some(found);
        }
    }
    Ok(title)
}

#[async_trait]
impl SessionRepositoryPort for ClaudeSessionAdapter {
    async fn list_sessions(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SessionSummary>, SessionRepositoryError> {
        let dir = self.project_dir(project_path);
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(SessionRepositoryError::ListFailed(format!(
                    "{}: {}",
                    dir.display(),
                    e
                )))
            }
        };

        let mut found: Vec<(SystemTime, SessionSummary)> = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            SessionRepositoryError::ListFailed(format!("{}: {}", dir.display(), e))
        })? {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            let Some(id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let Ok(metadata) = tokio::fs::metadata(&path).await else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let Ok(updated_at) = time::OffsetDateTime::from(modified).format(&Rfc3339) else {
                continue;
            };
            let title = read_title(&path).await.unwrap_or(None);
            found.push((
                modified,
                SessionSummary {
                    id: SessionId(id),
                    title,
                    updated_at,
                },
            ));
        }

        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.0.cmp(&b.1.id.0)));
        Ok(found.into_iter().map(|(_, summary)| summary).collect())
    }

    async fn load_session(
        &self,
        _project_path: &Path,
        _session_id: &SessionId,
    ) -> Result<Session, SessionRepositoryError> {
        unimplemented!("implemented in Task 2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{Duration, UNIX_EPOCH};

    fn project() -> PathBuf {
        PathBuf::from("/Users/test/project")
    }

    fn write_session(home: &Path, project: &Path, id: &str, lines: &[Value]) -> PathBuf {
        let dir = home.join("projects").join(escape_project_path(project));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        let body: String = lines.iter().map(|line| format!("{line}\n")).collect();
        std::fs::write(&path, body).unwrap();
        path
    }

    fn user_text(text: &str) -> Value {
        json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": text}})
    }

    fn ai_title(title: &str) -> Value {
        json!({"type": "ai-title", "aiTitle": title, "sessionId": "s"})
    }

    fn set_mtime(path: &Path, seconds_since_epoch: u64) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(UNIX_EPOCH + Duration::from_secs(seconds_since_epoch))
            .unwrap();
    }

    #[test]
    fn escape_project_path_replaces_slashes_and_dots_with_dashes() {
        assert_eq!(
            escape_project_path(Path::new("/Users/james/Developer/forge")),
            "-Users-james-Developer-forge"
        );
        assert_eq!(
            escape_project_path(Path::new("/Users/james/.claude")),
            "-Users-james--claude"
        );
        assert_eq!(
            escape_project_path(Path::new(
                "/Users/james/Developer/forge/.claude/worktrees/forge-adapter-config"
            )),
            "-Users-james-Developer-forge--claude-worktrees-forge-adapter-config"
        );
    }

    #[tokio::test]
    async fn list_sessions_returns_empty_when_the_project_directory_does_not_exist() {
        let home = tempfile::tempdir().unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert!(sessions.is_empty());
    }

    #[tokio::test]
    async fn list_sessions_returns_one_summary_per_jsonl_file_with_id_and_rfc3339_updated_at() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "session-one", &[user_text("hi")]);
        write_session(home.path(), &project(), "session-two", &[user_text("hello")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        let mut ids: Vec<&str> = sessions.iter().map(|s| s.id.0.as_str()).collect();
        ids.sort();
        assert_eq!(ids, vec!["session-one", "session-two"]);
        for session in &sessions {
            assert!(
                session.updated_at.contains('T') && session.updated_at.ends_with('Z'),
                "not RFC 3339 UTC: {}",
                session.updated_at
            );
        }
    }

    #[tokio::test]
    async fn list_sessions_reads_the_title_from_the_last_title_line() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "ai",
            &[ai_title("First"), user_text("hi"), ai_title("Second")],
        );
        write_session(
            home.path(),
            &project(),
            "custom",
            &[json!({"type": "custom-title", "customTitle": "Mine", "sessionId": "s"})],
        );
        write_session(home.path(), &project(), "untitled", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        let title_of = |id: &str| sessions.iter().find(|s| s.id.0 == id).unwrap().title.clone();
        assert_eq!(title_of("ai"), Some("Second".to_string()));
        assert_eq!(title_of("custom"), Some("Mine".to_string()));
        assert_eq!(title_of("untitled"), None);
    }

    #[tokio::test]
    async fn list_sessions_ignores_entries_that_are_not_jsonl_files() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "real", &[user_text("hi")]);
        let dir = path.parent().unwrap();
        std::fs::create_dir(dir.join("memory")).unwrap();
        std::fs::write(dir.join("notes.txt"), "not a session").unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, SessionId("real".to_string()));
    }

    #[tokio::test]
    async fn list_sessions_sorts_newest_first_and_breaks_ties_by_id() {
        let home = tempfile::tempdir().unwrap();
        for (id, seconds) in [("old", 100), ("new", 300), ("tie-b", 200), ("tie-a", 200)] {
            let path = write_session(home.path(), &project(), id, &[user_text("hi")]);
            set_mtime(&path, seconds);
        }
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        let ids: Vec<&str> = sessions.iter().map(|s| s.id.0.as_str()).collect();
        assert_eq!(ids, vec!["new", "tie-a", "tie-b", "old"]);
    }

    #[tokio::test]
    async fn list_sessions_keeps_a_session_whose_transcript_is_not_valid_utf8_with_no_title() {
        let home = tempfile::tempdir().unwrap();
        let good = write_session(home.path(), &project(), "good", &[ai_title("Good title")]);
        std::fs::write(good.parent().unwrap().join("broken.jsonl"), [0xff, 0xfe, b'\n']).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions.len(), 2);
        let title_of = |id: &str| sessions.iter().find(|s| s.id.0 == id).unwrap().title.clone();
        assert_eq!(title_of("good"), Some("Good title".to_string()));
        assert_eq!(title_of("broken"), None);
    }

    #[tokio::test]
    async fn list_sessions_does_not_mistake_a_non_title_line_containing_the_prefilter_text_for_a_title() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[json!({"type": "user", "note": "my-title", "message": {"content": "x"}})],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions[0].title, None);
    }
}
