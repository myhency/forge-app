# Card folder items: `폴더 추가…` and `폴더 변경…` (Plan 53 stage C)

Date: 2026-10-22. Builds on `2026-10-22-environment-card-design.md` (stage B). Observations: `docs/superpowers/roadmap/observations.md`, Rounds 23, 24 and 25 (desktop app 3.1623.1). Local only, as before: nothing about cloud, SSH or Remote Control.

## Intent

Make the folder row menu of the `환경` card match the desktop app: `폴더 변경…` and `폴더 추가…` between `경로 복사` and `터미널에서 열기`. The user said: do it the same as the desktop app, including that a changed folder keeps the environment name on the card.

## What the desktop app does (seen)

- Menu of the first row: `Finder에서 보기`, `경로 복사`, `폴더 변경…`, `폴더 추가…`, `터미널에서 열기`. An added row has only `Finder에서 보기`, `경로 복사`, `터미널에서 열기`. These exist only in a session (there is no card before the first message).
- `폴더 추가…`: a system folder dialog titled `세션에 폴더 추가`. No trust question. A new name-only row appears under the first row. The session (not the environment) gets the folder. Closing and reopening the session keeps the row. Removing is not possible.
- `폴더 변경…`: a dialog titled `작업 디렉터리 변경`. A folder that is not trusted first asks `이 워크스페이스를 신뢰하시겠습니까?` (`취소` / `작업 공간 신뢰`). Then a toast `세션이 <folder>(으)로 이동되었습니다`. The first row, the sidebar group and the header folder tag follow the new folder; added rows stay. The transcript moves to the new folder's project directory. The environment is NOT changed (its saved folders stay) and the card KEEPS the environment's name and gear, even though the session's first folder no longer is the environment's. When the environment is deleted, the card turns back to `이 컴퓨터` + `저장` while the new folders stay.

## Design

### 1. `폴더 추가…`

- A new item in the first row's menu opens `Workspace::pick_folder()`; on a pick the folder is registered in the session and added to the card's rows (`added_folders`). No trust question (as the desktop app).
- The SDK reads additional directories once, at start. The actor today ignores `add_directories` after the first send. New behavior: when the sidecar is already running and the session is idle, the actor remembers the folders, discards the sidecar and lets the next send start a new sidecar with `resume` and the extended list (the existing "restart on next send" path). While a turn is running the menu item is disabled.
- An already added folder or the first folder is ignored. The folders given by a stage B reopen and by `start_new` are unchanged.
- Added rows do not survive an app restart (they are in memory today, as stage B decided; the observed desktop app keeps them in the session file). Persisting them is left, see Left.

### 2. `폴더 변경…`

- Item in the first row's menu (not in added rows), disabled while a turn is running.
- Flow: pick a folder (same picker); if it is not trusted (`needs_trust`), show a small dialog with `이 워크스페이스를 신뢰하시겠습니까?` and `취소` / `작업 공간 신뢰` (trusting saves it in the trusted list); then move the session.
- Moving: (a) pin the environment shown now (see 3); (b) move the transcript file `<id>.jsonl` (and its same-named folder if any) from the old project directory to the new one with a rename (copy and delete when the rename crosses a device), never overwriting an existing file; (c) the actor changes the live `project_path`, discards the sidecar so the next send resumes in the new folder; (d) a toast `세션이 <name>(으)로 이동되었습니다`. If (b) fails the session stays where it is and a notice shows the error.
- A session that has never started a sidecar and has no transcript (no message yet) cannot occur here (the card exists only after a message). A past session that is open but not started yet moves the same way.
- No note is written into the transcript (the desktop app queues a hidden message about the new directory; Forge's model sees the new cwd at the next start).
- A worktree session: the first row shows the repository (stage B). `폴더 변경…` on it is out: the item is hidden for a session whose folder is a Forge worktree (a recorded worktree or a `.claude/worktrees` folder); this is a call to avoid moving into or out of a worktree (the desktop app was not observed here).
- A scratch (`폴더 없음`) session has no card rows, so nothing to do.

### 3. Keeping the environment's name after a change

Stage B computes a session's environment from its folder and stores nothing. A changed folder breaks that. New: Forge config gets `session_environments: Vec<SessionEnvironmentRef { project: PathBuf, session_id: String, environment_id: String }>`, written only by `폴더 변경…` (the environment shown at that moment is pinned; nothing is written when there is none). `session_environment(project, session)` returns the pinned environment when the ref exists and the environment still exists; otherwise the folder match of stage B. Deleting the environment therefore turns the card back, as seen. The pin is keyed by the Claude session id and the project path at the time of the change; moving again re-keys it. A later reopen in the new folder finds it; so does the start line. Reading is lenient (`#[serde(default)]`, bad entries dropped).

A reopened session that is pinned gets the environment's variables and extra folders as in stage B (the folder match is not needed any more).

### 4. Sidebar and header

Both derive from `project_path`, so they follow the change. The old transcript no longer lists under the old group after the move.

### Strings

New `tr!` strings with Korean entries in `crates/forge-shared/src/ko.rs`: `Change folder…` (`폴더 변경…`), `Add folder…` (`폴더 추가…`), `Do you trust this workspace?` (`이 워크스페이스를 신뢰하시겠습니까?`), `Trust workspace` (`작업 공간 신뢰`), `Session moved to {name}` (`세션이 {name}(으)로 이동되었습니다`). Reuse `Cancel`.

## Not in this stage (Left)

- The composer's `폴더` chip that the desktop app shows after `폴더 추가…` (it is an `@` mention; Forge's composer already has its own folder chips).
- Persisting added folders over an app restart; the hidden note about the new directory; `폴더 변경…` of a worktree session; whether the desktop app keeps the environment name after `폴더 변경…` to an already trusted folder and after an app restart (not seen); Windows (never run).

