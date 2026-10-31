# Fork a session and effort Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The session menu forks a session (copy of the whole transcript, opened at once); the composer gets an effort control that follows the SDK; the model ids are checked against the real SDK.

**Architecture:** Fork is a `SessionRepositoryPort` operation (the `claude-fs` adapter copies the transcript with new uuids, like rename and delete edit transcripts; no sidecar). Effort follows Plan 16's model path end to end: `set_effort` command, `effort_set` / `setting_failed` answers, two paths in the live-session actor (running sidecar: view follows the answer; none: change now). The sidecar also reports `models_available` (from `supportedModels()`) so the UI shows only the levels the model supports.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, tokio, serde, `uuid` 1 (already in `Cargo.lock`), `time` 0.3, mockall; sidecar in TypeScript (vitest, Claude Agent SDK 0.3.283).

**Spec:** `docs/superpowers/specs/2026-10-03-fork-effort-design.md` (read it first; it holds the SDK facts and the desktop behaviour this copies).

## Global Constraints

- Run tests per crate (`cargo test -p <crate>`), the whole workspace and `cargo clippy --workspace --all-targets` at the end of each task that touches several crates; `cd sidecar && npx vitest run && npx tsc --noEmit` for the sidecar.
- A fork changes nothing in the source transcript, writes the copy through a temporary name then renames it, and refuses ids that could leave the project directory (`valid_session_id`).
- The fork's title is `"<title> (fork)"`, at most 200 characters (`MAX_TITLE_CHARS`).
- Menu keys are matched by physical key (`event.code()`: `KeyF`), because of the Korean input method (Plan 15).
- Wire names: command `set_effort { id, effort }`, event `effort_set { effort }`, event `models_available { models: [{value, display_name, effort_levels}] }`, `start_session.effort`. Levels on the wire: `low`, `medium`, `high`, `xhigh`, `max`. `null` is the SDK default.
- A refusal of an effort change is `setting_failed`, never `error` (an `error` ends the sidecar).
- Colors in `theme.css` come from the theme variables (`var(--…)`); no hex or `rgb`.
- No "recommended" mark on the effort control (the SDK has no such field).
- Commit messages end with: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A transcript line that is not valid JSON, or a line without `uuid`, must not break a fork (it is copied; the fork still lists and loads). Pinned in Task 1.
2. A parent that is not in the file (a transcript cut short) must keep its `parentUuid` instead of pointing at nothing new. Pinned in Task 1.
3. Forking a session whose transcript is missing, or whose id contains `/` or `..`, must fail cleanly and create no file. Pinned in Task 1.
4. A long title must not make the fork title exceed 200 characters. Pinned in Task 2.
5. A model that takes no effort, or that `supportedModels()` does not list, must show no effort control (not an empty slider). Pinned in Tasks 4 and 6.

---

### Task 1: Fork copies a transcript (port, adapter)

**Files:**
- Modify: `crates/forge-ports/src/session_port.rs` (error and method)
- Modify: `crates/forge-adapter-claude-fs/Cargo.toml` (`uuid`)
- Create: `crates/forge-adapter-claude-fs/src/fork.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (`mod fork;`)
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (the method, tests)
- Modify: `crates/forge-application/src/registry.rs` (the `SlowRepository` test stub at the `impl SessionRepositoryPort for SlowRepository`)

**Interfaces:**
- Consumes: `valid_session_id`, `ClaudeSessionAdapter::project_dir`, `write_session`/`project()`/`custom_title` test helpers (all in `sessions.rs`).
- Produces: `SessionRepositoryPort::fork_session(&self, project_path: &Path, session_id: &SessionId, title: &str) -> Result<SessionId, SessionRepositoryError>`; `SessionRepositoryError::ForkFailed(String)`; `MockSessionRepositoryPort::expect_fork_session`; `fork::forked_transcript(source, old_id, new_id, title, now, new_uuid) -> String`.

- [ ] **Step 1: Write the failing tests for the pure copy.** Create `crates/forge-adapter-claude-fs/src/fork.rs` with only the tests and a stub signature:

```rust
use serde_json::Value;

/// Stub, replaced in step 3.
pub(crate) fn forked_transcript(
    _source: &str,
    _old_id: &str,
    _new_id: &str,
    _title: &str,
    _now: &str,
    _new_uuid: impl FnMut() -> String,
) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn counter() -> impl FnMut() -> String {
        let mut n = 0;
        move || {
            n += 1;
            format!("new-{n}")
        }
    }

    fn parse(text: &str) -> Vec<Value> {
        text.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    fn source(lines: &[Value]) -> String {
        lines.iter().map(|line| format!("{line}\n")).collect()
    }

    #[test]
    fn every_message_gets_a_new_uuid_a_remapped_parent_the_new_session_id_and_a_forked_from() {
        let text = source(&[
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "assistant", "uuid": "u2", "parentUuid": "u1", "sessionId": "old"}),
        ]);

        let out = parse(&forked_transcript(&text, "old", "new", "T (fork)", "2026-10-03T00:00:00Z", counter()));

        assert_eq!(out[0]["uuid"], "new-1");
        assert!(out[0]["parentUuid"].is_null());
        assert_eq!(out[1]["uuid"], "new-2");
        assert_eq!(out[1]["parentUuid"], "new-1");
        assert_eq!(out[0]["sessionId"], "new");
        assert_eq!(out[1]["sessionId"], "new");
        assert_eq!(out[0]["forkedFrom"], json!({"sessionId": "old", "messageUuid": "u1"}));
        assert_eq!(out[1]["forkedFrom"], json!({"sessionId": "old", "messageUuid": "u2"}));
    }

    #[test]
    fn the_old_title_is_dropped_and_one_new_title_line_ends_the_file() {
        let text = source(&[
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "custom-title", "customTitle": "Old", "sessionId": "old"}),
        ]);

        let out = parse(&forked_transcript(&text, "old", "new", "Old (fork)", "2026-10-03T00:00:00Z", counter()));

        assert_eq!(out.len(), 2, "the old title line is not copied");
        assert_eq!(out[1]["type"], "custom-title");
        assert_eq!(out[1]["customTitle"], "Old (fork)");
        assert_eq!(out[1]["sessionId"], "new");
        assert_eq!(out[1]["uuid"], "new-2");
        assert_eq!(out[1]["timestamp"], "2026-10-03T00:00:00Z");
    }

    #[test]
    fn a_line_that_is_not_json_is_kept_as_it_is() {
        let text = format!("{}\nnot json at all\n", json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}));

        let out = forked_transcript(&text, "old", "new", "T", "now", counter());

        assert!(out.lines().any(|line| line == "not json at all"));
        assert_eq!(out.lines().count(), 3, "two copied lines and the title");
    }

    #[test]
    fn a_parent_that_is_not_in_the_file_is_left_as_it_is() {
        let text = source(&[json!({"type": "user", "uuid": "u2", "parentUuid": "gone", "sessionId": "old"})]);

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out[0]["parentUuid"], "gone");
        assert_eq!(out[0]["uuid"], "new-1");
    }

    #[test]
    fn a_line_without_a_uuid_only_gets_the_new_session_id() {
        let text = source(&[json!({"type": "permission-mode", "permissionMode": "plan", "sessionId": "old"})]);

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out[0]["sessionId"], "new");
        assert_eq!(out[0]["permissionMode"], "plan");
        assert!(out[0].get("forkedFrom").is_none());
        assert!(out[0].get("uuid").is_none());
    }

    #[test]
    fn a_source_without_a_final_newline_and_with_blank_lines_is_fine() {
        let text = format!(
            "{}\n\n{}",
            json!({"type": "user", "uuid": "u1", "parentUuid": null, "sessionId": "old"}),
            json!({"type": "user", "uuid": "u2", "parentUuid": "u1", "sessionId": "old"})
        );

        let out = parse(&forked_transcript(&text, "old", "new", "T", "now", counter()));

        assert_eq!(out.len(), 3);
        assert_eq!(out[1]["parentUuid"], "new-1");
    }
}
```

Add `mod fork;` after `mod sessions;` in `crates/forge-adapter-claude-fs/src/lib.rs`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-adapter-claude-fs fork`
Expected: FAIL (the stub returns an empty string; `out[0]` panics).

- [ ] **Step 3: Write the copy.** Replace the stub in `fork.rs` (keep the `tests` module):

