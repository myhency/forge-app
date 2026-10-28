# Task list and next-step card (NEW-25)

Plan 70. To be built on branch `plan-70` (from master 30b2544). Observation: round 33 and 33b (`docs/superpowers/roadmap/observations.md`).

## What the desktop app does

The desktop app gives the model two tools of its own, `mcp__ccd_turn__update_plan` and `mcp__ccd_turn__wrap_up`. Their definitions, input schemas and tool results were read from the API request blobs saved in the transcript of the session `다음 로드맵`; the behaviour on screen was watched live in round 33b.

- **`update_plan`** keeps a task list under the model's latest message. First call: `{items: [{content, status}], scope}` with the whole plan (1-12 steps; `status` is `pending`, `in_progress` or `completed`). Later calls: `{advance: true}` (completes the `in_progress` step, starts the next pending one), `{done: [2], start: 4}` (1-based), or a new `{items}` when the plan changes. The tool result is `Plan shown (N steps).`
- **The plan card** is a white bordered card in the chat, at the place where the tool was called, one row per step: a filled grey circle with a check (done), an empty ring (pending), a dashed ring (in progress). It is **live only**: when the turn ends the card is gone and only the narration and the answer remain.
- **`wrap_up`** fills the end-of-turn box: `{recap, ask?, blocked?, actions?}`. `recap` is two or three sentences, `ask` is one question (at most 100 characters), `actions` are one or two buttons (a reply has `label` and `message`; an app action such as `commit` or `create_pr` has `action`). The tool result tells the model to write its closing message and not to repeat the question.
- **The end card** is a bordered card under the closing message, with `×` at the top right, the recap in grey, the question in black and the buttons: the first is black with the key `⌘⏎`, the second light grey. Pressing a button sends its `message` as an ordinary user message at once. The card stays in the chat as a **spent copy**: narrower, grey text, no buttons, the chosen label in grey under the question, no `×`. A later turn's card replaces an earlier live one.

This is also the card NEW-07 called "end card with `Fix them / Not now`": it is not drawn by the app from the answer, the model fills it.

## Scope

Built: both tools in Forge, the plan card, the end card with its spent look, a setting that turns them on and off (default on).

Left out, on purpose:
- The `actions[].action` buttons (`commit`, `create_pr`, `merge`, `fix_ci`, `address_comments`, `resolve_conflicts`, `verify`, `archive`, `new_session`): Forge has nothing for most of them, and a reply button covers the rest. The field is not in Forge's schema.
- `blocked` marking the session as waiting in the sidebar, and the recap's first sentence as the sidebar status.
- `update_plan`'s `session` field and the `new_task` start-session offer. `scope` is accepted and ignored.

## What already exists

- `sidecar/src/session.ts` starts the SDK with `mcpServers: command.mcp_servers ?? {}`, `allowedTools: command.allowed_tools ?? []` and passes inline `settings`. `dynamic_workflows` is the model for a new start flag: `AppConfig.dynamic_workflows` -> `SessionDefaults` (`forge-domain/src/session_defaults.rs`) -> `forge-adapter-sidecar/src/protocol.rs` -> `sidecar/src/protocol.ts` -> `session.ts`. A field that an older parent omits must mean off.
- The SDK has `createSdkMcpServer` and `tool()` (`sdk.d.ts:615`), so the tools run inside the sidecar; no other process.
- Tool calls reach the chat as `Block::Tool` in the transcript (`forge-entities/src/turn_view.rs`). The same file already turns `ExitPlanMode` into `Item::Plan` and `Workflow` into `Item::Workflow` from the call, so a call can become a card without any new channel.
- `build_turns` knows the live state of the turn, and `summarize` counts tool calls for the folded line (a call of an unknown tool is counted as `기타 도구`).
- Settings: `Switch` rows in `forge-widgets/src/settings_overlay.rs` and `forge-features/src/settings.rs` (`dynamic_workflows` is the nearest one).
- The composer's grey suggestion text (`prompt_suggestions`) is what shows at the end of a turn today.

