# Plan 47: Worktree sessions, worktree location and auto archive

Date: 2026-10-17. Branch `plan-47`. Evidence: observation round 17 (`docs/superpowers/roadmap/observations.md`; roadmap rows SET-07, SET-08), the desktop app's own files read on 2026-10-17 (`git-worktrees.json`, one session file), and the docs pages `code.claude.com/docs/en/desktop` and `/worktrees`.

## Goal

A person who opens a git folder on the new-session screen can start the session in its own git worktree, as the desktop app does. When they archive the session, a worktree that holds no work is removed. Two settings of the Code page follow: where worktrees are kept (SET-08) and archiving sessions that were not used for a while (SET-07).

Success:
- On the new-session screen, a git folder's menu ends with a branch row and a worktree switch (on). Sending the first message creates `<repo>/.claude/worktrees/<name>` on branch `claude/<name>` from the chosen branch and starts the session there.
- The session shows under the main repository's group in the sidebar, with the branch and a "Worktree" tag in the header line.
- Archiving it removes the worktree and its branch when there are no changes and no commits of its own; otherwise the session is archived and the worktree stays on disk.
- `Worktree location` and `Archive inactive sessions` work as in the desktop app's Settings > Claude Code.

Decided with the user (2026-10-17):
- One spec, three stages built in order A, B, C (below).
- Archiving removes a clean worktree by itself.
- The worktree name is made from the first message (a guess; see Open facts).
- Not in this plan: the `환경` side panel, `.worktreeinclude`, "archive after PR merge or close" (with SET-14, which is left out), a worktree switch for routines (PG-01 stays without it).

## Facts seen (not guessed)

