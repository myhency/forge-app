use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{Day, SessionId, SessionStats, StatsRow, TokenUsage, UsageStats, NO_MODEL};
use forge_ports::{UsageStatsError, UsageStatsPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};
use std::io::BufRead;

use crate::usage::token_usage;

/// Folds one transcript into rows of (day, hour, model), the way the desktop app's dashboard counts:
/// every user and assistant line is a message, and an assistant line adds its own `usage`. Claude Code
/// writes one line per content block, so a reply written on three lines counts three times, as it does
/// in the app. Sidechain and meta lines, messages Claude Code wrote itself (`<synthetic>`), lines
/// without a time that parses, and lines copied into a fork (they were counted in the original) are
/// skipped. A user line is a row with `NO_MODEL` and no tokens.
struct StatsCollector {
    offset: UtcOffset,
    rows: BTreeMap<(Day, u8, String), (u64, TokenUsage)>,
}

impl StatsCollector {
    fn new(offset: UtcOffset) -> StatsCollector {
        StatsCollector { offset, rows: BTreeMap::new() }
    }

    fn when(&self, line: &Value) -> Option<(Day, u8)> {
        let stamp = line.get("timestamp").and_then(Value::as_str)?;
        let local = OffsetDateTime::parse(stamp, &Rfc3339).ok()?.to_offset(self.offset);
        let date = local.date();
        Some((Day::new(date.year(), u8::from(date.month()), date.day()), local.hour()))
    }

    fn see(&mut self, line: &Value) {
        let flagged = |key: &str| line.get(key).and_then(Value::as_bool) == Some(true);
        if flagged("isSidechain") || flagged("isMeta") {
            return;
        }
        // A fork (Forge's or Claude Code's) copies the earlier lines with a `forkedFrom`; they were
        // counted in the original session and are not counted again.
        if line.get("forkedFrom").is_some() {
            return;
        }
        let Some((day, hour)) = self.when(line) else { return };
        let (model, usage) = match line.get("type").and_then(Value::as_str) {
            Some("assistant") => {
                let Some(message) = line.get("message") else { return };
                let Some(model) = message.get("model").and_then(Value::as_str) else { return };
                if model.is_empty() || model.starts_with('<') {
                    return;
                }
                (model.to_string(), message.get("usage").and_then(token_usage).unwrap_or_default())
            }
            Some("user") => (NO_MODEL.to_string(), TokenUsage::default()),
            _ => return,
        };
        let row = self.rows.entry((day, hour, model)).or_default();
        row.0 += 1;
        row.1 += usage;
    }

    fn finish(self) -> Vec<StatsRow> {
        self.rows
            .into_iter()
            .map(|((day, hour, model), (messages, tokens))| StatsRow { day, hour, model, messages, tokens })
            .collect()
    }
}

#[derive(Clone, PartialEq)]
struct FileKey {
    modified: Option<SystemTime>,
    len: u64,
    offset: i32,
}

struct Cached {
    key: FileKey,
    rows: Vec<StatsRow>,
}

/// Reads the statistics from the transcripts under `<claude home>/projects/*/*.jsonl`. The rows of
/// each file are kept in memory, keyed by the file's modified time, size and the UTC offset asked for,
/// so only a new or changed transcript is read again.
pub struct ClaudeUsageStatsAdapter {
    claude_home: PathBuf,
    cache: Mutex<HashMap<PathBuf, Cached>>,
    reads: AtomicUsize,
}

impl ClaudeUsageStatsAdapter {
    pub fn new(claude_home: PathBuf) -> ClaudeUsageStatsAdapter {
        ClaudeUsageStatsAdapter { claude_home, cache: Mutex::new(HashMap::new()), reads: AtomicUsize::new(0) }
    }