## Checks

- Unit tests: the actor restarts on the next send after `add_directories` on a running idle session and keeps folders de-duplicated; the actor's cwd change and sidecar discard; the transcript move helper (rename, existing target refused, missing source, moves the same-named folder); `session_environment` with a pin (pinned wins, deleted environment falls back, no pin falls back); lenient reading of `session_environments`; the menu item set by row and busy state (a pure helper).
- Real window (Forge harness): the menu items on the first and added rows; add a folder, send a probe and see the SDK use it; change the folder (trusted and untrusted), see the card keep the environment's name, the group, header and toast follow, send a probe message on the moved session in the real Claude (resume works), and delete the environment to see the card turn back. Use throwaway folders under `/private/tmp` and a fake Claude home for everything that does not need a real turn. One or two real turns are needed for the resume check (the user allowed several real messages for observations on 2026-10-22; ask again for the build check).

## As built

Plan 53 stage C, branch `plan-53c`, tasks 1 to 6, a fix wave after the window check, one after the real turns, and a final-review fix wave (`0c461a2`, `767d5b8`, `274c16e`).

**Menu.** The first folder row's menu is `Finder에서 보기`, `경로 복사`, `폴더 변경…`, `폴더 추가…`, `터미널에서 열기`. An added row has `Finder에서 보기` and `경로 복사` only. The items come from the pure helper `folder_menu` (by row and busy state). While a turn runs, while a background task runs, or while a move of the same session is in progress, the two folder items are hidden (not greyed). `폴더 변경…` is also hidden for a worktree session. A scratch session has no rows.

**`폴더 추가…`.** `Workspace::add_session_folder` opens `pick_folder`, ignores the first folder and folders already added, and adds the row (name only, no parent path). No trust question. After the pick it checks `session_is_busy` again; if the session became busy it adds no row and shows the info notice `턴이 끝난 뒤 폴더를 다시 추가해 주세요`. Added rows are in memory only. The actor (`add_directories`) keeps the new list and discards a started idle sidecar only when the list grew (or the sidecar was warm-only); the next send starts a new one with `resume` and the longer list. While a turn or a background task runs the call is refused and nothing is recorded. A folder the session already has keeps the sidecar.

