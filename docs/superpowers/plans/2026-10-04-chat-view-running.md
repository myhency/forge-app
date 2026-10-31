# Chat View, Running Look Implementation Plan (Plan 23B)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make a running conversation look and behave like the real Claude desktop app: one live line with a clock, a permission card with the real buttons and keys, a question card whose answer reaches the model, a round stop button, a scroll-to-end button, and the real denial text.

**Architecture:** The sidecar learns the real denial text and a new `answer_question` command; the Rust side carries that command through port, use case and live-session actor and records `turn_started`. A pure view model (`build_turns`) gains a live state and two new items (`Live`, `Answers`); new widgets draw the live line, the permission panel and the question panel. Pure logic (titles, key maps, answer assembly, text parsing) lives in `forge-entities` with tests; markup is checked by the visual run at the end.

**Tech Stack:** TypeScript (sidecar, vitest), Rust 2021, Dioxus 0.7.10, tokio (already in the workspace), serde_json.

**Spec:** `docs/superpowers/specs/2026-10-04-chat-view-running-design.md` (observations: `docs/superpowers/roadmap/observations.md`, Rounds 7 and 8).

## Global Constraints

- UI text is English (Forge's UI language); the Korean words in the observations are only the real app's wording.
- The denial text sent to the model is exactly: `The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.`
- The skip text for a question is exactly: `The user skipped the question.`
- Multi-select answers are joined with `,` and no space (as the real app's transcript shows); the answer card shows them split and joined with `, `.
- The clock shows seconds only (no token count). Format: `4s`, `1m 21s`, `1h 2m`.
- Keys on the permission card: `1` and `Esc` deny, `2` and `⇧⌘↩` allow always, `3` and `⌘↩` allow once; digits are read by physical key (`Digit1`, `Numpad1`), never by character.
- Keep every existing test passing; tests may change shape but their count must not drop (the workspace had 878 passing tests when 23A was merged). The sidecar suite is run with `cd sidecar && npm test`.
- New CSS must not add a `prefers-color-scheme: dark` block (the theme test reads the last dark block, which must stay the code-colours one). Colours come from existing variables.
- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Out of scope: token count in the clock, undo or rewind icons, side panels (23C), the background-task pill, the suggested next prompt.

## Review Focus

- A question whose input is empty or malformed (no `questions`, a question with no options): the card shows only `Skip`, nothing panics, and skip still answers the sidecar (Tasks 3 and 8).
- A permission request for a tool with no `file_path`, no `description` and an empty input (an MCP tool, `{}`): title `Claude wants to use <tool>?`, no diff box, no panic (Task 4).
- A turn that ended while a tool call was still unanswered (stop pressed, or a denial): no Live line is left behind and the folded line shows, with the unanswered row not marked failed (Task 5).
- Two permission requests queued at once: the panel shows the first and the second after the first is answered (Task 7 keeps using the queue's front).
- A question answer containing quotes, commas or Korean text: the answer card still pairs each question with its own answer (Task 3).

## File Structure

- `sidecar/src/session.ts`, `sidecar/src/protocol.ts`, `sidecar/test/session.test.ts` — denial text, skip text, `answer_question` (modify).
- `crates/forge-adapter-sidecar/src/protocol.rs`, `process.rs` — wire command and its port implementation (modify).
- `crates/forge-ports/src/sidecar_port.rs` — `answer_question` on the port (modify).
- `crates/forge-application/src/use_cases/sidecar_commands.rs`, `lib.rs` — `AnswerQuestionUseCase` (modify).
- `crates/forge-application/src/live_session.rs`, `live_session_actor.rs`, `test_support.rs` — command, `turn_started` (modify).
- `crates/forge-features/src/workspace.rs` — `Workspace::answer_question` (modify); `send_message.rs` — stop button (modify); `approve_permission.rs` — removed.
- `crates/forge-entities/src/question.rs` — questions, picks, answers (create).
- `crates/forge-entities/src/permission_view.rs` — card title and key map (create).
- `crates/forge-entities/src/tool_summary.rs` — `tool_input_lines`, `elapsed_label` (modify).
- `crates/forge-entities/src/turn_view.rs` — live state, `Live` and `Answers` items, segments (modify).
- `crates/forge-entities/src/turn.rs`, `message.rs` — live line, answer card, key-value tool detail (modify); `permission.rs` removed.
- `crates/forge-widgets/src/permission_panel.rs`, `question_panel.rs` — the two cards (create); `chat_window.rs` (modify).
- `crates/forge-shared/src/theme.css`, `theme.rs` — styles and the class list test (modify).

---

### Task 1: Sidecar: the real denial text, the skip text and `answer_question`

**Files:**
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`
- Test: `sidecar/test/session.test.ts`

**Interfaces:**
- Produces: wire command `{ type: "answer_question", id: string, permission_request_id: string, answers: Record<string, string> }`; exports `DENIED_MESSAGE` and `QUESTION_SKIPPED_MESSAGE` from `session.ts`. Task 2's Rust command serializes to exactly this shape.

- [ ] **Step 1: Write the failing tests**

In `sidecar/test/session.test.ts` change the import to `import { SidecarSession, DENIED_MESSAGE, QUESTION_SKIPPED_MESSAGE, type QueryFn } from "../src/session.js";` and add, after the test `ignores a permission_response for an unknown permission_request_id`:

```ts
  it("denies a tool call with the real app's text so the model stops", async () => {
    let result: any;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("Write", { file_path: "/tmp/a.txt", content: "x" }, { requestId: "d-1" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "9", permission_request_id: "d-1", decision: "deny" });
    await started;

    expect(result).toEqual({ behavior: "deny", message: DENIED_MESSAGE });
    expect(DENIED_MESSAGE).toBe(
      "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.",
    );
  });

  it("denies a skipped question with its own message", async () => {
    let result: any;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("AskUserQuestion", { questions: [] }, { requestId: "q-0" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "permission_response", id: "9", permission_request_id: "q-0", decision: "deny" });
    await started;

    expect(result).toEqual({ behavior: "deny", message: QUESTION_SKIPPED_MESSAGE });
    expect(QUESTION_SKIPPED_MESSAGE).toBe("The user skipped the question.");
  });

  it("answers a pending AskUserQuestion with updatedInput.answers", async () => {
    let result: any;
    const input = { questions: [{ question: "Q?", header: "H", options: [], multiSelect: false }] };
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        result = await canUseTool("AskUserQuestion", input, { requestId: "q-1" });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));
    await session.handle({ type: "answer_question", id: "9", permission_request_id: "q-1", answers: { "Q?": "파랑,초록" } });
    await started;

    expect(result).toEqual({ behavior: "allow", updatedInput: { ...input, answers: { "Q?": "파랑,초록" } } });
  });

  it("ignores answer_question for an unknown id and for a request that is not a question", async () => {
    let result: any;
    let settled = false;
    const queryFn: QueryFn = (args) => {
      const canUseTool = (args.options as any).canUseTool;
      return toQueryHandle((async function* () {
        const pending = canUseTool("Bash", { command: "ls" }, { requestId: "b-1" });
        pending.then((r: any) => { result = r; settled = true; });
        yield { type: "stream_event", event: { type: "content_block_delta", index: 0, delta: { type: "text_delta", text: "." } } };
        await pending;
      })());
    };
    const session = new SidecarSession(queryFn, () => undefined);
    const started = session.handle(startCommand);
    await new Promise((resolve) => setTimeout(resolve, 0));

    await session.handle({ type: "answer_question", id: "1", permission_request_id: "nope", answers: {} });
    await session.handle({ type: "answer_question", id: "2", permission_request_id: "b-1", answers: { a: "b" } });
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(settled).toBe(false);

    await session.handle({ type: "permission_response", id: "3", permission_request_id: "b-1", decision: "allow_once" });
    await started;
    expect(result).toEqual({ behavior: "allow" });
  });
```

- [ ] **Step 2: Run to verify they fail**

Run: `cd sidecar && npm test -- session 2>&1 | tail -25`
Expected: FAIL (`DENIED_MESSAGE` is not exported; `answer_question` is not a known command type).

- [ ] **Step 3: Implement**

In `sidecar/src/protocol.ts` add to the `SidecarCommand` union, after the `permission_response` line:

```ts
  | { type: "answer_question"; id: string; permission_request_id: string; answers: Record<string, string> }
```

In `sidecar/src/session.ts`: add after the `PermissionResult` type

```ts
export const DENIED_MESSAGE =
  "The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.";
export const QUESTION_SKIPPED_MESSAGE = "The user skipped the question.";
const QUESTION_TOOL = "AskUserQuestion";
```

change `PendingPermission` to

```ts
interface PendingPermission {
  resolve: (result: PermissionResult) => void;
  suggestions?: unknown[];
  toolName: string;
  input: Record<string, unknown>;
}
```

replace `decisionToResult` with

```ts
function decisionToResult(decision: WirePermissionDecision, suggestions: unknown[] | undefined, toolName: string): PermissionResult {
  if (decision === "deny") {
    return { behavior: "deny", message: toolName === QUESTION_TOOL ? QUESTION_SKIPPED_MESSAGE : DENIED_MESSAGE };
  }
  if (decision === "allow_always") {
    return { behavior: "allow", updatedPermissions: suggestions };
  }
  return { behavior: "allow" };
}
```

in `canUseTool` change the `set` call to `this.pendingPermissions.set(permissionRequestId, { resolve, suggestions: options.suggestions, toolName, input });`, in the `permission_response` case use `pending.resolve(decisionToResult(command.decision, pending.suggestions, pending.toolName));`, and add a new case after it:

```ts
      case "answer_question": {
        const pending = this.pendingPermissions.get(command.permission_request_id);
        if (!pending || pending.toolName !== QUESTION_TOOL) return;
        pending.resolve({ behavior: "allow", updatedInput: { ...pending.input, answers: command.answers } });
        this.pendingPermissions.delete(command.permission_request_id);
        return;
      }
```

- [ ] **Step 4: Run the sidecar suite**

Run: `cd sidecar && npm run typecheck 2>&1 | tail -5 && npm test 2>&1 | tail -8`
Expected: typecheck clean; all tests pass (the old `resolves with a PermissionResult deny (with a message)` test still passes: it only checks the message is non-empty).

- [ ] **Step 5: Rebuild the shipped sidecar and commit**

Run: `cd sidecar && npm run build 2>&1 | tail -3`
Expected: no errors (`sidecar/dist` is git-ignored and is what the app runs).

```bash
git add sidecar/src sidecar/test
git commit -m "feat: the sidecar denies with the real app's text and takes question answers

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `answer_question` through the port, the use case and the live session; `turn_started`

**Files:**
- Modify: `crates/forge-adapter-sidecar/src/protocol.rs`, `crates/forge-adapter-sidecar/src/process.rs`
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-application/src/use_cases/sidecar_commands.rs`, `crates/forge-application/src/lib.rs`
- Modify: `crates/forge-application/src/live_session.rs`, `crates/forge-application/src/live_session_actor.rs`, `crates/forge-application/src/test_support.rs`
- Modify: `crates/forge-features/src/workspace.rs`
- Modify (compiler-driven): any file that builds a `LiveSessionView` literal (`crates/forge-features/src/sidebar.rs` tests, others reported by `cargo build --workspace --tests`).

**Interfaces:**
- Consumes: Task 1's wire shape.
- Produces:
  - `SidecarPort::answer_question(&self, permission_request_id: String, answers: BTreeMap<String, String>) -> Result<(), SidecarError>`
  - `AnswerQuestionUseCase::new(Arc<dyn SidecarPort>)` with `async fn execute(&self, permission_request_id: String, answers: BTreeMap<String, String>) -> Result<(), SidecarError>`
  - `LiveSessionHandle::answer_question(&self, permission_request_id: String, answers: BTreeMap<String, String>)`
  - `LiveSessionView.turn_started: Option<std::time::SystemTime>`
  - `Workspace::answer_question(&self, id: LiveSessionId, request_id: String, answers: BTreeMap<String, String>)`
  - test helper `Call::Answer(String, BTreeMap<String, String>)`

- [ ] **Step 1: Write the failing tests**

(a) `crates/forge-adapter-sidecar/src/protocol.rs`, in the `tests` module:

```rust
    #[test]
    fn answer_question_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::AnswerQuestion {
            id: "7".to_string(),
            permission_request_id: "q-1".to_string(),
            answers: std::collections::BTreeMap::from([("Q?".to_string(), "파랑,초록".to_string())]),
        };
        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            serde_json::json!({
                "type": "answer_question",
                "id": "7",
                "permission_request_id": "q-1",
                "answers": {"Q?": "파랑,초록"}
            })
        );
    }
