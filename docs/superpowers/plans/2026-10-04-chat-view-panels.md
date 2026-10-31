# Chat View, Side Panels and Suggestions Implementation Plan (Plan 23C)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect the conversation to what it produced: a file name in the chat opens the file in the Files panel (markdown rendered, with a source switch), a command running in the background gets a pill and a panel with its live output and a stop button, and the message box suggests the next prompt.

**Architecture:** The sidecar forwards three new SDK signals (background task set, task finished, prompt suggestion) and takes one new command (`stop_task`); the Rust side carries them through port, use case, actor and view. Pure logic (path rules, link rendering, task parsing, suggestion rules) lives in `forge-entities` / `forge-features` with tests; widgets draw it. The Files panel gains a request channel and a markdown mode; a new `Panel::Tasks` shows background tasks.

**Tech Stack:** TypeScript (sidecar, vitest), Rust 2021, Dioxus 0.7.10, tokio, serde.

**Spec:** `docs/superpowers/specs/2026-10-04-chat-view-panels-design.md` (observations: `docs/superpowers/roadmap/observations.md`, Rounds 7 and 8).

## Global Constraints

- UI text is English.
- `FilesPort::read_tail` receives only paths that passed `valid_output_path`; a path with `..`, a relative path, another folder or another task id is never read.
- Files outside the session folder are never opened in the Files panel; the notice is exactly `Only files inside the session folder can be opened here.`
- Task output shows the last 20000 bytes (`TASK_OUTPUT_TAIL`), polled once a second only while the Tasks panel is open.
- Ambient tasks (`ambient: true`) are left out of the background task set.
- Refinement of the spec (section 3): a `prompt_suggestion` is stored whenever it arrives (not only while idle) and cleared by every send; the message box shows it only while no turn runs and the box is empty. (The SDK may emit it just before or after the turn's end marker; storing always is safe.)
- Keep every existing test passing; counts must not drop. The workspace had 927 passing tests and the sidecar 95 when 23B was merged. Sidecar suite: `cd sidecar && npm test`.
- New CSS must not add a `prefers-color-scheme: dark` block. Colours come from existing variables (`--link` is the blue; the pill uses it).
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Out of scope: editing files in the panel, preview-tab replacement, files outside the folder, a list of finished tasks, undo and rewind icons.

## Review Focus

- A file path outside the session folder, or in a sibling folder with a shared prefix (`/work/app2/x` against `/work/app`): no panel opens, the notice shows, nothing is read (Task 4).
- A running task with no matching Bash call (a sub-agent): a card with the SDK's description, no command box, no output box, no panic (Task 5).
- A task output file that is missing, empty, or has a `..` in its path: `No output yet`, and a path with `..` is never read (Tasks 3 and 5).
- An inline code span equal to a known path but inside a fenced block, or a bare name shared by two known paths: stays plain text (Task 4).
- A stop request for a task that has already finished: the SDK's `stopTask` throws, the banner shows the message, the session goes on (Task 1).

## File Structure

- `sidecar/src/session.ts`, `protocol.ts`, `sidecar/test/session.test.ts` — events, `stop_task`, options (modify).
- `crates/forge-domain/src/background.rs` — `BackgroundTaskInfo`, `RunningTask` (create); `lib.rs` (modify).
- `crates/forge-ports/src/sidecar_port.rs` — events, `stop_task` (modify); `files_port.rs` — `read_tail` (modify).
- `crates/forge-adapter-sidecar/src/protocol.rs`, `process.rs` — wire command (modify).
- `crates/forge-adapter-files/src/lib.rs` — `read_tail` (modify).
- `crates/forge-application/src/event.rs`, `live_session.rs`, `live_session_actor.rs`, `test_support.rs`, `use_cases/sidecar_commands.rs`, `use_cases/mod.rs`, `lib.rs` — events, command, view, use case (modify).
- `crates/forge-entities/src/file_open.rs`, `links.rs`, `background.rs` — pure logic (create); `turn.rs`, `lib.rs` (modify).
- `crates/forge-features/src/workspace.rs` — `open_file`, `file_request`, `stop_task` (modify); `dock.rs` — `Panel::Tasks` (modify); `suggestion.rs` (create); `send_message.rs` (modify); `lib.rs` (modify).
- `crates/forge-widgets/src/files_panel.rs`, `dock.rs`, `chat_window.rs` (modify); `tasks_panel.rs` (create); `lib.rs` (modify).
- `crates/forge-shared/src/theme.css`, `theme.rs` — styles and the class list test (modify).

---

### Task 1: Sidecar: background task events, `stop_task`, prompt suggestion, options

**Files:**
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Produces (Task 2's Rust side matches these shapes):
  - events `{ type: "background_tasks", tasks: [{ task_id, description, task_type }] }`, `{ type: "task_finished", task_id, status: "completed"|"failed"|"stopped", summary }`, `{ type: "prompt_suggestion", text }`;
  - command `{ type: "stop_task", id, task_id }`.

- [ ] **Step 1: Write the failing tests**

Add to `sidecar/test/session.test.ts`, after the test `ignores answer_question for an unknown id and for a request that is not a question`:

```ts
  function systemMessages(...messages: unknown[]): ReturnType<QueryFn> {
    return toQueryHandle((async function* () {
      for (const message of messages) yield message;
    })());
  }

  it("forwards the SDK's background task set, leaving ambient tasks out", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages({
        type: "system",
        subtype: "background_tasks_changed",
        tasks: [
          { task_id: "b1", task_type: "local_bash", description: "Sleep for 60 seconds" },
          { task_id: "a1", task_type: "agent", description: "ambient", ambient: true },
          { task_type: "local_bash", description: "no id" },
        ],
      });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "background_tasks",
      tasks: [{ task_id: "b1", description: "Sleep for 60 seconds", task_type: "local_bash" }],
    });
  });

  it("forwards an empty background task set too, so the last task can end", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () => systemMessages({ type: "system", subtype: "background_tasks_changed", tasks: [] });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);
    expect(events).toContainEqual({ type: "background_tasks", tasks: [] });
  });

  it("forwards a finished task, ignoring a status it does not know", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages(
        { type: "system", subtype: "task_notification", task_id: "b1", status: "completed", summary: "done", output_file: "/x" },
        { type: "system", subtype: "task_notification", task_id: "b2", status: "weird", summary: "?", output_file: "/y" },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const finished = events.filter((e) => e.type === "task_finished");
    expect(finished).toEqual([{ type: "task_finished", task_id: "b1", status: "completed", summary: "done" }]);
  });

  it("forwards a prompt suggestion and drops an empty one", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      systemMessages({ type: "prompt_suggestion", suggestion: "run the tests" }, { type: "prompt_suggestion", suggestion: "" });
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.filter((e) => e.type === "prompt_suggestion")).toEqual([{ type: "prompt_suggestion", text: "run the tests" }]);
  });

  it("starts the query with prompt suggestions and the per-task stop declaration", async () => {
    let options: any;
    const queryFn: QueryFn = (args) => {
      options = args.options;
      return toQueryHandle((async function* () {})());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    await session.handle(startCommand);
    expect(options.promptSuggestions).toBe(true);
    expect(options.perTaskStopAffordance).toBe(true);
  });

  it("stops a task through the SDK, and shows a refusal as setting_failed without ending the session", async () => {
    const events: SidecarEvent[] = [];
    const stopped: string[] = [];
    let release!: () => void;
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          await new Promise<void>((resolve) => (release = resolve));
        })(),
        {
          stopTask: async (taskId: string) => {
            if (taskId === "gone") throw new Error("no such task");
            stopped.push(taskId);
          },
        },
      );
    const session = new SidecarSession(queryFn, (e) => events.push(e));
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "stop_task", id: "5", task_id: "b1" });
    await session.handle({ type: "stop_task", id: "6", task_id: "gone" });
    release();
    await started;

    expect(stopped).toEqual(["b1"]);
    expect(events).toContainEqual({ type: "setting_failed", message: "no such task" });
    expect(events.some((e) => e.type === "error")).toBe(false);
  });

  it("answers stop_task with setting_failed when no session is running", async () => {
    const events: SidecarEvent[] = [];
    const session = new SidecarSession(() => toQueryHandle((async function* () {})()), (e) => events.push(e));
    await session.handle({ type: "stop_task", id: "1", task_id: "b1" });
    expect(events).toContainEqual({ type: "setting_failed", message: "no session is running" });
  });
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd sidecar && npm test -- session 2>&1 | tail -25`
Expected: FAIL (events not forwarded, options missing, `stop_task` unknown; type errors may also show for the new command/events).

- [ ] **Step 3: Implement**

`sidecar/src/protocol.ts`: add to `SidecarCommand` after `answer_question`:
```ts
  | { type: "stop_task"; id: string; task_id: string }
```
and add to `SidecarEvent` before `models_available`:
```ts
  // The SDK's full set of background tasks (replace semantics), a finished one, and a predicted next prompt.
  | { type: "background_tasks"; tasks: WireBackgroundTask[] }
  | { type: "task_finished"; task_id: string; status: "completed" | "failed" | "stopped"; summary: string }
  | { type: "prompt_suggestion"; text: string }
```
and above `SidecarCommand` add:
```ts
export interface WireBackgroundTask {
  task_id: string;
  description: string;
  task_type: string;
}
```

`sidecar/src/session.ts`:
- `QueryHandle` gets `stopTask?: (taskId: string) => Promise<void>;`.
- In `query = this.queryFn({ ... options: {` add after `additionalDirectories: ...,`:
```ts
          // Forge shows a stop button per background task and predicts the next prompt.
          promptSuggestions: true,
          perTaskStopAffordance: true,
```
- In `handle`, add a case before `case "shutdown":`:
```ts
      case "stop_task": {
        const query = this.activeQuery;
        if (!query?.stopTask) {
          this.emit({ type: "setting_failed", message: "no session is running" });
          return;
        }
        try {
          await query.stopTask(command.task_id);
        } catch (err) {
          this.emit({ type: "setting_failed", message: err instanceof Error ? err.message : String(err) });
        }
        return;
      }
```
- In `forwardMessage` replace the `case "system":` block with:
```ts
      case "system":
        if (message.subtype === "init") {
          this.emit({ type: "session_started", session_id: message.session_id });
          this.emitCommands(message.slash_commands, true);
        } else if (message.subtype === "background_tasks_changed") {
          const tasks = (Array.isArray(message.tasks) ? message.tasks : [])
            .filter((task: any) => task && !task.ambient && typeof task.task_id === "string" && task.task_id !== "")
            .map((task: any) => ({
              task_id: task.task_id as string,
              description: typeof task.description === "string" ? task.description : "",
              task_type: typeof task.task_type === "string" ? task.task_type : "",
            }));
          this.emit({ type: "background_tasks", tasks });
        } else if (message.subtype === "task_notification") {
          const status = message.status;
          if (typeof message.task_id === "string" && (status === "completed" || status === "failed" || status === "stopped")) {
            this.emit({ type: "task_finished", task_id: message.task_id, status, summary: typeof message.summary === "string" ? message.summary : "" });
          }
        }
        return;
      case "prompt_suggestion":
        if (typeof message.suggestion === "string" && message.suggestion !== "") {
          this.emit({ type: "prompt_suggestion", text: message.suggestion });
        }
        return;
```

- [ ] **Step 4: Run the sidecar suite**

Run: `cd sidecar && npm run typecheck 2>&1 | tail -5 && npm test 2>&1 | tail -8`
Expected: clean typecheck; all pass (95 + 7 = 102).

- [ ] **Step 5: Rebuild and commit**

Run: `cd sidecar && npm run build 2>&1 | tail -2`
```bash
git add sidecar/src sidecar/test
git commit -m "feat: the sidecar forwards background tasks and prompt suggestions and can stop a task

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Rust: events, `stop_task`, background tasks and suggestion in the live view

**Files:**
- Create: `crates/forge-domain/src/background.rs`
- Modify: `crates/forge-domain/src/lib.rs`, `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-adapter-sidecar/src/protocol.rs`, `crates/forge-adapter-sidecar/src/process.rs`
- Modify: `crates/forge-application/src/event.rs`, `live_session.rs`, `live_session_actor.rs`, `test_support.rs`, `use_cases/sidecar_commands.rs`, `use_cases/mod.rs`, `lib.rs`
- Modify: `crates/forge-features/src/workspace.rs`; compiler-driven: `LiveSessionView` literals (`forge-features/src/sidebar.rs` tests).

**Interfaces:**
- Consumes: Task 1's wire shapes.
- Produces:
  - `forge_domain::BackgroundTaskInfo { id, description, task_type }` (serde: `id` is `task_id` on the wire, `description` and `task_type` default to empty) and `RunningTask { info: BackgroundTaskInfo, seen_at: SystemTime }`;
  - `SidecarEvent::{BackgroundTasks { tasks: Vec<BackgroundTaskInfo> }, TaskFinished { task_id, status, summary }, PromptSuggestion { text }}`;
  - `SidecarPort::stop_task(&self, task_id: String) -> Result<(), SidecarError>`; `StopTaskUseCase`; `LiveSessionHandle::stop_task(task_id)`; `Workspace::stop_task(id, task_id)`;
  - `LiveSessionView.background_tasks: Vec<RunningTask>`, `LiveSessionView.suggestion: Option<String>`;
  - test helper `Call::StopTask(String)`.

- [ ] **Step 1: Write the failing tests**

(a) `crates/forge-domain/src/background.rs` — create with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_reads_from_the_wire_shape_with_defaults() {
        let task: BackgroundTaskInfo = serde_json::from_str(r#"{"task_id":"b1","description":"Sleep","task_type":"local_bash"}"#).unwrap();
        assert_eq!(task, BackgroundTaskInfo { id: "b1".to_string(), description: "Sleep".to_string(), task_type: "local_bash".to_string() });
        let bare: BackgroundTaskInfo = serde_json::from_str(r#"{"task_id":"b2"}"#).unwrap();
        assert_eq!(bare.description, "");
        assert_eq!(bare.task_type, "");
    }
}
```

(b) `crates/forge-ports/src/sidecar_port.rs`, in the `tests` module:

```rust
    #[test]
    fn background_task_and_suggestion_events_read_from_the_wire() {
        let tasks: SidecarEvent = serde_json::from_str(
            r#"{"type":"background_tasks","tasks":[{"task_id":"b1","description":"Sleep","task_type":"local_bash"}]}"#,
        )
        .unwrap();
        assert_eq!(
            tasks,
            SidecarEvent::BackgroundTasks {
                tasks: vec![forge_domain::BackgroundTaskInfo { id: "b1".to_string(), description: "Sleep".to_string(), task_type: "local_bash".to_string() }]
            }
        );
        let none: SidecarEvent = serde_json::from_str(r#"{"type":"background_tasks","tasks":[]}"#).unwrap();
        assert_eq!(none, SidecarEvent::BackgroundTasks { tasks: vec![] });
        let finished: SidecarEvent =
            serde_json::from_str(r#"{"type":"task_finished","task_id":"b1","status":"stopped","summary":"s"}"#).unwrap();
        assert_eq!(finished, SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "stopped".to_string(), summary: "s".to_string() });
        let suggestion: SidecarEvent = serde_json::from_str(r#"{"type":"prompt_suggestion","text":"run it"}"#).unwrap();
        assert_eq!(suggestion, SidecarEvent::PromptSuggestion { text: "run it".to_string() });
    }
```

(c) `crates/forge-adapter-sidecar/src/protocol.rs`, in `tests`:

```rust
    #[test]
    fn stop_task_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::StopTask { id: "8".to_string(), task_id: "b1".to_string() };
        assert_eq!(serde_json::to_value(&command).unwrap(), serde_json::json!({"type": "stop_task", "id": "8", "task_id": "b1"}));
    }
```

(d) `crates/forge-application/src/use_cases/sidecar_commands.rs`, in `tests`:

```rust
    #[tokio::test]
    async fn stop_task_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_stop_task().withf(|id| id == "b1").times(1).returning(|_| Ok(()));
        assert!(StopTaskUseCase::new(Arc::new(mock)).execute("b1".to_string()).await.is_ok());
    }
```

(e) `crates/forge-application/src/event.rs`, in `tests`:

```rust
    #[test]
    fn background_task_task_finished_and_suggestion_events_become_updates() {
        let mut session = empty_session();
        let tasks = vec![forge_domain::BackgroundTaskInfo { id: "b1".to_string(), description: "d".to_string(), task_type: "t".to_string() }];
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::BackgroundTasks { tasks: tasks.clone() }),
            SessionUpdate::BackgroundTasks(tasks)
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string() }),
            SessionUpdate::TaskFinished { task_id: "b1".to_string() }
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::PromptSuggestion { text: "next".to_string() }),
            SessionUpdate::PromptSuggestion("next".to_string())
        );
    }
```

(f) `crates/forge-application/src/live_session.rs`, in `tests` (add `BackgroundTaskInfo` to its `use forge_domain::{...}` line):

```rust
    fn task(id: &str) -> forge_domain::BackgroundTaskInfo {
        forge_domain::BackgroundTaskInfo { id: id.to_string(), description: format!("task {id}"), task_type: "local_bash".to_string() }
    }

    #[tokio::test]
    async fn stop_task_reaches_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.stop_task("b1".to_string());
        handle.interrupt(); // commands run in order
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(control.calls().contains(&Call::StopTask("b1".to_string())));
    }

    #[tokio::test]
    async fn the_task_set_is_replaced_and_a_staying_task_keeps_its_first_seen_time() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        let first = wait_for(&mut view, "one task", |v| v.background_tasks.len() == 1).await;
        let seen = first.background_tasks[0].seen_at;

        tokio::time::sleep(Duration::from_millis(20)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1"), task("b2")] });
        let second = wait_for(&mut view, "two tasks", |v| v.background_tasks.len() == 2).await;
        assert_eq!(second.background_tasks[0].seen_at, seen, "b1 keeps its time");
        assert!(second.background_tasks[1].seen_at > seen, "b2 is new");

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![] });
        wait_for(&mut view, "no tasks", |v| v.background_tasks.is_empty()).await;
    }

    #[tokio::test]
    async fn the_task_set_empties_when_the_sidecar_fails() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        wait_for(&mut view, "one task", |v| v.background_tasks.len() == 1).await;

        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let failed = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;
        assert!(failed.background_tasks.is_empty());
    }

    #[tokio::test]
    async fn a_suggestion_is_kept_until_the_next_send() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        control.push(turn_complete());
        control.push(SidecarEvent::PromptSuggestion { text: "run the tests".to_string() });
        let with = wait_for(&mut view, "the suggestion", |v| v.suggestion.is_some()).await;
        assert_eq!(with.suggestion.as_deref(), Some("run the tests"));

        handle.send("two".to_string());
        let cleared = wait_for(&mut view, "the suggestion cleared", |v| v.suggestion.is_none() && v.phase == Phase::Running).await;
        assert!(cleared.suggestion.is_none());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-domain 2>&1 | tail -4` (add `mod background;` first) and `cargo build --workspace --tests 2>&1 | grep -E "^error" | head -8`
Expected: compile errors (`BackgroundTaskInfo`, the variants, `stop_task`, `Call::StopTask`, the view fields are missing).

- [ ] **Step 3: Implement the domain, ports and wire**

`crates/forge-domain/src/background.rs`, above the tests:

```rust
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// A task the SDK runs in the background (a command started with `run_in_background`, a
/// sub-agent). `id` is `task_id` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTaskInfo {
    #[serde(rename = "task_id")]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub task_type: String,
}

/// A task that is running now, and when the live view first saw it (the clock counts from here).
#[derive(Debug, Clone, PartialEq)]
pub struct RunningTask {
    pub info: BackgroundTaskInfo,
    pub seen_at: SystemTime,
}
```
`lib.rs`: add `mod background;` and `pub use background::{BackgroundTaskInfo, RunningTask};`.

`crates/forge-ports/src/sidecar_port.rs`: add to `SidecarEvent` (before `Error`):
```rust
    /// The SDK's whole set of background tasks; each event replaces the earlier one.
    BackgroundTasks { tasks: Vec<forge_domain::BackgroundTaskInfo> },
    /// A background task ended (informational: the set comes from `BackgroundTasks`).
    TaskFinished { task_id: String, status: String, summary: String },
    /// The SDK's guess at what the person will ask next.
    PromptSuggestion { text: String },
```
and to the trait after `interrupt`:
```rust
    /// Stops one background task. A refusal comes back as a `SettingFailed` event.
    async fn stop_task(&self, task_id: String) -> Result<(), SidecarError>;
```

`crates/forge-adapter-sidecar/src/protocol.rs`: add `StopTask { id: String, task_id: String },` to `SidecarCommand` (after `Interrupt`). `process.rs`: add to `impl SidecarPort for SidecarProcess`:
```rust
    async fn stop_task(&self, task_id: String) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::StopTask { id, task_id })
    }
```

- [ ] **Step 4: Implement the application layer**

`test_support.rs`: add `StopTask(String),` to `Call` and
```rust
    async fn stop_task(&self, task_id: String) -> Result<(), SidecarError> {
        self.control.record(Call::StopTask(task_id));
        Ok(())
    }
```
to `impl SidecarPort for FakeSidecar`.

`use_cases/sidecar_commands.rs` add (before `InterruptTurnUseCase`):
```rust
pub struct StopTaskUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl StopTaskUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(&self, task_id: String) -> Result<(), SidecarError> {
        self.sidecar.stop_task(task_id).await
    }
}
```
and export `StopTaskUseCase` in `use_cases/mod.rs` (`pub use sidecar_commands::{...}`) and `lib.rs`.

`event.rs`: add to `SessionUpdate`: `BackgroundTasks(Vec<forge_domain::BackgroundTaskInfo>), TaskFinished { task_id: String }, PromptSuggestion(String),` and to `apply_sidecar_event` (before `Error`):
```rust
        SidecarEvent::BackgroundTasks { tasks } => SessionUpdate::BackgroundTasks(tasks.clone()),
        SidecarEvent::TaskFinished { task_id, .. } => SessionUpdate::TaskFinished { task_id: task_id.clone() },
        SidecarEvent::PromptSuggestion { text } => SessionUpdate::PromptSuggestion(text.clone()),
```

`live_session.rs`: add `StopTask { task_id: String },` to `LiveCommand`; to `LiveSessionHandle`:
```rust
    /// Stops one background task (see `SidecarPort::stop_task`).
    pub fn stop_task(&self, task_id: String) {
        let _ = self.commands.send(LiveCommand::StopTask { task_id });
    }
```
to `LiveSessionView` (after `turn_started`): `pub background_tasks: Vec<forge_domain::RunningTask>,` and `/// The SDK's guess at the next prompt; cleared by a send.\n    pub suggestion: Option<String>,`, and `background_tasks: Vec::new(), suggestion: None,` in `spawn_live_session`.

`live_session_actor.rs`: import `StopTaskUseCase`; dispatch `LiveCommand::StopTask { task_id } => self.stop_task(task_id).await,`; add:
```rust
    async fn stop_task(&mut self, task_id: String) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        if let Err(error) = StopTaskUseCase::new(sidecar).execute(task_id).await {
            self.fail(error.to_string(), false).await;
        }
    }
```
in `send`'s `Ok(())` arm add `self.view.suggestion = None;`; in `apply_event` add arms:
```rust
            SessionUpdate::BackgroundTasks(tasks) => {
                let now = std::time::SystemTime::now();
                let previous = std::mem::take(&mut self.view.background_tasks);
                self.view.background_tasks = tasks
                    .into_iter()
                    .map(|info| {
                        let seen_at = previous.iter().find(|task| task.info.id == info.id).map_or(now, |task| task.seen_at);
                        forge_domain::RunningTask { info, seen_at }
                    })
                    .collect();
            }
            SessionUpdate::PromptSuggestion(text) => self.view.suggestion = Some(text),
```
and add `| SessionUpdate::TaskFinished { .. }` to the no-op arm list; in `discard_sidecar` (the function `fail` calls) add `self.view.background_tasks.clear();` at its start.

`workspace.rs`: add after `answer_question`:
```rust
    pub fn stop_task(&self, id: LiveSessionId, task_id: String) {
        if let Some(handle) = self.handle(id) {
            handle.stop_task(task_id);
        }
    }
```
Fix every `LiveSessionView { ... }` literal the compiler reports (`turn_started: None,` is there; add `background_tasks: Vec::new(), suggestion: None,`).

- [ ] **Step 5: Run the tests**

Run: `cargo test --workspace > /tmp/t2c.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t2c.txt | head; grep -E "^test result" /tmp/t2c.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`; passed 927 + 9 = 936 (1 domain, 1 port, 1 wire, 1 use case, 1 event, 4 live session).

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: the live view knows the background tasks and the prompt suggestion, and can stop a task

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `FilesPort::read_tail`

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-adapter-files/src/lib.rs`

**Interfaces:**
- Produces: `FilesPort::read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError>` (the mock is generated).

- [ ] **Step 1: Write the failing tests**

In `crates/forge-adapter-files/src/lib.rs`'s test module (it already uses `tempfile`), add:

```rust
    #[test]
    fn read_tail_gives_the_end_of_a_long_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("out.txt");
        std::fs::write(&file, "0123456789").unwrap();
        assert_eq!(LocalFiles.read_tail(&file, 4).unwrap(), "6789");
        assert_eq!(LocalFiles.read_tail(&file, 100).unwrap(), "0123456789");
        assert_eq!(LocalFiles.read_tail(&file, 0).unwrap(), "");
    }

    #[test]
    fn read_tail_of_an_empty_or_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.txt");
        std::fs::write(&empty, "").unwrap();
        assert_eq!(LocalFiles.read_tail(&empty, 10).unwrap(), "");
        assert!(matches!(LocalFiles.read_tail(&dir.path().join("nope.txt"), 10), Err(FilesError::Failed(_))));
    }

    #[test]
    fn read_tail_replaces_bytes_that_are_not_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bin.txt");
        std::fs::write(&file, [b'a', 0xff, b'b']).unwrap();
        assert_eq!(LocalFiles.read_tail(&file, 10).unwrap(), "a\u{fffd}b");
    }
