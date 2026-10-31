# Finished Tasks Implementation Plan (Plan 45, OTH-10 last part)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A task that has ended stays in the Background tasks panel as a finished card (workflow or Bash), a running workflow shows its tokens, a denied agent shows a red dot, and a stopped or failed workflow says so in the chat line and its card.

**Architecture:** One value, `FinishedTask`, is filled from two places: the sidecar's `task_finished` event (live) and the `<task-notification>` block the SDK writes into the session record (reload). `Session.finished_tasks` holds it. `forge-entities` turns the list plus the session's Workflow and Bash calls into finished cards; the panel draws a `Finished N` fold under `Running`; the chat line gets a status suffix. The trash icon hides cards in per-session in-memory state.

**Tech Stack:** Rust workspace (Dioxus widgets, `cargo test -p <crate>`), TypeScript sidecar (`vitest`, `tsc`), Claude Agent SDK 0.3.295.

**Spec:** `docs/superpowers/specs/2026-10-17-finished-tasks-design.md`. Evidence: `docs/superpowers/roadmap/observations.md`, "Round 18" and "Round 18, part 2".

## Global Constraints

- Amber dots for an agent waiting for permission are **not built** (spec: not reproduced in round 18). Dot colours are blue running, grey done, red failed, hollow for an agent that never ended in a stopped run.
- Token text is `format_tokens` (`crates/forge-entities/src/usage_view.rs`), e.g. `157.4k`; it rounds to one decimal (157,385 reads `157.4k`, 157,469 reads `157.5k`, as the desktop app showed).
- `journal_path` keeps refusing any folder that is not `…/subagents/workflows/wf_*`. Nothing new reads a file outside it.
- Notification and journal parsers are loose: unknown tags, unknown lines and cut lines are skipped, never an error and never a panic.
- Every new visible string goes through `tr!` and has a Korean entry in `crates/forge-shared/src/ko.rs` (the table is alphabetical).
- The trash icon only hides cards (a per-session set of task ids in app memory). Nothing on disk is deleted.
- Commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Known limit, accepted: a task the person stopped leaves no `<task-notification>` in the session record (round 18: obs-b), so after a reload its card is gone. Live it shows as `Stopped`.

## Review Focus

- A `<result>` (or any text after the head) that contains `<status>failed</status>` lookalikes must not change the task's status or id (Task 1 test).
- A user message that is not a notification but starts with `<task-notification>` must stay an ordinary message when its line says another origin; a line with no `origin` at all (an older Claude Code) is still read as a notification (Task 2 tests).
- A session record with a Workflow call and no notification loads with an empty finished list and no card, not an error (Task 2 test).
- A task that is in both the running set and the finished list shows once, as finished (Task 5 test).
- Missing tokens, missing duration and a missing journal show nothing odd: no `0s`, no `0 tokens`, no empty steps box (Task 5 and Task 7 checks).

## File Structure

| File | Change |
| --- | --- |
| `crates/forge-domain/src/background.rs` | `TaskStatus`, `FinishedTask`, `record_finished`, `parse_task_notification`; `RunningTask.tokens` |
| `crates/forge-domain/src/session.rs`, `lib.rs` | `Session.finished_tasks`; exports |
| `crates/forge-adapter-claude-fs/src/sessions.rs` | the loader reads notification lines into `finished_tasks` |
| `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts`, `sidecar/scripts/print_task_events.mjs` | `task_progress`; `task_finished` with `tokens`, `duration_ms`; a probe script |
| `crates/forge-ports/src/sidecar_port.rs` | `TaskProgress`; `TaskFinished` gains `tokens`, `duration_ms` |
| `crates/forge-application/src/event.rs`, `live_session_actor.rs`, `live_session.rs` | apply the events; tokens on `RunningTask`; fill a finished task's missing tokens and time |
| `crates/forge-entities/src/workflow_progress.rs` | `AgentState::Failed`, `Step::failed`, `dot_class` |
| `crates/forge-entities/src/background.rs`, `lib.rs` | `TaskEnd`, `TaskLists`, `task_lists`, `error_text`, `end_label`, `card_key` |
| `crates/forge-features/src/task_list_views.rs` (new), `lib.rs`, `workspace.rs` | per-session fold state and cleared ids |
| `crates/forge-widgets/src/tasks_panel.rs`, `chat_window.rs`, `crates/forge-entities/src/turn.rs`, `crates/forge-shared/src/theme.css`, `ko.rs` | panel, chat line, styles, translations |
| `docs/superpowers/...`, memory | as built, roadmap row |

---

### Task 1: Domain types and the notification parser

**Files:**
- Modify: `crates/forge-domain/src/background.rs`, `crates/forge-domain/src/session.rs`, `crates/forge-domain/src/lib.rs`
- Modify (compile errors lead you): every `Session { … }` literal and every `RunningTask { … }` literal (Step 5 lists them).

**Interfaces:**
- Produces:
  - `pub enum TaskStatus { Completed, Failed, Stopped }` with `parse(&str) -> Option<Self>` and `as_str(self) -> &'static str`.
  - `pub struct FinishedTask { pub task_id: String, pub status: TaskStatus, pub summary: String, pub tokens: Option<u64>, pub duration_ms: Option<u64> }`.
  - `pub fn record_finished(list: &mut Vec<FinishedTask>, task: FinishedTask)`.
  - `pub fn parse_task_notification(text: &str) -> Option<FinishedTask>`.
  - `RunningTask.tokens: Option<u64>`; `Session.finished_tasks: Vec<FinishedTask>`.

- [ ] **Step 1: Write the failing tests** (append inside `mod tests` of `crates/forge-domain/src/background.rs`)

```rust
    const COMPLETED: &str = "<task-notification>\n<task-id>wm0ok3aqh</task-id>\n<tool-use-id>toolu_01A</tool-use-id>\n<output-file>/tmp/c/tasks/wm0ok3aqh.output</output-file>\n<status>completed</status>\n<summary>Dynamic workflow \"Find largest file\" completed</summary>\n<result>\"Combined summary\"</result>\n<diagnostics>Per-agent results: /x/journal.jsonl</diagnostics>\n<usage><agent_count>3</agent_count><agents_done>3</agents_done><subagent_tokens>157385</subagent_tokens><tool_uses>4</tool_uses><duration_ms>77189</duration_ms></usage>\n</task-notification>";
    const FAILED: &str = "<task-notification>\n<task-id>w5r3hmkem</task-id>\n<tool-use-id>toolu_01M</tool-use-id>\n<output-file>/tmp/c/tasks/w5r3hmkem.output</output-file>\n<status>failed</status>\n<summary>Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at &lt;anonymous&gt; (workflow.js:11:7)\n    at &lt;anonymous&gt; (evalmachine.&lt;anonymous&gt;:1:42)</summary>\n<recovery>To resume, call Workflow</recovery>\n<failures>parallel[1] failed: </failures>\n<usage><agent_count>3</agent_count><subagent_tokens>157469</subagent_tokens><tool_uses>3</tool_uses><duration_ms>70914</duration_ms></usage>\n</task-notification>";
    const BASH: &str = "<task-notification>\n<task-id>bg9x3ylba</task-id>\n<tool-use-id>toolu_01B</tool-use-id>\n<output-file>/tmp/c/tasks/bg9x3ylba.output</output-file>\n<status>completed</status>\n<summary>Background command \"Sleep 12 seconds then print done\" completed (exit code 0)</summary>\n</task-notification>";

    #[test]
    fn a_completed_workflow_notification_gives_status_tokens_and_time() {
        let task = parse_task_notification(COMPLETED).unwrap();
        assert_eq!(task.task_id, "wm0ok3aqh");
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.summary, "Dynamic workflow \"Find largest file\" completed");
        assert_eq!((task.tokens, task.duration_ms), (Some(157_385), Some(77_189)));
    }

    #[test]
    fn a_failed_notification_keeps_its_unescaped_multi_line_summary() {
        let task = parse_task_notification(FAILED).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.summary,
            "Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)\n    at <anonymous> (evalmachine.<anonymous>:1:42)"
        );
        assert_eq!((task.tokens, task.duration_ms), (Some(157_469), Some(70_914)));
    }

    #[test]
    fn a_bash_notification_has_no_tokens_and_no_time() {
        let task = parse_task_notification(BASH).unwrap();
        assert_eq!((task.task_id.as_str(), task.status, task.tokens, task.duration_ms), ("bg9x3ylba", TaskStatus::Completed, None, None));
    }

    #[test]
    fn tag_lookalikes_in_the_result_do_not_change_the_task() {
        let tricky = COMPLETED.replace("\"Combined summary\"", "<task-id>evil</task-id><status>failed</status><usage><subagent_tokens>1</subagent_tokens></usage>");
        let task = parse_task_notification(&tricky).unwrap();
        assert_eq!((task.task_id.as_str(), task.status), ("wm0ok3aqh", TaskStatus::Completed));
        assert_eq!(task.tokens, Some(157_385), "the real usage block is the last one");
    }

    #[test]
    fn other_text_is_not_a_notification() {
        for text in ["", "hello", "<task-notification></task-notification>", "<task-notification>\n<task-id>x</task-id>\n<status>weird</status>\n</task-notification>", "<task-notification>\n<task-id></task-id>\n<status>completed</status>\n</task-notification>", "run <task-notification> please"] {
            assert_eq!(parse_task_notification(text), None, "{text}");
        }
    }

    #[test]
    fn a_task_recorded_twice_keeps_the_newer_entry_last() {
        let mut list = Vec::new();
        let task = |id: &str, summary: &str| FinishedTask { task_id: id.to_string(), status: TaskStatus::Completed, summary: summary.to_string(), tokens: None, duration_ms: None };
        record_finished(&mut list, task("a", "first"));
        record_finished(&mut list, task("b", "other"));
        record_finished(&mut list, task("a", "second"));
        let seen: Vec<(&str, &str)> = list.iter().map(|item| (item.task_id.as_str(), item.summary.as_str())).collect();
        assert_eq!(seen, vec![("b", "other"), ("a", "second")]);
    }

    #[test]
    fn status_words_round_trip() {
        for status in [TaskStatus::Completed, TaskStatus::Failed, TaskStatus::Stopped] {
            assert_eq!(TaskStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(TaskStatus::parse(" stopped\n"), Some(TaskStatus::Stopped));
        assert_eq!(TaskStatus::parse("done"), None);
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-domain background 2>&1 | tail -15`
Expected: FAIL to compile (`parse_task_notification`, `TaskStatus` … not found).

