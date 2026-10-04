# Chat view, running look (Plan 23B)

Date: 2026-10-04. Branch `plan-23`. What the real app does: `docs/superpowers/roadmap/observations.md`, Round 7 (running state, permission prompt) and Round 8 (the permission card, a denial, the opened denied row). Follows Plan 23A (`2026-10-04-chat-view-static-design.md`, built and merged).

## 1. Purpose

While a turn runs, Forge's chat shows the finished-turn look growing in place and a plain permission card that replaces nothing. The real app shows one live line, a clock, a stop button, a permission card above the composer, and a question card. 23B makes a running conversation look and behave like that. 23C (side panels: opening files, background tasks, suggested next prompt) comes after.

Success: in the real Forge window, send a message that writes a file and runs a command; the running line reads like the real app's (verb, object, seconds); the permission card has the real app's buttons and keys and its choice reaches the model; the model asks a question and the answer comes back to it; stop interrupts. The user judges by eye; tests cover the view model, the card choice rules, the sidecar commands and the denial text.

Decided with the user (2026-10-04): the clock shows seconds only, no token count (the real app shows "6초 · 202 토큰"; Forge has no live token number and will not invent one); the question card is in 23B; the permission card's menu item "Allow and switch to Auto mode" is in; the real app was observed once more for the card's left button (it is Deny).

Not in 23B: token count in the clock, undo and rewind icons, anything that opens a side panel (23C), the background-task pill, the suggested next prompt, dark theme checks (the user does those).

## 2. What changes where

### 2.1 Sidecar (`sidecar/src/`)

