# Fork from a message (Plan 68A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A fork icon on each user message makes a new session cut before that message (the message text back in the composer), and a fork shows a `포크 원본 <title> <time>` line at the top of its chat.

**Architecture:** The session record already carries `forkedFrom`; Task 1 reads it into `Session.forked_from` (and makes an empty fork identifiable). Task 2 adds the workspace functions: the origin lookup and `fork_from_message`, which reuses `Registry::fork_before` and the ↺ composer hand-back (`replace_session`). Task 3 is the UI: the icon, the line, Korean text. Task 4 checks the real window and updates the docs.

**Tech Stack:** Rust, Dioxus, tokio, serde_json.

**Spec:** `docs/superpowers/specs/2026-10-28-message-fork-design.md`. Observation: `docs/superpowers/roadmap/observations.md`, round 32.

## Global Constraints

- Korean UI text goes through `tr!` with an entry in `crates/forge-shared/src/ko.rs`, **sorted by English key in byte order**; `cargo test -p forge-shared` fails on an unsorted table, a missing entry or an unused one.
- The original session is never changed by a fork.
- Match the surrounding comment density and naming. Commit after each task (`feat:` / `docs:`), ending the message with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Run focused crate tests per task and the whole workspace once in Task 4. The `forge-adapter-claude-plugins` `round_trip` test fails on this machine for reasons unrelated to this work.
- Work in `/Users/james/orca/workspaces/forge/features` (branch `plan-67`). Do not touch `/Users/james/Developer/forge`.

## Review Focus

- Forking the **first** user message gives an empty copy: it must still open, show the `포크 원본` line (Task 1 puts `forkedFrom` on the new title line for this) and hold the message text in the box.
- A fork of a fork names its **immediate** source, not the first ancestor.
- An origin that is no longer listed (deleted, another folder) or has no title hides the line, with no panic and no empty link.
- A message without a `uuid` (not written to the record yet) shows no fork icon, and the workspace call on a missing message is a notice, not a panic.
- Forking while a turn runs leaves the running session alone (the cut only reads the file).

---

### Task 1: Read `forkedFrom` into `Session`

**Files:**
- Modify: `crates/forge-domain/src/session.rs` (struct `Session`, its test constructor at ~303)
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (`load_session` ~638, tests module)
- Modify: `crates/forge-adapter-claude-fs/src/fork.rs` (`forked_transcript` title line, tests)
- Modify (add `forked_from: None,` to each `Session { .. }` literal): `crates/forge-application/src/registry.rs` (~136, ~327), `registry_host.rs` (~249), `event.rs` (~256), `live_session.rs` (~410), `use_cases/session_lifecycle.rs` (~113, ~163, ~192), `use_cases/sidecar_commands.rs` (~99), `crates/forge-features/src/tool_activity.rs` (~18), `sidebar.rs` (~612), `workspace.rs` (~3365)

**Interfaces:**
- Produces: `Session.forked_from: Option<SessionId>`: the session this one was forked from (its immediate source), `None` for an ordinary session.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-adapter-claude-fs/src/fork.rs` `mod tests`, add:

```rust
    #[test]
    fn the_new_title_line_names_the_source_so_an_empty_fork_is_still_a_fork() {
        let out = parse(&forked_transcript("", "old", "new", "Old (fork)", "now", counter()));

        assert_eq!(out.len(), 1);
        assert_eq!(out[0]["type"], "custom-title");
        assert_eq!(out[0]["forkedFrom"], json!({"sessionId": "old"}));
    }