- [ ] **Step 3: Write the implementation** (in `crates/forge-domain/src/background.rs`, above `#[cfg(test)]`; change `RunningTask`)

```rust
/// A task that is running now, when the live view first saw it (the clock counts from here) and
/// how many tokens the SDK says it has used so far.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningTask {
    pub info: BackgroundTaskInfo,
    pub seen_at: SystemTime,
    pub tokens: Option<u64>,
}

/// How a background task ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Completed,
    Failed,
    Stopped,
}

impl TaskStatus {
    pub fn parse(word: &str) -> Option<Self> {
        match word.trim() {
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "stopped" => Some(Self::Stopped),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// A task that has ended, with what the SDK said about it. `tokens` and `duration_ms` are what its
/// notification reported; a Bash command reports neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedTask {
    pub task_id: String,
    pub status: TaskStatus,
    pub summary: String,
    pub tokens: Option<u64>,
    pub duration_ms: Option<u64>,
}

/// Adds an ended task. One already listed under the same id is replaced and the new one goes last,
/// so the list stays in the order the tasks ended (a resumed workflow ends again).
pub fn record_finished(list: &mut Vec<FinishedTask>, task: FinishedTask) {
    list.retain(|item| item.task_id != task.task_id);
    list.push(task);
}

const NOTIFICATION_OPEN: &str = "<task-notification>";
/// What follows the head of a notification (`task-id` … `summary`); the head is read up to the first.
const AFTER_HEAD: [&str; 5] = ["<result>", "<diagnostics>", "<recovery>", "<failures>", "<usage>"];

fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&close)?;
    Some(&text[start..end])
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Reads the `<task-notification>` block the SDK writes into a session record when a task ends. The
/// format is the SDK's own, not a documented one, so it is read loosely: tags by name, the id, the
/// status and the summary from the head (before the result, which may hold any text), the usage from
/// the last `<usage>` block. `None` when it is not a notification or has no id or a status we do not know.
pub fn parse_task_notification(text: &str) -> Option<FinishedTask> {
    let text = text.trim();
    if !text.starts_with(NOTIFICATION_OPEN) {
        return None;
    }
    let head_end = AFTER_HEAD.iter().filter_map(|mark| text.find(mark)).min().unwrap_or(text.len());
    let head = &text[..head_end];
    let task_id = tag(head, "task-id")?.trim();
    let status = TaskStatus::parse(tag(head, "status")?)?;
    if task_id.is_empty() {
        return None;
    }
    let summary = unescape(tag(head, "summary").unwrap_or("").trim());
    let usage = text.rfind("<usage>").and_then(|at| tag(&text[at..], "usage"));
    let number = |name: &str| usage.and_then(|block| tag(block, name)).and_then(|value| value.trim().parse::<u64>().ok());
    Some(FinishedTask {
        task_id: task_id.to_string(),
        status,
        summary,
        tokens: number("subagent_tokens").or_else(|| number("total_tokens")),
        duration_ms: number("duration_ms"),
    })
}
```

Note: the old `RunningTask` doc comment is replaced by the new one; remove the old struct.

- [ ] **Step 4: Export, and add the Session field**

In `crates/forge-domain/src/lib.rs` replace `pub use background::{BackgroundTaskInfo, RunningTask};` with:

```rust
pub use background::{parse_task_notification, record_finished, BackgroundTaskInfo, FinishedTask, RunningTask, TaskStatus};
```

In `crates/forge-domain/src/session.rs` add to `pub struct Session` (after `usage`):

```rust
    /// The tasks that have ended, in the order they ended. Filled live from the sidecar and, for a
    /// session read from its record, from the `<task-notification>` lines.
    pub finished_tasks: Vec<crate::FinishedTask>,
```

- [ ] **Step 5: Fix the literals**

Run: `cargo check --workspace --all-targets 2>&1 | grep -E "^error|-->" | head -40`

Add `finished_tasks: Vec::new(),` to every `Session { … }` literal the compiler names. Known places: `crates/forge-features/src/tool_activity.rs:18`, `crates/forge-features/src/sidebar.rs:582`, `crates/forge-domain/src/session.rs:262`, `crates/forge-adapter-claude-fs/src/sessions.rs:546`, `crates/forge-application/src/event.rs:152`, `.../use_cases/sidecar_commands.rs:98`, `.../registry.rs:136` and `:298`, `.../registry_host.rs:249`, `.../live_session.rs:268`, `.../use_cases/session_lifecycle.rs:111`.

Add `tokens: None,` to every `RunningTask { … }` literal: `crates/forge-entities/src/background.rs` (the test helper `running`), and the `forge_domain::RunningTask { info, seen_at }` in `crates/forge-application/src/live_session_actor.rs:377` (write `tokens: None` there for now; Task 4 changes it).

Run: `cargo check --workspace --all-targets 2>&1 | tail -5`
Expected: no errors.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-domain 2>&1 | tail -5`
Expected: all pass, including the seven new tests.

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat: FinishedTask, TaskStatus and a loose parser for the SDK's task notification (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The loader reads notifications

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (`push_user_message` at about line 291, `apply_user_line` at about 319; tests next to `load_session_skips_harness_injected_meta_lines`)

**Interfaces:**
- Consumes: `forge_domain::{parse_task_notification, record_finished}` (Task 1).
- Produces: `load_session` fills `Session.finished_tasks` and makes no message from a notification line.

- [ ] **Step 1: Write the failing tests** (in `mod tests`, after `load_session_skips_harness_injected_meta_lines`)

```rust
    const NOTE_DONE: &str = "<task-notification>\n<task-id>wm0ok3aqh</task-id>\n<status>completed</status>\n<summary>Dynamic workflow \"x\" completed</summary>\n<usage><subagent_tokens>157385</subagent_tokens><duration_ms>77189</duration_ms></usage>\n</task-notification>";
    const NOTE_FAILED: &str = "<task-notification>\n<task-id>w5r3hmkem</task-id>\n<status>failed</status>\n<summary>Dynamic workflow \"y\" failed: Error: boom</summary>\n</task-notification>";

    fn note_line(text: &str, kind: Option<&str>) -> Value {
        let mut line = json!({"type": "user", "isSidechain": false, "message": {"role": "user", "content": text}});
        if let Some(kind) = kind {
            line["origin"] = json!({"kind": kind});
        }
        line
    }

    #[tokio::test]
    async fn load_session_turns_task_notifications_into_finished_tasks_and_no_message() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[
                user_text("run it"),
                note_line(NOTE_DONE, Some("task-notification")),
                note_line(NOTE_FAILED, Some("task-notification")),
                note_line(NOTE_DONE, Some("task-notification")),
                assistant_block(json!({"type": "text", "text": "done"})),
            ],
        );

        let session = load(home.path(), "s1").await.unwrap();

        let texts: Vec<String> = session.messages.iter().map(|m| m.text()).collect();
        assert_eq!(texts, vec!["run it", "done"]);
        let seen: Vec<(&str, forge_domain::TaskStatus)> = session.finished_tasks.iter().map(|t| (t.task_id.as_str(), t.status)).collect();
        assert_eq!(seen, vec![("w5r3hmkem", forge_domain::TaskStatus::Failed), ("wm0ok3aqh", forge_domain::TaskStatus::Completed)], "the repeat of wm0ok3aqh moved to the end");
        assert_eq!(session.finished_tasks[1].tokens, Some(157_385));
    }

    #[tokio::test]
    async fn a_line_with_no_origin_is_still_read_as_a_notification_but_another_origin_is_not() {
        let home = tempfile::tempdir().unwrap();
        write_session(
            home.path(),
            &project(),
            "s1",
            &[note_line(NOTE_DONE, None), note_line(NOTE_FAILED, Some("human"))],
        );

        let session = load(home.path(), "s1").await.unwrap();

        assert_eq!(session.finished_tasks.len(), 1);
        assert_eq!(session.finished_tasks[0].task_id, "wm0ok3aqh");
        assert_eq!(session.messages.len(), 1, "what a person typed stays a message");
    }

    #[tokio::test]
    async fn a_workflow_with_no_notification_in_the_record_loads_with_no_finished_task() {
        let home = tempfile::tempdir().unwrap();
        write_session(home.path(), &project(), "s1", &[user_text("run it"), assistant_block(json!({"type": "text", "text": "started"}))]);
        let session = load(home.path(), "s1").await.unwrap();
        assert!(session.finished_tasks.is_empty());
        assert_eq!(session.messages.len(), 2);
    }