```
(If the module builds the adapter differently than `LocalFiles` as a unit value, copy how the existing tests construct it.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-files read_tail 2>&1 | grep -E "^error" -A3 | head -6`
Expected: compile error (`read_tail` missing).

- [ ] **Step 3: Implement**

`files_port.rs`, in the trait after `search`:
```rust
    /// The last `max_bytes` of the file at an absolute `path`, as text (bytes that are not UTF-8 are
    /// replaced). Unlike `read` it is not confined to a root: the caller must have checked the path.
    fn read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError>;
```
`forge-adapter-files/src/lib.rs`, in `impl FilesPort for LocalFiles` after `search`:
```rust
    fn read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = fs::File::open(path).map_err(failed)?;
        let length = file.metadata().map_err(failed)?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(max_bytes as u64))).map_err(failed)?;
        let mut bytes = Vec::new();
        file.take(max_bytes as u64).read_to_end(&mut bytes).map_err(failed)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
```

- [ ] **Step 4: Run and commit**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`, 939 passed.
```bash
git add -A crates
git commit -m "feat: FilesPort reads the tail of a file

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Pure rules for opening files and linking names (`file_open.rs`, `links.rs`)

**Files:**
- Create: `crates/forge-entities/src/file_open.rs`, `crates/forge-entities/src/links.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn relative_to_folder(folder: &Path, path: &str) -> Option<String>;
  pub fn is_markdown(path: &str) -> bool;
  pub fn known_paths(messages: &[Message]) -> Vec<String>;
  pub fn render_markdown_linked(source: &str, known: &[String]) -> String;
  ```
  Linked output: `<span class="file-link" data-path="ABS" role="link" tabindex="0"><code>TEXT</code></span>`.

