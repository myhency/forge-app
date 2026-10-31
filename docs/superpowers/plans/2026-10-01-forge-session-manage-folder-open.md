# Forge Session Rename/Delete and Folder Open Implementation Plan (Plan 9)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session can be renamed and deleted (to the OS trash) from the sidebar, and the new-session panel has an "Open folder…" button backed by a native folder dialog. This finishes Tier 1.

**Architecture:** `SessionRepositoryPort` gains `rename_session` and `delete_session`; `forge-adapter-claude-fs` implements them (append a `custom-title` line; move the `.jsonl` and its sibling directory to the trash behind a thin `TrashBin` wrapper). `SessionRegistry` (it already knows which sessions are open) validates the title, refuses to delete an open session and pushes a new title into an open session's view. A new `FolderPickerPort` with an `rfd` adapter in a new crate `forge-adapter-dialog` opens the folder dialog. `Workspace` (forge-features) gets `rename_session`, `delete_session` and `open_folder`, reloading only the affected project. The sidebar gets a `SessionRow` widget with a `⋯` menu, an in-place rename input and an inline delete confirmation.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, `trash` 5, `rfd` 0.17, existing `tokio`, `serde_json`, `mockall`.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §19 (§7 for the Tier 1 list, §17.5 for overlay-aware shortcuts, §15.12 for the `cwd` collision that stays out of scope).

## Global Constraints

- Crate graph: `forge-adapter-dialog` depends on `forge-ports` only. Only `forge-app` names concrete adapters (the composition root). No UI crate imports an adapter.
- Rename appends `{"type":"custom-title","customTitle":<title>,"sessionId":<id>}` as one line; if the file does not end with a newline, one is written first (spec §19.2).
- Delete moves the `.jsonl` to the OS trash through the `trash` crate behind a thin wrapper, so tests never touch a real trash (spec §19.2).
- Title validation: trimmed, non-empty, at most 200 characters (spec §19.4). Counted in characters, not bytes.
- A session that is open in Forge cannot be deleted; it can be renamed, and an open session's title updates in the live view (spec §19.1, §19.4).
- After either action only that project's session list is reloaded (spec §19.4).
- UI text is English. Delete confirmation reads "Moves the session to the trash" (spec §19.5).
- Out of scope: trash restore UI, bulk delete, deleting a project, undo for rename, file watching, the §15.12 `cwd` collision (spec §19.1).
- Colors only through CSS variables from `forge-shared/src/theme.css`; nothing hard-coded outside `:root` and the dark `@media` block (spec §16.6).
- Pinned: Dioxus `=0.7.10`. New dependencies are added exactly as written in the tasks.
- Every commit message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (pass it as a second `-m`).
- Test commands: `cargo test -p <crate>`; full check `cargo test --workspace` (run it once in Task 1 and note the count; the `block v0.1.6` future-incompat notice is expected). Baseline before this plan: 341 tests.

## Decisions made in this plan (flag them if you disagree)

1. **"Open" means open in Forge, not "sidecar currently running".** Spec §19.1 says a session whose sidecar is running cannot be deleted. An open session with no sidecar yet (opened from the sidebar, nothing sent) would still be shown and could spawn a sidecar on its next send; deleting its file under it leaves a view that resumes a session that no longer exists. So delete is refused for every session open in Forge, with the message "Close the session first (⌘W) to delete it." Rename stays allowed for open sessions.
2. **A `custom-title` always beats an `ai-title`**, wherever each sits in the file (spec §19.2 says the reader takes the last title entry). Otherwise Claude Code appending an automatic title after a rename would silently undo it. Among titles of the same kind the last one wins, as before. Task 1 confirms against what Claude Code writes.
3. **The folder dialog result that is not in the list is added to the folder dropdown** for this run of the app (a path nobody has used with Claude Code has no history yet). It is kept in memory only.
4. **Delete confirmation is inline** in the sidebar row, not a modal, so it needs no overlay handling for the §17.5 shortcuts.
5. **Renaming bumps the session to the top of its project's list** (the file's modified time changes). Not compensated for; noted in Review Focus.

## Review Focus

Inputs and conditions the spec implies but that are easy to get wrong, most likely first. Each has a test in the task named.

1. A rename title with surrounding whitespace, only whitespace, 201 characters, or Korean text of 200 characters (600 bytes) → trimmed; empty and 201 rejected; the 200-character Korean title accepted (Task 4).
2. A title containing quotes, backslashes, a newline or a tab → the appended entry is still exactly one valid JSON line and reads back identical (Task 2).
3. A session file with no trailing newline, and an empty file → the new entry starts on its own line and the earlier last line still parses (Task 2).
4. Rename or delete of a session id that does not exist, or one containing `/`, `\` or `..` → `NotFound`, nothing is created or moved outside the project directory (Tasks 2, 3).
5. Delete of an open session → refused, file untouched; delete when the trash move fails → the error is shown and the session stays listed (Tasks 3, 4, 6).
6. Rename of an open session → the live view and sidebar show the new title at once, and a later `ai-title` line in the file does not undo it (Tasks 2, 4).
7. Renaming while an IME composition is in progress (Korean): Enter that confirms the composition must not submit the rename (Task 7).
8. The folder dialog cancelled → nothing changes. A picked path that is a file or no longer exists → rejected with a message. A picked path already in the list → that project is selected, not duplicated. Two clicks on "Open folder…" → one dialog (Tasks 6, 8).

---

## File Structure

- `crates/forge-ports/src/session_port.rs` — two new methods, two new error variants (modify)
- `crates/forge-ports/src/folder_picker_port.rs` — `FolderPickerPort` (create); `lib.rs` exports it
- `crates/forge-adapter-claude-fs/src/sessions.rs` — rename, delete, custom-title-wins reader, `TrashBin` use (modify)
- `crates/forge-adapter-claude-fs/src/trash.rs` — `TrashBin` trait and `SystemTrash` (create)
- `crates/forge-adapter-dialog/` — new crate: `RfdFolderPicker` (create)
- `crates/forge-application/src/{live_session.rs, live_session_actor.rs, registry.rs, lib.rs}` — `SetTitle`, `rename`, `delete`, `normalize_title`, `ManageSessionError` (modify)
- `crates/forge-features/src/{workspace.rs, folder_choice.rs (create), sidebar.rs, lib.rs}` — actions, reload scope, folder choice (modify/create)
- `crates/forge-entities/src/sidebar_item.rs` — a `children` slot (modify)
- `crates/forge-widgets/src/{session_row.rs (create), sidebar.rs, new_session_panel.rs?}`; the Open-folder button lives in `crates/forge-features/src/new_session.rs` (modify)
- `crates/forge-shared/src/theme.css` — row menu, rename input, confirm row (modify)
- `crates/forge-app/{Cargo.toml, src/main.rs}` — wiring (modify)
- `docs/superpowers/specs/2026-09-27-forge-design.md` — §19.7 spike findings, §19.9 as built (modify)

---

### Task 1: Spike

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§19.7)

**Interfaces:**
- Consumes: nothing.
- Produces: written answers to (a)–(f) in §19.7. Tasks 2, 3 and 5 rely on (a), (b), (c), (e); Task 8 on (d); the rest of the plan stands if they confirm the decisions above.

The pieces below are individually simple; the risk is the files Claude Code really writes and a dialog inside a Dioxus window. Only running the real thing shows it. Do not assume — record what you saw.

- [ ] **Step 1: Baseline**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4} END {print s}'`
Expected: a total; note it as the baseline in §19.7 (the plan assumed 341).

- [ ] **Step 2: (a) What Claude Code writes when a session is renamed**

In a scratch project (any temp directory, not this repo), start `claude`, send one message, run `/rename spike-title`, exit. Then:

```bash
F=$(ls -t ~/.claude/projects/*<scratch-dir-escaped>*/*.jsonl | head -1)
grep -n '"custom-title"\|"ai-title"' "$F" | cut -c1-300
tail -c 200 "$F" | od -c | tail -3
```

Record: the exact field names of the rename line, whether `sessionId` is present, whether `ai-title` lines appear after the `custom-title` line when you send one more message and exit, and whether the file ended with a newline before the rename. If `/rename` writes a different entry type than `custom-title`, stop and report: Tasks 2 and 3 change.

- [ ] **Step 3: (b)/(c) Siblings of a session's `.jsonl`**

```bash
cd ~/.claude/projects/<some project dir>
ls -d */ | head
ls <session-id>/ 2>/dev/null
```

Record what sits in a `<session-id>/` directory beside a `<session-id>.jsonl` (the plan assumes sub-agent transcripts and tool results) and whether anything else carries the session id (`~/.claude/todos`, `file-history`, `shell-snapshots`, `session-env`). Decide, and write it down: the plan trashes the `.jsonl` and the same-named sibling directory, nothing else. If the sibling directory holds nothing that belongs only to that session, remove the sibling lines from Task 3.

- [ ] **Step 4: (d) A folder dialog from inside the Dioxus window**

Add `rfd = "0.17"` temporarily to `crates/forge-app/Cargo.toml` and, in `NewSessionComposer` (crates/forge-features/src/new_session.rs), a button whose handler runs:

```rust
spawn(async move {
    let picked = rfd::AsyncFileDialog::new().pick_folder().await;
    eprintln!("picked: {:?}", picked.map(|handle| handle.path().to_path_buf()));
});
```

Run `cargo run -p forge-app`, open the new-session panel, click the button, and check: the dialog opens; the window stays responsive while it is open (type in the message box, resize); the chosen path is printed; cancelling prints `None`; the app does not hang or crash on macOS. Record whether the default features of `rfd` were enough. Then `git checkout -- crates/forge-app/Cargo.toml crates/forge-features/src/new_session.rs Cargo.lock`.

- [ ] **Step 5: (e) Where `trash` puts a file**

In a throwaway `cargo new --bin trashspike` inside the scratchpad, add `trash = "5"` and call `trash::delete(path)` on a temp file; record that it lands in `~/.Trash`, that `trash::delete` blocks (it is synchronous), and what error it returns for a path that does not exist.

- [ ] **Step 6: (f) Does a rename change the file's modified time?**

Yes by construction (an append). Confirm with `stat -f %m` before and after one `echo '{}' >> file` on a scratch copy, and record that the list order changes accordingly. (This is Decision 5.)

- [ ] **Step 7: Write the findings into §19.7 and commit**

Replace `(To be filled in by task 1.)` with the baseline and answers (a)–(f), each marked "Checked" or "Not checked", the way §15.10 does. If an answer contradicts a Decision above, say so at the top of §19.7 and stop for the controller before Task 2.

```bash
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record the Plan 9 spike findings" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Rename in the port and the adapter

**Files:**
- Modify: `crates/forge-ports/src/session_port.rs`
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs`
- Modify: `crates/forge-application/src/registry.rs` (the test-only `SlowRepository` must implement the new method)

**Interfaces:**
- Consumes: `SessionRepositoryPort` as it is today.
- Produces:
  - `SessionRepositoryError::RenameFailed(String)` (message `failed to rename session: {0}`).
  - `async fn rename_session(&self, project_path: &Path, session_id: &SessionId, title: &str) -> Result<(), SessionRepositoryError>` on `SessionRepositoryPort` (and on `MockSessionRepositoryPort`).
  - The adapter reads `custom-title` over `ai-title` (Decision 2).

- [ ] **Step 1: Write the failing port test**

In `session_port.rs` tests, add:

```rust
    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_rename_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_rename_session()
            .withf(|p: &Path, id: &SessionId, title: &str| {
                p == Path::new("/tmp/project") && id.0 == "s-1" && title == "New title"
            })
            .times(1)
            .returning(|_, _, _| Ok(()));

        let result = mock
            .rename_session(&PathBuf::from("/tmp/project"), &SessionId("s-1".to_string()), "New title")
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn rename_failed_message_names_the_cause() {
        let error = SessionRepositoryError::RenameFailed("disk full".to_string());
        assert_eq!(error.to_string(), "failed to rename session: disk full");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-ports session_port`
Expected: FAIL to compile (`no method named rename_session`, `no variant RenameFailed`).

- [ ] **Step 3: Add the variant and the method**

In `SessionRepositoryError` add:

```rust
    #[error("failed to rename session: {0}")]
    RenameFailed(String),
```

In the trait add:

```rust
    /// Gives the session a title of the user's choosing. The title is stored in the session's own
    /// transcript, so Claude Code shows it too.
    async fn rename_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<(), SessionRepositoryError>;
```

- [ ] **Step 4: Satisfy the other implementor**

In `crates/forge-application/src/registry.rs`, inside `impl SessionRepositoryPort for SlowRepository` add:

```rust
        async fn rename_session(&self, _project_path: &Path, _session_id: &SessionId, _title: &str) -> Result<(), SessionRepositoryError> {
            Ok(())
        }
```

- [ ] **Step 5: Write the failing adapter tests**

In `crates/forge-adapter-claude-fs/src/sessions.rs` tests (they already have `write_session`, `project`, `user_text`, `ai_title`), add:

```rust
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
```

- [ ] **Step 6: Run to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: FAIL to compile (`rename_session` not implemented for `ClaudeSessionAdapter`).

- [ ] **Step 7: Implement the reader change**

Replace `title_from_value`, `title_from_line` and `read_title` with:

```rust
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
```

In `apply_line`, delete the `Some("ai-title") | Some("custom-title")` arm (titles are handled in the loader). In `load_session`, add `let mut titles = TitleTracker::default();` beside `let mut session`, and replace

```rust
                Some(value) => apply_line(&mut session, &value),
```

with

```rust
                Some(value) => {
                    if let Some((kind, found)) = title_from_value(&value) {
                        titles.see(kind, found);
                    }
                    apply_line(&mut session, &value)
                }
```

and just before `Ok(session)` add `session.title = titles.title();`.

Note: `apply_line` skips sidechain/meta lines; a title line is not one, and the old code did not filter titles by those flags either — but the new code now sees title lines even when flagged. Title lines never carry those flags in the files inspected in Task 1; if Task 1 found otherwise, apply the same `flagged` check before `titles.see`.

- [ ] **Step 8: Implement `rename_session`**

Add to the imports `use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};` and `use std::io::SeekFrom;`. Add the helpers above `impl SessionRepositoryPort`:

```rust
// A session id names a file in the project directory; anything that could point elsewhere is not one.
fn valid_session_id(id: &str) -> bool {
    !(id.is_empty() || id.contains(['/', '\\']) || id.contains(".."))
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
```

In `load_session` replace the inline id check with `if !valid_session_id(id) { return Err(SessionRepositoryError::NotFound(session_id.clone())); }`. Then add inside the `impl SessionRepositoryPort for ClaudeSessionAdapter` block:

```rust
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
        let path = self.project_dir(project_path).await.join(format!("{id}.jsonl"));
        // Append mode, and no `create`: a rename must never conjure a transcript.
        let mut file = match tokio::fs::OpenOptions::new().read(true).append(true).open(&path).await {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(SessionRepositoryError::NotFound(session_id.clone()))
            }
            Err(e) => return Err(SessionRepositoryError::RenameFailed(format!("{}: {e}", path.display()))),
        };
        let failed = |e: std::io::Error| SessionRepositoryError::RenameFailed(format!("{}: {e}", path.display()));

        let mut entry = String::new();
        if lacks_trailing_newline(&mut file).await.map_err(failed)? {
            entry.push('\n');
        }
        entry.push_str(
            &serde_json::json!({"type": "custom-title", "customTitle": title, "sessionId": id}).to_string(),
        );
        entry.push('\n');
        // One write, so a transcript being appended to by Claude Code at the same moment gets the
        // whole line or none of it interleaved.
        file.write_all(entry.as_bytes()).await.map_err(failed)?;
        file.flush().await.map_err(failed)
    }
```