```

(The third notification line repeats `wm0ok3aqh` on purpose: the repeat must replace the first entry and move it to the end.)

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-adapter-claude-fs load_session_turns_task load_session_ a_line_with_no_origin a_workflow_with_no 2>&1 | tail -15`
Expected: FAIL (the notification text is still a user message).

- [ ] **Step 3: Implement**

`apply_user_line` passes the line's origin down. Change the two `push_user_message(session, …)` calls in it to `push_user_text(session, line, …)`, and add:

```rust
// A user line the SDK wrote for a task that ended is not something the person said: it goes into
// the finished list. A line that says another origin (a person typed the block) stays a message.
fn push_user_text(session: &mut Session, line: &Value, text: String) {
    let origin = line.pointer("/origin/kind").and_then(Value::as_str);
    if origin.is_none() || origin == Some("task-notification") {
        if let Some(task) = forge_domain::parse_task_notification(&text) {
            forge_domain::record_finished(&mut session.finished_tasks, task);
            return;
        }
    }
    push_user_message(session, text);
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-adapter-claude-fs 2>&1 | tail -5`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: a session record's task notifications become finished tasks, not user bubbles (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sidecar events and a probe

**Files:**
- Modify: `sidecar/src/protocol.ts:89`, `sidecar/src/session.ts:458-470`, `sidecar/test/session.test.ts` (after the `forwards a finished task` test)
- Create: `sidecar/scripts/print_task_events.mjs`

**Interfaces:**
- Produces on the wire: `{"type":"task_progress","task_id":string,"total_tokens":number}`; `{"type":"task_finished","task_id","status","summary","tokens"?:number,"duration_ms"?:number}`.

- [ ] **Step 1: Write the failing tests** (in `sidecar/test/session.test.ts`)

```ts
  it("forwards a running task's token count and drops progress without one", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        { type: "system", subtype: "task_progress", task_id: "w1", description: "d", usage: { total_tokens: 99100, tool_uses: 1, duration_ms: 5000 } },
        { type: "system", subtype: "task_progress", task_id: "w2", description: "d" },
        { type: "system", subtype: "task_progress", description: "no id", usage: { total_tokens: 1, tool_uses: 0, duration_ms: 0 } },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "task_progress")).toEqual([{ type: "task_progress", task_id: "w1", total_tokens: 99100 }]);
  });

  it("forwards a finished task's tokens and time when the SDK reports them", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        {
          type: "system",
          subtype: "task_notification",
          task_id: "w1",
          status: "failed",
          summary: "boom",
          output_file: "/x",
          usage: { total_tokens: 157469, tool_uses: 3, duration_ms: 70914 },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "task_finished")).toEqual([
      { type: "task_finished", task_id: "w1", status: "failed", summary: "boom", tokens: 157469, duration_ms: 70914 },
    ]);
  });
```

- [ ] **Step 2: Run to see them fail**

Run: `cd sidecar && npx vitest run test/session.test.ts -t "token" 2>&1 | tail -15`
Expected: FAIL.

- [ ] **Step 3: Implement**

`sidecar/src/protocol.ts`, replace the `task_finished` line and add `task_progress`:

```ts
  | { type: "task_progress"; task_id: string; total_tokens: number }
  | {
      type: "task_finished";
      task_id: string;
      status: "completed" | "failed" | "stopped";
      summary: string;
      // What the SDK's notification reported; a Bash command reports neither.
      tokens?: number;
      duration_ms?: number;
    }
```

`sidecar/src/session.ts`, replace the `task_notification` branch and add the `task_progress` branch before it:

```ts
        } else if (message.subtype === "task_progress") {
          const total = message.usage?.total_tokens;
          if (typeof message.task_id === "string" && message.task_id !== "" && typeof total === "number") {
            this.emit({ type: "task_progress", task_id: message.task_id, total_tokens: total });
          }
        } else if (message.subtype === "task_notification") {
          const status = message.status;
          if (typeof message.task_id === "string" && (status === "completed" || status === "failed" || status === "stopped")) {
            const usage = message.usage;
            this.emit({
              type: "task_finished",
              task_id: message.task_id,
              status,
              summary: typeof message.summary === "string" ? message.summary : "",
              ...(typeof usage?.total_tokens === "number" ? { tokens: usage.total_tokens } : {}),
              ...(typeof usage?.duration_ms === "number" ? { duration_ms: usage.duration_ms } : {}),
            });
          }
        }
```

- [ ] **Step 4: Run the sidecar checks**

Run: `cd sidecar && npx vitest run 2>&1 | tail -6 && npx tsc --noEmit && echo typecheck-ok`
Expected: all tests pass, `typecheck-ok`.

- [ ] **Step 5: Write the probe script** (`sidecar/scripts/print_task_events.mjs`, same style as `check_output_style.mjs`)

```js
// Prints the task_* and background_tasks_changed messages the real SDK sends while a small
// workflow runs, to see whether `task_progress` comes for a workflow and what the notification's
// `usage` holds. It sends ONE message to the model with the person's account: ask first.
//   node scripts/print_task_events.mjs /private/tmp/forge-observe
import { query } from "@anthropic-ai/claude-agent-sdk";

const cwd = process.argv[2];
if (!cwd) {
  console.error("usage: node scripts/print_task_events.mjs <throwaway folder>");
  process.exit(2);
}
const controller = new AbortController();
async function* prompt() {
  yield {
    type: "user",
    message: {
      role: "user",
      content:
        "Run a dynamic workflow named probe with the Workflow tool: phase Analyze, two parallel agents, one answers 'one' and one answers 'two' (no tools). Then say 'launched' and stop.",
    },
    parent_tool_use_id: null,
  };
  await new Promise((resolve) => controller.signal.addEventListener("abort", resolve));
}
const handle = query({
  prompt: prompt(),
  options: {
    cwd,
    abortController: controller,
    settingSources: [],
    settings: { enableWorkflows: true },
    permissionMode: "bypassPermissions",
    allowDangerouslySkipPermissions: true,
  },
});
const timer = setTimeout(() => controller.abort(), 240000);
try {
  for await (const message of handle) {
    if (message.type !== "system") continue;
    if (String(message.subtype).startsWith("task_") || message.subtype === "background_tasks_changed") {
      console.log(JSON.stringify(message));
    }
    if (message.subtype === "task_notification") {
      controller.abort();
    }
  }
} catch (error) {
  if (!controller.signal.aborted) console.error(String(error));
} finally {
  clearTimeout(timer);
}
```

- [ ] **Step 6: Run the probe (ask the user first; it sends one message with their account)**

Run: `cd sidecar && node scripts/print_task_events.mjs /private/tmp/forge-observe 2>&1 | cut -c1-400`
Expected: lines of `task_started`, `task_progress` (with `usage.total_tokens`), `background_tasks_changed`, and a `task_notification` with `usage`.
Write down: (a) does a `task_progress` come for the workflow task (`task_type` `local_workflow`)? (b) is `usage.total_tokens` of the notification the same as `<subagent_tokens>` in the record (compare with the new session file under `~/.claude/projects/-private-tmp-forge-observe/`)? Add the answer as a short paragraph under "Order of work" in the spec. **If (a) is no:** stop and tell the user; the spec's fallback (sum of the agents' last calls) becomes an added task. Do not build it unasked.

- [ ] **Step 7: Commit**

```bash
git add sidecar/src sidecar/test sidecar/scripts docs/superpowers/specs
git commit -m "feat: the sidecar forwards task progress and a finished task's tokens and time (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Ports and the live session

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs:71-73` and its test near line 248
- Modify: `crates/forge-application/src/event.rs` (`SessionUpdate`, `apply_sidecar_event`, the test near line 566)
- Modify: `crates/forge-application/src/live_session_actor.rs` (struct, `BackgroundTasks` arm at about line 370, the no-op arm at about line 414, the clear at about line 441)
- Test: `crates/forge-application/src/live_session.rs` (tests near line 1000)