```

In `crates/forge-adapter-claude-fs/src/sessions.rs` `mod tests`, after `fork_before_a_message_keeps_only_what_came_before_it`, add:

```rust
    #[tokio::test]
    async fn a_fork_knows_its_immediate_source_and_an_ordinary_session_has_none() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[message("u1", None, "hello"), message("u2", Some("u1"), "again")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());
        let first = SessionId("s1".to_string());

        let plain = adapter.load_session(&project(), &first).await.unwrap();
        assert_eq!(plain.forked_from, None);

        let fork = adapter.fork_session(&project(), &first, "T (fork)").await.unwrap();
        let loaded = adapter.load_session(&project(), &fork).await.unwrap();
        assert_eq!(loaded.forked_from, Some(first.clone()));

        let grandchild = adapter.fork_session_before(&project(), &fork, &{
            // the uuid of the fork's second message
            let text = std::fs::read_to_string(adapter.project_dir(&project()).await.join(format!("{}.jsonl", fork.0))).unwrap();
            text.lines().filter_map(|line| serde_json::from_str::<Value>(line).ok()).filter(|v| v["type"] == "user").nth(1).unwrap()["uuid"].as_str().unwrap().to_string()
        }, "T (fork) (fork)").await.unwrap();
        let loaded = adapter.load_session(&project(), &grandchild).await.unwrap();
        assert_eq!(loaded.forked_from, Some(fork), "the immediate source, not the first ancestor");
    }

    #[tokio::test]
    async fn forking_before_the_first_message_gives_an_empty_session_that_still_knows_its_source() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[message("u1", None, "hello"), message("u2", Some("u1"), "again")]);
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());
        let first = SessionId("s1".to_string());

        let fork = adapter.fork_session_before(&project(), &first, "u1", "T (fork)").await.unwrap();

        let loaded = adapter.load_session(&project(), &fork).await.unwrap();
        assert!(loaded.messages.is_empty());
        assert_eq!(loaded.forked_from, Some(first));
    }
```

If `project_dir` is private to the adapter and not reachable from the test, read the transcript with the same path helper the neighbouring tests use (`write_session` shows how the path is built).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-adapter-claude-fs fork 2>&1 | tail -20`
Expected: compile error (`no field forked_from`).

- [ ] **Step 3: Implement**

`crates/forge-domain/src/session.rs`, in `struct Session` after `finished_tasks`:

```rust
    /// The session this one was forked from (its immediate source), read from the `forkedFrom` a fork writes.
    pub forked_from: Option<SessionId>,
```

Add `forked_from: None,` to every `Session { .. }` literal listed under **Files**. Let the compiler find them all: `cargo build --workspace --tests 2>&1 | grep -B2 -A6 "missing field"`.

`crates/forge-adapter-claude-fs/src/fork.rs`, in `forked_transcript`, the final title line:

```rust
    out.push_str(&json!({"type": "custom-title", "customTitle": title, "sessionId": new_id, "uuid": new_uuid(), "timestamp": now, "forkedFrom": {"sessionId": old_id}}).to_string());
```

Also extend the doc comment of `forked_transcript`: "…and one new one, with `title` and a `forkedFrom` naming the source, ends the file."

`crates/forge-adapter-claude-fs/src/sessions.rs`, in `load_session` after `apply_line(&mut session, &value)` add `note_fork(&mut session, &value);` (same block), and add next to `apply_line`:

```rust
/// The session a fork came from: the `forkedFrom.sessionId` of the first line that has one. A fork
/// writes it on every line it copies and on its title line, always naming its immediate source.
fn note_fork(session: &mut Session, line: &Value) {
    if session.forked_from.is_some() {
        return;
    }
    if let Some(id) = line.pointer("/forkedFrom/sessionId").and_then(Value::as_str).filter(|id| !id.is_empty()) {
        session.forked_from = Some(SessionId(id.to_string()));
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo build --workspace --tests 2>&1 | tail -5 && cargo test -p forge-adapter-claude-fs -p forge-domain 2>&1 | grep -E "test result|FAILED|failed"`
Expected: builds; all pass. Also `cargo test -p forge-adapter-claude-fs usage_stats` still passes (a title line with `forkedFrom` is not an assistant or user line).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: a session read from its record knows which session it was forked from

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The origin lookup and `fork_from_message`

**Files:**
- Modify: `crates/forge-features/src/sidebar.rs` (new `ForkOrigin`, `fork_origin`, tests)
- Modify: `crates/forge-features/src/lib.rs` (export)
- Modify: `crates/forge-features/src/workspace.rs` (`open_past_in`, replace `fork_session_before`, new `fork_origin` method)

