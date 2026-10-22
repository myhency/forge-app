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
