# Chat view, side panels and suggestions (Plan 23C)

Date: 2026-10-04. Branch `plan-23`. What the real app does: `docs/superpowers/roadmap/observations.md`, Round 7 (a file row opens a panel; background task), Round 8 part 3 (a written markdown file and its preview panel, the editor menu), Round 7 part 2 (background task pill and panel; the suggested next prompt). Follows 23A (static look) and 23B (running look), both merged.

## 1. Purpose

The real app connects the conversation to what it produced: a file name in the chat opens the file at the right, a command running in the background has a pill and a panel with its live output and a stop button, and the message box suggests the next prompt. Forge shows files only through the dock's Files panel, ignores background tasks, and has no suggestion. 23C closes these three gaps.

Success: in the real Forge window, click a file name under a turn and the Files panel opens on it; a markdown file shows rendered with a `</>` switch to the source; start `sleep 20` in the background and see the pill, then the panel with the command, its seconds, its output and a working stop button; after a turn the message box shows a grey suggestion. The user judges by eye; tests cover the parsers, the path rules, the view model and the commands.

Decided with the user (2026-10-04): all of file opening, markdown rendering and the background-task pill and panel are in; the suggested next prompt is in too (it turned out cheap: an SDK option, "nearly free" by the SDK's own documentation, not an extra model call as first thought).

Not in 23C: editing a file in the panel (the real panel is an editor with auto save and tab size; Forge's Files viewer stays read only), the preview-tab behaviour (an italic tab that the next file replaces), the panel's search and `⋮` menu, files outside the session folder, completed tasks listed after they finish, sub-agents as tasks (the pill counts every task the SDK reports, but the card shows only a description for those that are not a Bash command), undo and rewind icons, dark theme checks.

## 2. Sidecar (`sidecar/src/`)

- **Events out** (new variants of `SidecarEvent`, snake_case on the wire):
  - `background_tasks { tasks: [{ task_id, description, task_type }] }` from the SDK's `background_tasks_changed` message. The SDK says this is a level signal with replace semantics (a missed edge cannot leave a stale running mark) and that nothing is sent at startup, so the Rust side starts from an empty set. Tasks the SDK marks `ambient` are left out.
  - `task_finished { task_id, status: "completed" | "failed" | "stopped", summary }` from `task_notification`. It is informational: the set comes from `background_tasks`.
  - `prompt_suggestion { text }` from the SDK message `{ type: "prompt_suggestion", suggestion }`.
- **Command in:** `{ type: "stop_task", id, task_id }` calls the query's `stopTask(taskId)`. A failure answers `setting_failed` (the banner shows it; the session goes on). Without a running query it answers `setting_failed` too.
- **Query options added:** `promptSuggestions: true`, and `perTaskStopAffordance: true` (Forge now has a per-task stop control, so the SDK's Stop (`interrupt`) ends only the turn and spares background tasks; without the declaration the SDK kills them on Stop). The `QueryHandle` type gets `stopTask?`.
- Message handling: the `system` case gains the subtypes `background_tasks_changed` and `task_notification`; a new case for `prompt_suggestion` (a top-level message type, not a system one).

## 3. Ports and application

- `SidecarEvent::BackgroundTasks { tasks: Vec<BackgroundTaskInfo> }`, `SidecarEvent::TaskFinished { task_id, status, summary }`, `SidecarEvent::PromptSuggestion { text }`; `SidecarPort::stop_task(task_id)`; `StopTaskUseCase`; `LiveCommand::StopTask`; `LiveSessionHandle::stop_task`; `Workspace::stop_task`.
- `forge-domain`: `BackgroundTaskInfo { id: String, description: String, task_type: String }`.
- `LiveSessionView` gains `background_tasks: Vec<RunningTask>` (`RunningTask { info: BackgroundTaskInfo, seen_at: SystemTime }`; a task keeps its `seen_at` while it stays in later lists, new ids get now) and `suggestion: Option<String>`. Rules: `BackgroundTasks` replaces the list; the list is emptied when the sidecar ends or fails (`fail`, `SessionEnded`, `Errored`); `suggestion` is set by `PromptSuggestion` while no turn runs, cleared on every send and when a turn starts.
- `FilesPort::read_tail(path: &Path, max_bytes: usize) -> Result<String, FilesError>`: reads the last `max_bytes` of a file given by an absolute path the caller has already checked (the adapter does not confine it, unlike `read`); lossy UTF-8; a file that cannot be read is `FilesError::Failed(message)` (there is no not-found variant); the panel shows a missing output file as `No output yet`.
- `Workspace::open_file(id, absolute_path)` and the file request (section 4).

## 4. Opening a file from the chat

- **Pure rule** (`forge-entities`, `file_open.rs`): `relative_to_folder(folder: &Path, path: &str) -> Option<String>`: `Some(relative)` when the absolute `path` is inside `folder` (after a lexical clean: no `..` escapes, no trailing slash trouble), else `None`. A path equal to the folder gives `None` (not a file).
- **Workspace:** `open_file(id, path)` computes the relative path against the session's folder; on `None` it sets the workspace notice `Only files inside the session folder can be opened here.`; otherwise it opens the Files panel if it is closed (`change_dock`, `toggle(Panel::Files)`) and stores the request (`file_requests: HashMap<LiveSessionId, String>` in a signal). `FilesPanel` reads its session's request in an effect, calls its existing `open_file(path)` and clears the request. (So a request made while the panel is closed is served when it mounts.)
- **Where the click is:**
  - a row of the end-of-turn file card (`FileCardView`): the name is a button;
  - the file name of a file row in the opened activity (the row itself still opens/closes its diff; only the name is a link);
  - a link in answer text: `known_paths(messages)` is the set of absolute `file_path` / `notebook_path` values of the Write, Edit, MultiEdit, NotebookEdit and Read calls in the session. `render_markdown_linked(source, known)` (a new function next to `render_markdown`, same safety) turns an inline code span whose text equals a known path, or equals the final name of exactly one known path, into `<a class="file-link" href="forge-file:<absolute path>">name</a>`. Nothing is checked on disk.
  - A click on `a.file-link` is caught by one delegated listener installed once per page with `document::eval` (like the scroll watch: a `setInterval` that installs it once, because the list may not exist yet); it calls `dioxus.send(path)` and `ChatWindow` receives it in a long-lived future and calls `workspace.open_file`. Other links (`http`, `https`, `mailto`) keep today's behaviour.
- Which tool calls count as known paths is the only rule; a file the model only names in prose is plain text.

## 5. Markdown in the Files viewer

- `is_markdown(path) -> bool` (`.md`, `.markdown`, case-insensitive).
- `FilesPanel`: for a markdown file the viewer starts rendered (`render_markdown(text)` in a `.markdown` block, links `http`/`https` unchanged) and a button at the right of the viewer header, `</>`, switches to the existing highlighted source with line numbers; the button then shows an eye (back to rendered). The choice is per opened file and resets when another file opens. Non-markdown files show only the source, without the button.
- Rendering is memoised with the existing `shown` memo (not on every filter keystroke).

## 6. Background tasks

- **Where the data comes from:** the Bash tool result text `Command running in background with ID: <id>. Output is being written to: <path>. You will be notified when it completes. ...` (seen in a real transcript). `background_ref(output: &str) -> Option<BackgroundRef { task_id, output_path }>` reads it (task id: letters and digits; path: up to `. You will be notified`); anything else is `None`.
- **View model** (`forge-entities`, `background.rs`): `task_cards(messages, running: &[RunningTask]) -> Vec<TaskCard>`: one card per running task, in the SDK's order; the card's `command` and `description` come from the Bash call (`run_in_background` input) whose result names that task id, else the SDK's description and no command; `output_path` only when `valid_output_path(path, task_id)` holds: absolute, no `..`, ends with `/tasks/<task_id>.output`. The pill text is `N background task(s)`.
- **The pill:** a blue pill at the end of the transcript (below the last item, above the message box) while `running` is not empty: `1 background task` / `2 background tasks`. It is lighter blue with a darker fill while the panel is open (as seen). A click opens the panel (`Panel::Tasks`).
- **`Panel::Tasks`** (new dock panel, title `Background tasks`, its own column like Files and Terminal): sections: `Running` with one card per task; an empty list shows `No running tasks`. A card: the description (bold), a line `Bash  29s` (type and a live clock from `seen_at`; the clock is the `Clock` component of 23B), the command in a grey mono box with a `$ ` prefix, the output (the last 20 KB, polled every second with `FilesPort::read_tail` through `run_blocking`; `No output yet` while empty or missing), and a square stop button at the right that calls `Workspace::stop_task`. A note at the bottom: `Tasks end when Forge closes the session.`
- A task that finishes disappears from the list (the SDK's next `background_tasks`); nothing else shows it.
- The pill and panel never block the composer.

## 7. The suggested next prompt

- The message box's placeholder is the suggestion (grey) when: a suggestion exists, the box is empty, and no turn is running; otherwise `Message Claude…` (a new session keeps its own wording).
- Tab with an empty box and a suggestion fills the box with it (Forge's choice: the real app's accept key was not observed) and clears the suggestion. Enter on an empty box sends nothing, as today.
- The suggestion is cleared by a send and when the turn starts. The SDK suppresses it in several cases (first turn, plan mode, after API errors, near the plan limit); then there simply is none.

## 8. Testing

- Sidecar (vitest): `background_tasks_changed` becomes `background_tasks` (ambient left out, empty list passes); `task_notification` becomes `task_finished`; `prompt_suggestion` becomes the event; `stop_task` calls `stopTask` and a throw answers `setting_failed`; the query options carry `promptSuggestions` and `perTaskStopAffordance`.
- Rust: wire shape of `stop_task` and of the three events; the use case; the actor (`StopTask` reaches the port; `BackgroundTasks` replaces the list and keeps `seen_at` of tasks that stay; the list empties on failure and session end; `PromptSuggestion` stored only while idle and cleared on send and on turn start); `read_tail` (a file larger than the limit gives its end; a missing file; a file with invalid UTF-8).
- Entities: `relative_to_folder` (inside, nested, `..` escape, equal to the folder, a sibling folder with a shared prefix such as `/work/app` and `/work/app2/x`, trailing slash); `known_paths` and `render_markdown_linked` (full path, a name that is unique, a name that is ambiguous stays plain, text outside code stays plain, a span in a code block stays plain, raw HTML still escaped); `is_markdown`; `background_ref` (the real sentence, other text, a missing path); `valid_output_path` (good, `..`, another folder, another id, relative); `task_cards`.
- Visual run (real Forge, `/private/tmp/forge-observe`, ask the user before sending): write a markdown file and click its name under the turn and in the text; the Files panel opens on it rendered, `</>` switches; run `sleep 20` in the background and watch the pill, the panel (command, seconds, output, stop) and that Stop of the turn leaves the task running; after a turn the grey suggestion shows and Tab fills it. List what is left in an "As built" section.

## 9. Risks

- The SDK's behaviour for `perTaskStopAffordance` and `background_tasks_changed` is read from its type text; the visual run is the proof (a task must survive the stop button and die with the stop-task button).
- `FilesPort::read_tail` is the first read outside the session folder; it only gets paths that passed `valid_output_path` and has a size limit.
- The delegated click listener works through `document::eval`; if `dioxus.send` from a long-lived page script is not reliable, the fallback is an `onclick` on the answer text container that reads the clicked `<a>` through a small eval round trip.
- Suggestions depend on the account's settings (`promptSuggestionEnabled`) and limits; no suggestion is a normal state.
- The real app's rule for which file names in text become links was not observed in detail (only one example); Forge's rule (a path or a unique name from this session's own tool calls) is its own choice.

## 10. As built (2026-10-04)

Built as planned in ten tasks (plan `docs/superpowers/plans/2026-10-04-chat-view-panels.md`): the sidecar forwards `background_tasks`, `task_finished` and `prompt_suggestion` and takes `stop_task`, with `promptSuggestions` and `perTaskStopAffordance` on; the live view holds `background_tasks` (first-seen times) and `suggestion`; `FilesPort::read_tail`; pure rules in `forge-entities` (`file_open.rs`, `links.rs`, `background.rs`); `Workspace::open_file`; the Files panel serves requests and renders markdown with a `</>` switch; a clickable file card, activity row name and answer-text link; the `Background tasks` panel, its pill and a stop button per task; the suggested prompt and Tab.

**Checked by the user by hand in the real app** (the harness could not drive Forge this time; see Lessons): opening a file from the end-of-turn card, from an opened activity row and from the underlined name in an answer; the rendered markdown and the source switch; the notice for a file outside the session folder; the background pill, the panel with command, seconds and output, the turn's stop button leaving the task running and the card's stop button ending it; the suggestion and Tab; and the 23B items that were left (number keys, focus while typing, dark mode). The user answered that all of it worked.

**Found by the final review and fixed:** a task card made before the Bash result named its output file never read the output (the panel starts reading when the card is created and the card was keyed by id only); cards are now keyed by id and whether the output file is known (`card_key`, tested). This case (a second background command started while the Tasks panel is already open) is **not re-checked in the real app** after the fix.

**Left (deferred minors):** a duplicate match arm in `live_session_actor.rs` (an unreachable-pattern warning); a session folder recorded through a symlink (`/tmp/x` against `/private/tmp/x`) refuses every file click, because the check is lexical; the task card always says `Bash`; `task_cards` reads every tool result, not only Bash calls, and `read_tail` does not check for a regular file; a suggestion stays after a failure; `TaskFinished` is unused; file-link spans react only to a click. Tab fills the box but does not clear the suggestion (a ruling: it comes back if the box is emptied).

**Not built, as planned:** editing in the panel, the preview-tab behaviour, finished tasks listed, files outside the folder.

**Lessons**
- Writing a file with `open(p, 'w')` in the same expression that reads it empties it when the read comes second; a failed script left `theme.css` truncated and the next commit carried it. The theme tests caught it (nine failures); a commit step chained after a test run must follow reading the result.
- A plain regular expression over a plan's code fences breaks when the code contains fences of its own; parse fences by a line that is exactly three backticks.
- The harness: the `Open folder` system dialog takes Return only through `osascript key code 36`, a webview context menu can stay open and swallow keys, and a `forge-app` can survive `stop.sh` and `pkill` (check `pgrep` and the process's working directory before killing, because a Forge started by another worktree looks the same).
- Dioxus: `use_future` does not re-run when a plain prop changes; key the component by whatever it captured at start.