- [ ] **Step 1: Write the failing tests**

`file_open.rs` — create with:

```rust
#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn a_file_inside_the_folder_gets_a_relative_path() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/src/main.rs").as_deref(), Some("src/main.rs"));
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/a.txt").as_deref(), Some("a.txt"));
        assert_eq!(relative_to_folder(Path::new("/work/app/"), "/work/app/a.txt").as_deref(), Some("a.txt"));
    }

    #[test]
    fn a_path_outside_the_folder_gets_none() {
        for outside in ["/work/other/a.txt", "/etc/passwd", "/work/app2/x.txt", "/work", "relative/a.txt", ""] {
            assert_eq!(relative_to_folder(Path::new("/work/app"), outside), None, "{outside}");
        }
    }

    #[test]
    fn dot_dot_cannot_climb_out_of_the_folder() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/../secret.txt"), None);
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/a/../b.txt").as_deref(), Some("b.txt"));
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/../app/c.txt").as_deref(), Some("c.txt"));
    }

    #[test]
    fn the_folder_itself_is_not_a_file() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app"), None);
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/"), None);
    }

    #[test]
    fn markdown_files_are_told_by_their_extension() {
        for yes in ["README.md", "a/b/Guide.MD", "notes.markdown", "x.Markdown"] {
            assert!(is_markdown(yes), "{yes}");
        }
        for no in ["main.rs", "md", "a.md.txt", "notes.mdx", "", "dir/"] {
            assert!(!is_markdown(no), "{no}");
        }
    }
}
```

