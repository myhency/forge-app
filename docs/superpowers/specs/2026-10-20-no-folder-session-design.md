# Plan 54: "No folder" sessions

Date: 2026-10-20. Branch `plan-53` (docs only). Evidence: observation Round 22 (`docs/superpowers/roadmap/observations.md`), roadmap row OTH-11 ("Not built: 폴더 없음").

## What the desktop app does (seen)

- The folder chip's menu is: search `폴더 검색…`, `폴더 없음` (checked when none is chosen), `최근` (recent folders), `폴더 열기…`. With none chosen the chip reads `선택…`. A first message can be sent without a folder.
- The session then runs in an **empty folder the app makes**: `~/Library/Application Support/Claude/scratch-workspaces/<account>/<org>/scratch-<YYYY-MM-DD>-<6 hex>` (`cwd` = `originCwd`). The Claude transcript goes under the matching `~/.claude/projects/` folder. The session's own record has no environment id (`envScopeId: "builtin_local"`).
- In the sidebar such sessions form a group named `이 컴퓨터` (with `+`). The session's `환경` card shows `이 컴퓨터` and a row `폴더 선택…` that lets the person choose a folder afterwards.

## Proposal

Goal: a person can start a session without choosing a folder and still get a place to work. Local only (no cloud/SSH/Remote Control).