**Interfaces:**
- Consumes: `FinishedTask`, `TaskStatus`, `record_finished`, `RunningTask.tokens` (Task 1).
- Produces: `SidecarEvent::TaskProgress { task_id, total_tokens }`; `SidecarEvent::TaskFinished { task_id, status, summary, tokens: Option<u64>, duration_ms: Option<u64> }`; `SessionUpdate::TaskProgress { task_id: String, total_tokens: u64 }`; a live view whose `session.finished_tasks` has tokens and time filled and whose `background_tasks[i].tokens` is set.

- [ ] **Step 1: Write the failing tests**

In `sidecar_port.rs` replace the `finished` part of the existing test with:

```rust
        let finished: SidecarEvent =
            serde_json::from_str(r#"{"type":"task_finished","task_id":"b1","status":"stopped","summary":"s"}"#).unwrap();
        assert_eq!(
            finished,
            SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "stopped".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None }
        );
        let timed: SidecarEvent =
            serde_json::from_str(r#"{"type":"task_finished","task_id":"w1","status":"failed","summary":"x","tokens":157469,"duration_ms":70914}"#).unwrap();
        assert_eq!(
            timed,
            SidecarEvent::TaskFinished { task_id: "w1".to_string(), status: "failed".to_string(), summary: "x".to_string(), tokens: Some(157_469), duration_ms: Some(70_914) }
        );
        let progress: SidecarEvent = serde_json::from_str(r#"{"type":"task_progress","task_id":"w1","total_tokens":99100}"#).unwrap();
        assert_eq!(progress, SidecarEvent::TaskProgress { task_id: "w1".to_string(), total_tokens: 99_100 });
```

In `event.rs` tests, change the `TaskFinished` call in `background_task_task_finished_and_suggestion_events_become_updates` to the five-field form (`tokens: None, duration_ms: None`) and add:

```rust
    #[test]
    fn a_finished_task_is_recorded_in_the_session_and_progress_becomes_an_update() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::TaskFinished { task_id: "w1".to_string(), status: "failed".to_string(), summary: "boom".to_string(), tokens: Some(5), duration_ms: Some(9) },
        );
        assert_eq!(update, SessionUpdate::TaskFinished { task_id: "w1".to_string() });
        assert_eq!(
            session.finished_tasks,
            vec![forge_domain::FinishedTask { task_id: "w1".to_string(), status: forge_domain::TaskStatus::Failed, summary: "boom".to_string(), tokens: Some(5), duration_ms: Some(9) }]
        );
        apply_sidecar_event(&mut session, &SidecarEvent::TaskFinished { task_id: "w2".to_string(), status: "weird".to_string(), summary: String::new(), tokens: None, duration_ms: None });
        assert_eq!(session.finished_tasks.len(), 1, "a status we do not know is not recorded");
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::TaskProgress { task_id: "w1".to_string(), total_tokens: 7 }),
            SessionUpdate::TaskProgress { task_id: "w1".to_string(), total_tokens: 7 }
        );
    }
```

In `live_session.rs` tests (next to `the_task_set_is_replaced_and_a_staying_task_keeps_its_first_seen_time`):

```rust
    #[tokio::test]
    async fn a_running_tasks_tokens_survive_a_new_task_set() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 1234 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(1234)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1"), task("b2")] });
        let second = wait_for(&mut view, "two tasks", |v| v.background_tasks.len() == 2).await;
        assert_eq!(second.background_tasks[0].tokens, Some(1234));
        assert_eq!(second.background_tasks[1].tokens, None);
    }

    #[tokio::test]
    async fn a_finished_task_gets_the_tokens_and_time_its_notification_left_out() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 500 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(500)).await;
        control.push(SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None });
        let done = wait_for(&mut view, "finished", |v| v.session.finished_tasks.len() == 1 && v.session.finished_tasks[0].tokens.is_some()).await;
        assert_eq!(done.session.finished_tasks[0].tokens, Some(500));
        assert!(done.session.finished_tasks[0].duration_ms.is_some());
    }

    #[tokio::test]
    async fn the_task_set_may_drop_a_task_before_its_notification_arrives() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 500 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(500)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![] });
        wait_for(&mut view, "no tasks", |v| v.background_tasks.is_empty()).await;
        control.push(SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None });
        let done = wait_for(&mut view, "finished", |v| v.session.finished_tasks.len() == 1 && v.session.finished_tasks[0].tokens.is_some()).await;
        assert_eq!(done.session.finished_tasks[0].tokens, Some(500));
    }
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-ports -p forge-application 2>&1 | tail -15`
Expected: FAIL to compile.

- [ ] **Step 3: Implement the port**

Replace the `TaskFinished` variant in `sidecar_port.rs` and add `TaskProgress`:

```rust
    /// A background task ended. The set still comes from `BackgroundTasks`; this says how it ended.
    /// `tokens` and `duration_ms` are what the SDK reported (a Bash command reports neither).
    TaskFinished {
        task_id: String,
        status: String,
        summary: String,
        #[serde(default)]
        tokens: Option<u64>,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    /// How many tokens a running background task has used so far.
    TaskProgress { task_id: String, total_tokens: u64 },
```

- [ ] **Step 4: Implement the events** (`event.rs`)

Add to `SessionUpdate`: `TaskProgress { task_id: String, total_tokens: u64 },`. Replace the `TaskFinished` arm of `apply_sidecar_event` and add the progress arm:

```rust
        SidecarEvent::TaskFinished { task_id, status, summary, tokens, duration_ms } => {
            if let Some(status) = forge_domain::TaskStatus::parse(status) {
                forge_domain::record_finished(
                    &mut session.finished_tasks,
                    forge_domain::FinishedTask { task_id: task_id.clone(), status, summary: summary.clone(), tokens: *tokens, duration_ms: *duration_ms },
                );
            }
            SessionUpdate::TaskFinished { task_id: task_id.clone() }
        }
        SidecarEvent::TaskProgress { task_id, total_tokens } => SessionUpdate::TaskProgress { task_id: task_id.clone(), total_tokens: *total_tokens },
```

- [ ] **Step 5: Implement the actor** (`live_session_actor.rs`)

Add a field `vanished: std::collections::HashMap<String, forge_domain::RunningTask>` to the actor struct (find where `refusal_shown` is declared and initialised and add it next to both; initialise with `Default::default()`). Add the helper next to `clear_refusal`:

```rust
    /// What a finished task's notification left out (its tokens, its time) is taken from what the
    /// view saw of it while it ran. False when no finished entry exists yet.
    fn fill_finished(&mut self, task: &forge_domain::RunningTask) -> bool {
        let Some(entry) = self.view.session.finished_tasks.iter_mut().find(|item| item.task_id == task.info.id) else { return false };
        if entry.tokens.is_none() {
            entry.tokens = task.tokens;
        }
        if entry.duration_ms.is_none() {
            entry.duration_ms = task.seen_at.elapsed().ok().map(|elapsed| elapsed.as_millis() as u64);
        }
        true
    }
```

Replace the `BackgroundTasks` arm and add the two arms:

```rust
            SessionUpdate::BackgroundTasks(tasks) => {
                let now = std::time::SystemTime::now();
                let previous = std::mem::take(&mut self.view.background_tasks);
                self.view.background_tasks = tasks
                    .into_iter()
                    .map(|info| {
                        let before = previous.iter().find(|task| task.info.id == info.id);
                        forge_domain::RunningTask { seen_at: before.map_or(now, |task| task.seen_at), tokens: before.and_then(|task| task.tokens), info }
                    })
                    .collect();
                for gone in previous.into_iter().filter(|old| !self.view.background_tasks.iter().any(|task| task.info.id == old.info.id)) {
                    if !self.fill_finished(&gone) {
                        self.vanished.insert(gone.info.id.clone(), gone);
                    }
                }
            }
            SessionUpdate::TaskProgress { task_id, total_tokens } => {
                if let Some(task) = self.view.background_tasks.iter_mut().find(|task| task.info.id == task_id) {
                    task.tokens = Some(total_tokens);
                }
            }
            SessionUpdate::TaskFinished { task_id } => {
                let seen = self.view.background_tasks.iter().find(|task| task.info.id == task_id).cloned().or_else(|| self.vanished.remove(&task_id));
                if let Some(task) = seen {
                    self.fill_finished(&task);
                }
            }
```

Remove `| SessionUpdate::TaskFinished { .. }` from the no-op arm. Where `self.view.background_tasks.clear();` runs (the sidecar gone, about line 441) add `self.vanished.clear();`.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-ports -p forge-application 2>&1 | tail -6`
Expected: PASS. Run `cargo check --workspace --all-targets 2>&1 | tail -3`; fix any remaining `TaskFinished { … }` literal the compiler names (add `tokens: None, duration_ms: None`).

- [ ] **Step 7: Commit**

```bash
git add -A crates
git commit -m "feat: finished tasks and running tokens reach the live session (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Cards, journal state and labels (`forge-entities`)