`links.rs` — create with:

```rust
#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall};
    use serde_json::json;

    use super::*;

    fn call(name: &str, input: serde_json::Value) -> Block {
        Block::Tool(ToolCall { id: "t".to_string(), name: name.to_string(), input, result: None })
    }
    fn assistant(blocks: Vec<Block>) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = blocks;
        message
    }
    fn known() -> Vec<String> {
        vec!["/p/docs/guide.md".to_string(), "/p/src/a.rs".to_string(), "/p/other/a.rs".to_string()]
    }

    #[test]
    fn known_paths_come_from_file_tools_once_each_in_order() {
        let messages = vec![assistant(vec![
            call("Write", json!({"file_path": "/p/docs/guide.md", "content": "x"})),
            call("Read", json!({"file_path": "/p/src/a.rs"})),
            call("Edit", json!({"file_path": "/p/docs/guide.md", "old_string": "a", "new_string": "b"})),
            call("NotebookEdit", json!({"notebook_path": "/p/n.ipynb", "new_source": "x"})),
            call("Bash", json!({"command": "cat /p/secret"})),
            call("Write", json!({"file_path": "relative.txt", "content": "x"})),
            call("Write", json!({"content": "no path"})),
        ])];
        assert_eq!(known_paths(&messages), vec!["/p/docs/guide.md", "/p/src/a.rs", "/p/n.ipynb"]);
    }

    #[test]
    fn an_inline_code_span_with_a_full_known_path_becomes_a_link() {
        let html = render_markdown_linked("see `/p/docs/guide.md` now", &known());
        assert!(html.contains(r#"<span class="file-link" data-path="/p/docs/guide.md" role="link" tabindex="0"><code>/p/docs/guide.md</code></span>"#), "{html}");
    }

    #[test]
    fn a_unique_bare_name_becomes_a_link_to_its_path() {
        let html = render_markdown_linked("saved to `guide.md`", &known());
        assert!(html.contains(r#"data-path="/p/docs/guide.md""#), "{html}");
        assert!(html.contains("<code>guide.md</code>"), "{html}");
    }

    #[test]
    fn a_name_shared_by_two_known_paths_stays_plain() {
        let html = render_markdown_linked("see `a.rs`", &known());
        assert!(!html.contains("file-link"), "{html}");
        assert!(html.contains("<code>a.rs</code>"), "{html}");
    }

    #[test]
    fn code_that_is_not_a_known_path_stays_plain() {
        for text in ["`other.md`", "`/p/docs/missing.md`", "`guide.md.bak`", "`GUIDE.MD`"] {
            assert!(!render_markdown_linked(text, &known()).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn text_outside_code_and_code_in_a_fenced_block_stay_plain() {
        assert!(!render_markdown_linked("guide.md and /p/docs/guide.md", &known()).contains("file-link"));
        let fenced = render_markdown_linked("```\n/p/docs/guide.md\n```", &known());
        assert!(!fenced.contains("file-link"), "{fenced}");
        let language = render_markdown_linked("```text\nguide.md\n```", &known());
        assert!(!language.contains("file-link"), "{language}");
    }

    #[test]
    fn a_path_with_a_quote_is_never_put_in_an_attribute() {
        let weird = vec!["/p/a\"b.md".to_string()];
        let html = render_markdown_linked("`/p/a\"b.md`", &weird);
        assert!(!html.contains("file-link"), "{html}");
    }

    #[test]
    fn raw_html_is_still_shown_as_text_and_with_no_known_paths_the_output_is_plain_markdown() {
        let html = render_markdown_linked("<script>x</script> `guide.md`", &known());
        assert!(!html.contains("<script"), "{html}");
        assert_eq!(render_markdown_linked("**hi**", &[]), crate::render_markdown("**hi**"));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Add to `lib.rs`: `mod file_open;`, `mod links;`, and `pub use file_open::{is_markdown, relative_to_folder}; pub use links::{known_paths, render_markdown_linked};`. Run: `cargo test -p forge-entities file_open 2>&1 | grep -E "^error" | head -4; cargo test -p forge-entities links 2>&1 | grep -E "^error" | head -4`
Expected: compile errors (functions missing).

- [ ] **Step 3: Implement**

`file_open.rs`, above the tests:

```rust
use std::path::{Component, Path};

/// `path` relative to `folder` when it is a file inside it, else `None`. Both are cleaned lexically
/// (no disk access): `.` is dropped and `..` goes up one level, and a path that climbs out of the
/// folder, a sibling that only shares a name prefix (`/work/app2` against `/work/app`), a relative
/// path, the folder itself and an empty string are all `None`.
pub fn relative_to_folder(folder: &Path, path: &str) -> Option<String> {
    let cleaned = clean(Path::new(path))?;
    let folder = clean(folder)?;
    let relative = cleaned.strip_prefix(&folder).ok()?;
    let text = relative.to_string_lossy().replace('\\', "/");
    (!text.is_empty()).then_some(text)
}

// An absolute path without `.` and `..` components; `None` when it is relative or climbs above the root.
fn clean(path: &Path) -> Option<std::path::PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(part) => parts.push(part),
        }
    }
    let mut cleaned = std::path::PathBuf::from("/");
    cleaned.extend(parts);
    Some(cleaned)
}

/// Whether the file is shown rendered by default: `.md` and `.markdown`, any case.
pub fn is_markdown(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, extension)) => !stem.is_empty() && matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"),
        None => false,
    }
}
```

`links.rs`, above the tests:

```rust
use forge_domain::{Block, Message};

use crate::highlight::escape_html;
use crate::markdown::render_markdown;

/// The absolute paths the session's own file tools named (Write, Edit, MultiEdit, NotebookEdit,
/// Read), each once, in order. A name that only appears in prose is not here.
pub fn known_paths(messages: &[Message]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for call in messages.iter().flat_map(|message| message.tool_calls()) {
        if !matches!(call.name.as_str(), "Write" | "Edit" | "MultiEdit" | "NotebookEdit" | "Read") {
            continue;
        }
        let path = ["file_path", "notebook_path"].iter().find_map(|key| call.input.get(*key).and_then(|value| value.as_str()));
        if let Some(path) = path.filter(|path| path.starts_with('/')) {
            if !paths.iter().any(|known| known == path) {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `render_markdown`, with each inline code span that names a known file wrapped in
/// `<span class="file-link" data-path="…" role="link" tabindex="0">`. Code in a fenced or indented
/// block and text outside code are left alone; a path that holds a quote, `<` or `&` is never put
/// in an attribute.
pub fn render_markdown_linked(source: &str, known: &[String]) -> String {
    let html = render_markdown(source);
    if known.is_empty() {
        return html;
    }
    const OPEN: &str = "<code>";
    const CLOSE: &str = "</code>";
    const BLOCK: &str = "<pre class=\"code-block\">";
    let mut out = String::with_capacity(html.len());
    let mut rest = html.as_str();
    while let Some(start) = rest.find(OPEN) {
        let (before, from_code) = rest.split_at(start);
        out.push_str(before);
        let after_open = &from_code[OPEN.len()..];
        let Some(end) = after_open.find(CLOSE) else {
            out.push_str(from_code);
            return out;
        };
        // `before` is already in `out`: a code span right after the block's `<pre>` is block code.
        let inside_block = out.ends_with(BLOCK);
        let text = &after_open[..end];
        let link = if inside_block { None } else { resolve_escaped(text, known) }.filter(|path| !path.contains(['"', '<', '>', '&']));
        match link {
            Some(path) => out.push_str(&format!(
                "<span class=\"file-link\" data-path=\"{path}\" role=\"link\" tabindex=\"0\">{OPEN}{text}{CLOSE}</span>"
            )),
            None => {
                out.push_str(OPEN);
                out.push_str(text);
                out.push_str(CLOSE);
            }
        }
        rest = &after_open[end + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

// The text between the tags is HTML-escaped; compare it with the escaped known paths and names.
fn resolve_escaped<'a>(escaped_text: &str, known: &'a [String]) -> Option<&'a str> {
    let escaped_known: Vec<String> = known.iter().map(|path| escape_html(path)).collect();
    let index = {
        let mut found = None;
        if let Some(position) = escaped_known.iter().position(|path| path == escaped_text) {
            found = Some(position);
        } else if !escaped_text.contains('/') {
            let matches: Vec<usize> = known.iter().enumerate().filter(|(_, path)| escape_html(file_name(path)) == escaped_text).map(|(index, _)| index).collect();
            if matches.len() == 1 {
                found = Some(matches[0]);
            }
        }
        found?
    };
    Some(known[index].as_str())
}
```
Note: if `escape_html` is not `pub(crate)` in `highlight.rs`, make it so (`markdown.rs` already imports it, so it is crate-visible).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities 2>&1 | grep -E "FAILED|panicked|left:|right:|^error|test result" -A3 | head -20`
Expected: all pass (the 13 new tests plus the old ones). If `raw_html...` fails because `render_markdown` escapes differently, print the HTML and fix the assertion to what `render_markdown` really produces for the same input, not the function.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: rules for opening a file from the chat and linking known file names

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Pure rules for background tasks (`background.rs`)

**Files:**
- Create: `crates/forge-entities/src/background.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{Message, RunningTask}`.
- Produces:
  ```rust
  pub struct BackgroundRef { pub task_id: String, pub output_path: String }
  pub fn background_ref(output: &str) -> Option<BackgroundRef>;
  pub fn valid_output_path(path: &str, task_id: &str) -> bool;
  pub struct TaskCard { pub id: String, pub description: String, pub task_type: String, pub command: Option<String>, pub output_path: Option<String>, pub seen_at: SystemTime }
  pub fn task_cards(messages: &[Message], running: &[RunningTask]) -> Vec<TaskCard>;
  pub fn pill_label(count: usize) -> String;
  pub const TASK_OUTPUT_TAIL: usize = 20_000;
  ```

- [ ] **Step 1: Write the failing tests**

`background.rs` — create with:

```rust
#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use forge_domain::{BackgroundTaskInfo, Block, Message, MessageRole, RunningTask, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    const SENTENCE: &str = "Command running in background with ID: bplyt9cwq. Output is being written to: /private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output. You will be notified when it completes. To check interim output, use Read on that file path.";

    #[test]
    fn the_real_sentence_gives_the_id_and_the_output_path() {
        assert_eq!(
            background_ref(SENTENCE),
            Some(BackgroundRef {
                task_id: "bplyt9cwq".to_string(),
                output_path: "/private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output".to_string()
            })
        );
    }

    #[test]
    fn other_text_gives_nothing() {
        for text in ["", "ok", "Command running in background with ID: x.", "Output is being written to: /a/tasks/x.output."] {
            assert_eq!(background_ref(text), None, "{text}");
        }
    }

    #[test]
    fn only_a_task_output_path_of_that_task_is_valid() {
        assert!(valid_output_path("/private/tmp/c/tasks/bplyt9cwq.output", "bplyt9cwq"));
        for bad in [
            "/private/tmp/c/tasks/other.output",
            "/private/tmp/c/tasks/../../etc/passwd",
            "/private/tmp/c/tasks/bplyt9cwq.output/../x",
            "tasks/bplyt9cwq.output",
            "/private/tmp/c/bplyt9cwq.output",
            "/private/tmp/c/tasks/bplyt9cwq.txt",
            "/tasks/bplyt9cwq.output\0",
            "",
        ] {
            assert!(!valid_output_path(bad, "bplyt9cwq"), "{bad}");
        }
    }

    fn running(id: &str, secs: u64) -> RunningTask {
        RunningTask {
            info: BackgroundTaskInfo { id: id.to_string(), description: format!("sdk {id}"), task_type: "local_bash".to_string() },
            seen_at: UNIX_EPOCH + Duration::from_secs(secs),
        }
    }
    fn bash(command: &str, description: Option<&str>, result: &str) -> Message {
        let input = match description {
            Some(d) => json!({"command": command, "description": d, "run_in_background": true}),
            None => json!({"command": command, "run_in_background": true}),
        };
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks.push(Block::Tool(ToolCall {
            id: "t".to_string(),
            name: "Bash".to_string(),
            input,
            result: Some(ToolResult { output: result.to_string(), is_error: false }),
        }));
        message
    }

    #[test]
    fn a_card_takes_its_command_and_description_from_the_bash_call_that_started_it() {
        let messages = vec![bash("sleep 60", Some("Sleep for 60 seconds"), SENTENCE)];
        let cards = task_cards(&messages, &[running("bplyt9cwq", 7)]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].command.as_deref(), Some("sleep 60"));
        assert_eq!(cards[0].description, "Sleep for 60 seconds");
        assert_eq!(cards[0].seen_at, UNIX_EPOCH + Duration::from_secs(7));
        assert_eq!(
            cards[0].output_path.as_deref(),
            Some("/private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output")
        );
    }

    #[test]
    fn a_card_without_a_description_falls_back_to_the_sdk_text_then_the_command() {
        let messages = vec![bash("sleep 60", None, SENTENCE)];
        let cards = task_cards(&messages, &[running("bplyt9cwq", 0)]);
        assert_eq!(cards[0].description, "sdk bplyt9cwq");
    }

    #[test]
    fn a_task_with_no_matching_bash_call_still_gets_a_card_without_command_or_output() {
        let cards = task_cards(&[], &[running("agent1", 0)]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].description, "sdk agent1");
        assert_eq!(cards[0].command, None);
        assert_eq!(cards[0].output_path, None);
    }

    #[test]
    fn an_output_path_that_is_not_a_task_file_is_never_offered() {
        let evil = "Command running in background with ID: b1. Output is being written to: /etc/passwd. You will be notified when it completes.";
        let cards = task_cards(&[bash("x", None, evil)], &[running("b1", 0)]);
        assert_eq!(cards[0].output_path, None);
        assert_eq!(cards[0].command.as_deref(), Some("x"));
    }

    #[test]
    fn cards_keep_the_order_of_the_sdk_set() {
        let cards = task_cards(&[], &[running("b", 0), running("a", 0)]);
        let ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn the_pill_counts_the_tasks() {
        assert_eq!(pill_label(1), "1 background task");
        assert_eq!(pill_label(2), "2 background tasks");
        assert_eq!(pill_label(0), "0 background tasks");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add to `lib.rs`: `mod background;` and `pub use background::{background_ref, pill_label, task_cards, valid_output_path, BackgroundRef, TaskCard, TASK_OUTPUT_TAIL};`. Run: `cargo test -p forge-entities background 2>&1 | grep -E "^error" | head -4`
Expected: compile errors.

- [ ] **Step 3: Implement**

Above the tests in `background.rs`:

```rust
use std::time::SystemTime;

use forge_domain::{Message, RunningTask};

/// How much of a task's output the panel shows: the last 20000 bytes.
pub const TASK_OUTPUT_TAIL: usize = 20_000;

/// What a Bash call that started a background command says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundRef {
    pub task_id: String,
    pub output_path: String,
}

const ID_MARK: &str = "Command running in background with ID: ";
const PATH_MARK: &str = "Output is being written to: ";

/// Reads `Command running in background with ID: <id>. Output is being written to: <path>. You will
/// be notified ...` (the Bash tool's result for `run_in_background`). Anything else is `None`.
pub fn background_ref(output: &str) -> Option<BackgroundRef> {
    let after_id = &output[output.find(ID_MARK)? + ID_MARK.len()..];
    let id_end = after_id.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after_id.len());
    let task_id = &after_id[..id_end];
    let after_path = &output[output.find(PATH_MARK)? + PATH_MARK.len()..];
    let path_end = after_path.find(". You will be notified").or_else(|| after_path.find('\n')).unwrap_or(after_path.len());
    let output_path = after_path[..path_end].trim().trim_end_matches('.');
    (!task_id.is_empty() && !output_path.is_empty()).then(|| BackgroundRef { task_id: task_id.to_string(), output_path: output_path.to_string() })
}

/// Whether Forge may read this file as a task's output: absolute, no `..`, no NUL, and exactly
/// `…/tasks/<task_id>.output`.
pub fn valid_output_path(path: &str, task_id: &str) -> bool {
    if task_id.is_empty() || !task_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if !path.starts_with('/') || path.contains('\0') || path.split('/').any(|part| part == "..") {
        return false;
    }
    path.ends_with(&format!("/tasks/{task_id}.output"))
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
}

/// A card for each running task, in the SDK's order. The command, a better description and the
/// output file come from the Bash call whose result names the task; a task without such a call
/// (a sub-agent) keeps the SDK's description and has no command and no output.
pub fn task_cards(messages: &[Message], running: &[RunningTask]) -> Vec<TaskCard> {
    let started: Vec<(BackgroundRef, &forge_domain::ToolCall)> = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter_map(|call| {
            let reference = background_ref(&call.result.as_ref()?.output)?;
            Some((reference, call))
        })
        .collect();
    running
        .iter()
        .map(|task| {
            let call = started.iter().find(|(reference, _)| reference.task_id == task.info.id);
            let command = call.and_then(|(_, call)| call.input.get("command")).and_then(|value| value.as_str()).map(str::to_string);
            let description = call
                .and_then(|(_, call)| call.input.get("description"))
                .and_then(|value| value.as_str())
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| task.info.description.clone());
            let output_path = call
                .map(|(reference, _)| reference.output_path.clone())
                .filter(|path| valid_output_path(path, &task.info.id));
            TaskCard { id: task.info.id.clone(), description, task_type: task.info.task_type.clone(), command, output_path, seen_at: task.seen_at }
        })
        .collect()
}

