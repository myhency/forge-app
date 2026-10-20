# Plan 54: "No folder" sessions (design proposal, not approved)

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

## Decisions needed from the user

- A. Scratch folders live under Forge's data directory (recommended) or under the Claude app's folder as the desktop app does.
- B. The sidebar group label is `이 컴퓨터` (as in the desktop app).
- C. Whether archive removes an empty scratch folder (recommended: yes) or never touches it.

## Out of scope

Choosing a folder after the session started; moving a scratch session into a project; the side panel `환경` card (stage B); Windows path specifics beyond using the platform's data directory (Task of the plan checks them).

## Testing (for the plan)

Pure: scratch folder name from a date and a seed, `is_scratch(path, root)`, the sidebar group key, the folder menu rows with the `폴더 없음` row. Use cases: making the folder, failure notice, archive removing an empty folder and keeping a non-empty one. Window: start a no-folder session in the harness (needs one real message: ask the user first), check the path, the sidebar group and the archive behaviour.