**Files:**
- Modify: `crates/forge-entities/src/workflow_progress.rs`, `crates/forge-entities/src/background.rs`, `crates/forge-entities/src/lib.rs`
- Tests: in the same files' `mod tests`

**Interfaces:**
- Consumes: `FinishedTask`, `TaskStatus`, `RunningTask.tokens` (Task 1).
- Produces:
  - `AgentState::Failed`; `Step::failed(&self) -> bool`; `pub fn dot_class(state: AgentState, run_stopped: bool) -> &'static str`.
  - `pub struct TaskEnd { pub status: TaskStatus, pub summary: String, pub tokens: Option<u64>, pub duration_ms: Option<u64> }`.
  - `TaskCard` gains `pub tokens: Option<u64>` and `pub end: Option<TaskEnd>`.
  - `pub struct TaskLists { pub running: Vec<TaskCard>, pub finished: Vec<TaskCard> }`; `pub fn task_lists(messages: &[Message], running: &[RunningTask], finished: &[FinishedTask]) -> TaskLists` (finished newest first; a task in both appears only as finished).
  - `pub fn error_text(summary: &str) -> String`; `pub fn end_label(status: TaskStatus) -> String`.
  - `task_cards` keeps its signature (running cards only).

- [ ] **Step 1: Write the failing journal tests** (`workflow_progress.rs`, `mod tests`)

```rust
    #[test]
    fn a_failed_line_marks_the_agent_failed_and_a_later_result_wins() {
        let text = concat!(
            "{\"type\": \"started\", \"agentId\": \"a1\", \"label\": \"a\", \"phase\": \"Analyze\"}\n",
            "{\"type\": \"started\", \"agentId\": \"a2\", \"label\": \"b\", \"phase\": \"Analyze\"}\n",
            "{\"type\": \"started\", \"agentId\": \"a3\", \"label\": \"c\", \"phase\": \"Analyze\"}\n",
            "{\"type\": \"failed\", \"key\": \"k2\", \"agentId\": \"a2\"}\n",
            "{\"type\": \"failed\", \"agentId\": \"a3\"}\n",
            "{\"type\": \"result\", \"agentId\": \"a3\", \"result\": \"ok\"}\n",
            "{\"type\": \"failed\", \"agentId\": \"a1\"}\n",
            "{\"type\": \"result\", \"agentId\": \"a1\", \"result\": \"ok\"}\n",
            "{\"type\": \"failed\", \"agentId\": \"a1\"}\n",
        );
        let agents = parse_journal(text);
        let states: Vec<AgentState> = agents.iter().map(|agent| agent.state).collect();
        assert_eq!(states, vec![AgentState::Done, AgentState::Failed, AgentState::Done], "a failure after a result does not undo it");
        let listed = steps(&["Analyze".to_string()], &agents);
        assert!(listed[0].failed());
        assert_eq!(listed[0].done(), 2, "a failed agent is not counted as done");
    }

    #[test]
    fn dots_are_blue_grey_red_or_hollow() {
        assert_eq!(dot_class(AgentState::Running, false), "wf-dot running");
        assert_eq!(dot_class(AgentState::Done, false), "wf-dot done");
        assert_eq!(dot_class(AgentState::Failed, false), "wf-dot failed");
        assert_eq!(dot_class(AgentState::Running, true), "wf-dot stopped", "an agent that never ended in a stopped run");
        assert_eq!(dot_class(AgentState::Done, true), "wf-dot done");
    }
```

- [ ] **Step 2: Implement the journal state**

In `workflow_progress.rs`: add `Failed` to `AgentState`; add the arm to `parse_journal`'s `match field("type")` (the `Some("result")` arm already sets `Done`):

```rust
            Some("failed") => {
                if let Some(agent) = agents.iter_mut().find(|agent| agent.id == id) {
                    if agent.state != AgentState::Done {
                        agent.state = AgentState::Failed;
                    }
                }
            }
```

and below `Step::done`:

```rust
    pub fn failed(&self) -> bool {
        self.agents.iter().any(|agent| agent.state == AgentState::Failed)
    }
```

and, after `steps`:

```rust
/// The CSS class of an agent's dot: blue running, grey done, red failed. In a run that was stopped an
/// agent that never ended shows hollow.
pub fn dot_class(state: AgentState, run_stopped: bool) -> &'static str {
    match (state, run_stopped) {
        (AgentState::Done, _) => "wf-dot done",
        (AgentState::Failed, _) => "wf-dot failed",
        (AgentState::Running, true) => "wf-dot stopped",
        (AgentState::Running, false) => "wf-dot running",
    }
}
```

Export `dot_class` next to `AgentState` in `crates/forge-entities/src/lib.rs`. Run `cargo test -p forge-entities workflow_progress`; expected PASS. (Existing matches on `AgentState` elsewhere break the build; Task 7 fixes the widgets. If `cargo check -p forge-widgets` complains now, leave it: only `forge-entities` must build here.)

- [ ] **Step 3: Write the failing card tests** (`crates/forge-entities/src/background.rs`, `mod tests`; reuse the helpers `running`, `bash`, `workflow_call`, `SENTENCE`, `WORKFLOW_RESULT`)

```rust
    fn finished(id: &str, status: TaskStatus, summary: &str) -> forge_domain::FinishedTask {
        forge_domain::FinishedTask { task_id: id.to_string(), status, summary: summary.to_string(), tokens: Some(157_469), duration_ms: Some(70_914) }
    }

    #[test]
    fn finished_cards_come_newest_first_with_their_end() {
        let lists = task_lists(
            &[workflow_call(WORKFLOW_RESULT)],
            &[],
            &[finished("b1", TaskStatus::Completed, "Background command \"x\" completed"), finished("wf1", TaskStatus::Failed, "Dynamic workflow \"risk-review\" failed: Error: boom")],
        );
        assert!(lists.running.is_empty());
        let ids: Vec<&str> = lists.finished.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, vec!["wf1", "b1"]);
        let end = lists.finished[0].end.as_ref().unwrap();
        assert_eq!((end.status, end.tokens, end.duration_ms), (TaskStatus::Failed, Some(157_469), Some(70_914)));
        assert_eq!(lists.finished[0].workflow.as_ref().unwrap().name, "risk-review", "the Workflow call gives the name");
        assert_eq!(lists.finished[1].description, "Background command \"x\" completed", "no Bash call: the summary stands in");
    }

    #[test]
    fn a_task_in_both_lists_shows_once_as_finished() {
        let lists = task_lists(&[], &[running("b1", 3), running("b2", 3)], &[finished("b1", TaskStatus::Completed, "s")]);
        assert_eq!(lists.running.iter().map(|card| card.id.as_str()).collect::<Vec<_>>(), vec!["b2"]);
        assert_eq!(lists.finished.len(), 1);
    }

    #[test]
    fn a_finished_bash_card_still_names_its_command_and_description() {
        let messages = vec![bash("sleep 12; echo done", Some("Sleep 12 seconds then print done"), &SENTENCE.replace("bplyt9cwq", "bg9"))];
        let lists = task_lists(&messages, &[], &[finished("bg9", TaskStatus::Completed, "Background command completed (exit code 0)")]);
        assert_eq!(lists.finished[0].description, "Sleep 12 seconds then print done");
        assert_eq!(lists.finished[0].command.as_deref(), Some("sleep 12; echo done"));
    }

    #[test]
    fn a_running_card_carries_the_tokens_the_view_saw() {
        let mut task = running("b1", 0);
        task.tokens = Some(99_100);
        assert_eq!(task_cards(&[], &[task])[0].tokens, Some(99_100));
    }

    #[test]
    fn a_running_card_and_its_finished_card_have_different_keys() {
        let before = task_cards(&[], &[running("b1", 0)]);
        let after = task_lists(&[], &[], &[finished("b1", TaskStatus::Stopped, "s")]);
        assert_ne!(card_key(&before[0]), card_key(&after.finished[0]));
    }

    #[test]
    fn the_error_box_shows_what_follows_the_failed_marker() {
        assert_eq!(
            error_text("Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)"),
            "Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)"
        );
        assert_eq!(error_text("Background command \"x\" failed with exit code 1"), "Background command \"x\" failed with exit code 1");
        assert_eq!(error_text(""), "");
    }

    #[test]
    fn each_end_has_a_label() {
        assert_eq!(end_label(TaskStatus::Completed), "Completed");
        assert_eq!(end_label(TaskStatus::Stopped), "Stopped");
        assert_eq!(end_label(TaskStatus::Failed), "Failed");
    }
```

- [ ] **Step 4: Run to see them fail**

Run: `cargo test -p forge-entities background 2>&1 | tail -10`
Expected: FAIL to compile.

- [ ] **Step 5: Implement** (`background.rs`; replace `TaskCard`, split `task_cards`)