/// The pill under the chat: `1 background task`, `2 background tasks`.
pub fn pill_label(count: usize) -> String {
    format!("{count} background task{}", if count == 1 { "" } else { "s" })
}
```

- [ ] **Step 4: Run and commit**

Run: `cargo test -p forge-entities 2>&1 | grep -E "FAILED|panicked|left:|right:|^error|test result" -A3 | head -16`
Expected: all pass. Note on `a_card_without_a_description_falls_back...`: the plan's rule is Bash description, else the SDK's text (the command is shown in its own box).
```bash
git add -A crates
git commit -m "feat: rules for background task cards, output paths and the pill

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Opening a file in the Files panel, and the markdown mode

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/files_panel.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `relative_to_folder`, `is_markdown`, `render_markdown` (forge-entities).
- Produces: `Workspace::open_file(id, absolute_path: String)`, `Workspace::open_file_in_active(absolute_path: String)`, `Workspace::file_request(id) -> Option<String>`, `Workspace::clear_file_request(id)`; CSS classes `file-markdown`, `files-source-toggle`.

- [ ] **Step 1: Write the failing class test**

In `theme.rs`'s class list add `".file-markdown"`, `".files-source-toggle"`, `".file-link"`, `".file-name-link"`. Run `cargo test -p forge-shared the_stylesheet_defines 2>&1 | grep missing` → FAIL `missing .file-markdown`.

