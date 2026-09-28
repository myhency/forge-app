# forge-adapter-claude-fs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `forge-adapter-claude-fs` crate: four read-only adapters that implement `SessionRepositoryPort`, `SkillRepositoryPort`, `PluginRepositoryPort` and `McpRegistryPort` against Claude Code's real on-disk state.

**Architecture:** One adapter struct per port, each constructed with an explicit root path (never resolving `~/.claude` itself), so every test runs against a `tempfile` directory tree that mimics the exact file shapes found on a real machine. Adapters translate loosely-typed external JSON/Markdown into the pure `forge-domain` types by inspecting `serde_json::Value` (sessions) or narrow `serde` structs (plugins, MCP) — unknown fields and line types are ignored, never errors.

**Tech Stack:** Rust 2021; `tokio` (`fs`, `io-util`), `serde` + `serde_json`, `async-trait`, `directories` 6.0.0, `time` 0.3.55 (`formatting` feature); dev-only `tempfile` 3.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §14 (with §12.2/§12.3 for the domain types and port traits this plan implements against).

## Global Constraints

- The crate depends on `forge-domain` and `forge-ports` only — never `forge-application` or another adapter (§6 crate graph). `forge-domain` and `forge-ports` are **not modified** by this plan; the four error enums (`SessionRepositoryError{ListFailed, NotFound, LoadFailed}`, and `SkillRepositoryError`/`PluginRepositoryError`/`McpRegistryError` each with only `ListFailed(String)`) already exist from Plan 2.
- All four adapters are read-only in v1 (§14).
- Every adapter takes an explicit root path in `new(...)` and never resolves `~/.claude` / `~/.claude.json` itself; `default_claude_home()` and `default_claude_json_path()` are separate free functions in `lib.rs` (§14.2, mirroring Plan 3's `default_config_path()`).
- The global MCP file is `~/.claude.json`, a **sibling** of `~/.claude/`, not inside it — `ClaudeMcpAdapter` takes that file's path; the other three take the `~/.claude/` directory.
- Tests build real temp directories (`tempfile`) and never touch the real `~/.claude/` or `~/.claude.json` (§14.3).
- Error strings include the offending path: `format!("{}: {}", path.display(), e)` (convention from Plan 3's final review).
- Dependency pins: `directories = "6.0.0"`, `time = { version = "0.3.55", features = ["formatting"] }`, `tokio` with `fs` + `io-util`. Task 1 writes the crate's whole `Cargo.toml`; Tasks 2-5 add **no** dependencies.
- Session-directory escaping rule (amended after Task 1's review — see Amendment A): Claude Code's own directory-name rule, read out of the installed binary — every UTF-16 unit that is not an ASCII letter or digit becomes `-`; a result longer than 200 characters is cut to its first 200 characters plus `-` plus the base-36 form of the absolute value of a Java-style 32-bit hash of the **original** path (§14.1). The plan as first written used "replace `/` and `.` only", which is wrong for `_`, spaces, `~` and non-ASCII.
- Environment: `cargo` is not on `PATH` in a fresh shell — prefix commands with `PATH="$HOME/.cargo/bin:$PATH"`. Baseline before this plan: 61 workspace tests passing.

## Review Focus

The failure modes most likely to bite a person using this software that no plain "happy path" test would catch. Each is pinned by a named test in the owning task.

1. **A session id containing path separators or `..`** must not read a file outside the project's transcript directory (`load_session` returns `NotFound`) → Task 2, `load_session_rejects_ids_that_could_escape_the_project_directory`.
2. **A transcript that a running Claude Code is appending to** (sessions are shared, §4.2) may end mid-line: a malformed *final* line must be ignored, a malformed line *followed by more lines* must fail loudly → Task 2, `load_session_ignores_a_malformed_final_line` and `load_session_fails_on_a_malformed_line_in_the_middle`.
3. **One corrupt or unreadable transcript must not blank the whole session list**, and the list must be deterministic, newest first → Task 1, `list_sessions_keeps_a_session_whose_transcript_is_not_valid_utf8_with_no_title` and `list_sessions_sorts_newest_first_and_breaks_ties_by_id`.
4. **A real assistant response is split one content block per transcript line** (2,574 of 2,574 real lines), so `thinking`-only lines must not create phantom empty messages, and `tool_result`s whose `content` is a block list or that lack `is_error` (≈9% and ≈57% of real ones) must still attach → Task 2, `load_session_merges_an_assistant_response_split_one_block_per_line` and `load_session_attaches_string_and_block_list_tool_results_and_defaults_is_error_to_false`.
5. **Symlinked skill directories are common** (users link a skill from a checkout) and must be found; a symlink loop must terminate; a project rooted at the home directory must not list every skill twice → Task 3, `list_skills_follows_a_symlinked_skill_directory`, `list_skills_terminates_on_a_symlink_loop`, `list_skills_does_not_duplicate_when_the_project_root_is_the_claude_home_parent`.

**Known limitations, accepted and not tested:** Windows paths (they fall under the same escaping rule but were not exercised); YAML block scalars in skill frontmatter (`description: >-` yields the literal `>-`, only top-level `name` is protected — see Task 3); the `custom-title` line's `customTitle` field name is carried forward from §13.1 and was not seen in any real file; `isSidechain: true` was never observed in a real file, so skipping it is defensive; `CLAUDE_CONFIG_DIR` overrides are the composition root's concern; the per-project `mcpServers` entry inside `~/.claude.json` is deliberately not read.

---

## Amendment A (after Task 1's review, before Task 2)

Task 1 was implemented and reviewed clean exactly as written below. Afterwards the controller read Claude Code's real directory-naming function out of the installed 2.1.283 binary and found the plan's escaping rule ("replace `/` and `.` only") wrong for any project path containing `_`, spaces, `~` or non-ASCII characters (none of the 29 registered paths on the author's machine contains one, so no local data could reveal it). Task 1 therefore received **fix round 1**: `escape_project_path` was replaced by the real rule (Global Constraints above; spec §14.1) and three tests were added — `escape_project_path_replaces_every_non_alphanumeric_character`, `escape_project_path_caps_at_200_characters_and_appends_a_hash_of_the_original_path` (expected values generated by running the binary's own JavaScript in Node) and `list_sessions_finds_transcripts_for_a_project_path_with_underscores_and_spaces`. Task 1's text below is kept as executed; where it disagrees with this amendment, the amendment wins. **As built, Task 1 ends at 13 crate tests / 74 workspace tests**, so every later "Expected" count is 3 higher than in the first version of this plan: Task 2 → 26 / 87, Task 3 → 39 / 100, Task 4 → 46 / 107, Task 5 → 56 / 117.

---

## File Structure

```
crates/forge-adapter-claude-fs/
  Cargo.toml
  src/
    lib.rs        # module declarations, re-exports, default_claude_home(), default_claude_json_path()
    sessions.rs   # ClaudeSessionAdapter: SessionRepositoryPort
    skills.rs     # ClaudeSkillAdapter: SkillRepositoryPort
    plugins.rs    # ClaudePluginAdapter: PluginRepositoryPort
    mcp.rs        # ClaudeMcpAdapter: McpRegistryPort
```

The workspace `Cargo.toml` uses `members = ["crates/*"]`, so the new directory is picked up automatically.

---

### Task 1: Crate scaffold and `ClaudeSessionAdapter::list_sessions`

**Files:**
- Create: `crates/forge-adapter-claude-fs/Cargo.toml`
- Create: `crates/forge-adapter-claude-fs/src/lib.rs`
- Create: `crates/forge-adapter-claude-fs/src/sessions.rs`

**Interfaces:**
- Consumes: `forge_domain::{Session, SessionId, SessionSummary}`; `forge_ports::{SessionRepositoryError, SessionRepositoryPort}` (`list_sessions(&self, project_path: &Path) -> Result<Vec<SessionSummary>, SessionRepositoryError>`, `load_session(&self, project_path: &Path, session_id: &SessionId) -> Result<Session, SessionRepositoryError>`).
- Produces (later tasks rely on these exact names):
  - `pub fn default_claude_home() -> Option<PathBuf>`, `pub fn default_claude_json_path() -> Option<PathBuf>` in `lib.rs`.
  - `pub struct ClaudeSessionAdapter { claude_home: PathBuf }` with `pub fn new(claude_home: PathBuf) -> Self`, private `fn project_dir(&self, project_path: &Path) -> PathBuf`.
  - private free fns in `sessions.rs`: `escape_project_path(project_path: &Path) -> String`, `title_from_value(value: &Value) -> Option<String>`, `title_from_line(line: &str) -> Option<String>`, `async fn read_title(path: &Path) -> std::io::Result<Option<String>>`.
  - `load_session` exists as an `unimplemented!()` stub that Task 2 replaces.
  - test helpers in `sessions.rs`'s `mod tests`: `project() -> PathBuf`, `write_session(home: &Path, project: &Path, id: &str, lines: &[Value]) -> PathBuf`, `user_text(text: &str) -> Value`, `ai_title(title: &str) -> Value`, `set_mtime(path: &Path, seconds_since_epoch: u64)`.

- [ ] **Step 1: Create the crate manifest**

`crates/forge-adapter-claude-fs/Cargo.toml`:

```toml
[package]
name = "forge-adapter-claude-fs"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["fs", "io-util"] }
directories = "6.0.0"
time = { version = "0.3.55", features = ["formatting"] }

[dev-dependencies]
tokio = { version = "1", features = ["fs", "io-util", "macros", "rt"] }
tempfile = "3"
```

- [ ] **Step 2: Create `lib.rs` (with its tests)**

`crates/forge-adapter-claude-fs/src/lib.rs`:

```rust
use std::path::PathBuf;

mod sessions;

pub use sessions::ClaudeSessionAdapter;

pub fn default_claude_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude"))
}

pub fn default_claude_json_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_claude_home_ends_with_dot_claude() {
        let path = default_claude_home().expect("a home directory should be resolvable on this platform");
        assert_eq!(path.file_name().unwrap(), ".claude");
    }

    #[test]
    fn default_claude_json_path_is_a_sibling_of_the_claude_directory_not_inside_it() {
        let json_path = default_claude_json_path().expect("a home directory should be resolvable on this platform");
        let claude_home = default_claude_home().expect("a home directory should be resolvable on this platform");
        assert_eq!(json_path.file_name().unwrap(), ".claude.json");
        assert_eq!(json_path.parent(), claude_home.parent());
    }
}
```

- [ ] **Step 3: Create `sessions.rs` with the scaffold and all Task 1 tests**

`list_sessions` and `load_session` are `unimplemented!()` for now, so the `list_sessions` tests fail (red). `escape_project_path` is real code from the start.

`crates/forge-adapter-claude-fs/src/sessions.rs`:

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{Session, SessionId, SessionSummary};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort};
use serde_json::Value;

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

#[async_trait]
impl SessionRepositoryPort for ClaudeSessionAdapter {
    async fn list_sessions(
        &self,
        _project_path: &Path,
    ) -> Result<Vec<SessionSummary>, SessionRepositoryError> {
        unimplemented!("implemented in Task 1, Step 5")
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
```

- [ ] **Step 4: Run the tests to verify the red state**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: the crate compiles (unused-import / dead-code warnings are expected in this red state); **3 passed** (the two `lib.rs` tests and `escape_project_path_replaces_slashes_and_dots_with_dashes`) and **7 failed** — every `list_sessions_*` test, each panicking with `not implemented: implemented in Task 1, Step 5`.

- [ ] **Step 5: Implement `list_sessions` and the title helpers**

In `sessions.rs`, replace the import block at the top with:

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{Session, SessionId, SessionSummary};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};
```

Insert these free functions directly after `escape_project_path` (before the `#[async_trait] impl` block):

```rust
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
```

Replace the `list_sessions` stub (the whole method, including its `_project_path` parameter) with:

```rust
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
```

- [ ] **Step 6: Run the tests to verify green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: **10 passed; 0 failed**, no compiler warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | grep -E "^test result|warning"`
Expected: every `test result` line `ok`, **71 tests in total** (61 baseline + 10), no warnings.

- [ ] **Step 7: Commit**

```bash
git add Cargo.lock crates/forge-adapter-claude-fs
git commit -m "feat: scaffold forge-adapter-claude-fs with SessionRepositoryPort::list_sessions"
```

---

### Task 2: `ClaudeSessionAdapter::load_session`

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (imports, the `load_session` stub, new free fns, test helpers and tests)

**Interfaces:**
- Consumes (from Task 1, same file): `ClaudeSessionAdapter::project_dir`, `title_from_value`, the `mod tests` helpers `project()`, `write_session`, `user_text`, `ai_title`. From `forge_domain`: `Message` (with `Message::new(role: MessageRole) -> Message`), `MessageRole::{User, Assistant}`, `ToolCall { id, name, input, result }`, `ToolResult { output, is_error }`, `PermissionMode::{Default, AcceptEdits, BypassPermissions, Plan}`.
- Produces: a working `load_session`; private fns `apply_line`, `apply_user_line`, `apply_assistant_line`, `push_user_message`, `attach_tool_result`, `flatten_tool_result_content`, `parse_permission_mode`. Nothing later depends on them.

Behavior recap (spec §14.1/§14.2): only `user`, `assistant`, `permission-mode`, `ai-title`/`custom-title` lines matter; every other `type` and any line with `isSidechain: true` is ignored. A real API response is split one content block per `assistant` line, so consecutive assistant content is merged into the last `Assistant` message until a user message intervenes (the same rule `apply_sidecar_event` in `forge-application` uses for live sessions).

- [ ] **Step 1: Add the failing tests**

Inside `sessions.rs`'s `mod tests`, add these helpers next to the existing ones:

```rust
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
```

and these tests at the end of `mod tests`:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs load_session`
Expected: **compile errors** — the new tests use `MessageRole`, `PermissionMode` and `ToolResult` (via `use super::*`), which the parent module does not import yet (`error[E0433]`/`E0412`: failed to resolve / cannot find type). That is the red state; Step 3 adds the imports and the implementation.

- [ ] **Step 3: Implement `load_session`**

Replace the import block at the top of `sessions.rs` with:

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{
    Message, MessageRole, PermissionMode, Session, SessionId, SessionSummary, ToolCall, ToolResult,
};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};
```

Insert these free functions directly after `read_title` (before the `#[async_trait] impl` block):

```rust
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
    if line.get("isSidechain").and_then(Value::as_bool) == Some(true) {
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
        Some("ai-title") | Some("custom-title") => {
            if let Some(title) = title_from_value(line) {
                session.title = Some(title);
            }
        }
        _ => {}
    }
}
```

Replace the `load_session` stub (the whole method, including its underscore-prefixed parameters) with:

```rust
    async fn load_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<Session, SessionRepositoryError> {
        let id = &session_id.0;
        if id.is_empty() || id.contains(['/', '\\']) || id.contains("..") {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }

        let path = self.project_dir(project_path).join(format!("{id}.jsonl"));
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

        let mut session = Session {
            id: session_id.clone(),
            project_path: project_path.to_path_buf(),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
        };
        let mut lines = BufReader::new(file).lines();
        let mut malformed_line: Option<usize> = None;
        let mut line_number = 0usize;
        while let Some(line) = lines.next_line().await.map_err(|e| {
            SessionRepositoryError::LoadFailed(format!("{}: {}", path.display(), e))
        })? {
            line_number += 1;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(bad_line) = malformed_line {
                return Err(SessionRepositoryError::LoadFailed(format!(
                    "{}: malformed JSON on line {}",
                    path.display(),
                    bad_line
                )));
            }
            match serde_json::from_str::<Value>(&line) {
                Ok(value) => apply_line(&mut session, &value),
                Err(_) => malformed_line = Some(line_number),
            }
        }
        Ok(session)
    }
```

- [ ] **Step 4: Run the tests to verify green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: **26 passed; 0 failed** (13 from Task 1 as amended + 13 new), no compiler warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | grep -E "^test result|warning"`
Expected: all `ok`, **87 tests in total**, no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/sessions.rs
git commit -m "feat: implement SessionRepositoryPort::load_session against real transcript shapes"
```

---

### Task 3: `ClaudeSkillAdapter::list_skills`

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/skills.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (add the module and re-export)

**Interfaces:**
- Consumes: `forge_domain::SkillDefinition { name: String, path: PathBuf, description: Option<String> }`; `forge_ports::{SkillRepositoryError, SkillRepositoryPort}` (`list_skills(&self, project_path: &Path) -> Result<Vec<SkillDefinition>, SkillRepositoryError>`).
- Produces: `pub struct ClaudeSkillAdapter { claude_home: PathBuf }` with `pub fn new(claude_home: PathBuf) -> Self`, re-exported from `lib.rs`.

Behavior recap (spec §14.2): walk `claude_home/skills/` and `project_path/.claude/skills/` for files named `SKILL.md` at any depth, following symlinked directories with a shared visited set of canonical paths; parse hand-rolled frontmatter (BOM/CRLF tolerant, top-level keys only, one pair of surrounding quotes stripped); skip a file with no frontmatter or no `name`; a missing `description` is kept as `None`; result sorted by name then path. `ListFailed` only when a root that exists cannot be read.

- [ ] **Step 1: Wire the module into `lib.rs`**

In `lib.rs`, add below the existing `mod sessions;` line:

```rust
mod skills;
```

and below `pub use sessions::ClaudeSessionAdapter;`:

```rust
pub use skills::ClaudeSkillAdapter;
```

- [ ] **Step 2: Create `skills.rs` with the scaffold and all tests**

`list_skills` is `unimplemented!()` so the tests are red.

`crates/forge-adapter-claude-fs/src/skills.rs`:

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::SkillDefinition;
use forge_ports::{SkillRepositoryError, SkillRepositoryPort};

pub struct ClaudeSkillAdapter {
    claude_home: PathBuf,
}

impl ClaudeSkillAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[async_trait]
impl SkillRepositoryPort for ClaudeSkillAdapter {
    async fn list_skills(
        &self,
        _project_path: &Path,
    ) -> Result<Vec<SkillDefinition>, SkillRepositoryError> {
        let _ = &self.claude_home;
        unimplemented!("implemented in Task 3, Step 4")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, relative_dir: &str, contents: &str) {
        let dir = root.join(relative_dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), contents).unwrap();
    }

    fn skill(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n")
    }

    fn names(skills: &[SkillDefinition]) -> Vec<&str> {
        skills.iter().map(|s| s.name.as_str()).collect()
    }

    async fn list(home: &Path, project: &Path) -> Vec<SkillDefinition> {
        ClaudeSkillAdapter::new(home.to_path_buf())
            .list_skills(project)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn list_skills_finds_a_skill_and_reads_name_description_and_directory_path() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "algorithmic-art",
            "---\nname: algorithmic-art\ndescription: Creating art\nlicense: Complete terms in LICENSE.txt\n---\n\nBody text\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(
            skills,
            vec![SkillDefinition {
                name: "algorithmic-art".to_string(),
                path: home.path().join("skills").join("algorithmic-art"),
                description: Some("Creating art".to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn list_skills_finds_skills_nested_three_levels_deep() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "document-skills/xlsx", &skill("xlsx", "spreadsheets"));
        write_skill(&root, "synced/uuid-1/deep-research", &skill("deep-research", "research"));
        write_skill(&root, "top-level", &skill("top-level", "flat"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["deep-research", "top-level", "xlsx"]);
    }

    #[tokio::test]
    async fn list_skills_reads_global_and_project_local_skills_sorted_by_name() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "zeta", &skill("zeta", "global"));
        write_skill(
            &project.path().join(".claude").join("skills"),
            "alpha",
            &skill("alpha", "project-local"),
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["alpha", "zeta"]);
        assert_eq!(
            skills[0].path,
            project.path().join(".claude").join("skills").join("alpha")
        );
    }

    #[tokio::test]
    async fn list_skills_skips_a_skill_without_a_name_but_keeps_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "nameless", "---\ndescription: no name here\n---\n");
        write_skill(&root, "ok", &skill("ok", "fine"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["ok"]);
    }

    #[tokio::test]
    async fn list_skills_keeps_a_skill_without_a_description_with_none() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "bare", "---\nname: bare\n---\n");

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["bare"]);
        assert_eq!(skills[0].description, None);
    }

    #[tokio::test]
    async fn list_skills_skips_files_without_frontmatter_or_with_unterminated_frontmatter() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "no-frontmatter", "# Just a heading\n");
        write_skill(&root, "unterminated", "---\nname: unterminated\ndescription: never closed\n");
        write_skill(&root, "ok", &skill("ok", "fine"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["ok"]);
    }

    #[tokio::test]
    async fn list_skills_handles_crlf_bom_and_quoted_values() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(
            &root,
            "crlf",
            "\u{feff}---\r\nname: crlf-skill\r\ndescription: Windows authored\r\n---\r\nBody\r\n",
        );
        write_skill(
            &root,
            "quoted",
            "---\nname: 'quoted-skill'\ndescription: \"Use when: the user asks\"\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["crlf-skill", "quoted-skill"]);
        assert_eq!(skills[0].description.as_deref(), Some("Windows authored"));
        assert_eq!(skills[1].description.as_deref(), Some("Use when: the user asks"));
    }

    #[tokio::test]
    async fn list_skills_ignores_indented_lines_inside_frontmatter() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "folded",
            "---\nname: real-name\ndescription: >-\n  name: fake-name\n  continued text\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["real-name"]);
    }

    #[tokio::test]
    async fn list_skills_returns_empty_when_no_skill_directories_exist() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();

        let skills = list(home.path(), project.path()).await;

        assert!(skills.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_follows_a_symlinked_skill_directory() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        write_skill(checkout.path(), "my-skill", &skill("my-skill", "from a checkout"));
        let skills_root = home.path().join("skills");
        std::fs::create_dir_all(&skills_root).unwrap();
        std::os::unix::fs::symlink(checkout.path().join("my-skill"), skills_root.join("linked")).unwrap();

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["my-skill"]);
        assert_eq!(skills[0].path, skills_root.join("linked"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_terminates_on_a_symlink_loop() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let skills_root = home.path().join("skills");
        write_skill(&skills_root, "a", &skill("looper", "d"));
        std::os::unix::fs::symlink(&skills_root, skills_root.join("a").join("back-to-root")).unwrap();

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["looper"]);
    }

    #[tokio::test]
    async fn list_skills_does_not_duplicate_when_the_project_root_is_the_claude_home_parent() {
        let base = tempfile::tempdir().unwrap();
        let claude_home = base.path().join(".claude");
        write_skill(&claude_home.join("skills"), "shared", &skill("shared", "d"));

        let skills = ClaudeSkillAdapter::new(claude_home)
            .list_skills(base.path())
            .await
            .unwrap();

        assert_eq!(names(&skills), vec!["shared"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_fails_when_the_skills_root_is_not_a_directory() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("skills"), "i am a file").unwrap();

        let result = ClaudeSkillAdapter::new(home.path().to_path_buf())
            .list_skills(project.path())
            .await;

        assert!(matches!(result, Err(SkillRepositoryError::ListFailed(_))));
    }
}
```

- [ ] **Step 3: Run the tests to verify the red state**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs list_skills`
Expected: compiles; **13 failed** — every `list_skills_*` test panics with `not implemented: implemented in Task 3, Step 4`.

