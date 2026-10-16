# Plan 45: finished tasks, tokens and failed workflows (OTH-10, the last part)

Date: 2026-10-17. Branch `plan-45`. Evidence: observation round 18 and its part 2 (`docs/superpowers/roadmap/observations.md`, OTH-10), checked against the session record `~/.claude/projects/<project>/<session>.jsonl` and the workflow's `journal.jsonl`.

## Goal

The Background tasks panel and the chat treat a task that has ended the way the desktop app does. Today a task that ends is dropped: the panel loses its card, the chat shows a grey `Workflow ran <name>` whatever happened, and a workflow card shows no tokens.

Success, checked in the real window with a small workflow run:
- A running workflow card shows `N agents  157.9k tokens`.
- When a workflow or a background Bash command ends, its card moves to a `Finished N` section of the panel and stays there (with its end state, time, tokens and, for a failure, the error).
- A stopped or failed workflow says so in the chat line and in its card; an agent whose permission was denied has a red dot.
- Closing the session and opening it again (or opening an old session) shows the same `Finished` list.

Decided with the user (2026-10-17): amber dots for an agent waiting for permission are **left out**. The desktop app did not show them in three runs (obs-a, obs-b, obs-c, 2 and 3 agents, up to 54 s of waiting), although round 16 once saw them. Forge shows no amber until a run reproduces it.