- **Denial text.** A denied tool call returns, as the SDK's `message`, the real app's text: `The user doesn't want to proceed with this tool use. The tool use was rejected (eg. if it was a file edit, the new_string was NOT written to the file). STOP what you are doing and wait for the user to tell you how to proceed.` (today: `Denied by user`). The model then stops, as in the real app. The existing `Interrupted` and `Sidecar is shutting down` messages stay.
- **New command `answer_question`:** `{type: "answer_question", permission_request_id, answers: {<question text>: <answer>}}`. The sidecar keeps the input of each pending `canUseTool` request (it already keeps `resolve` and `suggestions`). On `answer_question` for a pending `AskUserQuestion` it resolves `{behavior: "allow", updatedInput: {...input, answers}}` (the SDK's contract: answers map a question's text to the chosen label; several labels of a multi-select are joined with `,` and no space, as the real app's transcript shows (`"파랑,초록"`); the answer card shows them with `, `; a typed "Other" answer is the typed text). For any other pending request it ignores the command.
- **Skipping a question** is a normal `permission_response` with `deny`; for `AskUserQuestion` the sidecar sends `message: "The user skipped the question."` instead of the denial text above.
- `permission_request` events already carry `tool_name` and `input`; nothing changes on the way out.

### 2.2 Ports and application (`forge-ports`, `forge-application`)

- `SidecarPort::answer_question(permission_request_id: String, answers: BTreeMap<String, String>)`; the command is added to the sidecar adapter's wire format and to the mock.
- `LiveCommand::AnswerQuestion { id, answers }` and `LiveSessionHandle::answer_question(id, answers)`; the actor removes the pending request exactly as `Approve` does and calls the port.
- `LiveSessionView.turn_started: Option<std::time::SystemTime>`: set when a `Send` starts a turn (the place that sets `Phase::Running`, `live_session_actor.rs:173`), cleared when the turn ends (`TurnCompleted`, a failure, `SessionEnded`). It keeps running while a permission waits.
- `PermissionDecision` is unchanged. "Allow and switch to Auto mode" is `AllowOnce` followed by `SetMode(PermissionMode::Auto)` sent by the widget; the existing `set_permission_mode` path does the rest. (The two commands arrive in order; the mode applies from the next tool call.)

### 2.3 Entities (`forge-entities`)

**Questions.** `question.rs`: `pub struct Question { header, text, options: Vec<QuestionOption { label, description }>, multi: bool }` and `pub fn questions_of(input: &Value) -> Vec<Question>` reading `AskUserQuestion`'s input (`questions[].{header, question, options[].{label, description}, multiSelect}`); missing or malformed fields give an empty list, never a panic. `pub fn answers_map(questions, picks) -> BTreeMap<String, String>` builds the answer text (labels joined by `,`, "Other" text as typed).

**Running line.** `turn_view.rs` gets the running state:
- `build_turns(messages, view, running)` becomes `build_turns(messages, view, live: Option<&LiveState>)`, where `LiveState { waiting: Option<Waiting> }` and `Waiting` is `Permission { tool_name, input }` or `Question`. `running` is `live.is_some()`.
- In a running last turn the folded activity is **not** built. The turn shows exactly: its text blocks that come before its first tool call (ordinary text, as in 23A), then one `Item::Live` as the last item. Every block from the first tool call on (finished tools, text between tools, text after the last finished tool) is held back until the turn ends, when 23A's folded line, card and footer take its place. (Real app: only the current step shows while it runs.) A running turn with no assistant block yet shows just the user bubble and a `Thinking` Live line.
- `Item::Live(LiveLine { kind, label, subject, changes })` with `kind`:
  - `Tool` when the last block is a tool call without a result: `label` is the present-tense verb by tool (`Creating` for Write, `Editing` for Edit/MultiEdit/NotebookEdit, `Reading` for Read, `Searching` for Grep/Glob, `Using <name>` for others; for Bash the call's `description`, else the command, as written), `subject` the file name for file tools, empty for Bash and others.
  - `Thinking` when the model is producing text or thinking (the last block is Text or Thinking, or there are no blocks yet in this turn).
  - `NeedsInput` when `Waiting` is set: shown as the pill `Input needed`, with `changes` the `+a −r` of the pending write or edit when it has a diff.
- Finished turns are unchanged (23A's summary line, card, footer), except that an `AskUserQuestion` call is never part of the folded span: it splits the turn into segments, each folded on its own, and the call itself becomes an `Item::Answers` card between them (23A did not build the answer card; this task does). The answer pairs are read from the call's result text, `Your questions have been answered: "<question>"="<answer>", ... You can now continue ...`, matched against the question texts in the call's input. A pending question (no result yet) draws nothing in the transcript.

**Opened tool rows (23A fix).** A tool row without a diff opens to `key: value` lines (key in grey mono, value in normal text, a string value that looks like an absolute path in blue underline), not pretty JSON; a failed row shows its error text first in dark red. A denied call's error is the denial text above, so it shows in full.

### 2.4 Widgets (`forge-entities`, `forge-widgets`, `forge-features`)

- **`RunningLine`** (in `turn.rs`): draws `Item::Live`. For `Tool`: a spinner, the label, the subject, `4s`, a chevron (not clickable in 23B). For `Thinking`: three orange dots and `6s`. For `NeedsInput`: the orange ringed dot, the pill `Input needed ›` and the change chips. The seconds come from `turn_started` and re-render once a second through a Dioxus interval that only exists while a Live item is on screen.
- **`PermissionPanel`** (new, `forge-widgets`): shown by `ChatWindow` **above the composer** when `pending_permissions` is not empty and the request is not `AskUserQuestion`; the composer below stays (empty, not disabled, with the stop button). Layout, from Round 8: title `Claude wants to <verb> <file name>?` (verb: `create` for Write, `edit` for Edit/MultiEdit/NotebookEdit, `run` for Bash, `use` for others; for Bash the subject is the description), the file name, a grey mono box with the full path (for Bash the command), the path again as plain text, the diff block (reuse `DiffView`), then the buttons row. Buttons: left `Deny` with key chips `1` and `Esc`; right `Always allow` with `2` and `⇧⌘↩`, then `Allow once` (black) with `3` and `⌘↩`, joined to a `⌄` that opens a one-item menu `Allow and switch to Auto mode` / `The rest of this session is checked automatically.` Keys work while the card is on screen and focus is not in a text field that is not the composer: `1` and `Esc` deny, `2` and `⇧⌘↩` allow always, `3` and `⌘↩` allow once. The old `PermissionCard` / `PermissionActions` are removed.
- **`QuestionPanel`** (new): shown instead of the permission card for `AskUserQuestion`. A round chip `1/2` and the question in bold, `⌄` and `×` at the right (`×` skips). One row per option: bold label, grey description; for a single-select question a small box with the number key (1 to n), pressing the row or the key picks it and moves to the next question; for multi-select a checkbox, picking does not advance. The last row is `Other` with a text field `Type your answer here`. Footer: `Back` (from the second question), `Skip`, and a primary `Next ⌘↩` (grey until something is picked; `Submit` on the last question). Submit calls `answer_question`. After the answer the transcript shows the answer card (23A's existing look: narrow bordered card, each question grey, the answer black), built from the tool call's input and its result.
- **Stop button.** `SendMessageBox`: while running, the send arrow is replaced by a round stop button (a ring with a dot) that interrupts; the text `Stop` button is removed. A new `ScrollToEnd` round button (`↓`) floats centred above the composer when the message list is not scrolled to the end and disappears when it is.
- **Sidebar dot.** The orange dot for "waiting for input" already exists (`sidebar.rs` marker) and is driven by `pending_permissions`; a pending question uses the same marker. No change beyond the test.

## 3. Rules from the observations (so the plan can be checked against them)

1. The clock starts at send and does not stop while input is awaited.
2. A running turn never shows the folded line; a finished one always does.
3. The permission card sits above an untouched composer; answering it removes it at once and the Live line goes back to `Thinking` or the next tool.
4. Deny ends the turn; the transcript then shows the folded line with `failed` for the denied step and no further reply (that follows from the denial text; Forge does not suppress anything itself).
5. `Always allow` sends `AllowAlways`; the menu item sends `AllowOnce` and sets Auto.
6. An `AskUserQuestion` never shows the permission card.

## 4. Testing

- Sidecar (node tests in `sidecar/`): the denial text exactly; `answer_question` resolves with `updatedInput.answers` for a pending question and is ignored for any other request or an unknown id; skip uses the skip message; multi-select answers are joined by `, `.
- Ports/application: wire format of `answer_question`; the actor removes the pending request and calls the port; `turn_started` is set on send and cleared on completion, failure and session end.
- Entities: `questions_of` (normal input, missing fields, an empty list, a question with no options, `multiSelect` absent); `answers_map`; `build_turns` with a running turn: no Activity, Live line for each kind, text before the first tool kept, a turn that has finished tools and waits for permission shows `NeedsInput` with the write's `+a −r`; a finished turn unchanged (the 23A tests keep passing); key-value rendering of a tool input (a nested object, a non-string value, a path value, an error first).
- Widgets: no unit tests for markup (as in 23A); the pure parts (title text per tool, key to decision mapping, the answer assembly) are functions in `forge-entities` with tests.
- Visual: run Forge on a throwaway folder, send a message that writes a file in manual mode, deny once, allow once, ask for a question (`AskUserQuestion`, one single and one multi select), stop a `sleep 20`. Compare with Round 7 and 8; list the differences left in an "As built" section here. A session started from Forge runs on the real Claude: only in `/private/tmp/forge-observe`, and tell the user before sending.

## 5. Risks

- The `canUseTool` contract for `updatedInput.answers` is read from the SDK's type text; confirm against the real SDK in the visual check (the question must come back to the model with the answer). If the SDK rejects it, the sidecar change is isolated to one function.
- `build_turns` changes its last parameter; its 23A call sites and tests change mechanically.
- A once-a-second re-render while running must not rebuild the whole transcript. Only the `RunningLine` component subscribes to the clock; `build_turns` is not re-run for it.
- The real app's folding rule for a running turn is inferred from short runs (a two-step task). If a long run shows earlier finished steps staying visible, only rule 2 changes.
- Korean input method: the card keys (`1`, `2`, `3`) must work by physical key, as in Plan 15; `Esc` and `⌘↩` are not affected.
