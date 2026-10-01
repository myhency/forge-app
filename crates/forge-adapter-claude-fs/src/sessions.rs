use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{
    Message, MessageRole, PermissionMode, Session, SessionId, SessionSummary, ToolCall, ToolResult,
};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort};
use crate::trash::{SystemTrash, TrashBin};
use crate::usage::UsageCollector;
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt, AsyncWriteExt, BufReader};

pub struct ClaudeSessionAdapter {
    claude_home: PathBuf,
    trash: Arc<dyn TrashBin>,
}

impl ClaudeSessionAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self::with_trash(claude_home, Arc::new(SystemTrash))
    }

    pub fn with_trash(claude_home: PathBuf, trash: Arc<dyn TrashBin>) -> Self {
        Self { claude_home, trash }
    }

    // Claude Code names the directory after its resolved working directory (symlinks resolved,
    // on-disk Unicode form), so resolve the caller's path the same way; a path that no longer
    // exists is used as given.
    pub(crate) async fn project_dir(&self, project_path: &Path) -> PathBuf {
        let resolved = tokio::fs::canonicalize(project_path)
            .await
            .unwrap_or_else(|_| project_path.to_path_buf());
        self.claude_home
            .join("projects")
            .join(escape_project_path(&resolved))
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TitleKind {
    Auto,
    Custom,
}

fn title_from_value(value: &Value) -> Option<(TitleKind, String)> {
    let (kind, key) = match value.get("type")?.as_str()? {
        "ai-title" => (TitleKind::Auto, "aiTitle"),
        "custom-title" => (TitleKind::Custom, "customTitle"),
        _ => return None,
    };
    Some((kind, value.get(key)?.as_str()?.to_string()))
}

fn title_from_line(line: &str) -> Option<(TitleKind, String)> {
    if !line.contains("-title\"") {
        return None;
    }
    title_from_value(&serde_json::from_str::<Value>(line).ok()?)
}

// The last title of each kind; a title the user chose wins over one Claude wrote, wherever each sits.
#[derive(Default)]
struct TitleTracker {
    auto: Option<String>,
    custom: Option<String>,
}

impl TitleTracker {
    fn see(&mut self, kind: TitleKind, title: String) {
        match kind {
            TitleKind::Auto => self.auto = Some(title),
            TitleKind::Custom => self.custom = Some(title),
        }
    }

    fn title(self) -> Option<String> {
        self.custom.or(self.auto)
    }
}

async fn read_title(path: &Path) -> Option<String> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    let mut titles = TitleTracker::default();
    loop {
        buffer.clear();
        // A read error keeps the title found so far; an invalid-UTF-8 line is skipped.
        match reader.read_until(b'\n', &mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        if let Some((kind, found)) = std::str::from_utf8(&buffer).ok().and_then(title_from_line) {
            titles.see(kind, found);
        }
    }
    titles.title()
}

fn parse_permission_mode(raw: &str) -> PermissionMode {
    match raw {
        "acceptEdits" => PermissionMode::AcceptEdits,
        "bypassPermissions" => PermissionMode::BypassPermissions,
        "plan" => PermissionMode::Plan,
        _ => PermissionMode::Default,
    }
}

fn flatten_tool_result_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn push_user_message(session: &mut Session, text: String) {
    session.messages.push(Message {
        role: MessageRole::User,
        content: text,
        tool_calls: Vec::new(),
    });
}

fn attach_tool_result(session: &mut Session, block: &Value) {
    let Some(tool_use_id) = block.get("tool_use_id").and_then(Value::as_str) else {
        return;
    };
    let result = ToolResult {
        output: flatten_tool_result_content(block.get("content")),
        is_error: block
            .get("is_error")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    'search: for message in session.messages.iter_mut().rev() {
        for tool_call in message.tool_calls.iter_mut() {
            if tool_call.id == tool_use_id {
                tool_call.result = Some(result);
                break 'search;
            }
        }
    }
}

fn apply_user_line(session: &mut Session, line: &Value) {
    let Some(content) = line.pointer("/message/content") else {
        return;
    };
    match content {
        Value::String(text) => push_user_message(session, text.clone()),
        Value::Array(blocks) => {
            let mut texts: Vec<&str> = Vec::new();
            for block in blocks {
                match block.get("type").and_then(Value::as_str) {
                    Some("tool_result") => attach_tool_result(session, block),
                    Some("text") => {
                        if let Some(text) = block.get("text").and_then(Value::as_str) {
                            texts.push(text);
                        }
                    }
                    _ => {}
                }
            }
            if !texts.is_empty() {
                push_user_message(session, texts.join("\n"));
            }
        }
        _ => {}
    }
}

fn apply_assistant_line(session: &mut Session, line: &Value) {
    let Some(blocks) = line.pointer("/message/content").and_then(Value::as_array) else {
        return;
    };
    let mut texts: Vec<&str> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for block in blocks {
        match block.get("type").and_then(Value::as_str) {
            Some("text") => {
                if let Some(text) = block.get("text").and_then(Value::as_str) {
                    texts.push(text);
                }
            }
            Some("tool_use") => {
                if let (Some(id), Some(name)) = (
                    block.get("id").and_then(Value::as_str),
                    block.get("name").and_then(Value::as_str),
                ) {
                    tool_calls.push(ToolCall {
                        id: id.to_string(),
                        name: name.to_string(),
                        input: block.get("input").cloned().unwrap_or(Value::Null),
                        result: None,
                    });
                }
            }
            _ => {}
        }
    }

    let text = texts.join("\n\n");
    if text.is_empty() && tool_calls.is_empty() {
        return;
    }
    if !matches!(session.messages.last(), Some(m) if m.role == MessageRole::Assistant) {
        session.messages.push(Message::new(MessageRole::Assistant));
    }
    let message = session
        .messages
        .last_mut()
        .expect("an assistant message was just ensured");
    if !text.is_empty() {
        if !message.content.is_empty() {
            message.content.push_str("\n\n");
        }
        message.content.push_str(&text);
    }
    message.tool_calls.extend(tool_calls);
}

fn apply_line(session: &mut Session, line: &Value) {
    let flagged = |key: &str| line.get(key).and_then(Value::as_bool) == Some(true);
    if flagged("isSidechain") || flagged("isMeta") {
        return;
    }
    match line.get("type").and_then(Value::as_str) {
        Some("user") => apply_user_line(session, line),
        Some("assistant") => apply_assistant_line(session, line),
        Some("permission-mode") => {
            if let Some(mode) = line.get("permissionMode").and_then(Value::as_str) {
                session.permission_mode = parse_permission_mode(mode);
            }
        }
        _ => {}
    }
}

// A session id names a file in the project directory; anything that could point elsewhere is not one.
fn valid_session_id(id: &str) -> bool {
    // A leading dot is refused too: a file named `..jsonl` has the stem `.`, and `<dir>/.` is the
    // project directory itself.
    !(id.is_empty() || id.starts_with('.') || id.contains(['/', '\\']) || id.contains(".."))
}

async fn lacks_trailing_newline(file: &mut tokio::fs::File) -> std::io::Result<bool> {
    if file.metadata().await?.len() == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1)).await?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last).await?;
    Ok(last[0] != b'\n')
}