Change the imports at the top to `use forge_domain::{BackgroundTaskInfo, FinishedTask, Message, RunningTask, TaskStatus};` and add `use std::time::SystemTime;` is already there. Replace the struct `TaskCard` and the function `task_cards` and `card_key` with:

```rust
/// What a finished task's card knows about its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEnd {
    pub status: TaskStatus,
    pub summary: String,
    pub tokens: Option<u64>,
    pub duration_ms: Option<u64>,
}

/// One card of the Tasks panel.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskCard {
    pub id: String,
    pub description: String,
    pub task_type: String,
    /// The command of the Bash call that started it, when that call is in the session.
    pub command: Option<String>,
    /// The file the task writes its output to, only when `valid_output_path` holds.
    pub output_path: Option<String>,
    pub seen_at: SystemTime,
    /// Set for a workflow, when the Workflow call that started it is in the session.
    pub workflow: Option<WorkflowCard>,
    /// The tokens the SDK says a running task has used.
    pub tokens: Option<u64>,
    /// Set once the task has ended.
    pub end: Option<TaskEnd>,
}

/// The running cards and the finished ones of a session.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskLists {
    pub running: Vec<TaskCard>,
    /// Newest first.
    pub finished: Vec<TaskCard>,
}

struct Calls<'a> {
    started: Vec<(BackgroundRef, &'a forge_domain::ToolCall)>,
    workflows: Vec<(String, WorkflowCard, Option<String>)>,
}

fn calls(messages: &[Message]) -> Calls<'_> {
    let started = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter_map(|call| {
            let reference = background_ref(&call.result.as_ref()?.output)?;
            Some((reference, call))
        })
        .collect();
    let workflows = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter(|call| call.name == "Workflow")
        .filter_map(|call| {
            let reference = workflow_ref(&call.result.as_ref()?.output)?;
            let meta = workflow_meta(call.input.get("script")?.as_str()?)?;
            let card = WorkflowCard { name: meta.name, phases: meta.phases, journal_path: journal_path(&reference.transcript_dir) };
            Some((reference.task_id, card, meta.description))
        })
        .collect();
    Calls { started, workflows }
}

fn build_card(calls: &Calls<'_>, info: &BackgroundTaskInfo, seen_at: SystemTime, tokens: Option<u64>, end: Option<TaskEnd>) -> TaskCard {
    let workflow = calls.workflows.iter().find(|(task_id, _, _)| task_id == &info.id);
    let call = calls.started.iter().find(|(reference, _)| reference.task_id == info.id);
    let command = call.and_then(|(_, call)| call.input.get("command")).and_then(|value| value.as_str()).map(str::to_string);
    let description = call
        .and_then(|(_, call)| call.input.get("description"))
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| info.description.clone());
    let output_path = call.map(|(reference, _)| reference.output_path.clone()).filter(|path| valid_output_path(path, &info.id));
    let description = workflow.and_then(|(_, _, text)| text.clone()).unwrap_or(description);
    let workflow = workflow.map(|(_, card, _)| card.clone());
    TaskCard { id: info.id.clone(), description, task_type: info.task_type.clone(), command, output_path, seen_at, workflow, tokens, end }
}

/// A card for each running task, in the SDK's order. The command, a better description and the
/// output file come from the Bash call whose result names the task; a task without such a call
/// (a sub-agent) keeps the SDK's description and has no command and no output.
pub fn task_cards(messages: &[Message], running: &[RunningTask]) -> Vec<TaskCard> {
    let calls = calls(messages);
    running.iter().map(|task| build_card(&calls, &task.info, task.seen_at, task.tokens, None)).collect()
}

/// The running cards and the finished ones. A task that is in both (its notification came before the
/// set lost it) is a finished card only. A finished task with no call in the session has the
/// notification's summary for its description.
pub fn task_lists(messages: &[Message], running: &[RunningTask], finished: &[FinishedTask]) -> TaskLists {
    let calls = calls(messages);
    let ended = |id: &str| finished.iter().any(|item| item.task_id == id);
    let running = running.iter().filter(|task| !ended(&task.info.id)).map(|task| build_card(&calls, &task.info, task.seen_at, task.tokens, None)).collect();
    let finished = finished
        .iter()
        .rev()
        .map(|item| {
            let info = BackgroundTaskInfo { id: item.task_id.clone(), description: item.summary.clone(), task_type: String::new() };
            let end = TaskEnd { status: item.status, summary: item.summary.clone(), tokens: item.tokens, duration_ms: item.duration_ms };
            build_card(&calls, &info, SystemTime::UNIX_EPOCH, item.tokens, Some(end))
        })
        .collect();
    TaskLists { running, finished }
}

/// The text of a failed task's error box: what follows `failed: ` in the summary, else the summary.
pub fn error_text(summary: &str) -> String {
    match summary.find(" failed: ") {
        Some(at) => summary[at + " failed: ".len()..].to_string(),
        None => summary.to_string(),
    }
}

pub fn end_label(status: TaskStatus) -> String {
    match status {
        TaskStatus::Completed => tr!("Completed"),
        TaskStatus::Stopped => tr!("Stopped"),
        TaskStatus::Failed => tr!("Failed"),
    }
}

/// The key of a card in the panel. It changes when the card's output file becomes known: the panel
/// starts reading the output when a card is created, so a card made before the Bash result named
/// the file must be replaced by a new one. The same for a workflow's card and its journal, and for a
/// running card that has ended.
pub fn card_key(card: &TaskCard) -> String {
    let end = card.end.as_ref().map_or("running", |end| end.status.as_str());
    format!("{}:{}:{}:{}", card.id, card.output_path.is_some(), card.workflow.is_some(), end)
}
```

Keep `pill_label` and the rest. Export in `crates/forge-entities/src/lib.rs`: add `end_label, error_text, task_lists, TaskEnd, TaskLists` to the `pub use background::{…}` line.

- [ ] **Step 6: Run the tests**

Run: `cargo test -p forge-entities 2>&1 | tail -6`
Expected: PASS (the old `card_key` test still passes: it checks `starts_with("b1:")`).

- [ ] **Step 7: Commit**

```bash
git add -A crates/forge-entities
git commit -m "feat: finished and running task lists, failed agents and end labels (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Per-session fold and cleared state (`forge-features`)

**Files:**
- Create: `crates/forge-features/src/task_list_views.rs`
- Modify: `crates/forge-features/src/lib.rs` (add `mod task_list_views;` next to `mod transcript_views;`), `crates/forge-features/src/workspace.rs` (field at about line 111, init at 167, methods near `transcript_view` at 560/943, forget in `close` at about 1343)

**Interfaces:**
- Produces:
  - `pub struct TaskListView { pub finished_open: bool, pub cleared: HashSet<String> }` (Default: closed, nothing cleared).
  - `Workspace::task_list_view(&self, id: LiveSessionId) -> TaskListView`
  - `Workspace::toggle_finished_tasks(&self, id: LiveSessionId)`
  - `Workspace::clear_finished_tasks(&self, id: LiveSessionId, task_ids: Vec<String>)`
  - closing the session forgets its state.

- [ ] **Step 1: Write the failing tests** (in the new file)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_starts_closed_with_nothing_cleared() {
        let views = TaskListViews::default();
        let view = views.get(LiveSessionId(1));
        assert!(!view.finished_open);
        assert!(view.cleared.is_empty());
    }

    #[test]
    fn the_fold_toggles_per_session() {
        let mut views = TaskListViews::default();
        views.toggle_finished(LiveSessionId(1));
        assert!(views.get(LiveSessionId(1)).finished_open);
        assert!(!views.get(LiveSessionId(2)).finished_open);
        views.toggle_finished(LiveSessionId(1));
        assert!(!views.get(LiveSessionId(1)).finished_open);
    }

    #[test]
    fn clearing_hides_only_the_ids_given_and_adds_up() {
        let mut views = TaskListViews::default();
        views.clear(LiveSessionId(1), vec!["a".to_string()]);
        views.clear(LiveSessionId(1), vec!["b".to_string()]);
        let cleared = views.get(LiveSessionId(1)).cleared;
        assert!(cleared.contains("a") && cleared.contains("b") && !cleared.contains("c"));
        assert!(views.get(LiveSessionId(2)).cleared.is_empty());
    }

    #[test]
    fn a_closed_session_is_forgotten() {
        let mut views = TaskListViews::default();
        views.toggle_finished(LiveSessionId(1));
        views.clear(LiveSessionId(1), vec!["a".to_string()]);
        views.forget(LiveSessionId(1));
        assert_eq!(views.get(LiveSessionId(1)), TaskListView::default());
    }
}
```

- [ ] **Step 2: Implement** (top of the new file)