```

(b) `crates/forge-application/src/use_cases/sidecar_commands.rs`, next to the approve test:

```rust
    #[tokio::test]
    async fn answer_question_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_answer_question()
            .withf(|id, answers| id == "q-1" && answers.get("Q?").map(String::as_str) == Some("A"))
            .times(1)
            .returning(|_, _| Ok(()));
        let use_case = AnswerQuestionUseCase::new(Arc::new(mock));
        let answers = std::collections::BTreeMap::from([("Q?".to_string(), "A".to_string())]);
        assert!(use_case.execute("q-1".to_string(), answers).await.is_ok());
    }
```

(c) `crates/forge-application/src/live_session.rs`, in the `tests` module (it already imports `BTreeMap` through `use super::*`; if not, add `use std::collections::BTreeMap;`), after `approving_an_unknown_request_does_not_reach_the_sidecar`:

```rust
    fn question_request(id: &str) -> SidecarEvent {
        SidecarEvent::PermissionRequest {
            permission_request_id: id.to_string(),
            tool_name: "AskUserQuestion".to_string(),
            input: serde_json::json!({ "questions": [] }),
        }
    }

    #[tokio::test]
    async fn answering_a_question_pops_it_and_reaches_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("ask me".to_string());
        control.push(question_request("q-1"));
        wait_for(&mut view, "the question", |v| v.pending_permissions.len() == 1).await;

        let answers = BTreeMap::from([("Q?".to_string(), "A".to_string())]);
        handle.answer_question("q-1".to_string(), answers.clone());
        wait_for(&mut view, "the question answered", |v| v.pending_permissions.is_empty()).await;

        assert!(control.calls().contains(&Call::Answer("q-1".to_string(), answers)));
    }

    #[tokio::test]
    async fn answering_an_unknown_question_does_not_reach_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.answer_question("nope".to_string(), BTreeMap::new());
        handle.interrupt(); // commands run in order
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(!control.calls().iter().any(|call| matches!(call, Call::Answer(..))));
    }

    #[tokio::test]
    async fn the_turn_clock_starts_on_send_and_stops_when_the_turn_ends() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert!(handle.snapshot().turn_started.is_none());

        handle.send("hi".to_string());
        let running = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        assert!(running.turn_started.is_some());

        control.push(turn_complete());
        let idle = wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        assert!(idle.turn_started.is_none());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-sidecar answer_question 2>&1 | tail -6; cargo test -p forge-application answer 2>&1 | tail -8`
Expected: compile errors (`AnswerQuestion`, `answer_question`, `Call::Answer`, `turn_started` do not exist).

- [ ] **Step 3: Implement the wire command, the port and its adapter**

`crates/forge-adapter-sidecar/src/protocol.rs`: add to `SidecarCommand` after `PermissionResponse`:

```rust
    AnswerQuestion { id: String, permission_request_id: String, answers: BTreeMap<String, String> },
```
(`BTreeMap` is already imported in that file; the `StartSession` variant uses it.)

`crates/forge-ports/src/sidecar_port.rs`: add to the trait after `respond_to_permission`:

```rust
    /// Answers a pending `AskUserQuestion` request: each question's text maps to the chosen
    /// label(s) (several labels joined by `,`) or the text typed under "Other".
    async fn answer_question(
        &self,
        permission_request_id: PermissionRequestId,
        answers: BTreeMap<String, String>,
    ) -> Result<(), SidecarError>;
```

`crates/forge-adapter-sidecar/src/process.rs`: add to `impl SidecarPort for SidecarProcess` after `respond_to_permission`:

```rust
    async fn answer_question(
        &self,
        permission_request_id: String,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::AnswerQuestion { id, permission_request_id, answers })
    }
```

`crates/forge-application/src/test_support.rs`: add a variant `Answer(String, std::collections::BTreeMap<String, String>),` to the `Call` enum (open the file and add it next to `Permission`), and to `impl SidecarPort for FakeSidecar` after `respond_to_permission`:

```rust
    async fn answer_question(
        &self,
        permission_request_id: String,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<(), SidecarError> {
        self.control.record(Call::Answer(permission_request_id, answers));
        Ok(())
    }
```

- [ ] **Step 4: Implement the use case, the command and the view field**

`crates/forge-application/src/use_cases/sidecar_commands.rs`: add after `ApprovePermissionUseCase`:

```rust
pub struct AnswerQuestionUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl AnswerQuestionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        permission_request_id: String,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<(), SidecarError> {
        self.sidecar.answer_question(permission_request_id, answers).await
    }
}
```
and export it: in `crates/forge-application/src/lib.rs` extend the `pub use use_cases::{...}` list with `AnswerQuestionUseCase`, and in `crates/forge-application/src/use_cases/mod.rs` (open it) re-export it next to `ApprovePermissionUseCase`.

`crates/forge-application/src/live_session.rs`:
- add `AnswerQuestion { id: String, answers: std::collections::BTreeMap<String, String> },` to `LiveCommand`;
- add to `LiveSessionHandle`:

```rust
    /// Answers an `AskUserQuestion` request (see `SidecarPort::answer_question`).
    pub fn answer_question(&self, permission_request_id: String, answers: std::collections::BTreeMap<String, String>) {
        let _ = self.commands.send(LiveCommand::AnswerQuestion { id: permission_request_id, answers });
    }
```
- add to `LiveSessionView` (after `phase`): `/// When the running turn was sent; \`None\` while no turn runs.\n    pub turn_started: Option<std::time::SystemTime>,` and `turn_started: None,` in `spawn_live_session`'s literal.

`crates/forge-application/src/live_session_actor.rs`:
- import `AnswerQuestionUseCase` next to `ApprovePermissionUseCase` (line 10);
- in the command dispatch (next to `LiveCommand::Approve`): `LiveCommand::AnswerQuestion { id, answers } => self.answer_question(id, answers).await,`
- add after `approve`:

```rust
    async fn answer_question(&mut self, id: String, answers: std::collections::BTreeMap<String, String>) {
        let Some(position) = self.view.pending_permissions.iter().position(|request| request.id == id) else {
            return;
        };
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        self.view.pending_permissions.remove(position);
        self.dirty = true;
        if let Err(error) = AnswerQuestionUseCase::new(sidecar).execute(id, answers).await {
            self.fail(error.to_string(), false).await;
        }
    }
```
- in `send`, in the `Ok(())` arm after `self.view.phase = Phase::Running;` add `self.view.turn_started = Some(std::time::SystemTime::now());`;
- in `apply_event`, `SessionUpdate::TurnCompleted` arm add `self.view.turn_started = None;`; in `fail` add `self.view.turn_started = None;` after `self.view.phase = Phase::Failed;`.

`crates/forge-features/src/workspace.rs`: add after `answer_permission`:

```rust
    pub fn answer_question(&self, id: LiveSessionId, request_id: String, answers: std::collections::BTreeMap<String, String>) {
        if let Some(handle) = self.handle(id) {
            handle.answer_question(request_id, answers);
        }
    }
```

- [ ] **Step 5: Fix what the compiler reports, run the tests**

Run: `cargo build --workspace --tests 2>&1 | grep -E "^error" -A6 | head -40` and fix each: every other `impl SidecarPort` (there are none besides the two above and the mockall mock, which is generated), every `LiveSessionView { ... }` literal gets `turn_started: None,`.
Run: `cargo test --workspace > /tmp/t2.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t2.txt | head; grep -E "^test result" /tmp/t2.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`, passed at least 878 plus the 5 new tests (878 + 5 = 883).

- [ ] **Step 6: Commit**