fn rename_error(path: &Path, e: std::io::Error) -> SessionRepositoryError {
    SessionRepositoryError::RenameFailed(format!("{}: {e}", path.display()))
}

#[async_trait]
impl SessionRepositoryPort for ClaudeSessionAdapter {
    async fn list_sessions(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SessionSummary>, SessionRepositoryError> {
        let dir = self.project_dir(project_path).await;
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
            let title = read_title(&path).await;
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
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<Session, SessionRepositoryError> {
        let id = &session_id.0;
        if !valid_session_id(id) {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }

        let path = self
            .project_dir(project_path)
            .await
            .join(format!("{id}.jsonl"));
        let file = match tokio::fs::File::open(&path).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionRepositoryError::NotFound(session_id.clone()))
            }
            Err(e) => {
                return Err(SessionRepositoryError::LoadFailed(format!(
                    "{}: {}",
                    path.display(),
                    e
                )))
            }
        };

        let mut titles = TitleTracker::default();
        let mut usage = UsageCollector::default();
        let mut session = Session {
            id: session_id.clone(),
            project_path: project_path.to_path_buf(),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: forge_domain::SessionUsage::default(),
        };
        let mut reader = BufReader::new(file);
        let mut buffer = Vec::new();
        let mut malformed_line: Option<usize> = None;
        let mut line_number = 0usize;
        loop {
            buffer.clear();
            let bytes_read = reader.read_until(b'\n', &mut buffer).await.map_err(|e| {
                SessionRepositoryError::LoadFailed(format!("{}: {}", path.display(), e))
            })?;
            if bytes_read == 0 {
                break;
            }
            line_number += 1;
            let text = std::str::from_utf8(&buffer);
            if matches!(text, Ok(line) if line.trim().is_empty()) {
                continue;
            }
            if let Some(bad_line) = malformed_line {
                return Err(SessionRepositoryError::LoadFailed(format!(
                    "{}: malformed JSON on line {}",
                    path.display(),
                    bad_line
                )));
            }
            match text
                .ok()
                .and_then(|line| serde_json::from_str::<Value>(line).ok())
            {
                Some(value) => {
                    if let Some((kind, found)) = title_from_value(&value) {
                        titles.see(kind, found);
                    }
                    usage.see(&value);
                    apply_line(&mut session, &value)
                }
                None => malformed_line = Some(line_number),
            }
        }
        session.title = titles.title();
        session.usage = usage.finish();
        Ok(session)
    }

    async fn rename_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<(), SessionRepositoryError> {
        let id = &session_id.0;
        if !valid_session_id(id) {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }
        let path = self
            .project_dir(project_path)
            .await
            .join(format!("{id}.jsonl"));
        // Append mode, and no `create`: a rename must never conjure a transcript.
        let mut file = match tokio::fs::OpenOptions::new()
            .read(true)
            .append(true)
            .open(&path)
            .await
        {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionRepositoryError::NotFound(session_id.clone()))
            }
            Err(e) => return Err(rename_error(&path, e)),
        };

        let mut entry = String::new();
        if lacks_trailing_newline(&mut file)
            .await
            .map_err(|e| rename_error(&path, e))?
        {
            entry.push('\n');
        }
        entry.push_str(
            &serde_json::json!({"type": "custom-title", "customTitle": title, "sessionId": id})
                .to_string(),
        );
        entry.push('\n');
        // The whole entry goes out in one `write_all` on a file opened in append mode (O_APPEND), so
        // it is written in a single write at the end of the file.
        file.write_all(entry.as_bytes())
            .await
            .map_err(|e| rename_error(&path, e))?;
        file.flush().await.map_err(|e| rename_error(&path, e))
    }

    async fn delete_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<(), SessionRepositoryError> {
        let id = &session_id.0;
        if !valid_session_id(id) {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }
        let dir = self.project_dir(project_path).await;
        let transcript = dir.join(format!("{id}.jsonl"));
        if !tokio::fs::try_exists(&transcript).await.unwrap_or(false) {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }
        // The data Claude Code keeps beside a transcript (sub-agent transcripts, tool results) is
        // the session's own, so it goes with it. Anything else is left alone.
        let sibling = dir.join(id);
        let has_sibling = tokio::fs::metadata(&sibling).await.map(|m| m.is_dir()).unwrap_or(false);

        let trash = self.trash.clone();
        let outcome = tokio::task::spawn_blocking(move || {
            trash.trash(&transcript).map_err(|e| format!("{}: {e}", transcript.display()))?;
            if has_sibling {
                trash.trash(&sibling).map_err(|e| {
                    format!(
                        "the session transcript was moved to the trash, but its data folder could not be: {}: {e}",
                        sibling.display()
                    )
                })?;
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| SessionRepositoryError::DeleteFailed(e.to_string()))?;
        outcome.map_err(SessionRepositoryError::DeleteFailed)
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

    fn custom_title(title: &str) -> Value {
        json!({"type": "custom-title", "customTitle": title, "sessionId": "s"})
    }

    #[tokio::test]
    async fn rename_appends_a_custom_title_line_that_list_and_load_read_back() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[user_text("hi"), ai_title("Auto")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        adapter.rename_session(&project(), &SessionId("s1".to_string()), "Mine").await.unwrap();

        let listed = adapter.list_sessions(&project()).await.unwrap();
        assert_eq!(listed[0].title.as_deref(), Some("Mine"));
        let loaded = adapter.load_session(&project(), &SessionId("s1".to_string())).await.unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Mine"));
        assert_eq!(loaded.messages.len(), 1, "the history is untouched");
    }

    #[tokio::test]
    async fn rename_writes_one_json_line_with_the_documented_fields() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        adapter.rename_session(&project(), &SessionId("s1".to_string()), "Mine").await.unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        let last = text.lines().last().unwrap();
        let value: Value = serde_json::from_str(last).unwrap();
        assert_eq!(value, json!({"type": "custom-title", "customTitle": "Mine", "sessionId": "s1"}));
        assert!(text.ends_with('\n'));
    }

    #[tokio::test]
    async fn rename_with_awkward_characters_stays_one_valid_line() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());
        let title = "a \"quoted\" \\ back\tslash\nnext line 한글";

        adapter.rename_session(&project(), &SessionId("s1".to_string()), title).await.unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(text.lines().count(), 2);
        let listed = adapter.list_sessions(&project()).await.unwrap();
        assert_eq!(listed[0].title.as_deref(), Some(title));
    }

    #[tokio::test]
    async fn rename_starts_on_a_new_line_when_the_file_has_no_trailing_newline() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let without_newline = std::fs::read_to_string(&path).unwrap().trim_end().to_string();
        std::fs::write(&path, &without_newline).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        adapter.rename_session(&project(), &SessionId("s1".to_string()), "Mine").await.unwrap();

        let loaded = adapter.load_session(&project(), &SessionId("s1".to_string())).await.unwrap();
        assert_eq!(loaded.title.as_deref(), Some("Mine"));
        assert_eq!(loaded.messages.len(), 1, "the earlier last line still parses");
    }

    #[tokio::test]
    async fn rename_of_an_empty_file_writes_just_the_title_line() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        adapter.rename_session(&project(), &SessionId("s1".to_string()), "Mine").await.unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1);
    }

    #[tokio::test]
    async fn rename_of_a_missing_session_is_not_found_and_creates_nothing() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "other", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let result = adapter.rename_session(&project(), &SessionId("missing".to_string()), "Mine").await;

        assert!(matches!(result, Err(SessionRepositoryError::NotFound(id)) if id.0 == "missing"));
        let dir = home.path().join("projects").join(escape_project_path(&project()));
        assert!(!dir.join("missing.jsonl").exists(), "rename must never create a transcript");
    }

    #[tokio::test]
    async fn rename_refuses_ids_that_could_leave_the_project_directory() {
        let home = tempfile::tempdir().unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());
        for bad in ["", "../x", "a/b", "a\\b", ".."] {
            let result = adapter.rename_session(&project(), &SessionId(bad.to_string()), "Mine").await;
            assert!(matches!(result, Err(SessionRepositoryError::NotFound(_))), "{bad:?}");
        }
    }

    #[tokio::test]
    async fn a_custom_title_beats_an_ai_title_wherever_it_sits() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[custom_title("Mine"), user_text("hi"), ai_title("Auto written later")],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let listed = adapter.list_sessions(&project()).await.unwrap();
        let loaded = adapter.load_session(&project(), &SessionId("s1".to_string())).await.unwrap();

        assert_eq!(listed[0].title.as_deref(), Some("Mine"));
        assert_eq!(loaded.title.as_deref(), Some("Mine"));
    }

    #[tokio::test]
    async fn the_last_custom_title_wins_among_custom_titles() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[custom_title("One"), custom_title("Two")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let listed = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(listed[0].title.as_deref(), Some("Two"));
    }

    fn set_mtime(path: &Path, seconds_since_epoch: u64) {
        let file = std::fs::OpenOptions::new().write(true).open(path).unwrap();
        file.set_modified(UNIX_EPOCH + Duration::from_secs(seconds_since_epoch))
            .unwrap();
    }

    fn assistant_block(block: Value) -> Value {
        json!({"type": "assistant", "isSidechain": false, "message": {"role": "assistant", "content": [block]}})
    }

    fn user_blocks(blocks: Value) -> Value {
        json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": blocks}})
    }

    async fn load(home: &Path, id: &str) -> Result<Session, SessionRepositoryError> {
        ClaudeSessionAdapter::new(home.to_path_buf())
            .load_session(&project(), &SessionId(id.to_string()))
            .await
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

    #[tokio::test]
    async fn load_session_returns_not_found_for_a_missing_file() {
        let home = tempfile::tempdir().unwrap();

        let result = load(home.path(), "nope").await;

        assert!(matches!(result, Err(SessionRepositoryError::NotFound(id)) if id.0 == "nope"));
    }

    #[tokio::test]
    async fn load_session_rejects_ids_that_could_escape_the_project_directory() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "legit", &[user_text("hi")]);
        std::fs::write(
            home.path().join("projects").join("outside.jsonl"),
            format!("{}\n", user_text("secret")),
        )
        .unwrap();

        for bad_id in ["../outside", "a/b", "a\\b", "..", ""] {
            let result = load(home.path(), bad_id).await;
            assert!(
                matches!(result, Err(SessionRepositoryError::NotFound(_))),
                "id {bad_id:?} should be rejected, got {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn load_session_maps_a_plain_user_and_assistant_exchange() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_text("hi"), assistant_block(json!({"type": "text", "text": "hello"}))],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.id, SessionId("s1".to_string()));
        assert_eq!(session.project_path, project());
        assert_eq!(session.title, None);
        assert_eq!(session.permission_mode, PermissionMode::Default);
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[0].content, "hi");
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].content, "hello");
        assert!(session.messages[1].tool_calls.is_empty());
    }

    #[tokio::test]
    async fn load_session_merges_an_assistant_response_split_one_block_per_line() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("read it"),
                assistant_block(json!({"type": "thinking", "thinking": "hmm"})),
                assistant_block(json!({"type": "text", "text": "Let me look."})),
                assistant_block(json!({
                    "type": "tool_use", "id": "t-1", "name": "Read",
                    "input": {"path": "a.txt"}, "caller": {"type": "direct"}
                })),
                user_blocks(json!([{"type": "tool_result", "tool_use_id": "t-1", "content": "file body"}])),
                assistant_block(json!({"type": "thinking", "thinking": "ok"})),
                assistant_block(json!({"type": "text", "text": "Done."})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 2);
        let assistant = &session.messages[1];
        assert_eq!(assistant.role, MessageRole::Assistant);
        assert_eq!(assistant.content, "Let me look.\n\nDone.");
        assert_eq!(assistant.tool_calls.len(), 1);
        assert_eq!(assistant.tool_calls[0].id, "t-1");
        assert_eq!(assistant.tool_calls[0].name, "Read");
        assert_eq!(assistant.tool_calls[0].input, json!({"path": "a.txt"}));
        assert_eq!(
            assistant.tool_calls[0].result,
            Some(ToolResult { output: "file body".to_string(), is_error: false })
        );
    }

    #[tokio::test]
    async fn load_session_attaches_string_and_block_list_tool_results_and_defaults_is_error_to_false() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("go"),
                assistant_block(json!({"type": "tool_use", "id": "t-1", "name": "Bash", "input": {}})),
                assistant_block(json!({"type": "tool_use", "id": "t-2", "name": "Search", "input": {}})),
                user_blocks(json!([
                    {"type": "tool_result", "tool_use_id": "t-1", "content": "boom", "is_error": true},
                    {"type": "tool_result", "tool_use_id": "t-2", "content": [
                        {"type": "text", "text": "a"},
                        {"type": "tool_reference", "tool_name": "X"},
                        {"type": "text", "text": "b"}
                    ]}
                ])),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        let calls = &session.messages[1].tool_calls;
        assert_eq!(
            calls[0].result,
            Some(ToolResult { output: "boom".to_string(), is_error: true })
        );
        assert_eq!(
            calls[1].result,
            Some(ToolResult { output: "a\nb".to_string(), is_error: false })
        );
    }

    #[tokio::test]
    async fn load_session_ignores_a_tool_result_with_an_unknown_id() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("go"),
                assistant_block(json!({"type": "tool_use", "id": "t-1", "name": "Bash", "input": {}})),
                user_blocks(json!([{"type": "tool_result", "tool_use_id": "missing", "content": "x"}])),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 2);
        assert!(session.messages[1].tool_calls[0].result.is_none());
    }

    #[tokio::test]
    async fn load_session_turns_text_blocks_in_a_user_list_into_a_user_message() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_blocks(json!([
                {"type": "text", "text": "one"},
                {"type": "text", "text": "two"}
            ]))],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[0].content, "one\ntwo");
    }

    #[tokio::test]
    async fn load_session_starts_a_new_assistant_message_after_a_user_message() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("q1"),
                assistant_block(json!({"type": "text", "text": "a1"})),
                user_text("q2"),
                assistant_block(json!({"type": "text", "text": "a2"})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        let roles: Vec<MessageRole> = session.messages.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            vec![MessageRole::User, MessageRole::Assistant, MessageRole::User, MessageRole::Assistant]
        );
        let contents: Vec<&str> = session.messages.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["q1", "a1", "q2", "a2"]);
    }

    #[tokio::test]
    async fn load_session_uses_the_last_permission_mode_and_falls_back_to_default() {
        let home = tempfile::tempdir().unwrap();
        let mode_line = |mode: &str| json!({"type": "permission-mode", "permissionMode": mode, "sessionId": "s"});
        let cases = [
            (vec![mode_line("acceptEdits")], PermissionMode::AcceptEdits),
            (vec![mode_line("bypassPermissions")], PermissionMode::BypassPermissions),
            (vec![mode_line("plan")], PermissionMode::Plan),
            (vec![mode_line("default")], PermissionMode::Default),
            (vec![mode_line("auto")], PermissionMode::Default),
            (vec![mode_line("acceptEdits"), mode_line("plan")], PermissionMode::Plan),
            (vec![user_text("no mode line")], PermissionMode::Default),
        ];

        for (index, (lines, expected)) in cases.iter().enumerate() {
            let id = format!("case-{index}");
            write_session(home.path(), &project(), &id, lines);
            let session = load(home.path(), &id).await.unwrap();
            assert_eq!(session.permission_mode, *expected, "case {index}");
        }
    }

    #[tokio::test]
    async fn load_session_reads_the_last_title() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "ai",
            &[ai_title("First"), user_text("x"), ai_title("Second")],
        );
        write_session(
            home.path(),
            &project(),
            "custom",
            &[json!({"type": "custom-title", "customTitle": "Mine", "sessionId": "s"})],
        );

        assert_eq!(load(home.path(), "ai").await.unwrap().title, Some("Second".to_string()));
        assert_eq!(load(home.path(), "custom").await.unwrap().title, Some("Mine".to_string()));
    }

    #[tokio::test]
    async fn load_session_ignores_unrelated_line_types_blank_lines_and_sidechain_lines() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(
            home.path(),
            &project(),
            "s1",
            &[
                json!({"type": "mode", "mode": "normal", "sessionId": "s"}),
                json!({"type": "atis-latch", "atis": {}, "sessionId": "s"}),
                user_text("real question"),
                json!({"type": "attachment", "attachment": {"type": "hook_success"}, "sessionId": "s"}),
                json!({"type": "system", "subtype": "hook", "sessionId": "s"}),
                json!({"type": "user", "isSidechain": true, "message": {"role": "user", "content": "side thread"}}),
                json!({"type": "last-prompt", "leafUuid": "u", "sessionId": "s"}),
                assistant_block(json!({"type": "text", "text": "real answer"})),
                json!({"type": "cost-state", "totalCostUSD": 0.1, "sessionId": "s"}),
            ],
        );
        let mut body = std::fs::read_to_string(&path).unwrap();
        body.push_str("\n   \n");
        std::fs::write(&path, body).unwrap();

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].content, "real question");
        assert_eq!(session.messages[1].content, "real answer");
    }

    #[tokio::test]
    async fn load_session_ignores_a_malformed_final_line() {
        let home = tempfile::tempdir().unwrap();
        for (index, suffix) in ["{\"type\":\"assistant\",\"mess", "{\"type\":\"assistant\",\"mess\n \n"]
            .iter()
            .enumerate()
        {
            let id = format!("partial-{index}");
            let path = write_session(home.path(), &project(), &id, &[user_text("hi")]);
            let mut body = std::fs::read_to_string(&path).unwrap();
            body.push_str(suffix);
            std::fs::write(&path, body).unwrap();

            let session = load(home.path(), &id).await.unwrap();

            assert_eq!(session.messages.len(), 1, "suffix {index}");
        }
    }

    #[tokio::test]
    async fn load_session_fails_on_a_malformed_line_in_the_middle() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[]);
        std::fs::write(&path, format!("{}\nnot json\n{}\n", user_text("a"), user_text("b"))).unwrap();

        let result = load(home.path(), "s1").await;

        match result {
            Err(SessionRepositoryError::LoadFailed(message)) => {
                assert!(message.contains("line 2"), "unexpected message: {message}");
            }
            other => panic!("expected LoadFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn load_session_ignores_a_final_line_cut_off_inside_a_multibyte_character() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(b"{\"type\":\"user\",\"message\":{\"content\":\"");
        // The first two bytes of a three-byte character: a write cut off mid-character.
        bytes.extend_from_slice(&[0xE3, 0x81]);
        std::fs::write(&path, bytes).unwrap();

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.messages.len(), 1);
    }

    #[tokio::test]
    async fn load_session_fails_on_an_invalid_utf8_line_in_the_middle() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[]);
        let mut bytes = format!("{}\n", user_text("a")).into_bytes();
        bytes.extend_from_slice(&[0xff, 0xfe, b'\n']);
        bytes.extend_from_slice(format!("{}\n", user_text("b")).as_bytes());
        std::fs::write(&path, bytes).unwrap();

        let result = load(home.path(), "s1").await;

        match result {
            Err(SessionRepositoryError::LoadFailed(message)) => {
                assert!(message.contains("line 2"), "unexpected message: {message}");
            }
            other => panic!("expected LoadFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn load_session_adds_nothing_for_thinking_only_assistant_lines() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("q"),
                assistant_block(json!({"type": "thinking", "thinking": "hmm"})),
                user_text("q2"),
                assistant_block(json!({"type": "thinking", "thinking": "more"})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        let roles: Vec<MessageRole> = session.messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, vec![MessageRole::User, MessageRole::User]);
        assert_eq!(session.messages[0].content, "q");
        assert_eq!(session.messages[1].content, "q2");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn sessions_are_found_through_a_symlinked_project_path() {
        let home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        let real = workspace.path().join("real-project");
        std::fs::create_dir(&real).unwrap();
        let link = workspace.path().join("linked-project");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        // Claude Code stores transcripts under the resolved path of the directory it ran in.
        let resolved = std::fs::canonicalize(&real).unwrap();
        write_session(home.path(), &resolved, "s1", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let listed = adapter.list_sessions(&link).await.unwrap();
        let loaded = adapter
            .load_session(&link, &SessionId("s1".to_string()))
            .await
            .unwrap();

        assert_eq!(listed.len(), 1);
        assert_eq!(loaded.messages.len(), 1);
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn sessions_are_found_when_the_caller_passes_a_differently_normalised_path() {
        // APFS keeps a folder's name in the Unicode form it was created with (Finder writes the
        // decomposed form) but finds it by either form, and Claude Code names the transcript
        // directory from the on-disk form that getcwd/realpath report.
        let decomposed = "\u{1111}\u{1173}\u{1105}\u{1169}\u{110C}\u{1166}\u{11A8}\u{1110}\u{1173}";
        let composed = "프로젝트";
        assert_ne!(decomposed, composed);
        let home = tempfile::tempdir().unwrap();
        let workspace = tempfile::tempdir().unwrap();
        std::fs::create_dir(workspace.path().join(decomposed)).unwrap();
        let on_disk = std::fs::canonicalize(workspace.path().join(decomposed)).unwrap();
        write_session(home.path(), &on_disk, "s1", &[user_text("hi")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let listed = adapter
            .list_sessions(&workspace.path().join(composed))
            .await
            .unwrap();

        assert_eq!(listed.len(), 1);
    }

    #[tokio::test]
    async fn load_session_skips_harness_injected_meta_lines() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("real question"),
                json!({"type": "user", "isMeta": true, "message": {"role": "user", "content":
                    "<local-command-caveat>Caveat: The messages below were generated by the user while running local commands.</local-command-caveat>"}}),
                json!({"type": "user", "isMeta": true, "message": {"role": "user", "content":
                    [{"type": "text", "text": "Base directory for this skill: /x/skills/foo\n\n# Foo"}]}}),
                json!({"type": "user", "isMeta": false, "message": {"role": "user", "content": "second question"}}),
                assistant_block(json!({"type": "text", "text": "real answer"})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        let contents: Vec<&str> = session.messages.iter().map(|m| m.content.as_str()).collect();
        assert_eq!(contents, vec!["real question", "second question", "real answer"]);
    }

    #[tokio::test]
    async fn list_sessions_keeps_the_title_of_a_transcript_whose_tail_is_cut_off_mid_character() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(
            home.path(),
            &project(),
            "live",
            &[ai_title("Live title"), user_text("hi")],
        );
        let mut bytes = std::fs::read(&path).unwrap();
        bytes.extend_from_slice(b"{\"type\":\"user\",\"message\":{\"content\":\"");
        bytes.extend_from_slice(&[0xE3, 0x81]);
        std::fs::write(&path, bytes).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions[0].title, Some("Live title".to_string()));
    }

    #[tokio::test]
    async fn list_sessions_skips_an_invalid_utf8_line_and_still_finds_a_later_title() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[]);
        let mut bytes = vec![0xff, 0xfe, b'\n'];
        bytes.extend_from_slice(format!("{}\n", ai_title("Found after garbage")).as_bytes());
        std::fs::write(&path, bytes).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let sessions = adapter.list_sessions(&project()).await.unwrap();

        assert_eq!(sessions[0].title, Some("Found after garbage".to_string()));
    }

    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct FakeTrash {
        trashed: Mutex<Vec<PathBuf>>,
        fail_on: Mutex<Option<PathBuf>>,
    }

    impl crate::trash::TrashBin for FakeTrash {
        fn trash(&self, path: &Path) -> Result<(), String> {
            if self.fail_on.lock().unwrap().as_deref() == Some(path) {
                return Err("the trash is unavailable".to_string());
            }
            self.trashed.lock().unwrap().push(path.to_path_buf());
            // Behave like the real thing: the path is gone afterwards.
            if path.is_dir() { std::fs::remove_dir_all(path) } else { std::fs::remove_file(path) }
                .map_err(|e| e.to_string())
        }
    }

    fn adapter_with(home: &Path) -> (ClaudeSessionAdapter, Arc<FakeTrash>) {
        let trash = Arc::new(FakeTrash::default());
        (ClaudeSessionAdapter::with_trash(home.to_path_buf(), trash.clone()), trash)
    }

    #[tokio::test]
    async fn delete_moves_the_transcript_to_the_trash_and_it_is_no_longer_listed() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        write_session(home.path(), &project(), "s2", &[user_text("keep")]);
        let (adapter, trash) = adapter_with(home.path());

        adapter.delete_session(&project(), &SessionId("s1".to_string())).await.unwrap();

        assert_eq!(*trash.trashed.lock().unwrap(), vec![path.clone()]);
        let listed = adapter.list_sessions(&project()).await.unwrap();
        assert_eq!(listed.iter().map(|s| s.id.0.as_str()).collect::<Vec<_>>(), vec!["s2"]);
    }

    #[tokio::test]
    async fn delete_also_trashes_the_sibling_directory_of_the_session() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let sibling = path.with_extension("");
        std::fs::create_dir_all(sibling.join("subagents")).unwrap();
        let other = path.with_file_name("s2");
        std::fs::create_dir_all(&other).unwrap();
        let (adapter, trash) = adapter_with(home.path());

        adapter.delete_session(&project(), &SessionId("s1".to_string())).await.unwrap();

        assert_eq!(*trash.trashed.lock().unwrap(), vec![path, sibling]);
        assert!(other.exists(), "another session's directory is not touched");
    }

    #[tokio::test]
    async fn delete_of_a_missing_session_is_not_found_and_trashes_nothing() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "other", &[user_text("hi")]);
        let (adapter, trash) = adapter_with(home.path());

        let result = adapter.delete_session(&project(), &SessionId("missing".to_string())).await;

        assert!(matches!(result, Err(SessionRepositoryError::NotFound(id)) if id.0 == "missing"));
        assert!(trash.trashed.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn delete_refuses_ids_that_could_leave_the_project_directory() {
        let home = tempfile::tempdir().unwrap();
        let outside = home.path().join("projects").join("precious.jsonl");
        std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
        std::fs::write(&outside, "x").unwrap();
        let (adapter, trash) = adapter_with(home.path());
        for bad in ["", "../precious", "a/b", "a\\b", ".."] {
            let result = adapter.delete_session(&project(), &SessionId(bad.to_string())).await;
            assert!(matches!(result, Err(SessionRepositoryError::NotFound(_))), "{bad:?}");
        }
        assert!(trash.trashed.lock().unwrap().is_empty());
        assert!(outside.exists());
    }

    #[tokio::test]
    async fn a_failing_trash_is_a_delete_error_and_the_session_stays_listed() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let (adapter, trash) = adapter_with(home.path());
        *trash.fail_on.lock().unwrap() = Some(path);

        let result = adapter.delete_session(&project(), &SessionId("s1".to_string())).await;

        assert!(matches!(&result, Err(SessionRepositoryError::DeleteFailed(m)) if m.contains("the trash is unavailable")), "{result:?}");
        assert_eq!(adapter.list_sessions(&project()).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn rename_and_delete_refuse_dot_ids_even_with_a_dot_dot_jsonl_file_present() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), ".", &[user_text("hi")]);
        assert!(path.ends_with("..jsonl") && path.exists());
        write_session(home.path(), &project(), ".hidden", &[user_text("hi")]);
        let (adapter, trash) = adapter_with(home.path());
        for bad in [".", ".hidden"] {
            let id = SessionId(bad.to_string());
            let deleted = adapter.delete_session(&project(), &id).await;
            assert!(matches!(deleted, Err(SessionRepositoryError::NotFound(_))), "delete {bad:?}");
            let renamed = adapter.rename_session(&project(), &id, "x").await;
            assert!(matches!(renamed, Err(SessionRepositoryError::NotFound(_))), "rename {bad:?}");
        }
        assert!(trash.trashed.lock().unwrap().is_empty());
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).unwrap().lines().count(), 1, "nothing appended");
    }

    #[tokio::test]
    async fn a_sibling_that_cannot_be_trashed_is_reported_as_a_partial_delete() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[user_text("hi")]);
        let sibling = path.with_extension("");
        std::fs::create_dir_all(&sibling).unwrap();
        let (adapter, trash) = adapter_with(home.path());
        *trash.fail_on.lock().unwrap() = Some(sibling.clone());

        let result = adapter.delete_session(&project(), &SessionId("s1".to_string())).await;

        let Err(SessionRepositoryError::DeleteFailed(message)) = result else { panic!("{result:?}") };
        assert!(message.contains("moved to the trash"), "{message}");
        assert!(message.contains(&sibling.display().to_string()), "{message}");
        assert_eq!(*trash.trashed.lock().unwrap(), vec![path.clone()]);
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn load_session_sums_usage_once_per_message_id() {
        let home = tempfile::tempdir().unwrap();
        let line = |id: &str, output: u64| {
            json!({"type": "assistant", "isSidechain": false, "message": {
                "id": id, "role": "assistant", "model": "claude-sonnet-4-6", "content": [{"type": "text", "text": "hi"}],
                "usage": {"input_tokens": 3, "output_tokens": output, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 0}
            }})
        };
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_text("hello"), line("m1", 10), line("m1", 10), line("m2", 20)],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let loaded = adapter.load_session(&project(), &SessionId("s1".to_string())).await.unwrap();

        let total = loaded.usage.total();
        assert_eq!((total.input, total.output, total.cache_read), (6, 30, 200));
        assert_eq!(loaded.usage.models().count(), 1);
    }
}