(`failed` is a closure borrowing `path`; if the borrow checker objects to using it twice, make it a small `fn rename_error(path: &Path, e: std::io::Error) -> SessionRepositoryError` instead.)

- [ ] **Step 9: Run the tests**

Run: `cargo test -p forge-ports -p forge-adapter-claude-fs -p forge-application`
Expected: PASS (all new and existing).

- [ ] **Step 10: Commit**

```bash
git add crates/forge-ports crates/forge-adapter-claude-fs crates/forge-application
git commit -m "feat: rename sessions by appending a custom-title line, and let it win over ai-title" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Delete in the port and the adapter, behind a trash wrapper

**Files:**
- Modify: `crates/forge-ports/src/session_port.rs`
- Create: `crates/forge-adapter-claude-fs/src/trash.rs`
- Modify: `crates/forge-adapter-claude-fs/src/{lib.rs, sessions.rs, Cargo.toml}`
- Modify: `crates/forge-application/src/registry.rs` (`SlowRepository`)

**Interfaces:**
- Consumes: `valid_session_id` from Task 2; `ClaudeSessionAdapter::project_dir`.
- Produces:
  - `SessionRepositoryError::DeleteFailed(String)` (message `failed to delete session: {0}`).
  - `async fn delete_session(&self, project_path: &Path, session_id: &SessionId) -> Result<(), SessionRepositoryError>` on the port and its mock.
  - `pub trait TrashBin: Send + Sync { fn trash(&self, path: &Path) -> Result<(), String>; }` and `pub struct SystemTrash` (real, uses the `trash` crate) in `trash.rs`, re-exported from the crate root.
  - `ClaudeSessionAdapter::with_trash(claude_home: PathBuf, trash: Arc<dyn TrashBin>) -> Self`; `new` uses `SystemTrash`.

- [ ] **Step 1: Write the failing port test**

In `session_port.rs` tests:

```rust
    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_delete_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_delete_session().times(1).returning(|_, _| Ok(()));

        let result = mock
            .delete_session(&PathBuf::from("/tmp/project"), &SessionId("s-1".to_string()))
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn delete_failed_message_names_the_cause() {
        let error = SessionRepositoryError::DeleteFailed("no trash".to_string());
        assert_eq!(error.to_string(), "failed to delete session: no trash");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-ports session_port`
Expected: FAIL to compile.

- [ ] **Step 3: Add the variant and the method**

```rust
    #[error("failed to delete session: {0}")]
    DeleteFailed(String),
```

```rust
    /// Moves the session's transcript (and the data kept beside it) to the OS trash. Nothing is
    /// removed permanently.
    async fn delete_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<(), SessionRepositoryError>;
```

Add the same stub as Task 2 Step 4 for `delete_session` to `SlowRepository` (`Ok(())`).

- [ ] **Step 4: Write the failing adapter tests**

In `crates/forge-adapter-claude-fs/src/sessions.rs` tests add a fake and tests:

```rust
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
```

- [ ] **Step 5: Run to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: FAIL to compile (`with_trash`, `trash` module missing).

- [ ] **Step 6: Implement the wrapper**

`crates/forge-adapter-claude-fs/Cargo.toml`, under `[dependencies]` add `trash = "5"`.

Create `crates/forge-adapter-claude-fs/src/trash.rs`:

```rust
use std::path::Path;

/// Moves a path to the operating system's trash. A trait so tests never touch a real trash.
pub trait TrashBin: Send + Sync {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// The real trash (Finder's Trash, the Windows Recycle Bin, the freedesktop trash).
pub struct SystemTrash;

impl TrashBin for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        ::trash::delete(path).map_err(|error| error.to_string())
    }
}
```

In `lib.rs` add `mod trash;` and `pub use trash::{SystemTrash, TrashBin};`.

- [ ] **Step 7: Implement `delete_session`**

In `sessions.rs` change the struct and constructors:

```rust
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
```

(import `std::sync::Arc` and `crate::trash::{SystemTrash, TrashBin}`). Add to the trait impl:

```rust
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
                trash.trash(&sibling).map_err(|e| format!("{}: {e}", sibling.display()))?;
            }
            Ok::<(), String>(())
        })
        .await
        .map_err(|e| SessionRepositoryError::DeleteFailed(e.to_string()))?;
        outcome.map_err(SessionRepositoryError::DeleteFailed)
    }