- The switch and branch rows live in the folder chip's menu, below `폴더 열기…`, only for a git folder. The switch was on for both git folders tried. A non-git folder shows neither row.
- A first message started a session in the worktree at once: `<repo>/.claude/worktrees/<slug>-<6 hex>`, branch `claude/<slug>-<6 hex>` from `master`, the Claude transcript under the worktree's own project folder with `cwd` = the worktree.
- The desktop app keeps `git-worktrees.json` and a session file with `originCwd` (the main repo), `worktreePath`, `branch`, `sourceBranch`.
- The sidebar row has no worktree mark; the group is the main repository.
- Settings text (Korean, kept as the key of Forge's own text): `비활성 세션 보관`: "일정 기간 활동이 없으면 로컬 세션을 자동으로 보관합니다. 실행 중이거나 백그라운드 작업이 있는 세션은 보관되지 않으며, 커밋되지 않은 변경사항이 있는 워크트리는 디스크에 유지됩니다." Menu: 없음 (default), 1일, 2일, 7일, 14일, 30일. `워크트리 위치`: `프로젝트 내부` (`.claude/worktrees`) and `사용자 설정…` (system folder dialog).
- Docs: a folder chosen inside the project is ignored and the default is used; branches start from the default branch unless chosen; archiving removes the worktree.

## Design

### Stage A: worktree sessions

**1. Ports and adapter**

- New `WorktreePort` in `forge-ports` (the existing `GitPort` stays read only):
  - `is_repository(path) -> bool` and `branches(repo) -> Result<BranchList, WorktreeError>` where `BranchList { current: Option<String>, local: Vec<String> }` (`git branch --format`, detached HEAD gives `current: None`).
  - `create(repo, name, base_branch, location) -> Result<Worktree, WorktreeError>`; `Worktree { name, path, branch, base_branch, repo }`.
  - `inspect(worktree) -> Result<WorktreeState, WorktreeError>`; `WorktreeState { dirty: bool, own_commits: bool }` (`status --porcelain` is not empty; `rev-list <base>..<branch>` is not empty).
  - `remove(worktree) -> Result<(), WorktreeError>`: `git worktree remove` (never `--force`), then `git branch -d` (never `-D`), so git itself refuses to lose work.
- `forge-adapter-git` gets `GitWorktrees` implementing it with the same rules as `GitCli`: `git` command, timeout, `GIT_TERMINAL_PROMPT=0`, paths after `--`. A missing folder or git gives `WorktreeError::Failed` with a message.
- Domain (`forge-domain`, pure): `worktree.rs` with `Worktree`, `slug(text) -> String`, `worktree_name(first_line, suffix) -> String`, `worktree_dir(repo, name, &WorktreeLocation) -> PathBuf`, `main_repo_of(path) -> Option<PathBuf>`.

**2. Naming**

`slug`: lower case, each run of characters that are not ASCII letters or digits becomes one `-`, trimmed, cut to 40 characters, never empty (`session` when nothing is left; Korean text gives `session` since only ASCII is kept). The name is `<slug>-<6 hex>`; the suffix comes from the caller (a random value in the app, fixed in tests) and is made again until the directory and the branch are both free. The slug comes from the first non-empty line of the first message.

**3. New-session screen**

- `NewSessionDraft` gets `worktree: Signal<bool>` (default true) and `branch: Signal<Option<String>>` (`None` = the repository's current branch).
- `FolderChip` (`forge-widgets/src/new_session_controls.rs`): when the chosen folder is a repository (probed through `WorktreePort` when the folder changes, like the existing slot probe), the menu gets two rows below `폴더 열기…`: `Branch  <current> >`, a submenu (search field `Search branches…`, the local branches, the chosen one checked), and `Worktree`, a switch. Both rows are hidden for a folder that is not a repository, and while the probe has not answered.
- The probe reads the branch list once per folder; a failed probe hides both rows and the session starts in the folder itself.
- On send (`NewSessionComposer::submit` → `Workspace::start_new`): with the switch on and a repository, create the worktree first (off the UI thread, in the same root-scope task `start_new` already uses), then open the session with `cwd` = the worktree path. The record (below) is written before the session opens. A failed create shows as a notice with git's message; no session is opened and the typed text stays.

**4. Record and grouping**

- `AppConfig.worktrees: Vec<WorktreeRecord>` (`{ name, path, repo, branch, base_branch, created_at }`), loaded with a default so an old config still reads; saved through `SettingsUseCase::save_worktrees` (the same lock and "do not replace an unreadable config" rule as `save_archived`).
- Sidebar and search group a project by `origin_of(path)`: the `repo` of a record whose `path` equals the project path; else `main_repo_of(path)` (the path read as `<root>/.claude/worktrees/<name>`, with `<root>` an existing folder — this also covers worktrees the desktop app made); else the path itself. A worktree's sessions merge into the main repo's group; the group's `+` button starts a session in the main repo.
- The session's own project path stays the worktree path (that is where the transcript is). Opening, resuming, archiving and forking keep using it; only the group header and the filters use the origin.
- The header line of an open worktree session shows the branch with a "Worktree" tag. A session whose record is missing but whose path matches `main_repo_of` shows the tag with the branch read from `git`'s `HEAD` (skipped if that fails).

### Stage B: archive removes a clean worktree, and SET-08

**5. Archive rule**

`Workspace::archive_session` (and every route to it: row menu, key `A`, `⌥⌘A`) runs, for a session whose project path is a known worktree (record, or `main_repo_of`), after the list is saved:
1. `inspect`. `dirty` or `own_commits` → keep the worktree (the row is archived; nothing else).
2. Not dirty and no own commits → `remove`. A failure shows a notice naming the folder; the session stays archived.
3. On success the record is dropped from `AppConfig.worktrees`.

The worktree of a session that is open and live is only removed after the session is closed (archiving already closes it). The check and the removal run on a blocking task, never on the UI thread. Un-archiving a session whose folder is gone keeps today's behaviour for a missing folder (the row opens read only; sending says the folder does not exist).

**6. Setting `Worktree location` (SET-08)**

- `AppConfig.worktree_location: WorktreeLocation` = `Project` (default) or `Folder(PathBuf)`; Settings > Code has a row right after the `Archive inactive sessions` row of stage C (the desktop app's order is archive, unrelated tasks, worktree location; Forge has no unrelated-tasks row).
- The row is a menu: `Inside the project` with the grey path `.claude/worktrees` (checked when `Project`), the chosen folder's path and name as a second row when `Folder`, and `Custom…`, which opens the same folder dialog the Settings page already uses (`FolderPickerPort`). A cancelled dialog changes nothing.
- `worktree_dir(repo, name, location)`: `Project` → `<repo>/.claude/worktrees/<name>`; `Folder(f)` → `<f>/<repo folder name>/<name>`, except that an `f` that is the repository or inside it is ignored and `Project` is used (the docs' rule). The folder is made when it is missing. Changing the setting never moves a worktree that exists.

### Stage C: archive inactive sessions (SET-07)

**7. Setting and rule**

- `AppConfig.auto_archive_days: Option<u32>` (`None` = off, the default); the menu offers 없음 / 1 / 2 / 7 / 14 / 30 days in the desktop app's labels (Forge's own text keys: "None", "1 day", "2 days", "7 days", "14 days", "30 days"). A menu with the same search field the desktop menu shows is not needed; the plain menu of the other rows is used.
- Pure function `sessions_to_archive(now, days, rows) -> Vec<(project, session)>` in `forge-domain`: a row is due when it is not archived, `updated_at` is at least `days` days before `now`, and it is not busy. A row's `busy` is decided by the caller from the live registry: the session is open and running, waiting for an approval, or has background work.
- `Workspace` runs it once when the project lists have loaded and then every hour (a timer in the root scope, the way the routines tick runs). Each due row goes through `archive_session`, so a worktree follows the stage B rule. One notice per run ("N sessions archived") when anything was archived. Off → the timer does nothing.
- A session that is closed but whose `updated_at` is old is archived without being opened. Forge being closed means nothing runs; the next start catches up.

### Changes made while planning (2026-10-17)

- The branch row and the worktree switch of the desktop folder menu are a **Branch chip** beside the folder chip (a normal picker list, shown only for a repository with the switch on) and a checkable **`Worktree` row** in the folder menu; the existing picker has no submenu or switch.
- `WorktreeRecord.created_at` is epoch milliseconds.
- Removal on archive only applies to worktrees Forge recorded; one the desktop app made still groups under its repository (path rule) and gets the `Worktree` tag, without a branch. `main_repo_of` reads the path only.
- A worktree is kept while another session (open, or not archived) uses the same folder (a fork keeps its folder).
- When creation fails the typed text comes back into the box; attachments and extra folders do not.
- The search palette and the Date grouping label a worktree session's row with the worktree's folder name.
- `git worktree remove` (without --force) also deletes ignored files such as `.env` and build output in the worktree; `status --porcelain` does not list them. The desktop app and git document the same behaviour.

## Out of scope

- The `환경` side panel (environment, context, connectors, Remote Control).
- `.worktreeinclude`, `worktree.baseRef`, branch from a pull request, `WorktreeCreate` hooks, SSH worktrees.
- "Branch prefix" setting and "auto archive after PR merge or close" (SET-14, left out by the user).
- Moving a session between a worktree and its main folder; deleting branches Forge did not create.
- A worktree switch for routines (PG-01); the routine form keeps no Worktree checkbox.
- The `관련 없는 작업을 요청할 때` setting and task chips.

## Open facts to check in the plan (not guessed here)

- The desktop app's worktree name for a first message: it was `acknowledge-instruction-75511b` for "Reply with just the word ok…", equal to the auto title, which only appears after the first reply. Forge cannot know the title at create time, so it uses the first line; the names may differ.
- What the desktop app does on archive with a clean worktree (removal is in the docs and the setting text; the branch's fate and the confirmation, if any, were not seen), and whether it removes a worktree that has only committed work.
- The folder layout under a custom location (`<folder>/<repo>/<name>` is a guess; the docs only describe SSH).
- Whether the branch switch of the folder menu lists remote branches. Only local ones were seen.
- `git worktree add` against a repo with submodules, LFS filters or a detached HEAD: plan 47A tests a detached HEAD and a repository with no commits (git refuses; the message is shown).

## Testing

- Pure: `slug`, `worktree_name`, `worktree_dir` (project, folder, folder inside the repo), `main_repo_of`, `origin_of` (record, path rule, neither), `sessions_to_archive` (boundary day, archived, busy, off).
- Adapter, on real temporary repositories: `branches` (current, detached), `create` (branch, folder, base), `inspect` (clean, dirty, own commit), `remove` (refused when dirty), a name clash re-rolled.
- Application: `save_worktrees` and the two settings keep an unreadable config unreplaced; the archive rule with a mocked `WorktreePort` (clean → remove, dirty → keep, remove fails → notice, record dropped).
- Features: draft defaults, the menu rows shown only for a repository, the send path calling `create` before the session opens, an error keeping the text.
- Real window (harness): the menu rows, a first message in a fixture repo, the group in the sidebar, archive of a clean and of a dirty worktree, the two settings rows, and an auto archive set to 1 day against a fixture session whose transcript is dated old. Korean and English strings through `tr!`.

## Task order

1. Stage A: domain (worktree, slug, paths), `WorktreePort` and `GitWorktrees`, config field and save, draft and menu rows, send path, grouping and header tag. Review and real-window check.
2. Stage B: archive rule, `worktree_location` and its row. Review and real-window check.
3. Stage C: `auto_archive_days`, `sessions_to_archive`, the timer and its notice, the row. Review and real-window check.
4. Roadmap rows SET-07, SET-08 and a new row for worktree sessions marked built; the memory file; this spec's "As built".