- [ ] **Step 4: Implement `list_skills`**

Replace the import block at the top of `skills.rs` with:

```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::SkillDefinition;
use forge_ports::{SkillRepositoryError, SkillRepositoryPort};
```

Insert these free functions after `impl ClaudeSkillAdapter { ... }` (before the `#[async_trait] impl`):

```rust
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

fn parse_frontmatter(contents: &str) -> Option<HashMap<String, String>> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let mut lines = contents.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut fields = HashMap::new();
    for line in lines {
        if line.trim_end() == "---" {
            return Some(fields);
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            fields.insert(key.trim().to_string(), unquote(value.trim()).to_string());
        }
    }
    None
}

async fn read_skill(path: &Path) -> Option<SkillDefinition> {
    let contents = tokio::fs::read_to_string(path).await.ok()?;
    let frontmatter = parse_frontmatter(&contents)?;
    let name = frontmatter.get("name").filter(|n| !n.is_empty())?.clone();
    let description = frontmatter
        .get("description")
        .filter(|d| !d.is_empty())
        .cloned();
    Some(SkillDefinition {
        name,
        path: path.parent()?.to_path_buf(),
        description,
    })
}

async fn collect_skills(
    root: &Path,
    visited: &mut HashSet<PathBuf>,
    skills: &mut Vec<SkillDefinition>,
) -> Result<(), SkillRepositoryError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(canonical) = tokio::fs::canonicalize(&dir).await else {
            continue;
        };
        if !visited.insert(canonical) {
            continue;
        }
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if dir == root && e.kind() != std::io::ErrorKind::NotFound => {
                return Err(SkillRepositoryError::ListFailed(format!(
                    "{}: {}",
                    dir.display(),
                    e
                )));
            }
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let Ok(metadata) = tokio::fs::metadata(&path).await else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() && entry.file_name() == "SKILL.md" {
                if let Some(skill) = read_skill(&path).await {
                    skills.push(skill);
                }
            }
        }
    }
    Ok(())
}
```