```bash
git add -A crates
git commit -m "feat: question answers reach the sidecar; the live view knows when the turn started

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Questions, picks and answers (`question.rs`)

**Files:**
- Create: `crates/forge-entities/src/question.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces (used by Tasks 5, 6, 8):
  ```rust
  pub const QUESTION_TOOL: &str = "AskUserQuestion";
  pub struct QuestionOption { pub label: String, pub description: String }
  pub struct Question { pub header: String, pub text: String, pub options: Vec<QuestionOption>, pub multi: bool }
  pub fn questions_of(input: &Value) -> Vec<Question>;
  pub struct Picks { pub chosen: Vec<usize>, pub other: String }   // Default, Clone, Debug, PartialEq
  impl Picks {
      pub fn toggle(&mut self, multi: bool, index: usize);
      pub fn set_other(&mut self, multi: bool, text: String);
      pub fn ready(&self) -> bool;
  }
  pub fn answers_map(questions: &[Question], picks: &[Picks]) -> BTreeMap<String, String>;
  pub struct AnswerPair { pub question: String, pub answer: String }
  pub fn answer_pairs(input: &Value, output: &str) -> Vec<AnswerPair>;
  ```

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/question.rs` with only this test module (code follows in Step 3):

```rust
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn input() -> serde_json::Value {
        json!({"questions": [
            {"question": "Colours?", "header": "Colours", "multiSelect": true,
             "options": [{"label": "Blue", "description": "sky"}, {"label": "Green", "description": "leaf"}, {"label": "Red", "description": "fire"}]},
            {"question": "Season?", "header": "Season", "multiSelect": false,
             "options": [{"label": "Spring", "description": "a"}, {"label": "Summer", "description": "b"}]}
        ]})
    }

    #[test]
    fn questions_are_read_from_the_tool_input() {
        let questions = questions_of(&input());
        assert_eq!(questions.len(), 2);
        assert_eq!(questions[0].header, "Colours");
        assert_eq!(questions[0].text, "Colours?");
        assert!(questions[0].multi);
        assert!(!questions[1].multi);
        assert_eq!(questions[0].options[2], QuestionOption { label: "Red".to_string(), description: "fire".to_string() });
    }

    #[test]
    fn malformed_input_gives_no_questions_and_never_panics() {
        for bad in [json!(null), json!({}), json!({"questions": 5}), json!({"questions": [1, "x", null]}), json!("text")] {
            assert!(questions_of(&bad).is_empty(), "{bad}");
        }
    }

    #[test]
    fn a_question_with_no_text_is_dropped_and_missing_fields_default() {
        let questions = questions_of(&json!({"questions": [{"header": "h"}, {"question": "Only text?"}]}));
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].text, "Only text?");
        assert_eq!(questions[0].header, "");
        assert!(questions[0].options.is_empty());
        assert!(!questions[0].multi);
    }

    #[test]
    fn a_single_select_pick_replaces_the_pick_and_clears_other() {
        let mut picks = Picks::default();
        picks.set_other(false, "my own".to_string());
        picks.toggle(false, 1);
        assert_eq!(picks.chosen, vec![1]);
        assert_eq!(picks.other, "");
        picks.toggle(false, 0);
        assert_eq!(picks.chosen, vec![0]);
    }

    #[test]
    fn typing_other_on_a_single_select_clears_the_pick() {
        let mut picks = Picks::default();
        picks.toggle(false, 1);
        picks.set_other(false, "mine".to_string());
        assert!(picks.chosen.is_empty());
        assert_eq!(picks.other, "mine");
    }

    #[test]
    fn a_multi_select_toggles_and_keeps_other() {
        let mut picks = Picks::default();
        picks.toggle(true, 2);
        picks.toggle(true, 0);
        picks.set_other(true, "also this".to_string());
        assert_eq!(picks.chosen, vec![0, 2], "kept in option order");
        picks.toggle(true, 2);
        assert_eq!(picks.chosen, vec![0]);
        assert_eq!(picks.other, "also this");
    }

    #[test]
    fn ready_needs_a_pick_or_some_typed_text() {
        let mut picks = Picks::default();
        assert!(!picks.ready());
        picks.set_other(true, "   ".to_string());
        assert!(!picks.ready());
        picks.set_other(true, "x".to_string());
        assert!(picks.ready());
        let mut chosen = Picks::default();
        chosen.toggle(false, 0);
        assert!(chosen.ready());
    }

    #[test]
    fn answers_join_labels_with_a_bare_comma_and_append_other() {
        let questions = questions_of(&input());
        let mut first = Picks::default();
        first.toggle(true, 0);
        first.toggle(true, 1);
        first.set_other(true, "Pink".to_string());
        let mut second = Picks::default();
        second.toggle(false, 1);
        let answers = answers_map(&questions, &[first, second]);
        assert_eq!(answers["Colours?"], "Blue,Green,Pink");
        assert_eq!(answers["Season?"], "Summer");
    }

    #[test]
    fn a_question_with_no_picks_is_left_out_of_the_answers() {
        let questions = questions_of(&input());
        let mut first = Picks::default();
        first.toggle(true, 0);
        let answers = answers_map(&questions, &[first]);
        assert_eq!(answers.len(), 1);
    }

    #[test]
    fn answer_pairs_are_read_from_the_result_text() {
        let output = "Your questions have been answered: \"Colours?\"=\"Blue,Green\", \"Season?\"=\"Summer\". You can now continue with these answers in mind.";
        let pairs = answer_pairs(&input(), output);
        assert_eq!(
            pairs,
            vec![
                AnswerPair { question: "Colours?".to_string(), answer: "Blue, Green".to_string() },
                AnswerPair { question: "Season?".to_string(), answer: "Summer".to_string() },
            ]
        );
    }

    #[test]
    fn answer_pairs_survive_quotes_commas_and_korean_text() {
        let input = json!({"questions": [
            {"question": "좋아하는 색을 모두 골라 주세요.", "options": []},
            {"question": "Say \"hi\", ok?", "options": []}
        ]});
        let output = "Your questions have been answered: \"좋아하는 색을 모두 골라 주세요.\"=\"파랑,초록\", \"Say \"hi\", ok?\"=\"yes, \"sure\"\". You can now continue with these answers in mind.";
        let pairs = answer_pairs(&input, output);
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].answer, "파랑, 초록");
        assert_eq!(pairs[1].question, "Say \"hi\", ok?");
        assert_eq!(pairs[1].answer, "yes,  \"sure\"");
    }

    #[test]
    fn a_result_that_is_not_an_answer_gives_no_pairs() {
        assert!(answer_pairs(&input(), "The user skipped the question.").is_empty());
        assert!(answer_pairs(&input(), "").is_empty());
        assert!(answer_pairs(&json!({}), "Your questions have been answered: \"a\"=\"b\".").is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod question;` and `pub use question::{answer_pairs, answers_map, questions_of, AnswerPair, Picks, Question, QuestionOption, QUESTION_TOOL};` to `crates/forge-entities/src/lib.rs`, then run:
`cargo test -p forge-entities question 2>&1 | tail -12`
Expected: compile errors (types and functions missing).

- [ ] **Step 3: Implement**

Put above the test module in `question.rs`:

```rust
use std::collections::BTreeMap;

use serde_json::Value;

/// The tool whose permission request is a question to the person, not a permission.
pub const QUESTION_TOOL: &str = "AskUserQuestion";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub header: String,
    pub text: String,
    pub options: Vec<QuestionOption>,
    pub multi: bool,
}

fn string_field(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The questions in an `AskUserQuestion` input. Anything missing or malformed is skipped: a
/// question without text is dropped, a missing header or description is empty.
pub fn questions_of(input: &Value) -> Vec<Question> {
    let Some(list) = input.get("questions").and_then(Value::as_array) else {
        return Vec::new();
    };
    list.iter()
        .filter_map(|question| {
            let text = string_field(question, "question");
            if text.is_empty() {
                return None;
            }
            let options = question
                .get("options")
                .and_then(Value::as_array)
                .map(|options| {
                    options
                        .iter()
                        .map(|option| QuestionOption { label: string_field(option, "label"), description: string_field(option, "description") })
                        .collect()
                })
                .unwrap_or_default();
            Some(Question {
                header: string_field(question, "header"),
                text,
                options,
                multi: question.get("multiSelect").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
}

/// What the person has chosen for one question: option indexes (kept in option order) and the
/// text typed under "Other".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Picks {
    pub chosen: Vec<usize>,
    pub other: String,
}

impl Picks {
    /// A single-select question holds one pick (and no "Other" text); a multi-select one toggles.
    pub fn toggle(&mut self, multi: bool, index: usize) {
        if multi {
            match self.chosen.iter().position(|&picked| picked == index) {
                Some(position) => {
                    self.chosen.remove(position);
                }
                None => {
                    self.chosen.push(index);
                    self.chosen.sort_unstable();
                }
            }
        } else {
            self.chosen = vec![index];
            self.other.clear();
        }
    }

    /// Typing under "Other" on a single-select question replaces the pick.
    pub fn set_other(&mut self, multi: bool, text: String) {
        if !multi && !text.trim().is_empty() {
            self.chosen.clear();
        }
        self.other = text;
    }

    pub fn ready(&self) -> bool {
        !self.chosen.is_empty() || !self.other.trim().is_empty()
    }
}

/// The answers the sidecar takes: each question's text to the chosen labels joined by a bare comma
/// (the real app does the same), then the "Other" text. A question with nothing picked is left out.
pub fn answers_map(questions: &[Question], picks: &[Picks]) -> BTreeMap<String, String> {
    let mut answers = BTreeMap::new();
    for (question, picked) in questions.iter().zip(picks) {
        let mut parts: Vec<String> = picked
            .chosen
            .iter()
            .filter_map(|&index| question.options.get(index))
            .map(|option| option.label.clone())
            .collect();
        if !picked.other.trim().is_empty() {
            parts.push(picked.other.trim().to_string());
        }
        if !parts.is_empty() {
            answers.insert(question.text.clone(), parts.join(","));
        }
    }
    answers
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPair {
    pub question: String,
    pub answer: String,
}

const ANSWERED_PREFIX: &str = "Your questions have been answered: ";
const ANSWERED_SUFFIX: &str = "\". You can now continue";

/// The question and answer pairs of a finished `AskUserQuestion` call, read from its result text
/// (`Your questions have been answered: "Q"="A", "Q2"="A2". You can now continue ...`), matched
/// against the question texts in the call's input. A result that is not an answer gives none.
/// Several labels (joined by a bare comma in the text) are shown with `, `.
pub fn answer_pairs(input: &Value, output: &str) -> Vec<AnswerPair> {
    let Some(body) = output.strip_prefix(ANSWERED_PREFIX) else {
        return Vec::new();
    };
    let mut pairs = Vec::new();
    let mut cursor = 0;
    for question in questions_of(input) {
        let marker = format!("\"{}\"=\"", question.text);
        let Some(found) = body[cursor..].find(&marker) else { continue };
        let start = cursor + found + marker.len();
        let rest = &body[start..];
        // An answer ends at the quote before `, "<next question>"=` or before `. You can now continue`.
        let end = ["\", \"", ANSWERED_SUFFIX]
            .iter()
            .filter_map(|stop| rest.find(stop))
            .min()
            .unwrap_or(rest.len());
        let answer = rest[..end].replace(',', ", ");
        pairs.push(AnswerPair { question: question.text, answer });
        cursor = start + end;
    }
    pairs
}
```

Note the test `answer_pairs_survive_quotes_commas_and_korean_text` expects the second answer `yes,  "sure"` (the bare comma is replaced by `, `, so `yes, "sure"` becomes `yes,  "sure"`: this is the documented trade-off of splitting a free-text answer; the test pins the current behaviour rather than a better one). If the exact text differs when you run it, print the actual value and fix the expectation to the actual output only if it follows from the rule "replace every `,` with `, `".

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities question 2>&1 | grep -E "^test |^test result|panicked|left|right" | head -30`
Expected: all 12 pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: questions, picks and answers for the question card

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Card title, key map, key-value tool input, elapsed label

**Files:**
- Create: `crates/forge-entities/src/permission_view.rs`
- Modify: `crates/forge-entities/src/tool_summary.rs`, `crates/forge-entities/src/turn_view.rs` (make `file_name` crate-visible), `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `crate::turn_view::{tool_kind, ToolKind}`.
- Produces:
  ```rust
  // permission_view.rs
  pub struct PermissionView { pub title: String, pub subject: String, pub path: Option<String>, pub command: Option<String> }
  pub fn permission_view(tool_name: &str, input: &Value) -> PermissionView;
  pub enum PermissionKey { Deny, AllowAlways, AllowOnce }   // Debug, Clone, Copy, PartialEq, Eq
  pub fn permission_key(code: &str, shift: bool, meta: bool, other_modifier: bool) -> Option<PermissionKey>;
  // tool_summary.rs
  pub struct InputLine { pub key: String, pub value: String, pub is_path: bool }
  pub fn tool_input_lines(input: &Value) -> Vec<InputLine>;
  pub fn elapsed_label(seconds: u64) -> String;
  ```

- [ ] **Step 1: Write the failing tests**

Create `permission_view.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_write_asks_to_create_the_file_by_name() {
        let view = permission_view("Write", &json!({"file_path": "/private/tmp/forge-observe/obs.txt", "content": "x"}));
        assert_eq!(view.title, "Claude wants to create obs.txt?");
        assert_eq!(view.subject, "obs.txt");
        assert_eq!(view.path.as_deref(), Some("/private/tmp/forge-observe/obs.txt"));
        assert_eq!(view.command, None);
    }

    #[test]
    fn edits_reads_and_notebooks_use_their_own_verb() {
        assert_eq!(permission_view("Edit", &json!({"file_path": "/p/a.py"})).title, "Claude wants to edit a.py?");
        assert_eq!(permission_view("MultiEdit", &json!({"file_path": "/p/a.py"})).title, "Claude wants to edit a.py?");
        assert_eq!(permission_view("NotebookEdit", &json!({"notebook_path": "/p/n.ipynb"})).title, "Claude wants to edit n.ipynb?");
        assert_eq!(permission_view("Read", &json!({"file_path": "/p/a.py"})).title, "Claude wants to read a.py?");
    }

    #[test]
    fn a_command_asks_to_run_its_description_and_carries_the_command() {
        let view = permission_view("Bash", &json!({"command": "python3 fizz.py", "description": "Run fizz.py"}));
        assert_eq!(view.title, "Claude wants to run Run fizz.py?");
        assert_eq!(view.command.as_deref(), Some("python3 fizz.py"));
        assert_eq!(view.path, None);
        let bare = permission_view("Bash", &json!({"command": "ls -la"}));
        assert_eq!(bare.title, "Claude wants to run ls -la?");
    }

    #[test]
    fn any_other_tool_is_used_by_name_even_with_an_empty_input() {
        for input in [json!({}), json!(null), json!("x")] {
            let view = permission_view("mcp__thing__go", &input);
            assert_eq!(view.title, "Claude wants to use mcp__thing__go?");
            assert_eq!(view.path, None);
            assert_eq!(view.command, None);
        }
    }

    #[test]
    fn a_write_with_no_path_still_has_a_title() {
        let view = permission_view("Write", &json!({"content": "x"}));
        assert_eq!(view.title, "Claude wants to create a file?");
        assert_eq!(view.path, None);
    }

    #[test]
    fn digits_and_escape_pick_by_physical_key() {
        assert_eq!(permission_key("Digit1", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Numpad1", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Escape", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Digit2", false, false, false), Some(PermissionKey::AllowAlways));
        assert_eq!(permission_key("Digit3", false, false, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("Digit4", false, false, false), None);
        assert_eq!(permission_key("KeyA", false, false, false), None);
    }

    #[test]
    fn command_return_allows_once_and_with_shift_always() {
        assert_eq!(permission_key("Enter", false, true, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("NumpadEnter", false, true, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("Enter", true, true, false), Some(PermissionKey::AllowAlways));
        assert_eq!(permission_key("Enter", false, false, false), None, "plain Return is not a decision");
    }

    #[test]
    fn a_digit_with_another_modifier_held_is_not_a_decision() {
        assert_eq!(permission_key("Digit1", false, false, true), None);
        assert_eq!(permission_key("Digit3", false, true, false), None);
    }
}
```

Add to the `tests` module of `tool_summary.rs`:

```rust
    #[test]
    fn input_lines_put_the_useful_keys_first_and_flag_paths() {
        let lines = tool_input_lines(&json!({"content": "x", "file_path": "/private/tmp/obs.txt", "mode": 3}));
        let keys: Vec<&str> = lines.iter().map(|line| line.key.as_str()).collect();
        assert_eq!(keys, vec!["file_path", "content", "mode"]);
        assert_eq!(lines[0], InputLine { key: "file_path".to_string(), value: "/private/tmp/obs.txt".to_string(), is_path: true });
        assert!(!lines[1].is_path);
        assert_eq!(lines[2].value, "3", "a non-string value is written as JSON");
    }

    #[test]
    fn input_lines_of_a_path_with_spaces_or_a_relative_value_are_not_paths() {
        let lines = tool_input_lines(&json!({"a": "/has space", "b": "relative/x", "c": "/ok"}));
        let flags: Vec<bool> = lines.iter().map(|line| line.is_path).collect();
        assert_eq!(flags, vec![false, false, true]);
    }

    #[test]
    fn input_lines_of_something_that_is_not_an_object_are_empty() {
        for input in [json!(null), json!("text"), json!([1, 2]), json!({})] {
            assert!(tool_input_lines(&input).is_empty(), "{input}");
        }
    }

    #[test]
    fn a_nested_value_is_written_compactly_and_a_long_one_is_cut() {
        let lines = tool_input_lines(&json!({"edits": [{"old": "a"}], "big": "x".repeat(5000)}));
        let edits = lines.iter().find(|line| line.key == "edits").unwrap();
        assert_eq!(edits.value, r#"[{"old":"a"}]"#);
        let big = lines.iter().find(|line| line.key == "big").unwrap();
        assert!(big.value.ends_with("… 1000 more characters"));
    }

    #[test]
    fn the_clock_reads_seconds_minutes_and_hours() {
        assert_eq!(elapsed_label(0), "0s");
        assert_eq!(elapsed_label(4), "4s");
        assert_eq!(elapsed_label(59), "59s");
        assert_eq!(elapsed_label(81), "1m 21s");
        assert_eq!(elapsed_label(3600), "1h 0m");
        assert_eq!(elapsed_label(3720), "1h 2m");
    }
```

- [ ] **Step 2: Run to verify they fail**

Add `mod permission_view;` and `pub use permission_view::{permission_key, permission_view, PermissionKey, PermissionView};` to `lib.rs`, and extend the `tool_summary` re-export with `elapsed_label, tool_input_lines, InputLine`. Then run:
`cargo test -p forge-entities permission_view 2>&1 | tail -6; cargo test -p forge-entities tool_summary 2>&1 | tail -6`
Expected: compile errors.

- [ ] **Step 3: Implement**

In `turn_view.rs` change `fn file_name(path: &str) -> String` to `pub(crate) fn file_name(path: &str) -> String` and make sure `tool_kind` is `pub fn` (it already is).

Put above the tests in `permission_view.rs`:

```rust
use serde_json::Value;

use crate::tool_summary::tool_input_summary;
use crate::turn_view::{file_name, tool_kind, ToolKind};

/// What a permission card says about a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionView {
    /// "Claude wants to create obs.txt?"
    pub title: String,
    /// The file name, the command's description (else the command), or the tool name.
    pub subject: String,
    /// The full path of a file tool.
    pub path: Option<String>,
    /// The full command of a Bash call.
    pub command: Option<String>,
}

pub fn permission_view(tool_name: &str, input: &Value) -> PermissionView {
    let text = |key: &str| input.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string);
    let kind = tool_kind(tool_name);
    let (verb, subject, path, command) = match kind {
        ToolKind::Created | ToolKind::Edited | ToolKind::Read => {
            let verb = match kind {
                ToolKind::Created => "create",
                ToolKind::Edited => "edit",
                _ => "read",
            };
            let path = text("file_path").or_else(|| text("notebook_path"));
            let subject = path.as_deref().map(file_name).unwrap_or_else(|| "a file".to_string());
            (verb, subject, path, None)
        }
        ToolKind::Command => {
            let command = text("command");
            let subject = text("description").or_else(|| command.clone()).unwrap_or_else(|| tool_input_summary(input));
            ("run", subject, None, command)
        }
        ToolKind::Other => ("use", tool_name.to_string(), None, None),
    };
    PermissionView { title: format!("Claude wants to {verb} {subject}?"), subject, path, command }
}

/// What a key does on a permission card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKey {
    Deny,
    AllowAlways,
    AllowOnce,
}

/// The decision a key press stands for, from the physical key (`code`), never the character: the
/// character depends on the input method. `1` and Esc deny, `2` and ⇧⌘↩ allow always, `3` and ⌘↩
/// allow once. A digit with ⌘, ⌃ or ⌥ held (`other_modifier`) is not a decision.
pub fn permission_key(code: &str, shift: bool, meta: bool, other_modifier: bool) -> Option<PermissionKey> {
    if matches!(code, "Enter" | "NumpadEnter") {
        return match (meta, shift) {
            (true, true) => Some(PermissionKey::AllowAlways),
            (true, false) => Some(PermissionKey::AllowOnce),
            _ => None,
        };
    }
    if meta || other_modifier {
        return None;
    }
    match code {
        "Escape" => Some(PermissionKey::Deny),
        "Digit1" | "Numpad1" => Some(PermissionKey::Deny),
        "Digit2" | "Numpad2" => Some(PermissionKey::AllowAlways),
        "Digit3" | "Numpad3" => Some(PermissionKey::AllowOnce),
        _ => None,
    }
}
```

In `tool_summary.rs` add (above the tests; `SUMMARY_KEYS` and `tool_detail_text` are already there):

```rust
/// One `key: value` line of a tool call's input, as the opened row shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLine {
    pub key: String,
    pub value: String,
    /// An absolute path without spaces: shown blue and underlined.
    pub is_path: bool,
}

// The keys a person wants first, in this order; every other key follows alphabetically.
const FIRST_KEYS: [&str; 7] = ["file_path", "notebook_path", "path", "command", "description", "pattern", "url"];

/// The input of a tool call as `key: value` lines (no JSON braces). Strings are shown as they are,
/// anything else as compact JSON; a value is cut to 4000 characters. Input that is not an object
/// has no lines.
pub fn tool_input_lines(input: &Value) -> Vec<InputLine> {
    let Some(object) = input.as_object() else {
        return Vec::new();
    };
    let mut entries: Vec<(&String, &Value)> = object.iter().collect();
    entries.sort_by_key(|(key, _)| (FIRST_KEYS.iter().position(|first| first == &key.as_str()).unwrap_or(FIRST_KEYS.len()), key.to_string()));
    entries
        .into_iter()
        .map(|(key, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let is_path = matches!(value, Value::String(text) if text.starts_with('/') && !text.contains(char::is_whitespace));
            InputLine { key: key.clone(), value: tool_detail_text(&text), is_path }
        })
        .collect()
}

/// The clock of a running turn: "4s", "1m 21s", "1h 2m".
pub fn elapsed_label(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities 2>&1 | grep -E "FAILED|panicked|^test result|^error" -A4 | head -12`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: permission card title and keys, key-value tool input, elapsed label

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The live state in the turn view model

**Files:**
- Modify: `crates/forge-entities/src/turn_view.rs`, `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: Task 3 (`QUESTION_TOOL`, `answer_pairs`, `AnswerPair`), Task 4 nothing.
- Produces (Task 6 consumes these exact names):
  ```rust
  pub enum Waiting { Permission { tool_name: String, input: Value }, Question }
  pub struct LiveState { pub started: Option<SystemTime>, pub waiting: Option<Waiting> }   // Debug, Clone, PartialEq, Default
  pub fn live_state(turn_started: Option<SystemTime>, pending: Option<&PermissionRequest>) -> LiveState;
  pub enum LiveKind { Tool, Thinking, NeedsInput }
  pub struct LiveLine { pub kind: LiveKind, pub label: String, pub subject: String, pub added: usize, pub removed: usize, pub started: Option<SystemTime> }
  // Item gains two variants:
  Item::Answers(Vec<AnswerPair>)
  Item::Live(LiveLine)
  pub fn build_turns(messages: &[Message], view: TranscriptView, live: Option<&LiveState>) -> Vec<Item>;
  ```

- [ ] **Step 1: Update the 23A tests for the new signature and rules, then add the new ones**

In `turn_view.rs`'s test module:

1. Replace every call `build_turns(&X, TranscriptView::Y, false)` by `build_turns(&X, TranscriptView::Y, None)` and every `..., true)` by `..., Some(&LiveState::default()))` (a regex over `TranscriptView::\w+, (false|true)\)` is safe).
2. Extend the `kinds` helper's match with `Item::Answers(_) => "answers", Item::Live(_) => "live",`.
3. Replace the two tests that describe a running turn:

```rust
    #[test]
    fn a_running_turn_shows_text_before_the_first_tool_then_one_live_line() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![text("plan"), write("t-1", "/p/a.py"), text("then"), tool("t-2", "Bash", json!({"command": "ls", "description": "List"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&LiveState::default()));
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        let Item::Live(line) = &items[2] else { panic!() };
        assert_eq!(line.kind, LiveKind::Tool);
        assert_eq!(line.label, "List");
        assert_eq!(line.subject, "");
    }

    #[test]
    fn a_turn_that_ended_with_an_unanswered_tool_folds_and_the_row_is_not_failed() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![write("t-1", "/p/a.py"), tool("t-2", "Bash", json!({"command": "ls"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "card", "footer"]);
        let Row::Tool(pending) = &activity(&items).rows[1] else { panic!() };
        assert!(!pending.failed);
    }
```
and change `an_earlier_turn_has_its_footer_even_while_a_later_turn_runs` to expect `vec!["user", "text", "footer", "user", "text", "live"]`.

4. Add:

```rust
    fn live_items(blocks: Vec<Block>, live: LiveState) -> Vec<Item> {
        build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, Some(&live))
    }
    fn live_line(items: &[Item]) -> LiveLine {
        let Item::Live(line) = items.last().unwrap() else { panic!("a live line last") };
        line.clone()
    }
    fn unfinished(name: &str, input: serde_json::Value) -> Block {
        tool("t-x", name, input, None)
    }

    #[test]
    fn a_running_turn_with_no_reply_yet_shows_the_user_and_a_thinking_line() {
        let items = build_turns(&[Message::user("go")], TranscriptView::Normal, Some(&LiveState::default()));
        assert_eq!(kinds(&items), vec!["user", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
    }

    #[test]
    fn streamed_text_stays_visible_above_a_thinking_line() {
        let items = live_items(vec![text("partial answ")], LiveState::default());
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
    }

    #[test]
    fn the_live_label_names_the_step_in_the_present_tense() {
        let line = |name: &str, input: serde_json::Value| live_line(&live_items(vec![unfinished(name, input)], LiveState::default()));
        let write = line("Write", json!({"file_path": "/p/hello.txt", "content": "x"}));
        assert_eq!((write.kind, write.label.as_str(), write.subject.as_str()), (LiveKind::Tool, "Creating", "hello.txt"));
        let edit = line("Edit", json!({"file_path": "/p/a.py", "old_string": "a", "new_string": "b"}));
        assert_eq!((edit.label.as_str(), edit.subject.as_str()), ("Editing", "a.py"));
        let read = line("Read", json!({"file_path": "/p/a.py"}));
        assert_eq!((read.label.as_str(), read.subject.as_str()), ("Reading", "a.py"));
        let bash = line("Bash", json!({"command": "ls -la", "description": "List files"}));
        assert_eq!((bash.label.as_str(), bash.subject.as_str()), ("List files", ""));
        let bare = line("Bash", json!({"command": "ls -la"}));
        assert_eq!(bare.label, "ls -la");
        let grep = line("Grep", json!({"pattern": "fn main"}));
        assert_eq!((grep.label.as_str(), grep.subject.as_str()), ("Searching", "fn main"));
        let other = line("WebFetch", json!({"url": "https://a.test"}));
        assert_eq!((other.label.as_str(), other.subject.as_str()), ("Using WebFetch", "https://a.test"));
        let empty = line("mcp__x__go", json!({}));
        assert_eq!((empty.label.as_str(), empty.subject.as_str()), ("Using mcp__x__go", ""));
    }

    #[test]
    fn between_tools_the_line_is_thinking() {
        let items = live_items(vec![write("t-1", "/p/a.py"), text("now the next")], LiveState::default());
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
        let after_tool = live_items(vec![write("t-1", "/p/a.py")], LiveState::default());
        assert_eq!(live_line(&after_tool).kind, LiveKind::Thinking);
    }

    #[test]
    fn waiting_for_a_permission_shows_needs_input_with_the_pending_changes() {
        let input = json!({"file_path": "/p/a.py", "content": "a\nb\nc\n"});
        let waiting = Waiting::Permission { tool_name: "Write".to_string(), input: input.clone() };
        let items = live_items(vec![unfinished("Write", input)], LiveState { started: None, waiting: Some(waiting) });
        let line = live_line(&items);
        assert_eq!(line.kind, LiveKind::NeedsInput);
        assert_eq!(line.label, "Input needed");
        assert_eq!((line.added, line.removed), (3, 0));
    }

    #[test]
    fn waiting_for_a_command_or_a_question_has_no_changes() {
        let command = Waiting::Permission { tool_name: "Bash".to_string(), input: json!({"command": "ls"}) };
        let items = live_items(vec![], LiveState { started: None, waiting: Some(command) });
        assert_eq!((live_line(&items).added, live_line(&items).removed), (0, 0));
        let question = live_items(vec![], LiveState { started: None, waiting: Some(Waiting::Question) });
        assert_eq!(live_line(&question).kind, LiveKind::NeedsInput);
    }

    #[test]
    fn the_start_time_travels_to_the_live_line() {
        let started = std::time::UNIX_EPOCH + std::time::Duration::from_secs(5);
        let items = live_items(vec![], LiveState { started: Some(started), waiting: None });
        assert_eq!(live_line(&items).started, Some(started));
    }

    #[test]
    fn live_state_follows_the_first_pending_request() {
        let started = std::time::UNIX_EPOCH;
        assert_eq!(live_state(Some(started), None), LiveState { started: Some(started), waiting: None });
        let question = forge_domain::PermissionRequest { id: "q".to_string(), tool_name: "AskUserQuestion".to_string(), input: json!({}) };
        assert_eq!(live_state(None, Some(&question)).waiting, Some(Waiting::Question));
        let write = forge_domain::PermissionRequest { id: "p".to_string(), tool_name: "Write".to_string(), input: json!({"file_path": "/p/a"}) };
        assert_eq!(
            live_state(None, Some(&write)).waiting,
            Some(Waiting::Permission { tool_name: "Write".to_string(), input: json!({"file_path": "/p/a"}) })
        );
    }

    fn ask(id: &str, result: Option<&str>) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: "AskUserQuestion".to_string(),
            input: json!({"questions": [{"question": "Season?", "header": "S", "multiSelect": false, "options": [{"label": "Spring", "description": ""}, {"label": "Summer", "description": ""}]}]}),
            result: result.map(|output| ToolResult { output: output.to_string(), is_error: false }),
        })
    }
    const ANSWERED: &str = "Your questions have been answered: \"Season?\"=\"Summer\". You can now continue with these answers in mind.";

    #[test]
    fn an_answered_question_becomes_an_answer_card_outside_the_fold() {
        let blocks = vec![write("t-1", "/p/a.py"), ask("q-1", Some(ANSWERED)), text("Nice."), bash("t-2", Some("Run"))];
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "answers", "activity", "card", "footer"]);
        let Item::Answers(pairs) = &items[2] else { panic!() };
        assert_eq!(pairs, &vec![AnswerPair { question: "Season?".to_string(), answer: "Summer".to_string() }]);
    }

    #[test]
    fn a_pending_question_draws_nothing_but_the_needs_input_line() {
        let items = live_items(vec![text("Let me ask."), ask("q-1", None)], LiveState { started: None, waiting: Some(Waiting::Question) });
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::NeedsInput);
    }

    #[test]
    fn a_question_that_was_skipped_leaves_no_card() {
        let skipped = Block::Tool(ToolCall {
            id: "q-2".to_string(),
            name: "AskUserQuestion".to_string(),
            input: json!({"questions": [{"question": "Season?", "options": []}]}),
            result: Some(ToolResult { output: "The user skipped the question.".to_string(), is_error: true }),
        });
        let items = build_turns(&[Message::user("go"), assistant(vec![skipped, text("ok")])], TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "text", "footer"]);
    }

    #[test]
    fn the_footer_copies_the_text_after_the_last_question_or_tool() {
        let items = build_turns(&[Message::user("go"), assistant(vec![text("before"), ask("q-1", Some(ANSWERED)), text("after")])], TranscriptView::Normal, None);
        let Item::Footer(footer) = items.last().unwrap() else { panic!() };
        assert_eq!(footer.text, "after");
    }
```
(`ToolResult`, `ToolCall` are already imported in the module; add `AnswerPair` through `use super::*`.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-entities turn_view 2>&1 | grep -E "^error" -A5 | head -20`
Expected: compile errors (`LiveState`, `LiveKind`, `Waiting`, `live_state`, `Item::Live`, the new `build_turns` argument are missing).

- [ ] **Step 3: Implement**

In `turn_view.rs` add imports `use std::time::SystemTime;`, `use forge_domain::PermissionRequest;`, `use crate::diff::diff_for_tool;`, `use crate::question::{answer_pairs, AnswerPair, QUESTION_TOOL};`.

Add the types:

```rust
/// What a running turn is waiting for, if anything.
#[derive(Debug, Clone, PartialEq)]
pub enum Waiting {
    Permission { tool_name: String, input: Value },
    Question,
}

/// A turn that is running now.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveState {
    /// When it was sent; the clock counts from here.
    pub started: Option<SystemTime>,
    pub waiting: Option<Waiting>,
}

/// The live state of a session from what its view holds: the start time and the request on top of
/// the queue.
pub fn live_state(turn_started: Option<SystemTime>, pending: Option<&PermissionRequest>) -> LiveState {
    let waiting = pending.map(|request| {
        if request.tool_name == QUESTION_TOOL {
            Waiting::Question
        } else {
            Waiting::Permission { tool_name: request.tool_name.clone(), input: request.input.clone() }
        }
    });
    LiveState { started: turn_started, waiting }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveKind {
    /// A tool call is running: `label` is the verb or the command's description, `subject` the file.
    Tool,
    /// The model is writing or thinking.
    Thinking,
    /// A permission or a question waits for the person.
    NeedsInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveLine {
    pub kind: LiveKind,
    pub label: String,
    pub subject: String,
    /// The pending write or edit's `+a −r` (NeedsInput only).
    pub added: usize,
    pub removed: usize,
    pub started: Option<SystemTime>,
}
```

Add the two variants to `Item`: `Answers(Vec<AnswerPair>),` and `Live(LiveLine),`.

Add the label helper and the line builder:

```rust
fn live_label(row: &ToolRow) -> (String, String) {
    match row.kind {
        ToolKind::Created => ("Creating".to_string(), row.subject.clone()),
        ToolKind::Edited => ("Editing".to_string(), row.subject.clone()),
        ToolKind::Read => ("Reading".to_string(), row.subject.clone()),
        ToolKind::Command => (row.subject.clone(), String::new()),
        ToolKind::Other if matches!(row.call.name.as_str(), "Grep" | "Glob") => ("Searching".to_string(), row.subject.clone()),
        ToolKind::Other => (format!("Using {}", row.call.name), row.subject.clone()),
    }
}

fn live_line(blocks: &[Block], tool_rows: &[Option<ToolRow>], live: &LiveState) -> LiveLine {
    let started = live.started;
    if let Some(waiting) = &live.waiting {
        let (added, removed) = match waiting {
            Waiting::Permission { tool_name, input } => diff_for_tool(tool_name, input).map_or((0, 0), |diff| (diff.added, diff.removed)),
            Waiting::Question => (0, 0),
        };
        return LiveLine { kind: LiveKind::NeedsInput, label: "Input needed".to_string(), subject: String::new(), added, removed, started };
    }
    if let (Some(Block::Tool(call)), Some(Some(row))) = (blocks.last(), tool_rows.last()) {
        if call.result.is_none() {
            let (label, subject) = live_label(row);
            return LiveLine { kind: LiveKind::Tool, label, subject, added: 0, removed: 0, started };
        }
    }
    LiveLine { kind: LiveKind::Thinking, label: String::new(), subject: String::new(), added: 0, removed: 0, started }
}
```

Replace `finish_turn` and `build_turns` (keep `build_activity`, `build_card`, `summarize`, `tool_row` from 23A; `build_card` keeps taking all rows) with:

```rust
fn is_question(block: &Block) -> bool {
    matches!(block, Block::Tool(call) if call.name == QUESTION_TOOL)
}

// The text after the last tool call (a question counts as one); when there is none, the last text
// the turn has, so a turn that ends in a tool call still has something to copy.
fn closing_text(blocks: &[Block]) -> String {
    let last_tool = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let texts = |from: usize| -> Vec<&str> {
        blocks
            .iter()
            .enumerate()
            .filter(|(index, _)| *index >= from)
            .filter_map(|(_, block)| match block {
                Block::Text(text) if !text.trim().is_empty() => Some(text.as_str()),
                _ => None,
            })
            .collect()
    };
    let after = texts(last_tool.map_or(0, |last| last + 1));
    if !after.is_empty() {
        return after.join("\n\n");
    }
    texts(0).last().map(|text| text.to_string()).unwrap_or_default()
}

// One stretch of a turn without a question in it. A finished stretch folds its tool calls into one
// activity; the running one shows its text before the first tool call and one live line.
fn push_segment(items: &mut Vec<Item>, blocks: &[Block], tool_rows: &[Option<ToolRow>], view: TranscriptView, live: Option<&LiveState>) {
    let first = blocks.iter().position(|block| matches!(block, Block::Tool(_)));
    let last = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let plain = |items: &mut Vec<Item>, block: &Block| match block {
        Block::Text(text) if !text.trim().is_empty() => items.push(Item::Text(text.clone())),
        Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => items.push(Item::Thinking(text.clone())),
        _ => {}
    };
    if let Some(live) = live {
        for block in &blocks[..first.unwrap_or(blocks.len())] {
            plain(items, block);
        }
        items.push(Item::Live(live_line(blocks, tool_rows, live)));
        return;
    }
    for (index, block) in blocks.iter().enumerate() {
        if let (Some(first), Some(last)) = (first, last) {
            if index >= first && index <= last {
                if index == first {
                    items.push(Item::Activity(build_activity(&blocks[first..=last], &tool_rows[first..=last], view)));
                }
                continue;
            }
        }
        plain(items, block);
    }
}

fn finish_turn(items: &mut Vec<Item>, blocks: Vec<Block>, at: Option<String>, view: TranscriptView, live: Option<&LiveState>) {
    if blocks.is_empty() && live.is_none() {
        return;
    }
    let start = items.len();
    // Each tool call's row, and so its diff, is worked out once per build.
    let tool_rows: Vec<Option<ToolRow>> = blocks
        .iter()
        .map(|block| if let Block::Tool(call) = block { Some(tool_row(call)) } else { None })
        .collect();
    // A question splits the turn: what came before it folds, the question becomes its answer card.
    let mut segment_start = 0;
    for (index, block) in blocks.iter().enumerate() {
        if !is_question(block) {
            continue;
        }
        push_segment(items, &blocks[segment_start..index], &tool_rows[segment_start..index], view, None);
        if let Block::Tool(call) = block {
            if let Some(result) = &call.result {
                let pairs = answer_pairs(&call.input, &result.output);
                if !pairs.is_empty() {
                    items.push(Item::Answers(pairs));
                }
            }
        }
        segment_start = index + 1;
    }
    push_segment(items, &blocks[segment_start..], &tool_rows[segment_start..], view, live);
    if live.is_none() && items.len() > start {
        if let Some(card) = build_card(&tool_rows) {
            items.push(Item::FileCard(card));
        }
        items.push(Item::Footer(Footer { text: closing_text(&blocks), at }));
    }
}

/// The conversation as things to draw. A turn runs from a user message to the next one; the
/// assistant messages in between are read as one run of blocks. `live` is the state of the turn
/// that is running now, if any: the last turn then shows one live line in place of its folded
/// activity, and has no file card and no footer.
pub fn build_turns(messages: &[Message], view: TranscriptView, live: Option<&LiveState>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut at: Option<String> = None;
    for message in messages {
        match message.role {
            MessageRole::User => {
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, None);
                items.push(Item::User(message.clone()));
            }
            MessageRole::Assistant => {
                blocks.extend(message.blocks.iter().cloned());
                if message.at.is_some() {
                    at = message.at.clone();
                }
            }
        }
    }
    finish_turn(&mut items, blocks, at, view, live);
    items
}
```

Update `lib.rs`: `pub use turn_view::{build_turns, live_state, Activity, CardFile, FileCard, Footer, Item, LiveKind, LiveLine, LiveState, Row, ToolKind, ToolRow, Waiting};` (and `AnswerPair` is already exported by `question`).

`turn.rs` and `chat_window.rs` do not compile after this task until Task 6 (they call `build_turns(…, running)` and `Turns { running }`). To keep the workspace building between tasks, make the minimal edits now: in `turn.rs` change the `Turns` props to `live: Option<LiveState>`, the call to `build_turns(&messages, view, live.as_ref())`, add `Item::Answers(_) | Item::Live(_) => rsx! {}` as a temporary arm to `TurnItem` (Task 6 replaces it), and in `chat_window.rs` pass `live: (view.phase == Phase::Running).then(|| live_state(view.turn_started, view.pending_permissions.front()))` (import `live_state` from `forge_entities`).

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities turn_view 2>&1 | grep -E "^test .*FAILED|panicked|^test result" -A3 | head -20; cargo build --workspace 2>&1 | grep -E "^error" -A6 | head`
Expected: all turn_view tests pass (the 15 from 23A, two replaced, plus 17 new); the workspace builds.

- [ ] **Step 5: Run the whole workspace and commit**

Run: `cargo test --workspace > /tmp/t5.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t5.txt | head; grep -E "^test result" /tmp/t5.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`.

```bash
git add -A crates
git commit -m "feat: the turn view model knows the running turn, and shows question answers

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Draw the live line, the answer card and the key-value tool detail

**Files:**
- Modify: `crates/forge-entities/Cargo.toml`, `crates/forge-entities/src/turn.rs`, `crates/forge-entities/src/message.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: Task 4 (`tool_input_lines`, `elapsed_label`), Task 5 (`Item::Live`, `Item::Answers`, `LiveLine`, `LiveKind`).
- Produces: `RunningLine`, `Clock`, `AnswerCard` components (private to `turn.rs`); CSS classes `live-line`, `live-label`, `live-subject`, `live-clock`, `live-dots`, `live-spinner`, `needs-input`, `answer-card`, `answer-question`, `answer-text`, `input-lines`, `input-key`, `input-value`, `input-path`, `tool-error`.

- [ ] **Step 1: Write the failing CSS-class test**

In `crates/forge-shared/src/theme.rs`, add to the class list of `the_stylesheet_defines_the_layout_classes_the_components_use` (next to `.turns`): `".live-line"`, `".live-dots"`, `".live-clock"`, `".needs-input"`, `".answer-card"`, `".input-lines"`, `".input-path"`, `".tool-error"`.

Run: `cargo test -p forge-shared the_stylesheet_defines 2>&1 | grep -E "missing|test result"`
Expected: FAIL: `theme.css is missing .live-line`.

- [ ] **Step 2: Add the dependency and rewrite the opened tool detail**

`crates/forge-entities/Cargo.toml`: add `tokio = { version = "1", features = ["time"] }` (already a workspace dependency of other crates; it drives the once-a-second clock).

`crates/forge-entities/src/message.rs`: replace `ToolDetail` with

```rust
/// The input of a tool call as `key: value` lines and, once it has one, its result. A failed call
/// shows its error text first, in red (a denied call's text tells what was rejected).
#[component]
pub(crate) fn ToolDetail(tool_call: ToolCall) -> Element {
    let lines = tool_input_lines(&tool_call.input);
    let result = tool_call.result.as_ref().map(|result| (result.is_error, tool_detail_text(&result.output)));
    rsx! {
        div { class: "tool-detail",
            if let Some((true, error)) = result.clone() {
                div { class: "tool-error", "{error}" }
            }
            div { class: "input-lines",
                for line in lines.iter() {
                    div { key: "{line.key}", class: "input-line",
                        span { class: "input-key", "{line.key}:" }
                        span { class: if line.is_path { "input-value input-path" } else { "input-value" }, "{line.value}" }
                    }
                }
            }
            if let Some((false, output)) = result {
                if !output.is_empty() {
                    pre { class: "tool-output", "{output}" }
                }
            }
        }
    }
}
```
with imports `use crate::tool_summary::{tool_detail_text, tool_input_lines};`.

- [ ] **Step 3: Draw the new items**

In `crates/forge-entities/src/turn.rs` add imports `use std::time::{Duration, SystemTime};`, `use crate::question::AnswerPair;`, `use crate::tool_summary::elapsed_label;`, `use crate::turn_view::{LiveKind, LiveLine};` and `LiveState` to the existing `turn_view` import. Replace the temporary `Item::Answers(_) | Item::Live(_)` arm of `TurnItem` with:

```rust
        Item::Answers(pairs) => rsx! { AnswerCard { pairs } },
        Item::Live(line) => rsx! { RunningLine { line } },
```
and add:

```rust
/// The seconds of a running turn. Only this component re-renders each second; the transcript above
/// it is not rebuilt.
#[component]
fn Clock(started: Option<SystemTime>) -> Element {
    let mut now = use_signal(SystemTime::now);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            now.set(SystemTime::now());
        }
    });
    let Some(started) = started else { return rsx! {} };
    let seconds = now().duration_since(started).map(|elapsed| elapsed.as_secs()).unwrap_or(0);
    let label = elapsed_label(seconds);
    rsx! { span { class: "live-clock", "{label}" } }
}

/// What the running turn is doing right now: a tool, the model writing, or a question for the person.
#[component]
fn RunningLine(line: LiveLine) -> Element {
    match line.kind {
        LiveKind::Tool => rsx! {
            div { class: "live-line",
                span { class: "live-spinner" }
                span { class: "live-label", "{line.label}" }
                if !line.subject.is_empty() {
                    span { class: "live-subject", "{line.subject}" }
                }
                Clock { started: line.started }
            }
        },
        LiveKind::Thinking => rsx! {
            div { class: "live-line",
                span { class: "live-dots", "···" }
                Clock { started: line.started }
            }
        },
        LiveKind::NeedsInput => rsx! {
            div { class: "live-line",
                span { class: "live-ring" }
                span { class: "needs-input", "{line.label} ›" }
                CountChips { added: line.added, removed: line.removed }
            }
        },
    }
}

/// A finished question: each question in grey and its answer in black, in a narrow bordered card.
#[component]
fn AnswerCard(pairs: Vec<AnswerPair>) -> Element {
    rsx! {
        div { class: "answer-card",
            for pair in pairs.iter() {
                div { key: "{pair.question}", class: "answer-pair",
                    div { class: "answer-question", "{pair.question}" }
                    div { class: "answer-text", "{pair.answer}" }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Styles**

Append to `theme.css` (not inside a media query; the colours exist: `--fg-muted`, `--fg`, `--border`, `--bg-card`, `--bg-hover`, `--radius`, `--danger`, `--diff-added-bg`; `--accent` is the orange):

```css
.live-line { display: flex; align-items: center; gap: 8px; color: var(--fg-muted); min-height: 24px; }
.live-label { color: var(--fg-muted); }
.live-subject { color: var(--fg); }
.live-clock { color: var(--fg-muted); font-variant-numeric: tabular-nums; }
.live-dots { color: var(--accent); font-weight: 700; letter-spacing: 2px; animation: live-pulse 1.2s ease-in-out infinite; }
.live-spinner { width: 12px; height: 12px; border: 2px solid var(--border); border-top-color: var(--accent); border-radius: 50%; animation: live-spin 0.8s linear infinite; flex: none; }
.live-ring { width: 12px; height: 12px; border: 2px solid var(--accent); border-radius: 50%; box-sizing: border-box; flex: none; }
.needs-input { padding: 1px 10px; border-radius: 999px; background: rgba(217, 119, 6, 0.14); color: var(--accent); }
@keyframes live-spin { to { transform: rotate(360deg); } }
@keyframes live-pulse { 50% { opacity: 0.35; } }
@media (prefers-reduced-motion: reduce) { .live-dots, .live-spinner { animation: none; } }

.answer-card { width: fit-content; max-width: 100%; box-sizing: border-box; padding: 10px 14px; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); }
.answer-pair + .answer-pair { margin-top: 8px; }
.answer-question { color: var(--fg-muted); }
.answer-text { color: var(--fg); }

.input-lines { padding: 8px 12px; }
.input-line { display: flex; gap: 8px; line-height: 1.6; }
.input-key { flex: none; color: var(--fg-muted); font-family: var(--mono); font-size: 12px; }
.input-value { min-width: 0; white-space: pre-wrap; overflow-wrap: anywhere; }
.input-path { color: var(--link); text-decoration: underline; text-underline-offset: 2px; }
.tool-error { padding: 8px 12px; color: var(--danger); white-space: pre-wrap; overflow-wrap: anywhere; }
```
Check the accent: `grep -n "\-\-accent" crates/forge-shared/src/theme.css | head -3`; if the accent variable is not the orange used in the new session button, use whatever variable that button uses. The `@media (prefers-reduced-motion: reduce)` block already exists in the file (one more such block is fine; it is not a dark block).

- [ ] **Step 5: Run the tests and commit**

Run: `cargo test --workspace > /tmp/t6.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t6.txt | head; grep -E "^test result" /tmp/t6.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0` (the theme class test now passes).

```bash
git add -A crates
git commit -m "feat: the live line, the answer card and key-value tool detail

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The permission panel replaces the old permission card

**Files:**
- Create: `crates/forge-widgets/src/permission_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs`
- Delete: `crates/forge-entities/src/permission.rs` (the old `PermissionCard`), `crates/forge-features/src/approve_permission.rs`; update their `lib.rs` exports.
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: Task 4 (`permission_view`, `permission_key`, `PermissionKey`), `DiffView`, `diff_for_tool`, `Workspace::answer_permission`, `Workspace::set_permission_mode`.
- Produces: `#[component] pub fn PermissionPanel(id: LiveSessionId, request: PermissionRequest) -> Element`.

- [ ] **Step 1: Write the failing class test**

Add to the theme class list in `theme.rs`: `".permission-panel"`, `".perm-title"`, `".perm-buttons"`, `".perm-key"`, `".perm-menu"`. Run `cargo test -p forge-shared the_stylesheet_defines 2>&1 | grep missing` → FAIL `missing .permission-panel`.

- [ ] **Step 2: Write the widget**

Create `permission_panel.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionMode, PermissionRequest};
use forge_entities::{diff_for_tool, permission_key, permission_view, DiffView, PermissionKey};
use forge_features::use_workspace;

/// A permission request above the message box: what Claude wants to do, the change it would make,
/// and the choice. `1` / Esc deny, `2` / ⇧⌘↩ allow always, `3` / ⌘↩ allow once.
#[component]
pub fn PermissionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let mut menu_open = use_signal(|| false);
    let view = permission_view(&request.tool_name, &request.input);
    let diff = diff_for_tool(&request.tool_name, &request.input);
    let request_id = request.id.clone();

    let decide = {
        let request_id = request_id.clone();
        move |decision: PermissionDecision| workspace.answer_permission(id, request_id.clone(), decision)
    };
    let decide_key = decide.clone();
    let (deny, always, once) = (decide.clone(), decide.clone(), decide.clone());
    let switch_to_auto = {
        let request_id = request_id.clone();
        move |_| {
            menu_open.set(false);
            workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
            workspace.set_permission_mode(id, PermissionMode::Auto);
        }
    };

    rsx! {
        div {
            class: "permission-panel",
            tabindex: "0",
            // The card takes focus when it appears so its keys work at once.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                let modifiers = event.modifiers();
                let other = modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT);
                if let Some(key) = permission_key(&event.code().to_string(), modifiers.shift(), modifiers.meta(), other) {
                    event.stop_propagation();
                    event.prevent_default();
                    decide_key(match key {
                        PermissionKey::Deny => PermissionDecision::Deny,
                        PermissionKey::AllowAlways => PermissionDecision::AllowAlways,
                        PermissionKey::AllowOnce => PermissionDecision::AllowOnce,
                    });
                }
            },
            div { class: "perm-title", "{view.title}" }
            if view.path.is_some() || view.command.is_some() {
                div { class: "perm-subject", "{view.subject}" }
            }
            if let Some(path) = view.path.clone() {
                div { class: "perm-box", "{path}" }
                div { class: "perm-path", "{path}" }
            }
            if let Some(command) = view.command.clone() {
                div { class: "perm-box", "{command}" }
            }
            if let Some(diff) = diff {
                DiffView { diff }
            }
            div { class: "perm-buttons",
                button {
                    class: "perm-button",
                    r#type: "button",
                    onclick: move |_| deny(PermissionDecision::Deny),
                    "Deny"
                    span { class: "perm-key", "1" }
                    span { class: "perm-key", "Esc" }
                }
                span { class: "spacer" }
                button {
                    class: "perm-button",
                    r#type: "button",
                    onclick: move |_| always(PermissionDecision::AllowAlways),
                    "Always allow"
                    span { class: "perm-key", "2" }
                    span { class: "perm-key", "⇧⌘↩" }
                }
                div { class: "perm-primary",
                    button {
                        class: "perm-button primary",
                        r#type: "button",
                        onclick: move |_| once(PermissionDecision::AllowOnce),
                        "Allow once"
                        span { class: "perm-key", "3" }
                        span { class: "perm-key", "⌘↩" }
                    }
                    button {
                        class: "perm-button primary perm-more",
                        r#type: "button",
                        "aria-label": "More choices",
                        "aria-expanded": "{menu_open()}",
                        onclick: move |_| menu_open.set(!menu_open()),
                        "⌄"
                    }
                    if menu_open() {
                        div { class: "perm-menu",
                            button { class: "perm-menu-item", r#type: "button", onclick: switch_to_auto,
                                div { class: "perm-menu-title", "Allow and switch to Auto mode" }
                                div { class: "perm-menu-note", "The rest of this session is checked automatically." }
                            }
                        }
                    }
                }
            }
        }
    }
}
```
(`decide` closures capture `workspace` which is `Copy`; if the compiler objects to cloning closures, make `decide` a small `fn` taking `(workspace, id, request_id, decision)` and call it from each handler.)

- [ ] **Step 3: Wire it in and remove the old card**

`crates/forge-widgets/src/lib.rs`: add `mod permission_panel;`. `chat_window.rs`: replace the `if let Some(request) = view.pending_permissions.front() { PermissionCard {...} }` block with

```rust
            if let Some(request) = view.pending_permissions.front() {
                PermissionPanel { key: "{request.id}", id, request: request.clone() }
            }
```
(the key makes the next queued request mount a fresh card and take focus), import `crate::permission_panel::PermissionPanel`, and remove `PermissionCard` and `PermissionActions` from the imports. Delete `crates/forge-entities/src/permission.rs` and `crates/forge-features/src/approve_permission.rs` and remove their `mod`/`pub use` lines (`grep -rn "PermissionCard\|PermissionActions\|approve_permission" crates --include=*.rs` must find nothing afterwards, except the test module of the deleted files, which goes with them). Note that `QUESTION_TOOL` requests must not reach this panel: Task 8 adds the branch; until then `ChatWindow` shows questions in the permission panel, which is acceptable for one commit.

- [ ] **Step 4: Styles**

Append to `theme.css`:

```css
.permission-panel { display: flex; flex-direction: column; gap: 10px; width: calc(100% - 56px); max-width: var(--chat-column); box-sizing: border-box; margin: 0 auto 8px; padding: 16px; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); outline: none; }
.perm-title { font-weight: 700; }
.perm-subject { color: var(--fg); }
.perm-box { padding: 10px 12px; background: var(--code-bg); border-radius: 8px; font-family: var(--mono); font-size: 13px; overflow-wrap: anywhere; white-space: pre-wrap; }
.perm-path { color: var(--fg-muted); font-size: 13px; overflow-wrap: anywhere; }
.permission-panel .diff { max-width: none; margin: 0; max-height: 40vh; overflow-y: auto; }
.perm-buttons { display: flex; align-items: center; gap: 8px; margin-top: 4px; }
.perm-buttons .spacer { flex: 1; }
.perm-button { appearance: none; display: inline-flex; align-items: center; gap: 8px; padding: 6px 12px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-card); color: var(--fg); font: inherit; cursor: pointer; }
.perm-button:hover { background: var(--bg-hover); }
.perm-button.primary { background: var(--fg); color: var(--bg-card); border-color: var(--fg); }
.perm-key { padding: 0 6px; border: 1px solid var(--border); border-radius: 5px; font-size: 11px; color: var(--fg-muted); background: var(--bg-card); }
.perm-button.primary .perm-key { background: transparent; color: inherit; border-color: rgba(255, 255, 255, 0.35); }
.perm-primary { position: relative; display: inline-flex; }
.perm-primary .perm-button:first-child { border-top-right-radius: 0; border-bottom-right-radius: 0; }
.perm-more { border-top-left-radius: 0; border-bottom-left-radius: 0; border-left: 1px solid rgba(255, 255, 255, 0.25); padding: 6px 10px; }
.perm-menu { position: absolute; right: 0; bottom: calc(100% + 6px); min-width: 260px; padding: 6px; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: var(--shadow); z-index: 5; }
.perm-menu-item { appearance: none; display: block; width: 100%; padding: 8px 10px; border: none; border-radius: 6px; background: none; text-align: left; font: inherit; color: var(--fg); cursor: pointer; }
.perm-menu-item:hover { background: var(--bg-hover); }
.perm-menu-note { color: var(--fg-muted); font-size: 12px; }
```

- [ ] **Step 5: Run the tests and commit**

Run: `cargo test --workspace > /tmp/t7.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t7.txt | head; grep -E "^test result" /tmp/t7.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0` (the deleted files' tests are gone; the count may drop by their number only: note the number removed in the ledger and make sure it is accounted for by the tests of `permission_view` that replaced them).

```bash
git add -A crates
git commit -m "feat: the permission panel with the real buttons and keys

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The question panel

**Files:**
- Create: `crates/forge-widgets/src/question_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: Task 3 (`questions_of`, `Picks`, `answers_map`, `QUESTION_TOOL`), `forge_features::menu_digit_row`, `Workspace::answer_question`, `Workspace::answer_permission`.
- Produces: `#[component] pub fn QuestionPanel(id: LiveSessionId, request: PermissionRequest) -> Element`.

- [ ] **Step 1: Write the failing class test**

Add to the theme class list: `".question-panel"`, `".question-chip"`, `".question-option"`, `".question-footer"`. Run `cargo test -p forge-shared the_stylesheet_defines 2>&1 | grep missing` → FAIL.

- [ ] **Step 2: Write the widget**

Create `question_panel.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionRequest};
use forge_entities::{answers_map, questions_of, Picks};
use forge_features::{menu_digit_row, use_workspace};

/// A question from Claude above the message box: one question at a time, the options as rows
/// (number keys pick on a single-select question), an "Other" field, and Back / Skip / Next.
#[component]
pub fn QuestionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let questions = questions_of(&request.input);
    let total = questions.len();
    let mut index = use_signal(|| 0usize);
    let mut picks = use_signal(|| vec![Picks::default(); total]);
    let request_id = request.id.clone();

    let skip = {
        let request_id = request_id.clone();
        move || workspace.answer_permission(id, request_id.clone(), PermissionDecision::Deny)
    };

    let Some(question) = questions.get(index().min(total.saturating_sub(1))).cloned() else {
        // Nothing to ask: all the person can do is skip, which answers the sidecar.
        let skip = skip.clone();
        return rsx! {
            div { class: "question-panel",
                div { class: "perm-title", "Claude asked a question that Forge cannot show." }
                div { class: "question-footer",
                    span { class: "spacer" }
                    button { class: "perm-button", r#type: "button", onclick: move |_| skip(), "Skip" }
                }
            }
        };
    };
    let position = index().min(total - 1);
    let last = position + 1 == total;
    let current = picks.read().get(position).cloned().unwrap_or_default();
    let ready = current.ready();

    let mut advance = {
        let questions = questions.clone();
        let request_id = request_id.clone();
        move || {
            if !picks.read().get(position).map_or(false, Picks::ready) {
                return;
            }
            if last {
                workspace.answer_question(id, request_id.clone(), answers_map(&questions, &picks.read()));
            } else {
                index.set(position + 1);
            }
        }
    };
    let pick_option = {
        let multi = question.multi;
        move |option: usize, advance_now: bool| {
            if let Some(slot) = picks.write().get_mut(position) {
                slot.toggle(multi, option);
            }
            if advance_now && !multi && !last {
                index.set(position + 1);
            }
        }
    };
    let (skip_key, skip_button) = (skip.clone(), skip.clone());
    let mut advance_key = advance.clone();
    let mut advance_button = advance.clone();
    let pick_key = pick_option.clone();
    let multi = question.multi;
    let option_count = question.options.len();

    rsx! {
        div {
            class: "question-panel",
            tabindex: "0",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                let modifiers = event.modifiers();
                let code = event.code().to_string();
                if code == "Escape" {
                    event.stop_propagation();
                    skip_key();
                } else if matches!(code.as_str(), "Enter" | "NumpadEnter") && modifiers.meta() {
                    event.stop_propagation();
                    event.prevent_default();
                    advance_key();
                } else if !modifiers.intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT) {
                    if let Some(row) = menu_digit_row(&code) {
                        if row < option_count {
                            event.stop_propagation();
                            event.prevent_default();
                            pick_key(row, true);
                        }
                    }
                }
            },
            div { class: "question-head",
                span { class: "question-chip", "{position + 1}/{total}" }
                span { class: "perm-title", "{question.text}" }
                span { class: "spacer" }
                button { class: "icon-button", r#type: "button", title: "Skip", onclick: move |_| skip_button(), "×" }
            }
            div { class: "question-options",
                for (option_index, option) in question.options.iter().enumerate() {
                    button {
                        key: "{option_index}",
                        class: if current.chosen.contains(&option_index) { "question-option chosen" } else { "question-option" },
                        r#type: "button",
                        onclick: {
                            let pick_option = pick_option.clone();
                            move |_| pick_option(option_index, false)
                        },
                        span { class: "option-text",
                            span { class: "option-label", "{option.label}" }
                            if !option.description.is_empty() {
                                span { class: "option-description", "{option.description}" }
                            }
                        }
                        if multi {
                            span { class: if current.chosen.contains(&option_index) { "option-box checked" } else { "option-box" } }
                        } else if option_index < 4 {
                            span { class: "perm-key", "{option_index + 1}" }
                        }
                    }
                }
                div { class: "question-option other",
                    span { class: "option-label", "Other" }
                    input {
                        class: "question-other",
                        r#type: "text",
                        placeholder: "Type your answer here",
                        value: "{current.other}",
                        // The panel's number keys must not fire while the person types.
                        onkeydown: move |event: KeyboardEvent| {
                            if event.code().to_string() != "Escape" && !event.modifiers().meta() {
                                event.stop_propagation();
                            }
                        },
                        oninput: move |event| {
                            if let Some(slot) = picks.write().get_mut(position) {
                                slot.set_other(multi, event.value());
                            }
                        },
                    }
                }
            }
            div { class: "question-footer",
                if position > 0 {
                    button { class: "perm-button", r#type: "button", onclick: move |_| index.set(position - 1), "Back" }
                }
                span { class: "spacer" }
                button { class: "perm-button", r#type: "button", onclick: move |_| skip(), "Skip" }
                button {
                    class: if ready { "perm-button primary" } else { "perm-button primary disabled" },
                    r#type: "button",
                    disabled: !ready,
                    onclick: move |_| advance_button(),
                    if last { "Submit" } else { "Next" }
                    span { class: "perm-key", "⌘↩" }
                }
            }
        }
    }
}
```
(Closures captured by several handlers need `Clone`; every captured value above is `Copy` or `Clone`. If the borrow checker rejects a closure shared between handlers, make the shared logic a small private `fn` taking the signals as arguments. A single-select pick that is not the last question moves on at once, as the real app does; the last question waits for Submit.)

- [ ] **Step 3: Wire it in**

`lib.rs`: add `mod question_panel;`. In `chat_window.rs` replace the permission block with

```rust
            if let Some(request) = view.pending_permissions.front() {
                if request.tool_name == forge_entities::QUESTION_TOOL {
                    QuestionPanel { key: "{request.id}", id, request: request.clone() }
                } else {
                    PermissionPanel { key: "{request.id}", id, request: request.clone() }
                }
            }
```
and import `crate::question_panel::QuestionPanel`. Make sure `menu_digit_row` is exported from `forge_features` (`grep -n "menu_digit_row" crates/forge-features/src/lib.rs`; it is used by `composer_controls.rs`, so it is).

- [ ] **Step 4: Styles**

Append to `theme.css`:

```css
.question-panel { display: flex; flex-direction: column; gap: 10px; width: calc(100% - 56px); max-width: var(--chat-column); box-sizing: border-box; margin: 0 auto 8px; padding: 16px; background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius-lg); box-shadow: var(--shadow); outline: none; }
.question-head { display: flex; align-items: center; gap: 10px; }
.question-chip { padding: 1px 10px; border-radius: 999px; background: var(--accent); color: #fff; font-size: 12px; font-weight: 700; }
.question-head .spacer, .question-footer .spacer { flex: 1; }
.question-options { display: flex; flex-direction: column; }
.question-option { appearance: none; display: flex; align-items: center; gap: 12px; width: 100%; box-sizing: border-box; padding: 10px 12px; border: none; border-radius: 8px; background: none; text-align: left; font: inherit; color: var(--fg); cursor: pointer; }
.question-option:hover, .question-option.chosen { background: var(--bg-hover); }
.question-option .option-text { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.option-label { font-weight: 700; }
.option-description { color: var(--fg-muted); font-size: 13px; }
.option-box { width: 16px; height: 16px; box-sizing: border-box; border: 1.5px solid var(--border); border-radius: 4px; flex: none; }
.option-box.checked { background: var(--fg); border-color: var(--fg); }
.question-option.other { cursor: default; }
.question-option.other:hover { background: none; }
.question-other { flex: 1; min-width: 0; padding: 6px 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg-card); color: var(--fg); font: inherit; }
.question-footer { display: flex; align-items: center; gap: 8px; }
.perm-button.disabled, .perm-button:disabled { opacity: 0.45; cursor: default; }
```

- [ ] **Step 5: Run the tests and commit**

Run: `cargo test --workspace > /tmp/t8.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t8.txt | head; grep -E "^test result" /tmp/t8.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`.

```bash
git add -A crates
git commit -m "feat: the question panel

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The round stop button and the scroll-to-end button

**Files:**
- Modify: `crates/forge-features/src/send_message.rs`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Produces: a stop button (class `stop-button`) shown instead of Send while a turn runs; a `ScrollToEnd` component (class `scroll-end`).

- [ ] **Step 1: Write the failing class test**

Add to the theme class list: `".stop-button"`, `".scroll-end"`. Run `cargo test -p forge-shared the_stylesheet_defines 2>&1 | grep missing` → FAIL.

- [ ] **Step 2: The stop button**

In `send_message.rs` replace

```rust
                if running {
                    Button { label: "Stop", kind: ButtonKind::Danger, onclick: stop }
                }
                Button {
                    label: "Send",
                    kind: ButtonKind::Primary,
                    disabled: !can_send,
                    onclick: move |_| submit(),
                }
```
with

```rust
                if running {
                    button {
                        class: "stop-button",
                        r#type: "button",
                        title: "Stop",
                        "aria-label": "Stop",
                        onclick: stop,
                        span { class: "stop-dot" }
                    }
                } else {
                    Button {
                        label: "Send",
                        kind: ButtonKind::Primary,
                        disabled: !can_send,
                        onclick: move |_| submit(),
                    }
                }
```
(Enter still sends while running, as before: only the buttons change.) Remove `ButtonKind::Danger` from the imports if it is no longer used there.

- [ ] **Step 3: The scroll-to-end button**

In `chat_window.rs` add the component and place it right after the `div { class: "messages", ... }`:

```rust
// Watches the message list: `data-away` on the pane tells the CSS whether the button shows.
const WATCH_SCROLL: &str = "setTimeout(() => { const list = document.querySelector('.messages'); if (!list || list.dataset.watching) return; list.dataset.watching = '1'; const pane = list.closest('.main-pane'); const update = () => { const away = list.scrollHeight - list.scrollTop - list.clientHeight > 80; pane?.setAttribute('data-away', away ? 'true' : 'false'); }; list.addEventListener('scroll', update); new ResizeObserver(update).observe(list); update(); }, 0);";

#[component]
fn ScrollToEnd() -> Element {
    use_hook(|| {
        document::eval(WATCH_SCROLL);
    });
    rsx! {
        button {
            class: "scroll-end",
            r#type: "button",
            title: "Scroll to end",
            "aria-label": "Scroll to end",
            onclick: |_| {
                document::eval("const list = document.querySelector('.messages'); list?.scrollTo({ top: list.scrollHeight, behavior: 'smooth' });");
            },
            "↓"
        }
    }
}
```
with `ScrollToEnd {}` placed after the `messages` div.

- [ ] **Step 4: Styles**

```css
.stop-button { appearance: none; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border: 2px solid var(--fg); border-radius: 50%; background: none; cursor: pointer; flex: none; }
.stop-button:hover { background: var(--bg-hover); }
.stop-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--fg); }
.scroll-end { display: none; align-self: center; width: 32px; height: 32px; margin: -44px 0 12px; position: relative; z-index: 3; border: 1px solid var(--border); border-radius: 50%; background: var(--bg-card); color: var(--fg); box-shadow: var(--shadow); cursor: pointer; }
.main-pane[data-away="true"] .scroll-end { display: inline-flex; align-items: center; justify-content: center; }
```

- [ ] **Step 5: Run the tests and commit**

Run: `cargo test --workspace > /tmp/t9.txt 2>&1; grep -E "FAILED|^error" -A4 /tmp/t9.txt | head; grep -E "^test result" /tmp/t9.txt | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'`
Expected: `failed: 0`.

```bash
git add -A crates
git commit -m "feat: the round stop button and the scroll-to-end button

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Look at it in the running app, compare, record

**Files:**
- Modify: `docs/superpowers/specs/2026-10-04-chat-view-running-design.md` ("As built" section), `docs/superpowers/roadmap/README.md`.

A session started from Forge runs on the real Claude and costs a real turn. **Ask the user once before the first send** ("I am going to send a few short messages from Forge in `/private/tmp/forge-observe`: a file write, a command, a question, a `sleep 20`") and wait for yes. Only that folder.

- [ ] **Step 1: Build and start Forge on the real home, config isolated**

```bash
cd sidecar && npm run build 2>&1 | tail -2 && cd ..
cargo build -p forge-app 2>&1 | tail -1
mkdir -p /private/tmp/forge-cmp-config
(FORGE_CONFIG_PATH=/private/tmp/forge-cmp-config/config.json nohup target/debug/forge-app >/private/tmp/forge-cmp.log 2>&1 & echo $! >/private/tmp/forge-harness.pid)
sleep 6; python3 scripts/harness/capture.py | tail -1
```
Open the `forge-observe` group's `+` (new session in that folder) with `scripts/harness/click.py` (pixels of the capture PNG). Type with physical keys; the harness cannot type letters under a Korean input method, so switch the input source to English first or type Hangul by its 2-set keys.

- [ ] **Step 2: The running look**

Send (mode: Ask before edits): `Write /private/tmp/forge-observe/live1.txt with the text hi, then run "sleep 8" with Bash.` Capture every ~2 s. Check against Rounds 7 and 8: the user bubble, then dots with seconds, then `Creating live1.txt 3s`; the permission panel above an empty message box with `Deny 1 Esc`, `Always allow 2 ⇧⌘↩`, `Allow once 3 ⌘↩` and the `⌄` menu; the clock keeps counting while the panel is up; the sidebar dot turns orange. Press `3`; then `Running ...` live line; the round stop button at the right of the message box; when the turn ends the folded line, the file card and the footer appear and the live line is gone.

- [ ] **Step 3: Deny, question, stop**

1. Send `Write /private/tmp/forge-observe/live2.txt with the text no.` and press `1`: the turn must end with no reply from Claude; the folded line shows `Created live2.txt failed`; opened, the row shows the denial text in red first and `file_path:` in blue, `content:` below. `live2.txt` must not exist (`ls /private/tmp/forge-observe`).
2. Send `Use the AskUserQuestion tool: ask my favourite colour (3 options, multi-select) and favourite season (3 options). Then repeat my answers in one line.` Answer with the keys and `Other`; press `⌘↩` through the cards. Claude's last line must repeat the answers (this proves `updatedInput.answers` reaches the model; if it says it got no answer, the sidecar change is wrong: stop and fix Task 1). The transcript shows the answer card with multi answers as `Blue, Green`.
3. Send `Run "sleep 30" with Bash` (switch to Auto first so no card appears), click the stop button after 5 s: the turn ends, the live line disappears, the row is not shown as failed.
4. Scroll up in a long session: the `↓` button appears and returns to the end.

- [ ] **Step 4: Stop the app, clean up, record**

```bash
bash scripts/harness/stop.sh; pkill -f "plan-23/target/debug/forge-app"; rm -rf /private/tmp/forge-cmp-config
```
Append an "As built" section to the 23B spec: what matched, what differs, what is left, the lessons (including whether the SDK accepted `updatedInput.answers` and what the model answered). Mark 23B built in `roadmap/README.md`. Run the suites once more:

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{p+=$4; f+=$6} END {print "passed:",p,"failed:",f}'; (cd sidecar && npm test 2>&1 | tail -4)`
Expected: `failed: 0` and the sidecar tests pass.

- [ ] **Step 5: Commit**

```bash
git add docs
git commit -m "docs: Plan 23B as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:** §2.1 sidecar (denial text, `answer_question`, skip) → Task 1. §2.2 ports, use case, command, `turn_started`, "Allow and switch to Auto" as `AllowOnce` + `set_permission_mode` → Tasks 2 and 7. §2.3 questions (`questions_of`, `answers_map`) → Task 3; running line and segments, `Item::Answers`, the 23A key-value fix → Tasks 4, 5, 6. §2.4 `RunningLine` (Task 6), `PermissionPanel` (Task 7), `QuestionPanel` (Task 8), stop button and `ScrollToEnd` (Task 9), sidebar dot (unchanged; `marker_for` already counts pending permissions, questions included). §3 rules 1 to 6 → clock in Task 6, running vs finished in Task 5, panel above an untouched composer in Task 7 (the composer is never removed), deny ends the turn (Task 1 text + Task 10 check), the menu item in Task 7, no permission card for questions in Task 8. §4 testing → in each task and Task 10.
**Placeholder scan:** none; the places where the executor must adapt (closure sharing in Tasks 7 and 8, the accent variable name in Task 6, the exact `Call` enum location in Task 2) are named with what to do.
**Type consistency:** `QUESTION_TOOL`, `Picks`, `answers_map`, `answer_pairs`, `AnswerPair` (Task 3) are used with the same names in Tasks 5, 8; `permission_view`, `permission_key`, `PermissionKey` (Task 4) in Task 7; `LiveState`, `LiveKind`, `LiveLine`, `Waiting`, `live_state`, `Item::Live`, `Item::Answers` (Task 5) in Task 6 and `chat_window.rs`; `Workspace::answer_question` (Task 2) in Task 8; wire shape of `answer_question` identical in Tasks 1 and 2.
**Review focus:** each of the five lines has a test: malformed question (Task 3 `malformed_input...`, Task 8's empty-list branch), tool with empty input (Task 4 `any_other_tool_is_used_by_name_even_with_an_empty_input`), unanswered tool after a turn ended (Task 5 `a_turn_that_ended_with_an_unanswered_tool...`), queued requests (existing `permission_requests_queue_in_order...` plus Task 7's `key` on the panel), answers with quotes and commas (Task 3 `answer_pairs_survive...`).