```rust
use std::collections::HashMap;

use serde_json::{json, Value};

enum Line<'a> {
    Json(Value),
    Raw(&'a str),
}

fn is_title(value: &Value) -> bool {
    value.get("type").and_then(Value::as_str) == Some("custom-title")
}

/// The text of a fork's transcript, made from the text of the source's. This is what the SDK's
/// `forkSession` does (seen on a probe transcript, spec 2026-10-03-fork-effort-design.md): every
/// line with a `uuid` gets a new one and a `forkedFrom: {sessionId, messageUuid}`; `parentUuid`
/// follows the new uuids (a parent that is not in the file stays); `sessionId` becomes the new id;
/// the old `custom-title` lines are dropped and one new one, with `title`, ends the file. A line
/// that is not a JSON object is copied as it is; blank lines are dropped.
pub(crate) fn forked_transcript(
    source: &str,
    old_id: &str,
    new_id: &str,
    title: &str,
    now: &str,
    mut new_uuid: impl FnMut() -> String,
) -> String {
    let lines: Vec<Line> = source
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| match serde_json::from_str::<Value>(line) {
            Ok(value @ Value::Object(_)) => Line::Json(value),
            _ => Line::Raw(line),
        })
        .collect();

    // Every uuid first, so a child that comes before its parent still finds it.
    let mut ids: HashMap<String, String> = HashMap::new();
    for line in &lines {
        if let Line::Json(value) = line {
            if is_title(value) {
                continue;
            }
            if let Some(old) = value.get("uuid").and_then(Value::as_str) {
                ids.entry(old.to_string()).or_insert_with(|| new_uuid());
            }
        }
    }

    let mut out = String::new();
    for line in lines {
        match line {
            Line::Raw(text) => out.push_str(text),
            Line::Json(mut value) => {
                if is_title(&value) {
                    continue;
                }
                let old_uuid = value.get("uuid").and_then(Value::as_str).map(str::to_string);
                let object = value.as_object_mut().expect("only objects are parsed as JSON lines");
                if let Some(old) = &old_uuid {
                    object.insert("uuid".to_string(), json!(ids[old]));
                    object.insert("forkedFrom".to_string(), json!({"sessionId": old_id, "messageUuid": old}));
                }
                let parent = object.get("parentUuid").and_then(Value::as_str).and_then(|p| ids.get(p)).cloned();
                if let Some(parent) = parent {
                    object.insert("parentUuid".to_string(), json!(parent));
                }
                if object.contains_key("sessionId") {
                    object.insert("sessionId".to_string(), json!(new_id));
                }
                out.push_str(&value.to_string());
            }
        }
        out.push('\n');
    }
    out.push_str(&json!({"type": "custom-title", "customTitle": title, "sessionId": new_id, "uuid": new_uuid(), "timestamp": now}).to_string());
    out.push('\n');
    out
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-adapter-claude-fs fork`
Expected: PASS (6 tests).

- [ ] **Step 5: Add the port method and error.** In `crates/forge-ports/src/session_port.rs` add to `SessionRepositoryError` after `DeleteFailed`:

```rust
    #[error("failed to fork session: {0}")]
    ForkFailed(String),
```

and to the trait, after `delete_session`:

```rust
    /// Copies the session's whole transcript into a new session of the same project and gives the
    /// copy `title`. The source is not touched. Returns the new session's id.
    async fn fork_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<SessionId, SessionRepositoryError>;
```

In `crates/forge-application/src/registry.rs`, in the `impl SessionRepositoryPort for SlowRepository` test stub, add next to its `rename_session` stub:

```rust
        async fn fork_session(&self, _project_path: &Path, _session_id: &SessionId, _title: &str) -> Result<SessionId, SessionRepositoryError> {
            unimplemented!("not used by these tests")
        }
```

- [ ] **Step 6: Write the failing adapter tests.** In `crates/forge-adapter-claude-fs/Cargo.toml` add under `[dependencies]`: `uuid = { version = "1", features = ["v4"] }`. In the `tests` module of `sessions.rs`, add the helper and tests:

```rust
    fn message(uuid: &str, parent: Option<&str>, text: &str) -> Value {
        json!({"type": "user", "uuid": uuid, "parentUuid": parent, "sessionId": "s1", "isSidechain": false,
               "message": {"role": "user", "content": text}})
    }

    #[tokio::test]
    async fn fork_makes_a_new_session_that_list_and_load_read_back() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[message("u1", None, "hello"), message("u2", Some("u1"), "again"), custom_title("Old")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let new_id = adapter.fork_session(&project(), &SessionId("s1".to_string()), "Old (fork)").await.unwrap();

        assert_ne!(new_id.0, "s1");
        let listed = adapter.list_sessions(&project()).await.unwrap();
        let titles: Vec<(String, Option<String>)> = listed.iter().map(|s| (s.id.0.clone(), s.title.clone())).collect();
        assert_eq!(listed.len(), 2);
        assert!(titles.contains(&("s1".to_string(), Some("Old".to_string()))));
        assert!(titles.contains(&(new_id.0.clone(), Some("Old (fork)".to_string()))));
        let loaded = adapter.load_session(&project(), &new_id).await.unwrap();
        assert_eq!(loaded.messages.len(), 2);
    }

    #[tokio::test]
    async fn fork_leaves_the_source_untouched_and_no_temporary_file_behind() {
        let home = tempfile::tempdir().unwrap();
        let source = write_session(home.path(), &project(), "s1", &[message("u1", None, "hello")]);
        let before = std::fs::read(&source).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        adapter.fork_session(&project(), &SessionId("s1".to_string()), "T").await.unwrap();

        assert_eq!(std::fs::read(&source).unwrap(), before);
        let names: Vec<String> = std::fs::read_dir(source.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(names.len(), 2);
        assert!(names.iter().all(|name| name.ends_with(".jsonl")), "{names:?}");
    }

    #[tokio::test]
    async fn fork_keeps_a_broken_line_and_the_fork_still_lists() {
        let home = tempfile::tempdir().unwrap();
        let path = write_session(home.path(), &project(), "s1", &[message("u1", None, "hello")]);
        std::fs::write(&path, format!("{}not json\n", std::fs::read_to_string(&path).unwrap())).unwrap();
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let new_id = adapter.fork_session(&project(), &SessionId("s1".to_string()), "T").await.unwrap();

        let copy = std::fs::read_to_string(path.parent().unwrap().join(format!("{}.jsonl", new_id.0))).unwrap();
        assert!(copy.lines().any(|line| line == "not json"));
        assert!(adapter.list_sessions(&project()).await.unwrap().iter().any(|s| s.id == new_id));
    }

    #[tokio::test]
    async fn fork_of_a_missing_session_is_not_found_and_creates_nothing() {
        let home = tempfile::tempdir().unwrap();
        let other = write_session(home.path(), &project(), "s1", &[message("u1", None, "hello")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let result = adapter.fork_session(&project(), &SessionId("nope".to_string()), "T").await;

        assert!(matches!(result, Err(SessionRepositoryError::NotFound(_))));
        assert_eq!(std::fs::read_dir(other.parent().unwrap()).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn fork_refuses_ids_that_could_leave_the_project_directory() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[message("u1", None, "hello")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        for id in ["../s1", "a/b", "..", "", ".hidden"] {
            let result = adapter.fork_session(&project(), &SessionId(id.to_string()), "T").await;
            assert!(matches!(result, Err(SessionRepositoryError::NotFound(_))), "{id:?}");
        }
    }
```

- [ ] **Step 7: Run to see them fail**

Run: `cargo test -p forge-adapter-claude-fs fork_`
Expected: compile error "not all trait items implemented, missing: `fork_session`".

- [ ] **Step 8: Implement the adapter method.** In `sessions.rs` add above `impl SessionRepositoryPort for ClaudeSessionAdapter`:

```rust
fn fork_error(path: &Path, e: std::io::Error) -> SessionRepositoryError {
    SessionRepositoryError::ForkFailed(format!("{}: {e}", path.display()))
}
```

and inside the impl, after `delete_session`:

```rust
    async fn fork_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<SessionId, SessionRepositoryError> {
        let old = &session_id.0;
        if !valid_session_id(old) {
            return Err(SessionRepositoryError::NotFound(session_id.clone()));
        }
        let dir = self.project_dir(project_path).await;
        let source = dir.join(format!("{old}.jsonl"));
        let bytes = match tokio::fs::read(&source).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionRepositoryError::NotFound(session_id.clone()))
            }
            Err(e) => return Err(fork_error(&source, e)),
        };
        let new = uuid::Uuid::new_v4().to_string();
        let now = time::OffsetDateTime::now_utc().format(&Rfc3339).unwrap_or_default();
        let text = crate::fork::forked_transcript(
            &String::from_utf8_lossy(&bytes),
            old,
            &new,
            title,
            &now,
            || uuid::Uuid::new_v4().to_string(),
        );
        // A temporary name that no listing reads, then a rename: a half-written copy is never listed.
        let target = dir.join(format!("{new}.jsonl"));
        let temp = dir.join(format!(".{new}.jsonl.tmp"));
        if let Err(e) = tokio::fs::write(&temp, text).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(fork_error(&target, e));
        }
        if let Err(e) = tokio::fs::rename(&temp, &target).await {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(fork_error(&target, e));
        }
        Ok(SessionId(new))
    }
```