Replace the `list_skills` stub (whole method) with:

```rust
    async fn list_skills(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SkillDefinition>, SkillRepositoryError> {
        let roots = [
            self.claude_home.join("skills"),
            project_path.join(".claude").join("skills"),
        ];
        let mut visited = HashSet::new();
        let mut skills = Vec::new();
        for root in &roots {
            collect_skills(root, &mut visited, &mut skills).await?;
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        Ok(skills)
    }
```

- [ ] **Step 5: Run the tests to verify green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: **39 passed; 0 failed** (26 + 13), no compiler warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | grep -E "^test result|warning"`
Expected: all `ok`, **100 tests in total**, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/lib.rs crates/forge-adapter-claude-fs/src/skills.rs
git commit -m "feat: implement SkillRepositoryPort with recursive, symlink-safe SKILL.md discovery"
```

---

### Task 4: `ClaudePluginAdapter::list_plugins`

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/plugins.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (add the module and re-export)

**Interfaces:**
- Consumes: `forge_domain::PluginDefinition { name: String, marketplace: Option<String>, path: PathBuf }`; `forge_ports::{PluginRepositoryError, PluginRepositoryPort}` (`list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError>` — global only, no project path).
- Produces: `pub struct ClaudePluginAdapter { claude_home: PathBuf }` with `pub fn new(claude_home: PathBuf) -> Self`, re-exported from `lib.rs`.