**Interfaces:**
- Consumes: `Session.forked_from` (Task 1); `Registry::fork_before(&Path, &SessionId, &str, &str) -> Result<SessionId, _>`; `LiveSessionHandle::replace_session(Session, Option<(String, Vec<Attachment>)>)`.
- Produces:
  - `pub struct ForkOrigin { pub project: ProjectSummary, pub session_id: SessionId, pub title: String, pub forked_at: String }`
  - `pub fn fork_origin(past: &[(ProjectSummary, Vec<SessionSummary>)], project_path: &Path, origin: &SessionId, own: Option<&SessionId>) -> Option<ForkOrigin>`
  - `Workspace::fork_origin(&self, id: LiveSessionId) -> Option<ForkOrigin>`
  - `Workspace::fork_from_message(&self, id: LiveSessionId, message_uuid: String)`

- [ ] **Step 1: Write the failing tests**

In `crates/forge-features/src/sidebar.rs` `mod tests` (it has `summary(id, title)` and `listed(path)` helpers; `listed` returns a project with an empty session list, so build the tuple directly):

```rust
    fn past_with(path: &str, sessions: Vec<SessionSummary>) -> Vec<(ProjectSummary, Vec<SessionSummary>)> {
        let (project, _) = listed(path);
        vec![(project, sessions)]
    }

    #[test]
    fn the_origin_of_a_fork_is_found_in_its_own_project_with_the_forks_start_time() {
        let past = past_with("/work/a", vec![summary("origin", Some("Fix login")), summary("fork", Some("Fix login (fork)"))]);

        let found = fork_origin(&past, std::path::Path::new("/work/a"), &SessionId("origin".to_string()), Some(&SessionId("fork".to_string()))).unwrap();

        assert_eq!(found.title, "Fix login");
        assert_eq!(found.session_id, SessionId("origin".to_string()));
        assert_eq!(found.project.path, PathBuf::from("/work/a"));
        assert_eq!(found.forked_at, "2026-09-28T00:00:04Z", "the fork's created_at");
    }

    #[test]
    fn a_missing_untitled_or_other_project_origin_hides_the_line() {
        let past = past_with("/work/a", vec![summary("origin", None), summary("fork", Some("F"))]);
        let fork = SessionId("fork".to_string());
        let path = std::path::Path::new("/work/a");

        assert_eq!(fork_origin(&past, path, &SessionId("origin".to_string()), Some(&fork)), None, "no title");
        assert_eq!(fork_origin(&past, path, &SessionId("gone".to_string()), Some(&fork)), None, "not listed");
        assert_eq!(fork_origin(&past, std::path::Path::new("/work/b"), &SessionId("origin".to_string()), Some(&fork)), None, "another folder");
    }

    #[test]
    fn a_fork_that_is_not_listed_yet_has_no_time_but_still_shows_its_origin() {
        let past = past_with("/work/a", vec![summary("origin", Some("Mine"))]);

        let found = fork_origin(&past, std::path::Path::new("/work/a"), &SessionId("origin".to_string()), None).unwrap();

        assert_eq!(found.forked_at, "");
    }
```

(`summary("fork", ..)` has `id.len() % 10` = 4, so its `created_at` is `2026-09-28T00:00:04Z`; keep the assertion in step with the helper.)

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-features fork_origin 2>&1 | tail -15`
Expected: `cannot find function fork_origin`.

- [ ] **Step 3: Implement the helper**

`crates/forge-features/src/sidebar.rs`, after `project_for_open`:

```rust
/// Where a fork came from, as the `포크 원본` line at the top of its chat shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ForkOrigin {
    pub project: ProjectSummary,
    pub session_id: SessionId,
    pub title: String,
    /// When the fork began (RFC 3339); empty while the fork is not in the listing yet.
    pub forked_at: String,
}

