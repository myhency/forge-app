# Changes Panel 2C (look and minor fixes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Finish PNL-02: the diff and tree look of the Claude desktop app (round 19d) and the twelve minor fixes of the Plan 48 ledger.

**Architecture:** Mostly CSS and rsx in `forge-widgets/src/changes_*.rs`; pure logic stays in `forge-features/src/change_rows.rs` / `change_comments.rs` with unit tests; git reads stay behind `GitPort` (`forge-ports`, `forge-adapter-git`).

**Tech Stack:** Rust workspace, Dioxus 0.7 desktop, mockall, `tr!` + `crates/forge-shared/src/ko.rs` (sorted by English key, tests check both ways).

**Spec:** `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md`, section "50C: look and minor fixes".

## Global Constraints

- Every visible string and `aria-label` goes through `tr!`, with a Korean entry in `ko.rs` (byte-order sorted).
- Branch `plan-50`; commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- macOS: no `sed -i`; use Edit or python.
- Tests: `cargo test -p forge-entities -p forge-features -p forge-widgets -p forge-shared -p forge-adapter-git -p forge-domain` stay green; `cargo build -p forge-app` builds.
- Colours only through existing CSS tokens (`--diff-added-bg`, `--danger`, `--hl-string`, `--fg-muted`); both light and dark must look right.
- Do not touch `crates/forge-shared/assets/ime-guard.js` or the editor bundle.

## Review Focus

- A diff with only removed lines, only added lines, and a context line with different old/new numbers: the one number column shows old for removed, new otherwise.
- Side by side keeps one column per half and the comment `+` still lines up.
- A file that is binary, `TooLarge`, or created: header has no `+0 −0`.
- Branch equals base: the button shows the branch once.
- Fallback to Uncommitted: the button no longer claims `base → head`.
- Context menu near the right/bottom edge stays inside the window (check whether already true before changing).

---

### Task 1: Look (round 19d)

**Files:**
- Modify: `crates/forge-widgets/src/changes_stack.rs` (`line_row`, `cell`, chevrons at ~344, 356, 385)
- Modify: `crates/forge-widgets/src/changes_tree.rs` (chevrons at ~51, 62, 144)
- Modify: `crates/forge-widgets/src/changes_header.rs` (`FileSearch` rows; `cd-chevron` ~73)
- Modify: `crates/forge-shared/src/theme.css` (`.cd-no`, `.diff-line`, `.cd-chevron`)
- Modify: `crates/forge-features/src/change_comments.rs` or `change_rows.rs` for one pure helper (below)

**Interfaces:**
- Produces: `pub fn line_number(line: &PatchLine) -> Option<u32>` in `forge-features` (removed → `old_no`, otherwise `new_no`, falling back to `old_no`), exported from the crate root; used by `line_row`.

- [ ] **Step 1: Failing unit test** for `line_number`: removed line with `old_no=Some(7), new_no=None` → 7; added `new_no=Some(9)` → 9; context `old_no=Some(3), new_no=Some(5)` → 5. Run `cargo test -p forge-features line_number` → FAIL (not defined).
- [ ] **Step 2: Implement** `line_number`, export it, tests pass.
- [ ] **Step 3: One number column.** In `line_row` replace the two `cd-no` spans with a single span showing `line_number(line)`. Side by side (`cell`) unchanged. In `theme.css` keep `.cd-no` width (`4ch`) and make sure the comment `+` padding (18px) still clears it.
- [ ] **Step 4: 3 px colour bar.** In `theme.css`: `.cd-lines .diff-line.added { box-shadow: inset 3px 0 0 var(--hl-string); }` and `.removed` with `var(--danger)`; keep the fill. Check that `--hl-string` is green in both themes; if the dark token is not a clear green, add `--diff-added-bar` / `--diff-removed-bar` tokens for light and dark.
- [ ] **Step 5: Chevrons.** Replace `▾`/`▸` text in the file headers, tree rows, section headers and `Commits` header with the outline chevron Forge uses elsewhere (find it: `grep -rn "chevron" crates/forge-shared/src/theme.css crates/forge-widgets/src` and reuse that class/SVG; `⌄` open, `›` closed rotation by CSS). One `.cd-chevron` rule drives them; open/closed is a class (`open`) rather than a different glyph.
- [ ] **Step 6: Search rows `+N −M`.** In `FileSearch` each row shows `+added −removed` (class `cd-counts`, `cd-add`/`cd-del`) after the name, using the counts `FileChange.file.added/removed` already carries; omit when `None`. The `files` prop currently is a list of file entries: check it has counts, otherwise pass them.
- [ ] **Step 7:** `cargo test -p forge-features -p forge-widgets -p forge-shared`, `cargo build -p forge-app`, commit `feat: Changes look: one number column, colour bar, outline chevrons, counts in search`.

### Task 2: Minors in the header, tree and menus

**Files:**
- Modify: `crates/forge-widgets/src/changes_header.rs`, `changes_tree.rs`, `changes_stack.rs`, `changes_panel.rs`
- Modify: `crates/forge-features/src/change_rows.rs`, `file_menu.rs`
- Modify: `crates/forge-shared/src/ko.rs`
- Modify: `crates/forge-ports/src/git_port.rs`, `crates/forge-adapter-git/src/lib.rs` (minor 7)

