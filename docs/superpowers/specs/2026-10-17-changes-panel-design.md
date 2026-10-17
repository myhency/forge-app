# Plan 48: Changes panel like the desktop app (PNL-02)

Date: 2026-10-17. Roadmap row: PNL-02 (`docs/superpowers/roadmap/backlog.md`). Seen in observation rounds 5, 19, 19b and 19c (`observations.md`): the Changes panel of the installed Claude app with real changes in a git worktree. Plan 21B built the first, read-only version (`2026-10-03-right-dock-panels-design.md`, "Changes").

## Goal

The Changes panel looks and works like the desktop app's: every changed file as a stacked, foldable diff; a choice of what to compare (all changes against a base branch, uncommitted changes, one commit); a folder tree of changed files; viewing options; a file search; a menu on each file row.

Plan 49 (separate) builds the `Environment` side panel with the `Changes` card and the `Commit changes` button. This plan builds no commit button.

Success:

- Opening the panel on a repository with changes shows all changed files one under the other, each with a header (fold chevron, file icon, name, grey folder, `+N −M`) and its hunks with old and new line numbers.
- The branch button names the base and the branch (`master → claude/x ⌄`); its menu switches between `All changes` (against the base), `Uncommitted changes` and the commits of the branch; the base can be changed from a searchable branch list.
- A click on a commit shows that commit's own files and diff; the button then names the commit.
- The right column lists the changed files as a folder tree (or flat) and, at its foot, a `Commits N` section with `All changes`, `Uncommitted changes` and one row per commit (subject, short sha, author, age).
- Side by side, wrap, word highlight, hide whitespace changes and folder grouping can be switched in the panel `⋮` menu and are kept between runs.
- A search icon opens a popover that filters the changed files by name; a click scrolls the diff to the file.
- A right click on a file row offers Attach as context, Copy, Open file, Open in, Show in Finder.
- A big diff does not freeze the window; a folder that is not a repository, a missing git and a git error show the same texts as today.

## Decisions made with the user