    #[cfg(test)]
    fn files_read(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

async fn transcript_files(projects: &Path) -> Result<Vec<PathBuf>, UsageStatsError> {
    let mut folders = match tokio::fs::read_dir(projects).await {
        Ok(folders) => folders,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(UsageStatsError::ReadFailed(format!("{}: {error}", projects.display()))),
    };
    let mut files = Vec::new();
    while let Ok(Some(folder)) = folders.next_entry().await {
        let Ok(mut inside) = tokio::fs::read_dir(folder.path()).await else { continue };
        while let Ok(Some(entry)) = inside.next_entry().await {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

// Plain blocking reads: it runs on the blocking pool (see `load`), so parsing a large transcript never
// holds the UI thread.
fn read_rows(path: &Path, offset: UtcOffset) -> Vec<StatsRow> {
    let Ok(file) = std::fs::File::open(path) else { return Vec::new() };
    let mut reader = std::io::BufReader::new(file);
    let mut collector = StatsCollector::new(offset);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(line) = std::str::from_utf8(&buffer) else { continue };
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collector.see(&value);
        }
    }
    collector.finish()
}

#[async_trait]
impl UsageStatsPort for ClaudeUsageStatsAdapter {
    async fn load(&self, utc_offset_seconds: i32) -> Result<UsageStats, UsageStatsError> {
        let offset = UtcOffset::from_whole_seconds(utc_offset_seconds).unwrap_or(UtcOffset::UTC);
        let files = transcript_files(&self.claude_home.join("projects")).await?;
        let mut sessions = Vec::new();
        let mut kept = HashSet::new();
        for path in files {
            let Ok(meta) = tokio::fs::metadata(&path).await else { continue };
            let key = FileKey { modified: meta.modified().ok(), len: meta.len(), offset: utc_offset_seconds };
            let cached = self.cache.lock().unwrap().get(&path).filter(|cached| cached.key == key).map(|cached| cached.rows.clone());
            let rows = match cached {
                Some(rows) => rows,
                None => {
                    let read_path = path.clone();
                    let rows = tokio::task::spawn_blocking(move || read_rows(&read_path, offset)).await.unwrap_or_default();
                    self.reads.fetch_add(1, Ordering::SeqCst);
                    self.cache.lock().unwrap().insert(path.clone(), Cached { key, rows: rows.clone() });
                    rows
                }
            };
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()).filter(|_| !rows.is_empty()) {
                sessions.push(SessionStats { session: SessionId(stem.to_string()), rows });
            }
            kept.insert(path);
        }
        self.cache.lock().unwrap().retain(|path, _| kept.contains(path));
        Ok(UsageStats { sessions })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    const UTC: i32 = 0;
    const JST: i32 = 9 * 3600;

    fn write(home: &Path, project: &str, id: &str, lines: &[Value]) -> PathBuf {
        let dir = home.join("projects").join(project);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        fs::write(&path, text).unwrap();
        path
    }

    fn assistant(ts: &str, id: &str, model: &str, input: u64, output: u64) -> Value {
        json!({"type": "assistant", "timestamp": ts,
               "message": {"id": id, "model": model, "content": [],
                           "usage": {"input_tokens": input, "output_tokens": output}}})
    }

    fn user_text(ts: &str, text: &str) -> Value {
        json!({"type": "user", "timestamp": ts, "message": {"role": "user", "content": text}})
    }

    async fn rows_of(home: &Path, offset: i32) -> Vec<StatsRow> {
        let adapter = ClaudeUsageStatsAdapter::new(home.to_path_buf());
        let stats = adapter.load(offset).await.unwrap();
        stats.sessions.into_iter().flat_map(|s| s.rows).collect()
    }

    #[tokio::test]
    async fn every_line_counts_as_a_message_with_its_own_usage_like_the_desktop_app() {
        let home = tempfile::tempdir().unwrap();
        write(
            home.path(),
            "p",
            "s1",
            &[
                assistant("2026-10-03T10:00:00Z", "m1", "claude-sonnet-5", 5, 1),
                assistant("2026-10-03T10:00:05Z", "m1", "claude-sonnet-5", 5, 9),
            ],
        );
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].messages, 2, "one message written on two lines is two lines");
        assert_eq!((rows[0].tokens.input, rows[0].tokens.output), (10, 10), "each line adds its own usage");
        assert_eq!((rows[0].day, rows[0].hour), (Day::new(2026, 10, 3), 10));
    }

    #[tokio::test]
    async fn sidechain_meta_and_synthetic_lines_are_not_counted_but_tool_results_are() {
        let home = tempfile::tempdir().unwrap();
        let mut sidechain = assistant("2026-10-03T10:00:00Z", "m2", "claude-sonnet-5", 100, 100);
        sidechain["isSidechain"] = json!(true);
        let mut meta = user_text("2026-10-03T10:00:00Z", "caveat");
        meta["isMeta"] = json!(true);
        let tool_result = json!({"type": "user", "timestamp": "2026-10-03T10:00:00Z", "toolUseResult": {"x": 1},
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "ok"}]}});
        write(
            home.path(),
            "p",
            "s1",
            &[
                sidechain,
                meta,
                tool_result,
                assistant("2026-10-03T10:00:00Z", "m3", "<synthetic>", 1, 1),
                user_text("2026-10-03T10:00:00Z", "hello"),
            ],
        );
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.len(), 1, "the user rows are the only ones left");
        assert_eq!(rows[0].model, NO_MODEL);
        assert_eq!(rows[0].messages, 2, "the tool result and the typed message, as the desktop app counts user lines");
    }

    #[tokio::test]
    async fn a_line_without_a_usable_timestamp_or_with_broken_json_is_skipped() {
        let home = tempfile::tempdir().unwrap();
        let path = write(
            home.path(),
            "p",
            "s1",
            &[
                json!({"type": "user", "message": {"role": "user", "content": "no stamp"}}),
                user_text("not a time", "bad stamp"),
                user_text("2026-10-03T10:00:00Z", "fine"),
            ],
        );
        let mut text = fs::read_to_string(&path).unwrap();
        text.push_str("{ this is not json\n");
        fs::write(&path, text).unwrap();
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.iter().map(|r| r.messages).sum::<u64>(), 1);
    }

    #[tokio::test]
    async fn the_utc_offset_moves_a_late_message_to_the_next_local_day() {
        let home = tempfile::tempdir().unwrap();
        write(home.path(), "p", "s1", &[assistant("2026-10-03T23:30:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let east = rows_of(home.path(), JST).await;
        assert_eq!((east[0].day, east[0].hour), (Day::new(2026, 10, 4), 8));
        let west = rows_of(home.path(), -8 * 3600).await;
        assert_eq!((west[0].day, west[0].hour), (Day::new(2026, 10, 3), 15));
        let early = tempfile::tempdir().unwrap();
        write(early.path(), "p", "s1", &[assistant("2026-10-03T03:00:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let back = rows_of(early.path(), -8 * 3600).await;
        assert_eq!((back[0].day, back[0].hour), (Day::new(2026, 10, 2), 19), "a negative offset reaches the day before");
    }

    #[tokio::test]
    async fn an_unchanged_file_is_not_read_again_and_a_rewritten_one_is() {
        let home = tempfile::tempdir().unwrap();
        let path = write(home.path(), "p", "s1", &[assistant("2026-10-03T10:00:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let adapter = ClaudeUsageStatsAdapter::new(home.path().to_path_buf());
        adapter.load(UTC).await.unwrap();
        adapter.load(UTC).await.unwrap();
        assert_eq!(adapter.files_read(), 1);

        adapter.load(JST).await.unwrap();
        assert_eq!(adapter.files_read(), 2, "another offset needs another reading");

        let mut text = fs::read_to_string(&path).unwrap();
        text.push_str(&format!("{}\n", assistant("2026-10-03T11:00:00Z", "m2", "claude-sonnet-5", 2, 2)));
        fs::write(&path, text).unwrap();
        let stats = adapter.load(JST).await.unwrap();
        assert_eq!(adapter.files_read(), 3);
        assert_eq!(stats.sessions[0].rows.iter().map(|r| r.messages).sum::<u64>(), 2);
    }

    #[tokio::test]
    async fn a_deleted_transcript_leaves_the_stats() {
        let home = tempfile::tempdir().unwrap();
        let path = write(home.path(), "p", "s1", &[user_text("2026-10-03T10:00:00Z", "hi")]);
        let adapter = ClaudeUsageStatsAdapter::new(home.path().to_path_buf());
        assert_eq!(adapter.load(UTC).await.unwrap().sessions.len(), 1);
        fs::remove_file(path).unwrap();
        assert!(adapter.load(UTC).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_missing_projects_folder_is_empty_stats_and_sessions_are_named_by_file() {
        let home = tempfile::tempdir().unwrap();
        assert!(ClaudeUsageStatsAdapter::new(home.path().to_path_buf()).load(UTC).await.unwrap().is_empty());
        write(home.path(), "p", "abc-123", &[user_text("2026-10-03T10:00:00Z", "hi")]);
        fs::write(home.path().join("projects/p/notes.txt"), "x").unwrap();
        let stats = ClaudeUsageStatsAdapter::new(home.path().to_path_buf()).load(UTC).await.unwrap();
        assert_eq!(stats.sessions.len(), 1);
        assert_eq!(stats.sessions[0].session, SessionId("abc-123".to_string()));
    }

    #[tokio::test]
    async fn lines_copied_into_a_fork_are_not_counted_again_but_its_own_lines_are() {
        let home = tempfile::tempdir().unwrap();
        let original = [assistant("2026-10-03T10:00:00Z", "m1", "claude-sonnet-5", 100, 100), user_text("2026-10-03T10:00:00Z", "hello")];
        write(home.path(), "p", "original", &original);
        let mut copied: Vec<Value> = original.to_vec();
        for line in &mut copied {
            line["forkedFrom"] = json!({"sessionId": "original", "messageUuid": "u1"});
        }
        copied.push(user_text("2026-10-04T10:00:00Z", "a new question in the fork"));
        write(home.path(), "p", "fork", &copied);

        let stats = ClaudeUsageStatsAdapter::new(home.path().to_path_buf()).load(UTC).await.unwrap();
        let messages: u64 = stats.sessions.iter().flat_map(|s| s.rows.iter()).map(|r| r.messages).sum();
        let tokens: u64 = stats.sessions.iter().flat_map(|s| s.rows.iter()).map(|r| r.tokens.input + r.tokens.output).sum();
        assert_eq!(messages, 3, "the original's two plus the fork's own one");
        assert_eq!(tokens, 200, "the copied reply is counted once");
        let fork = stats.sessions.iter().find(|s| s.session == SessionId("fork".to_string())).unwrap();
        assert_eq!(fork.rows.len(), 1, "only the fork's own line is a row of the fork");
    }
}