/// The origin of the fork `own`, looked up in the listing of `project_path`. `None` (the line is
/// hidden) when the origin is not listed there or has no title.
pub fn fork_origin(
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    project_path: &std::path::Path,
    origin: &SessionId,
    own: Option<&SessionId>,
) -> Option<ForkOrigin> {
    let (project, sessions) = past.iter().find(|(project, _)| project.path == project_path)?;
    let found = sessions.iter().find(|session| &session.id == origin)?;
    let title = found.title.clone().filter(|title| !title.trim().is_empty())?;
    let forked_at = own
        .and_then(|own| sessions.iter().find(|session| &session.id == own))
        .map(|session| session.created_at.clone())
        .unwrap_or_default();
    Some(ForkOrigin { project: project.clone(), session_id: found.id.clone(), title, forked_at })
}
```

`crates/forge-features/src/lib.rs`: add `fork_origin, ForkOrigin,` to the `pub use sidebar::{ ... }` list (keep it alphabetical-ish with the neighbours).

- [ ] **Step 4: Run the helper tests**

Run: `cargo test -p forge-features fork_origin 2>&1 | grep -E "test result|FAILED"`
Expected: PASS.

- [ ] **Step 5: Implement the workspace functions**

`crates/forge-features/src/workspace.rs`:

(a) Replace `fork_session_before` (the unused one near `fork_session`) with:

```rust
    /// The fork icon of a user message: copies the open session as it was before that message,
    /// opens the copy and puts the message back in its box. The original is left as it is. A failure
    /// shows as a notice and nothing is opened.
    pub fn fork_from_message(&self, id: LiveSessionId, message_uuid: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        let Some(view) = self.live_view(id) else { return };
        let (project, session_id, title, message) = {
            let view = view.peek();
            (
                view.project_path.clone(),
                view.claude_session_id.clone(),
                view.session.title.clone().unwrap_or_default(),
                view.session.messages.iter().find(|m| m.uuid.as_deref() == Some(message_uuid.as_str())).cloned(),
            )
        };
        let (Some(session_id), Some(message)) = (session_id, message) else {
            return self.report(tr!("This message cannot be forked yet.").to_string());
        };
        let unsent = (forge_domain::with_cards(&message.cards, &message.text()), message.attachments.clone());
        // Root scope: the copy is made on disk and must be opened even if this chat re-renders meanwhile.
        dioxus::core::spawn_forever(async move {
            let new_id = match services.registry.fork_before(&project, &session_id, &message_uuid, &title).await {
                Ok(id) => id,
                Err(error) => return this.report(error.to_string()),
            };
            match services.sessions.list_sessions(&project).await {
                Ok(sessions) => replace_project_sessions(&mut past.write(), &project, sessions),
                Err(error) => return this.report(format!("{}: {error}", project.display())),
            }
            let summary = project_for_open(&past.peek(), &project);
            this.open_past_with(summary, new_id, PastOpen::Replace, Some(unsent));
        });
    }

    /// The origin of the open session when it is a fork and the origin is listed (the `포크 원본` line).
    pub fn fork_origin(&self, id: LiveSessionId) -> Option<ForkOrigin> {
        let view = self.live_view(id)?;
        let view = view.read();
        let origin = view.session.forked_from.as_ref()?;
        crate::sidebar::fork_origin(&self.past.read(), &view.project_path, origin, view.claude_session_id.as_ref())
    }