- Scope: view parity only (groups A, B and C of the 2026-10-17 talk). The commit button goes to Plan 49 with the `Environment` panel. There is no stage, unstage or revert: the desktop app has none (round 19).
- Data approach: one `git diff` call per mode for the whole patch, split by file, with a size cap; not one call per file, not one file at a time.
- Left out until seen in the desktop app: the `File list position ›` submenu, `Separate test, build and generated files`, the `Copy ›` submenu contents beyond a path copy, the `Commits ›` submenu of the branch menu (the right column's list is the same content).

## Background

What Forge has today (`crates/forge-widgets/src/changes_panel.rs`, `crates/forge-ports/src/git_port.rs`, `crates/forge-adapter-git`):

- `GitPort` has `status(cwd)` and `diff(cwd, path)`, both against `HEAD`; the panel shows a list at the left and the diff of one picked file. The panel reloads on Refresh and when a tool call of the session finishes (`finished_tool_calls`).
- `parse_unified_diff` (`forge-entities/src/diff.rs`) drops the `@@` lines, so a diff has no line numbers, and it keeps at most 300 lines. `DiffView` draws a `FileDiff`; the chat uses it too (Edit and Write cards).
- `WorktreePort::branches` lists local branches and the current one (Plan 47). The worktree record knows its base branch.
- Panels and menus: `PanelFrame` (header with `tools`), `ContextMenu` (used by the Files rows), composer attachments (`Attachment`, Plan 18).

What the desktop app does, in short (details in observations 19, 19b, 19c):

- Header: `base → branch ⌄`, search, list toggle, `⋮`, expand, close.
- Body: stacked per-file diffs, each foldable; a fold row `더 많은 줄 보기` for unchanged lines; right column with the file tree and the `Commits N` section.
- The mode and the base choice are the same state, set from the branch button's menu or from the `Commits` section.

## Design

### Entities and ports

`forge-entities/diff.rs`:

- `parse_unified_diff` keeps each hunk's header numbers: `Hunk { old_start, new_start, lines }`. `FileDiff.hunks` changes from `Vec<Vec<DiffLine>>` to `Vec<Hunk>`; the chat's `diff_for_tool` makes hunks with `old_start = new_start = 0` and the view draws no numbers for those (as today). The 300-line cap stays for the chat; the panel passes a larger cap (a parameter).
- A new `word_spans(old: &str, new: &str) -> (Vec<Span>, Vec<Span>)` (uses `similar`, already a dependency) marks the changed words of a paired removed and added line. Pairing: inside a run of removed lines followed by a run of added lines, line i pairs with line i.

`forge-ports/git_port.rs` (still read only):

```rust
pub enum ChangeSource {
    /// merge-base(base, HEAD) against the working tree, untracked files included.
    All { base: String },
    /// HEAD against the working tree, untracked files included (what 21B showed).
    Uncommitted,
    /// `sha^..sha`; for a root commit, the empty tree against `sha`.
    Commit { sha: String },
}
pub struct DiffOptions { pub ignore_whitespace: bool }
pub struct BranchInfo { pub current: Option<String>, pub default_base: Option<String>, pub branches: Vec<String> }
pub struct CommitInfo { pub sha: String, pub subject: String, pub author: String, pub time: i64 }
pub struct FileChange { pub file: ChangedFile, pub patch: PatchState }
pub enum PatchState { Text(String), Binary, TooLarge }

fn branches(&self, cwd: &Path) -> Result<BranchInfo, GitError>;
fn commits(&self, cwd: &Path, base: &str) -> Result<Vec<CommitInfo>, GitError>;   // base..HEAD, newest first, at most 200
fn changes(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions) -> Result<Vec<FileChange>, GitError>;
fn file_lines(&self, cwd: &Path, source: &ChangeSource, path: &str) -> Result<Vec<String>, GitError>; // new side, for "more lines"
```

The old `status` and `diff` stay until nothing uses them, then go (the panel is their only user; the worktree code uses `WorktreePort`).

`forge-adapter-git`:

- `default_base`: the worktree record's base if the session has one; else `origin/HEAD`'s branch; else `main`, then `master`; else `None` (then `All` is the same as `Uncommitted` and the menu says so).
- `changes`: one `git diff --no-color --no-ext-diff -M -z --numstat` for the list and counts and one `git diff ... -U3` for the patch (`-w` when `ignore_whitespace`), split on `diff --git` headers. Untracked files (not in `git diff`) are added from `git ls-files --others --exclude-standard -z` and their patch made like the 21B code does (`--no-index` against the null device). Total patch text is capped at 2 MB; a file that would pass the cap gets `TooLarge`, a binary one `Binary`. The 10 s timeout and the glob and option-looking path rules of 21B stay.
- `commits` uses `git log base..HEAD --format=%H%x1f%an%x1f%at%x1f%s -z`.
- `file_lines` uses the working tree for `All` and `Uncommitted`, and `git show sha:path` for `Commit`.

### Panel state

A `ChangesState` per session (kept in the workspace like Files state, Plan 25D): `source` (default `All` with the default base), `base` override, `collapsed: HashSet<String>`, `expanded_gaps`, `search_open`. Switching session keeps it; a restart returns to `All`. The viewing options are global, not per session (below).

Reload rule as today: Refresh, a finished tool call, and a change of `source`; each run takes a ticket and a stale result is dropped. A commit row's data does not change, so a `Commit` source is not reloaded by tool calls.

### Widgets (`forge-widgets`)

`changes_panel.rs` is split:

- `changes_panel.rs`: state, loading, layout (left diff, right column), header.
- `changes_header.rs`: the branch button and its menu (`All changes ✓ <base> compare`, `Uncommitted changes`, a rule, `Compare to <base> ›` opening a searchable branch list with the default badge) and the search icon with its popover.
- `diff_stack.rs`: one section per file (header + hunks + gap rows); side by side and wrap are CSS classes; word spans from `word_spans` draw the darker words.
- `change_tree.rs`: the right column: folder tree (chevron per folder, files indented, `+N −M`), flat when grouping is off, the `Commits N` section, row menu.

A folder tree row and a stacked section both scroll the diff to a file; the diff pane scrolls with `scrollIntoView` through the existing JS bridge. A fold row `More lines` appears after the last hunk and between hunks when `new_start` of the next hunk is more than the end of the previous; a click reads `file_lines` once and shows the gap's lines as context.

A very long list is cut at 500 rows as today (`… N more files`); only the first 50 sections are drawn expanded, the rest are folded with their header, so the window stays light.

### Viewing options

Panel `⋮` menu (checks as in the app): `Show files` (⇧⌘Y, exists), `Group files by folder`, `Collapse all files`, `Expand all files`, `Side by side`, `Wrap lines`, `Highlight changed words`, `Hide whitespace changes`, `Refresh` (also stays as a button as in 21B). Stored in the Forge config as `changes_view { group_by_folder: true, side_by_side: false, wrap: true, highlight_words: true, hide_whitespace: false }`. `Hide whitespace changes` is the only one that asks git again.

### File row menu

A `ContextMenu` on a tree row and on a stacked section header: `Attach as context` (adds the file as an `Attachment` chip in the session's composer, the Plan 18 path; disabled for a deleted file), `Copy ›` (`Path`, `Relative path`; the real app's list was not seen, so only these two), `Open file` (⌘↵, opens the Files panel on that file, as the chat's path links do), `Open in ›` (the Plan 15 editor list), `Show in Finder` (Explorer on Windows).

### Errors and edges

- Not a repository, no changes, git missing or failing: the texts of 21B (`No changes to show`, the first line of stderr).
- A base that no longer exists: `All` falls back to `Uncommitted`, a notice line names the missing base, and the branch list opens with the default marked.
- A detached HEAD: the branch button shows the short sha; `All` still works against the base.
- A deleted file: header with its name and `−N`; Attach disabled; Open file disabled.
- A renamed file: shown under its new name with the old name in grey (`-M`).
- A binary file: header, then `No textual changes to show`. `TooLarge`: header, then a row `Load diff` that runs `changes` for that file alone without the cap.
- Windows: paths from git use `/`; the tree splits on `/`; Show in Finder reads `Show in Explorer`. Not checked in a real Windows window (unit tests only, as in earlier plans).
- All strings go through `tr!` with the Korean text of the desktop app (`변경 사항`, `모든 변경사항`, `커밋되지 않은 변경사항`, `비교 대상`, `브랜치 검색`, `변경된 파일 검색`, …).

## Not built

`File list position ›`, `Separate test, build and generated files`, the `Commits ›` submenu of the branch button, the full `Copy ›` list, a commit button (Plan 49), stage, unstage and revert (not in the app), keeping the chosen mode across a restart (not observed), inline comments on diff lines, `Open in` for a deleted file.

## Testing

- Rust unit tests for `forge-adapter-git` on a temp repository (as the existing tests): `All` against a base with commits, with a renamed, deleted, binary and untracked file; `Uncommitted`; `Commit` including a root commit; `-w` hides a whitespace-only change; names with spaces and Korean letters; a base that is missing; `TooLarge` at the cap; `commits` order and limit; `branches` with a detached HEAD.
- `forge-entities`: hunk numbers from `@@ -a,b +c,d @@`, `word_spans` on paired lines (and on unpaired runs, which get none), the chat's diffs unchanged.
- Widget logic that can be tested without a window (tree building from paths, the fold-gap rule, the section cap) in `forge-features`.
- Real window checks with `scripts/harness` and a fixture repository with a base branch and two commits: the header and both menus, a commit row, folding, side by side, word highlight, hide whitespace (the git call changes), the search popover, the row menu, a big generated diff, and a narrow window (Plan 26 layout). A comparison screenshot against the desktop app for the same repository.
- After building, record the new observations of the unseen menus in `observations.md` and set PNL-02 in `backlog.md`.

## As built

Built on branch `plan-48` (commits `5b0664c` to the Task 14 fixes). Where the code differs from the design above:

- **Types**: `FileDiff`, `parse_unified_diff` and the chat's `DiffView` are unchanged (no `Hunk`, no cap parameter). The panel has its own types in `forge-entities/src/patch.rs`: `PatchHunk`, `PatchLine` (with `old_no` / `new_no`), `Span`, and the helpers `parse_patch`, `gap_after`, `gap_lines`, `side_by_side`, `word_spans`, `paired_spans`, `split_by_spans` and `cap_lines`.
- **Renames**: every git call uses `--no-renames`, not `-M`: a renamed file shows as one deleted and one added file, and no old name is drawn in grey.
- **Ports**: `GitPort` has `branches`, `commits`, `changes`, `file_lines` and also `file_patch` (one file with no size budget, for `Load diff`). `BranchInfo` also has `head_label` (the branch, or the short sha on a detached HEAD). The old `status` and `diff` are removed (Task 13).
- **Base default order**: the session's worktree record (read in the panel), then `origin/HEAD`'s branch, then `main`, then `master`. The adapter's `default_base` covers the last three; it may be a remote ref.
- **State**: per session in `ChangesSession` (forge-features `change_rows.rs`, kept in the workspace): `view` (`ChangeView`: All, Uncommitted, Commit(sha)), `base`, `folded_files`, `expanded_files` (files past the 50-section cap opened by hand), `closed_folders`. `search_open` and the gap lines are widget signals. The tree row type is exported as `ChangeTreeRow` (`TreeRow` already names the Files tree's row).
- **Result and source**: the panel keeps the source a result was loaded with next to the result, and the sections read gaps and `Load diff` against that source (in the missing-base fallback that is `Uncommitted`, so `Load diff` does not fail with the missing base).
- **Header**: the branch button sits in the panel header's tools beside the title `변경 사항` (the desktop app has no title); then the search icon, `⋮`, expand and close. The list toggle is the `Show files` row of `⋮` (⇧⌘Y) rather than its own icon. The branch menu, the `Compare to` list and the `⋮` menu are `position: fixed` at a corner measured when they open, so `.dock-tools` cannot clip them; the corner is not recomputed on a resize.
- **Commits column**: also drawn when the list has no files (a clean tree, a commit with an empty diff, a git error) once the commits are loaded, so another row can be picked. A selected commit that is gone after a reload (for example after `git reset --hard`) resets the view to `All changes`; this is noticed on a reload (Refresh or a finished tool call).
- **Edges**: a tracked file whose name git quotes (a control character or `"`) is listed as `Binary`; untracked files are never followed through a symlink and are not read when they pass the 2 MB budget (they show `Load diff`).
- **Large diffs** (Task 14): a section draws its first 2,000 lines, then a row `Show N more lines` adds 2,000 at a time up to 20,000; past that a note says how many lines are not shown. A loaded 70,000-line diff otherwise broke the layout (the web view stops laying out rows near 65,536 children).
- **Wrap off**: each section scrolls sideways on its own and its rows are as wide as the longest line (the halves of side by side no longer overlap).

### Checked in the real window (Task 14, macOS, Korean UI)

Fixture: the harness session folder made a git repository (`main` with one commit; `claude/x` with two; an edited file in `src/lib/`, a whitespace-only change, an untracked file, a 3.2 MB untracked generated file, a binary file, later 60 new files and a deleted file). Checked: the header `main → claude/x ⌄` and its menu, Escape closes it, the `Compare to` list with its search, the default badge and a pick that changes the list; the tree, a folder fold, a file click that scrolls; the `Commits 2` section, a commit row with its diff and the button naming it, `All changes` back; `git reset --hard HEAD~1` with the reset commit selected, then Refresh: back to All with the Commits section still there; an empty commit (`No changes to show` with the Commits column); fold by header, collapse all, expand all; `Load diff` on the 3 MB file; the binary note; `Show more lines`; side by side, wrap off, word highlight, hide whitespace (the whitespace-only file leaves); every `⋮` row fits (11 rows, no scroll); the search popover, Escape, a pick that scrolls to a file past the 50-section cap; the row menu on a tree row and a header (Attach adds a chip, Copy › Relative path, Open file opens the Files panel, the Open in submenu), a deleted file's menu is only Copy; a renamed base: the notice line, the uncommitted list, `Load diff` still works; a base that is gone with no default: the button reads `커밋되지 않은 변경사항`; a 700-point window (floating dock) with both menus; dark mode; the `⋮` options kept after a restart. Captures `/private/tmp/p48-*.png` (outside the repository).

Not checked in the window: Show in Finder and Open in › VS Code (they open other apps), a detached HEAD, the 500-row cap, a real worktree record as the base, Windows.
