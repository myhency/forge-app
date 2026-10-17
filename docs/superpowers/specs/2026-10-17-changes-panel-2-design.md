# Plan 50: Changes panel, the parts Plan 48 left out (PNL-02)

Date: 2026-10-17. Roadmap row: PNL-02 (`docs/superpowers/roadmap/backlog.md`). Builds on Plan 48 (`2026-10-17-changes-panel-design.md`, merged to master 4209513). Source: observation rounds 19d, 20 and 20b (`observations.md`), the installed Claude app seen on 2026-10-17. Commit button: still Plan 49, not here.

## Goal

Finish the Changes panel so it matches the desktop app where the app was looked at: the file list position, the test/build/generated split, the `Commits N ›` submenu, the full `Copy ›` list, comments on diff lines, one line-number column with a colour bar, a clean tree without the `Uncommitted` row, and the minor defects Plan 48's reviews left open.

Success:

- `File list position ›` offers Auto, Left, Right and the right column moves.
- With `Separate test, build and generated files` on, the file list and the stacked diffs show normal files first, then `Test files N` and `Generated files N` sections, using the rules seen in the app (round 20b).
- The branch button's menu has `Commits N ›` (a search field and one row per commit); on a clean tree there is no `Uncommitted changes` row there or in the `Commits` section.
- A diff line that has a new-side number shows a `+` on hover; it opens a comment box; a saved comment is a card in the composer and goes with the next message, the same cards the Files panel makes (Plan 25B).
- The unified view has one number column and a coloured bar at the left of changed lines.
- The chosen view and base of a session survive a restart.

## Decisions made with the user (2026-10-17)

- Scope is all of it (menus, state, inline comments, look, minors), in three stages that are built and checked one after the other: **50A** menus and state, **50B** diff comments, **50C** look and minors.
- Observe the desktop app first for the split (done, round 20b), then build.
- No stage, unstage, revert or commit control (the app has none; commit is Plan 49).
- 2026-10-17, while planning 50A: `Copy ›` and the deleted file's `Open in` were already built in Plan 48; they leave the scope.

## 50A: menus and state

### Settings (`forge-domain` `ChangesViewPrefs`, saved in `AppConfig.changes_view`)

Two new fields, both with serde defaults so an old config reads:

```rust
pub enum FileListPosition { Auto, Left, Right }   // default Auto
pub struct ChangesViewPrefs { ..., pub file_list_position: FileListPosition, pub separate_special: bool /* default false */ }
```

Panel `⋮` menu gets the two rows in the app's order: `Show files`, `File list position ›` (current value at the right, submenu `Auto`, `Left`, `Right` with a check), `Group files by folder`, `Separate test, build and generated files`, then the rule and the fold rows as today. Submenu opens beside the row on hover (as the app's does).

`Left` puts the file column at the left of the stacked diffs, `Right` at the right (today). `Auto` is `Right` while the panel body is 560 px or wider and moves the column UNDER the diffs below that. This narrow rule is a guess: the app's rule for `자동` was not observed (the panel was one width). It is recorded in the spec's "As built" when checked.

### Splitting special files (`forge-entities`, new `file_class.rs`)

```rust
pub enum FileClass { Normal, Test, Generated }
pub fn file_class(path: &str) -> FileClass
```

Rules from round 20b (path uses `/`, matched case-sensitively on the lower-case forms below; the first match wins, Test before Generated):

- Test: any folder named `tests`, `test`, `spec`, `specs` or `__tests__`; a file name matching `*.test.*` or `*.spec.*`, `*_test.<ext>`, `test_*.<ext>`.
- Generated: any folder named `dist` or `.next`; a file name `*.generated.*`, `*.min.js`, `*.min.css`, `*.js.map`, `*.css.map`, `*.lock`, `package-lock.json`, `pnpm-lock.yaml`.
- Everything else is Normal, in particular `build/`, `out/`, `target/` and `*.d.ts` (all seen staying in the normal list). `test` and `specs` as folder names and `*.min.css`, `*.css.map` were not probed; they follow the pattern of the seen ones and are the only guesses.