- [ ] **Step 9: Run to see them pass; check the workspace builds**

Run: `cargo test -p forge-adapter-claude-fs && cargo build --workspace --tests`
Expected: PASS; the build compiles (the mock gained `expect_fork_session` automatically).

- [ ] **Step 10: Compare with the real SDK once.** Run the SDK's `forkSession` and the adapter on the same transcript and compare the key sets. Needs `sidecar/node_modules` (`cd sidecar && npm ci`; or use another worktree's, e.g. `/Users/james/orca/workspaces/forge/plan-18/sidecar/node_modules`):

```bash
S=/private/tmp/claude-501/-Users-james-orca-workspaces-forge-plan-19/a1cf7a44-dd53-40db-9d1d-b3d29c8bf75c/scratchpad/forkprobe
cd $S && CLAUDE_CONFIG_DIR=$S/home node run.mjs   # the probe from the spec run: prints {"sessionId": ...}
python3 - <<'EOF'
import json,glob,os
S=os.environ.get("S","/private/tmp/claude-501/-Users-james-orca-workspaces-forge-plan-19/a1cf7a44-dd53-40db-9d1d-b3d29c8bf75c/scratchpad/forkprobe")
for f in sorted(glob.glob(S+"/home/projects/-tmp-forkprobe/*.jsonl")):
    for l in open(f):
        print(os.path.basename(f)[:8], sorted(json.loads(l).keys()))
EOF
```
Expected: the SDK's copy has, per message, the keys of the source plus `forkedFrom`, and a `custom-title` line with `uuid` and `timestamp`: the same shape `forked_transcript` writes (the unit tests above pin it). If the key sets differ from the unit tests' expectations, fix `forked_transcript` and its tests before going on.

- [ ] **Step 11: Commit**

