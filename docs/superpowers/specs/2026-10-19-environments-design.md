# Plan 53: Local environments (OTH-11, stage A)

Date: 2026-10-19. Branch `plan-53`. Evidence: observation round 21 in `docs/superpowers/roadmap/observations.md` (parts 1 to 3), the desktop app's own files read on 2026-10-19 (`ccd-session-presets.json`, one session file), and the docs page `code.claude.com/docs/en/desktop` (environment configuration).

## Goal

A person can save a set of folders (and `.env` variables) as a named environment, pick it on the new-session screen from an environment chip, and start a session that works in the first folder, may use the others, and runs with the variables, as the desktop app does. Local only.

Success:
- A chip left of the folder chip on the new-session screen shows the environment (`이 컴퓨터` by default). Its menu has a search field, `이 컴퓨터` (⌥⌘1), saved environments (⌥⌘2…), `더 보기 N ›` and `+ 새 환경` (⌥⌘0).
- `새 환경` and `환경 편집` dialogs make, change and delete an environment: name, folders (the first with its branch and worktree choice), `.env` variables.
- Picking an environment hides the folder, branch and add-folder chips. The first message starts a session with `cwd` = the first folder, the other folders as added directories, and the variables in the session's environment.

Decided with the user (2026-10-19):
- Local only. The cloud, SSH and Remote Control tabs, the `클라우드` and `Remote Control` submenus, the Remote Control switch and the Remote Control section of Settings are NOT built, not even as disabled rows. The sidebar filter's `환경` submenu is not built (nothing to filter by with one kind).
- Two stages: this spec is stage A. Stage B (separate spec) is the side panel's `환경` card and the `시작 위치 … 환경으로 저장` line; the card only, no `변경 사항` card.

## Facts seen (not guessed)