```

Add `ForkOrigin` to the file's `use crate::sidebar::{...}` import (look at how `project_for_open` is imported and add it beside).

(b) `open_past_in`: rename its body into `open_past_with` and add the parameter. Keep `open_past_in` as a thin wrapper:

```rust
    fn open_past_in(&self, project: ProjectSummary, session_id: SessionId, how: PastOpen) {
        self.open_past_with(project, session_id, how, None);
    }

    /// Opens a past session; `unsent` is a message to put in its box once it is open (a fork from a message).
    fn open_past_with(&self, project: ProjectSummary, session_id: SessionId, how: PastOpen, unsent: Option<(String, Vec<forge_domain::Attachment>)>) {
```

Inside the spawned task, after the existing `match how { ... }` block (still inside `Ok(outcome) => { ... }`), add:

```rust
                    if let Some(unsent) = unsent {
                        match services.sessions.load_session(&project.path, &session_id).await {
                            Ok(session) => handle.replace_session(session, Some(unsent)),
                            Err(error) => this.report(error.to_string()),
                        }
                    }
```

`handle` is the `outcome.handle.clone()` already bound there. If `session_id` was moved into `open_existing`, it is cloned in the existing call (`session_id.clone()`); keep it so.

- [ ] **Step 6: Build and run the crate tests**

Run: `cargo build -p forge-features 2>&1 | tail -5 && cargo test -p forge-features 2>&1 | grep -E "test result|FAILED|failed"`
Expected: builds; PASS. (`forge_domain::Attachment` is the type of `message.attachments`; if the path differs, use the type that `rewind_conversation` passes to `replace_session`.)

- [ ] **Step 7: Commit**

```bash
git add -A && git commit -m "feat: fork from a message and find a fork's origin in the workspace

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The icon, the line, the Korean text

**Files:**
- Modify: `crates/forge-entities/src/turn.rs` (`Turns`, `TurnItem`: prop `on_fork`, the button)
- Create: `crates/forge-widgets/src/fork_line.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod fork_line;`)
- Modify: `crates/forge-widgets/src/chat_window.rs` (wire both)
- Modify: `crates/forge-shared/src/ko.rs` (three entries, sorted)

**Interfaces:**
- Consumes: `Workspace::fork_from_message`, `Workspace::fork_origin`, `Workspace::open_past_clicked(ProjectSummary, SessionId)` (Task 2).
- Produces: `Turns { on_fork: Option<EventHandler<String>> }` (default `None`: no icon); component `ForkLine { id: LiveSessionId }`.

- [ ] **Step 1: Add the Korean entries first so the sort test guides placement**

`crates/forge-shared/src/ko.rs`, keep byte order:

```rust
    ("Fork", "포크"),
    ("Fork from this message", "이 메시지에서 포크"),
    ("Fork the session", "세션 포크"),
    ("Forked from", "포크 원본"),
    ("Forward", "앞으로"),
```
and before `("This message cannot be rewound yet.", ...)`:
```rust
    ("This message cannot be forked yet.", "이 메시지는 아직 포크할 수 없습니다."),
```

- [ ] **Step 2: The icon**

`crates/forge-entities/src/turn.rs`: in `Turns` add after `on_rewind`:

```rust
    /// Asked to fork the session before the user message with this id (its action row's ⑂); no ⑂ without it.
    #[props(default)] on_fork: Option<EventHandler<String>>,
```
pass `on_fork` in the `TurnItem { ... }` call, add `on_fork: Option<EventHandler<String>>,` to `TurnItem`'s parameters, and after the `↺` button block add:

```rust
                            if let (Some(on_fork), Some(uuid)) = (on_fork, message.uuid.clone()) {
                                button {
                                    class: "icon-button",
                                    r#type: "button",
                                    title: tr!("Fork from this message"),
                                    "aria-label": tr!("Fork from this message"),
                                    onclick: move |_| on_fork.call(uuid.clone()),
                                    "⑂"
                                }
                            }
```

- [ ] **Step 3: The line**

Create `crates/forge-widgets/src/fork_line.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::relative_time;
use forge_features::use_workspace;
use forge_shared::tr;
use time::OffsetDateTime;

/// The grey line at the top of a fork's chat: the session it came from (a link to it) and how long ago the fork began.
#[component]
pub fn ForkLine(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(origin) = workspace.fork_origin(id) else { return rsx! {} };
    let time = relative_time(&origin.forked_at, OffsetDateTime::now_utc());
    let label = tr!("Forked from");
    let project = origin.project.clone();
    let session_id = origin.session_id.clone();
    rsx! {
        div { class: "start-line fork-line",
            span { "{label} " }
            button {
                class: "start-line-link",
                r#type: "button",
                onclick: move |_| workspace.open_past_clicked(project.clone(), session_id.clone()),
                "{origin.title}"
            }
            if !time.is_empty() {
                span { " {time}" }
            }
        }
    }
}
```