1. **Menu row.** The new-session folder menu gets `폴더 없음` as its first row (checked while nothing is chosen), above `최근`. The chip reads `선택…` while it is chosen. Picking a recent folder or `폴더 열기…` unchecks it.
2. **Scratch folder.** On the first send with no folder, Forge makes an empty folder `<scratch root>/scratch-<YYYY-MM-DD>-<6 hex>` and starts the session in it. `<scratch root>` is a folder under Forge's own data directory (the config file's folder, `scratch-workspaces/`), not under the Claude app's folder: Forge must not write into another app's data. The folder is made right before the session opens; if making it fails the notice says so and the text comes back (as the worktree failure does).
3. **Sidebar group.** A session in a scratch folder is grouped under `이 컴퓨터` (the label of the desktop app; Korean "This computer") instead of a folder name. The group header's `+` starts another no-folder session.
4. **Which sessions are scratch.** A project path inside `<scratch root>` is a scratch project. It is never offered in the folder menu's recent list, and it is not added to the trusted folders.
5. **No default for the empty list.** Today an empty folder list disables the composer ("No existing project folder was found…"). With `폴더 없음` the composer stays usable: a person with no past projects can start a scratch session. The banner text is kept for the case where the person chose a folder that no longer exists.
6. **Cleanup.** A scratch folder that is still empty when its session is archived is removed with it (like a clean worktree). A scratch folder that holds files stays (the files are the person's); archiving never deletes them. No timer, no bulk delete.
7. **Environments.** An environment always has folders (it is not a "no folder" session). Out of scope: choosing a folder afterwards (`폴더 선택…` in the card is part of stage B's `환경` card).

## Decided with the user (2026-10-20)

- A. Scratch folders live under Forge's own data directory: `<folder of the config file>/scratch-workspaces/` (so a harness run with `FORGE_CONFIG_PATH` keeps them in its fixture).
- B. The sidebar group label is `이 컴퓨터`.
- C. Archiving removes an empty scratch folder; one that holds files stays.

## Out of scope

Choosing a folder after the session started; moving a scratch session into a project; the side panel `환경` card (stage B); Windows path specifics beyond using the platform's data directory (Task of the plan checks them).

## Testing (for the plan)

Pure: scratch folder name from a date and a seed, `is_scratch(path, root)`, the sidebar group key, the folder menu rows with the `폴더 없음` row. Use cases: making the folder, failure notice, archive removing an empty folder and keeping a non-empty one. Window: start a no-folder session in the harness (needs one real message: ask the user first), check the path, the sidebar group and the archive behaviour.

## As built (Plan 54)

Built on branch plan-53 in five tasks, each reviewed:

- Scratch helpers in `forge-domain`: folder name from a date and a seed, `is_scratch(root, path)` (strict: a direct child of the root named `scratch-...`), `is_scratch_group` (the root or a direct child, display only) and `origin_for_grouping`.
- `ScratchUseCase` (`create`, `ensure_root`, `release`) over a port with `create_new_dir`. The deletion gate in `release` is lexical plus a canonical re-check and a non-recursive `remove_dir` (never follows a symlink): only an empty direct child of the root goes; `Removed`, `Kept` and `NotScratch` are the results. The root is `<folder of the config file>/scratch-workspaces/`.
- New-session screen: `폴더 없음` row in the folder menu (checked when no folder is chosen), chip `폴더 선택`; the scratch folder is made inside `start_new` and released by `abandon_start` when opening the session fails (the notice `이 세션의 폴더를 만들지 못했습니다` returns the text). The composer no longer dead-ends with an empty folder list. Environments take precedence over `폴더 없음`.
- Sidebar: every scratch project is one `이 컴퓨터` group (the root is the group key; each row keeps its own scratch folder as project path); the group `+` and ⇧⌘N from a scratch session start another no-folder session.
- Archive (this task): `archive_session` calls `release_scratch` after `release_worktree`: only for a strict scratch project, skipped while another open or un-archived session lives in the folder (`worktree_in_use`), then `ScratchUseCase::release`; `Removed` re-reads the list; an error shows `폴더 {name}을(를) 삭제할 수 없습니다`. The decision is `should_release_scratch`, tested.

Review-time fixes recorded in the ledger: Task 2 made folder creation atomic through the port (`create_new_dir`; `create` retries five seeds) instead of check-then-create. Task 3: the slash commands and the model/effort pickers vanished while `폴더 없음` was chosen, so the probe now runs in the scratch root (`ensure_root`, `probe_in_scratch_root`), and `context_project` is empty on those screens. Task 4: several scratch folders made several `이 컴퓨터` groups and ⇧⌘N from a scratch session went to an unrelated folder, so all scratch projects now merge into one group and `new_session_here` starts a no-folder session; the `+` shows whenever a scratch root exists.

Checked in a real window (harness, fixture, no message sent): the menu row and chip; one `이 컴퓨터` group with several rows; `+` and ⇧⌘N; commands, effort and model pickers while `폴더 없음` is chosen; ⌥⌘A on a scratch session whose folder is empty removed the folder from disk and the row (and the group when it was the last); with a file in the folder the row went and the folder stayed; with two sessions in one folder, archiving one kept the folder.

Whole-branch review fixes: with no past projects, no extra folders and no environment, `폴더 없음` now counts as chosen from the start (`effective_no_folder`, used by the chip, the composer gate, submit, the probe, the banner and `context_project`; the stored `new_no_folder` flag is only the pick), so a first-time user is not stuck. The scratch root is canonical (`ScratchUseCase::root()` resolves symlinks once the root exists, and `create` returns the real path), because Claude Code writes the real path as a session's cwd; past sessions are therefore recognised after a reload even when the config folder sits behind a symlink. The date in the folder name is the local date (UTC when the offset is unknown).

Deferred (not fixed): `폴더 없음` sits under the `최근` heading (the Picker heading is its title); an archived scratch session whose empty folder was removed fails to resume with a notice (same as worktrees); `release_scratch` repeats the live-list gathering of `release_worktree`; the Home dashboard and the Settings line may show `scratch-...` instead of `이 컴퓨터`; a scratch folder kept because it holds files stays on disk (no UI to clean it; its archived session is reachable through the Status filter); `.DS_Store` or Claude's own `.claude/` files make a folder non-empty, so it is kept.

NOT checked: the end-to-end send of a real message from a `폴더 없음` session (a real turn on the user's account); the effort and model pickers against the real SDK probe in the scratch root; the Windows behaviour of the data directory path; the `+` tooltip text.