- Menu: search field `환경 검색…`; group `로컬` with `이 컴퓨터` (⌥⌘1), saved environments (⌥⌘2…), `더 보기 N ›`; then `클라우드 ›`, `Remote Control ›`; then `+ 새 환경` (⌥⌘0). A saved row shows a gear (edit) and a star (pin) on hover and a check when current. `이 컴퓨터` shows only a star. With one saved environment the visible list was `이 컴퓨터`, one saved row, `더 보기 1`; after a third was made, the newest took ⌥⌘2 and the older two went under `더 보기 2` (most recently used first).
- Dialog `새 환경`: `이름` (prefilled with the current folder's name), `폴더 (선택 사항)` with `+ 추가` (a popover: search field, known folders, `선택…` opens the system dialog), one row per folder (name; a git folder shows `master ⌄` and a checked `워크트리` box, a non-git folder shows `Git 저장소가 아님`), `×` on hover; no folder gives the dashed text `세션을 시작할 폴더를 선택하세요.`; a card `환경 변수` / `이 환경에는 없음` opens a page with a mono text area (`.env` format, placeholder `NODE_ENV=production`, `GIT_AUTHOR_NAME=Your Name`, `# Multiline values - wrap in quotes`, `CONFIG="key1=val1` / `key2=val2"`), the note "`.env 형식`으로 제공됩니다. 이 값들은 안전하게 저장되어 Claude 세션에 전달됩니다.", `완료`. Buttons `취소`, `환경 만들기`. Closing a changed dialog asks `이 환경을 만드시겠습니까?` (`삭제`, `계속 편집`, `환경 만들기`). `환경 만들기` asks to trust each folder (`이 워크스페이스를 신뢰하시겠습니까?`).
- `환경 편집`: the same fields, `환경 삭제` (red text) at the left, `취소`, `저장`. Deleting asks `환경을 삭제하시겠습니까?` with `취소`, `환경 삭제`, and (the desktop app only) `세션 N개도 삭제`.
- Using it: chip shows the name, the folder chip is gone, the mode stays as it was. The session file has `cwd` = `originCwd` = the first folder, `requestedAdditionalFolders` and a session-scoped `addDirectories` for the others. The saved record is `{id, name, target: {kind: "local", cwd, additionalFolders}, settings: {modelId, effort, worktree}, createdAt, lastUsedAt}`.
- Docs: variables saved in the local environment editor are "stored encrypted on your machine" and apply to every local session started from it; they reach Claude sessions and dev servers.

## Design

### 1. Domain (`forge-domain`, `forge-entities`)

- `Environment { id: String, name: String, folders: Vec<PathBuf>, branch: Option<String>, worktree: bool, env_vars: String, pinned: bool, created_at: i64, last_used_at: i64 }`. `folders[0]` is the working folder; `branch` and `worktree` belong to it (Forge's worktree flow already takes one folder and one base branch). The other folders are plain rows: the desktop app's per-folder box on later folders was not seen to do anything, so Forge shows none rather than a box that does nothing.
- `AppConfig.environments: Vec<Environment>`, `#[serde(default)]`, so an old config reads. The environment id is a random UUID-like string made by the caller (pure code gets the seed, as `slug` does).
- `parse_env(text) -> Result<Vec<(String, String)>, EnvParseError>` in `forge-entities` (pure): `KEY=value` lines; `#` starts a comment line; blank lines skipped; a value may be wrapped in `"` or `'` and then runs over lines to the closing quote; `export ` prefix allowed; a key is `[A-Za-z_][A-Za-z0-9_]*`; a line that is not a pair or an unclosed quote gives an error with its line number. Later duplicates win.
- `menu_rows(environments, current, query) -> EnvironmentMenu` (pure, in `forge-features`): the order and the split between the visible rows and `더 보기`: `이 컴퓨터` first, then pinned environments, then the most recent not pinned one, the rest under `더 보기`, sorted by `last_used_at` desc; at most 8 visible saved rows (⌥⌘2…⌥⌘9). A query filters every environment by name (case folded) and lists matches flat. Numbers are assigned from the visible order. (The cap of one unpinned visible row is read from two observations; the plan re-checks it with a pin.)

### 2. Storage

- `SettingsUseCase` gets `save_environments` next to `save_worktrees`: same lock, read-modify-write, refuses to replace an unreadable config.
- Variables are kept as plain text in Forge's `config.json`, like `api_key`. The desktop app says "stored encrypted"; Forge has no keychain adapter, so the dialog's note says where they are kept ("Saved in Forge's settings file on this computer."), not "securely". A work-PC reviewer should not be told something false.
- No seeding from past folders (the desktop app seeds its first presets from old sessions).

### 3. New-session screen

- `NewSessionDraft` gets `environment: Signal<Option<String>>` (`None` = `이 컴퓨터`).
- `EnvironmentChip` (`forge-widgets/src/new_session_controls.rs`), placed first in `NewSessionPanel`'s chips. It uses `Picker` with a search field, rows with a check, hover icons (gear, star) and shortcut text. `Picker` gets only what it lacks: a search field and per-row trailing icons; the plan reads `composer_controls.rs` and extends it minimally. `더 보기 ›` opens a second list.
- With an environment picked: `FolderChip`, `BranchChip` and `AddFolderButton` are not rendered. Draft `worktree` and `branch` are set from the environment when it is picked (the folder probe runs on `folders[0]`). The mode, model and effort are left as they are.
- Keys: ⌥⌘1…⌥⌘9 pick the numbered row, ⌥⌘0 opens `새 환경`; read from the physical key (`Digit1`/`Numpad1`, Korean input method safe); active on the new-session screen only. The plan checks the keys against the existing shortcut list and `shortcuts_dialog.rs` for conflicts and adds the rows to the shortcut list.

### 4. Dialogs

- `EnvironmentDialog` (new `forge-widgets/src/environment_dialog.rs`), one component for create and edit. Fields as in the observation; the folder popover reuses the recent-folder list and `open_folder`'s system dialog (`forge-adapter-dialog`). A git first folder shows its branch picker and a `Worktree` checkbox (default on, as in the folder menu); the branch list comes from the existing `WorktreePort` probe. `Create environment` is disabled while the name is empty. An environment may be saved with no folder (as in the desktop app); picking it then shows the dashed hint, and sending is blocked with the message `Add a folder to this environment first`.
- The variables page is a second view inside the dialog (back arrow, `×`, `Done`); invalid text shows the parse error under the text area and `Done` stays disabled.
- Closing a changed dialog (× or Escape) asks `Create this environment?` / `Save the changes?` with `Discard`, `Keep editing`, and the save button.
- Delete asks `Delete environment?` with `Cancel` and `Delete environment` (Forge has no per-session deletion, so no third button).
- Trust: before saving, each folder that is not trusted is asked in turn with the existing trust dialog (Plan 43); cancelling stops the save.
- All strings go through `tr!` with Korean entries in `forge-shared/src/ko.rs` (`scripts/i18n_add.py`).

### 5. Starting a session

- `start_new` gets the environment: `project` = `folders[0]` (a missing folder reports `Folder <path> does not exist` and nothing starts), `additional_directories` = the other folders merged with the attachment folders already passed, `env` = `parse_env(env_vars)`, worktree request from the environment's `worktree` and `branch` (the existing `worktree_request` rule: only when the folder is the probed repository).
- `StartSessionParams` gets `env: Vec<(String, String)>`; the sidecar protocol gets an optional `env` object on the start command; `sidecar/src/session.ts` passes `env: { ...process.env, ...command.env }` to the SDK (the plan reads the SDK to confirm that `env` replaces rather than merges, hence the explicit spread). Variables never go into logs or `Debug` output.
- On send, `last_used_at` of the environment is set (this is what orders the menu).
- A session does not record which environment it came from in this stage; stage B adds that with the `환경` card.

## Out of scope

Cloud, SSH, Slack, Remote Control (all of it, see Goal); the sidebar `환경` filter; the side panel card and the `환경으로 저장` line (stage B); seeding from past sessions; model and effort in an environment (the desktop app saves them but was not seen to apply them on pick; not built until seen); a per-folder worktree box on later folders; encrypted storage.

## Open facts to check in the plan

- Whether the SDK's `env` option merges or replaces (read the installed SDK; test with a real session that prints a variable).
- That one unpinned row is visible; what the star does (pin and its effect on the order); whether `이 컴퓨터`'s star does anything.
- Whether picking a saved environment changes the permission mode in the desktop app (it showed `수동` once, then `자동` again).
- Where the desktop app keeps the variables (none were set in the observation); not needed for Forge but noted if seen.
- Conflicts of ⌥⌘0…9 with existing Forge shortcuts.

## Testing

- Unit: `parse_env` (quotes, multi-line, comments, `export`, errors with line numbers, duplicates); `menu_rows` (order, split, numbers, query, cap); config round trip with and without `environments`; `save_environments` keeps other fields; the start parameters (first folder, additional directories merged without duplicates, env pairs, worktree rule); sidecar test that `env` reaches the SDK options and that no variable is printed.
- Widget/feature tests where the repo already has them for pickers and dialogs.
- Real window (the Forge harness): make an environment with two folders and a variable, restart, pick it with the mouse and with ⌥⌘2, send "print $NAME", see the variable and the second folder reachable; edit, pin, delete; a Korean input method check is left to the user.

## Task order

1. Domain: `Environment`, `parse_env`, `menu_rows`, config field, `save_environments` (tests first).
2. Start path: `StartSessionParams.env`, sidecar `env`, `start_new` with an environment.
3. `EnvironmentChip`, `Picker` extension, keys.
4. `EnvironmentDialog` (create, edit, variables page, close prompt, delete, trust).
5. Korean text, shortcut list, backlog and observation notes, real-window check.

## As built (2026-10-19)

### What differs from the spec
- A dedicated `EnvironmentChip` (forge-widgets) instead of extending `Picker`.
- Trust is asked when a folder is added to the dialog (the card shows in the dialog and `Trust and add` adds the folder), not at save.
- `EnvVars` and `parse_env` live in `forge-domain`, not `forge-entities`.
- `Environment`'s `Debug` redacts `env_vars`, so values never reach logs.
- An environment whose first folder is gone is refused at send: nothing starts, a notice says "The folder of this environment no longer exists. Edit the environment.", and the typed text is kept. Two fixes found in review in `new_session.rs`: the missing-folder check (`folder_missing`) replaced a plain `is_dir` flag on the project, and the send returns early so the draft text is not cleared.
- The branch `<select>` sets `selected` on the matching `<option>`, because the web view ignores `value` on a `<select>` (it showed the first branch).
- Also from the dialog work: the folder popover opens below its button (`.picker-menu` opens upward and was clipped), the gear closes the chip menu, a stale edit (environment deleted) closes the dialog, and the name is selected on open.

- Picking an environment does not write its worktree switch or branch into the new-session draft. While one is picked the draft values are unused (sending uses the environment's own plan and the folder and branch chips are hidden), and writing them would leave the person's own Worktree switch changed after going back to `This computer`.
- Picking an environment with no folder shows no dashed hint on the new-session screen (only the dialog has one); sending then shows the notice "Add a folder to this environment first". This is a recorded gap against the spec.
- The config reads `environments` entry by entry: an entry that does not parse is skipped and the others stay, so a hand edit cannot empty the list (the next save would have written the empty list back). A value that is not a list reads as empty.
- While the environment dialog is open it counts as an overlay for the shortcut keys (the `on_key` overlay flag), and ⌥⌘0…9 do nothing, so unsaved edits stay and nothing acts on the hidden screen.
- On the Variables page the back arrow and Escape keep the typed text when it parses (a bad line keeps the page); the close button and backdrop ask the Discard question when the text differs.

### Checked in the real window (harness, tasks 4 and 5)
- Chip left of the folder chip; menu with search, group `로컬`, `이 컴퓨터 ✓ ⌥⌘1`, `새 환경 ⌥⌘0`; no cloud or Remote row; Escape closes it.
- Dialog: name prefilled and selected; `+ Add` popover and `Choose…` with a system dialog; trust card (`Trust and add` adds, Cancel does not); a git repo shows the branch select (current branch correct after the fix) and Worktree, saved and reloaded through the gear; Variables page with the note, a bad line shows `Line 2: expected NAME=value` and disables Done; Create adds the row with `⌥⌘2`; hover shows gear and star; close prompt (Discard, Keep editing, Save) on Escape, backdrop and the close button, also from the Variables page; delete asks and removes, the chip falls back to `이 컴퓨터` and the other chips return.
- Light mode only.

### What is left
- The real-session check (Step 3 of Task 6): an environment with a variable and a second folder, one real message; verify the variable reaches the command, the second folder is readable without a folder question, and the transcript path is under the first folder. Not done yet because it costs a real turn on the user's account.
- Dark mode of the dialog and the chip menu.
- Not seen: the star (pin) click, the `Not a Git repository` flash before a repo is read.
- Deferred minors: `1 variables` has no singular; the folder popover is cut off at the bottom when the folder list is empty; two folders with the same name look alike in the popover (only the hover title tells them apart); the Variables page error banner is indented; the folder popover search has no placeholder; Escape does not dismiss the close question; the name is selected again when returning from the Variables page.
- A session reopened from the sidebar or with ⇧⌘T runs without the environment's variables and extra folders: nothing records the environment on the session until stage B.
- Stage B: the `환경` side-panel card and the `시작 위치 … 환경으로 저장` line; model and effort in an environment; seeding from past sessions; encrypted storage.

## As built: A2 (2026-10-20)

Every git folder of an environment now has its own branch and worktree choice (option A, decided with the user on 2026-10-20). The desktop app shows an unchecked box on extra git folders but stores only the first folder's flag; Forge stores and uses all of them.

- Model: `FolderChoice { branch, worktree }`; `Environment.extra_choices` holds one choice per extra folder, aligned with `extra` by position. `normalized()` pads with default choices or trims so the two lists always match; adding or removing a folder moves its choice with it. A stage A config without `extra_choices` loads with default choices (no branch, box off). The dialog draft holds `choices` (index 0 is the first folder).
- Start: after the first folder's worktree, `start_new` makes a worktree for each extra folder whose box is on. The worktree path replaces the folder as the added directory. If one fails, nothing starts, the text comes back to the composer, and the notice is `Could not create the worktree: {error}`. The worktrees already made for that send (the first folder's and the earlier extras') are released again, as on archive, and dropped from the records; the same happens when the saved model cannot be read or the session cannot be opened. If one cannot be removed, the notice adds `Some worktrees made for this message could not be removed: {paths}`. On a failed send only the text comes back; attachments and composer folder chips are lost (this predates A2).
- An environment extra folder that became a worktree replaces the same folder when it is also a composer chip, so the folder and its worktree are not both added.
- A malformed `extra_choices` (null, a non-array, a bad entry) reads as no choices or the default choice, so the environment keeps its folders and variables.
- Dialog: one `FolderRow` per folder, keyed by path, with a branch select and the Worktree box for every git folder, and `Not a Git repository` for others. Removing a folder keeps the other rows' choices.
- Layout fixes: the folder popover scrolls its known-folder list inside the dialog (search and `Choose...` stay fixed); a long environment name is ellipsized in the chip menu next to the hover gear and star.
- One new user-visible string (the left-worktrees notice), with its Korean entry.

### Checked in the real window
- Long name (70 characters) ellipsized on hover, gear and star do not overlap the shortcut.
- Edit dialog with two git folders and one plain folder (`t3-a`, `t3-b`, `t3-plain`): each git folder shows branch select and Worktree box, the plain one shows the note. The remove checks used three git folders. Ticking an extra folder's box saves to `extra_choices` and reloads. A seeded branch (`dev`) shows in the select after reopening.
- Removing the first and then another folder keeps the right names, selects and boxes; saved `folders` and `extra_choices` stay aligned.
- Folder popover fully visible at 900x560 and 900x420, list scrolls. Hovering a chip-menu row shifts the shortcut text left (padding-right 96px for the gear and star); the popover is capped at 50vh.

### Not checked
- Changing a branch through the native select was not driven live (keys do not reach the native popup).
- An end-to-end start with extra worktrees was not run.
- A long or empty known-folder popover list was not checked live (the fixture has only 3 or 4 entries).
- The first folder's worktree box being on by default for a new environment was not driven live (the fixture folder was not a git repository).
- The chip menu is still clipped at the top in a 420pt-high window (older defect).
- The release of worktrees after a failed send was not driven live.

### Known limit
- Archiving a session removes only the first folder's worktree. Extra worktrees stay on disk and in the `worktrees` records.
- The notice for a failed extra worktree does not name the folder.

### What is left
- `폴더 없음`: the desktop app runs such a session in an empty scratch folder it makes (seen in Round 22); Forge needs a design for it.