**`폴더 변경…` flow.** `Workspace::change_session_folder` opens `pick_folder`; if the folder is the one the view already has (`should_move`, compared by component) it stops. If the folder is not trusted it sets `trust_request` and the dialog `WorkspaceTrustDialog` asks `이 워크스페이스를 신뢰하시겠습니까?` with `취소` / `작업 공간 신뢰` (Esc and a click on the backdrop cancel; trusting saves the folder in the trusted list; closing the session clears a pending request). Then `finish_change` runs: a running turn, a background task or a move already in progress gives the plain notice `턴이 끝난 뒤 폴더를 다시 변경해 주세요` and stops; a session with no Claude session id gives `첫 메시지를 보내기 전에는 세션을 옮길 수 없습니다.`; a session closed in the meantime gives `세션이 닫혀 폴더를 변경하지 않았습니다.` Otherwise, in this order: the session id goes into the `moving` set; `handle.suspend()` (`LiveCommand::Suspend`, a oneshot) stops the idle sidecar BEFORE the rename, so a live CLI cannot recreate a ghost transcript in the old folder (the actor answers false and does nothing while a turn or background task runs; then `moving` is cleared and the wait notice shows); `registry.move_to` moves the transcript; on an error the notice shows it and nothing else changes (no pin, no folder change; only the sidecar was stopped and the next send starts one again in the old folder); on success `pin_environment`, then `handle.move_to` (the actor `MoveTo` changes `project_path`; it is refused only by a busy sidecar, which after `Suspend` cannot exist, and does nothing when the folder is the same), `moving` is cleared, `forget_folder_state` runs, the sidebar reloads (`reload_project` for old and new folder, or `reload_past` when the new folder has no group yet), then the info notice `세션이 {name}(으)로 이동되었습니다`. `Suspend`, `MoveTo` and `AddDirectories` that would restart are all refused while a turn or a background task runs. While the id is in `moving`, `session_is_busy` is true (it also counts background tasks) and the composer cannot send (`can_send`). `change_session_folder` and `add_session_folder` use `spawn_forever`.

**Panels after a move.** The dock keys the `Files` and `Changes` panels by panel and folder (`"{panel:?}-{cwd}"`), so they are rebuilt for the new folder. `forget_folder_state(id)` clears the session's Files store (open tabs, view state and unsaved text, dropped without asking like on close), queued files, the file request, the comment focus, the composer cards (comments and excerpts) and the Changes state. The `Terminal` panel keeps its key: open shells stay in the OLD folder and a new tab starts in the new one.