Not in this plan: the per-agent token column of the expanded panel (the desktop app's numbers per agent could not be explained from the transcripts), a `Finished` fold per card, notifications.

## What the desktop app does (round 18, summary)

- Running workflow card: `Workflow  <clock>` then `N agents  <tokens> tokens`, the description, `Steps` folds with dots. The token number is the sum, over the agents, of the token total (input + cache creation + cache read + output) of the agent's last call. The SDK's own number for it is `usage.total_tokens` in `task_progress` and in `task_notification` (obs-a: 157,385 from `<subagent_tokens>`, screen 157.4k; obs-c: 157,469, screen 157.5k).
- The chat card shows no tokens.
- When a task ends the `Running` section goes (if it was the only one) and a `Finished N` section appears under it, opened as the person left it, newest card first.
- Finished workflow card: name and total time at the right, `Workflow  Completed | Stopped | Failed` (the last in red), `N agents  <tokens> tokens`, the description, a pink box `Error` with the error text for a failure, then the steps (folded, grey dots; a denied agent red; an agent that never finished in a stopped run a hollow dot; the count `n/m` counts only agents that finished).
- Finished Bash card: the description and `Bash  Completed  12s`; no tokens, steps or stop button.
- Chat line: `Workflow ran <name>`, then ` stopped` or ` failed` (red) when it did not complete. Only workflows have this line.
- The trash icon clears every finished card (and goes grey); the chat lines stay.
- The list belongs to the session and is rebuilt from its record.

## Design

### 1. Where the facts come from

One value describes an ended task: `FinishedTask { task_id, status, summary, tokens: Option<u64>, duration_ms: Option<u64> }` with `status` one of `Completed`, `Failed`, `Stopped` (`forge-domain`, next to `RunningTask`). `Session` gets `finished_tasks: Vec<FinishedTask>` in the order the tasks ended.

Two sources fill it, so the list is the same live and after a reload:

- **Live:** the sidecar's `task_finished` event already carries `status` and `summary`; it gets `tokens` and `duration_ms` from the SDK's `task_notification.usage` (`total_tokens`, `duration_ms`; both optional). `apply_sidecar_event` pushes a `FinishedTask` into the session. The session already holds the messages, so the list is saved nowhere else.
- **Reload:** the SDK writes each notification into the session record as a user line whose text is a `<task-notification>` block (`task-id`, `status`, `summary`, optional `usage` with `subagent_tokens` and `duration_ms`). Today the loader turns such a line into a user message (a raw XML bubble). Instead it parses the block into a `FinishedTask` and does not make a message. The same parse is used for the `queue-operation` duplicate lines: a task id already in the list is not added twice. A block that does not parse is skipped, never an error, as with the journal.

The notification text is not a documented format, so the parser reads loosely (tags by name, any order, unknown tags ignored) and has tests with the real blocks of obs-a (completed), obs-b (stopped) and obs-c (failed, with a multi-line `summary`).

### 2. Running tokens

The sidecar forwards `task_progress` as `{ type: "task_progress", task_id, total_tokens }` (the field `usage.total_tokens`; a message without it is dropped). `SidecarEvent::TaskProgress` becomes `SessionUpdate::TaskProgress`, and `RunningTask` gets `tokens: Option<u64>`, kept across `BackgroundTasks` updates the way `seen_at` is. Before the first progress event the card shows no token part, as now.

`task_progress` is an assumption for a workflow task: the first step of the plan sends a real workflow through the sidecar and prints the events. If it does not come for `local_workflow`, the number falls back to the sum the desktop app uses, computed from the agents' transcript files in the workflow's folder (each `agent-*.jsonl`, the last assistant line's usage); that fallback is written only if the probe needs it.

### 3. Cards (`forge-entities::background`)

`task_cards(messages, running)` becomes `task_cards(messages, running, finished) -> TaskCards { running: Vec<TaskCard>, finished: Vec<TaskCard> }`. A finished card is built like a running one (the Workflow call gives name, description, phases and the journal path; the Bash call gives description and command) and has `end: Option<TaskEnd>` with the status, the summary, the tokens and the duration. `finished` is newest first. A task id that is in both lists (the notification arrived before `background_tasks` lost it) is shown as finished only.

`card_key` includes the end state so a running card is replaced by its finished card.

Agent states in `workflow_progress.rs` grow a third value, `Failed` (a journal line `{"type":"failed","agentId":…}`). `parse_journal` sets it; a later `result` for the same agent wins (an agent may be retried). `Step::done()` still counts only `Done`.

### 4. Panel and chat

- The panel body is: `Running` (when there is a running task) and `Finished N ⌄/›` (when there is a finished one). The fold's open state is kept per session, like the Files tabs (Plan 25D), and starts closed.
- The finished workflow card has no stop button and shows the end state, the time (`duration_ms`, else the clock from `seen_at` to the end), the tokens, the error box for `Failed`, and steps folded. A running card shows the tokens when it has them.
- The finished Bash card is the small card: description, `Bash  <status>  <time>`; the command and output are not shown after the end (the desktop app does not).
- Dots: blue running, grey done, red failed, hollow (outline) for an agent that did not finish in a stopped run, one pale dot for a step that has not started. The step name is red when it has a failed agent.
- The chat line shows `Workflow ran <name>` plus `stopped` or `failed` (red) from the finished list. The running chat card also shows red dots.
- The trash icon hides the session's finished cards: their task ids go into a per-session `cleared` set that lives next to the fold's open state, in the app's memory only. Nothing on disk changes, and the record still holds the notifications, so after Forge restarts (or the session is reloaded) the list is back. This is a decision, not an observation: whether the desktop app's cleared list survives a restart was not seen. A task that ends after the click is shown (only the ids present at the click are hidden). The icon is grey when no finished card is shown.

### 5. Errors and edge cases

- A task that ends without a notification (the sidecar dies): it leaves `background_tasks`, no `FinishedTask` arrives, and the card is dropped as today. The chat line stays `Workflow ran <name>` with no suffix.
- A workflow whose journal is gone (moved or deleted): the finished card shows its facts without steps.
- A resumed workflow keeps its task id (the SDK says a resumed task gets a new run id); the newer notification replaces the older entry with the same id.
- Windows paths and the journal's location rules are unchanged (`journal_path` still refuses any folder that is not a `…/subagents/workflows/wf_*`).
- Translations: `Finished`, `Completed`, `Stopped`, `Failed`, `Error`, `stopped`, `failed` and the new token text go through `tr!` with Korean in `ko.rs`. The agent-count format keeps the existing key.

## Testing

- Unit (pure): the notification parser (three real blocks, a block with missing tags, a block with extra tags, a duplicate id); `parse_journal` with `failed`; `task_cards` with finished tasks (order, both lists, the same id in both, a workflow with no journal); the status labels.
- Sidecar (`vitest`): `task_progress` forwarding and `task_finished` with `usage`.
- Application: `TaskFinished` and `TaskProgress` through the actor into the view; the loader turns a notification line into a `FinishedTask` and no message; a session loaded twice gives the same list.
- Real window (the harness and the desktop app's `forge-observe` folder, as in round 18): a two-agent workflow to the end; a denied agent; a stop; a Bash task; close and reopen the session; compare each card with round 18's text.

## Order of work

1. Probe: run a real workflow through the sidecar and print the `task_progress` and `task_notification` messages (check `usage`, `status` and that `task_progress` comes for a workflow); read a reloaded session in Forge to see today's raw-XML bubble. Write what was seen into the spec before building.
2. Domain and parser; loader change.
3. Sidecar events; actor.
4. Cards and journal state.
5. Panel, chat line and styles.
6. Real-window check; docs and the roadmap row.

## Risks

- The notification block format may change in a later Claude Code release; the parser is loose and a block it cannot read only means a missing card.
- The probe may show `task_progress` is not sent for workflows; the fallback of section 2 covers it.

## As built (2026-10-17)

Built on `plan-45` in eight steps with a reviewer per step and a final whole-branch review (one Important found and fixed: a resumed workflow with the same task id showed as its old finished card).

**Probe (real SDK, `sidecar/scripts/print_task_events.mjs`, one small workflow):** `task_progress` is sent for a `local_workflow` task with `usage.total_tokens`; the notification's `usage.total_tokens` equals the record's `<subagent_tokens>` (20,730 = 20,730); the notification comes before `background_tasks_changed: []`. So the spec's fallback (sum of the agents' last calls) was not needed. The SDK also sends `workflow_progress` (agents with labels) inside `task_progress`; Forge still reads the journal and does not use it.

**Real window (Forge through the harness, `dynamic_workflows` switched on in its config, real Claude):**
- A running workflow card shows `에이전트 2개  18.3k 토큰` and the number grows; the chat card shows no tokens.
- A finished workflow card (`fa`): total time at the right, `워크플로우  완료됨`, `에이전트 3개  27.9k 토큰`, description, folded steps with grey dots; `완료 N` fold under `실행 중`, newest first, closed until opened.
- A failed run (`fb`, script throws): `워크플로우  실패` (red), the pink `오류` box (`Error: fb boom` and its two `at` lines), chat line `워크플로 실행됨 fb 실패` with `실패` in red.
- A Bash background task joins the fold as the small card (`Bash  완료됨  10s` live).
- The trash icon clears the cards, greys out, and the chat lines stay.
- After Forge was restarted and the session opened again the list was rebuilt from the record (3 cards, the failed one with its error and tokens) and no raw `<task-notification>` bubble showed.
- Not checked: the stop button of a run (stopped state), a red dot for an agent that failed (the two agents whose requests were denied finished with a report instead of failing), Windows.

**Differences from the design:**
- A new pill under the chat: when nothing runs and finished cards are shown, it reads `작업 N개 완료` / `N tasks finished` and opens the panel. The design had no way to open the panel when nothing ran (found in the real window; after a restart the list was unreachable).
- The trash `cleared` set holds a per-end key (`id:status:tokens:duration`), not the bare task id, so a resumed task that ends again shows after a click.
- A task that is new to the running set (a resume) leaves the finished list at once.
- The notification parser reads only the session record's user lines; the `queue-operation` duplicates are not parsed (the user line is always written too).
- After a restart a finished Bash card has no time (its notification has no duration); a workflow card has it. A task the person stopped leaves no notification in the record, so its card is gone after a restart (live it shows `Stopped`).
- Amber dots were left out as decided; dot colours are blue, grey, red and hollow (stopped).

**Left:** per-agent token column, amber dots (not reproduced), a retry when a finished card's single journal read fails, deferred minors in the review ledger (parser hypotheticals, a duration slightly long for a task that vanished before its notification).
