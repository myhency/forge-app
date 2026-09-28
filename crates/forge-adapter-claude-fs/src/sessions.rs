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

const MAX_ESCAPED_LEN: usize = 200;

// Mirrors Claude Code's own project-directory naming: every character that is not an ASCII
// letter or digit becomes '-', and names over 200 characters are cut and given a hash suffix.
fn escape_project_path(project_path: &Path) -> String {
    let original = project_path.to_string_lossy();
    let escaped: String = original
        .encode_utf16()
        .map(|unit| match u8::try_from(unit) {
            Ok(byte) if byte.is_ascii_alphanumeric() => char::from(byte),
            _ => '-',
        })
        .collect();
    if escaped.len() <= MAX_ESCAPED_LEN {
        return escaped;
    }
    format!(
        "{}-{}",
        &escaped[..MAX_ESCAPED_LEN],
        to_base36(string_hash(&original).unsigned_abs())
    )
}

fn string_hash(text: &str) -> i32 {
    text.encode_utf16().fold(0i32, |hash, unit| {
        hash.wrapping_mul(31).wrapping_add(i32::from(unit))
    })
}

fn to_base36(mut value: u32) -> String {
    if value == 0 {
        return "0".to_string();
    }
    let mut digits = Vec::new();
    while value > 0 {
        digits.push(
            char::from_digit(value % 36, 36)
                .expect("a remainder below 36 is a valid base-36 digit"),
        );
        value /= 36;
    }
    digits.iter().rev().collect()
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

    #[test]
    fn escape_project_path_replaces_every_non_alphanumeric_character() {
        assert_eq!(
            escape_project_path(Path::new("/Users/james/my_project")),
            "-Users-james-my-project"
        );
        assert_eq!(
            escape_project_path(Path::new(
                "/Users/james/Library/Mobile Documents/com~apple~CloudDocs/notes"
            )),
            "-Users-james-Library-Mobile-Documents-com-apple-CloudDocs-notes"
        );
        assert_eq!(
            escape_project_path(Path::new("/Users/james/Documents/프로젝트")),
            "-Users-james-Documents-----"
        );
        // A character outside the BMP is two UTF-16 units, so it becomes two dashes.
        assert_eq!(
            escape_project_path(Path::new("/Users/test/emoji😀dir")),
            "-Users-test-emoji--dir"
        );
    }

    #[test]
    fn escape_project_path_caps_at_200_characters_and_appends_a_hash_of_the_original_path() {
        let exactly_200 = format!("/{}", "a".repeat(199));
        assert_eq!(
            escape_project_path(Path::new(&exactly_200)),
            format!("-{}", "a".repeat(199))
        );

        let just_over = format!("/{}", "a".repeat(200));
        assert_eq!(
            escape_project_path(Path::new(&just_over)),
            format!("-{}-b6ymvl", "a".repeat(199))
        );

        let long_ascii = format!("/{}", "a".repeat(250));
        assert_eq!(
            escape_project_path(Path::new(&long_ascii)),
            format!("-{}-feo44x", "a".repeat(199))
        );

        // The hash is over the original path's UTF-16 units, not its sanitized form.
        let long_hangul = format!("/{}/{}", "가".repeat(120), "b".repeat(100));
        assert_eq!(
            escape_project_path(Path::new(&long_hangul)),
            format!("{}{}-fmkleo", "-".repeat(122), "b".repeat(78))
        );
        let long_emoji = format!("/{}{}", "😀".repeat(60), "x".repeat(100));
        assert_eq!(
            escape_project_path(Path::new(&long_emoji)),
            format!("{}{}-pt4f1", "-".repeat(121), "x".repeat(79))
        );
    }

    #[tokio::test]
    async fn list_sessions_finds_transcripts_for_a_project_path_with_underscores_and_spaces() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("projects").join("-Users-test-my-project-x");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("s1.jsonl"), format!("{}\n", user_text("hi"))).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter
            .list_sessions(Path::new("/Users/test/my_project x"))
            .await
            .unwrap();

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, SessionId("s1".to_string()));
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