**Log line.** Forge had no logging. The actor now prints to stderr whenever a started sidecar is dropped for an add, a move or a suspend: `forge: session <n> stops its sidecar (<reason>); the next send resumes it` (reasons: a folder was added, the session moved to another folder, the session's transcript is about to move).

**Transcript move (`move_session`).** In `crates/forge-adapter-claude-fs/src/sessions.rs`: validates the id, refuses an existing target file or folder (never overwrites), is a no-op when both folders map to the same project directory, renames `<id>.jsonl` and then its same-named folder (copy and delete when the rename crosses a device; the file is rolled back if the folder move fails), then appends one line `{"type":"relocated","relocatedCwd":"<new folder>"}`. If the append fails everything is moved back; when moving back fails too, the error reads `<move error>; moving it back failed too (<io error>); the transcript is at <target>`. Without this line the moved transcript still names the old folder in its first `cwd` lines, and after a restart the session was listed nowhere (window check defect D).

**Reading the folder (`read_cwd`).** In `transcript_cwd.rs`. If the first `cwd` already maps to the transcript's own directory it returns it without reading the tail. Otherwise it reads the tail of the file (16 KiB; while the window holds neither a `relocated` nor a `cwd` line and does not start at byte 0 it doubles, up to 1 MiB, `MAX_TAIL_BYTES`) and parses only lines holding `"relocatedCwd"` or `"cwd"`. The last `relocated` record in the tail decides the folder. Forge honors only records WITHOUT `sessionId`. The Claude CLI writes its own `{"type":"relocated","relocatedCwd":…,"sessionId":…}` record on a resume; its folder is the one the CLI was spawned in (the folder of the directory the transcript was resumed from), which goes stale after a second move (A to B to A: Forge's record says A, the CLI's later record says B, and the old rule listed the session nowhere). Records with a `sessionId` are ignored entirely. Without a Forge record the first `cwd` of the first 60 lines decides as before, except that when that first `cwd` is not of the transcript's own directory and the tail's last `cwd` is, the last one wins (a moved session whose record scrolled out of the window). `escape_project_path` became `pub(crate)`. Side effect: transcripts the desktop app relocated also list correctly. `load_session` skips the record as an unknown type (message count unchanged).

**Pin.** `AppConfig.session_environments: Vec<SessionEnvironmentRef { project, session_id, environment_id }>` (lenient, `#[serde(default)]`, bad entries dropped). `forge_domain::environment_of_session` and `Workspace::session_environment(project, Option<&SessionId>)` return the pinned environment when the ref exists and the environment still exists, else the folder match of stage B. `pins_after` replaces every ref of that session id (never two). The pin is stored under the folder's resolved key (`trust::resolved`, computed off the UI thread); `pins_seen_from` treats a pin spelled differently (for example `/tmp` against `/private/tmp`) that resolves to the same folder as that folder's pin. The card, the start line and a reopen (`open_past_in` passes the session id) all use it, so a pinned session reopened in its new folder gets the environment's variables and extra folders. `pin_environment` sets the signal at once, saves the config in a root task, and on a failed save restores the previous list and reports `Could not save the environment`.

**`report_info`.** `Workspace::report_info(message)` shows a notice drawn with the grey `banner info` class instead of the error box (`notice_is_info()` is true while the shown notice is that exact text). The move message and the wait message use it. It is still a sidebar banner with `닫기`, not a toast, and stays until dismissed.

**Deviations from the design.**
- `pins_after` has no `project_old` parameter (unused, because refs are replaced by session id).
- The folder items are hidden, not greyed, while a turn or background task runs (the design said disabled; the desktop app was not seen busy).
- The pin is written after the transcript move succeeds (design 2(a) listed it first); a failed move writes no pin. In the code it comes before `handle.move_to`, so the card does not render the new folder without a pin.
- The extra notices (no Claude session id, session closed, wait before adding) and the `moving` guard, the `Suspend` step and the panel reset (above) were not in the design.
- Added rows show the name only (window check defect A; Round 24 shows the same in the desktop app).
- No `터미널에서 열기` on added rows (ruling): the Terminal panel opens only in the session's own folder, and a per-row working folder is out of this stage.
- The move notice is a banner in the sidebar, not a toast; the dialog title of the system pickers is the default `Open folder`, not `세션에 폴더 추가` / `작업 디렉터리 변경`.
- Design 3 said the pin is "written only by `폴더 변경…`": still true. The `moving` guard and the busy notice were not in the design.

## Checked

Unit tests: forge-domain 282, forge-application 298, forge-features 610, forge-shared 35, forge-adapter-claude-fs 258 (all passed in the last fix report); each task had a RED run before the code.

**Real window** (harness, fake Claude home, throwaway folders under `/private/tmp`, fixture session, no Claude turn, commit `5f754f4`): the first-row menu with all five items; `폴더 추가…` added a row with no trust question and kept it after switching sessions and back; the trust dialog with `취소`, Esc (nothing changed) and `작업 공간 신뢰` (moved); after the move the card kept the environment name and gear (no flash in a capture right after), the first row, sidebar group, header tag and start line followed, added rows stayed, the transcript was in the new project directory, and config held the trusted folder and the pin; deleting the environment turned the card to `이 컴퓨터` + `저장` and kept the new folders; a trusted folder moved with no dialog and re-keyed the pin; the pinned name survived an app restart; the menu at 760 points wide showed no clipping. Defects found: A (added row showed the path), B (no terminal item on added rows, ruled as above), C (notice looked like an error), D (session vanished after a restart). A, C and D were fixed (`715406c`, `eda7ca3`); B stayed.

**Real turns, first run** (4 messages, commit `eda7ca3`): a session started in `forge-r-a` through the environment `r-env`; `폴더 변경…` to `forge-r-b`; the next turn resumed in the new folder (`pwd` gave `forge-r-b`, `b.txt` was read, and Claude remembered the first turn, so the resume carried the conversation and memory); after a restart the session listed under `forge-r-b` with the full conversation and the card showing `r-env`; `폴더 추가…` then a turn reading a file in the added folder answered without an outside-directory permission prompt, and the sidecar's node and claude process ids were the same before and after that turn, so no restart was seen; a move back to `forge-r-a` resumed. Defect E: after A to B to A and a turn, the session was missing from the sidebar after a restart, because Forge took the CLI's stale `relocated` record (it names `forge-r-b`).

**Real turns, second run** (4 messages, commit `49ae9cf`, after the fix for E): start in `forge-r-a`, move to `forge-r-b`, move back to `forge-r-a`, restart 1 (listed under `forge-r-a`, all three exchanges loaded, card showed `r-env`), move to `forge-r-b` again with a fourth turn, restart 2 (listed under `forge-r-b` only, Home counted one session). The transcript showed the pairs of records: Forge's without `sessionId` and the CLI's with it, the latter stale after the move back; the fix ignores the CLI's. Because the harness uses a fake home and the real Claude resumes from the real projects directory, transcripts were copied by hand between the two (real to fake after each turn, fake to the real new-folder directory after each move); the real directories this created were deleted afterwards and the real listing matched the one before the run.

**Real turns, third run** (5 messages, commit `274c16e`): one Claude home, Forge reading the real `~/.claude` with a throwaway Forge config and throwaway folders `forge-s-a`, `forge-s-b`, `forge-s-extra` under `/private/tmp`, environment `s-env`. (1) The start line showed `저장된 환경: s-env` and `pwd` gave `forge-s-a`. (2) A late `폴더 추가…` stopped the sidecar (stderr line seen, no sidecar process right after); the next turn read a file in `forge-s-extra` with no outside-directory permission prompt, remembered the first turn, and the node and claude process ids had changed. (3) `폴더 변경…` to `forge-s-b` (trust dialog shown) with a live sidecar: the old project directory held only `memory/` (made by the real Claude, no `.jsonl`: no ghost), also 10 seconds later; the new one held the transcript ending with Forge's `relocated` line; the card kept `s-env`; the next turn ran in `forge-s-b` and recalled the earlier output. (4) After an app restart the session listed under `forge-s-b` with the whole chat and `s-env`. (5) Move back to `forge-s-a` was not refused ("already exists" did not show), the old directory held only `memory/`; the CLI's stale `relocated` line (with `sessionId`) after Forge's line was seen and ignored, and after another restart the session listed under `forge-s-a` with `s-env`. (6) With a background `sleep 120` running, the folder row menu showed `Finder에서 보기`, `경로 복사` and `터미널에서 열기` only. The real project directories made were removed afterwards and the real listing matched the one before.

## Left

Not checked in a window or real turn:
- A worktree session (the item is hidden for it; no git fixture was made).
- The grey info banner on a second notice, and the composer being disabled during the rename (the hidden items with a background task were seen in the third run; with a running turn they were not).
- The Files and Changes panels after a move (rebuilt for the new folder, state cleared): not opened in a window or the third run, only reviewed in code. Same for the wait-to-add and session-closed notices and a `/tmp` against `/private/tmp` pin spelling.
- Dark mode.
- Windows (never run).
- A pinned session after a restart whose environment has a worktree.
- The system picker titles (`세션에 폴더 추가`, `작업 디렉터리 변경`).

Not built:
- `터미널에서 열기` on added rows (needs a per-row working folder; ruled out).
- Persisting added folders over an app restart (the desktop app keeps them in the session file). Seen in the third run: the added `forge-s-extra` row was gone after a restart (known, in memory only).
- Terminal tabs after `폴더 변경…` keep the old folder (ruling).
- Unsaved Files text of the session is dropped on a move without asking (ruling, same as close).
- The hidden note about the new directory in the transcript.
- The composer's `폴더` chip after `폴더 추가…`.
- Whether the desktop app keeps the environment name after a restart or after a move to an already trusted folder (not seen).

Not measured: the cost of the extra tail read (16 KiB, up to 1 MiB for a huge last line) of every transcript on each listing; a transcript with no `cwd` line re-reads up to about 2 MiB per listing.

Deferred minors from the ledger that remain (fix wave 2 minors and older ones):
- A stale Files store may be re-created in the window before `MoveTo` publishes.
- The pin is canonicalized by a synchronous call on render for differently spelled pins.
- A failed folder put-back in the append rollback is silent.
- `suspend` answers false for a closed session, which shows the "wait" notice (the closed-session notice only covers a session gone at a later step).
- A stderr line shows on every new session with environment folders (the warm-up drop).
- The duplicate-folder check compares paths exactly (no canonicalize); a handle gone after the pick drops silently.
- Overlapping pin saves restore the `previous` list.
- The pin stays in `config.json` after its environment is deleted (ignored while the environment is gone, and it applies again if the same id returns).
- On a second move of an already pinned session the old pin is removed before the view moves, so the card can show the old folder's own folder match for a moment.
- Task 2: the rollback and cross-device branches and the refuse-on-existing-folder branch are untested (a failing move-back cannot be made in a test; only its message is); the check-then-rename race is accepted.