- [ ] **Step 2: Workspace**

In `workspace.rs` add the field `file_requests: Signal<HashMap<LiveSessionId, String>>,` to `Workspace` (next to `docks`; make sure `HashMap` is imported: it is used by other fields in the crate; add `use std::collections::HashMap;` if missing) and `file_requests: Signal::new(HashMap::new()),` in the constructor (next to `notice: Signal::new(None),`). Add methods next to `change_dock`:

```rust
    /// Opens a file in the session's Files panel. The path must be absolute and inside the session
    /// folder; anything else only sets a notice. The panel opens if it is closed and shows the file
    /// once it is mounted.
    pub fn open_file(&self, id: LiveSessionId, absolute_path: String) {
        let Some(view) = self.live_view(id) else { return };
        let folder = view.read().project_path.clone();
        let Some(relative) = forge_entities::relative_to_folder(&folder, &absolute_path) else {
            let mut notice = self.notice;
            notice.set(Some("Only files inside the session folder can be opened here.".to_string()));
            return;
        };
        self.change_dock(id, |dock| {
            if !dock.is_open(Panel::Files) {
                dock.toggle(Panel::Files);
            }
        });
        let mut requests = self.file_requests;
        requests.write().insert(id, relative);
    }

    /// The same for the session on screen (a click in the page does not know its session).
    pub fn open_file_in_active(&self, absolute_path: String) {
        if let Active::Live(id) = self.active() {
            self.open_file(id, absolute_path);
        }
    }

    /// The file a click asked the Files panel of this session to show, if any.
    pub fn file_request(&self, id: LiveSessionId) -> Option<String> {
        self.file_requests.read().get(&id).cloned()
    }

    pub fn clear_file_request(&self, id: LiveSessionId) {
        let mut requests = self.file_requests;
        requests.write().remove(&id);
    }
```
(`forge_entities` is already a dependency of `forge-features`; check `Cargo.toml`, add `forge-entities = { path = "../forge-entities" }` only if missing. `Panel` is defined in the same crate (`crate::dock::Panel`); import it if not in scope.)

- [ ] **Step 3: FilesPanel: serve the request and the markdown mode**

In `files_panel.rs`:
- imports: `use forge_entities::{code_html, is_markdown, language_of, render_markdown};`
- after `let mut hits = ...` add `let mut source = use_signal(|| false);`
- in `open_file`, add `source.set(false);` as its first line inside the closure body (next to `opened.set(...)`).
- add, after the `open_file` closure is defined and before `let tools = ...`:

```rust
    // A click in the chat asked for a file: show it (the request is read when the panel mounts, too).
    {
        let mut open_file = open_file.clone();
        use_effect(move || {
            if let Some(path) = workspace.file_request(id) {
                workspace.clear_file_request(id);
                open_file(path);
            }
        });
    }
```
- extend the `shown` memo to also carry the rendered markdown: change its body so the tuple is `(path, html, numbers, rendered)` where `rendered: Option<String>` is `is_markdown(&path).then(|| render_markdown(&text))`.
- extend `tools` with the switch (inside the same `rsx!`, after the `☰` button):

```rust
        if opened().as_deref().is_some_and(is_markdown) {
            button {
                class: "icon-button files-source-toggle",
                r#type: "button",
                title: if source() { "Show rendered" } else { "Show source" },
                "aria-label": if source() { "Show rendered" } else { "Show source" },
                onclick: move |_| source.set(!source()),
                if source() { "👁" } else { "</>" }
            }
        }
```
- in the viewer, for `Some((path, html, numbers, rendered))`: when `rendered` is `Some(markdown)` and `!source()` draw `div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{markdown}" }`, otherwise the existing gutter + code view.

- [ ] **Step 4: Styles**

```css
.file-markdown { padding: 12px 16px; overflow: auto; display: block; }
.files-source-toggle { font-family: var(--mono); font-size: 12px; }
.file-link { cursor: pointer; color: var(--link); text-decoration: underline; text-underline-offset: 2px; }
.file-link code { color: inherit; }
.file-name-link { cursor: pointer; text-decoration: underline; text-underline-offset: 2px; text-decoration-color: transparent; }
.file-name-link:hover { text-decoration-color: currentColor; }
```
(The existing `.file-viewer` rule may set a grid for the gutter; the `.file-markdown` rule must override it with `display: block`.)

- [ ] **Step 5: Run and commit**

Run: `cargo build --workspace 2>&1 | grep -E "^error" -A9 | head -30; cargo test --workspace > /tmp/t6c.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t6c.txt | head -5; grep -E "^test result" /tmp/t6c.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: builds; `failed: 0`.
```bash
git add -A crates
git commit -m "feat: open a file in the Files panel from a request, and render markdown with a source switch

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Clickable file names in the chat and the link listener

**Files:**
- Modify: `crates/forge-entities/src/turn.rs`, `crates/forge-widgets/src/chat_window.rs`

**Interfaces:**
- Consumes: Task 4 (`known_paths`, `render_markdown_linked`), Task 6 (`Workspace::open_file`, `open_file_in_active`).
- Produces: `Turns { messages, view, live, on_open_file: EventHandler<String> }`; `WATCH_FILE_LINKS` const in `chat_window.rs`.

- [ ] **Step 1: Write the failing test (the listener script)**

In `chat_window.rs`'s test module add:

```rust
    #[test]
    fn the_file_link_listener_replaces_its_old_self_so_a_new_page_scope_is_the_one_that_hears_clicks() {
        use super::WATCH_FILE_LINKS;
        // `dioxus.send` belongs to the eval that installed the listener; when the chat is shown
        // again a new eval must take over, so the old handler is removed first.
        assert!(WATCH_FILE_LINKS.contains("removeEventListener"), "{WATCH_FILE_LINKS}");
        assert!(WATCH_FILE_LINKS.contains(".file-link"));
        assert!(WATCH_FILE_LINKS.contains("dioxus.send"));
        assert!(WATCH_FILE_LINKS.contains("dataset.path"));
    }
```
Run: `cargo test -p forge-widgets file_link_listener 2>&1 | grep -E "^error" -A3 | head -5` → compile error RED (const missing).

- [ ] **Step 2: Implement the listener and the handler**

In `chat_window.rs` add:

```rust
// A click on a file name in an answer: the page sends the path to Rust. The handler is replaced each
// time (`dioxus.send` belongs to the eval that created it).
const WATCH_FILE_LINKS: &str = "if (window.__forgeFileLinkHandler) { document.removeEventListener('click', window.__forgeFileLinkHandler); } window.__forgeFileLinkHandler = (event) => { const link = event.target && event.target.closest ? event.target.closest('.file-link') : null; if (link && link.dataset.path) { dioxus.send(link.dataset.path); } }; document.addEventListener('click', window.__forgeFileLinkHandler);";
```
and in `ChatWindow`, after the `let workspace = use_workspace();` line:

```rust
    use_future(move || async move {
        let mut links = document::eval(WATCH_FILE_LINKS);
        while let Ok(path) = links.recv::<String>().await {
            workspace.open_file_in_active(path);
        }
    });
```
(The hook must run before any early return; place it directly after `use_workspace()`, before the `let Some(view_signal) = ... else { return ... }`.) Pass the opener to `Turns`: `on_open_file: move |path: String| workspace.open_file(id, path),`.

- [ ] **Step 3: Turns**

In `turn.rs`:
- imports: `use crate::links::{known_paths, render_markdown_linked};`, remove the `render_markdown` import if unused.
- `Turns(messages, view, live, on_open_file: EventHandler<String>)`: compute `let known = known_paths(&messages);` and pass `known: known.clone(), on_open_file` to each `TurnItem`.
- `TurnItem(item, detailed, known: Vec<String>, on_open_file: EventHandler<String>)`: the `Item::Text` arm uses `render_markdown_linked(&text, &known)`; pass `on_open_file` to `ActivityBlock` and `FileCardView`.
- `FileCardView(card, on_open_file)`: make each name a button:

