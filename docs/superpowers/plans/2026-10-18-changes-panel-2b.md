# Changes Panel 2B: Comments on Diff Lines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A diff line in the Changes panel shows a `+` on hover; it opens a comment box under the line; a saved comment is the same card the Files panel makes, goes with the next message, and shows in both panels.

**Architecture:** Pure helpers in `forge-features` (`change_comments.rs`: which source and which line may be commented, which comments sit on a line, the box's key rule, the absolute path). A new widget `DiffCommentBox` (`changes_comment.rs`) draws the editing and saved states and talks to the existing `Workspace` card API (`add_comment`, `update_comment`, `remove_card`, `cards`). `changes_stack.rs` passes a small `CommentEnv` to its row functions, which draw the `+` and a slot under the line. The panel passes the repository root so card paths are absolute.

**Tech Stack:** Rust 2021, Dioxus 0.7 desktop, CSS.

**Spec:** `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md` (stage 50B). Observation: `docs/superpowers/roadmap/observations.md` round 20 (the `+`, the `N번 줄` box, the placeholder `Claude에게 변경할 내용을 알려주세요`, a round icon bottom right, Escape closes without saving). Plan 25B (`docs/superpowers/specs/2026-10-05-line-comments-design.md`) built the cards this plan reuses.

## Global Constraints

- A comment is a `forge_domain::Card` of kind `Comment` made through `Workspace::add_comment(id, absolute_path, LineRange { start: n, end: n }, text) -> u64`; edits use `update_comment(id, card_id, text)`, deletes `remove_card(id, card_id)`. No new storage, no new tag, no change to how cards are sent or restored. (spec "50B")
- `Card.path` is an ABSOLUTE path. The Changes panel's file paths are relative to the repository root, which may be above the session folder: the absolute path is `menu_root(branch_info.root, &cwd).join(path)` (the same root the file menu uses).
- The `+` is drawn only on a row that has a new-side line number and is not a removed line: an added or a context line in the unified view, and the right half of side by side. It is never drawn in a `Commit` view (`ChangeSource::Commit`), because that view's new side is the commit's file, not the working tree Claude reads. (spec "50B")
- Single line only: `LineRange::new(n, n)`. A comment whose line is past the end of the file is clamped when sent (existing `clamp_cards`, nothing to build). (spec)
- Box: grey title `Line N` (the existing string `Line {n}`), a text field with the placeholder `Tell Claude what to change` (Korean `Claude에게 변경할 내용을 알려주세요`), a round submit icon at the bottom right, accent border while editing. Enter saves, Shift+Enter is a new line, Enter during IME composition (`event.is_composing()`) does not save, Escape cancels, an empty or blank text saves nothing. (spec "50B")
- A saved comment is drawn under its line as a grey box with its text, a pencil (edit in place) and a red trash (delete). It shows for every `Comment` card whose `path` is this file's absolute path and whose `range.end` is this line, so a comment made in the Files editor shows in the diff and the reverse. (spec "50B")
- What the desktop app does after submit and where its comment goes was NOT seen; Forge uses its own existing path (record it in the spec's As built). (spec)
- Every user-visible string goes through `tr!` and has its Korean text in `crates/forge-shared/src/ko.rs`, kept sorted by English text in byte order; `cargo test -p forge-shared` checks both ways.
- Known limit, accepted for this stage and recorded in the spec: a reload that changes a file's diff (a tool call finished and edited it) remounts that file's section, so an unsent draft in its open box is lost.
- Rust style: match the surrounding code (comments explain why, short; no new crates). The commit trailer is `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- Enter while an IME is composing (Korean) must not save a half-typed syllable. Test: `enter_while_composing_is_ignored` (Task 1).
- A blank or whitespace-only text saves nothing and does not close the box with an empty card. Tests: `a_blank_text_saves_nothing` (Task 1) and the box's `store` helper returning false (Task 2).
- A removed line, a line with no new-side number and a `Commit` view never offer a comment. Tests: `a_removed_line_has_no_comment_line`, `a_commit_view_cannot_be_commented` (Task 1).
- Two files with the same name in different folders, or the same line number in two files: comments stay with their own path and line. Test: `comments_on_matches_the_path_and_the_line_only` (Task 1).
- A multi-line comment made in the Files editor shows once, under its last line. Test: `a_range_comment_shows_under_its_last_line` (Task 1).
- Comments made while the session folder is below the repository root: absolute path = root + relative path. Test: `the_absolute_path_is_the_root_plus_the_relative_path` (Task 1).
- A removed `+` hover target must not steal a row click or break text selection (the button sits in the row's left margin). Checked in the window (Task 4).

---

### Task 1: Pure helpers

**Files:**
- Create: `crates/forge-features/src/change_comments.rs`
- Modify: `crates/forge-features/src/lib.rs` (add `mod change_comments;` next to `mod change_rows;` and a `pub use change_comments::{...}` line)

**Interfaces:**
- Produces (re-exported from `forge_features`):
  - `commentable(source: &ChangeSource) -> bool`
  - `comment_line(line: &PatchLine) -> Option<u32>`
  - `comments_on(cards: &[Card], path: &str, line: u32) -> Vec<Card>`
  - `comment_text(raw: &str) -> Option<String>`
  - `absolute_path(root: &Path, relative: &str) -> String`
  - `enum CommentKey { Save, Cancel, Ignore }` and `comment_key(enter: bool, escape: bool, shift: bool, composing: bool) -> CommentKey`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/change_comments.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::LineRange;

    fn line(kind: DiffLineKind, old_no: Option<u32>, new_no: Option<u32>) -> PatchLine {
        PatchLine { kind, text: String::new(), old_no, new_no }
    }

    fn comment(id: u64, path: &str, from: u32, to: u32) -> Card {
        Card { id, kind: CardKind::Comment, path: path.to_string(), range: LineRange::new(from, to), text: format!("c{id}") }
    }

    #[test]
    fn a_commit_view_cannot_be_commented() {
        assert!(commentable(&ChangeSource::Uncommitted));
        assert!(commentable(&ChangeSource::All { base: "main".to_string() }));
        assert!(!commentable(&ChangeSource::Commit { sha: "abc".to_string() }));
    }

    #[test]
    fn an_added_or_context_line_is_commented_by_its_new_number() {
        assert_eq!(comment_line(&line(DiffLineKind::Added, None, Some(7))), Some(7));
        assert_eq!(comment_line(&line(DiffLineKind::Context, Some(5), Some(7))), Some(7));
    }

    #[test]
    fn a_removed_line_has_no_comment_line() {
        assert_eq!(comment_line(&line(DiffLineKind::Removed, Some(5), None)), None);
        // Even a removed line that somehow carries a new number is not commentable.
        assert_eq!(comment_line(&line(DiffLineKind::Removed, Some(5), Some(5))), None);
    }

    #[test]
    fn a_line_with_no_new_number_has_no_comment_line() {
        assert_eq!(comment_line(&line(DiffLineKind::Added, None, None)), None);
    }

    #[test]
    fn comments_on_matches_the_path_and_the_line_only() {
        let cards = vec![
            comment(1, "/r/src/a.rs", 3, 3),
            comment(2, "/r/src/a.rs", 4, 4),
            comment(3, "/r/other/a.rs", 3, 3),
            Card { id: 4, kind: CardKind::Excerpt, path: "/r/src/a.rs".to_string(), range: LineRange::new(3, 3), text: "x".to_string() },
        ];
        let found: Vec<u64> = comments_on(&cards, "/r/src/a.rs", 3).iter().map(|c| c.id).collect();
        assert_eq!(found, vec![1]);
        assert!(comments_on(&cards, "/r/src/a.rs", 9).is_empty());
    }

    #[test]
    fn a_range_comment_shows_under_its_last_line() {
        let cards = vec![comment(1, "/r/a.rs", 3, 6)];
        assert!(comments_on(&cards, "/r/a.rs", 3).is_empty());
        assert_eq!(comments_on(&cards, "/r/a.rs", 6).len(), 1);
    }

    #[test]
    fn a_blank_text_saves_nothing() {
        assert_eq!(comment_text(""), None);
        assert_eq!(comment_text("  \n\t "), None);
        assert_eq!(comment_text("  fix this \n"), Some("fix this".to_string()));
        assert_eq!(comment_text("여기 고쳐주세요"), Some("여기 고쳐주세요".to_string()));
    }

    #[test]
    fn the_absolute_path_is_the_root_plus_the_relative_path() {
        assert_eq!(absolute_path(Path::new("/work/repo"), "src/a.rs"), "/work/repo/src/a.rs");
        assert_eq!(absolute_path(Path::new("/work/repo"), "한글 폴더/a b.rs"), "/work/repo/한글 폴더/a b.rs");
    }

    #[test]
    fn enter_saves_shift_enter_is_a_new_line_and_escape_cancels() {
        assert!(matches!(comment_key(true, false, false, false), CommentKey::Save));
        assert!(matches!(comment_key(true, false, true, false), CommentKey::Ignore));
        assert!(matches!(comment_key(false, true, false, false), CommentKey::Cancel));
        assert!(matches!(comment_key(false, false, false, false), CommentKey::Ignore));
    }

    #[test]
    fn enter_while_composing_is_ignored() {
        assert!(matches!(comment_key(true, false, false, true), CommentKey::Ignore));
        assert!(matches!(comment_key(false, true, false, true), CommentKey::Ignore));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod change_comments;` to `forge-features/src/lib.rs` first, then run: `cargo test -p forge-features change_comments 2>&1 | tail -15`
Expected: FAIL to compile (`commentable` and the rest not found).

- [ ] **Step 3: Implement**

Put above the tests in `change_comments.rs`:

```rust
//! Rules for comments on diff lines of the Changes panel. A comment is the same card the Files
//! panel makes (`Workspace::add_comment`), so these helpers only decide where one may be made and
//! which saved ones belong under a line.

use std::path::Path;

use forge_domain::{Card, CardKind};
use forge_entities::{DiffLineKind, PatchLine};
use forge_ports::ChangeSource;

/// Whether the new side of the diff is the working tree. In a commit's own diff it is the commit's
/// file, and a line number from it could point at the wrong line of the file Claude reads now.
pub fn commentable(source: &ChangeSource) -> bool {
    !matches!(source, ChangeSource::Commit { .. })
}

/// The line number a comment on this row would carry: the new-side number of an added or a context
/// line. A removed line is not in the working tree.
pub fn comment_line(line: &PatchLine) -> Option<u32> {
    match line.kind {
        DiffLineKind::Removed => None,
        DiffLineKind::Added | DiffLineKind::Context => line.new_no,
    }
}

/// The comment cards of the file `path` (absolute) drawn under `line`. A comment made over a range
/// in the Files editor shows once, under its last line.
pub fn comments_on(cards: &[Card], path: &str, line: u32) -> Vec<Card> {
    cards.iter().filter(|card| card.kind == CardKind::Comment && card.path == path && card.range.end == line).cloned().collect()
}

/// The text to save: trimmed, or `None` when there is nothing but blanks.
pub fn comment_text(raw: &str) -> Option<String> {
    let text = raw.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The absolute path of a changed file. Git prints paths relative to the repository root, which may
/// lie above the session folder.
pub fn absolute_path(root: &Path, relative: &str) -> String {
    root.join(relative).to_string_lossy().to_string()
}

/// What a key press in the comment field does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKey {
    Save,
    Cancel,
    Ignore,
}

/// Enter saves, Shift+Enter is a new line (the field handles it), Escape cancels. While an input
/// method is composing, Enter confirms the letters being typed and must not save.
pub fn comment_key(enter: bool, escape: bool, shift: bool, composing: bool) -> CommentKey {
    if composing {
        return CommentKey::Ignore;
    }
    if escape {
        CommentKey::Cancel
    } else if enter && !shift {
        CommentKey::Save
    } else {
        CommentKey::Ignore
    }
}
```

In `crates/forge-features/src/lib.rs`, after the `pub use change_rows::{...}` line add:

```rust
pub use change_comments::{absolute_path, comment_key, comment_line, comment_text, commentable, comments_on, CommentKey};
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features change_comments 2>&1 | tail -15`
Expected: PASS, 10 tests. If `absolute_path` clashes with an existing `absolute` export, keep the name `absolute_path` (the widgets' `files_state::absolute` returns a `PathBuf`; this one returns a `String`).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/change_comments.rs crates/forge-features/src/lib.rs
git commit -m "feat: rules for comments on diff lines

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The comment box widget

**Files:**
- Create: `crates/forge-widgets/src/changes_comment.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (add `mod changes_comment;` beside the other `changes_*` mods)
- Modify: `crates/forge-shared/src/theme.css` (rules after the `.cm-comment-*` block near line 896)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: Task 1 helpers; `Workspace::{add_comment, update_comment, remove_card}`, `forge_features::{use_workspace, Workspace}`.
- Produces: `pub(crate) fn store(workspace: Workspace, id: LiveSessionId, path: &str, line: u32, card: &Option<Card>, raw: &str) -> bool` (true when something was saved) and the component

```rust
#[component]
pub(crate) fn DiffCommentBox(id: LiveSessionId, path: String, line: u32, card: Option<Card>, on_done: EventHandler<()>) -> Element
```

`card: None` is a new comment being typed (`on_done` closes the slot when it is saved or cancelled); `card: Some(..)` is a saved one (grey, pencil, trash; the pencil edits in place and Escape returns to the saved view).

- [ ] **Step 1: Write the failing test**

Create `crates/forge-widgets/src/changes_comment.rs` with a test of `store` first. `Workspace` needs a running Dioxus runtime to build, so test through the pure part instead: split the decision out as a function that takes closures and test that.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{CardKind, LineRange};

    fn saved() -> Card {
        Card { id: 9, kind: CardKind::Comment, path: "/r/a.rs".to_string(), range: LineRange::new(3, 3), text: "old".to_string() }
    }

    #[test]
    fn a_blank_text_stores_nothing() {
        let mut calls = Vec::new();
        let kept = apply("/r/a.rs", 3, &None, "  \n", |c| calls.push(c));
        assert!(!kept);
        assert!(calls.is_empty());
        let kept = apply("/r/a.rs", 3, &Some(saved()), "", |c| calls.push(c));
        assert!(!kept);
        assert!(calls.is_empty());
    }

    #[test]
    fn a_new_text_is_added_on_its_line_and_path() {
        let mut calls = Vec::new();
        assert!(apply("/r/a.rs", 3, &None, " fix it ", |c| calls.push(c)));
        assert_eq!(calls, vec![Store::Add { path: "/r/a.rs".to_string(), line: 3, text: "fix it".to_string() }]);
    }

    #[test]
    fn a_text_for_a_saved_card_updates_that_card() {
        let mut calls = Vec::new();
        assert!(apply("/r/a.rs", 3, &Some(saved()), "new", |c| calls.push(c)));
        assert_eq!(calls, vec![Store::Update { card_id: 9, text: "new".to_string() }]);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod changes_comment;` to `crates/forge-widgets/src/lib.rs` first, then run: `cargo test -p forge-widgets changes_comment 2>&1 | tail -12`
Expected: FAIL to compile (`apply`, `Store` not found).

- [ ] **Step 3: Implement the component**

Put above the tests:

```rust
//! The box under a diff line of the Changes panel: a comment being typed, or a saved one. A comment
//! is the card the Files panel makes too (`Workspace::add_comment`), so it rides with the next message
//! and shows in both panels. See the spec `2026-10-17-changes-panel-2-design.md`, stage 50B.

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{Card, LineRange};
use forge_features::{comment_key, comment_text, use_workspace, CommentKey, Workspace};
use forge_shared::tr;

/// What saving a comment asks of the card list. A value, so the decision can be tested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Store {
    Add { path: String, line: u32, text: String },
    Update { card_id: u64, text: String },
}

/// Decides what saving `raw` does: nothing for a blank text, an update for a saved card, an add
/// for a new one. `run` performs it. True when something was saved.
pub(crate) fn apply(path: &str, line: u32, card: &Option<Card>, raw: &str, mut run: impl FnMut(Store)) -> bool {
    let Some(text) = comment_text(raw) else { return false };
    match card {
        Some(card) => run(Store::Update { card_id: card.id, text }),
        None => run(Store::Add { path: path.to_string(), line, text }),
    }
    true
}

/// Saves `raw` through the workspace's cards.
pub(crate) fn store(workspace: Workspace, id: LiveSessionId, path: &str, line: u32, card: &Option<Card>, raw: &str) -> bool {
    apply(path, line, card, raw, |action| match action {
        Store::Add { path, line, text } => {
            workspace.add_comment(id, path, LineRange::new(line, line), text);
        }
        Store::Update { card_id, text } => workspace.update_comment(id, card_id, text),
    })
}

/// `card: None` is a new comment being typed: `on_done` runs when it is saved or cancelled, so the
/// slot that holds it can close (a saved comment then comes back from the cards as a `Some` box).
/// `card: Some(..)` is a saved comment with a pencil and a trash.
#[component]
pub(crate) fn DiffCommentBox(id: LiveSessionId, path: String, line: u32, card: Option<Card>, on_done: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut text = use_signal(|| card.as_ref().map(|card| card.text.clone()).unwrap_or_default());
    let mut editing = use_signal(|| card.is_none());
    let title = tr!("Line {n}", n = line);
    let saved_text = card.as_ref().map(|card| card.text.clone()).unwrap_or_default();

    // The one way out of the field: save (Enter or the round icon), cancel (Escape).
    let finish = {
        let (path, card) = (path.clone(), card.clone());
        move |save: bool| {
            if save {
                if store(workspace, id, &path, line, &card, &text()) {
                    editing.set(false);
                    if card.is_none() {
                        on_done.call(());
                    }
                }
            } else if let Some(card) = &card {
                text.set(card.text.clone());
                editing.set(false);
            } else {
                on_done.call(());
            }
        }
    };
    let (finish_key, finish_icon) = (finish.clone(), finish);

    rsx! {
        div { class: if editing() { "cd-comment editing" } else { "cd-comment saved" },
            div { class: "cd-comment-title", "{title}" }
            if editing() {
                textarea {
                    class: "cd-comment-input",
                    rows: "2",
                    placeholder: tr!("Tell Claude what to change"),
                    value: "{text}",
                    oninput: move |event| text.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        let key = event.key();
                        match comment_key(key == Key::Enter, key == Key::Escape, event.modifiers().shift(), event.is_composing()) {
                            CommentKey::Save => {
                                event.prevent_default();
                                finish_key(true);
                            }
                            CommentKey::Cancel => {
                                // The panel's own Escape handling (closing a menu) must not also run.
                                event.stop_propagation();
                                finish_key(false);
                            }
                            CommentKey::Ignore => {}
                        }
                    },
                }
                button {
                    class: "cd-comment-ok",
                    r#type: "button",
                    title: tr!("Add comment"),
                    "aria-label": "Add comment",
                    onclick: move |_| finish_icon(true),
                    "⊕"
                }
            } else {
                div { class: "cd-comment-text", onclick: move |_| editing.set(true), "{saved_text}" }
                if let Some(card) = card.clone() {
                    button {
                        class: "cd-comment-edit",
                        r#type: "button",
                        title: tr!("Edit comment"),
                        "aria-label": "Edit comment",
                        onclick: move |_| editing.set(true),
                        "✎"
                    }
                    button {
                        class: "cd-comment-delete",
                        r#type: "button",
                        title: tr!("Delete comment"),
                        "aria-label": "Delete comment",
                        onclick: move |_| workspace.remove_card(id, card.id),
                        "🗑"
                    }
                }
            }
        }
    }
}
```

If `Workspace` is not exported from `forge_features` under that name, import it from where `use_workspace`'s return type lives (`crates/forge-features/src/workspace.rs`, `pub struct Workspace`); if the closure `finish` is not `Clone` because it captures a `Signal`-based closure by move, it is (signals and `EventHandler` are `Copy`, `String`/`Option<Card>` are `Clone`), so `finish.clone()` compiles.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-widgets changes_comment 2>&1 | tail -12`
Expected: PASS, 3 tests. Then `cargo build -p forge-app 2>&1 | tail -5` (the component is not used yet; `#![deny(dead_code)]`-style warnings for unused `pub(crate)` items are fine only if the crate already tolerates them: if the build fails on dead code, add the use in Task 3 first and rerun then).

- [ ] **Step 5: CSS and Korean text**

In `crates/forge-shared/src/theme.css` after the `.cm-comment-box.saved` rule add (look at the file's UI font variable in `:root` or `body` and use it for `font-family`, because the box sits inside the monospace `.cd-lines`):

```css
.cd-comment { box-sizing: border-box; margin: 4px 12px 4px 48px; max-width: 640px; padding: 6px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg); font-family: inherit; font-size: 12px; line-height: 1.5; white-space: normal; position: relative; }
.cd-comment.editing { border-color: var(--accent); }
.cd-comment-title { color: var(--fg-muted); margin-bottom: 4px; }
.cd-comment-input { display: block; width: calc(100% - 30px); box-sizing: border-box; resize: none; border: none; outline: none; background: transparent; color: var(--fg); font: inherit; }
.cd-comment-ok, .cd-comment-edit, .cd-comment-delete { border: none; background: transparent; cursor: pointer; color: var(--fg-muted); font-size: 13px; }
.cd-comment-ok { position: absolute; right: 8px; bottom: 6px; }
.cd-comment-edit, .cd-comment-delete { float: right; margin-left: 6px; }
.cd-comment-delete { color: var(--danger); }
.cd-comment.saved .cd-comment-text { cursor: pointer; white-space: pre-wrap; overflow-wrap: anywhere; }
.cd-comment-slot { grid-column: 1 / -1; }
```

Where the page's UI font is a CSS variable (for example `--font-sans`), replace `font-family: inherit` with it so the monospace of the diff does not leak into the box. Add to `crates/forge-shared/src/ko.rs`, each in sorted byte position:

```rust
    ("Delete comment", "코멘트 삭제"),
    ("Edit comment", "코멘트 수정"),
    ("Tell Claude what to change", "Claude에게 변경할 내용을 알려주세요"),
```

Run: `cargo test -p forge-shared 2>&1 | tail -6`
Expected: PASS. (The three strings are used by `tr!` in `changes_comment.rs`; the used-in-sources test counts them.)

- [ ] **Step 6: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: the comment box for a diff line

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The `+` and the slot in the diff rows

**Files:**
- Modify: `crates/forge-widgets/src/changes_stack.rs` (the row functions, `hunk_view`, `FileSection`, `DiffStack`)
- Modify: `crates/forge-widgets/src/changes_panel.rs` (pass the root)
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 1 helpers (`commentable`, `comment_line`, `comments_on`, `absolute_path`), Task 2 `DiffCommentBox`.
- Produces: `DiffStack` gets a prop `root: PathBuf`; `FileSection` gets `root: PathBuf` and `commentable: bool`; an internal `CommentEnv` struct.

- [ ] **Step 1: Write the failing test**

In `changes_stack.rs`, inside the existing `tests` module, add a test for the one pure decision this task adds: which rows get a `+`. Extract it first as `fn offers_comment(line: &PatchLine, right: bool, side_by_side: bool) -> Option<u32>` (the number to comment on, or `None`), and test it:

```rust
    #[test]
    fn a_plus_is_offered_on_added_and_context_lines_but_not_on_removed_ones() {
        let added = line(DiffLineKind::Added, None, Some(11));
        let context = line(DiffLineKind::Context, Some(10), Some(10));
        let removed = line(DiffLineKind::Removed, Some(5), None);
        assert_eq!(offers_comment(&added, false, false), Some(11));
        assert_eq!(offers_comment(&context, false, false), Some(10));
        assert_eq!(offers_comment(&removed, false, false), None);
    }

    #[test]
    fn side_by_side_offers_it_in_the_right_half_only() {
        let context = line(DiffLineKind::Context, Some(10), Some(10));
        assert_eq!(offers_comment(&context, true, true), Some(10));
        assert_eq!(offers_comment(&context, false, true), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-widgets a_plus_is_offered 2>&1 | tail -10`
Expected: FAIL to compile (`offers_comment` not found).

- [ ] **Step 3: Implement the rows**

In `changes_stack.rs` add the imports `use forge_domain::Card;`, `use forge_features::{absolute_path, comment_line, commentable, comments_on};` (merge into the existing `forge_features` import) and `use crate::changes_comment::DiffCommentBox;`, then:

```rust
/// What the rows need to offer comments on the new side of the diff. It exists only when the new
/// side is the working tree (`commentable`).
#[derive(Clone)]
struct CommentEnv {
    id: LiveSessionId,
    /// The file's absolute path: the key of its comment cards.
    path: String,
    cards: Vec<Card>,
    /// The line whose empty box is open.
    open: Option<u32>,
    on_open: EventHandler<u32>,
    on_close: EventHandler<()>,
}

/// The line a `+` on this row would comment on: the new number of an added or a context line; in
/// side by side only the right half has one.
fn offers_comment(line: &PatchLine, right: bool, side_by_side: bool) -> Option<u32> {
    if side_by_side && !right {
        return None;
    }
    comment_line(line)
}

/// The round `+` in the row's left margin (shown while the pointer is over the row).
fn add_button(line: Option<u32>, env: Option<&CommentEnv>) -> Element {
    let (Some(line), Some(env)) = (line, env) else { return rsx! {} };
    let on_open = env.on_open;
    rsx! {
        button {
            class: "cd-add-comment",
            r#type: "button",
            title: tr!("Add comment"),
            "aria-label": "Add comment",
            onclick: move |event: MouseEvent| {
                event.stop_propagation();
                on_open.call(line);
            },
            "+"
        }
    }
}

/// The saved comments of a line and, when its `+` was clicked, an empty box, under the row.
fn comment_slot(line: Option<u32>, env: Option<&CommentEnv>) -> Element {
    let (Some(line), Some(env)) = (line, env) else { return rsx! {} };
    let saved = comments_on(&env.cards, &env.path, line);
    if saved.is_empty() && env.open != Some(line) {
        return rsx! {};
    }
    let (id, path, on_close) = (env.id, env.path.clone(), env.on_close);
    let open_here = env.open == Some(line);
    rsx! {
        div { class: "cd-comment-slot",
            for card in saved {
                DiffCommentBox { key: "c{card.id}", id, path: path.clone(), line, card: Some(card), on_done: move |_| {} }
            }
            if open_here {
                DiffCommentBox { key: "new{line}", id, path: path.clone(), line, card: None, on_done: on_close }
            }
        }
    }
}
```

Change `line_row` and `cell` and `hunk_view` to take `env: Option<&CommentEnv>` and use them. `line_row`:

```rust
fn line_row(line: &PatchLine, spans: &[Span], highlight: bool, env: Option<&CommentEnv>) -> Element {
    let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
    let target = env.and_then(|_| offers_comment(line, false, false));
    rsx! {
        div { class: "diff-line {line_class(line.kind)}",
            {add_button(target, env)}
            span { class: "cd-no", "{number(line.old_no)}" }
            span { class: "cd-no", "{number(line.new_no)}" }
            span { class: "diff-sign", "{sign(line.kind)}" }
            span { class: "diff-text",
                for (piece , hot) in pieces {
                    span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                }
            }
        }
        {comment_slot(target, env)}
    }
}
```

`cell` takes `env` too; inside the `Some(line)` arm compute `let target = env.and_then(|_| offers_comment(line, right, true));` and render `{add_button(target, env)}` as the first child of its `div.diff-line` and `{comment_slot(target, env)}` after that div (a sibling; `.cd-comment-slot` spans both grid columns). The `None` arm is unchanged. In `hunk_view(hunk, prefs, env)` pass `env` through both branches (`line_row(line, spans_of(index), prefs.highlight_words, env)` and the two `cell(..., env)` calls). The gap rows (`line_row(line, &[], false)`) become `line_row(line, &[], false, None)`: no comments on revealed gap lines.

In `FileSection` add the props `root: PathBuf, commentable: bool`, a signal `let mut open_comment = use_signal(|| None::<u32>);` and build the environment once per render before `body`:

```rust
    let env = commentable.then(|| CommentEnv {
        id,
        path: absolute_path(&root, &path),
        cards: workspace.cards(id),
        open: open_comment(),
        on_open: EventHandler::new(move |line| open_comment.set(Some(line))),
        on_close: EventHandler::new(move |_| open_comment.set(None)),
    });
```

and use `hunk_view(hunk, &prefs, env.as_ref())`. `DiffStack` gets `root: PathBuf` and passes to each section `root: root.clone(), commentable: commentable(&source)`.

In `changes_panel.rs`, where `DiffStack { ... }` is built add `root: menu_root(info().and_then(|read| read.ok()).and_then(|branches| branches.root).as_deref(), &cwd),` (read how `menu_view` there gets the root from `BranchInfo` and copy that expression exactly, adjusting only for the `Option` shape).

- [ ] **Step 4: CSS**

In `theme.css`, next to the other `.cd-lines` rules add:

```css
.cd-lines .diff-line { position: relative; }
.cd-add-comment { display: none; position: absolute; left: 4px; top: 50%; transform: translateY(-50%); width: 16px; height: 16px; padding: 0; border: none; border-radius: 50%; background: var(--accent); color: #fff; font-size: 13px; line-height: 16px; text-align: center; cursor: pointer; z-index: 1; }
.cd-lines .diff-line:hover .cd-add-comment { display: block; }
```

(The `.cd-comment-slot` rule from Task 2 gives the side by side slot its full grid row.)

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test -p forge-widgets -p forge-features -p forge-shared 2>&1 | grep "^test result"` and `cargo build -p forge-app 2>&1 | tail -4`
Expected: all PASS; clean build. Fix any signature fallout from `line_row`/`cell`/`hunk_view` (all call sites are in `changes_stack.rs`).

- [ ] **Step 6: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: comments on the lines of a diff in the Changes panel

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Check in the real window and record the stage

**Files:**
- Modify: `docs/superpowers/specs/2026-10-17-changes-panel-2-design.md` ("As built"), `docs/superpowers/roadmap/backlog.md` (PNL-02 row), `docs/superpowers/roadmap/observations.md` (only if something new was seen)
- Memory: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-50b-diff-comments.md` and its line in `MEMORY.md`

- [ ] **Step 1: Whole test run**

Run: `cargo test -p forge-domain -p forge-entities -p forge-ports -p forge-adapter-git -p forge-features -p forge-widgets -p forge-shared 2>&1 | grep "^test result"`
Expected: no failures.

- [ ] **Step 2: Real-window check**

Read `scripts/harness/README.md`, launch with `scripts/harness/launch.sh`, make the harness `my-app` folder a git repository (branch `claude/x`, two commits, uncommitted edits to `a.txt` and a few added files), open its Changes panel, and check (capture after every click; do NOT send chat messages; always run `scripts/harness/stop.sh`):

1. Hovering an added or context line shows a round accent `+` in the left margin; a removed line shows none; the `+` does not appear while the pointer is elsewhere.
2. Clicking it opens the box under that line (rows below move down): grey `N번째 줄`/`Line N` title, the placeholder `Claude에게 변경할 내용을 알려주세요`, a `⊕` at the bottom right, accent border, the caret in the field.
3. Typing text and pressing Enter saves: the box turns grey with the text, a pencil and a trash; the composer of the chat now shows a comment card for that file and line. Shift+Enter makes a new line. Escape on an empty new box closes it with nothing saved; a blank text with Enter saves nothing. (Korean composition Enter cannot be driven by the harness: say so.)
4. The pencil edits in place; Enter saves the new text (the composer card updates); Escape returns to the saved text. The trash removes the box and the composer card.
5. Open the same file in the Files panel: the comment is at the same line there; add one there and see it in the diff.
6. Side by side (`⋮` > Side by side): the `+` shows only in the right half; the box spans the full width under the row.
7. Choose a commit in the branch menu (a `Commit` view): no `+` anywhere.
8. Wrap off (`⋮` > Wrap lines): the box still fits; no horizontal jump.
9. Dark mode is left for the 50C check (the user asked for it then); note anything you see.

Record anything that differs from the desktop app (for example where the app sends the comment).

- [ ] **Step 3: Record the stage**

Add `### Stage 50B (built on branch plan-50)` to the spec's "As built": what was built, what differs from the app (the submit icon, Enter-to-save, the card going to the composer are Forge's own; the app's behaviour after submit was not seen), the known limit (a reload that changes a file's diff remounts the section and drops an unsent draft), what was checked in the window and what was not. Update the PNL-02 row in `backlog.md` (`Plan 50B (built)`; remove `inline comments on diff lines` from its "Still not built" list). Write the memory note and its `MEMORY.md` line (project type, merge state, the Cards sharing and the `Commit`-view rule).

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 50B as built, checked in the real window

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