```rust
use std::collections::{HashMap, HashSet};

use forge_application::LiveSessionId;

/// What the Background tasks panel keeps for one session: whether its `Finished` fold is open and
/// which finished cards the person cleared with the trash icon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskListView {
    pub finished_open: bool,
    pub cleared: HashSet<String>,
}

/// The task list state of each open session. In memory only: nothing is deleted from the session's
/// record, so after a restart the finished cards are back.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskListViews {
    by_session: HashMap<LiveSessionId, TaskListView>,
}

impl TaskListViews {
    pub fn get(&self, id: LiveSessionId) -> TaskListView {
        self.by_session.get(&id).cloned().unwrap_or_default()
    }

    pub fn toggle_finished(&mut self, id: LiveSessionId) {
        let view = self.by_session.entry(id).or_default();
        view.finished_open = !view.finished_open;
    }

    /// Hides these finished tasks. A task that ends later is not in the set and shows.
    pub fn clear(&mut self, id: LiveSessionId, task_ids: Vec<String>) {
        self.by_session.entry(id).or_default().cleared.extend(task_ids);
    }

    /// Drops what is kept for a session that was closed.
    pub fn forget(&mut self, id: LiveSessionId) {
        self.by_session.remove(&id);
    }
}
```

- [ ] **Step 3: Wire it into the workspace** (`workspace.rs`)

Add `use crate::task_list_views::{TaskListView, TaskListViews};`; field `task_list_views: Signal<TaskListViews>,` next to `transcript_views`; init `task_list_views: Signal::new(TaskListViews::default()),`; methods next to `transcript_view`:

```rust
    /// The Background tasks panel's state for the open session.
    pub fn task_list_view(&self, id: LiveSessionId) -> TaskListView {
        self.task_list_views.read().get(id)
    }

    pub fn toggle_finished_tasks(&self, id: LiveSessionId) {
        let mut views = self.task_list_views;
        views.write().toggle_finished(id);
    }

    /// Hides the finished cards with these ids (the trash icon).
    pub fn clear_finished_tasks(&self, id: LiveSessionId, task_ids: Vec<String>) {
        let mut views = self.task_list_views;
        views.write().clear(id, task_ids);
    }
```

In `close`, after `views.write().forget(id);` of the transcript views add:

```rust
        let mut task_lists = self.task_list_views;
        task_lists.write().forget(id);
```

Re-export `TaskListView` where `forge_features` exports its public types (find the `pub use` list in `lib.rs` containing `Panel` and add `TaskListView`).

- [ ] **Step 4: Run**

Run: `cargo test -p forge-features task_list_views 2>&1 | tail -6 && cargo check -p forge-features 2>&1 | tail -3`
Expected: 4 tests pass; check clean.

- [ ] **Step 5: Commit**

```bash
git add -A crates/forge-features
git commit -m "feat: the tasks panel keeps its Finished fold and cleared cards per session (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Panel, chat line, styles and translations

**Files:**
- Modify: `crates/forge-widgets/src/tasks_panel.rs`, `crates/forge-widgets/src/chat_window.rs` (the `Turns` call at about line 95), `crates/forge-entities/src/turn.rs` (`Turns`, `TurnItem`, `WorkflowLine`), `crates/forge-shared/src/theme.css` (after line 796), `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `task_lists`, `TaskCard.end`, `TaskEnd`, `end_label`, `error_text`, `dot_class`, `Step::failed` (Task 5); `workspace.task_list_view / toggle_finished_tasks / clear_finished_tasks` (Task 6); `format_tokens`, `elapsed_label` (existing).
- Produces: the visible behaviour of the spec's section 4.

There is no unit test for Dioxus markup in this repo; the checks are `cargo check`, the existing tests, and the real-window run of Task 8.

- [ ] **Step 1: Translations** (`ko.rs`, alphabetical positions; skip any key that already exists, grep first: `grep -n '"Failed"\|"Completed"\|"Error"\|"Stopped"' crates/forge-shared/src/ko.rs`)

```rust
    ("Clear finished tasks", "완료된 작업 지우기"),
    ("Completed", "완료됨"),
    ("Error", "오류"),
    ("Failed", "실패"),
    ("Finished {count}", "완료 {count}"),
    ("Stopped", "중지됨"),
    ("failed", "실패"),
    ("stopped", "중지됨"),
    ("{amount} tokens", "{amount} 토큰"),
```