```rust
                    button {
                        class: "file-name file-name-link",
                        r#type: "button",
                        title: "Open {file.path}",
                        onclick: {
                            let path = file.path.clone();
                            move |_| on_open_file.call(path.clone())
                        },
                        "{file.name}"
                    }
```
(replacing the `span { class: "file-name", ... }` of a single file row; the multi-file header row keeps its span.)
- `ActivityBlock` → `ActivityRow` → `ToolRowView` get `on_open_file`; in `ToolRowView` a file row's subject becomes a link that does not toggle the row:

```rust
                if !tool.path.is_empty() {
                    span {
                        class: "row-subject file-name-link",
                        onclick: {
                            let path = tool.path.clone();
                            move |event: MouseEvent| {
                                event.stop_propagation();
                                on_open_file.call(path.clone());
                            }
                        },
                        "{tool.subject}"
                    }
                } else {
                    span { class: "row-subject", "{tool.subject}" }
                }
```
(replacing the existing `span { class: "row-subject", "{tool.subject}" }`).

- [ ] **Step 4: Run and commit**

Run: `cargo build --workspace 2>&1 | grep -E "^error" -A9 | head -30; cargo test --workspace > /tmp/t7c.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t7c.txt | head -5; grep -E "^test result" /tmp/t7c.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: builds; `failed: 0`.
```bash
git add -A crates
git commit -m "feat: file names in the chat open the file in the Files panel

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The Tasks panel, the pill and the stop button

**Files:**
- Modify: `crates/forge-features/src/dock.rs`, `crates/forge-entities/src/turn.rs` (export `Clock`), `crates/forge-entities/src/lib.rs`
- Create: `crates/forge-widgets/src/tasks_panel.rs`
- Modify: `crates/forge-widgets/src/dock.rs`, `lib.rs`, `chat_window.rs`
- Modify: `crates/forge-shared/src/theme.css`, `theme.rs`

**Interfaces:**
- Consumes: Task 2 (`background_tasks` view field, `Workspace::stop_task`), Task 5 (`task_cards`, `pill_label`, `TaskCard`, `TASK_OUTPUT_TAIL`), Task 3 (`FilesPort::read_tail` via `workspace.files()`).
- Produces: `Panel::Tasks` (title `Background tasks`); `#[component] pub fn TasksPanel(id)`; pub `Clock` in forge-entities.

- [ ] **Step 1: Write the failing tests**

In `forge-features/src/dock.rs` tests:

```rust
    #[test]
    fn tasks_open_in_their_own_column_and_close_again() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Tasks);
        assert_eq!(dock.columns.len(), 2);
        assert_eq!(dock.columns[1].panels, vec![Panel::Tasks]);
        assert!(dock.is_open(Panel::Tasks));
        assert_eq!(Panel::Tasks.title(), "Background tasks");
        dock.toggle(Panel::Tasks);
        assert!(!dock.is_open(Panel::Tasks));
        assert_eq!(dock.columns.len(), 1);
    }
```
and add to `theme.rs`'s list: `".task-pill"`, `".task-card"`, `".task-output"`, `".task-stop"`. Run `cargo build --workspace --tests 2>&1 | grep -E "^error" | head -3` → RED (`Panel::Tasks` missing).

- [ ] **Step 2: Implement the panel kind**

`dock.rs` (features): add `Tasks,` to `Panel`; `Panel::Tasks => "Background tasks",` to `title`; in `partner`, `Panel::Terminal | Panel::Files | Panel::Tasks => None`. Fix other exhaustive matches the compiler reports (the widget `dock.rs` match: next step).

`turn.rs`: change `fn Clock(` to `pub fn Clock(` and export it: `pub use turn::{Clock, Turns};` in `lib.rs`.

- [ ] **Step 3: The widget**

Create `tasks_panel.rs`:

```rust
use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{task_cards, Clock, TaskCard, TASK_OUTPUT_TAIL};
use forge_features::{run_blocking, use_workspace, Panel};

use crate::panel_frame::PanelFrame;

/// The background tasks of a session: a card each with its command, a clock, the tail of its output
/// and a stop button.
#[component]
pub fn TasksPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let cards = {
        let view = view.read();
        task_cards(&view.session.messages, &view.background_tasks)
    };
    rsx! {
        PanelFrame { id, panel: Panel::Tasks, tools: rsx! {},
            div { class: "tasks-body",
                div { class: "tasks-heading", "Running" }
                if cards.is_empty() {
                    div { class: "dock-note", "No running tasks" }
                }
                for card in cards {
                    TaskCardView { key: "{card.id}", id, card }
                }
                div { class: "dock-note", "Tasks end when Forge closes the session." }
            }
        }
    }
}

#[component]
fn TaskCardView(id: LiveSessionId, card: TaskCard) -> Element {
    let workspace = use_workspace();
    let mut output = use_signal(String::new);
    let path = card.output_path.clone();
    use_future(move || {
        let path = path.clone();
        async move {
            let Some(path) = path else { return };
            loop {
                let files = workspace.files();
                let target = PathBuf::from(&path);
                if let Some(Ok(text)) = run_blocking(move || files.read_tail(&target, TASK_OUTPUT_TAIL)).await {
                    if output.peek().as_str() != text {
                        output.set(text);
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });
    let task_id = card.id.clone();
    let text = output();
    rsx! {
        div { class: "task-card",
            div { class: "task-head",
                div { class: "task-title",
                    div { class: "task-description", "{card.description}" }
                    div { class: "task-meta",
                        span { "Bash" }
                        Clock { started: Some(card.seen_at) }
                    }
                }
                button {
                    class: "task-stop",
                    r#type: "button",
                    title: "Stop task",
                    "aria-label": "Stop task",
                    onclick: move |_| workspace.stop_task(id, task_id.clone()),
                    span { class: "task-stop-square" }
                }
            }
            if let Some(command) = card.command.clone() {
                div { class: "task-command", "$ {command}" }
            }
            if card.output_path.is_some() {
                if text.trim().is_empty() {
                    div { class: "task-output empty", "No output yet" }
                } else {
                    pre { class: "task-output", "{text}" }
                }
            }
        }
    }
}
```
The `use_future` must not run the loop for a card without an output path (it returns at once). `run_blocking` and `Panel` are exported by `forge_features` (check `lib.rs`; both are used by `files_panel.rs`).

`widgets/dock.rs`: add `use crate::tasks_panel::TasksPanel;` and the arm `Panel::Tasks => rsx! { TasksPanel { key: "{panel:?}", id } },`. `widgets/lib.rs`: `mod tasks_panel;`.

- [ ] **Step 4: The pill in the chat**

In `chat_window.rs`, add imports `use forge_entities::{pill_label, task_cards, ...}` and `use forge_features::Panel;`. After the `Turns { ... }` call inside the `messages` div (so it scrolls with the transcript) add:

```rust
                {
                    let count = view.background_tasks.len();
                    let open = workspace.dock(id).is_open(Panel::Tasks);
                    rsx! {
                        if count > 0 {
                            button {
                                class: if open { "task-pill open" } else { "task-pill" },
                                r#type: "button",
                                onclick: move |_| workspace.change_dock(id, |dock| if !dock.is_open(Panel::Tasks) { dock.toggle(Panel::Tasks) }),
                                "{pill_label(count)}"
                            }
                        }
                    }
                }
```
(The pill counts the SDK's set; the panel builds the cards. `task_cards` need not be imported in `chat_window.rs`.)

- [ ] **Step 5: Styles**

```css
.task-pill { appearance: none; align-self: flex-start; margin-top: 10px; padding: 4px 12px; border: none; border-radius: 999px; background: rgba(29, 95, 191, 0.12); color: var(--link); font: inherit; font-size: 13px; cursor: pointer; }
.task-pill.open { background: var(--link); color: #fff; }
.tasks-body { display: flex; flex-direction: column; gap: 10px; padding: 12px; overflow-y: auto; }
.tasks-heading { color: var(--fg-muted); font-size: 12px; text-transform: uppercase; letter-spacing: 0.04em; }
.task-card { display: flex; flex-direction: column; gap: 8px; padding: 12px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); }
.task-head { display: flex; align-items: flex-start; gap: 8px; }
.task-title { flex: 1; min-width: 0; }
.task-description { font-weight: 700; overflow-wrap: anywhere; }
.task-meta { display: flex; gap: 10px; color: var(--fg-muted); font-size: 12px; }
.task-stop { appearance: none; flex: none; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border: 1px solid var(--border); border-radius: 8px; background: none; cursor: pointer; }
.task-stop:hover { background: var(--bg-hover); }
.task-stop-square { width: 10px; height: 10px; border-radius: 2px; background: var(--fg); }
.task-command { padding: 8px 10px; background: var(--code-bg); border-radius: 8px; font-family: var(--mono); font-size: 12.5px; overflow-wrap: anywhere; white-space: pre-wrap; }
.task-output { margin: 0; padding: 8px 10px; max-height: 240px; overflow: auto; background: var(--code-bg); border-radius: 8px; font-family: var(--mono); font-size: 12px; white-space: pre-wrap; overflow-wrap: anywhere; }
.task-output.empty { font-family: inherit; color: var(--fg-muted); }
```

- [ ] **Step 6: Run and commit**

Run: `cargo build --workspace 2>&1 | grep -E "^error" -A9 | head -30; cargo test --workspace > /tmp/t8c.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t8c.txt | head -5; grep -E "^test result" /tmp/t8c.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: builds; `failed: 0`.
```bash
git add -A crates
git commit -m "feat: the Background tasks panel, its pill in the chat and a stop button per task

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The suggested next prompt in the message box

**Files:**
- Create: `crates/forge-features/src/suggestion.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/src/send_message.rs`

**Interfaces:**
- Produces:
  ```rust
  pub fn composer_placeholder(suggestion: Option<&str>, draft_empty: bool, running: bool, default: &str) -> String;
  pub fn suggestion_to_accept(code: &str, suggestion: Option<&str>, draft_empty: bool, running: bool) -> Option<String>;
  ```

- [ ] **Step 1: Write the failing tests**

`suggestion.rs` — create with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "Message Claude…";

    #[test]
    fn the_placeholder_is_the_suggestion_only_when_the_box_is_empty_and_nothing_runs() {
        assert_eq!(composer_placeholder(Some("run the tests"), true, false, DEFAULT), "run the tests");
        assert_eq!(composer_placeholder(Some("run the tests"), false, false, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(Some("run the tests"), true, true, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(None, true, false, DEFAULT), DEFAULT);
    }

    #[test]
    fn a_blank_suggestion_is_not_shown() {
        assert_eq!(composer_placeholder(Some("   "), true, false, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(Some(""), true, false, DEFAULT), DEFAULT);
    }

    #[test]
    fn tab_in_an_empty_box_takes_the_suggestion() {
        assert_eq!(suggestion_to_accept("Tab", Some("run the tests"), true, false).as_deref(), Some("run the tests"));
    }

    #[test]
    fn no_other_key_and_no_other_state_takes_it() {
        assert_eq!(suggestion_to_accept("Enter", Some("x"), true, false), None);
        assert_eq!(suggestion_to_accept("Tab", Some("x"), false, false), None, "the box has text");
        assert_eq!(suggestion_to_accept("Tab", Some("x"), true, true), None, "a turn is running");
        assert_eq!(suggestion_to_accept("Tab", None, true, false), None);
        assert_eq!(suggestion_to_accept("Tab", Some("  "), true, false), None);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod suggestion;` and `pub use suggestion::{composer_placeholder, suggestion_to_accept};` to `lib.rs`. Run: `cargo test -p forge-features suggestion 2>&1 | grep -E "^error" | head -3` → RED.

- [ ] **Step 3: Implement**

Above the tests in `suggestion.rs`:

```rust
/// What the message box says when it is empty: the SDK's suggested next prompt while no turn runs,
/// else `default`.
pub fn composer_placeholder(suggestion: Option<&str>, draft_empty: bool, running: bool, default: &str) -> String {
    match usable(suggestion) {
        Some(text) if draft_empty && !running => text.to_string(),
        _ => default.to_string(),
    }
}

/// The text Tab puts in the box: the suggestion, when the box is empty and no turn runs. Tab is
/// Forge's own choice of key (the desktop app's was not observed).
pub fn suggestion_to_accept(code: &str, suggestion: Option<&str>, draft_empty: bool, running: bool) -> Option<String> {
    if code != "Tab" || !draft_empty || running {
        return None;
    }
    usable(suggestion).map(str::to_string)
}

fn usable(suggestion: Option<&str>) -> Option<&str> {
    suggestion.filter(|text| !text.trim().is_empty())
}
```

- [ ] **Step 4: Wire it into `SendMessageBox`**

In `send_message.rs`: `use crate::suggestion::{composer_placeholder, suggestion_to_accept};`. After `let view = workspace.live_view(id);` add `let suggestion = view.and_then(|view| view.read().suggestion.clone());`. Change `placeholder: "Message Claude…",` to `placeholder: composer_placeholder(suggestion.as_deref(), draft.read().is_empty(), running, "Message Claude…"),`. In the composer div's `onkeydown`, before the existing `⌘U` check, add:

```rust
                if let Some(text) = suggestion_to_accept(&event.code().to_string(), suggestion.as_deref(), draft.peek().is_empty(), running) {
                    event.prevent_default();
                    draft.set(text);
                    return;
                }
```
(`suggestion` must be cloned into the closure: `let accept = suggestion.clone();` before `rsx!` and use `accept.as_deref()` inside; the placeholder uses the original.) The `CommandTextArea` placeholder colour is the browser's grey already.

- [ ] **Step 5: Run and commit**

Run: `cargo test --workspace > /tmp/t9c.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t9c.txt | head -5; grep -E "^test result" /tmp/t9c.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`.
```bash
git add -A crates
git commit -m "feat: the message box suggests the next prompt and Tab takes it

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Look at it in the running app, compare, record

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-chat-view-panels-design.md` (an "As built" section), `docs/superpowers/roadmap/README.md`.

A session started from Forge runs on the real Claude and costs a real turn. **Ask the user once before the first send** ("I am going to send a few short messages from Forge in `/private/tmp/forge-observe`: write a markdown file, run a command in the background, and ask one more thing for the suggestion") and wait for yes. Only that folder.

How to type into Forge: press Ctrl+Space to leave the Korean input method, then type with `CGEventKeyboardSetUnicodeString` events (a small script, one event per character; see the spec 23B "Lessons"); Return is key code 36. Number keys from the harness do not arrive: do not test them.

- [ ] **Step 1: Build and start**

```bash
cd sidecar && npm run build 2>&1 | tail -2 && cd ..
cargo build -p forge-app 2>&1 | tail -1
mkdir -p /private/tmp/forge-cmp-config
(FORGE_CONFIG_PATH=/private/tmp/forge-cmp-config/config.json nohup target/debug/forge-app >/private/tmp/forge-cmp.log 2>&1 & echo $! >/private/tmp/forge-harness.pid)
sleep 7; python3 scripts/harness/capture.py | tail -1
```
Open a new session in the `forge-observe` folder.

- [ ] **Step 2: Files and markdown**

Send (mode: Ask before edits, allow with the mouse): `Write /private/tmp/forge-observe/panel1.md with a heading, a table of two columns and a short list, then answer in one line naming panel1.md in backticks.` After the turn: click the file name in the end-of-turn card, and the file name in the opened activity row, and the underlined `panel1.md` in the answer. Each opens the Files panel on the file, rendered; `</>` switches to the source and back to the rendering. Click a name of a file outside the folder if one is on screen (a Read of `/etc/hosts` makes one): the notice appears and no panel opens.

- [ ] **Step 3: Background task**

Send: `Run "sleep 25; echo finished" with Bash in the background (run_in_background), then reply "started".` Check: the pill `1 background task` under the transcript; clicking it opens `Background tasks` with the description, `Bash` and a running clock, `$ sleep 25; echo finished`, `No output yet`, then `finished` after about 25 s and the card disappears. Start another `sleep 60` the same way, press the turn's stop button while it runs, then press the card's stop button: the task survives the first and ends with the second (the pill disappears).

- [ ] **Step 4: Suggestion**

After a finished turn (not the first of the session) the empty message box shows a grey suggestion; Tab fills it; typing replaces it; sending clears it. If the SDK shows none (it may refuse after errors or near the limit), say so.

- [ ] **Step 5: Stop the app, clean up, record**

```bash
bash scripts/harness/stop.sh; pkill -f "plan-23/target/debug/forge-app"; rm -rf /private/tmp/forge-cmp-config
```
Append an "As built" section to the 23C spec (what matched, what differs, what is left, lessons); mark 23C built in `roadmap/README.md`. Run both suites:

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'; (cd sidecar && npm test 2>&1 | tail -4)`
Expected: `failed: 0`; sidecar passes.

- [ ] **Step 6: Commit**

```bash
git add docs
git commit -m "docs: Plan 23C as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:** §2 sidecar (events, `stop_task`, options, message cases) → Task 1; §3 ports, view fields, `read_tail`, use case → Tasks 2, 3; §4 opening files (pure rule, workspace, request, three click sites, delegated listener, known paths) → Tasks 4, 6, 7; §5 markdown mode → Task 6; §6 background tasks (parser, view model, pill, panel, stop, output tail, note) → Tasks 5, 8; §7 suggestion (placeholder, Tab, clear on send) → Tasks 2 (storage/clear) and 9; §8 testing → each task + Task 10; §9 risks → Task 10 and Review Focus. The spec's "stored while idle" is refined to "stored always, shown when idle" (Global Constraints).
**Placeholder scan:** none; the places that depend on exact existing code are named with what to do (`LocalFiles` construction in tests, `escape_html` visibility, exports of `run_blocking`/`Panel`, `forge-entities` dependency in `forge-features`).
**Type consistency:** `BackgroundTaskInfo`/`RunningTask` (Task 2) are used in Tasks 5, 8; `Call::StopTask`, `stop_task` signatures match across Tasks 1, 2, 8; `read_tail(path, max)` (Task 3) is called in Task 8; `TaskCard`, `task_cards`, `pill_label`, `TASK_OUTPUT_TAIL` (Task 5) in Task 8; `relative_to_folder`, `known_paths`, `render_markdown_linked`, `is_markdown` (Tasks 4) in Tasks 6, 7; `Turns(.., on_open_file)` (Task 7) matches the ChatWindow call; `composer_placeholder`/`suggestion_to_accept` (Task 9).
**Review focus:** outside/sibling-prefix paths → Task 4 tests + Task 6 notice; sub-agent card → Task 5 test; missing/empty/`..` output → Tasks 3, 5 tests and `No output yet`; fenced/ambiguous names → Task 4 tests; stop for a finished task → Task 1 test.
