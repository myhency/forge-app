# Plan 31: Routines

Date: 2026-10-09. Branch `plan-31`. Evidence: observation round 14 (`docs/superpowers/roadmap/observations.md`, PG-01), the desktop app's files read on 2026-10-09 (`~/.claude/scheduled-tasks/<id>/SKILL.md`, the app's own `scheduled-tasks.json`, a session json with `scheduledTaskId`), `SidecarPort` and the `session_lifecycle` use cases, `SleepPort` (a port with a macOS adapter, the model for new ports), spec §4 (definitions shared, activation private).

## Goal

A person can save a prompt with a folder and a schedule as a **routine**, and Forge starts a normal session from it whenever the time comes, while Forge is open. They can also run it at once, pause it, edit it, delete it, and see what each run did.

Success: a routine set to run every hour starts a session in its folder at the due time, the session appears in the sidebar and in the routine's history, and a run that was due while Forge was closed or the computer asleep is shown as "Skipped" and not run late. A routine that needs a permission the person has not allowed waits in its session and is marked, and nothing is approved by itself.

Decided with the user (2026-10-09):
- Local routines only. No cloud routines, no cloud menu item.
- A routine waits for approval when a tool needs a permission; Forge marks it and tells the person. It does not deny and it does not allow.
- **Everything is Forge's own**: the routine's name, description and instructions are *not* written to `~/.claude/scheduled-tasks/`, and the desktop app's `scheduled-tasks.json` is not read or written. Routines made in the desktop app are not shown. (They are not Claude Code files Forge may change, and two apps running one routine would run it twice.)
- Approach A: a timer inside Forge (no launchd, no Windows task scheduler). Forge runs routines only while it is open, as the desktop app says of its own.

## Design

### 1. Domain (`forge-domain`, pure)

`routine.rs`:

- `Routine { id, name, description, instructions, folder, permission_mode, model: Option<String>, worktree: bool, schedule: Schedule, enabled: bool, created_at, baseline: i64, always_allowed: Vec<String> }`. `id` is a slug of the name made once and never changed; a clash gets `-2`, `-3`. `baseline` is the instant from which the next due time is counted: set when the routine is created, when it is switched on, and after each run or recorded skip. Times are epoch milliseconds.
- `Schedule`: `Manual`, `Hourly`, `Daily { hour, minute }`, `Weekdays { hour, minute }`, `Weekly { day, hour, minute }`, `Cron(String)`. These are the six tabs of the form. `Hourly` runs at minute 0 plus the delay below.
- `next_after(schedule, after_ms, offset_at) -> Option<i64>`: the first due instant strictly after `after_ms`; `Manual` gives `None`. `offset_at` is the local UTC offset in effect at an instant (given by the caller, so daylight saving is right and the function stays testable). `Cron` takes the five-field form; a bad expression gives `None` and `validate_cron` says why for the form.
- `delay_ms(id) -> i64`: a fixed offset of 0 to 5 minutes taken from a hash of the routine id (the app says "a few minutes of random delay"; a fixed one is the same for the person and testable). Not added for `Manual` or "Run now".
- `RoutineRun { routine_id, due_at, started_at, outcome, session_id: Option<String> }`; `RunOutcome`: `Started`, `Skipped(SkipReason)`. `SkipReason`: `Missed` (Forge was closed or the computer asleep: the instant is more than 30 minutes past), `Busy` (another routine session is still running in that folder), `FolderMissing`, `StartFailed(String)`. History keeps the latest 50 runs per routine.
- A run's later state ("running", "waiting for approval", "done") is **not stored**: it is read from the live session when the history is drawn.

New dependency: the `cron` crate, pinned to an exact version (parsing five fields by hand is the wrong place to be clever). Its six-field form is fed `0 <expr>`.

### 2. Storage

Both lists live in Forge's config (`AppConfig.routines`, `AppConfig.routine_runs`, `#[serde(default)]`, so an old config loads). Every write goes through the existing config lock use case, because the runner, the pages and the settings all write the same file. Instructions are stored inline: they are Forge's own, and a routine is read as one unit.

### 3. Runner (`forge-application`)

`RoutineRunner` with a tick every 30 seconds, started with the app, stopped with it. Dependencies are ports, so it is tested without a clock or a sidecar: a `ClockPort` (now, local offset at an instant), the config port and lock, the session start use case, `SessionRegistry` (to ask which sessions are live and what they wait for), `NotifyPort` (§5).

Each tick, for every enabled routine that is not `Manual`:

1. `due = next_after(schedule, baseline) + delay`. Not yet due: nothing.
2. Due and no more than 30 minutes late: run (below).
3. Due and more than 30 minutes late: record **one** `Skipped(Missed)` for the latest missed instant (not one per missed occurrence), set `baseline = now`, do not run.
4. Run: if the folder is missing, `Skipped(FolderMissing)`. If a session started by a routine is still live in that folder (any state but finished), `Skipped(Busy)`. Otherwise start a session in the folder with the routine's permission mode, model and worktree flag; the prompt is the instructions (the description when the instructions are empty); the title is `<date> – <name>`. On success record `Started` with the session id; on an error `Skipped(StartFailed)`. Set `baseline = due`.