`crates/forge-widgets/src/lib.rs`: add `mod fork_line;` next to `mod start_line;` (sorted position among the `mod` lines).

`crates/forge-widgets/src/chat_window.rs`: add `use crate::fork_line::ForkLine;` beside the `StartLine` import; render `ForkLine { id }` right after `StartLine { id }`; and in the `Turns { ... }` call add after `on_rewind`:

```rust
                    on_fork: move |uuid: String| workspace.fork_from_message(id, uuid),
```

- [ ] **Step 4: Build and test**

Run: `cargo build --workspace 2>&1 | tail -5 && cargo test -p forge-shared -p forge-entities -p forge-widgets 2>&1 | grep -E "test result|FAILED|failed"`
Expected: builds; PASS (the i18n tests check that the three new sentences are used and translated, sorted).

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat: a fork icon on each user message and a 'Forked from' line at the top of a fork

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Real-window check and docs

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (NEW-15, NEW-18 rows)
- Modify: `docs/superpowers/roadmap/README.md` (a Plan 68 row)
- Modify: `docs/superpowers/specs/2026-10-28-message-fork-design.md` (an "As built" section)
- Create: the memory file `plan-68-message-fork.md` and its line in `MEMORY.md` (in `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/`)

- [ ] **Step 1: Whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E "test result|FAILED|failed|panicked" | sort | uniq -c`
Expected: every crate passes except `forge-adapter-claude-plugins` `round_trip` (known).

- [ ] **Step 2: Real window (harness)**

`scripts/harness/stop.sh; scripts/harness/launch.sh`, then `python3 scripts/harness/capture.py` and read the PNG after every click (`click.py` takes pixels of the PNG file; multiply what the viewer shows by its printed factor). In the fixture, open `Read the config` (it has an answered turn). Make sure it has at least two user messages: if it has one, the fork of that one is the "first message" case below; check the second case with a session of your own only if a turn is cheap (a real send costs a real turn and leaves a transcript in `~/.claude/projects/-private-tmp-forge-harness-my-app/`; delete it after).

Check and write down:
1. Hover the user message: the row shows time, copy, ↺ and `⑂` with the title `이 메시지에서 포크`.
2. Click `⑂`: a session `<title> (fork)` opens at once; its chat holds only what came before the message; the message text is in the box; the top shows `포크 원본 <title> <time>`.
3. The `포크 원본` title is a link: clicking it opens the original, which is whole.
4. For the first message: the fork is empty, the text is in the box, the line shows.
5. The sidebar lists the fork in the same group (flat; nesting is 68B).
6. A fork made from the row menu (key F) also shows the line.

Stop the harness: `scripts/harness/stop.sh`. Fix any defect with a test where one is possible, then commit it separately (`fix:`).

- [ ] **Step 3: Docs**

- `backlog.md` NEW-18: set Forge `partial`, Plan to `Plan 68A (built, window checked)`, and in the Notes say what is left: the nested sidebar row and its fold (68B). NEW-15: change the Plan cell to say the fork icon is built in Plan 68A (`✓✓` left).
- `README.md`: add a row `| 68A | Fork from a message (NEW-18) | built on plan-67 | Spec 2026-10-28-message-fork-design.md: ⑂ on a user message, the copy cut before it with the text in the box, the 포크 원본 line; the sidebar nesting is 68B |` after the Plan 61 row.
- Spec: add `## As built` with the window check results (what was checked, what was not) and the decision that an origin without a title hides the line.
- Memory: write `plan-68-message-fork.md` (frontmatter `name`, `description`, `metadata: type: project`) with what was built, the commit range, what is left (68B sidebar nesting, `✓✓`), and one-line lessons from the work; add its pointer line to `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add -A && git commit -m "docs: Plan 68A is built; backlog, roadmap and the spec's as-built notes

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