```

Trashing is synchronous and can be slow, hence `spawn_blocking`. Add `"rt"` to the adapter's non-dev `tokio` features: `tokio = { version = "1", features = ["fs", "io-util", "rt"] }`.

- [ ] **Step 8: Run the tests**

Run: `cargo test -p forge-ports -p forge-adapter-claude-fs -p forge-application`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add Cargo.lock crates
git commit -m "feat: delete sessions by moving them to the trash behind a TrashBin wrapper" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Application logic — title validation, live title, the registry guards

**Files:**
- Modify: `crates/forge-application/src/live_session.rs` (`LiveCommand::SetTitle`, `LiveSessionHandle::set_title`)
- Modify: `crates/forge-application/src/live_session_actor.rs` (handle the command)
- Modify: `crates/forge-application/src/registry.rs` (`rename`, `delete`, `ManageSessionError`, `normalize_title`)
- Modify: `crates/forge-application/src/lib.rs` (exports)

**Interfaces:**
- Consumes: `SessionRepositoryPort::{rename_session, delete_session}` (Tasks 2, 3); `SessionRegistry::find_open` (private, existing).
- Produces:
  - `pub fn normalize_title(raw: &str) -> Result<String, ManageSessionError>`.
  - `pub enum ManageSessionError { EmptyTitle, TitleTooLong { max: usize }, SessionOpen, Repository(SessionRepositoryError) }` with `Display` messages: `the title cannot be empty`, `the title can be at most {max} characters`, `Close the session first (⌘W) to delete it.`, and the repository error's message.
  - `SessionRegistry::rename(&self, project_path: &Path, session_id: &SessionId, title: &str) -> Result<String, ManageSessionError>` (returns the stored, trimmed title).
  - `SessionRegistry::delete(&self, project_path: &Path, session_id: &SessionId) -> Result<(), ManageSessionError>`.
  - `LiveSessionHandle::set_title(&self, title: String)`.
  - Exported from the crate root: `ManageSessionError`, `normalize_title`.

- [ ] **Step 1: Write the failing tests**

In `registry.rs` tests (they already import `MockSessionRepositoryPort`, `registry`, `project`, `repository_returning`, `wait_for`), add:

```rust
    #[test]
    fn titles_are_trimmed_and_must_not_be_empty() {
        assert_eq!(normalize_title("  Fix the build \n").unwrap(), "Fix the build");
        assert!(matches!(normalize_title(""), Err(ManageSessionError::EmptyTitle)));
        assert!(matches!(normalize_title("  \t\n "), Err(ManageSessionError::EmptyTitle)));
    }

    #[test]
    fn a_title_may_be_200_characters_but_not_201() {
        let at_limit = "a".repeat(200);
        assert_eq!(normalize_title(&at_limit).unwrap(), at_limit);
        assert!(matches!(
            normalize_title(&"a".repeat(201)),
            Err(ManageSessionError::TitleTooLong { max: 200 })
        ));
    }

    #[test]
    fn the_title_limit_counts_characters_not_bytes() {
        let korean = "한".repeat(200); // 600 bytes
        assert_eq!(normalize_title(&korean).unwrap(), korean);
        assert!(normalize_title(&"한".repeat(201)).is_err());
    }

    #[test]
    fn the_limit_applies_after_trimming() {
        let padded = format!("  {}  ", "a".repeat(200));
        assert_eq!(normalize_title(&padded).unwrap().chars().count(), 200);
    }

    #[tokio::test]
    async fn rename_stores_the_trimmed_title_and_returns_it() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_rename_session()
            .withf(|p: &Path, id: &SessionId, title: &str| p == Path::new("/work/a") && id.0 == "claude-1" && title == "Mine")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let stored = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "  Mine ").await.unwrap();

        assert_eq!(stored, "Mine");
    }

    #[tokio::test]
    async fn rename_with_an_invalid_title_never_reaches_the_repository() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_rename_session().times(0);
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "   ").await;

        assert!(matches!(result, Err(ManageSessionError::EmptyTitle)));
    }

    #[tokio::test]
    async fn rename_of_an_open_session_updates_its_live_title() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_rename_session().times(1).returning(|_, _, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let mut view = outcome.handle.subscribe();

        registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Renamed").await.unwrap();

        let updated = wait_for(&mut view, "the new title", |v| v.session.title.as_deref() == Some("Renamed")).await;
        assert_eq!(updated.session.title.as_deref(), Some("Renamed"));
    }

    #[tokio::test]
    async fn a_failed_rename_leaves_the_open_sessions_title_alone() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository
            .expect_rename_session()
            .times(1)
            .returning(|_, _, _| Err(SessionRepositoryError::RenameFailed("read-only".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        let result = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Renamed").await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::RenameFailed(_)))));
        tokio::task::yield_now().await;
        assert_eq!(outcome.handle.snapshot().session.title.as_deref(), Some("A title"));
    }

    #[tokio::test]
    async fn delete_goes_to_the_repository_for_a_session_that_is_not_open() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_delete_session().times(1).returning(|_, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-9".to_string())).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn delete_of_an_open_session_is_refused_and_touches_nothing() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_delete_session().times(0);
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        registry.open_existing(&project("/work/a", true), SessionId("claude-1".to_string())).await.unwrap();

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-1".to_string())).await;

        assert!(matches!(result, Err(ManageSessionError::SessionOpen)));
        assert_eq!(
            result.unwrap_err().to_string(),
            "Close the session first (⌘W) to delete it."
        );
    }

    #[tokio::test]
    async fn delete_works_again_once_the_session_is_closed() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_delete_session().times(1).returning(|_, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        registry.close(outcome.handle.id());

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-1".to_string())).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn a_repository_failure_on_delete_is_reported() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_delete_session()
            .times(1)
            .returning(|_, _| Err(SessionRepositoryError::DeleteFailed("no trash".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-9".to_string())).await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::DeleteFailed(_)))));
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-application registry`
Expected: FAIL to compile (`normalize_title`, `ManageSessionError`, `rename`, `delete` do not exist).

- [ ] **Step 3: Add `SetTitle` to the live session**

In `live_session.rs`: add a variant `SetTitle(String)` to `LiveCommand` and a method to `LiveSessionHandle`:

```rust
    /// Shows `title` as the session's title. Called after the new title is stored, so the view
    /// never claims a name the transcript does not have.
    pub fn set_title(&self, title: String) {
        let _ = self.commands.send(LiveCommand::SetTitle(title));
    }
```

In `live_session_actor.rs` `handle_command`, add:

```rust
            LiveCommand::SetTitle(title) => {
                self.view.session.title = Some(title);
                self.dirty = true;
            }
```

- [ ] **Step 4: Implement the registry methods**

In `registry.rs`, add the imports `std::path::Path` and `forge_domain::SessionId` (already there), then above `SessionRegistry`:

```rust
const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Error)]
pub enum ManageSessionError {
    #[error("the title cannot be empty")]
    EmptyTitle,
    #[error("the title can be at most {max} characters")]
    TitleTooLong { max: usize },
    #[error("Close the session first (⌘W) to delete it.")]
    SessionOpen,
    #[error("{0}")]
    Repository(#[from] SessionRepositoryError),
}

/// A session title as it will be stored: trimmed, not empty, at most 200 characters.
pub fn normalize_title(raw: &str) -> Result<String, ManageSessionError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ManageSessionError::EmptyTitle);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ManageSessionError::TitleTooLong { max: MAX_TITLE_CHARS });
    }
    Ok(title.to_string())
}
```

and in `impl SessionRegistry`:

```rust
    /// Renames a past or open session. The title is stored first; an open session's view is
    /// updated only once that succeeded. Returns the title as stored.
    pub async fn rename(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<String, ManageSessionError> {
        let title = normalize_title(title)?;
        self.sessions.rename_session(project_path, session_id, &title).await?;
        if let Some(handle) = self.find_open(session_id) {
            handle.set_title(title.clone());
        }
        Ok(title)
    }

    /// Moves a session that is not open to the trash. An open session is refused: its view would
    /// go on showing (and could resume) a transcript that no longer exists.
    pub async fn delete(&self, project_path: &Path, session_id: &SessionId) -> Result<(), ManageSessionError> {
        if self.find_open(session_id).is_some() {
            return Err(ManageSessionError::SessionOpen);
        }
        self.sessions.delete_session(project_path, session_id).await?;
        Ok(())
    }
```

In `lib.rs` change the registry export to `pub use registry::{normalize_title, ManageSessionError, OpenError, OpenOutcome, SessionRegistry};`.

- [ ] **Step 5: Run the tests**

Run: `cargo test -p forge-application`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-application
git commit -m "feat: rename and delete sessions through the registry, refusing to delete an open one" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: `FolderPickerPort` and its `rfd` adapter

**Files:**
- Create: `crates/forge-ports/src/folder_picker_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`
- Create: `crates/forge-adapter-dialog/Cargo.toml`, `crates/forge-adapter-dialog/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `#[async_trait] pub trait FolderPickerPort: Send + Sync { async fn pick_folder(&self) -> Option<PathBuf>; }` and `MockFolderPickerPort` (under the `mocks` feature / tests), exported from `forge_ports`.
  - `forge_adapter_dialog::RfdFolderPicker` (unit struct, `RfdFolderPicker::new()`), implementing the port. `None` means the dialog was cancelled.

- [ ] **Step 1: Write the failing port test**

Create `crates/forge-ports/src/folder_picker_port.rs`:

```rust
use std::path::PathBuf;

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Asks the person to choose a folder. `None` means they cancelled.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait FolderPickerPort: Send + Sync {
    async fn pick_folder(&self) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_folder_picker_can_return_a_chosen_folder_or_a_cancel() {
        let mut mock = MockFolderPickerPort::new();
        let mut answers = vec![None, Some(PathBuf::from("/work/a"))];
        mock.expect_pick_folder().times(2).returning(move || answers.pop().unwrap());

        assert_eq!(mock.pick_folder().await, Some(PathBuf::from("/work/a")));
        assert_eq!(mock.pick_folder().await, None);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-ports folder_picker`
Expected: FAIL (module not declared, so zero tests run — confirm with `cargo test -p forge-ports 2>&1 | grep folder_picker` printing nothing).

- [ ] **Step 3: Export it**

In `forge-ports/src/lib.rs` add `mod folder_picker_port;`, `pub use folder_picker_port::FolderPickerPort;` and, in the mocks block, `#[cfg(any(test, feature = "mocks"))] pub use folder_picker_port::MockFolderPickerPort;`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-ports`
Expected: PASS.

- [ ] **Step 5: Create the adapter crate**

`crates/forge-adapter-dialog/Cargo.toml`:

```toml
[package]
name = "forge-adapter-dialog"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
rfd = "0.17"
```

`crates/forge-adapter-dialog/src/lib.rs`:

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use forge_ports::FolderPickerPort;

/// The operating system's folder dialog, through `rfd`. The dialog itself cannot be tested
/// without a person; spec §19.6 leaves it to the manual checklist.
#[derive(Default)]
pub struct RfdFolderPicker;

impl RfdFolderPicker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FolderPickerPort for RfdFolderPicker {
    async fn pick_folder(&self) -> Option<PathBuf> {
        rfd::AsyncFileDialog::new()
            .set_title("Open folder")
            .pick_folder()
            .await
            .map(|handle| handle.path().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_is_usable_as_a_port_object() {
        let _picker: std::sync::Arc<dyn FolderPickerPort> = std::sync::Arc::new(RfdFolderPicker::new());
    }
}
```

If Task 1 (d) found that the default `rfd` features were not enough, use the features it recorded instead of the bare version.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-adapter-dialog`
Expected: PASS (1 test). `cargo build -p forge-adapter-dialog` prints no new warnings other than the known `block v0.1.6` notice.

- [ ] **Step 7: Commit**

```bash
git add Cargo.lock crates/forge-ports crates/forge-adapter-dialog
git commit -m "feat: add FolderPickerPort and an rfd adapter" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Workspace logic — rename, delete, open folder, reload one project

**Files:**
- Create: `crates/forge-features/src/folder_choice.rs`
- Modify: `crates/forge-features/src/{workspace.rs, sidebar.rs, lib.rs}`
- Modify: `crates/forge-app/src/main.rs` (wiring; `forge-app/Cargo.toml` gets `forge-adapter-dialog` and `forge-ports`)

**Interfaces:**
- Consumes: `SessionRegistry::{rename, delete}`, `ManageSessionError` (Task 4); `FolderPickerPort`, `RfdFolderPicker` (Task 5); `SessionRepositoryPort::list_sessions`.
- Produces:
  - `pub enum FolderChoice { Known(PathBuf), Added(PathBuf), Rejected(String) }` and `pub fn choose_folder(known: &[PathBuf], picked: PathBuf, is_dir: bool) -> FolderChoice` (pure).
  - `pub fn replace_project_sessions(past: &mut Vec<(ProjectSummary, Vec<SessionSummary>)>, project_path: &Path, sessions: Vec<SessionSummary>)` (pure; also refreshes `session_count`).
  - `SidebarEntry::actions(&self) -> RowActions` with `pub struct RowActions { pub can_rename: bool, pub delete: DeleteAction }` and `pub enum DeleteAction { Allowed, WhileOpen, Unavailable }`.
  - `WorkspaceServices` gains `pub folder_picker: Arc<dyn FolderPickerPort>`.
  - `Workspace::rename_session(&self, project: PathBuf, session: SessionId, title: String)`, `Workspace::delete_session(&self, project: PathBuf, session: SessionId)`, `Workspace::open_folder(&self)`, `Workspace::picking_folder(&self) -> bool`; `new_session_projects()` includes folders added this way.

- [ ] **Step 1: Write the failing tests for the pure pieces**

Create `crates/forge-features/src/folder_choice.rs` with the tests first:

```rust
use std::path::PathBuf;

/// What to do with a folder the person picked in the dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderChoice {
    /// Already offered in the dropdown: select that project.
    Known(PathBuf),
    /// A folder with no Claude Code history yet: offer it and select it.
    Added(PathBuf),
    /// Not usable; the message says why.
    Rejected(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn known() -> Vec<PathBuf> {
        vec![PathBuf::from("/work/a"), PathBuf::from("/work/b")]
    }

    #[test]
    fn a_folder_already_in_the_list_selects_that_project() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/b"), true),
            FolderChoice::Known(PathBuf::from("/work/b"))
        );
    }

    #[test]
    fn a_trailing_slash_does_not_make_a_known_folder_new() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/b/"), true),
            FolderChoice::Known(PathBuf::from("/work/b"))
        );
    }

    #[test]
    fn a_new_existing_folder_is_added() {
        assert_eq!(
            choose_folder(&known(), PathBuf::from("/work/new"), true),
            FolderChoice::Added(PathBuf::from("/work/new"))
        );
    }

    #[test]
    fn a_path_that_is_not_a_directory_is_rejected_even_if_it_is_listed() {
        for path in ["/work/a", "/work/missing"] {
            let choice = choose_folder(&known(), PathBuf::from(path), false);
            assert!(matches!(&choice, FolderChoice::Rejected(m) if m.contains(path)), "{choice:?}");
        }
    }
}
```

Add `mod folder_choice;` and `pub use folder_choice::{choose_folder, FolderChoice};` to `lib.rs`.

Add to `sidebar.rs` tests (the file has helpers `open(...)`, and entries are built through `merge_sidebar`; build entries directly):

```rust
    fn entry(key: EntryKey, session: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key,
            session_id: session.map(|id| SessionId(id.to_string())),
            title: "t".to_string(),
            updated_at: None,
            marker: SessionMarker::Idle,
        }
    }

    #[test]
    fn a_past_session_can_be_renamed_and_deleted() {
        let past = entry(EntryKey::Past { project: PathBuf::from("/w"), session: SessionId("s".into()) }, Some("s"));
        assert_eq!(past.actions(), RowActions { can_rename: true, delete: DeleteAction::Allowed });
    }

    #[test]
    fn an_open_session_can_be_renamed_but_not_deleted() {
        let open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.actions(), RowActions { can_rename: true, delete: DeleteAction::WhileOpen });
    }

    #[test]
    fn an_open_session_without_a_claude_id_has_no_actions() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.actions(), RowActions { can_rename: false, delete: DeleteAction::Unavailable });
    }

    #[test]
    fn replacing_one_projects_sessions_leaves_the_others_and_updates_the_count() {
        let mut past = vec![
            (project("/a"), vec![summary("1", Some("one")), summary("2", Some("two"))]),
            (project("/b"), vec![summary("3", Some("three"))]),
        ];

        replace_project_sessions(&mut past, Path::new("/a"), vec![summary("2", Some("renamed"))]);

        assert_eq!(past[0].1.len(), 1);
        assert_eq!(past[0].1[0].title.as_deref(), Some("renamed"));
        assert_eq!(past[0].0.session_count, 1);
        assert_eq!(past[1].1.len(), 1, "the other project is untouched");
    }

    #[test]
    fn replacing_sessions_of_an_unknown_project_changes_nothing() {
        let mut past = vec![(project("/a"), vec![summary("1", None)])];
        replace_project_sessions(&mut past, Path::new("/zzz"), Vec::new());
        assert_eq!(past[0].1.len(), 1);
    }
```

(import `std::path::Path` in the tests module if not present.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features`
Expected: FAIL to compile (`choose_folder`, `RowActions`, `DeleteAction`, `actions`, `replace_project_sessions` missing).

- [ ] **Step 3: Implement the pure pieces**

`folder_choice.rs` (above the tests):

```rust
/// Decides what picking `picked` means. `known` are the folders the dropdown already offers;
/// `is_dir` is whether `picked` is an existing directory (the caller asks the file system, so this
/// stays pure). Trailing separators do not make a path different.
pub fn choose_folder(known: &[PathBuf], picked: PathBuf, is_dir: bool) -> FolderChoice {
    if !is_dir {
        return FolderChoice::Rejected(format!("{} is not a folder that exists.", picked.display()));
    }
    // `components()` ignores a trailing separator, so `/work/b/` equals `/work/b`.
    let normalized: PathBuf = picked.components().collect();
    match known.iter().find(|path| path.components().eq(normalized.components())) {
        Some(existing) => FolderChoice::Known(existing.clone()),
        None => FolderChoice::Added(normalized),
    }
}
```

In `sidebar.rs` add (top-level, after `SidebarEntry`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAction {
    Allowed,
    /// The session is open in Forge; it has to be closed first.
    WhileOpen,
    /// There is no transcript on disk to act on yet.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowActions {
    pub can_rename: bool,
    pub delete: DeleteAction,
}

impl SidebarEntry {
    /// What the row's menu offers. A session with no Claude id has no transcript to rename or delete.
    pub fn actions(&self) -> RowActions {
        match (&self.key, &self.session_id) {
            (_, None) => RowActions { can_rename: false, delete: DeleteAction::Unavailable },
            (EntryKey::Live(_), Some(_)) => RowActions { can_rename: true, delete: DeleteAction::WhileOpen },
            (EntryKey::Past { .. }, Some(_)) => RowActions { can_rename: true, delete: DeleteAction::Allowed },
        }
    }
}

/// Puts `sessions` in place of one project's listing and refreshes its count; other projects and
/// unknown paths are left alone.
pub fn replace_project_sessions(
    past: &mut [(ProjectSummary, Vec<SessionSummary>)],
    project_path: &std::path::Path,
    sessions: Vec<SessionSummary>,
) {
    if let Some((project, listed)) = past.iter_mut().find(|(project, _)| project.path == project_path) {
        project.session_count = sessions.len();
        *listed = sessions;
    }
}
```

(If `session_count` is not `usize`, match its type — check `ProjectSummary` in forge-domain.) In `lib.rs` extend the sidebar export with `replace_project_sessions, DeleteAction, RowActions`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-features`
Expected: PASS.

- [ ] **Step 5: Wire the workspace (no new tests possible without rendering; keep the logic in the pure functions above)**

In `workspace.rs`:

1. Import `forge_ports::FolderPickerPort`, `crate::folder_choice::{choose_folder, FolderChoice}`, `crate::sidebar::replace_project_sessions`, and `std::path::Path`.
2. Add to `WorkspaceServices`: `pub folder_picker: Arc<dyn FolderPickerPort>,`.
3. Add to `Workspace`: `extra_folders: Signal<Vec<PathBuf>>` and `picking: Signal<bool>`, initialised `Signal::new(Vec::new())` and `Signal::new(false)` in `use_workspace_provider`.
4. Change `new_session_projects` to append the added folders:

```rust
    pub fn new_session_projects(&self) -> Vec<ProjectSummary> {
        let past = self.past.read();
        let mut projects: Vec<ProjectSummary> = past
            .iter()
            .filter(|(project, _)| project.exists)
            .map(|(project, _)| project.clone())
            .collect();
        for path in self.extra_folders.read().iter() {
            if !projects.iter().any(|project| project.path == *path) {
                projects.push(ProjectSummary {
                    path: path.clone(),
                    last_active: String::new(),
                    session_count: 0,
                    exists: true,
                });
            }
        }
        projects
    }
```

5. Add the methods:

```rust
    /// Reloads one project's session list, leaving every other project as drawn.
    pub fn reload_project(&self, project: PathBuf) {
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        let mut notice = self.notice;
        spawn(async move {
            match services.sessions.list_sessions(&project).await {
                Ok(sessions) => replace_project_sessions(&mut past.write(), &project, sessions),
                Err(error) => notice.set(Some(format!("{}: {error}", project.display()))),
            }
        });
    }

    /// Renames a session (open or not). The list is re-read for that project only.
    pub fn rename_session(&self, project: PathBuf, session: SessionId, title: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            match services.registry.rename(&project, &session, &title).await {
                Ok(_) => this.reload_project(project),
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Moves a session that is not open to the trash. The list is re-read for that project only.
    pub fn delete_session(&self, project: PathBuf, session: SessionId) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            match services.registry.delete(&project, &session).await {
                Ok(()) => this.reload_project(project),
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Whether a folder dialog is open; the button is disabled meanwhile so only one can be.
    pub fn picking_folder(&self) -> bool {
        *self.picking.read()
    }

    /// Opens the native folder dialog and applies the choice to the new-session panel.
    pub fn open_folder(&self) {
        if self.picking_folder() {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        picking.set(true);
        // Root scope: the dialog outlives a re-render of the composer that asked for it.
        dioxus::core::spawn_forever(async move {
            let picked = services.folder_picker.pick_folder().await;
            picking.set(false);
            let Some(picked) = picked else {
                return;
            };
            let known: Vec<PathBuf> = this.new_session_projects().into_iter().map(|project| project.path).collect();
            let is_dir = tokio::fs::metadata(&picked).await.map(|m| m.is_dir()).unwrap_or(false);
            match choose_folder(&known, picked, is_dir) {
                FolderChoice::Known(path) => this.set_new_folder(Some(path)),
                FolderChoice::Added(path) => {
                    let mut extra = this.extra_folders;
                    extra.write().push(path.clone());
                    this.set_new_folder(Some(path));
                }
                FolderChoice::Rejected(message) => this.report(message),
            }
        });
    }
```

`forge-features` does not depend on `tokio`; use `std::fs::metadata(&picked).map(|m| m.is_dir()).unwrap_or(false)` (a single stat, fine on the UI thread) instead of `tokio::fs`, and drop the tokio line above.

6. In `forge-app/Cargo.toml` add `forge-adapter-dialog = { path = "../forge-adapter-dialog" }`, and in `main.rs` import `forge_adapter_dialog::RfdFolderPicker`, then change the last line of `build_services` to:

```rust
    let folder_picker = Arc::new(RfdFolderPicker::new());
    WorkspaceServices { registry, projects, sessions, settings, extensions, folder_picker }
```

- [ ] **Step 6: Build and run the full check**

Run: `cargo build --workspace && cargo test --workspace`
Expected: build succeeds with only the known `block v0.1.6` notice; all tests pass (count = baseline + the tests added in Tasks 2–6).

- [ ] **Step 7: Commit**

```bash
git add Cargo.lock crates
git commit -m "feat: workspace actions for rename, delete and open folder, reloading only the affected project" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Sidebar — the `⋯` menu, in-place rename and delete confirmation

**Files:**
- Modify: `crates/forge-entities/src/sidebar_item.rs` (a `children` slot)
- Create: `crates/forge-widgets/src/session_row.rs`
- Modify: `crates/forge-widgets/src/{lib.rs, sidebar.rs}`
- Modify: `crates/forge-shared/src/theme.css` (and its class-list test)

**Interfaces:**
- Consumes: `SidebarEntry::actions`, `RowActions`, `DeleteAction` (Task 6); `Workspace::{rename_session, delete_session}`.
- Produces:
  - `SidebarItem` gains `#[props(default)] children: Element`, drawn after the subtitle.
  - `SessionRow` component: props `project: PathBuf`, `entry: SidebarEntry`, `active: bool`, `onopen: EventHandler<()>`.
  - Pure helper in `session_row.rs`: `pub fn rename_submit(draft: &str, original: &str) -> Option<String>` — the trimmed title to save, or `None` when there is nothing to save (empty after trimming, or unchanged).
  - CSS classes `.row-menu-button`, `.row-menu`, `.row-menu-item`, `.row-menu-note`, `.rename-input`, `.confirm-row`.

- [ ] **Step 1: Write the failing tests**

In `session_row.rs` (create it with the helper's tests only first):

```rust
/// What a rename input should save: the trimmed draft, unless it is empty or the title the session
/// already has (then closing the input is all that is needed).
pub fn rename_submit(draft: &str, original: &str) -> Option<String> {
    let _ = (draft, original);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_title_is_saved_trimmed() {
        assert_eq!(rename_submit("  New name ", "Old name"), Some("New name".to_string()));
    }

    #[test]
    fn an_empty_or_blank_draft_saves_nothing() {
        assert_eq!(rename_submit("", "Old"), None);
        assert_eq!(rename_submit("   ", "Old"), None);
    }

    #[test]
    fn an_unchanged_title_saves_nothing() {
        assert_eq!(rename_submit("Old", "Old"), None);
        assert_eq!(rename_submit(" Old ", "Old"), None);
    }
}
```

Add to the `theme.rs` class-list test the classes `".row-menu-button"`, `".row-menu"`, `".rename-input"`, `".confirm-row"`.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-widgets session_row && cargo test -p forge-shared`
Expected: FAIL — `a_changed_title_is_saved_trimmed` fails (stub returns `None`) and the stylesheet test fails (classes missing). (Add `mod session_row;` to `forge-widgets/src/lib.rs` first so the test is found.)

- [ ] **Step 3: Implement the helper**

```rust
pub fn rename_submit(draft: &str, original: &str) -> Option<String> {
    let trimmed = draft.trim();
    if trimmed.is_empty() || trimmed == original.trim() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

(Length over 200 is not checked here: the registry owns that rule and its message appears as the sidebar notice.)

- [ ] **Step 4: Add the CSS**

Append to `theme.css` after the `.sidebar-item` rules, using only variables:

```css
.row-menu-button { flex: none; width: 22px; height: 22px; border: none; border-radius: 6px; background: transparent; color: var(--fg-muted); font: inherit; line-height: 1; cursor: pointer; opacity: 0; }
.sidebar-item:hover .row-menu-button, .row-menu-button:focus-visible, .row-menu-button.open { opacity: 1; }
.row-menu-button:hover { background: var(--bg-hover); color: var(--fg); }
.row-menu { position: absolute; right: 4px; top: 100%; z-index: 5; min-width: 150px; padding: 4px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); box-shadow: var(--shadow); }
.row-menu-item { display: block; width: 100%; padding: 6px 8px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
.row-menu-item:hover:not(:disabled) { background: var(--bg-hover); }
.row-menu-item.danger { color: var(--danger); }
.row-menu-item:disabled { color: var(--fg-muted); cursor: default; }
.row-menu-note { padding: 2px 8px 4px; font-size: 11px; color: var(--fg-muted); }
.sidebar-row { position: relative; }
.rename-input { flex: 1; min-width: 0; padding: 2px 6px; border: 1px solid var(--accent); border-radius: 6px; background: var(--bg-card); color: var(--fg); font: inherit; outline: none; }
.confirm-row { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; padding: 6px 8px; border: 1px solid var(--danger); border-radius: 8px; font-size: 12px; }
.confirm-row .confirm-text { flex: 1 1 100%; color: var(--fg-muted); }
```

- [ ] **Step 5: Add the `children` slot**

In `sidebar_item.rs`:

```rust
#[component]
pub fn SidebarItem(
    title: String,
    subtitle: String,
    marker: SessionMarker,
    active: bool,
    onclick: EventHandler<MouseEvent>,
    #[props(default)] children: Element,
) -> Element {
    let class = if active { "sidebar-item active" } else { "sidebar-item" };
    rsx! {
        div { class, onclick: move |evt| onclick.call(evt),
            StatusDot { tone: marker.tone() }
            span { class: "title", "{title}" }
            span { class: "subtitle", "{subtitle}" }
            {children}
        }
    }
}
```

- [ ] **Step 6: Write `SessionRow`**

Append to `session_row.rs` (above the tests):

```rust
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_entities::{relative_time, SidebarItem};
use forge_features::{use_workspace, DeleteAction, SidebarEntry};
use forge_shared::{Button, ButtonKind};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
enum RowMode {
    Normal,
    MenuOpen,
    Renaming(String),
    ConfirmingDelete,
}

/// One session in the sidebar: the item itself, a `⋯` menu on hover, an in-place rename input and
/// an inline delete confirmation. The mode is local to the row.
#[component]
pub fn SessionRow(project: PathBuf, entry: SidebarEntry, active: bool, onopen: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut mode = use_signal(|| RowMode::Normal);
    let actions = entry.actions();
    let now = OffsetDateTime::now_utc();
    let session_id = entry.session_id.clone();

    match mode() {
        RowMode::Renaming(draft) => {
            let original = entry.title.clone();
            let project = project.clone();
            let session_id = session_id.clone();
            rsx! {
                div { class: "sidebar-item sidebar-row",
                    input {
                        class: "rename-input",
                        r#type: "text",
                        value: "{draft}",
                        autofocus: true,
                        spellcheck: "false",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        // A click in the field must not open the session behind it.
                        onclick: move |event| event.stop_propagation(),
                        oninput: move |event| mode.set(RowMode::Renaming(event.value())),
                        onblur: move |_| mode.set(RowMode::Normal),
                        onkeydown: {
                            let draft = draft.clone();
                            move |event: KeyboardEvent| {
                                event.stop_propagation();
                                match event.key() {
                                    // Enter that ends an IME composition (Korean, Japanese, Chinese)
                                    // confirms the text being composed, not the rename.
                                    Key::Enter if !event.is_composing() => {
                                        event.prevent_default();
                                        if let (Some(title), Some(id)) = (rename_submit(&draft, &original), session_id.clone()) {
                                            workspace.rename_session(project.clone(), id, title);
                                        }
                                        mode.set(RowMode::Normal);
                                    }
                                    Key::Escape => mode.set(RowMode::Normal),
                                    _ => {}
                                }
                            }
                        },
                    }
                }
            }
        }
        RowMode::ConfirmingDelete => {
            let project = project.clone();
            let session_id = session_id.clone();
            rsx! {
                div { class: "confirm-row",
                    span { class: "confirm-text", "Delete “{entry.title}”? Moves the session to the trash." }
                    Button { label: "Cancel", onclick: move |_| mode.set(RowMode::Normal) }
                    Button {
                        label: "Delete",
                        kind: ButtonKind::Danger,
                        onclick: move |_| {
                            if let Some(id) = session_id.clone() {
                                workspace.delete_session(project.clone(), id);
                            }
                            mode.set(RowMode::Normal);
                        },
                    }
                }
            }
        }
        current => {
            let menu_open = current == RowMode::MenuOpen;
            let has_menu = actions.can_rename || actions.delete != DeleteAction::Unavailable;
            rsx! {
                div { class: "sidebar-row",
                    SidebarItem {
                        title: entry.title.clone(),
                        subtitle: entry
                            .updated_at
                            .as_deref()
                            .map(|updated_at| relative_time(updated_at, now))
                            .unwrap_or_default(),
                        marker: entry.marker,
                        active,
                        onclick: move |_| {
                            mode.set(RowMode::Normal);
                            onopen.call(());
                        },
                        if has_menu {
                            button {
                                class: if menu_open { "row-menu-button open" } else { "row-menu-button" },
                                title: "Rename or delete",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    mode.set(if menu_open { RowMode::Normal } else { RowMode::MenuOpen });
                                },
                                "⋯"
                            }
                        }
                    }
                    if menu_open {
                        div { class: "row-menu", onclick: move |event| event.stop_propagation(),
                            if actions.can_rename {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let title = entry.title.clone();
                                        move |_| mode.set(RowMode::Renaming(title.clone()))
                                    },
                                    "Rename"
                                }
                            }
                            button {
                                class: "row-menu-item danger",
                                disabled: actions.delete != DeleteAction::Allowed,
                                onclick: move |_| mode.set(RowMode::ConfirmingDelete),
                                "Delete"
                            }
                            if actions.delete == DeleteAction::WhileOpen {
                                div { class: "row-menu-note", "Close the session first (⌘W) to delete it." }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

Add `pub use session_row::SessionRow;` to `lib.rs` (and `time`/`forge-entities` are already dependencies of `forge-widgets`).

- [ ] **Step 7: Use it in the sidebar**

In `forge-widgets/src/sidebar.rs` replace the `for entry in group.entries.iter() { SidebarItem { … } }` loop with:

```rust
                    for entry in group.entries.iter() {
                        SessionRow {
                            key: "{entry.key.dom_key()}",
                            project: group.project.path.clone(),
                            entry: entry.clone(),
                            active: match &entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            onopen: {
                                let project = group.project.clone();
                                let key = entry.key.clone();
                                move |_| match &key {
                                    EntryKey::Live(id) => workspace.select(*id),
                                    EntryKey::Past { session, .. } => {
                                        workspace.open_past(project.clone(), session.clone())
                                    }
                                }
                            },
                        }
                    }
```

and fix the imports: drop `relative_time`, `SidebarItem` and `OffsetDateTime`/`now` from `sidebar.rs` if now unused (the compiler names them), add `use crate::session_row::SessionRow;`.

- [ ] **Step 8: Run the tests and a build**

Run: `cargo test --workspace && cargo build -p forge-app`
Expected: PASS; no new warnings.

- [ ] **Step 9: Commit**

```bash
git add crates
git commit -m "feat: session menu in the sidebar with in-place rename and delete confirmation" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: "Open folder…" in the new-session panel

**Files:**
- Modify: `crates/forge-features/src/new_session.rs`
- Modify: `crates/forge-shared/src/theme.css` (a small `.field-row`, only if the existing `.field` layout cannot hold the button)

**Interfaces:**
- Consumes: `Workspace::{open_folder, picking_folder}` (Task 6).
- Produces: nothing for later tasks.

- [ ] **Step 1: Add the button**

In `NewSessionComposer`, change the Folder field to:

```rust
            div { class: "field",
                label { "Folder" }
                Select {
                    value: selected,
                    options,
                    disabled: !has_folder,
                    onchange: move |path: String| workspace.set_new_folder(Some(path.into())),
                }
                Button {
                    label: "Open folder…",
                    disabled: workspace.picking_folder(),
                    onclick: move |_| workspace.open_folder(),
                }
            }
```

The `Select` stays enabled-by-`has_folder`, but "Open folder…" must work when the list is empty (that is when it is most needed). Also change the empty-state banner to: "No existing project folder was found in your Claude Code history. Use “Open folder…” to choose one." and make the message box and Start button's `disabled` depend on `has_folder`, which now becomes true as soon as a folder is added (it is computed from `new_session_projects()`, which includes added folders).

- [ ] **Step 2: Check the layout**

Run: `cargo build -p forge-app` — expected: no new warnings. If the Select and button do not sit on one line, add `.field { display: flex; align-items: center; gap: 8px; }`-compatible styling in a new `.field-folder` class instead of changing `.field` (it is shared with the Model and Mode rows).

- [ ] **Step 3: Commit**

```bash
git add crates
git commit -m "feat: Open folder button in the new session panel" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Manual checklist, fixes and "As built"

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§19.9 As built; fix §19.1 for Decision 1)

**Interfaces:** none.

The dialog and the sidebar interactions cannot be unit-tested (spec §8). Do the checklist with the real window: `cd sidecar && npm run build`, then `cargo run -p forge-app`. Use a scratch project, not a real one. A person must do the dialog steps.

- [ ] **Step 1: Walk the checklist, recording pass/fail for each**

1. Hover a past session: a `⋯` appears; clicking it opens the menu and does **not** open the session; clicking elsewhere does not leave a stuck menu.
2. Rename: type a new name, Enter → the title changes, the list reloads for that project only (other groups do not flicker), the renamed session moves to the top. Esc cancels. Clicking away cancels. An unchanged or blank name does nothing.
3. Korean IME: in the rename input, type a Korean word and press Enter to commit the composition → the rename is **not** submitted; a second Enter submits.
4. Rename an open session: the sidebar and the live view show the new title at once; send another message, wait for the turn to finish → the title is still the new one. Open the same project in `claude` → the new title is shown there.
5. A 201-character title → the notice says the limit; nothing changes on disk.
6. Delete a past session: confirmation reads "Moves the session to the trash"; Cancel keeps it; Delete removes it from the list and it is in the Trash (Finder) with its sibling directory if it had one.
7. An open session's menu has Delete disabled with the "Close the session first" note; after ⌘W it can be deleted.
8. Shortcuts still work while a rename input or the menu is open (⌘N, Ctrl+Tab); ⌘W while renaming closes the session as before and does not leave a stuck input.
9. "Open folder…": the dialog opens, the window stays responsive; cancel changes nothing; picking a listed folder selects it; picking a new folder adds it to the dropdown and selects it; Start works there and the project then shows in the sidebar; picking while the dialog is already open (double click) opens one dialog.
10. With no Claude Code history at all (set `HOME` to an empty temp directory for one run), "Open folder…" is usable and Start becomes enabled after a folder is chosen.

- [ ] **Step 2: Fix anything that failed**, each with a failing test first where the logic is testable, and a commit per fix.

- [ ] **Step 3: Write §19.9 "As built" and correct §19.1**

Record: branch, date, test count, what the checklist showed (checked vs not checked), the differences from §19 (Decisions 1–5 above if they stand), and the deferred minors found. In §19.1 change the delete rule to "A session open in Forge cannot be deleted" if Decision 1 stands.

- [ ] **Step 4: Full verification**

Run: `cargo test --workspace && (cd sidecar && npm test && npm run typecheck)`
Expected: all pass; paste the totals into §19.9.

- [ ] **Step 5: Commit**

```bash
git add docs
git commit -m "docs: record Plan 9 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage (§19):** 19.1 scope/decisions → Decisions 1, Global Constraints; 19.2 ports/adapters → Tasks 2, 3, 5; 19.3 spike → Task 1; 19.4 validation, live guard, live title, reload scope → Tasks 4, 6; 19.5 UI (menu, in-place rename, confirmation, Open folder, "folder already listed selects it", "path that does not exist is rejected", overlay-aware shortcuts) → Tasks 6, 7, 8 and checklist items 8–9; 19.6 testing → each task's tests, dialog manual; 19.7 findings → Task 1; 19.8 order → Tasks 1–9 (spec's steps 2–4 are Tasks 2, 3, 5 with the port methods folded into the adapter tasks so the workspace always compiles).

**Placeholder scan:** no "TBD"/"TODO"/"similar to". Conditionals are explicit and tied to a Task 1 outcome (sibling-directory lines in Task 3; `rfd` features in Task 5; the flagged-line note in Task 2).

**Type consistency:** `rename_session(&Path, &SessionId, &str)` / `delete_session(&Path, &SessionId)` match across Tasks 2, 3, 4, 6; `ManageSessionError` variants match between Task 4's tests, implementation and Task 7's use; `RowActions { can_rename, delete: DeleteAction }` matches Tasks 6 and 7; `choose_folder(&[PathBuf], PathBuf, bool) -> FolderChoice` matches Tasks 6 tests and `open_folder`; `WorkspaceServices.folder_picker` is added in Task 6 and wired in the same task.

**Review Focus coverage:** 1 → Task 4 tests; 2–4 → Tasks 2, 3; 5 → Tasks 3, 4; 6 → Tasks 2, 4; 7 → Task 7 `onkeydown` guard plus checklist 3 (a keyboard-IME case cannot be unit-tested); 8 → Tasks 6 (`choose_folder`, `picking` guard) and checklist 9.

**Known soft spots to watch during execution:** the `failed` closure in Task 2 Step 8 may need to become a function; `ProjectSummary::session_count`'s type must be matched in `replace_project_sessions`; `wait_for` takes `(&mut receiver, what: &str, condition)`; the plan's calls follow that.