Run: `cargo test -p forge-shared 2>&1 | tail -5` (the table's order and uniqueness tests, if any, must pass).

- [ ] **Step 2: Styles** (`theme.css`, after the `.wf-dot.done` rule and near `.task-stop-square`)

```css
.wf-dot.failed { background: var(--danger); }
.wf-dot.stopped { background: transparent; box-shadow: inset 0 0 0 1px var(--fg-muted); }
.wf-step-name.failed { color: var(--danger); }
.end-failed, .workflow-end.failed { color: var(--danger); }
.task-total { margin-left: auto; color: var(--fg-muted); font-size: 13px; }
.task-error { padding: 10px 12px; border-radius: 8px; background: color-mix(in srgb, var(--danger) 12%, transparent); }
.task-error-title { color: var(--danger); font-size: 13px; }
.task-error pre { margin: 4px 0 0; font: inherit; font-size: 12px; white-space: pre-wrap; overflow-wrap: anywhere; }
.tasks-fold-head { appearance: none; display: flex; align-items: center; gap: 6px; padding: 0; border: none; background: none; color: var(--fg-muted); font: inherit; cursor: pointer; }
.task-card-done .task-description, .workflow-card.ended .task-description { color: var(--fg-muted); }
```

- [ ] **Step 3: The chat line** (`turn.rs`)

Add `use forge_domain::{FinishedTask, TaskStatus};` (the file already imports from `forge_domain`; merge) and `use crate::workflow_progress::dot_class;`. `Turns` gets `#[props(default)] finished: Vec<FinishedTask>,` and passes `finished: finished.clone()` into `TurnItem`, which takes `finished: Vec<FinishedTask>` and in the `Item::Workflow(item)` arm:

```rust
        Item::Workflow(item) => {
            let running = workflows.iter().find(|workflow| workflow.task_id == item.task_id).cloned();
            let ended = finished.iter().find(|task| task.task_id == item.task_id).cloned();
            rsx! { WorkflowLine { item, running, ended, on_open_tasks } }
        }
```

`WorkflowLine` takes `ended: Option<FinishedTask>`; the finished line and the dots become:

```rust
    let Some(running) = running else {
        let suffix = ended.as_ref().and_then(|task| match task.status {
            TaskStatus::Completed => None,
            TaskStatus::Stopped => Some(("workflow-end", tr!("stopped"))),
            TaskStatus::Failed => Some(("workflow-end failed", tr!("failed"))),
        });
        return rsx! {
            div { class: "workflow-done",
                {tr!("Workflow ran {name}", name = item.name)}
                if let Some((class, label)) = suffix {
                    " "
                    span { class: "{class}", "{label}" }
                }
            }
        };
    };
```

and in the running card replace the dot class expression with `class: dot_class(agent.state, false)`.

In `chat_window.rs`, where `Turns { … workflows: workflows(), … }` is built (about line 95), add `finished: view.read().session.finished_tasks.clone(),` (use the same `view` handle the surrounding code reads the messages from).

- [ ] **Step 4: The panel** (`tasks_panel.rs`)

Imports: replace the `forge_entities` line with `use forge_entities::{card_key, dot_class, elapsed_label, end_label, error_text, format_tokens, parse_journal, steps, task_lists, AgentState, Clock, RunningWorkflow, Step, TaskCard, TASK_OUTPUT_TAIL};` and add `use forge_domain::TaskStatus;`. Make sure `elapsed_label`, `format_tokens`, `dot_class`, `end_label`, `error_text`, `task_lists` are exported from `forge-entities` (`lib.rs`; add any that are not).

`use_running_workflows`: change the `task_cards(...)` call to `task_lists(&view.session.messages, &view.background_tasks, &view.session.finished_tasks).running`.

Replace `TasksPanel`:

```rust
#[component]
pub fn TasksPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let lists = {
        let view = view.read();
        task_lists(&view.session.messages, &view.background_tasks, &view.session.finished_tasks)
    };
    let state = workspace.task_list_view(id);
    let finished: Vec<TaskCard> = lists.finished.into_iter().filter(|card| !state.cleared.contains(&card.id)).collect();
    let shown_ids: Vec<String> = finished.iter().map(|card| card.id.clone()).collect();
    let nothing_to_clear = finished.is_empty();
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Tasks,
            tools: rsx! {
                button {
                    class: "icon-button",
                    r#type: "button",
                    disabled: nothing_to_clear,
                    title: tr!("Clear finished tasks"),
                    "aria-label": tr!("Clear finished tasks"),
                    onclick: move |_| workspace.clear_finished_tasks(id, shown_ids.clone()),
                    "🗑"
                }
            },
            div { class: "tasks-body",
                if !lists.running.is_empty() {
                    div { class: "tasks-heading", {tr!("Running")} }
                    for card in lists.running {
                        TaskCardView { key: "{card_key(&card)}", id, card }
                    }
                }
                if lists.running.is_empty() && finished.is_empty() {
                    div { class: "dock-note", {tr!("No running tasks")} }
                }
                if !finished.is_empty() {
                    button {
                        class: "tasks-fold-head tasks-heading",
                        r#type: "button",
                        "aria-expanded": "{state.finished_open}",
                        onclick: move |_| workspace.toggle_finished_tasks(id),
                        span { {tr!("Finished {count}", count = finished.len())} }
                        span { class: "wf-step-chevron", if state.finished_open { "⌄" } else { "›" } }
                    }
                    if state.finished_open {
                        for card in finished {
                            TaskCardView { key: "{card_key(&card)}", id, card }
                        }
                    }
                }
                div { class: "dock-note", {tr!("Tasks end when Forge closes the session.")} }
            }
        }
    }
}
```

(`lists.running` is moved in the `for` after the `is_empty` test: read `let running_empty = lists.running.is_empty();` first and use it in both conditions if the borrow checker asks.) If the repo already has a trash glyph (`grep -rn "🗑" crates`), use that one.

`TaskCardView`: at the top, after the workflow early return, a finished Bash card returns early:

```rust
    if let Some(end) = card.end.clone() {
        let class = if end.status == TaskStatus::Failed { "end-failed" } else { "" };
        return rsx! {
            div { class: "task-card task-card-done",
                div { class: "task-title",
                    div { class: "task-description", "{card.description}" }
                    div { class: "task-meta",
                        span { "Bash" }
                        span { class: "{class}", {end_label(end.status)} }
                        if let Some(ms) = end.duration_ms {
                            span { {elapsed_label(ms / 1000)} }
                        }
                    }
                }
                if end.status == TaskStatus::Failed && !end.summary.is_empty() {
                    div { class: "task-error", pre { "{error_text(&end.summary)}" } }
                }
            }
        };
    }
```

Hooks must run unconditionally in Dioxus; put this early return **before** the `use_signal`/`use_future` of the output reader (the existing function already returns early for workflows before them, so the same position is correct).

`WorkflowCardView`: (a) read the journal once when the card has ended: in the `use_future` loop add after `journal.set(text)` handling `if ended { break; }` where `let ended = card.end.is_some();` is captured before the future; (b) replace the header and meta with:

```rust
    let end = card.end.clone();
    let stopped = end.as_ref().is_some_and(|end| end.status == TaskStatus::Stopped);
    let tokens = end.as_ref().and_then(|end| end.tokens).or(card.tokens);
    // ...
        div { class: if end.is_some() { "task-card workflow-card ended" } else { "task-card workflow-card" },
            div { class: "task-head",
                div { class: "task-title",
                    div { class: "task-description", "{workflow.name}" }
                    div { class: "task-meta",
                        span { {tr!("Workflow")} }
                        if let Some(end) = end.clone() {
                            span { class: if end.status == TaskStatus::Failed { "end-failed" } else { "" }, {end_label(end.status)} }
                        } else {
                            Clock { started: Some(card.seen_at) }
                        }
                    }
                    div { class: "task-meta",
                        if !agents.is_empty() {
                            span { {if agents.len() == 1 { tr!("{count} agent", count = 1) } else { tr!("{count} agents", count = agents.len()) }} }
                        }
                        if let Some(tokens) = tokens {
                            span { {tr!("{amount} tokens", amount = format_tokens(tokens))} }
                        }
                    }
                }
                if let Some(ms) = end.as_ref().and_then(|end| end.duration_ms) {
                    span { class: "task-total", {elapsed_label(ms / 1000)} }
                }
                if end.is_none() {
                    button {
                        class: "task-stop",
                        r#type: "button",
                        title: tr!("Stop task"),
                        "aria-label": tr!("Stop task"),
                        onclick: move |_| workspace.stop_task(id, task_id.clone()),
                        span { class: "task-stop-square" }
                    }
                }
            }
            if !card.description.is_empty() {
                div { class: "task-note", "{card.description}" }
            }
            if let Some(end) = end.clone() {
                if end.status == TaskStatus::Failed && !end.summary.is_empty() {
                    div { class: "task-error",
                        div { class: "task-error-title", {tr!("Error")} }
                        pre { "{error_text(&end.summary)}" }
                    }
                }
            }
            // steps: as before, with `stopped` passed to PhaseView
```

Pass `stopped` to `PhaseView { …, stopped }`, and change `PhaseView` and `step_open`:

```rust
/// A phase is open by default while one of its agents still runs, unless the run has ended.
fn step_open(step: &Step, ended: bool) -> bool {
    !ended && step.agents.iter().any(|agent| agent.state == AgentState::Running)
}

#[component]
fn PhaseView(step: Step, open: bool, stopped: bool, ontoggle: EventHandler<()>) -> Element {
    let done = step.done();
    let total = step.agents.len();
    let name_class = if step.failed() { "wf-step-name failed" } else { "wf-step-name" };
    rsx! {
        div { class: "wf-step",
            button { class: "wf-step-head", r#type: "button", "aria-expanded": "{open}", onclick: move |_| ontoggle.call(()),
                span { class: "{name_class}", "{step.name}" }
                if total > 0 {
                    span { class: "wf-step-count", "{done}/{total}" }
                }
                span { class: "wf-step-chevron", if open { "⌄" } else { "›" } }
            }
            div { class: "wf-dots",
                for agent in step.agents.iter() {
                    span { class: dot_class(agent.state, stopped) }
                }
            }
            if open {
                if step.agents.is_empty() {
                    div { class: "dock-note", {tr!("No agents started yet")} }
                }
                for agent in step.agents.iter() {
                    div { class: "wf-agent", key: "{agent.id}",
                        span { class: "wf-agent-mark", if agent.state == AgentState::Done { "✓" } else { "" } }
                        span { class: "wf-agent-label", "{agent.label}" }
                    }
                }
            }
        }
    }
}
```

and in `WorkflowCardView` the call sites use `step_open(&step, end.is_some())`.

- [ ] **Step 5: Build and run all the tests**

Run: `cargo check --workspace --all-targets 2>&1 | tail -5 && cargo test --workspace 2>&1 | grep -E "test result|FAILED|failed" | head`
Expected: clean build; every suite passes (a known flaky plugin test may need one rerun; note it if so).

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: the tasks panel keeps finished tasks, shows tokens and failed agents, the chat line says stopped or failed (OTH-10)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Real-window check, docs and memory

**Files:**
- Modify: `docs/superpowers/specs/2026-10-17-finished-tasks-design.md` (an "As built" section), `docs/superpowers/roadmap/backlog.md` (the OTH-10 row), `docs/superpowers/roadmap/README.md` (a plan-order row)
- Create (outside the repo): `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-45-finished-tasks.md` and a line in `MEMORY.md`

- [ ] **Step 1: Build and launch Forge with the harness**

Read `scripts/harness/README.md`. Build the sidecar (`cd sidecar && npm run build`; `dist/` is not committed) and the app (`cargo build -p forge-app`), then launch with `scripts/harness/launch.sh` and the `FORGE_CLAUDE_HOME` fixture as that README says. Use the `/private/tmp/forge-observe` folder and mode 수동 (ask permission), as round 18 did.

- [ ] **Step 2: Check each behaviour against round 18** (take a capture after every click)

Ask the user before sending messages (each run uses their account). Run, in one session:
1. A two-agent workflow to the end (allow each request): the running card shows `N agents  <tokens> tokens` that grows; the chat card shows no tokens and blue dots; at the end the card moves to `Finished 1` (closed), opens to the card with `Workflow  Completed`, total time at the right, tokens, steps folded; the chat line reads `Workflow ran <name>`.
2. A workflow with three agents where one request is denied and the script throws at the end: the denied agent's dot is red in the chat card and the panel; the step name is red; `n/m` does not count it; the card ends `Failed` with the pink `Error` box (text after `failed: `); the chat line ends with a red `failed`.
3. A workflow stopped with the stop button: the chat line ends `stopped`; the card says `Stopped`, hollow dots, last tokens kept.
4. A Bash `run_in_background` task: it joins the fold as the small card (`Bash  Completed  12s`), no tokens.
5. The trash icon: hides every shown finished card, goes disabled; a task finished afterwards shows.
6. Switch to another session and back: the fold stays as left. Close the session and open it again: finished cards of the completed and the failed workflow are back (the stopped one is not: known limit), with tokens and time from the record; no raw `<task-notification>` bubble in the chat.
Note any difference from round 18 in the spec's "As built".

- [ ] **Step 3: Stop the app and write the docs**

Run `scripts/harness/stop.sh`. In the spec add "## As built (2026-10-17)": what was built, what the probe showed (Task 3 Step 6), the result of each check above, the known limit, and anything left. In `backlog.md` update the OTH-10 row: Plan 45 built (finished fold, tokens, red dots, stopped and failed), left: amber dots (not reproduced), the per-agent token column. In `README.md` add a Plan 45 row to the order table (`Finished tasks, tokens, failed workflows (built)`; spec path). Write the memory file with the lessons (loader turned notifications into user bubbles; stop leaves no notification; tokens rounding) and add its one-line pointer to `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 45 as built, roadmap updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