## Design

### 1. The sidecar (`sidecar/src/turn_tools.ts`, new)

`createSdkMcpServer({ name: "ccd_turn", tools: [update_plan, wrap_up], alwaysLoad: true })`. `alwaysLoad` (also set on each tool) keeps the two tools out of tool search: without it the SDK sends only their names, the model never sees the descriptions, and the first real turn called neither tool. The server name is the desktop app's, so the model sees the same tool names, and a session made in the desktop app and opened in Forge draws the same cards from its old calls.

- `update_plan` input (zod): `scope?` (enum, ignored), `items?` (1-12 of `{content, status, ...}`), `advance?`, `done?` (numbers), `start?` (number). The handler checks that at least one of `items`, `advance`, `done`, `start` is present and that the numbers are in range of the last list it was given; a bad call returns an error text the model can read, not a throw. It keeps the last list only to count steps and to say `Plan shown (N steps).` The cards are drawn by the app side from the calls, not from this memory.
- `wrap_up` input: `recap` (required), `ask?`, `blocked?` (accepted, ignored), `actions?` (up to 2 of `{label, message}`). The label and message limits (24 and 300 characters) are applied by the app when it reads the call, since the recorded call keeps what the model sent. Result: `Shown in the next-step box. Your closing message is your answer to the user: write it now, in full, as the last thing you say this turn.` and, when there was an `ask`, `The box asks your question, so do not ask it again.`
- Descriptions are Forge's own words (not the desktop app's text), but carry the same rules: one plan for any request that edits files or runs more than one command; send `update_plan` in the same message as another tool call; keep exactly one step in progress; call `wrap_up` once at the end of such a turn, before the closing message; skip both for a short answer.
- `start_session` gets `turn_tools: boolean` (`protocol.ts`, `protocol.rs`). When true, `session.ts` adds the server to `mcpServers` and `mcp__ccd_turn__update_plan`, `mcp__ccd_turn__wrap_up` to `allowedTools`, so no permission question appears. When missing or false nothing changes.

### 2. The domain and the setting

`AppConfig.turn_tools` (default true, `#[serde(default = "default_true")]`, so a config written before the switch gets true), copied into `SessionDefaults.turn_tools`, sent as `turn_tools` in `start_session`. Settings, Code page: a `Switch` row `Task list and next step` with the text `Claude shows its plan as a list while it works and ends a turn with a summary and a question.` (Korean `작업 목록과 다음 단계`, wording to be settled when the strings are added). It applies to sessions started after the change; a session that is already open keeps what it started with, as the workflow switch does.

### 3. The cards from the transcript (`forge-entities`)

New module `turn_cards.rs`, pure functions with no Dioxus:

- `PlanSteps`: `fn apply(steps: &mut Vec<Step>, input: &Value)` folds one `update_plan` input: `items` replaces the list; `advance` completes the `in_progress` step and starts the next `pending` one (none left: nothing is in progress); `done` marks the 1-based numbers completed; `start` makes that step `in_progress` (and any other `in_progress` step goes back to `pending`); a number out of range is skipped. `Step { content, state }` with `StepState::{Pending, InProgress, Done}`; an unknown `status` reads as pending. `fn plan_of(calls: &[&ToolCall]) -> Option<Vec<Step>>` folds the calls of one turn in order.
- `EndCard { recap, ask: Option<String>, actions: Vec<CardAction> }`, `CardAction { label, message }`; `fn end_card_of(call: &ToolCall) -> Option<EndCard>` reads a `wrap_up` call (empty `recap` -> none; more than 2 actions -> first 2; an action with no `label` and no `message` is dropped; a reply with no `message` sends its `label`; no `ask` -> no card at all (the tool's own rule: a turn with nothing pending ends with no box)).
- Only the full name `mcp__ccd_turn__…` counts (Forge uses the desktop app's server name).

`turn_view.rs`:
- `Item::PlanList(Vec<Step>)` placed just above the live line, after the text that came before the first tool call, **only while the turn is live** (the live state is already passed to `build_turns`). A finished turn emits none.
- `Item::EndCard(EndCardItem { card, state })`, built from the **last** `wrap_up` call of the **last** turn of the transcript:
  - `Live` when nothing came after the turn;
  - `Spent { chosen }` when the next user message equals the `message` (or `label`) of one of its buttons: it stays in place, grey, with the chosen label;
  - nothing otherwise (any other next message makes the card disappear, as a later card replaces an earlier live one).
  An earlier turn's card is drawn only as `Spent`.
- The calls of both tools are left out of the folded line (`summarize`, `build_activity`): they are neither counted nor listed as rows.
- An empty or unknown plan, or a `wrap_up` that fails to parse, draws nothing and the call stays hidden; it does not become an error row.

### 4. The widgets

`forge-entities/src/turn.rs` draws the two new items; the key and the send go through callbacks, like the existing `on_open_file`:

- `PlanListView`: a bordered card, one `plan-step` row per step with the ring or check icon (CSS, no images) and the text; the in-progress ring may spin slowly (`prefers-reduced-motion` stops it).
- `EndCardView` (Live): `×` button (`on_dismiss`), recap, question, buttons. The first button is `kbd`-marked `⌘⏎`; the second is plain. A click calls `on_send(message)` and the card turns Spent when the transcript shows the message. `×` hides the card for this view (kept only while the card is on screen (a component-local flag)).
- `EndCardView` (Spent): about 70% of the column width, grey text, the chosen label under the question, no buttons, no `×`.
- The `⌘⏎` key works when the composer is empty and a live end card has a first button; with text in the box `⌘⏎` keeps its present meaning. Where this is wired is decided in the plan, by reading how the composer handles `⌘⏎` now (queue, `⌘⏎` adds to the queue while a turn runs, so the key only applies when no turn is running).
- With a live end card, the composer's grey next-prompt suggestion is not shown (the card is the suggestion).

### 5. Strings

All new text goes through `tr!` with a Korean entry in `ko.rs` (the table is sorted; the test `every_sentence_in_the_sources_has_a_korean_text` checks it).

## Testing

- Sidecar (`vitest`): the server lists both tools with the right names; `update_plan` accepts each of the four input shapes and refuses an empty call and an out-of-range number with a readable text; `wrap_up` keeps at most 2 actions and words the result with and without `ask`; `session.test.ts`: with `turn_tools` the SDK options carry the server and the two allowed names, without it they do not.
- Rust: `apply` (items, advance past the end, done, start, out of range, unknown status), `end_card_of` (every drop rule above), `build_turns` (plan card only while live and at the last call; end card Live, Spent by label, Spent by message, gone after another message; both tools hidden in the folded line; the desktop app's prefixed names), the config default for a file with no `turn_tools`, the protocol field in `protocol.rs` tests (present and defaulting to off for an older parent).
- Real window (harness): a sample session whose transcript holds an `update_plan` call and a `wrap_up` call, for the Live card, and a second one whose next message is a button's text, for the Spent card; the plan card is checked with a session that is running (the fixture has to keep a turn live, or the card is looked at through a unit test of the view data only, said so in the report).
- Real SDK: one real turn with the real sidecar in a throwaway folder, to see that the model calls both tools and that the permission question does not appear. The tool descriptions are only proven by this.

## Risks and open points

- The model calls these tools only as often as the descriptions make it. The first real turn may show it calls neither or calls `wrap_up` every time; the wording is adjusted then, and that is part of this plan, not a new one.
- `strictMcpConfig: true` is set today. An in-process SDK server passed in `mcpServers` is expected to be allowed; the first real turn confirms it, and if it is not, the fallback is to drop the flag only when `turn_tools` is on.
- Whether to also show the card of a session resumed from a transcript that ends in a live-looking `wrap_up` (the process died before the next message): it is drawn Live until the next message, which matches the desktop app's behaviour as far as observed.
- Windows is not checked (no Windows environment).