```bash
git add crates/forge-ports crates/forge-adapter-claude-fs crates/forge-application/src/registry.rs Cargo.lock
git commit -m "feat: fork_session copies a transcript into a new session

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Fork from the session menu (registry, workspace, menu, key F)

**Files:**
- Modify: `crates/forge-application/src/registry.rs` (`fork_title`, `SessionRegistry::fork`, tests)
- Modify: `crates/forge-application/src/lib.rs` (re-export `fork_title` if `normalize_title` is re-exported there)
- Modify: `crates/forge-features/src/sidebar.rs` (`RowActions.can_fork`, `MenuKeyAction::Fork`, tests)
- Modify: `crates/forge-features/src/workspace.rs` (`fork_session`)
- Modify: `crates/forge-widgets/src/session_row.rs` (menu item, key)

**Interfaces:**
- Consumes: `SessionRepositoryPort::fork_session` (Task 1); `Workspace::{reload via replace_project_sessions, open_past, report}`.
- Produces: `fork_title(&str) -> String`; `SessionRegistry::fork(&self, &Path, &SessionId, &str) -> Result<SessionId, ManageSessionError>`; `RowActions.can_fork: bool`; `MenuKeyAction::Fork`; `Workspace::fork_session(&self, project: PathBuf, session: SessionId, title: String)`.

- [ ] **Step 1: Write the failing registry tests.** In the `tests` module of `registry.rs` (next to the rename tests; `registry(...)`, `MockSessionRepositoryPort`, `Path`, `SessionId` are in scope there):

```rust
    #[test]
    fn the_fork_title_adds_the_suffix_and_stays_within_the_limit() {
        assert_eq!(fork_title("Mine"), "Mine (fork)");
        assert_eq!(fork_title("  Mine  "), "Mine (fork)");
        assert_eq!(fork_title("   "), "Session (fork)");
        let long = fork_title(&"a".repeat(300));
        assert_eq!(long.chars().count(), 200);
        assert!(long.ends_with(" (fork)"));
    }

    #[tokio::test]
    async fn fork_asks_the_repository_for_a_copy_with_the_fork_title() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_fork_session()
            .withf(|p: &Path, id: &SessionId, title: &str| p == Path::new("/work/a") && id.0 == "claude-1" && title == "Mine (fork)")
            .times(1)
            .returning(|_, _, _| Ok(SessionId("claude-2".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let new_id = registry.fork(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Mine").await.unwrap();

        assert_eq!(new_id.0, "claude-2");
    }

    #[tokio::test]
    async fn a_failed_fork_is_an_error_from_the_repository() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_fork_session().returning(|_, _, _| Err(SessionRepositoryError::ForkFailed("disk full".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.fork(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Mine").await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::ForkFailed(_)))));
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-application fork`
Expected: compile error (`fork_title`, `fork` not found).

- [ ] **Step 3: Implement.** In `registry.rs`, below `normalize_title`:

```rust
const FORK_SUFFIX: &str = " (fork)";

/// The title of a fork of a session called `title`: the title and " (fork)", cut so the whole stays
/// within the title limit. A blank title gives "Session (fork)".
pub fn fork_title(title: &str) -> String {
    let base = title.trim();
    let base = if base.is_empty() { "Session" } else { base };
    let room = MAX_TITLE_CHARS - FORK_SUFFIX.chars().count();
    let base: String = base.chars().take(room).collect();
    format!("{base}{FORK_SUFFIX}")
}
```

and in `impl SessionRegistry`, after `delete`:

```rust
    /// Copies a past or open session into a new one called "<title> (fork)" and returns the new id.
    /// The source is untouched; an open session's copy holds what is on disk now.
    pub async fn fork(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<SessionId, ManageSessionError> {
        Ok(self.sessions.fork_session(project_path, session_id, &fork_title(title)).await?)
    }
```

If `normalize_title` is re-exported in `forge-application/src/lib.rs`, add `fork_title` to that `pub use`.

- [ ] **Step 4: Run to see them pass**

Run: `cargo test -p forge-application fork`
Expected: PASS (3 tests).

- [ ] **Step 5: Write the failing menu tests.** In `crates/forge-features/src/sidebar.rs` tests, replace the helper and the three tests that name `RowActions { can_rename, delete }`:

```rust
    fn actions(can_rename: bool, delete: DeleteAction) -> RowActions {
        // A session that can be renamed has a Claude id, and so can be forked.
        RowActions { can_rename, can_fork: can_rename, delete }
    }
```

In the three `actions()` assertions change the right-hand side to include `can_fork`: past `RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Allowed }`, open `RowActions { can_rename: true, can_fork: true, delete: DeleteAction::WhileOpen }`, fresh `RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable }`. Replace the line `assert_eq!(menu_key_action("KeyF", false, all), None, "fork is not built");` with nothing, and add:

```rust
    #[test]
    fn the_f_key_forks_when_the_session_has_an_id() {
        let all = actions(true, DeleteAction::Allowed);
        assert_eq!(menu_key_action("KeyF", false, all), Some(MenuKeyAction::Fork));
        assert_eq!(menu_key_action("KeyF", false, actions(true, DeleteAction::WhileOpen)), Some(MenuKeyAction::Fork), "an open session can be forked");
        assert_eq!(menu_key_action("KeyF", false, actions(false, DeleteAction::Unavailable)), None, "no Claude id yet");
        assert_eq!(menu_key_action("KeyF", true, all), None, "⌘F");
    }
```

- [ ] **Step 6: Run to see them fail**

Run: `cargo test -p forge-features sidebar`
Expected: compile error (`can_fork`, `MenuKeyAction::Fork`).

- [ ] **Step 7: Implement the menu model.** In `sidebar.rs`:

```rust
pub struct RowActions {
    pub can_rename: bool,
    pub can_fork: bool,
    pub delete: DeleteAction,
}
```

`actions()`:

```rust
    pub fn actions(&self) -> RowActions {
        match (&self.key, &self.session_id) {
            (_, None) => RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable },
            (EntryKey::Live(_), Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::WhileOpen },
            (EntryKey::Past { .. }, Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Allowed },
        }
    }
```

`MenuKeyAction` gains `Fork,` (between `Rename` and `Delete`) and `menu_key_action` gains the arm `"KeyF" if actions.can_fork => Some(MenuKeyAction::Fork),` after the `KeyR` arm.

- [ ] **Step 8: Run to see them pass**

Run: `cargo test -p forge-features sidebar`
Expected: PASS.

- [ ] **Step 9: Workspace method.** In `crates/forge-features/src/workspace.rs`, after `delete_session`:

```rust
    /// Forks a session (open or not): the copy is made, the project's list is re-read and the copy is
    /// opened. A failure shows as a notice and nothing is opened.
    pub fn fork_session(&self, project: PathBuf, session: SessionId, title: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        // Root scope: a disk mutation must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            let new_id = match services.registry.fork(&project, &session, &title).await {
                Ok(id) => id,
                Err(error) => return this.report(error.to_string()),
            };
            match services.sessions.list_sessions(&project).await {
                Ok(sessions) => replace_project_sessions(&mut past.write(), &project, sessions),
                Err(error) => return this.report(format!("{}: {error}", project.display())),
            }
            let summary = past.peek().iter().find(|(p, _)| p.path == project).map(|(p, _)| p.clone());
            match summary {
                Some(summary) => this.open_past(summary, new_id),
                None => this.report(format!("{}: the project is no longer listed", project.display())),
            }
        });
    }
```

- [ ] **Step 10: Menu item and key in the row.** In `crates/forge-widgets/src/session_row.rs`:

1. `let has_menu = actions.can_rename || actions.can_fork || actions.delete != DeleteAction::Unavailable;` and the button `title: "Rename, fork or delete"`.
2. In the `onkeydown` match add, between the Rename and Delete arms:

```rust
                                        Some(MenuKeyAction::Fork) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            if let Some(id) = session_id.clone() {
                                                workspace.fork_session(project.clone(), id, title.clone());
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
```

`session_id` and `project` are captured by `move`; the `onkeydown` closure already clones `title`. If the compiler asks, add `let project = project.clone(); let session_id = session_id.clone();` next to `let title = entry.title.clone();` above the closure.
3. Between the Rename and Delete buttons in the menu:

```rust
                            if actions.can_fork {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let project = project.clone();
                                        let session_id = session_id.clone();
                                        let title = entry.title.clone();
                                        move |_| {
                                            if let Some(id) = session_id.clone() {
                                                workspace.fork_session(project.clone(), id, title.clone());
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
                                    },
                                    span { "Fork" }
                                    span { class: "menu-key", "F" }
                                }
                            }
```

- [ ] **Step 11: Build, test, lint**

Run: `cargo test -p forge-application -p forge-features -p forge-widgets && cargo clippy --workspace --all-targets`
Expected: PASS, no warnings. (The UI itself is checked in Task 8.)

- [ ] **Step 12: Commit**

```bash
git add crates
git commit -m "feat: fork a session from the session menu, key F

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Effort in the domain, ports and wire (Rust)

**Files:**
- Create: `crates/forge-domain/src/effort.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-adapter-sidecar/src/protocol.rs`, `crates/forge-adapter-sidecar/src/process.rs`
- Modify: every `StartSessionParams { … }` (compiler lists them): `crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs`, `…/tests/contract_test.rs`, `…/tests/factory_test.rs`, `crates/forge-application/src/command_probe.rs`, `…/use_cases/session_lifecycle.rs`, `…/live_session.rs` (test helper), `…/live_session_actor.rs`, `crates/forge-application/src/test_support.rs` (fake sidecar)

**Interfaces:**
- Produces: `forge_domain::{EffortLevel, AvailableModel}`; `StartSessionParams.effort: Option<EffortLevel>`; `SidecarEvent::{EffortSet { effort: Option<EffortLevel> }, ModelsAvailable { models: Vec<AvailableModel> }}`; `SidecarPort::set_effort(&self, Option<EffortLevel>)`; `SidecarCommand::SetEffort`; test `Call::SetEffort(Option<EffortLevel>)`.

- [ ] **Step 1: Write the failing domain tests.** Create `crates/forge-domain/src/effort.rs`:

```rust
use serde::{Deserialize, Serialize};

/// How much thinking the model does for a turn (the SDK's `EffortLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

impl EffortLevel {
    pub const ALL: [EffortLevel; 5] = [EffortLevel::Low, EffortLevel::Medium, EffortLevel::High, EffortLevel::XHigh, EffortLevel::Max];

    pub fn label(self) -> &'static str {
        match self {
            EffortLevel::Low => "Low",
            EffortLevel::Medium => "Medium",
            EffortLevel::High => "High",
            EffortLevel::XHigh => "Extra high",
            EffortLevel::Max => "Max",
        }
    }
}

/// A model the SDK lists for the running session, and the effort levels it takes (empty: none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableModel {
    pub value: String,
    pub display_name: String,
    #[serde(default)]
    pub effort_levels: Vec<EffortLevel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_use_the_sdk_names_on_the_wire() {
        let json: Vec<String> = EffortLevel::ALL.iter().map(|l| serde_json::to_string(l).unwrap()).collect();
        assert_eq!(json, vec!["\"low\"", "\"medium\"", "\"high\"", "\"xhigh\"", "\"max\""]);
        assert_eq!(serde_json::from_str::<EffortLevel>("\"xhigh\"").unwrap(), EffortLevel::XHigh);
    }

    #[test]
    fn levels_are_ordered_from_low_to_max() {
        let mut sorted = EffortLevel::ALL.to_vec();
        sorted.reverse();
        sorted.sort();
        assert_eq!(sorted, EffortLevel::ALL.to_vec());
    }

    #[test]
    fn an_available_model_without_levels_reads_as_taking_none() {
        let model: AvailableModel = serde_json::from_str(r#"{"value":"m","display_name":"M"}"#).unwrap();
        assert!(model.effort_levels.is_empty());
    }
}
```

In `crates/forge-domain/src/lib.rs` add `mod effort;` (after `mod config;`) and `pub use effort::{AvailableModel, EffortLevel};`.

- [ ] **Step 2: Run to see them pass** (the file holds both the code and its tests)

Run: `cargo test -p forge-domain effort`
Expected: PASS (3 tests).

- [ ] **Step 3: Write the failing port and wire tests.** In `crates/forge-ports/src/sidecar_port.rs` tests add:

```rust
    #[test]
    fn effort_events_read_from_the_wire() {
        let set: SidecarEvent = serde_json::from_str(r#"{"type":"effort_set","effort":"xhigh"}"#).unwrap();
        assert_eq!(set, SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::XHigh) });
        let cleared: SidecarEvent = serde_json::from_str(r#"{"type":"effort_set","effort":null}"#).unwrap();
        assert_eq!(cleared, SidecarEvent::EffortSet { effort: None });
        let models: SidecarEvent = serde_json::from_str(
            r#"{"type":"models_available","models":[{"value":"m","display_name":"M","effort_levels":["low","high"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            models,
            SidecarEvent::ModelsAvailable {
                models: vec![forge_domain::AvailableModel {
                    value: "m".to_string(),
                    display_name: "M".to_string(),
                    effort_levels: vec![forge_domain::EffortLevel::Low, forge_domain::EffortLevel::High],
                }]
            }
        );
    }
```

In `crates/forge-adapter-sidecar/src/protocol.rs` tests add (it already has `params_with(Vec<String>) -> StartSessionParams`):

```rust
    #[test]
    fn set_effort_serializes_a_level_and_the_default_as_null() {
        let named = SidecarCommand::SetEffort { id: "6".to_string(), effort: Some(forge_domain::EffortLevel::XHigh) };
        let default = SidecarCommand::SetEffort { id: "7".to_string(), effort: None };

        assert_eq!(serde_json::to_value(&named).unwrap(), serde_json::json!({"type": "set_effort", "id": "6", "effort": "xhigh"}));
        assert_eq!(serde_json::to_value(&default).unwrap(), serde_json::json!({"type": "set_effort", "id": "7", "effort": null}));
    }

    #[test]
    fn start_session_carries_the_effort() {
        let mut params = params_with(Vec::new());
        params.effort = Some(forge_domain::EffortLevel::High);
        let started = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params)).unwrap();
        assert_eq!(started["effort"], "high");

        let default = serde_json::to_value(SidecarCommand::start_session("2".to_string(), params_with(Vec::new()))).unwrap();
        assert!(default["effort"].is_null());
    }
```

- [ ] **Step 4: Run to see them fail**

Run: `cargo test -p forge-ports -p forge-adapter-sidecar`
Expected: compile errors (`EffortSet`, `SetEffort`, `effort` field not found).

- [ ] **Step 5: Implement the ports and wire.**

`crates/forge-ports/src/sidecar_port.rs`: import `AvailableModel, EffortLevel` from `forge_domain`; add to `StartSessionParams` after `model`:

```rust
    /// The effort the session starts with; `None` is the SDK's default.
    pub effort: Option<EffortLevel>,
```

add to `SidecarEvent` after `ModelSet`:

```rust
    /// The SDK accepted a `set_effort`; `None` is its default.
    EffortSet { effort: Option<EffortLevel> },
    /// The models the SDK lists for this session and the effort levels each takes. Sent once after the start.
    ModelsAvailable { models: Vec<AvailableModel> },