`forge-features/change_rows.rs`: `tree_rows` and `stack_order` take `separate: bool`. When on, the normal files come first (folder tree as today), then a row `ChangeTreeRow::Section { kind, count, open }` for tests and one for generated, each followed, when open, by its own folder tree. Sections with no files are not drawn. `stack_order` returns normal, then tests, then generated; inside each, the existing order. A section is collapsed when it first appears; `ChangesSession.open_sections: HashSet<FileClass>` holds the opened ones for this run. The stacked diffs on the left are NOT hidden by a collapsed section (seen in the app). The search popover lists all files.

### Branch menu

- A third row `Commits N ›` (N = commits of the branch beyond the base; the row is absent when N is 0). Its submenu: a search field `Search commits` with focus, then one row per commit with the subject cut with `…` and the short sha in grey at the right. A click sets `ChangeView::Commit(sha)` and closes the menu. The filter matches subject, sha and author, case-insensitively.
- `Uncommitted changes` is hidden in the menu and in the `Commits N` section when the tree is clean, except while it is the chosen view. Clean means `GitPort::is_dirty` is false.
- `GitPort::is_dirty(cwd) -> Result<bool, GitError>`: `git status --porcelain=v1 -z --untracked-files=all` with the same timeout, true on the first byte of output. It runs with every reload (not for a `Commit` view's own data, which does not change, but the answer is still needed for the menu).

### File row menu

Nothing to build: `Copy ›` already offers `Absolute path`, `Relative path` and `File name`, and a deleted file's menu already offers only `Copy ›` (`file_menu(MenuTarget::Change { deleted: true })`). Plan 48 listed both as left out before the desktop app's submenu was seen; round 20 showed the same three items.

### Mode across a restart

`AppConfig` gets `changes_modes: HashMap<String, SavedChangeMode>` keyed by the session's transcript id (the key `archived` uses), `SavedChangeMode { view: All | Uncommitted | Commit(sha), base: Option<String> }`. Written when the person changes view or base, read when the session's panel first loads. An entry whose commit is gone resets to All (the existing `vanished_commit` rule). Closing and deleting a session removes its entry. Folded files, open sections and closed folders stay per run.

## 50B: comments on diff lines

- A diff row with a new-side number (an added or a context line, in unified view, and the right half in side by side) shows a round `+` in the left margin while the pointer is over it. Removed lines have no `+` (they have no line in the working tree). Single line only: the app's range behaviour was not seen.
- The `+` is drawn only when the right side is the working tree: views `All changes` and `Uncommitted`. In a `Commit` view the new side is that commit's file, and a line number from it would point at the wrong line of the file Claude reads now.
- Click opens a box under that line (the rows below move down): grey title `Line N` (`N번 줄`), a text field with the placeholder `Tell Claude what to change` (`Claude에게 변경할 내용을 알려주세요`), a round submit icon at the bottom right, blue (accent) border while editing. Enter saves (Shift+Enter is a new line, also with a Korean IME composing: composition Enter does not save), Escape cancels, an empty text saves nothing.
- Saving calls `Cards::add_comment(session, absolute_path, LineRange { start: n, end: n }, text)`: the same cards, the same composer chips, the same `<forge-comment>` tag, the same restore after reopening as Plan 25B. So a comment made in the diff also shows in the Files editor on that file and line, and one made there shows in the diff (`Cards::comments_for(session, path)`). Sending consumes it.
- A saved comment is drawn under its line as a grey box with the text, a pencil (edit in place) and a red trash (`Cards::remove`). `Cards::update_comment` serves the edit.
- A comment whose line is past the end of the new file is clamped when sent (`clamp_cards`, existing). Lines that move after later edits are not followed (same rule as Plan 25B).
- What the app does after the submit, and where its comment goes, was not seen (round 20 stopped at the open box). Forge uses its own existing path; this is recorded as an assumption in the spec's "As built" and the roadmap.

Widget work: `changes_stack.rs` `line_row` and `cell` take the session id, the absolute path and the comments of that file; a small `DiffCommentBox` component holds the box states (editing, saved). The hover `+` is CSS (`:hover` on the row) with a click handler. The scroll-to-file bridge is unchanged.

## 50C: look and minor fixes

### Look (round 19d)

- Unified view: ONE number column. A removed line shows its old number, an added line and a context line their new number. Side by side keeps one column per half (it already does).
- A changed line has a 3 px bar at its left edge, green for added and red for removed, with the row fill kept.
- Fold chevrons are the outline chevrons the app uses (`⌄` open, `›` closed), drawn with the CSS chevron Forge uses in its other panels; the tree rows, section headers and the `Commits` header follow.
- Search popover rows show `+N −M` after the name.
- The `Uncommitted changes` row on a clean tree is covered in 50A.

### Minor fixes (from the Plan 48 ledger, each rated Minor there; the visible and the cheap ones)

1. The branch button reads `base → head` only when the shown data is against the base; after a fall back to Uncommitted it reads `Uncommitted changes`.
2. When base and branch are the same ref the button shows the branch only, not `claude/x → claude/x`.
3. A `TooLarge` or binary file's header shows no `+0 −0`.
4. File headers react to Enter and Space (fold), with `role="button"` and `tabindex`.
5. `ContextMenu` is clamped into the window (a menu opened near the right or bottom edge is cut off today). Shared component: the Files panel menus gain the fix.
6. `Show in Finder` goes through `tr!`; the `aria-label` texts of `More` and `Refresh` too.
7. A selected commit past the 200-commit cap is not taken for a vanished one: the check asks git (`rev-parse --verify sha^{commit}`) when the sha is not in the list.
8. `commit_meta` does not end in a dangling ` · ` when the age is empty.
9. The first load on a branch runs once, not twice (the Uncommitted pass then the All pass).
10. A loaded untracked file larger than 50 MB is not read; it shows `This file is too large to show`.
11. The per-session Changes state is removed when the session closes (it leaks today).
12. The comment and ordering nits of the ledger (doc comments on `side_by_side` and `wrap`, `PatchHunk`, `vanished_commit` and the moved comment above `reload_prefs`) are tidied in one commit with no behaviour change.

Not fixed (stay deferred): `numstat` counts ignore `-w`, `GitCli::branches` name clash, memoising `parse_patch`, `searching` flag, resize of the fixed menu corner, a 120,000-line diff beyond the 20,000-line cap.

## Not built after this plan

a range comment, a comment on a removed line, a comment in a `Commit` view, the app's own wording of the comment send (not seen), `Commits ›` item keyboard navigation, Windows real-window checks.

## Testing

- `forge-entities` `file_class`: every probe of round 20b as a table test (the 7 Test, 9 Generated and 8 Normal paths seen), plus nested folders, upper-case names, a file named `tests` (not a folder) and Windows-style separators (never reach it; paths use `/`).
- `forge-features`: `tree_rows` and `stack_order` with `separate` on and off (order, counts, empty sections hidden, closed and open); `open_sections` kept per session; saved-mode round trip, vanished commit, key removal on close; the clean-tree row rule; the commit filter.
- `forge-adapter-git` on a temp repository: `is_dirty` (clean, modified, only untracked, only ignored), `rev-parse` check for a commit older than the 200 cap, the 50 MB ceiling (with a sparse file).
- Config: an old `changes_view` without the new fields reads with the defaults; `FileListPosition` round trip.
- Widget logic without a window: the one-number rule per line kind, the comment box states, `Cards` linking by path (a diff comment appears in `comments_for` for the Files editor).
- Real window checks (harness, a fixture repository with a base branch, two commits, tests, generated and build files, a Korean file name): all menus and submenus, the split on and off, the Commits submenu and its search, a clean tree, the hover `+`, a comment saved, sent and read back after reopening the session, the same comment in Files, a `Commit` view with no `+`, Auto at a narrow width, restart keeping the mode, dark mode, the window at 700 pt (Plan 26). One screenshot comparison with the desktop app on the same repository for the split and the look.
- After each stage: update `observations.md` if something new was seen, `backlog.md` PNL-02, and the memory note.

## As built

(Filled in after each stage.)

### Stage 50A (built on branch plan-50)

- Git status is read with `--untracked-files=normal` instead of `all`.
- `changes_modes` is a Vec keyed by project path and Claude session id (like `archived`), not a HashMap keyed by transcript id; closing a session keeps its entry, only deleting drops it.
- The commit list replaces the menu content (inline, like the base list) instead of opening a flyout.
- Folders inside the Test and Generated sections have namespaced folder keys (`test:src`, `generated:src`), so the same folder among the normal files stays a separate row and closes separately.
- A mode chosen before the config has loaded is not saved.