First audit each item against the code (some may already be done, e.g. `ContextMenu` already clamps with `min()/max()` and `menu_height`); fix only what is still wrong and say which were already fine in the report.

- [ ] **Step 1 (minor 1, 2):** The branch button label: `base → head` only when the shown data is against the base (`missing_base` is `None` and view is not Uncommitted); after a fall back it reads `Uncommitted changes` (`tr!`). When base == head show the branch alone. Extract the label choice into a pure fn in `change_rows.rs` (`branch_label(base, head, uncommitted_fallback) -> BranchLabel`) with table tests first (RED then GREEN), then use it in `BranchButton`; pass `fallback: bool` from `changes_panel.rs`.
- [ ] **Step 2 (minor 3):** A `TooLarge` or binary file header (and tree row) shows no `+0 −0`: render counts only when `added`/`removed` are `Some`; for those states pass nothing. Test the predicate in a pure fn if one is added.
- [ ] **Step 3 (minor 4):** File section headers get `role: "button"`, `tabindex: "0"`, and an `onkeydown` that toggles fold on Enter and Space (prevent default for Space).
- [ ] **Step 4 (minor 5):** Verify `ContextMenu` clamps at both edges (it should). If the Changes panel's menu or the `⋮`/branch popovers can still overflow at the right or bottom, fix them; otherwise record "already fine".
- [ ] **Step 5 (minor 6):** `Show in Finder` in `file_menu.rs` goes through `tr!` (see how `SHOW_IN_FILE_MANAGER` is replaced at line ~134; the Korean entry exists or is added); the `aria-label`s `More`, `Search changed files`, `Refresh` through `tr!` with `ko.rs` entries.
- [ ] **Step 6 (minor 7):** `vanished_commit` must not treat a selected commit older than the 200-commit cap as vanished. Add `GitPort::commit_exists(cwd, sha) -> Result<bool, GitError>` (`git rev-parse --verify --quiet <sha>^{commit}`), adapter test on a temp repo (a commit beyond a small cap is found; a bogus sha is not; mockall automock keeps compiling). In `changes_panel.rs` the commits effect asks `commit_exists` when `vanished_commit` says true, and resets to `All` only if it also says false.
- [ ] **Step 7 (minor 8):** `commit_meta` never ends or starts with a dangling ` · ` (unit test with an empty age and with an empty author).
- [ ] **Step 8:** tests, build, `ko.rs` sorted-order test passes, commit `fix: Changes minors: header labels, hidden counts, keyboard fold, translated labels, commit past the cap`.

### Task 3: Minors in loading, size and state

**Files:**
- Modify: `crates/forge-widgets/src/changes_panel.rs`, `crates/forge-adapter-git/src/lib.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-features/src/change_rows.rs`, `crates/forge-widgets/src/changes_stack.rs`
- Modify: `crates/forge-shared/src/ko.rs`

- [ ] **Step 1 (minor 9):** The first load runs once. Today the loaders rerun when `info` arrives with the default base (source goes from Uncommitted to All). Make the changes effect wait until the base is known (`info()` is `Some`) before its first load, unless the view or a saved base already fixes the source. Add a pure helper (e.g. `ready_to_load(view, saved_base, info_loaded) -> bool`) with table tests, use it as the early return. Verify in a log or by counting `git.changes` calls in a widget-free test of the helper only.
- [ ] **Step 2 (minor 10):** In the adapter, an untracked file larger than 50 MB is not read into the diff: it becomes `PatchState::TooLarge` (the existing 'too large' note path then shows `This file is too large to show` and the load button must NOT try to read it: the button path returns that same message). Adapter test with a sparse file (`File::set_len(51 * 1024 * 1024)`) in a temp repo: result is `TooLarge`, no allocation. Add the `ko.rs` entry if the string is new.
- [ ] **Step 3 (minor 11):** The per-session Changes state (`changes_sessions` or equivalent map in `workspace.rs`) is removed when the session closes (`close_session`/`delete_session` paths). Unit test: after close, `changes_session(id)` returns the default and the map has no entry.
- [ ] **Step 4 (minor 12):** One no-behaviour commit: doc comments on `side_by_side` and `wrap` prefs, `PatchHunk`, `vanished_commit`, and move the comment sitting above `reload_prefs` to where it belongs.
- [ ] **Step 5:** full test run for the six crates, `cargo build -p forge-app`, commits `fix: first Changes load runs once; untracked files past 50 MB are not read`, `fix: Changes state is dropped when the session closes`, `docs: tidy Changes doc comments`.

### Task 4: Docs

**Files:** `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md` ("As built" for 50C), `docs/superpowers/roadmap/backlog.md` (PNL-02 row), `docs/superpowers/roadmap/observations.md` (only if new), memory note.

- [ ] Fill 50C "As built" with what each task did and which minors were already fine; update the PNL-02 row; commit `docs: Stage 50C as built`. (The controller does the real-window check and adds its results afterwards.)