```

change the doc of `SettingFailed` to "…refused a `set_permission_mode`, `set_model` or `set_effort`", and add to the trait after `set_model`:

```rust
    /// Asks for an effort change in the running session (`None`: the default). The answer comes back
    /// as an event.
    async fn set_effort(&self, effort: Option<EffortLevel>) -> Result<(), SidecarError>;
```

`crates/forge-adapter-sidecar/src/protocol.rs`: in `StartSession` add `effort: Option<forge_domain::EffortLevel>,` after `model`; in `SidecarCommand::start_session` add `effort: params.effort,`; add the variant after `SetModel`:

```rust
    /// `None` is the SDK's default effort.
    SetEffort { id: String, effort: Option<forge_domain::EffortLevel> },
```

`crates/forge-adapter-sidecar/src/process.rs`, after `set_model`:

```rust
    async fn set_effort(&self, effort: Option<forge_domain::EffortLevel>) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::SetEffort { id, effort })
    }
```

`crates/forge-application/src/test_support.rs`: add `SetEffort(Option<forge_domain::EffortLevel>),` to `Call` and to the fake, after `set_model`:

```rust
    async fn set_effort(&self, effort: Option<forge_domain::EffortLevel>) -> Result<(), SidecarError> {
        self.control.record(Call::SetEffort(effort));
        Ok(())
    }
```

Then add `effort: None,` to every `StartSessionParams { … }` the compiler reports (the list is in **Files**); in `live_session_actor.rs` use `effort: None,` for now (Task 5 changes it).

- [ ] **Step 6: Run to see them pass**

Run: `cargo test -p forge-domain -p forge-ports -p forge-adapter-sidecar && cargo build --workspace --tests`
Expected: PASS and the whole workspace compiles.

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat: effort levels in the domain, the sidecar port and the wire

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The sidecar sets effort and reports the models (TypeScript)

**Files:**
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Consumes: the wire names from Task 3.
- Produces: command `set_effort`; events `effort_set`, `models_available`; `start_session.effort` passed as the SDK option `effort`.

- [ ] **Step 1: Install the sidecar's dependencies** (this worktree has no `node_modules`)

Run: `cd sidecar && npm ci`
Expected: installs without error; `npx vitest run` passes before any change.

- [ ] **Step 2: Write the failing tests.** In `sidecar/test/session.test.ts`, after the `describe("changing the mode and the model of a running session", …)` block, add (inside the outer `describe` that holds it):

```ts
  describe("effort and the models the SDK lists", () => {
    function openQuery(extra: Record<string, unknown> = {}) {
      let stopped = false;
      const flags: unknown[] = [];
      const queryFn: QueryFn = () => {
        const gen = (async function* () {
          while (!stopped) {
            await new Promise((resolve) => setTimeout(resolve, 1));
            yield { type: "noop" };
          }
        })();
        return Object.assign(
          gen,
          { interrupt: async () => undefined, applyFlagSettings: async (settings: unknown) => void flags.push(settings) },
          extra,
        );
      };
      return { queryFn, flags, stop: () => (stopped = true) };
    }

    async function running(open: ReturnType<typeof openQuery>) {
      const events: SidecarEvent[] = [];
      const session = new SidecarSession(open.queryFn, (e) => events.push(e), { commandsTimeoutMs: 50 });
      const started = session.handle(startCommand);
      await new Promise((resolve) => setTimeout(resolve, 10));
      return { session, events, finish: async () => (open.stop(), await started) };
    }

    it("sets the effort through applyFlagSettings and says so; null clears it", async () => {
      const open = openQuery();
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_effort", id: "2", effort: "xhigh" });
      await session.handle({ type: "set_effort", id: "3", effort: null });
      await finish();

      expect(open.flags).toEqual([{ effortLevel: "xhigh" }, { effortLevel: null }]);
      expect(events).toContainEqual({ type: "effort_set", effort: "xhigh" });
      expect(events).toContainEqual({ type: "effort_set", effort: null });
    });

    it("reports a refusal as setting_failed and not as a change", async () => {
      const open = openQuery({
        applyFlagSettings: async () => {
          throw new Error("effort is capped by your organization");
        },
      });
      const { session, events, finish } = await running(open);

      await session.handle({ type: "set_effort", id: "2", effort: "max" });
      await finish();

      expect(events).toContainEqual({ type: "setting_failed", message: "effort is capped by your organization" });
      expect(events.some((e) => e.type === "effort_set" || e.type === "error")).toBe(false);
    });

    it("says setting_failed when no session runs, and when the SDK cannot change effort", async () => {
      const idle: SidecarEvent[] = [];
      await new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => idle.push(e)).handle({ type: "set_effort", id: "1", effort: "low" });
      expect(idle.map((e) => e.type)).toEqual(["setting_failed"]);

      const open = openQuery({ applyFlagSettings: undefined });
      const { session, events, finish } = await running(open);
      await session.handle({ type: "set_effort", id: "2", effort: "low" });
      await finish();
      expect(events.some((e) => e.type === "setting_failed")).toBe(true);
      expect(events.some((e) => e.type === "effort_set")).toBe(false);
    });

    it("starts the SDK with the effort it is told, and without one when none is", async () => {
      const seen: any[] = [];
      const queryFn: QueryFn = (args) => {
        seen.push(args.options);
        return toQueryHandle((async function* () {})());
      };
      await new SidecarSession(queryFn, () => {}).handle({ ...startCommand, effort: "high" });
      await new SidecarSession(queryFn, () => {}).handle(startCommand);

      expect(seen[0].effort).toBe("high");
      expect(seen[1].effort).toBeUndefined();
    });

    it("sends the models the SDK lists, with only the effort levels Forge knows", async () => {
      const open = openQuery({
        supportedModels: async () => [
          { value: "claude-opus-5-5", displayName: "Opus 5.5", supportsEffort: true, supportedEffortLevels: ["low", "high", "ultra"] },
          { value: "claude-haiku-4-5-20251001", displayName: "Haiku 4.5", supportsEffort: false },
          { displayName: "no value" },
        ],
      });
      const { events, finish } = await running(open);
      await finish();

      expect(events).toContainEqual({
        type: "models_available",
        models: [
          { value: "claude-opus-5-5", display_name: "Opus 5.5", effort_levels: ["low", "high"] },
          { value: "claude-haiku-4-5-20251001", display_name: "Haiku 4.5", effort_levels: [] },
        ],
      });
    });

    it("sends nothing when the model list fails or is empty", async () => {
      for (const supportedModels of [async () => { throw new Error("no"); }, async () => []]) {
        const open = openQuery({ supportedModels });
        const { events, finish } = await running(open);
        await finish();
        expect(events.some((e) => e.type === "models_available")).toBe(false);
      }
    });
  });
```

- [ ] **Step 3: Run to see them fail**

Run: `cd sidecar && npx vitest run test/session.test.ts`
Expected: FAIL (type errors or no `effort_set`).

- [ ] **Step 4: Implement.** `sidecar/src/protocol.ts`: add

```ts
export type WireEffort = "low" | "medium" | "high" | "xhigh" | "max";

export interface WireAvailableModel {
  value: string;
  display_name: string;
  effort_levels: WireEffort[];
}
```

add `effort?: WireEffort | null;` after `model: string | null;` in `start_session`, add `| { type: "set_effort"; id: string; effort: WireEffort | null }` after `set_model`, and add after the `model_set` event:

```ts
  | { type: "effort_set"; effort: WireEffort | null }
  // The models the SDK lists for this session and the effort levels each takes; sent once after the start.
  | { type: "models_available"; models: WireAvailableModel[] }
```

`sidecar/src/session.ts`: import `WireAvailableModel, WireEffort` with the other protocol types; add to `QueryHandle`:

```ts
  applyFlagSettings?: (settings: Record<string, unknown>) => Promise<unknown>;
  supportedModels?: () => Promise<unknown>;
```

add above `withTimeout`:

```ts
const EFFORT_LEVELS: WireEffort[] = ["low", "medium", "high", "xhigh", "max"];

// The SDK's model list as Forge wants it: the levels are cut to the ones Forge knows, and a model
// that takes no effort has none.
function toWireModels(raw: unknown): WireAvailableModel[] {
  if (!Array.isArray(raw)) return [];
  const models: WireAvailableModel[] = [];
  for (const item of raw) {
    const model = item as { value?: unknown; displayName?: unknown; supportsEffort?: unknown; supportedEffortLevels?: unknown } | null;
    if (typeof model?.value !== "string" || model.value === "") continue;
    const offered = model.supportsEffort === false || !Array.isArray(model.supportedEffortLevels) ? [] : model.supportedEffortLevels;
    models.push({
      value: model.value,
      display_name: typeof model.displayName === "string" && model.displayName !== "" ? model.displayName : model.value,
      effort_levels: EFFORT_LEVELS.filter((level) => offered.includes(level)),
    });
  }
  return models;
}
```

add the command case after `set_model`:

```ts
      case "set_effort":
        await this.change(
          (query) =>
            query.applyFlagSettings
              ? query.applyFlagSettings({ effortLevel: command.effort })
              : Promise.reject(new Error("this version of the SDK cannot change the effort")),
          { type: "effort_set", effort: command.effort },
        );
        return;