"Run now" does the same fourth step at once, with no delay and no lateness rule, and does not move `baseline`. It still refuses when the folder is busy or missing and says why in a toast. Starting Forge runs a catch-up tick at once, so the 30-minute rule covers the time Forge was closed.

Switching a routine on sets `baseline = now`, so nothing that was due while it was paused runs.

### 4. Permissions without a person

- The permission mode chosen in the form is the session's mode. The default is the app's default ("ask").
- A permission request in a routine's session is the same card as in any session; the session simply waits. Forge shows the wait in three places: the sidebar row gets the marker used for "needs attention", the run's row in the history says "Waiting for approval", and the routine's card in the list says it too.
- "Always allowed" (the detail screen's list): when the person answers a card from a routine's session with the "always allow" choice, the tool or pattern is added to `always_allowed` and passed as allowed tools when the next run starts. The list is read-only text in this plan, with a remove button on each entry.

### 5. Notification

`NotifyPort::notify(title, body)`. The macOS adapter calls `osascript -e 'display notification …'` (arguments passed as separate process arguments and escaped; title and body come from a routine's name and are never put in a shell line). Windows has no adapter in this plan: the port answers "not available" and the sidebar marker is the only sign there. A notification is sent when a routine's session first waits for approval, and when a start fails. Not for ordinary completion. This is the last task and can be cut without changing the rest.

### 6. Pages (`forge-features`, `forge-pages`, `forge-widgets`)

Follows the Customize page's pattern: "Routines" is a sidebar item that shows a page in place of the chat view; the page has three screens.

- **List.** Title "Routines"; search field (name and description); a sort menu (Name, Last run, Created); a status filter (All, On, Paused); a button "New routine". The info notice "Routines run only while Forge is open and the computer is awake." Cards: name, description, state and schedule ("Paused · Every day at 9:49 AM"; "On" when enabled; "Manual"), and a "Waiting for approval" tag when a run waits. An empty list shows a short text and the button. Not copied: the warning icon on the observed card (its meaning was not seen); the `템플릿` tab and the cloud menu.
- **Detail.** Breadcrumb "Routines / <name>"; edit, delete (a confirm dialog), and "Run now". Left: description, a switch "On / Paused", folder, schedule text, "Always allowed" (with remove buttons). Right: instructions (three lines, "Show all"), and "History": rows with the due time, the outcome ("Started" with the session's state, or "Skipped" with the reason), a click on a started row opens its session.
- **Form** (new and edit; the same fields as the app): name\*, description\*, instructions, permission mode and model pickers (the composer's own), folder (the folder menu of the new-session screen) and a "Worktree" checkbox (greyed until a folder is chosen), schedule tabs and the time field or the cron field, a line about the delay. Buttons "Cancel" and "Create" (or "Save"), the second off until name and description are filled and the cron expression is valid. Leaving a changed form asks "Discard unsaved changes?" with "Keep editing" and "Discard".

Deleting a routine keeps the sessions it made.

### 7. Where a routine's sessions show

They are ordinary sessions of the folder's project, listed in the sidebar like any other, titled `<date> – <name>`. Which sessions were made by a routine is Forge's own record (`routine_runs.session_id`); the transcript is not changed.

## Out of scope

- Cloud routines; the Templates tab (no catalogue and not seen); routines made in the desktop app; the warning icon; a notification on Windows; running while Forge is closed; a per-routine "always allowed" editor beyond remove.
- Cron fields beyond five (no seconds, no `@daily` aliases).

## Open facts to check in the plan (not guessed here)

1. How a session started without a window waits on a permission card in the current `LiveSession`, and whether the sidebar marker already has a "needs attention" state to reuse.
2. How the SDK reports an "always allow" answer (a suggestion with a rule) and how an allowed-tools list is passed to a new query.
3. Whether `SessionRegistry` can answer "which live sessions started by a routine are in this folder" without a new field.
4. The `cron` crate's exact current version and its handling of day-of-week numbers (Sunday as 0 and 7).

## Testing

- `next_after` for every schedule, across a daylight-saving change, with a negative and a half-hour offset; `delay_ms` fixed and in range; `validate_cron`.
- The runner with a fake clock and mock ports: due, late by less and by more than 30 minutes, one skip for many missed instants, busy folder, missing folder, start error, switching on resets the baseline, run now, a disabled and a manual routine, a config write failure.
- The list's search, sort, filter and the card text; form validation; the dirty confirm.
- In the real window with the harness fixture: create, edit, pause, run now, delete; a one-minute routine started with the real SDK; a permission wait and its marker; the narrow layout.

## Task order

1. Domain types, `next_after`, `delay_ms`, `cron` crate, tests.
2. Config fields and the lock; `RoutineRunner` and `ClockPort`, tests with mocks; wire to the app.
3. List, detail and form pages with the sidebar item.
4. Permission wait marking and "Always allowed".
5. `NotifyPort` and the macOS adapter (cuttable).
6. Real-window checks, review fixes, docs: backlog row PG-01, README order, an "As built" section here.

## As built (2026-10-09)

Plan `docs/superpowers/plans/2026-10-09-routines.md`, branch `plan-31`.

### Differs from the spec above

1. **One engine.** Every schedule tab becomes a cron expression and runs on the `cron` crate over `chrono` time zones (no `offset_at` argument). Weekdays are handed to the crate by name, because it numbers them from another start than the usual `0` or `7` for Sunday.
2. **No Worktree checkbox** (Forge sessions have no worktree support). No `worktree` field is stored.
3. **A folder is required** to create a routine, not only a name and a description.
4. **Plain selects** for permission mode and model, and the folder picker button, in place of the composer's pickers and the folder menu.
5. **Finished routine sessions are closed by Forge** (idle for over 60 s, nothing waiting, not on screen), so an hourly routine does not leave a process per run. The transcript stays and the session is a past session in the sidebar.
6. **"Always allowed" needed a sidecar change.** The sidecar reports the rules an "always allow" would add (`permission_request.rules`, from the SDK's `addRules` suggestions) and takes `allowed_tools` at start. The session view keeps `allowed_rules`; the registry host copies them to the routine.
7. A routine card for a manual routine says "Manual" once (found in the real window).
8. **No notification when a start fails** (spec §5 asked for one): the failure is a `Skipped: could not start (...)` line in the history. Only a session that waits for approval notifies.
9. **Fixed after the whole-branch review:** the timer no longer closes a routine session the person has opened (`keep_open` on select); a session the person closed no longer keeps the "waiting" marker or its folder busy, and a failed session does not hold its folder; a config save that fails after a start no longer starts the routine again on every tick, and one routine's failed save no longer stops the others (the runner remembers the due time it handled).

### Checked in the real window (harness fixture, real SDK, throwaway folder)

- Sidebar link, empty list, the form (Create off until name, description and folder; the reason shows once something is typed; the discard dialog after a change), the list with cards, the detail screen, the edit form filled from the routine, the delete dialog and a delete that removes the history too, and the 640 pt layout (columns stack).
- **Run now** starts a session titled `Oct 9 – <name>` in the folder; the history row reads "Done"; the session closes by itself after about a minute when it is not on screen. A second run while the folder is busy is refused with "A session of a routine is still working in this folder."
- **Waiting for approval**: a routine in "Ask before edits" mode that needs Bash stops at the permission card; the sidebar row has the needs-attention marker, the detail screen shows a banner, the history row says "Waiting for approval". Nothing is approved by itself.
- **Always allow** on a Bash request puts `Bash(touch /private/tmp/forge-routines/x2.txt)` under "Always allowed", and the next run of the routine does not ask.
- **Timer**: a once-a-minute routine started a session at the minute (the Claude session id was written to its history line); a daily routine whose time passed hours ago was not run and left one `Skipped: Forge was closed or the computer was asleep` line.
- The sessions the checks made were removed from `~/.claude/projects/-private-tmp-forge-routines/` by hand.

### Not checked

- Windows (no notification there; the timer and pages are the same code).
- The macOS notification itself was sent through `osascript` but not seen on screen.
- A click on a history row opens the session when it is in the sidebar's list; the harness reads a fake Claude home, so the transcripts of the checks were not listed there. The "not in the sidebar any more" message was seen instead.
- The packaged app; the form's folder picker (a system dialog, so routines were seeded in the fixture config and the form was checked up to the folder).

### Known limits and deferred minors

- **"Always allow" on a file write** (Write, Edit) leaves "Always allowed" empty: the SDK suggests a mode change for those, not a rule, so the next run asks again.
- Opening the Routines page again while it is shown does not re-read the list; the 30 second timer does.
- The card shows no warning icon (its meaning was not seen); no Templates tab; no cloud routines.
- Deferred after the review (all minor): cron edge cases (weekday ranges ending in 7, a numeric start with a step, day-of-month and weekday combined with AND, Feb 31 accepted, raw crate error text in the form); the "Missed" line of a very frequent schedule carries an old due time and the schedule is parsed again on every step; a time inside the spring-forward gap does not run and leaves no line, history labels use the offset read at startup, and no daylight-saving or half-hour-offset tests; the tick's snapshot is read outside the config lock, so an edit made during the tick can lose its baseline; ids of deleted routines are reused; a just-started run reads "Done" until its session id arrives and an asynchronous sidecar failure is recorded as started; the macOS notification runs on the UI thread; history rows are keyed by due time only; every check clones whole session views.
- `forge-adapter-claude-plugins` has two timing tests (`a_command_that_times_out_takes_the_processes_it_started_with_it`, `a_process_left_behind_holding_the_pipe_does_not_hang_the_command`) that fail now and then when the whole workspace runs at once and pass alone. Not touched by this plan.