Behavior recap (spec §14.2): parse `claude_home/plugins/installed_plugins.json` (`{version, plugins: {"<name>@<marketplace>": [{installPath, ...}]}}`); missing file → empty; split each key on the **first** `@` (no `@` → whole key is `name`, `marketplace: None`); one `PluginDefinition` per array entry that has an `installPath`; unknown fields ignored; result sorted by name, marketplace, path (a `HashMap` iterates in random order); malformed JSON → `ListFailed` with the path in the message.

- [ ] **Step 1: Wire the module into `lib.rs`**

Add `mod plugins;` below the existing `mod skills;` line, and `pub use plugins::ClaudePluginAdapter;` below `pub use skills::ClaudeSkillAdapter;`.

- [ ] **Step 2: Create `plugins.rs` with the scaffold and all tests**

`crates/forge-adapter-claude-fs/src/plugins.rs`:

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::PluginDefinition;
use forge_ports::{PluginRepositoryError, PluginRepositoryPort};

pub struct ClaudePluginAdapter {
    claude_home: PathBuf,
}

impl ClaudePluginAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[async_trait]
impl PluginRepositoryPort for ClaudePluginAdapter {
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        let _ = &self.claude_home;
        unimplemented!("implemented in Task 4, Step 4")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_registry(home: &Path, json: &str) {
        let dir = home.join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("installed_plugins.json"), json).unwrap();
    }

    async fn list(home: &Path) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        ClaudePluginAdapter::new(home.to_path_buf()).list_plugins().await
    }

    #[tokio::test]
    async fn list_plugins_returns_empty_when_the_registry_file_is_missing() {
        let home = tempfile::tempdir().unwrap();

        let plugins = list(home.path()).await.unwrap();

        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn list_plugins_splits_name_and_marketplace_ignores_extra_fields_and_sorts_by_name() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{
                "version": 2,
                "plugins": {
                    "frontend-design@claude-plugins-official": [{
                        "scope": "user",
                        "installPath": "/plugins/frontend-design/fa59",
                        "version": "fa59",
                        "installedAt": "2026-02-20T18:35:42.344Z",
                        "lastUpdated": "2026-09-27T07:32:47.339Z",
                        "gitCommitSha": "fa59bc9037741ecfa131aa27938272605710d7b2"
                    }],
                    "andrej-karpathy-skills@karpathy-skills": [{
                        "scope": "user",
                        "installPath": "/plugins/karpathy/1.0.0",
                        "version": "1.0.0",
                        "installedAt": "2026-02-17T04:26:33.277Z",
                        "lastUpdated": "2026-02-17T04:26:33.277Z",
                        "gitCommitSha": "aa4467f0b33e1e80d11c7c043d4b27e7c79a73a3"
                    }]
                }
            }"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(
            plugins,
            vec![
                PluginDefinition {
                    name: "andrej-karpathy-skills".to_string(),
                    marketplace: Some("karpathy-skills".to_string()),
                    path: PathBuf::from("/plugins/karpathy/1.0.0"),
                },
                PluginDefinition {
                    name: "frontend-design".to_string(),
                    marketplace: Some("claude-plugins-official".to_string()),
                    path: PathBuf::from("/plugins/frontend-design/fa59"),
                },
            ]
        );
    }

    #[tokio::test]
    async fn list_plugins_emits_one_definition_per_install_entry() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"tool@market": [
                {"scope": "project", "installPath": "/b"},
                {"scope": "user", "installPath": "/a"}
            ]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        let paths: Vec<&PathBuf> = plugins.iter().map(|p| &p.path).collect();
        assert_eq!(paths, vec![&PathBuf::from("/a"), &PathBuf::from("/b")]);
        assert!(plugins.iter().all(|p| p.name == "tool" && p.marketplace.as_deref() == Some("market")));
    }

    #[tokio::test]
    async fn list_plugins_treats_a_key_without_an_at_sign_as_a_name_with_no_marketplace() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"local-plugin": [{"installPath": "/local"}]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(
            plugins,
            vec![PluginDefinition {
                name: "local-plugin".to_string(),
                marketplace: None,
                path: PathBuf::from("/local"),
            }]
        );
    }

    #[tokio::test]
    async fn list_plugins_skips_an_install_entry_without_an_install_path() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"tool@market": [{"scope": "user"}, {"installPath": "/ok"}]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].path, PathBuf::from("/ok"));
    }

    #[tokio::test]
    async fn list_plugins_fails_on_malformed_json_and_names_the_file() {
        let home = tempfile::tempdir().unwrap();
        write_registry(home.path(), "{not json");

        let result = list(home.path()).await;

        match result {
            Err(PluginRepositoryError::ListFailed(message)) => {
                assert!(message.contains("installed_plugins.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_plugins_returns_empty_when_the_plugins_map_is_missing() {
        let home = tempfile::tempdir().unwrap();
        write_registry(home.path(), r#"{"version": 2}"#);

        let plugins = list(home.path()).await.unwrap();

        assert!(plugins.is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify the red state**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs list_plugins`
Expected: compiles; **7 failed**, each panicking with `not implemented: implemented in Task 4, Step 4`.

- [ ] **Step 4: Implement `list_plugins`**

Replace the import block at the top of `plugins.rs` with:

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::PluginDefinition;
use forge_ports::{PluginRepositoryError, PluginRepositoryPort};
use serde::Deserialize;
```

Insert after `impl ClaudePluginAdapter { ... }` (before the `#[async_trait] impl`):

```rust
#[derive(Deserialize)]
struct InstalledPlugins {
    #[serde(default)]
    plugins: HashMap<String, Vec<InstalledPlugin>>,
}

#[derive(Deserialize)]
struct InstalledPlugin {
    #[serde(default, rename = "installPath")]
    install_path: Option<PathBuf>,
}
```

Replace the `list_plugins` stub (whole method) with:

```rust
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        let path = self
            .claude_home
            .join("plugins")
            .join("installed_plugins.json");
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(PluginRepositoryError::ListFailed(format!(
                    "{}: {}",
                    path.display(),
                    e
                )))
            }
        };
        let installed: InstalledPlugins = serde_json::from_str(&contents).map_err(|e| {
            PluginRepositoryError::ListFailed(format!("{}: {}", path.display(), e))
        })?;

        let mut plugins = Vec::new();
        for (key, installs) in installed.plugins {
            let (name, marketplace) = match key.split_once('@') {
                Some((name, marketplace)) => (name.to_string(), Some(marketplace.to_string())),
                None => (key.clone(), None),
            };
            for install in installs {
                if let Some(install_path) = install.install_path {
                    plugins.push(PluginDefinition {
                        name: name.clone(),
                        marketplace: marketplace.clone(),
                        path: install_path,
                    });
                }
            }
        }
        plugins.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.marketplace.cmp(&b.marketplace))
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(plugins)
    }
```

- [ ] **Step 5: Run the tests to verify green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: **46 passed; 0 failed** (39 + 7), no compiler warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | grep -E "^test result|warning"`
Expected: all `ok`, **107 tests in total**, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/lib.rs crates/forge-adapter-claude-fs/src/plugins.rs
git commit -m "feat: implement PluginRepositoryPort from installed_plugins.json"
```

---

### Task 5: `ClaudeMcpAdapter::list_servers`

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/mcp.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (add the module and re-export)

**Interfaces:**
- Consumes: `forge_domain::McpServerDefinition { name: String, config: serde_json::Value }`; `forge_ports::{McpRegistryError, McpRegistryPort}` (`list_servers(&self, project_path: &Path) -> Result<Vec<McpServerDefinition>, McpRegistryError>`).
- Produces: `pub struct ClaudeMcpAdapter { claude_json_path: PathBuf }` with `pub fn new(claude_json_path: PathBuf) -> Self` (the path of the `~/.claude.json` **file**), re-exported from `lib.rs`.

Behavior recap (spec §14.2): read only the `mcpServers` field of the global `~/.claude.json` through a narrow struct (the file has ~100 other keys, some sensitive — they are never touched); merge with `project_path/.mcp.json`, which exists in two real shapes — bare `{"<name>": {...}}` and wrapped `{"mcpServers": {"<name>": {...}}}`; project entries win on name collision; either file missing contributes nothing; entries whose config is not a JSON object are dropped (guards the bare-shape heuristic against stray keys like `"version": 2`); malformed JSON or a non-object `.mcp.json` top level → `ListFailed` with the path in the message; result sorted by name.

- [ ] **Step 1: Wire the module into `lib.rs`**

Add `mod mcp;` below the existing `mod plugins;` line, and `pub use mcp::ClaudeMcpAdapter;` below `pub use plugins::ClaudePluginAdapter;`.

- [ ] **Step 2: Create `mcp.rs` with the scaffold and all tests**

`crates/forge-adapter-claude-fs/src/mcp.rs`:

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::McpServerDefinition;
use forge_ports::{McpRegistryError, McpRegistryPort};

pub struct ClaudeMcpAdapter {
    claude_json_path: PathBuf,
}

impl ClaudeMcpAdapter {
    pub fn new(claude_json_path: PathBuf) -> Self {
        Self { claude_json_path }
    }
}

#[async_trait]
impl McpRegistryPort for ClaudeMcpAdapter {
    async fn list_servers(
        &self,
        _project_path: &Path,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        let _ = &self.claude_json_path;
        unimplemented!("implemented in Task 5, Step 4")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        _dir: tempfile::TempDir,
        claude_json: PathBuf,
        project: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let claude_json = dir.path().join(".claude.json");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        Fixture { _dir: dir, claude_json, project }
    }

    async fn list(fixture: &Fixture) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        ClaudeMcpAdapter::new(fixture.claude_json.clone())
            .list_servers(&fixture.project)
            .await
    }

    fn server(name: &str, config: serde_json::Value) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config }
    }

    #[tokio::test]
    async fn list_servers_returns_empty_when_neither_file_exists() {
        let fixture = fixture();

        assert!(list(&fixture).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_servers_reads_the_global_mcp_servers_and_ignores_every_other_key() {
        let fixture = fixture();
        std::fs::write(
            &fixture.claude_json,
            json!({
                "numStartups": 3,
                "oauthAccount": {"emailAddress": "someone@example.com"},
                "projects": {},
                "mcpServers": {"playwright": {"command": "npx", "args": ["playwright-mcp"]}}
            })
            .to_string(),
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server("playwright", json!({"command": "npx", "args": ["playwright-mcp"]}))]
        );
    }

    #[tokio::test]
    async fn list_servers_reads_a_bare_project_mcp_json() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"atlassian": {"command": "uvx", "args": ["mcp-atlassian"]}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server("atlassian", json!({"command": "uvx", "args": ["mcp-atlassian"]}))]
        );
    }

    #[tokio::test]
    async fn list_servers_reads_a_wrapped_project_mcp_json() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"mcpServers": {"docs": {"type": "http", "url": "https://example.com/mcp", "headers": {"Authorization": "Bearer t"}}}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server(
                "docs",
                json!({"type": "http", "url": "https://example.com/mcp", "headers": {"Authorization": "Bearer t"}})
            )]
        );
    }

    #[tokio::test]
    async fn list_servers_lets_project_entries_override_global_entries_by_name_and_sorts_by_name() {
        let fixture = fixture();
        std::fs::write(
            &fixture.claude_json,
            r#"{"mcpServers": {"a": {"from": "global"}, "b": {"from": "global"}}}"#,
        )
        .unwrap();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"b": {"from": "project"}, "c": {"from": "project"}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![
                server("a", json!({"from": "global"})),
                server("b", json!({"from": "project"})),
                server("c", json!({"from": "project"})),
            ]
        );
    }

    #[tokio::test]
    async fn list_servers_drops_bare_entries_that_are_not_objects() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"real": {"command": "x"}, "note": "hello", "version": 2}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(servers, vec![server("real", json!({"command": "x"}))]);
    }

    #[tokio::test]
    async fn list_servers_fails_on_malformed_global_json_and_names_the_file() {
        let fixture = fixture();
        std::fs::write(&fixture.claude_json, "{oops").unwrap();

        match list(&fixture).await {
            Err(McpRegistryError::ListFailed(message)) => {
                assert!(message.contains(".claude.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_servers_fails_on_malformed_project_json_and_names_the_file() {
        let fixture = fixture();
        std::fs::write(fixture.project.join(".mcp.json"), "{oops").unwrap();

        match list(&fixture).await {
            Err(McpRegistryError::ListFailed(message)) => {
                assert!(message.contains(".mcp.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_servers_fails_when_project_mcp_json_is_not_an_object() {
        let fixture = fixture();
        std::fs::write(fixture.project.join(".mcp.json"), "[1, 2]").unwrap();

        assert!(matches!(
            list(&fixture).await,
            Err(McpRegistryError::ListFailed(_))
        ));
    }

    #[tokio::test]
    async fn list_servers_returns_empty_when_the_global_file_has_no_mcp_servers_key() {
        let fixture = fixture();
        std::fs::write(&fixture.claude_json, r#"{"numStartups": 1}"#).unwrap();

        assert!(list(&fixture).await.unwrap().is_empty());
    }
}
```

- [ ] **Step 3: Run the tests to verify the red state**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs list_servers`
Expected: compiles; **10 failed**, each panicking with `not implemented: implemented in Task 5, Step 4`.

- [ ] **Step 4: Implement `list_servers`**

Replace the import block at the top of `mcp.rs` with:

```rust
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::McpServerDefinition;
use forge_ports::{McpRegistryError, McpRegistryPort};
use serde::Deserialize;
use serde_json::Value;
```

Insert after `impl ClaudeMcpAdapter { ... }` (before the `#[async_trait] impl`):

```rust
#[derive(Deserialize)]
struct GlobalClaudeJson {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: HashMap<String, Value>,
}

fn failed(path: &Path, reason: impl std::fmt::Display) -> McpRegistryError {
    McpRegistryError::ListFailed(format!("{}: {}", path.display(), reason))
}

async fn read_optional(path: &Path) -> Result<Option<String>, McpRegistryError> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(Some(contents)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(failed(path, e)),
    }
}

fn normalize_project_servers(parsed: Value) -> Result<HashMap<String, Value>, &'static str> {
    let Value::Object(mut top) = parsed else {
        return Err("expected a JSON object at the top level");
    };
    match top.remove("mcpServers") {
        Some(Value::Object(wrapped)) => Ok(wrapped.into_iter().collect()),
        Some(_) => Err("\"mcpServers\" is not an object"),
        None => Ok(top.into_iter().collect()),
    }
}
```

Replace the `list_servers` stub (whole method) with:

```rust
    async fn list_servers(
        &self,
        project_path: &Path,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        let mut servers: HashMap<String, Value> = HashMap::new();

        if let Some(contents) = read_optional(&self.claude_json_path).await? {
            let parsed: GlobalClaudeJson = serde_json::from_str(&contents)
                .map_err(|e| failed(&self.claude_json_path, e))?;
            servers.extend(parsed.mcp_servers);
        }

        let project_file = project_path.join(".mcp.json");
        if let Some(contents) = read_optional(&project_file).await? {
            let parsed: Value =
                serde_json::from_str(&contents).map_err(|e| failed(&project_file, e))?;
            let project_servers =
                normalize_project_servers(parsed).map_err(|reason| failed(&project_file, reason))?;
            servers.extend(project_servers);
        }

        let mut servers: Vec<McpServerDefinition> = servers
            .into_iter()
            .filter(|(_, config)| config.is_object())
            .map(|(name, config)| McpServerDefinition { name, config })
            .collect();
        servers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(servers)
    }
```

- [ ] **Step 5: Run the tests to verify green**

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test -p forge-adapter-claude-fs`
Expected: **56 passed; 0 failed** (46 + 10), no compiler warnings.

Run: `PATH="$HOME/.cargo/bin:$PATH" cargo test --workspace 2>&1 | grep -E "^test result|warning"`
Expected: all `ok`, **117 tests in total**, no warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs/src/lib.rs crates/forge-adapter-claude-fs/src/mcp.rs
git commit -m "feat: implement McpRegistryPort merging global and project-local MCP servers"
```

---

## Acceptance check against real data (controller-run, read-only, never committed)

Before this plan was handed off, its final-state code was assembled in a scratch workspace and every red/green stage was run: the per-task test counts and "no warnings" claims above are measured, not estimated (10 → 23 → 36 → 43 → 53 crate tests as first written; 13 → 26 → 39 → 46 → 56 after Amendment A). The same assembled code was then run **read-only against the author's real `~/.claude`** and produced: 18 project directories, 66 sessions listed (56 with titles), **66/66 loaded with 0 failures** (the largest transcript, 13.6 MB, loaded in 53 ms in a release build), 453 messages with **0 empty assistant messages**, 1,233 tool calls (2 without a result), 30 skills (matching the 30 `SKILL.md` files on disk), 9 plugins, 0 MCP servers.

After Task 5 (and again after any final-review fix wave), the controller re-runs that check: temporarily add `crates/forge-adapter-claude-fs/examples/smoke.rs` in the worktree (do **not** commit it; delete it afterwards), run `PATH="$HOME/.cargo/bin:$PATH" cargo run --release -p forge-adapter-claude-fs --example smoke`, and confirm: `failed: 0`, `empty assistant msgs: 0`, skills count equals `find ~/.claude/skills -name SKILL.md | wc -l` (plus any project-local skills for the chosen project), plugins count equals the number of install entries in `~/.claude/plugins/installed_plugins.json`.

```rust
use std::path::PathBuf;
use std::time::Instant;

use forge_adapter_claude_fs::*;
use forge_domain::*;
use forge_ports::*;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let home = default_claude_home().unwrap();
    let sessions = ClaudeSessionAdapter::new(home.clone());

    let mut dirs: Vec<String> = std::fs::read_dir(home.join("projects"))
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    dirs.sort();

    let (mut n_sessions, mut n_loaded, mut n_failed) = (0, 0, 0);
    let (mut msgs, mut empty_assistant, mut calls, mut calls_without_result, mut titled) =
        (0usize, 0usize, 0usize, 0usize, 0usize);
    let mut slowest = (0u128, String::new());
    for dir in &dirs {
        // Replacing every '-' with '/' round-trips through escape_project_path to the same directory name.
        let project = PathBuf::from(dir.replace('-', "/"));
        let list = sessions.list_sessions(&project).await.expect("list_sessions");
        n_sessions += list.len();
        for s in &list {
            if s.title.is_some() {
                titled += 1;
            }
            let started = Instant::now();
            match sessions.load_session(&project, &s.id).await {
                Ok(session) => {
                    n_loaded += 1;
                    msgs += session.messages.len();
                    for m in &session.messages {
                        if m.role == MessageRole::Assistant && m.content.is_empty() && m.tool_calls.is_empty() {
                            empty_assistant += 1;
                        }
                        for c in &m.tool_calls {
                            calls += 1;
                            if c.result.is_none() {
                                calls_without_result += 1;
                            }
                        }
                    }
                }
                Err(e) => {
                    n_failed += 1;
                    println!("LOAD FAILED {dir}/{}: {e}", s.id.0);
                }
            }
            let ms = started.elapsed().as_millis();
            if ms > slowest.0 {
                slowest = (ms, format!("{dir}/{}", s.id.0));
            }
        }
    }
    println!("project dirs: {}", dirs.len());
    println!("sessions listed: {n_sessions}, titled: {titled}, loaded ok: {n_loaded}, failed: {n_failed}");
    println!("messages: {msgs}, empty assistant msgs: {empty_assistant}, tool calls: {calls}, without result: {calls_without_result}");
    println!("slowest load: {} ms ({})", slowest.0, slowest.1);

    let project = PathBuf::from("/Users/james/Developer/forge");
    let skills = ClaudeSkillAdapter::new(home.clone()).list_skills(&project).await.unwrap();
    println!("skills: {}", skills.len());
    let plugins = ClaudePluginAdapter::new(home.clone()).list_plugins().await.unwrap();
    println!("plugins: {}", plugins.len());
    let mcp = ClaudeMcpAdapter::new(default_claude_json_path().unwrap())
        .list_servers(&project)
        .await
        .unwrap();
    println!("mcp servers: {}", mcp.len());
}
```