```

in `run`, add `effort: command.effort ?? undefined,` after `model: command.model ?? undefined,` in the query options, and after `void this.emitSupportedCommands();` add `void this.emitSupportedModels();`; add the method after `emitSupportedCommands`:

```ts
  // A failure, a slow answer or an empty list only means no model list; it never touches the session.
  private async emitSupportedModels(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.supportedModels) return;
    try {
      const models = toWireModels(await withTimeout(query.supportedModels(), this.commandsTimeoutMs));
      if (this.shuttingDown || models.length === 0) return;
      this.emit({ type: "models_available", models });
    } catch {
      // No list from this call.
    }
  }
```

- [ ] **Step 5: Run to see them pass**

Run: `cd sidecar && npx vitest run && npx tsc --noEmit`
Expected: all tests PASS, no type errors.

- [ ] **Step 6: Commit**

```bash
git add sidecar/src sidecar/test
git commit -m "feat: the sidecar sets effort and reports the models the SDK lists

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Effort in the live session (application)

**Files:**
- Modify: `crates/forge-application/src/event.rs`, `live_session.rs`, `live_session_actor.rs`
- Modify: `crates/forge-features/src/workspace.rs` (`set_effort`), `crates/forge-features/src/sidebar.rs` (test view literal)

**Interfaces:**
- Consumes: `SidecarEvent::{EffortSet, ModelsAvailable}`, `SidecarPort::set_effort`, `StartSessionParams.effort`, `Call::SetEffort` (Task 3).
- Produces: `LiveSessionView.effort: Option<EffortLevel>`, `LiveSessionView.available_models: Vec<AvailableModel>`; `LiveSessionHandle::set_effort(Option<EffortLevel>)`; `Workspace::set_effort(id, Option<EffortLevel>)`; `SessionUpdate::{EffortChanged, ModelsAvailable}`.

- [ ] **Step 1: Write the failing event tests.** In `event.rs` tests (next to `ModelSet`):

```rust
    #[test]
    fn effort_set_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let before = session.clone();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::Low) });

        assert_eq!(session, before);
        assert_eq!(update, SessionUpdate::EffortChanged { effort: Some(forge_domain::EffortLevel::Low) });
    }

    #[test]
    fn the_model_list_is_reported() {
        let mut session = empty_session();
        let models = vec![forge_domain::AvailableModel { value: "m".to_string(), display_name: "M".to_string(), effort_levels: vec![] }];
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ModelsAvailable { models: models.clone() });

        assert_eq!(update, SessionUpdate::ModelsAvailable(models));
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-application event`
Expected: compile error.

- [ ] **Step 3: Implement the events.** In `event.rs` import `AvailableModel, EffortLevel` from `forge_domain`; add to `SessionUpdate` after `ModelChanged`:

```rust
    /// The SDK accepted an effort change (`None`: its default).
    EffortChanged { effort: Option<EffortLevel> },
    /// The models the SDK listed for this session.
    ModelsAvailable(Vec<AvailableModel>),
```

and to `apply_sidecar_event` after the `ModelSet` arm:

```rust
        SidecarEvent::EffortSet { effort } => SessionUpdate::EffortChanged { effort: *effort },
        SidecarEvent::ModelsAvailable { models } => SessionUpdate::ModelsAvailable(models.clone()),
```

Update the comment of `SettingFailed` to "a mode, model or effort change".

- [ ] **Step 4: Write the failing live-session tests.** In `live_session.rs` tests (they use `fake_sidecar`, `factory_returning`, `new_init`, `wait_for`, `Call`), add, and add `effort: None` to `start_params` there if Task 3 did not:

```rust
    #[tokio::test]
    async fn before_a_sidecar_exists_an_effort_change_applies_at_once_and_to_the_next_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.set_effort(Some(EffortLevel::High));
        wait_for(&mut view, "the change", |v| v.effort == Some(EffortLevel::High)).await;
        handle.send("go".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let started = control.calls().into_iter().find_map(|call| match call {
            Call::Start(params) => Some(params),
            _ => None,
        });
        assert_eq!(started.expect("a sidecar was started").effort, Some(EffortLevel::High));
    }

    #[tokio::test]
    async fn with_a_sidecar_the_effort_is_sent_and_shown_only_when_the_sidecar_answers() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.set_effort(Some(EffortLevel::Max));
        for _ in 0..50 {
            if control.calls().contains(&Call::SetEffort(Some(EffortLevel::Max))) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(control.calls().contains(&Call::SetEffort(Some(EffortLevel::Max))));
        assert_eq!(handle.snapshot().effort, None, "not shown before the answer");

        control.push(SidecarEvent::EffortSet { effort: Some(EffortLevel::Max) });
        let seen = wait_for(&mut view, "the answer", |v| v.effort == Some(EffortLevel::Max)).await;
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn a_refused_effort_change_keeps_the_old_level_and_the_refusal_goes_away_when_a_change_is_accepted() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::SettingFailed { message: "capped".to_string() });
        let refused = wait_for(&mut view, "the refusal", |v| v.last_error.is_some()).await;
        assert_eq!(refused.effort, None);
        assert_eq!(refused.phase, Phase::Running);

        control.push(SidecarEvent::EffortSet { effort: Some(EffortLevel::Low) });
        let seen = wait_for(&mut view, "the change", |v| v.effort == Some(EffortLevel::Low)).await;
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn the_models_the_sdk_lists_are_kept_in_the_view() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let models = vec![AvailableModel { value: "model-x".to_string(), display_name: "X".to_string(), effort_levels: vec![EffortLevel::Low, EffortLevel::High] }];
        control.push(SidecarEvent::ModelsAvailable { models: models.clone() });
        let seen = wait_for(&mut view, "the list", |v| !v.available_models.is_empty()).await;

        assert_eq!(seen.available_models, models);
    }
```

(import `forge_domain::{AvailableModel, EffortLevel}` in the test module if missing.)

- [ ] **Step 5: Run to see them fail**

Run: `cargo test -p forge-application live_session`
Expected: compile errors (`set_effort`, `effort`, `available_models`).

- [ ] **Step 6: Implement.** `live_session.rs`: import `AvailableModel, EffortLevel`; add to `LiveSessionView` after `model`:

```rust
    /// The effort of this session; `None` is the SDK's default. Changes when the SDK accepts a change
    /// (or at once, while no sidecar is running). A transcript does not record it, so a resumed
    /// session starts on `None`.
    pub effort: Option<EffortLevel>,
    /// The models the SDK listed for this session, with the effort levels each takes; empty until the
    /// sidecar has reported.
    pub available_models: Vec<AvailableModel>,
```

add `SetEffort(Option<EffortLevel>),` to `LiveCommand` after `SetModel`; add to `LiveSessionHandle` after `set_model`:

```rust
    /// Changes the effort (`None`: the default) the same way.
    pub fn set_effort(&self, effort: Option<EffortLevel>) {
        let _ = self.commands.send(LiveCommand::SetEffort(effort));
    }
```

and `effort: None, available_models: Vec::new(),` in `spawn_live_session`'s view (after `model: init.model.clone(),`), and the same two fields in the literal in `forge-features/src/sidebar.rs` (`fn view(...)` in tests).

`live_session_actor.rs`: import `EffortLevel`; add field `effort: Option<EffortLevel>,` (after `model`) initialised `effort: None,` in `new`; change the `refusal_shown` doc to "a mode, model or effort change"; add the dispatch `LiveCommand::SetEffort(effort) => self.set_effort(effort).await,` after the `SetModel` line; in `start_sidecar` replace the `effort: None,` that Task 3 left with `effort: self.effort,`; add after `set_model`:

```rust
    async fn set_effort(&mut self, effort: Option<EffortLevel>) {
        let Some(sidecar) = self.sidecar.clone() else {
            self.effort = effort;
            self.view.effort = effort;
            self.dirty = true;
            return;
        };
        if let Err(error) = sidecar.set_effort(effort).await {
            self.fail(error.to_string(), false).await;
        }
    }
```

and in `apply_event`, after the `ModelChanged` arm:

```rust
            SessionUpdate::EffortChanged { effort } => {
                self.effort = effort;
                self.view.effort = effort;
                self.clear_refusal();
            }
            SessionUpdate::ModelsAvailable(models) => self.view.available_models = models,
```

`workspace.rs`: add `EffortLevel` to the `forge_domain` import and, after `set_model`:

```rust
    /// Changes the effort of the open session (`None`: the SDK's default), from its next turn on.
    pub fn set_effort(&self, id: LiveSessionId, effort: Option<EffortLevel>) {
        if let Some(handle) = self.handle(id) {
            handle.set_effort(effort);
        }
    }
```

- [ ] **Step 7: Run to see them pass**

Run: `cargo test -p forge-application -p forge-features && cargo clippy --workspace --all-targets`
Expected: PASS, no warnings.

- [ ] **Step 8: Commit**

```bash
git add crates
git commit -m "feat: the live session follows the SDK on effort and keeps its model list

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The effort control in the composer

**Files:**
- Create: `crates/forge-features/src/effort.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-widgets/src/composer_controls.rs`, `lib.rs`, `chat_window.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (style test)

**Interfaces:**
- Consumes: `LiveSessionView.{effort, available_models, model}`, `Workspace::set_effort` (Task 5).
- Produces: `effort_levels(&[AvailableModel], Option<&str>) -> Vec<EffortLevel>`, `slider_position(&[EffortLevel], Option<EffortLevel>) -> usize`, `effort_button_label(Option<EffortLevel>) -> String`, `EFFORT_HELP`, `SLIDER_FASTER`, `SLIDER_SMARTER`; widget `EffortPicker { id }`.

- [ ] **Step 1: Write the failing tests and the signatures.** Create `crates/forge-features/src/effort.rs`:

```rust
use forge_domain::{AvailableModel, EffortLevel};

pub const EFFORT_HELP: &str = "Higher effort means a more thorough answer, but it takes longer and uses your limit faster.";
pub const SLIDER_FASTER: &str = "Faster";
pub const SLIDER_SMARTER: &str = "Smarter";

pub fn effort_levels(_models: &[AvailableModel], _model: Option<&str>) -> Vec<EffortLevel> {
    Vec::new()
}

pub fn slider_position(_levels: &[EffortLevel], _current: Option<EffortLevel>) -> usize {
    0
}

pub fn effort_button_label(_current: Option<EffortLevel>) -> String {
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use EffortLevel::*;

    fn model(value: &str, levels: &[EffortLevel]) -> AvailableModel {
        AvailableModel { value: value.to_string(), display_name: value.to_string(), effort_levels: levels.to_vec() }
    }

    #[test]
    fn the_levels_are_those_of_the_sessions_model_lowest_first() {
        let models = vec![model("a", &[High, Low]), model("b", &[Medium])];
        assert_eq!(effort_levels(&models, Some("a")), vec![Low, High]);
        assert_eq!(effort_levels(&models, Some("b")), vec![Medium]);
    }

    #[test]
    fn a_model_that_is_not_listed_or_takes_no_effort_has_no_levels() {
        let models = vec![model("a", &[]), model("b", &[Low])];
        assert!(effort_levels(&models, Some("a")).is_empty(), "takes no effort");
        assert!(effort_levels(&models, Some("zzz")).is_empty(), "not listed");
        assert!(effort_levels(&[], Some("a")).is_empty(), "nothing listed yet");
    }

    #[test]
    fn the_default_model_uses_the_first_listed_one() {
        let models = vec![model("first", &[Low, Medium]), model("second", &[Max])];
        assert_eq!(effort_levels(&models, None), vec![Low, Medium]);
    }

    #[test]
    fn the_thumb_sits_on_the_current_level_else_on_high_else_on_the_first_stop() {
        assert_eq!(slider_position(&[Low, Medium, High], Some(Medium)), 1);
        assert_eq!(slider_position(&[Low, Medium, High], None), 2, "the SDK default is high");
        assert_eq!(slider_position(&[Low, Medium], None), 0, "no high to stand on");
        assert_eq!(slider_position(&[Low, Medium], Some(Max)), 0, "a level the model lacks");
    }

    #[test]
    fn the_button_names_the_level_or_the_default() {
        assert_eq!(effort_button_label(Some(XHigh)), "Extra high");
        assert_eq!(effort_button_label(None), "Default");
    }
}
```

Add `mod effort;` (after `mod command_box;`) and `pub use effort::{effort_button_label, effort_levels, slider_position, EFFORT_HELP, SLIDER_FASTER, SLIDER_SMARTER};` to `crates/forge-features/src/lib.rs`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features effort`
Expected: FAIL (the stubs return empty values).

- [ ] **Step 3: Implement the rules.** Replace the three stubs:

```rust
/// The effort levels the session's model takes, lowest first; empty when the model is not listed or
/// takes none. A session on the SDK's default model (`None`) uses the first model the SDK listed.
pub fn effort_levels(models: &[AvailableModel], model: Option<&str>) -> Vec<EffortLevel> {
    let entry = match model {
        Some(id) => models.iter().find(|candidate| candidate.value == id),
        None => models.first(),
    };
    let mut levels = entry.map(|entry| entry.effort_levels.clone()).unwrap_or_default();
    levels.sort();
    levels.dedup();
    levels
}

/// Where the slider thumb stands: on the current level, else on High (the SDK's default), else on
/// the first stop.
pub fn slider_position(levels: &[EffortLevel], current: Option<EffortLevel>) -> usize {
    let wanted = current.unwrap_or(EffortLevel::High);
    levels.iter().position(|level| *level == wanted).unwrap_or(0)
}

/// The text of the composer's effort button.
pub fn effort_button_label(current: Option<EffortLevel>) -> String {
    current.map_or("Default", EffortLevel::label).to_string()
}
```

- [ ] **Step 4: Run to see them pass**

Run: `cargo test -p forge-features effort`
Expected: PASS (5 tests).

- [ ] **Step 5: Add the styles with a failing test first.** In `crates/forge-shared/src/theme.rs` tests add:

```rust
    #[test]
    fn the_effort_styles_exist_and_use_theme_colors() {
        for selector in [".effort-menu", ".effort-head", ".effort-ends", ".effort-slider", ".effort-help"] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
        for rule in THEME_CSS.lines().filter(|line| line.starts_with(".effort")) {
            assert!(!rule.contains('#'), "a hard-coded color in: {rule}");
            assert!(!rule.contains("rgb"), "a hard-coded color in: {rule}");
        }
    }
```

Run: `cargo test -p forge-shared effort` — Expected: FAIL (missing `.effort-menu`). Then add to `theme.css`, after the `.picker-confirm span` line:

```css
.effort-menu { min-width: 240px; padding: 10px 12px; }
.effort-head { display: flex; align-items: center; gap: 8px; margin-bottom: 8px; }
.effort-head .picker-title { padding: 0; }
.effort-current { color: var(--fg); font-weight: 600; font-size: 13px; }
.effort-help { margin-left: auto; width: 16px; height: 16px; display: inline-flex; align-items: center; justify-content: center; border: 1px solid var(--border); border-radius: 50%; color: var(--fg-muted); font-size: 11px; cursor: help; }
.effort-ends { display: flex; justify-content: space-between; margin-bottom: 4px; color: var(--fg-muted); font-size: 11px; }
.effort-slider { width: 100%; margin: 0 0 8px; accent-color: var(--accent); }
```

Run again — Expected: PASS.

- [ ] **Step 6: Write the widget.** In `crates/forge-widgets/src/composer_controls.rs` extend the `forge_features` import with `effort_button_label, effort_levels, slider_position, EFFORT_HELP, SLIDER_FASTER, SLIDER_SMARTER` and append:

```rust
/// The effort of the open session, changed from the composer with a stepped slider. Only the levels
/// the session's model takes get a stop; a model that takes none, or that the SDK has not listed,
/// shows no control.
#[component]
pub fn EffortPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (levels, current) = {
        let view = view.read();
        (effort_levels(&view.available_models, view.model.as_deref()), view.effort)
    };
    if levels.is_empty() {
        return rsx! {};
    }
    let position = slider_position(&levels, current);
    let last_stop = levels.len() - 1;
    let label = effort_button_label(current);
    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: "Effort",
                "aria-haspopup": "dialog",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                span { "{label}" }
                span { class: "picker-chevron", "▾" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        refocus_composer();
                    },
                }
                div {
                    class: "picker-menu effort-menu",
                    role: "dialog",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            refocus_composer();
                        }
                    },
                    div { class: "effort-head",
                        span { class: "picker-title", "Effort" }
                        span { class: "effort-current", "{label}" }
                        span { class: "effort-help", title: "{EFFORT_HELP}", "?" }
                    }
                    div { class: "effort-ends",
                        span { "{SLIDER_FASTER}" }
                        span { "{SLIDER_SMARTER}" }
                    }
                    input {
                        class: "effort-slider",
                        r#type: "range",
                        min: "0",
                        max: "{last_stop}",
                        step: "1",
                        value: "{position}",
                        "aria-label": "Effort",
                        oninput: {
                            let levels = levels.clone();
                            move |event: FormEvent| {
                                if let Some(level) = event.value().parse::<usize>().ok().and_then(|index| levels.get(index)) {
                                    workspace.set_effort(id, Some(*level));
                                }
                            }
                        },
                    }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        onclick: move |_| workspace.set_effort(id, None),
                        span { class: "picker-text", span { class: "picker-label", "Use the default" } }
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` change the export to `pub use composer_controls::{EffortPicker, ModePicker, ModelPicker};`. In `chat_window.rs` import `EffortPicker` with the others and add `EffortPicker { id }` right after `ModelPicker { id }`.

- [ ] **Step 7: Build, test and lint**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: PASS, no warnings. (The look and the slider are checked in the real window in Task 8.)

- [ ] **Step 8: Commit**

```bash
git add crates
git commit -m "feat: an effort slider in the composer, from the models the SDK lists

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Check the model ids against the real SDK (OTH-07)

**Files:**
- Create: `sidecar/scripts/list_models.mjs`
- Modify (as the check decides): `crates/forge-entities/src/model_catalog.rs` (`KNOWN_MODELS`, `MENU_ORDER`, tests), `crates/forge-features/src/effort.rs` (the default-model rule, if it is wrong)
- Modify: `docs/superpowers/specs/2026-10-03-fork-effort-design.md` (the "As built" section, Task 8)

**Interfaces:**
- Consumes: the SDK in `sidecar/node_modules`; the user's real Claude login.
- Produces: a table of what the SDK lists and accepts, and ids in `KNOWN_MODELS` that match it.

This task runs the real Claude Code once. It sends **no message**: the script opens a query, asks for the model list, and closes it. If the SDK refuses to list models before a first message, stop and ask the user before sending one (Plan 16 sent one `hello`).

- [ ] **Step 1: Write the script.** Create `sidecar/scripts/list_models.mjs`:

```js
// Prints the models the real SDK lists for a session: value, resolved id, name, effort levels, auto mode.
// Opens a query and closes it without sending a message.
import { query } from "@anthropic-ai/claude-agent-sdk";

const controller = new AbortController();
async function* never() {
  await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
}
const handle = query({ prompt: never(), options: { cwd: process.cwd(), abortController: controller, settingSources: [] } });
try {
  const models = await Promise.race([
    handle.supportedModels(),
    new Promise((_, reject) => setTimeout(() => reject(new Error("no answer in 60 s")), 60000)),
  ]);
  for (const m of models) {
    console.log(JSON.stringify({
      value: m.value,
      resolvedModel: m.resolvedModel ?? null,
      displayName: m.displayName,
      supportsEffort: m.supportsEffort ?? null,
      effortLevels: m.supportedEffortLevels ?? null,
      supportsAutoMode: m.supportsAutoMode ?? null,
    }));
  }
} finally {
  controller.abort();
  process.exit(0);
}
```

- [ ] **Step 2: Run it and keep the output**

Run: `cd sidecar && node scripts/list_models.mjs | tee /private/tmp/claude-501/-Users-james-orca-workspaces-forge-plan-19/a1cf7a44-dd53-40db-9d1d-b3d29c8bf75c/scratchpad/models.txt`
Expected: one JSON line per model. Read it and write down, for each of Forge's four ids (`claude-fable-5-1`, `claude-opus-5-5`, `claude-sonnet-5-5`, `claude-haiku-4-5-20251001`), whether it equals a `value`, a `resolvedModel`, or neither. Also write down which row is first (the default) and its `value`.

- [ ] **Step 3: Decide and fix.** With the table in hand:
  - An id of `KNOWN_MODELS` that is neither a `value` nor a `resolvedModel`: replace it by the `value` the SDK lists for that model (keep the label), in `KNOWN_MODELS` and `MENU_ORDER`, and change the assertions in `model_catalog.rs` tests that name the old id (`claude-sonnet-5-5` appears in `a_known_current_model_is_not_listed_twice` and in the Plan 16 notes).
  - The session's `model` is `None` (the SDK default) and the first listed row is **not** the default model: change `effort_levels` in `effort.rs` for `None` to pick the row the table shows (and update `the_default_model_uses_the_first_listed_one`).
  - Every id already matches and the first row is the default: change nothing and say so in "As built".

- [ ] **Step 4: Run the tests**

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add sidecar/scripts crates
git commit -m "fix: model ids follow what the real SDK lists

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

If nothing changed besides the script, the message is `chore: a script that lists the models the real SDK knows`.

---

### Task 8: Check in the real window, write down what was built

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-fork-effort-design.md` (add "As built")
- Modify: `docs/superpowers/roadmap/backlog.md`, `docs/superpowers/roadmap/README.md`
- Create: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-19-fork-effort.md` and add its line to `MEMORY.md`

**Interfaces:** none (verification and records).

- [ ] **Step 1: Full test run**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --all --check && (cd sidecar && npx vitest run && npx tsc --noEmit)`
Expected: all PASS. Fix any failure before the harness steps.

- [ ] **Step 2: Start the harness** (fake Claude home; the real `~/.claude` is not read)

Run: `scripts/harness/launch.sh` then `python3 scripts/harness/capture.py` and Read `/private/tmp/forge-harness-shot.png`. The viewer prints a scale factor: multiply the coordinates by it before `click.py`.
Expected: the Forge window with the fixture projects.

- [ ] **Step 3: Fork from the menu and with F.** Open a session row's `⋯` menu (click, capture): "Rename", "Fork" (key F), "Delete" are listed. Click "Fork" (capture, a second capture after one second): a row "<title> (fork)" appears in the same project and is opened; the original row is still there. Open the menu again and press F with `osascript -e 'tell application "System Events" to key code 3'` (the F key) with the Korean input method on: a second fork appears ("<title> (fork) (fork)" if the source was the first fork). Write down what was seen, including whether the composer of the opened fork has the cursor (the desktop app focuses it; this plan does not add code for it, so if it does not, record that as a gap in "As built").
Expected: as above. A failure shows a notice instead.

- [ ] **Step 4: Effort, no sidecar.** Open a session, open the effort button. Expected: hidden until the sidecar has listed the models (a fixture session has no sidecar until the first send).

- [ ] **Step 5: Effort with the real SDK.** Ask the user before this step: it sends one real message ("hello") on the real Claude and leaves a transcript in the real `~/.claude/projects/-private-tmp-forge-harness-my-app/` to delete by hand afterwards (see `scripts/harness/README.md`). After the answer: open the effort button (the levels the model takes, thumb on High or the current level), move the slider with the arrow keys (the label changes when the SDK accepts), choose "Use the default", and, on a model that takes no effort (Haiku 4.5 if the list says so), see that the button is gone. Write down every SDK answer (accepted or refused text), the stops seen, and whether the default model's row is the one the effort rule picked.
Expected: the view follows the SDK's answer; a refusal shows as a banner and keeps the old level.

- [ ] **Step 6: Stop the harness and clean up**

Run: `scripts/harness/stop.sh`, then delete the transcript the harness left in `~/.claude/projects/-private-tmp-forge-harness-my-app/` (list it first, delete only the files from this run).

- [ ] **Step 7: Write "As built"** at the end of the spec: what was built as designed, what was seen in steps 3 to 5 (a table of the SDK's answers like Plan 16's), the result of the OTH-07 check (the table from Task 7), anything left out or not tried.

- [ ] **Step 8: Update the roadmap.** In `backlog.md` set `Plan` to `Plan 19 (built)` for SES-01, CMP-05 and OTH-07 (SES-01 notes: the key F and the menu item; CMP-05 notes: levels come from `supportedModels()`, no "추천"), and in the SES-06 row change "F and A wait for SES-01 and SES-02" to "F built in Plan 19; A waits for SES-02". In `README.md` change the Plan 19 row of the tentative table to `| 19 | Fork and effort (built) | SES-01, CMP-05, OTH-07 | Small items the user chose first |` and renumber the terminal panel to the next free plan row, keeping the order of the rest.

- [ ] **Step 9: Write the memory file** `plan-19-fork-effort.md` (type `project`): the goal, what is built, what is left (SES-02 archive, effort stop names, "추천"), the lessons from this plan (fork as a repository operation; the desktop app logs out when the session dialog is dismissed; the observation script and which choices were made on the real account), then the one-line pointer in `MEMORY.md`.

- [ ] **Step 10: Commit**

```bash
git add docs
git commit -m "docs: Plan 19 as built, backlog and roadmap updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
