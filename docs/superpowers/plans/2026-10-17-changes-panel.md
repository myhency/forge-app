# Changes Panel Parity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Changes panel shows every changed file as a stacked, foldable diff, lets the person compare all changes against a base branch, uncommitted changes, or one commit, and offers a folder tree, viewing options, file search and a file row menu, as the Claude desktop app does.

**Architecture:** `GitPort` gets four read-only methods (`branches`, `commits`, `changes`, `file_lines`, plus `file_patch`) built on one `git diff` call per mode; a new `patch` module in `forge-entities` parses a unified diff with line numbers, word spans and side-by-side pairing; `forge-features` holds the pure logic (tree order, view state, search filter) and the Workspace state; `forge-widgets` splits `changes_panel.rs` into header, stack and tree widgets.

**Tech Stack:** Rust 2021, Dioxus 0.7 desktop, `similar` (already a dependency of `forge-entities`), the `git` CLI, `tempfile` for adapter tests.

**Spec:** `docs/superpowers/specs/2026-10-17-changes-panel-design.md` (Plan 48). Observations: `docs/superpowers/roadmap/observations.md`, rounds 19, 19b, 19c.

## Global Constraints

- `GitPort` never changes a repository: no `add`, `commit`, `checkout`, `reset`, `stash`. (spec "Entities and ports")
- Every git command runs through `GitCli::run` (10 s timeout, `GIT_LITERAL_PATHSPECS=1`, `LC_ALL=C`); a branch name or sha that starts with `-` is refused before it reaches git. (spec "Errors and edges", 21B rules)
- Total patch text of one `changes` call is capped at 2 MB (`PATCH_BUDGET = 2 * 1024 * 1024`); a file over the remaining budget is `PatchState::TooLarge`. (spec)
- At most 500 file rows are drawn in the right column; at most the first 50 stack sections start unfolded. (spec "Widgets")
- Viewing options live in `AppConfig.changes_view` with defaults `group_by_folder: true, side_by_side: false, wrap: true, highlight_words: true, hide_whitespace: false`. (spec "Viewing options")
- The chat's `FileDiff`, `parse_unified_diff` and `DiffView` stay as they are; the panel uses the new `Patch*` types. (decision made while planning: less blast radius than changing `FileDiff`)
- git is run with `--no-renames`: a renamed file shows as a deleted and an added file. (decision made while planning; the 21B code did the same; the spec's rename line is amended in Task 14)
- Every user-visible string goes through `tr!` and has its Korean text in `crates/forge-shared/src/ko.rs`, kept sorted by English text in byte order; `cargo test -p forge-shared` checks both ways.
- The panel is a viewer: no stage, unstage, revert or commit button (Plan 49 adds the commit card).
- Rust style: match the surrounding code (comments explain why, short; no new crates).

## Review Focus

- A base branch that does not exist (renamed, deleted, or a worktree record that is stale): the panel falls back to Uncommitted with a one-line notice, never an error wall. Test: `a_missing_base_is_reported_as_base_not_found` (Task 5) and the fallback in Task 9.
- File names with spaces, Korean letters and a leading `-`: listed, diffed and matched to their patch. Test: `names_with_spaces_and_korean_letters_get_their_patch` (Task 5).
- A diff over the 2 MB budget or a binary file: the list still opens at once; the file shows a note or a `Load diff` row. Tests: `a_patch_over_the_budget_is_too_large_until_asked_for` and `an_untracked_binary_file_is_binary` (Task 5).
- A repository with no commit, and a detached HEAD: no crash, sensible labels. Tests: `a_repository_with_no_commit_lists_its_files` (Task 5), `a_detached_head_is_labelled_with_its_sha` (Task 5).
- Whitespace-only changes hidden with `-w`: the file leaves the list instead of showing an empty section. Test: `hiding_whitespace_drops_a_file_that_only_changed_in_whitespace` (Task 5).
- Lines of a gap that does not exist (the file ends before the gap) or a hunk of a deleted file: no panic, no empty row. Tests: `the_last_hunk_has_an_open_ended_gap` and `a_deleted_file_has_no_gap` (Task 2).
- A very long file list: only 500 rows and 50 open sections. Test: `sections_after_the_first_fifty_start_folded` (Task 6).

---

### Task 0: Branch and the documents

**Files:**
- Modify: `docs/superpowers/roadmap/observations.md`, `docs/superpowers/roadmap/backlog.md` (already edited, uncommitted)
- Create: `docs/superpowers/specs/2026-10-17-changes-panel-design.md`, this plan (already written)

- [ ] **Step 1: Make the branch and commit the documents**

```bash
git checkout -b plan-48
git add docs/superpowers/roadmap docs/superpowers/specs/2026-10-17-changes-panel-design.md docs/superpowers/plans/2026-10-17-changes-panel.md
git commit -m "docs: Plan 48 spec and plan, observation rounds 19 to 19c (PNL-02)"
```

---

### Task 1: The viewing options in the config

**Files:**
- Modify: `crates/forge-domain/src/config.rs` (after `EditorPrefs` impl, the `AppConfig` struct, its `Default`, tests)
- Modify: `crates/forge-domain/src/lib.rs` (the `pub use config::{...}` list)

**Interfaces:**
- Produces: `forge_domain::ChangesViewPrefs { group_by_folder: bool, side_by_side: bool, wrap: bool, highlight_words: bool, hide_whitespace: bool }` (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`, `Default` as in Global Constraints) and `AppConfig.changes_view: ChangesViewPrefs`.

- [ ] **Step 1: Write the failing test** in the `tests` module of `config.rs`

```rust
    #[test]
    fn changes_view_defaults_and_round_trips() {
        let config: AppConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(
            config.changes_view,
            ChangesViewPrefs { group_by_folder: true, side_by_side: false, wrap: true, highlight_words: true, hide_whitespace: false }
        );
        let prefs = ChangesViewPrefs { side_by_side: true, hide_whitespace: true, ..ChangesViewPrefs::default() };
        let config = AppConfig { changes_view: prefs.clone(), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.changes_view, prefs);
    }

    #[test]
    fn a_partial_changes_view_keeps_the_defaults_for_the_rest() {
        let config: AppConfig = serde_json::from_str(r#"{"changes_view":{"wrap":false}}"#).unwrap();
        assert!(!config.changes_view.wrap);
        assert!(config.changes_view.group_by_folder);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p forge-domain changes_view`
Expected: FAIL to compile (`ChangesViewPrefs` not found).

- [ ] **Step 3: Implement.** After the `impl EditorPrefs` block add:

```rust
/// How the Changes panel draws a diff (its `⋮` menu).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangesViewPrefs {
    /// Whether the right column shows folders (else a flat list).
    pub group_by_folder: bool,
    pub side_by_side: bool,
    pub wrap: bool,
    /// Whether the changed words of a changed line are marked.
    pub highlight_words: bool,
    /// Whether git leaves out whitespace-only changes (`-w`).
    pub hide_whitespace: bool,
}

impl Default for ChangesViewPrefs {
    fn default() -> Self {
        Self { group_by_folder: true, side_by_side: false, wrap: true, highlight_words: true, hide_whitespace: false }
    }
}
```

In `AppConfig`, after `pub editor: EditorPrefs,` (keep its attribute style; look at the lines above `editor` for `#[serde(default)]`) add:

```rust
    #[serde(default)]
    pub changes_view: ChangesViewPrefs,
```

In `impl Default for AppConfig` after `editor: EditorPrefs::default(),` add `changes_view: ChangesViewPrefs::default(),`. In `crates/forge-domain/src/lib.rs` add `ChangesViewPrefs` to the `pub use config::{ ... }` list.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-domain`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: Changes panel viewing options in the config"
```

---

### Task 2: Patch parsing, gaps, side by side and word spans (forge-entities)

**Files:**
- Create: `crates/forge-entities/src/patch.rs`
- Modify: `crates/forge-entities/src/lib.rs` (add `mod patch;` and the `pub use`)

**Interfaces:**
- Consumes: `crate::diff::DiffLineKind` (`Context | Added | Removed`).
- Produces (all `pub`, re-exported from `forge_entities`):
  - `PatchLine { kind: DiffLineKind, text: String, old_no: Option<u32>, new_no: Option<u32> }`
  - `PatchHunk { old_start: u32, new_start: u32, lines: Vec<PatchLine> }` with `old_len()`, `new_len()`, `old_end()`, `new_end()` → `u32`
  - `parse_patch(text: &str) -> Vec<PatchHunk>`
  - `gap_after(hunks: &[PatchHunk], index: usize) -> Option<(u32, u32)>` (inclusive new-side range; `u32::MAX` end for the last hunk)
  - `gap_lines(hunks: &[PatchHunk], index: usize, file: &[String]) -> Vec<PatchLine>`
  - `side_by_side(lines: &[PatchLine]) -> Vec<(Option<usize>, Option<usize>)>` (indexes into `lines`)
  - `Span { start: usize, end: usize }`, `word_spans(old, new) -> (Vec<Span>, Vec<Span>)`, `paired_spans(lines: &[PatchLine]) -> Vec<Vec<Span>>`, `split_by_spans(text: &str, spans: &[Span]) -> Vec<(String, bool)>`

- [ ] **Step 1: Write the failing tests.** Create `crates/forge-entities/src/patch.rs` with only the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffLineKind::{Added, Context, Removed};

    const PATCH: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -2,3 +2,4 @@ fn x\n b\n-c\n+C\n+d\n e\n@@ -20,2 +21,2 @@\n y\n-z\n+Z\n";

    fn kinds(hunk: &PatchHunk) -> Vec<(DiffLineKind, Option<u32>, Option<u32>)> {
        hunk.lines.iter().map(|line| (line.kind, line.old_no, line.new_no)).collect()
    }

    #[test]
    fn hunks_keep_their_start_lines_and_number_every_line() {
        let hunks = parse_patch(PATCH);
        assert_eq!(hunks.len(), 2);
        assert_eq!((hunks[0].old_start, hunks[0].new_start), (2, 2));
        assert_eq!(
            kinds(&hunks[0]),
            vec![
                (Context, Some(2), Some(2)),
                (Removed, Some(3), None),
                (Added, None, Some(3)),
                (Added, None, Some(4)),
                (Context, Some(4), Some(5)),
            ]
        );
        assert_eq!((hunks[1].old_start, hunks[1].new_start), (20, 21));
        assert_eq!(hunks[1].lines[0].new_no, Some(21));
    }

    #[test]
    fn a_new_file_starts_at_line_one_with_no_old_side() {
        let hunks = parse_patch("@@ -0,0 +1,2 @@\n+one\n+two\n");
        assert_eq!(hunks[0].lines.iter().map(|l| (l.old_no, l.new_no)).collect::<Vec<_>>(), vec![(None, Some(1)), (None, Some(2))]);
    }

    #[test]
    fn the_header_lines_before_the_first_hunk_and_a_missing_newline_note_are_not_lines() {
        let hunks = parse_patch("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+b\n\\ No newline at end of file\n");
        assert_eq!(hunks[0].lines.len(), 2);
    }

    #[test]
    fn a_removed_line_that_starts_with_dashes_is_still_a_removed_line() {
        let hunks = parse_patch("@@ -1,2 +1 @@\n--- not a header\n x\n");
        assert_eq!(hunks[0].lines[0].kind, Removed);
        assert_eq!(hunks[0].lines[0].text, "-- not a header");
    }

    #[test]
    fn a_gap_lies_between_two_hunks() {
        let hunks = parse_patch(PATCH);
        // hunk 0 ends at new line 5; hunk 1 starts at 21.
        assert_eq!(gap_after(&hunks, 0), Some((6, 20)));
    }

    #[test]
    fn the_last_hunk_has_an_open_ended_gap() {
        let hunks = parse_patch(PATCH);
        assert_eq!(gap_after(&hunks, 1), Some((23, u32::MAX)));
    }

    #[test]
    fn two_touching_hunks_have_no_gap() {
        let hunks = parse_patch("@@ -1,1 +1,1 @@\n-a\n+b\n@@ -2,1 +2,1 @@\n-c\n+d\n");
        assert_eq!(gap_after(&hunks, 0), None);
    }

    #[test]
    fn a_deleted_file_has_no_gap() {
        let hunks = parse_patch("@@ -1,2 +0,0 @@\n-a\n-b\n");
        assert_eq!(gap_after(&hunks, 0), None);
    }

    #[test]
    fn gap_lines_are_context_numbered_on_both_sides() {
        let hunks = parse_patch(PATCH);
        let file: Vec<String> = (1..=30).map(|n| format!("line {n}")).collect();
        let lines = gap_lines(&hunks, 0, &file);
        assert_eq!(lines.len(), 15);
        assert_eq!(lines[0].text, "line 6");
        // Hunk 0 added one line on net (+4 -3), so old numbers are one less than new ones.
        assert_eq!((lines[0].new_no, lines[0].old_no), (Some(6), Some(5)));
        assert!(lines.iter().all(|line| line.kind == Context));
    }

    #[test]
    fn the_open_ended_gap_stops_at_the_end_of_the_file() {
        let hunks = parse_patch(PATCH);
        let file: Vec<String> = (1..=24).map(|n| format!("line {n}")).collect();
        let lines = gap_lines(&hunks, 1, &file);
        assert_eq!(lines.iter().map(|l| l.new_no.unwrap()).collect::<Vec<_>>(), vec![23, 24]);
        // A file that ends inside the hunk has no more lines.
        let short: Vec<String> = (1..=22).map(|n| format!("line {n}")).collect();
        assert!(gap_lines(&hunks, 1, &short).is_empty());
    }

    fn line(kind: DiffLineKind, text: &str) -> PatchLine {
        PatchLine { kind, text: text.to_string(), old_no: None, new_no: None }
    }

    #[test]
    fn side_by_side_pairs_a_removed_run_with_the_added_run() {
        let lines = vec![line(Context, "a"), line(Removed, "b"), line(Removed, "c"), line(Added, "B"), line(Context, "d")];
        assert_eq!(side_by_side(&lines), vec![(Some(0), Some(0)), (Some(1), Some(3)), (Some(2), None), (Some(4), Some(4))]);
    }

    #[test]
    fn side_by_side_puts_a_lone_addition_on_the_right() {
        let lines = vec![line(Added, "x"), line(Added, "y")];
        assert_eq!(side_by_side(&lines), vec![(None, Some(0)), (None, Some(1))]);
    }

    #[test]
    fn word_spans_mark_only_the_words_that_differ() {
        let (old, new) = word_spans("let a = 1;", "let b = 1;");
        assert_eq!(old.iter().map(|s| &"let a = 1;"[s.start..s.end]).collect::<Vec<_>>(), vec!["a"]);
        assert_eq!(new.iter().map(|s| &"let b = 1;"[s.start..s.end]).collect::<Vec<_>>(), vec!["b"]);
    }

    #[test]
    fn word_spans_work_on_korean_text() {
        let (old, new) = word_spans("안녕 세상", "안녕 우주");
        assert_eq!(old.iter().map(|s| &"안녕 세상"[s.start..s.end]).collect::<Vec<_>>(), vec!["세상"]);
        assert_eq!(new.iter().map(|s| &"안녕 우주"[s.start..s.end]).collect::<Vec<_>>(), vec!["우주"]);
    }

    #[test]
    fn only_paired_lines_get_spans() {
        let lines = vec![line(Removed, "a x"), line(Removed, "only removed"), line(Added, "a y"), line(Context, "same")];
        let spans = paired_spans(&lines);
        assert!(!spans[0].is_empty());
        assert!(spans[1].is_empty(), "the second removed line has no added partner");
        assert!(!spans[2].is_empty());
        assert!(spans[3].is_empty());
    }

    #[test]
    fn split_by_spans_cuts_the_text_at_the_spans() {
        let pieces = split_by_spans("let a = 1;", &[Span { start: 4, end: 5 }]);
        assert_eq!(pieces, vec![("let ".to_string(), false), ("a".to_string(), true), (" = 1;".to_string(), false)]);
        assert_eq!(split_by_spans("abc", &[]), vec![("abc".to_string(), false)]);
    }
}
```

- [ ] **Step 2: Wire the module so the tests compile and fail.** In `crates/forge-entities/src/lib.rs` add `mod patch;` next to `mod diff;` and, next to the `pub use diff::...` line:

```rust
pub use patch::{gap_after, gap_lines, paired_spans, parse_patch, side_by_side, split_by_spans, word_spans, PatchHunk, PatchLine, Span};
```

Run: `cargo test -p forge-entities patch`
Expected: FAIL to compile (functions not defined).

- [ ] **Step 3: Implement.** Put this above the test module in `patch.rs`:

```rust
use similar::{ChangeTag, TextDiff};

use crate::diff::DiffLineKind;

/// One line of a git patch with its line numbers on the old and the new side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchLine {
    pub kind: DiffLineKind,
    pub text: String,
    pub old_no: Option<u32>,
    pub new_no: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchHunk {
    pub old_start: u32,
    pub new_start: u32,
    pub lines: Vec<PatchLine>,
}

impl PatchHunk {
    pub fn old_len(&self) -> u32 {
        self.lines.iter().filter(|line| line.kind != DiffLineKind::Added).count() as u32
    }

    pub fn new_len(&self) -> u32 {
        self.lines.iter().filter(|line| line.kind != DiffLineKind::Removed).count() as u32
    }

    /// The last old-side line the hunk covers; for a hunk with no old lines, the line before it.
    pub fn old_end(&self) -> u32 {
        self.old_start + self.old_len().saturating_sub(1)
    }

    pub fn new_end(&self) -> u32 {
        self.new_start + self.new_len().saturating_sub(1)
    }
}

// `@@ -a[,b] +c[,d] @@ ...`: the two start lines.
fn parse_header(line: &str) -> Option<(u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (old, rest) = rest.split_once(' ')?;
    let new = rest.strip_prefix('+')?.split(' ').next()?;
    let start = |text: &str| text.split(',').next()?.parse::<u32>().ok();
    Some((start(old)?, start(new)?))
}

/// The hunks of the patch of ONE file (git's `diff --git` header lines before the first `@@` are
/// skipped). A binary file or empty text gives no hunks.
pub fn parse_patch(text: &str) -> Vec<PatchHunk> {
    let mut hunks: Vec<PatchHunk> = Vec::new();
    let (mut old_no, mut new_no) = (0u32, 0u32);
    for line in text.lines() {
        if line.starts_with("@@") {
            let Some((old_start, new_start)) = parse_header(line) else { continue };
            (old_no, new_no) = (old_start, new_start);
            hunks.push(PatchHunk { old_start, new_start, lines: Vec::new() });
            continue;
        }
        let Some(hunk) = hunks.last_mut() else { continue };
        let clean = |rest: &str| rest.trim_end_matches('\r').to_string();
        let parsed = match line.chars().next() {
            Some('+') => {
                let parsed = PatchLine { kind: DiffLineKind::Added, text: clean(&line[1..]), old_no: None, new_no: Some(new_no) };
                new_no += 1;
                parsed
            }
            Some('-') => {
                let parsed = PatchLine { kind: DiffLineKind::Removed, text: clean(&line[1..]), old_no: Some(old_no), new_no: None };
                old_no += 1;
                parsed
            }
            Some(' ') | None => {
                let rest = if line.is_empty() { "" } else { &line[1..] };
                let parsed = PatchLine { kind: DiffLineKind::Context, text: clean(rest), old_no: Some(old_no), new_no: Some(new_no) };
                old_no += 1;
                new_no += 1;
                parsed
            }
            // "\ No newline at end of file" and anything else.
            Some(_) => continue,
        };
        hunk.lines.push(parsed);
    }
    hunks
}

/// The new-side lines (inclusive) after hunk `index` and before the next hunk; for the last hunk
/// up to the end of the file (`u32::MAX`, the file decides). `None` when nothing lies between, and
/// for a file that was removed.
pub fn gap_after(hunks: &[PatchHunk], index: usize) -> Option<(u32, u32)> {
    let hunk = hunks.get(index)?;
    if hunk.new_start == 0 {
        return None;
    }
    let from = hunk.new_end() + 1;
    match hunks.get(index + 1) {
        Some(next) => {
            let to = next.new_start.saturating_sub(1);
            (from <= to).then_some((from, to))
        }
        None => Some((from, u32::MAX)),
    }
}

/// The lines of that gap read from the file, as context lines with both numbers.
pub fn gap_lines(hunks: &[PatchHunk], index: usize, file: &[String]) -> Vec<PatchLine> {
    let Some((from, to)) = gap_after(hunks, index) else { return Vec::new() };
    let hunk = &hunks[index];
    // new = old + delta for every line after the hunk.
    let delta = i64::from(hunk.new_end()) - i64::from(hunk.old_end());
    let last = (file.len() as u32).min(to);
    (from..=last)
        .filter(|number| *number >= 1)
        .map(|number| PatchLine {
            kind: DiffLineKind::Context,
            text: file[(number - 1) as usize].clone(),
            old_no: Some((i64::from(number) - delta) as u32),
            new_no: Some(number),
        })
        .collect()
}

/// Rows of two columns: a context line on both sides; a run of removed lines beside the run of
/// added lines that follows it. Indexes into `lines`.
pub fn side_by_side(lines: &[PatchLine]) -> Vec<(Option<usize>, Option<usize>)> {
    let mut rows = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if lines[at].kind == DiffLineKind::Context {
            rows.push((Some(at), Some(at)));
            at += 1;
            continue;
        }
        let removed_from = at;
        while at < lines.len() && lines[at].kind == DiffLineKind::Removed {
            at += 1;
        }
        let added_from = at;
        while at < lines.len() && lines[at].kind == DiffLineKind::Added {
            at += 1;
        }
        let (removed, added) = (added_from - removed_from, at - added_from);
        for step in 0..removed.max(added) {
            rows.push(((step < removed).then_some(removed_from + step), (step < added).then_some(added_from + step)));
        }
    }
    rows
}

/// A byte range of a line text that changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

fn push_span(spans: &mut Vec<Span>, start: usize, end: usize) {
    if let Some(last) = spans.last_mut() {
        if last.end == start {
            last.end = end;
            return;
        }
    }
    spans.push(Span { start, end });
}

/// The words that differ between an old and a new line: spans of the old text and of the new text.
pub fn word_spans(old: &str, new: &str) -> (Vec<Span>, Vec<Span>) {
    let (mut old_at, mut new_at) = (0usize, 0usize);
    let (mut old_spans, mut new_spans) = (Vec::new(), Vec::new());
    for change in TextDiff::from_words(old, new).iter_all_changes() {
        let length = change.value().len();
        match change.tag() {
            ChangeTag::Equal => {
                old_at += length;
                new_at += length;
            }
            ChangeTag::Delete => {
                push_span(&mut old_spans, old_at, old_at + length);
                old_at += length;
            }
            ChangeTag::Insert => {
                push_span(&mut new_spans, new_at, new_at + length);
                new_at += length;
            }
        }
    }
    (old_spans, new_spans)
}

/// For each line the changed words: a removed line is compared with the added line at the same
/// place of the same change block; every other line has none.
pub fn paired_spans(lines: &[PatchLine]) -> Vec<Vec<Span>> {
    let mut out = vec![Vec::new(); lines.len()];
    let mut at = 0;
    while at < lines.len() {
        if lines[at].kind != DiffLineKind::Removed {
            at += 1;
            continue;
        }
        let removed_from = at;
        while at < lines.len() && lines[at].kind == DiffLineKind::Removed {
            at += 1;
        }
        let added_from = at;
        while at < lines.len() && lines[at].kind == DiffLineKind::Added {
            at += 1;
        }
        for step in 0..(added_from - removed_from).min(at - added_from) {
            let (old, new) = word_spans(&lines[removed_from + step].text, &lines[added_from + step].text);
            out[removed_from + step] = old;
            out[added_from + step] = new;
        }
    }
    out
}

/// The text cut at the span edges: each piece with whether it is inside a span.
pub fn split_by_spans(text: &str, spans: &[Span]) -> Vec<(String, bool)> {
    let mut pieces = Vec::new();
    let mut at = 0;
    for span in spans {
        let (Some(before), Some(inside)) = (text.get(at..span.start), text.get(span.start..span.end)) else { continue };
        if !before.is_empty() {
            pieces.push((before.to_string(), false));
        }
        pieces.push((inside.to_string(), true));
        at = span.end;
    }
    if let Some(rest) = text.get(at..).filter(|rest| !rest.is_empty()) {
        pieces.push((rest.to_string(), false));
    }
    pieces
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-entities`
Expected: PASS (the new tests and the existing diff tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: patch parser with line numbers, gaps, side by side rows and word spans"
```

---

### Task 3: The new port types

**Files:**
- Modify: `crates/forge-ports/src/git_port.rs`
- Modify: `crates/forge-ports/src/lib.rs:32` (the `pub use git_port::{...}` line)

**Interfaces:**
- Produces (re-exported from `forge_ports`): `ChangeSource`, `DiffOptions`, `BranchInfo`, `CommitInfo`, `PatchState`, `FileChange`, `GitError::BaseNotFound(String)`, and on `GitPort`: `branches`, `commits`, `changes`, `file_lines`, `file_patch`. The old `status` and `diff` stay until Task 13.

- [ ] **Step 1: Write the failing test** in the `tests` module of `git_port.rs`

```rust
    #[test]
    fn the_new_error_names_the_base() {
        assert_eq!(GitError::BaseNotFound("dev".to_string()).to_string(), "Base branch \"dev\" was not found");
    }
```

- [ ] **Step 2: Run it**

Run: `cargo test -p forge-ports base`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** In `git_port.rs`, change `GitError` and add the types after `ChangedFile`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GitError {
    #[error("Not a git repository")]
    NotARepository,
    /// The branch or commit asked for as a base does not exist.
    #[error("Base branch \"{0}\" was not found")]
    BaseNotFound(String),
    #[error("{0}")]
    Failed(String),
}

/// What a change list compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeSource {
    /// Where the branch left `base` (the merge base) against the working tree, untracked files included.
    All { base: String },
    /// `HEAD` against the working tree, untracked files included.
    Uncommitted,
    /// One commit against its parent (a root commit against the empty tree).
    Commit { sha: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// `git diff -w`: a file whose only change is whitespace is left out.
    pub ignore_whitespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchInfo {
    /// The checked-out branch, `None` while HEAD is detached.
    pub current: Option<String>,
    /// The branch name, or the short sha of a detached HEAD.
    pub head_label: String,
    /// `origin/HEAD`'s branch, else `main`, else `master`, if there is one.
    pub default_base: Option<String>,
    pub branches: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub sha: String,
    pub subject: String,
    pub author: String,
    /// Seconds since the epoch.
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchState {
    /// The unified diff of the file, from its `diff --git` line on; empty for a mode-only change.
    Text(String),
    Binary,
    /// Over the budget of one `changes` call; `file_patch` reads it alone.
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file: ChangedFile,
    pub patch: PatchState,
}
```

Update the trait doc and add the methods (keep `status` and `diff`):

```rust
/// Reads the state of a git working tree. It never changes it.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait GitPort: Send + Sync {
    /// ... (existing `status` and `diff` stay as they are)

    /// The local branches, the checked-out one and the default base of the repository holding `cwd`.
    fn branches(&self, cwd: &Path) -> Result<BranchInfo, GitError>;
    /// The commits of `base..HEAD`, newest first, at most 200. Empty when there is no commit yet.
    /// `BaseNotFound` when `base` does not exist.
    fn commits(&self, cwd: &Path, base: &str) -> Result<Vec<CommitInfo>, GitError>;
    /// The changed files of `source` with their patches, in path order. `BaseNotFound` for an `All`
    /// whose base does not exist.
    fn changes(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions) -> Result<Vec<FileChange>, GitError>;
    /// The lines of a file on the new side of `source` (the working tree, or the commit's blob).
    fn file_lines(&self, cwd: &Path, source: &ChangeSource, path: &str) -> Result<Vec<String>, GitError>;
    /// The patch of one file with no size budget, for a file `changes` called `TooLarge`.
    fn file_patch(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, path: &str) -> Result<String, GitError>;
}
```

In `lib.rs` change line 32 to:

```rust
pub use git_port::{BranchInfo, ChangeSource, ChangeStatus, ChangedFile, CommitInfo, DiffOptions, FileChange, GitError, GitPort, PatchState};
```

- [ ] **Step 4: Build.** The adapter does not compile yet (trait methods missing); that is expected.

Run: `cargo test -p forge-ports`
Expected: PASS (`forge-ports` itself builds and its tests pass; `forge-adapter-git` is fixed in the next tasks).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: git port types for change sources, branches, commits and patches"
```

---

### Task 4: Patch and status parsing in the git adapter

**Files:**
- Create: `crates/forge-adapter-git/src/changes.rs`
- Modify: `crates/forge-adapter-git/src/lib.rs` (add `mod changes;` after the `use` block)

**Interfaces:**
- Consumes: `forge_ports::{ChangeStatus, PatchState}`.
- Produces (crate-private): `EMPTY_TREE: &str`, `PATCH_BUDGET: usize`, `Count { Lines(usize, usize), Binary }`, `parse_name_status(&[u8]) -> Vec<(ChangeStatus, String)>`, `parse_counts(&[u8]) -> HashMap<String, Count>`, `split_patch(&str) -> HashMap<String, String>`, `untracked_patch(&[u8], budget: usize) -> (PatchState, usize)`, `safe_ref(&str) -> bool`.

- [ ] **Step 1: Write the failing tests.** Create `changes.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_status_reads_a_letter_and_a_path_per_entry() {
        let files = parse_name_status(b"M\0a.txt\0A\0dir/b c.txt\0D\0gone.txt\0");
        assert_eq!(
            files,
            vec![
                (ChangeStatus::Modified, "a.txt".to_string()),
                (ChangeStatus::Added, "dir/b c.txt".to_string()),
                (ChangeStatus::Deleted, "gone.txt".to_string()),
            ]
        );
    }

    #[test]
    fn counts_mark_a_binary_file() {
        let counts = parse_counts(b"3\t1\ta.txt\0-\t-\timg.png\0");
        assert!(matches!(counts["a.txt"], Count::Lines(3, 1)));
        assert!(matches!(counts["img.png"], Count::Binary));
    }

    #[test]
    fn a_patch_is_split_by_its_diff_header_even_when_the_name_has_spaces() {
        let text = "diff --git a/a b.txt b/a b.txt\n--- a/a b.txt\n+++ b/a b.txt\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/한글.txt b/한글.txt\n--- a/한글.txt\n+++ b/한글.txt\n@@ -1 +1 @@\n-1\n+2\n";
        let parts = split_patch(text);
        assert_eq!(parts.len(), 2);
        assert!(parts["a b.txt"].starts_with("diff --git a/a b.txt b/a b.txt\n"));
        assert!(parts["a b.txt"].ends_with("+y\n"));
        assert!(parts["한글.txt"].contains("+2"));
    }

    #[test]
    fn a_body_line_that_looks_like_a_header_is_not_a_split_point() {
        let text = "diff --git a/x b/x\n@@ -1 +1 @@\n- diff --git a/y b/y\n+z\n";
        assert_eq!(split_patch(text).len(), 1);
    }

    #[test]
    fn an_untracked_text_file_is_all_added() {
        let (patch, added) = untracked_patch(b"one\ntwo\n", 1000);
        assert_eq!(added, 2);
        assert_eq!(patch, PatchState::Text("@@ -0,0 +1,2 @@\n+one\n+two\n".to_string()));
    }

    #[test]
    fn an_untracked_file_with_a_nul_byte_is_binary() {
        assert_eq!(untracked_patch(b"ab\0cd", 1000).0, PatchState::Binary);
    }

    #[test]
    fn an_untracked_file_over_the_budget_is_too_large() {
        assert_eq!(untracked_patch(b"aaaa\nbbbb\n", 5).0, PatchState::TooLarge);
    }

    #[test]
    fn a_name_that_looks_like_an_option_is_not_a_ref() {
        assert!(safe_ref("main"));
        assert!(!safe_ref("-x"));
        assert!(!safe_ref(""));
    }
}
```

- [ ] **Step 2: Wire the module and run to see failure.** In `lib.rs` add `mod changes;` below the `use` lines.

Run: `cargo test -p forge-adapter-git changes::` 
Expected: FAIL to compile (the trait impl is also incomplete: ignore that error; the target is the missing functions).

- [ ] **Step 3: Implement.** Above the tests in `changes.rs`:

```rust
use std::collections::HashMap;

use forge_ports::{ChangeStatus, PatchState};

/// git's empty tree: what a root commit, or a repository with no commit, is compared with.
pub(crate) const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// One `changes` call keeps at most this much patch text.
pub(crate) const PATCH_BUDGET: usize = 2 * 1024 * 1024;

pub(crate) enum Count {
    Lines(usize, usize),
    Binary,
}

/// A branch or commit name goes to git without `--`, so one that looks like an option is refused.
pub(crate) fn safe_ref(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('-')
}

// `git diff --name-status -z --no-renames`: `S NUL path NUL` per file.
pub(crate) fn parse_name_status(bytes: &[u8]) -> Vec<(ChangeStatus, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut parts = text.split('\0');
    let mut files = Vec::new();
    while let Some(code) = parts.next() {
        if code.is_empty() {
            continue;
        }
        let Some(path) = parts.next() else { break };
        let status = match code.as_bytes()[0] {
            b'A' => ChangeStatus::Added,
            b'D' => ChangeStatus::Deleted,
            b'U' => ChangeStatus::Conflicted,
            _ => ChangeStatus::Modified,
        };
        files.push((status, path.to_string()));
    }
    files
}

// `git diff --numstat -z --no-renames`: `added TAB removed TAB path NUL`; `-` for a binary file.
pub(crate) fn parse_counts(bytes: &[u8]) -> HashMap<String, Count> {
    let text = String::from_utf8_lossy(bytes);
    let mut counts = HashMap::new();
    for entry in text.split('\0') {
        let mut fields = entry.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (fields.next(), fields.next(), fields.next()) else { continue };
        match (added.parse(), removed.parse()) {
            (Ok(added), Ok(removed)) => counts.insert(path.to_string(), Count::Lines(added, removed)),
            _ if added == "-" => counts.insert(path.to_string(), Count::Binary),
            _ => None,
        };
    }
    counts
}

// `P b/P` of a `diff --git a/P b/P` line; the diff is made with --no-renames so both names agree.
fn header_path(rest: &str) -> Option<String> {
    let names = rest.len().checked_sub(3)?;
    if names % 2 != 0 {
        return None;
    }
    let path = rest.get(..names / 2)?;
    (rest.get(names / 2..)? == format!(" b/{path}")).then(|| path.to_string())
}

/// The patch of each file, by path. A chunk whose header cannot be read (a name git had to quote)
/// is dropped; the caller then treats the file as having no text patch.
pub(crate) fn split_patch(text: &str) -> HashMap<String, String> {
    let mut parts = HashMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        // A body line starts with space, `+`, `-` or `\`, so this only ever matches a header.
        if let Some(rest) = line.strip_prefix("diff --git a/") {
            if let Some((path, chunk)) = current.take() {
                parts.insert(path, chunk);
            }
            current = header_path(rest.trim_end_matches('\n')).map(|path| (path, String::new()));
        }
        if let Some((_, chunk)) = current.as_mut() {
            chunk.push_str(line);
        }
    }
    if let Some((path, chunk)) = current {
        parts.insert(path, chunk);
    }
    parts
}

/// What `git diff --no-index /dev/null file` would show, made here so a thousand untracked files
/// do not start a thousand processes. Returns the state and the number of added lines.
pub(crate) fn untracked_patch(bytes: &[u8], budget: usize) -> (PatchState, usize) {
    if bytes.iter().take(8000).any(|byte| *byte == 0) {
        return (PatchState::Binary, 0);
    }
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return (PatchState::Text(String::new()), 0);
    }
    let mut patch = format!("@@ -0,0 +1,{} @@\n", lines.len());
    for line in &lines {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    if patch.len() > budget {
        return (PatchState::TooLarge, lines.len());
    }
    (PatchState::Text(patch), lines.len())
}
```

- [ ] **Step 4: Run the tests** (the crate still lacks the trait impl, so test with the module compiled alone is not possible; do Task 5 Step 1-3 before running). Skip running here.

- [ ] **Step 5: Commit after Task 5 passes** (Tasks 4 and 5 are one commit: the crate must compile).

---

### Task 5: `GitCli` implements the new methods

**Files:**
- Modify: `crates/forge-adapter-git/src/lib.rs` (`impl GitPort for GitCli` and the `tests` module)

**Interfaces:**
- Consumes: Task 3 types, Task 4 helpers.
- Produces: the five methods on `GitCli`.

- [ ] **Step 1: Write the failing tests.** In the `tests` module of `lib.rs` add (imports: extend the `use forge_ports::{...}` line with `BranchInfo, ChangeSource, DiffOptions, PatchState`):

```rust
    fn write(dir: &Path, name: &str, text: &str) {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn commit(dir: &Path, message: &str) {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(dir).output().unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    // `main` holds `a.txt`. `feature` changes `a.txt` and adds `b.txt` in two commits; then `b.txt`
    // is edited and `c.txt` is untracked. Returns the repository and the sha of the second commit.
    fn feature_repo() -> (TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        write(dir.path(), "a.txt", "a\nb\nc\n");
        commit(dir.path(), "first");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        write(dir.path(), "a.txt", "a\nB\nc\n");
        commit(dir.path(), "change a");
        write(dir.path(), "b.txt", "one\n");
        commit(dir.path(), "add b");
        let second = head(dir.path());
        write(dir.path(), "b.txt", "one\ntwo\n");
        write(dir.path(), "c.txt", "new\n");
        (dir, second)
    }

    fn all(base: &str) -> ChangeSource {
        ChangeSource::All { base: base.to_string() }
    }

    fn list(dir: &Path, source: &ChangeSource) -> Vec<FileChange> {
        GitCli::new().changes(dir, source, &DiffOptions::default()).unwrap()
    }

    fn named<'a>(files: &'a [FileChange], path: &str) -> &'a FileChange {
        files.iter().find(|change| change.file.path == path).unwrap_or_else(|| panic!("{path} in {files:?}"))
    }

    #[test]
    fn all_changes_run_from_the_merge_base_to_the_working_tree() {
        let (dir, _) = feature_repo();
        let files = list(dir.path(), &all("main"));
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "b.txt", "c.txt"]);
        assert_eq!(named(&files, "a.txt").file.status, ChangeStatus::Modified);
        assert_eq!((named(&files, "a.txt").file.added, named(&files, "a.txt").file.removed), (Some(1), Some(1)));
        assert_eq!(named(&files, "b.txt").file.status, ChangeStatus::Added);
        assert_eq!(named(&files, "c.txt").file.status, ChangeStatus::Untracked);
        assert!(matches!(&named(&files, "a.txt").patch, PatchState::Text(text) if text.contains("-b\n+B\n")));
        assert_eq!(named(&files, "c.txt").patch, PatchState::Text("@@ -0,0 +1,1 @@\n+new\n".to_string()));
    }

    #[test]
    fn uncommitted_changes_are_against_head() {
        let (dir, _) = feature_repo();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["b.txt", "c.txt"]);
        assert_eq!(named(&files, "b.txt").file.status, ChangeStatus::Modified);
    }

    #[test]
    fn one_commit_is_against_its_parent() {
        let (dir, second) = feature_repo();
        let files = list(dir.path(), &ChangeSource::Commit { sha: second });
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file.path, "b.txt");
        assert_eq!(files[0].file.status, ChangeStatus::Added);
    }

    #[test]
    fn a_root_commit_is_against_the_empty_tree() {
        let (dir, _) = feature_repo();
        let output = Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(dir.path()).output().unwrap();
        let root = String::from_utf8(output.stdout).unwrap().trim().to_string();
        let files = list(dir.path(), &ChangeSource::Commit { sha: root });
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file.status, ChangeStatus::Added);
    }

    #[test]
    fn a_missing_base_is_reported_as_base_not_found() {
        let (dir, _) = feature_repo();
        let result = GitCli::new().changes(dir.path(), &all("nope"), &DiffOptions::default());
        assert_eq!(result, Err(GitError::BaseNotFound("nope".to_string())));
        let option_like = GitCli::new().changes(dir.path(), &all("-x"), &DiffOptions::default());
        assert_eq!(option_like, Err(GitError::BaseNotFound("-x".to_string())));
        assert!(matches!(GitCli::new().commits(dir.path(), "nope"), Err(GitError::BaseNotFound(_))));
    }

    #[test]
    fn names_with_spaces_and_korean_letters_get_their_patch() {
        let (dir, _) = feature_repo();
        write(dir.path(), "dir/my file.txt", "x\n");
        write(dir.path(), "한글 이름.txt", "y\n");
        git(dir.path(), &["add", "-A"]);
        write(dir.path(), "dir/my file.txt", "x\nz\n");
        write(dir.path(), "-dash.txt", "d\n");
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        for name in ["dir/my file.txt", "한글 이름.txt", "-dash.txt"] {
            assert!(matches!(&named(&files, name).patch, PatchState::Text(text) if !text.is_empty()), "{name}");
        }
    }

    #[test]
    fn hiding_whitespace_drops_a_file_that_only_changed_in_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        write(dir.path(), "w.txt", "a\n  b\n");
        write(dir.path(), "k.txt", "k\n");
        commit(dir.path(), "first");
        write(dir.path(), "w.txt", "a\n    b\n");
        write(dir.path(), "k.txt", "k\nmore\n");
        let shown = GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap();
        assert_eq!(shown.len(), 2);
        let hidden = GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions { ignore_whitespace: true }).unwrap();
        assert_eq!(hidden.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["k.txt"]);
    }

    #[test]
    fn a_patch_over_the_budget_is_too_large_until_asked_for() {
        let dir = repo();
        write(dir.path(), "big.txt", &"abcdef\n".repeat(400_000));
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        let big = named(&files, "big.txt");
        assert_eq!(big.patch, PatchState::TooLarge);
        assert_eq!(big.file.added, Some(400_000));
        let patch = GitCli::new().file_patch(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default(), "big.txt").unwrap();
        assert!(patch.len() > 2 * 1024 * 1024);
    }

    #[test]
    fn an_untracked_binary_file_is_binary() {
        let dir = repo();
        fs::write(dir.path().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(named(&files, "pic.bin").patch, PatchState::Binary);
    }

    #[test]
    fn a_tracked_binary_file_is_binary() {
        let dir = repo();
        fs::write(dir.path().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
        commit(dir.path(), "pic");
        fs::write(dir.path().join("pic.bin"), [0u8, 9, 9, 9]).unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(named(&files, "pic.bin").patch, PatchState::Binary);
    }

    #[test]
    fn a_repository_with_no_commit_lists_its_files() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        fs::write(dir.path().join("a.txt"), "a\n").unwrap();
        git(dir.path(), &["add", "a.txt"]);
        fs::write(dir.path().join("b.txt"), "b\n").unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "b.txt"]);
        // A repository with no commit has no `main` to compare with.
        assert_eq!(GitCli::new().commits(dir.path(), "main"), Err(GitError::BaseNotFound("main".to_string())));
    }

    #[test]
    fn a_subfolder_sees_root_relative_paths() {
        let (dir, _) = feature_repo();
        write(dir.path(), "sub/x.txt", "x\n");
        let files = list(&dir.path().join("sub"), &ChangeSource::Uncommitted);
        assert!(files.iter().any(|c| c.file.path == "sub/x.txt"));
    }

    #[test]
    fn commits_come_newest_first_with_subject_and_author() {
        let (dir, second) = feature_repo();
        let commits = GitCli::new().commits(dir.path(), "main").unwrap();
        assert_eq!(commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), vec!["add b", "change a"]);
        assert_eq!(commits[0].sha, second);
        assert_eq!(commits[0].author, "t");
        assert!(commits[0].time > 0);
    }

    #[test]
    fn branches_name_the_current_branch_and_the_default_base() {
        let (dir, _) = feature_repo();
        let info = GitCli::new().branches(dir.path()).unwrap();
        assert_eq!(info.current.as_deref(), Some("feature"));
        assert_eq!(info.head_label, "feature");
        assert_eq!(info.default_base.as_deref(), Some("main"));
        assert_eq!(info.branches, vec!["feature".to_string(), "main".to_string()]);
    }

    #[test]
    fn a_detached_head_is_labelled_with_its_sha() {
        let (dir, second) = feature_repo();
        git(dir.path(), &["checkout", "-q", "--detach", &second]);
        let info = GitCli::new().branches(dir.path()).unwrap();
        assert_eq!(info.current, None);
        assert!(second.starts_with(&info.head_label) && info.head_label.len() >= 7);
    }

    #[test]
    fn file_lines_read_the_working_tree_or_the_commit() {
        let (dir, second) = feature_repo();
        let now = GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "b.txt").unwrap();
        assert_eq!(now, vec!["one".to_string(), "two".to_string()]);
        let then = GitCli::new().file_lines(dir.path(), &ChangeSource::Commit { sha: second }, "b.txt").unwrap();
        assert_eq!(then, vec!["one".to_string()]);
        assert!(GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "../x").is_err());
    }

    #[test]
    fn a_folder_that_is_not_a_repository_says_so_for_changes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()), Err(GitError::NotARepository));
    }

    #[test]
    fn a_missing_git_is_reported_as_a_message_for_changes() {
        let dir = repo();
        let cli = GitCli::with_program("forge-no-such-git", Duration::from_secs(5));
        let error = cli.changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap_err();
        assert_eq!(error, GitError::Failed("git is not installed".to_string()));
    }
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-adapter-git`
Expected: FAIL to compile (trait methods not implemented on `GitCli`).

- [ ] **Step 3: Implement.** In `lib.rs` extend the imports:

```rust
use forge_ports::{BranchInfo, BranchList, ChangeSource, ChangeStatus, ChangedFile, CommitInfo, DiffOptions, FileChange, GitError, GitPort, PatchState, WorktreeError, WorktreePort, WorktreeState};

use changes::{parse_counts, parse_name_status, split_patch, safe_ref, untracked_patch, Count, EMPTY_TREE, PATCH_BUDGET};
```

Add helper methods in `impl GitCli` (after `root`):

```rust
    fn checked(&self, output: Output) -> Result<Output, GitError> {
        if output.success() {
            Ok(output)
        } else {
            Err(failure(&output.stderr))
        }
    }

    fn has_commit(&self, root: &Path, name: &str) -> bool {
        safe_ref(name)
            && self
                .run(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")])
                .is_ok_and(|output| output.success())
    }

    fn diff_cmd(&self, root: &Path, flags: &[&str], from: &str, to: Option<&str>, only: Option<&str>) -> Result<Output, GitError> {
        let mut args = vec!["-c", "core.quotepath=off", "diff"];
        args.extend_from_slice(flags);
        args.push(from);
        if let Some(to) = to {
            args.push(to);
        }
        if let Some(path) = only {
            args.extend(["--", path]);
        }
        self.run(root, &args)
    }

    // What `source` compares: the starting tree, the end (`None` is the working tree) and whether
    // untracked files belong to it.
    fn range(&self, root: &Path, source: &ChangeSource) -> Result<(String, Option<String>, bool), GitError> {
        let has_head = self.has_commit(root, "HEAD");
        match source {
            ChangeSource::Uncommitted => Ok((if has_head { "HEAD".to_string() } else { EMPTY_TREE.to_string() }, None, true)),
            ChangeSource::All { base } => {
                if !self.has_commit(root, base) {
                    return Err(GitError::BaseNotFound(base.clone()));
                }
                if !has_head {
                    return Ok((EMPTY_TREE.to_string(), None, true));
                }
                // Unrelated histories have no merge base: compare with the base itself.
                let from = match self.run(root, &["merge-base", base, "HEAD"]) {
                    Ok(output) if output.success() => String::from_utf8_lossy(&output.stdout).trim().to_string(),
                    _ => base.clone(),
                };
                Ok((from, None, true))
            }
            ChangeSource::Commit { sha } => {
                if !self.has_commit(root, sha) {
                    return Err(GitError::BaseNotFound(sha.clone()));
                }
                let from = if self.has_commit(root, &format!("{sha}^")) { format!("{sha}^") } else { EMPTY_TREE.to_string() };
                Ok((from, Some(sha.clone()), false))
            }
        }
    }

    // `changes` and `file_patch`: `only` limits it to one path, `budget` is the patch text allowed.
    fn collect(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, only: Option<&str>, mut budget: usize) -> Result<Vec<FileChange>, GitError> {
        let root = self.root(cwd)?;
        let (from, to, untracked) = self.range(&root, source)?;
        let to = to.as_deref();
        let names = self.checked(self.diff_cmd(&root, &["--name-status", "-z", "--no-renames"], &from, to, only)?)?;
        let counts = self.checked(self.diff_cmd(&root, &["--numstat", "-z", "--no-renames"], &from, to, only)?)?;
        let mut flags = vec!["--no-color", "--no-ext-diff", "--no-renames", "-U3"];
        if options.ignore_whitespace {
            flags.push("-w");
        }
        let patch = self.checked(self.diff_cmd(&root, &flags, &from, to, only)?)?;
        let mut patches = split_patch(&String::from_utf8_lossy(&patch.stdout));
        let counts = parse_counts(&counts.stdout);
        let mut files = Vec::new();
        for (status, path) in parse_name_status(&names.stdout) {
            let (added, removed, binary) = match counts.get(&path) {
                Some(Count::Lines(added, removed)) => (Some(*added), Some(*removed), false),
                Some(Count::Binary) => (None, None, true),
                None => (None, None, false),
            };
            let state = if binary {
                PatchState::Binary
            } else {
                match patches.remove(&path) {
                    Some(text) if text.len() <= budget => {
                        budget -= text.len();
                        PatchState::Text(text)
                    }
                    Some(_) => PatchState::TooLarge,
                    // `-w` leaves out a file whose only change is whitespace.
                    None if options.ignore_whitespace => continue,
                    None => PatchState::Text(String::new()),
                }
            };
            files.push(FileChange { file: ChangedFile { path, status, added, removed }, patch: state });
        }
        if untracked {
            let mut args = vec!["ls-files", "--others", "--exclude-standard", "-z"];
            if let Some(path) = only {
                args.extend(["--", path]);
            }
            let others = self.checked(self.run(&root, &args)?)?;
            let mut paths: Vec<String> = String::from_utf8_lossy(&others.stdout).split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect();
            paths.sort();
            for path in paths {
                // A link to a folder or a file that went away between the two commands reads as binary.
                let (state, added) = match std::fs::read(root.join(&path)) {
                    Ok(bytes) => untracked_patch(&bytes, budget),
                    Err(_) => (PatchState::Binary, 0),
                };
                if let PatchState::Text(text) = &state {
                    budget -= text.len();
                }
                let counts = (!matches!(state, PatchState::Binary)).then_some((added, 0));
                files.push(FileChange {
                    file: ChangedFile { path, status: ChangeStatus::Untracked, added: counts.map(|c| c.0), removed: counts.map(|c| c.1) },
                    patch: state,
                });
            }
        }
        files.sort_by(|a, b| a.file.path.cmp(&b.file.path));
        Ok(files)
    }
```

Add the trait methods to `impl GitPort for GitCli` (keep `status` and `diff`):

```rust
    fn branches(&self, cwd: &Path) -> Result<BranchInfo, GitError> {
        let root = self.root(cwd)?;
        let refs = self.checked(self.run(&root, &["for-each-ref", "--format=%(refname:short)", "--sort=refname", "refs/heads"])?)?;
        let branches = lines_of(&refs.stdout);
        let current = self
            .run(&root, &["symbolic-ref", "--short", "-q", "HEAD"])
            .ok()
            .filter(|output| output.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|name| !name.is_empty());
        let head_label = match &current {
            Some(name) => name.clone(),
            None => self
                .run(&root, &["rev-parse", "--short", "HEAD"])
                .ok()
                .filter(|output| output.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_default(),
        };
        let remote_head = self
            .run(&root, &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"])
            .ok()
            .filter(|output| output.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .map(|name| name.strip_prefix("origin/").filter(|local| branches.iter().any(|b| b == local)).map(str::to_string).unwrap_or(name))
            .filter(|name| !name.is_empty());
        let default_base = remote_head.or_else(|| ["main", "master"].into_iter().find(|name| branches.iter().any(|b| b == name)).map(str::to_string));
        Ok(BranchInfo { current, head_label, default_base, branches })
    }

    fn commits(&self, cwd: &Path, base: &str) -> Result<Vec<CommitInfo>, GitError> {
        let root = self.root(cwd)?;
        if !self.has_commit(&root, base) {
            return Err(GitError::BaseNotFound(base.to_string()));
        }
        if !self.has_commit(&root, "HEAD") {
            return Ok(Vec::new());
        }
        let range = format!("{base}..HEAD");
        let output = self.checked(self.run(&root, &["log", "-z", "-n", "200", "--format=%H%x1f%an%x1f%at%x1f%s", &range])?)?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .split('\0')
            .filter_map(|entry| {
                let mut fields = entry.trim_start_matches('\n').splitn(4, '\x1f');
                Some(CommitInfo {
                    sha: fields.next()?.to_string(),
                    author: fields.next()?.to_string(),
                    time: fields.next()?.parse().ok()?,
                    subject: fields.next()?.to_string(),
                })
            })
            .collect())
    }

    fn changes(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions) -> Result<Vec<FileChange>, GitError> {
        self.collect(cwd, source, options, None, PATCH_BUDGET)
    }

    fn file_lines(&self, cwd: &Path, source: &ChangeSource, path: &str) -> Result<Vec<String>, GitError> {
        if path.split('/').any(|part| part == "..") {
            return Err(GitError::Failed("Path is outside the repository".to_string()));
        }
        let root = self.root(cwd)?;
        let bytes = match source {
            ChangeSource::Commit { sha } => {
                if !self.has_commit(&root, sha) {
                    return Err(GitError::BaseNotFound(sha.clone()));
                }
                self.checked(self.run(&root, &["show", &format!("{sha}:{path}")])?)?.stdout
            }
            _ => std::fs::read(root.join(path)).map_err(|error| GitError::Failed(error.to_string()))?,
        };
        Ok(String::from_utf8_lossy(&bytes).lines().map(str::to_string).collect())
    }

    fn file_patch(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, path: &str) -> Result<String, GitError> {
        let files = self.collect(cwd, source, options, Some(path), usize::MAX)?;
        Ok(match files.into_iter().next().map(|change| change.patch) {
            Some(PatchState::Text(text)) => text,
            _ => String::new(),
        })
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-adapter-git`
Expected: PASS (the new tests, the Task 4 tests, and the existing `status`/`diff` tests).

If `a_patch_over_the_budget_is_too_large_until_asked_for` is slow, that is the git call over a 2.8 MB patch; it must still finish well inside the 10 s timeout.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-git
git commit -m "feat: git adapter reads branches, commits, changes by source, file lines and patches"
```

---

### Task 6: Pure view logic in forge-features

**Files:**
- Create: `crates/forge-features/src/change_rows.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod change_rows;` and `pub use`)

**Interfaces:**
- Consumes: `forge_ports::{ChangeSource, ChangeStatus, ChangedFile}`.
- Produces (re-exported from `forge_features`):
  - `ChangeView { All, Uncommitted, Commit(String) }` (`Default` = `All`)
  - `ChangesSession { view, base: Option<String>, folded_files: HashSet<String>, expanded_files: HashSet<String>, closed_folders: HashSet<String> }` (`Default`)
  - `source_for(view: &ChangeView, base: Option<&str>) -> ChangeSource`
  - `TreeRow::{Folder { path, name, depth, open }, File { index, path, name, folder, depth, added, removed, status }}`
  - `tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>) -> Vec<TreeRow>`
  - `stack_order(files: &[ChangedFile], group: bool) -> Vec<usize>`
  - `filter_files(files: &[ChangedFile], query: &str) -> Vec<usize>`
  - `is_folded(session: &ChangesSession, path: &str, position: usize) -> bool`, `toggle_fold(session: &mut ChangesSession, path: &str, folded_now: bool)`, `fold_all(session: &mut ChangesSession, paths: &[String])`, `unfold_all(session: &mut ChangesSession, paths: &[String])`
  - `split_folder(path: &str) -> (String, String)` (folder, name), constants `OPEN_SECTIONS = 50`, `ROW_LIMIT = 500`

- [ ] **Step 1: Write the failing tests.** Create `change_rows.rs` starting with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> ChangedFile {
        ChangedFile { path: path.to_string(), status: ChangeStatus::Modified, added: Some(1), removed: Some(0) }
    }

    fn paths(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                TreeRow::Folder { path, depth, .. } => format!("{}{path}/", " ".repeat(*depth)),
                TreeRow::File { path, depth, .. } => format!("{}{path}", " ".repeat(*depth)),
            })
            .collect()
    }

    #[test]
    fn a_grouped_tree_lists_folders_first_then_files() {
        let files = vec![file("b.txt"), file("src/z.rs"), file("a.txt"), file("src/a.rs"), file("docs/x/y.md")];
        let rows = tree_rows(&files, true, &HashSet::new());
        assert_eq!(paths(&rows), vec!["docs/", " docs/x/", "  docs/x/y.md", "src/", " src/a.rs", " src/z.rs", "a.txt", "b.txt"]);
    }

    #[test]
    fn a_flat_list_is_by_path() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        let rows = tree_rows(&files, false, &HashSet::new());
        assert_eq!(paths(&rows), vec!["a.txt", "b.txt", "src/a.rs"]);
    }

    #[test]
    fn a_closed_folder_hides_what_is_inside_it() {
        let files = vec![file("src/a.rs"), file("a.txt")];
        let closed: HashSet<String> = ["src".to_string()].into();
        assert_eq!(paths(&tree_rows(&files, true, &closed)), vec!["src/", "a.txt"]);
    }

    #[test]
    fn the_stack_follows_the_tree_order_even_when_a_folder_is_closed() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        assert_eq!(stack_order(&files, true), vec![1, 2, 0]);
        assert_eq!(stack_order(&files, false), vec![2, 0, 1]);
    }

    #[test]
    fn a_file_row_knows_its_folder_and_name() {
        let rows = tree_rows(&[file("src/deep/a.rs")], false, &HashSet::new());
        match &rows[0] {
            TreeRow::File { folder, name, .. } => assert_eq!((folder.as_str(), name.as_str()), ("src/deep", "a.rs")),
            other => panic!("{other:?}"),
        }
        assert_eq!(split_folder("a.txt"), (String::new(), "a.txt".to_string()));
    }

    #[test]
    fn the_search_matches_a_part_of_the_path_without_case() {
        let files = vec![file("src/Main.rs"), file("docs/readme.md"), file("한글.txt")];
        assert_eq!(filter_files(&files, "MAIN"), vec![0]);
        assert_eq!(filter_files(&files, "한글"), vec![2]);
        assert_eq!(filter_files(&files, ""), vec![0, 1, 2]);
        assert!(filter_files(&files, "zzz").is_empty());
    }

    #[test]
    fn a_missing_base_turns_all_changes_into_uncommitted() {
        assert_eq!(source_for(&ChangeView::All, Some("main")), ChangeSource::All { base: "main".to_string() });
        assert_eq!(source_for(&ChangeView::All, None), ChangeSource::Uncommitted);
        assert_eq!(source_for(&ChangeView::Uncommitted, Some("main")), ChangeSource::Uncommitted);
        assert_eq!(source_for(&ChangeView::Commit("abc".to_string()), None), ChangeSource::Commit { sha: "abc".to_string() });
    }

    #[test]
    fn sections_after_the_first_fifty_start_folded() {
        let session = ChangesSession::default();
        assert!(!is_folded(&session, "a", 0));
        assert!(!is_folded(&session, "a", OPEN_SECTIONS - 1));
        assert!(is_folded(&session, "a", OPEN_SECTIONS));
    }

    #[test]
    fn a_click_on_a_header_flips_the_fold_and_remembers_it() {
        let mut session = ChangesSession::default();
        toggle_fold(&mut session, "a", false);
        assert!(is_folded(&session, "a", 0));
        toggle_fold(&mut session, "a", true);
        assert!(!is_folded(&session, "a", 0));
        // A section past the cap, opened by hand, stays open.
        toggle_fold(&mut session, "z", true);
        assert!(!is_folded(&session, "z", OPEN_SECTIONS + 5));
    }

    #[test]
    fn collapse_all_and_expand_all_cover_every_file() {
        let mut session = ChangesSession::default();
        let all = vec!["a".to_string(), "b".to_string()];
        fold_all(&mut session, &all);
        assert!(is_folded(&session, "a", 0) && is_folded(&session, "b", 1));
        unfold_all(&mut session, &all);
        assert!(!is_folded(&session, "a", 0) && !is_folded(&session, "b", OPEN_SECTIONS + 9));
    }
}
```

- [ ] **Step 2: Wire and run to see failure.** In `forge-features/src/lib.rs` add `mod change_rows;` and:

```rust
pub use change_rows::{filter_files, fold_all, is_folded, source_for, split_folder, stack_order, toggle_fold, tree_rows, unfold_all, ChangeView, ChangesSession, TreeRow, OPEN_SECTIONS, ROW_LIMIT};
```

Run: `cargo test -p forge-features change_rows`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** Above the tests:

```rust
use std::collections::{BTreeMap, HashSet};

use forge_ports::{ChangeSource, ChangeStatus, ChangedFile};

/// Only this many stack sections start unfolded: a thousand open diffs would freeze the window.
pub const OPEN_SECTIONS: usize = 50;
/// Only this many file rows are drawn.
pub const ROW_LIMIT: usize = 500;

/// What the panel shows. `All` is against the base branch; `Commit` is one commit of the branch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ChangeView {
    #[default]
    All,
    Uncommitted,
    Commit(String),
}

/// What the person set in one session's Changes panel (kept while Forge runs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangesSession {
    pub view: ChangeView,
    /// The base the person picked; `None` follows the default.
    pub base: Option<String>,
    /// Files folded by hand.
    pub folded_files: HashSet<String>,
    /// Files past `OPEN_SECTIONS` opened by hand.
    pub expanded_files: HashSet<String>,
    pub closed_folders: HashSet<String>,
}

/// All changes with no base to compare with are the uncommitted changes.
pub fn source_for(view: &ChangeView, base: Option<&str>) -> ChangeSource {
    match (view, base) {
        (ChangeView::All, Some(base)) => ChangeSource::All { base: base.to_string() },
        (ChangeView::All, None) | (ChangeView::Uncommitted, _) => ChangeSource::Uncommitted,
        (ChangeView::Commit(sha), _) => ChangeSource::Commit { sha: sha.clone() },
    }
}

/// The folder (no trailing slash, empty at the top) and the name of a path.
pub fn split_folder(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder.to_string(), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    Folder { path: String, name: String, depth: usize, open: bool },
    File { index: usize, path: String, name: String, folder: String, depth: usize, added: Option<usize>, removed: Option<usize>, status: ChangeStatus },
}

#[derive(Default)]
struct Node {
    folders: BTreeMap<String, Node>,
    files: Vec<usize>,
}

fn file_row(files: &[ChangedFile], index: usize, depth: usize) -> TreeRow {
    let file = &files[index];
    let (folder, name) = split_folder(&file.path);
    TreeRow::File { index, path: file.path.clone(), name, folder, depth, added: file.added, removed: file.removed, status: file.status }
}

fn walk(node: &Node, prefix: &str, depth: usize, files: &[ChangedFile], closed: &HashSet<String>, respect_closed: bool, rows: &mut Vec<TreeRow>) {
    for (name, child) in &node.folders {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let open = !closed.contains(&path);
        rows.push(TreeRow::Folder { path: path.clone(), name: name.clone(), depth, open });
        if open || !respect_closed {
            walk(child, &path, depth + 1, files, closed, respect_closed, rows);
        }
    }
    let mut ids = node.files.clone();
    ids.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
    rows.extend(ids.into_iter().map(|index| file_row(files, index, depth)));
}

fn build(files: &[ChangedFile], group: bool, closed: &HashSet<String>, respect_closed: bool) -> Vec<TreeRow> {
    if !group {
        let mut order: Vec<usize> = (0..files.len()).collect();
        order.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
        return order.into_iter().map(|index| file_row(files, index, 0)).collect();
    }
    let mut root = Node::default();
    for (index, file) in files.iter().enumerate() {
        let mut node = &mut root;
        let mut parts: Vec<&str> = file.path.split('/').collect();
        parts.pop();
        for part in parts {
            node = node.folders.entry(part.to_string()).or_default();
        }
        node.files.push(index);
    }
    let mut rows = Vec::new();
    walk(&root, "", 0, files, closed, respect_closed, &mut rows);
    rows
}

/// The rows of the right column: folders (folders first, then files, each by name) or a flat list.
pub fn tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>) -> Vec<TreeRow> {
    build(files, group, closed, true)
}

/// The order of the stacked diffs: the tree's order with every folder open.
pub fn stack_order(files: &[ChangedFile], group: bool) -> Vec<usize> {
    build(files, group, &HashSet::new(), false)
        .into_iter()
        .filter_map(|row| match row {
            TreeRow::File { index, .. } => Some(index),
            TreeRow::Folder { .. } => None,
        })
        .collect()
}

/// The files whose path holds `query` (no case); every file for an empty query.
pub fn filter_files(files: &[ChangedFile], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    (0..files.len()).filter(|index| query.is_empty() || files[*index].path.to_lowercase().contains(&query)).collect()
}

pub fn is_folded(session: &ChangesSession, path: &str, position: usize) -> bool {
    session.folded_files.contains(path) || (position >= OPEN_SECTIONS && !session.expanded_files.contains(path))
}

/// A click on a section header: folds an open section, opens a folded one.
pub fn toggle_fold(session: &mut ChangesSession, path: &str, folded_now: bool) {
    if folded_now {
        session.folded_files.remove(path);
        session.expanded_files.insert(path.to_string());
    } else {
        session.expanded_files.remove(path);
        session.folded_files.insert(path.to_string());
    }
}

pub fn fold_all(session: &mut ChangesSession, paths: &[String]) {
    session.expanded_files.clear();
    session.folded_files = paths.iter().cloned().collect();
}

pub fn unfold_all(session: &mut ChangesSession, paths: &[String]) {
    session.folded_files.clear();
    session.expanded_files = paths.iter().cloned().collect();
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-features change_rows`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: tree order, view state and file search for the Changes panel"
```

---

### Task 7: The file menu target and the Workspace state

**Files:**
- Modify: `crates/forge-features/src/file_menu.rs` (`FileAction::OpenFile`, `MenuTarget::Change`, tests)
- Modify: `crates/forge-widgets/src/files_state.rs:138-180` (`run_action` gets the `OpenFile` arm)
- Modify: `crates/forge-features/src/workspace.rs` (signals at ~line 119 and ~181, `reload_prefs` ~line 338, new methods)
- Modify: `crates/forge-shared/src/ko.rs` (strings below)

**Interfaces:**
- Consumes: `ChangesSession`, `ChangesViewPrefs`.
- Produces: `FileAction::OpenFile`; `MenuTarget::Change { deleted: bool }`; on `Workspace`: `changes_view_prefs() -> ChangesViewPrefs`, `update_changes_view_prefs(impl FnOnce(&mut ChangesViewPrefs))`, `changes_session(id: LiveSessionId) -> ChangesSession`, `update_changes_session(id, impl FnOnce(&mut ChangesSession))`, `worktree_base(path: &Path) -> Option<String>`.

- [ ] **Step 1: Write the failing tests** in `file_menu.rs`'s test module (find it at the bottom; add)

```rust
    #[test]
    fn a_changed_file_menu_offers_attach_copy_open_and_reveal() {
        let labels: Vec<_> = file_menu(MenuTarget::Change { deleted: false })
            .into_iter()
            .filter_map(|entry| match entry {
                MenuEntry::Action { label, enabled, .. } => Some((label, enabled)),
                MenuEntry::Submenu { label, .. } => Some((label, true)),
                MenuEntry::Divider => None,
            })
            .collect();
        assert_eq!(labels.first(), Some(&(tr!("Attach as context"), true)));
        assert!(labels.iter().any(|(label, _)| *label == tr!("Open file")));
        assert!(labels.iter().any(|(label, _)| *label == tr!("Copy")));
        assert!(labels.iter().any(|(label, _)| *label == tr!("Open in")));
    }

    #[test]
    fn a_deleted_file_can_only_be_copied_by_name() {
        let entries = file_menu(MenuTarget::Change { deleted: true });
        assert!(entries.iter().all(|entry| matches!(entry, MenuEntry::Submenu { .. } | MenuEntry::Divider)));
        assert!(entries.iter().any(|entry| matches!(entry, MenuEntry::Submenu { label, .. } if *label == tr!("Copy"))));
    }
```

Add a test in `crates/forge-features/src/workspace.rs`'s existing test module only if one exists for prefs; otherwise the Workspace methods are covered by Task 9's window check (they are thin wrappers over signals).

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-features file_menu`
Expected: FAIL to compile (`MenuTarget::Change` missing).

- [ ] **Step 3: Implement.**

`file_menu.rs`: add `OpenFile,` to `FileAction` (after `Reveal`), add `Change { deleted: bool },` to `MenuTarget`, and in `file_menu`:

```rust
        MenuTarget::Change { deleted: true } => vec![copy_menu()],
        MenuTarget::Change { deleted: false } => vec![
            item(tr!("Attach as context"), FileAction::Attach),
            copy_menu(),
            MenuEntry::Divider,
            item(tr!("Open file"), FileAction::OpenFile),
            MenuEntry::Submenu {
                label: tr!("Open in"),
                entries: vec![item(tr!("VS Code"), FileAction::OpenVsCode), MenuEntry::Divider, item(tr!("Default app"), FileAction::OpenDefault)],
            },
            item(forge_domain::SHOW_IN_FILE_MANAGER, FileAction::Reveal),
        ],
```

`files_state.rs` in `run_action`: add before the final arm

```rust
            FileAction::OpenFile => self.workspace.open_file(self.id, absolute(&self.cwd, path).to_string_lossy().to_string()),
```

and add `FileAction::OpenFile` is not in the last no-op arm (it has its own arm). The compiler lists any other exhaustive `match` over `FileAction` (grep `FileAction::Reveal` across `crates/`); add the arm there as well.

`ko.rs` (insert each pair at its byte-order place; the i18n test shows where it is wrong): `("Open file", "파일 열기")`, `("failed to save the changes view settings: {error}", "변경 사항 보기 설정을 저장하지 못했습니다: {error}")`. (`Attach as context`, `Copy`, `Open in`, `VS Code`, `Default app` exist already.)

`workspace.rs`: add the fields next to `editor`:

```rust
    changes_view: Signal<ChangesViewPrefs>,
    changes_sessions: Signal<HashMap<LiveSessionId, ChangesSession>>,
```

with initial values `Signal::new(ChangesViewPrefs::default())` and `Signal::new(HashMap::new())` next to `editor: Signal::new(EditorPrefs::default()),`; extend the `use forge_domain::{...}` import with `ChangesViewPrefs`; import `ChangesSession` from the crate (`use crate::ChangesSession;` or the module path used by neighbours). In `reload_prefs` add `let mut changes_view = self.changes_view;` and `changes_view.set(config.changes_view);` after `editor.set(config.editor);`. Add the methods after `update_editor_prefs`:

```rust
    /// How the Changes panel draws a diff (its `⋮` menu).
    pub fn changes_view_prefs(&self) -> ChangesViewPrefs {
        self.changes_view.read().clone()
    }

    /// Changes the Changes panel options and saves them.
    pub fn update_changes_view_prefs(&self, change: impl FnOnce(&mut ChangesViewPrefs)) {
        let mut view = self.changes_view;
        change(&mut view.write());
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.changes_view_prefs();
        // Root scope: the save must finish even if the menu that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_code_settings(move |config| config.changes_view = prefs).await {
                this.report(tr!("failed to save the changes view settings: {error}", error = error));
            }
        });
    }

    /// What the person chose in one session's Changes panel.
    pub fn changes_session(&self, id: LiveSessionId) -> ChangesSession {
        self.changes_sessions.read().get(&id).cloned().unwrap_or_default()
    }

    pub fn update_changes_session(&self, id: LiveSessionId, change: impl FnOnce(&mut ChangesSession)) {
        let mut sessions = self.changes_sessions;
        change(sessions.write().entry(id).or_default());
    }

    /// The branch a worktree of Forge's record started from; `None` for any other folder.
    pub fn worktree_base(&self, path: &Path) -> Option<String> {
        self.worktrees.read().iter().find(|record| record.path == path).map(|record| record.base_branch.clone())
    }
```

- [ ] **Step 4: Run the tests**

Run: `cargo test -p forge-features -p forge-shared -p forge-widgets`
Expected: PASS (the i18n test passes only with both new strings in `ko.rs`).

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: file menu for a changed file, Changes view options and per-session state in the workspace"
```

---

### Task 8: Styles for the new panel

**Files:**
- Modify: `crates/forge-shared/src/theme.css` (replace lines 584-595, the old `.changes-*` and `.change-*` block, and keep `.diff*` rules as they are)

- [ ] **Step 1: Replace the block** (the `.changes-body` ... `.changes-diff .diff` rules) with:

```css
.changes-body { flex: 1; min-height: 0; display: flex; }
.cd-main { flex: 1; min-width: 0; overflow: auto; }
.cd-side { flex: none; width: 26%; min-width: 180px; display: flex; flex-direction: column; border-left: 1px solid var(--border); box-sizing: border-box; min-height: 0; }
.cd-tree { flex: 1; min-height: 0; margin: 0; padding: 4px; list-style: none; overflow-y: auto; }
.cd-row { display: flex; align-items: center; gap: 6px; width: 100%; padding: 3px 6px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; text-align: left; cursor: pointer; }
.cd-row:hover { background: var(--bg-hover); }
.cd-row .cd-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-chevron { flex: none; width: 1.2em; color: var(--fg-muted); user-select: none; }
.cd-add { color: var(--hl-string); }
.cd-del { color: var(--danger); }
.cd-counts { flex: none; font-size: 11px; color: var(--fg-muted); white-space: nowrap; }
.cd-note { padding: 8px 10px; color: var(--fg-muted); font-size: 12px; }
.cd-stack { padding: 0 0 12px; }
.cd-file { border-bottom: 1px solid var(--border); }
.cd-file-head { position: sticky; top: 0; z-index: 1; display: flex; align-items: center; gap: 6px; padding: 6px 10px; background: var(--bg-card); font-size: 12px; cursor: pointer; user-select: none; }
.cd-file-head .cd-name { color: var(--fg); }
.cd-folder { color: var(--fg-muted); }
.cd-file-head .cd-counts { margin-left: 4px; }
.cd-lines { font-family: var(--mono); font-size: 12px; }
.cd-lines .diff-line { min-height: 1.5em; }
.cd-lines.nowrap .diff-text { white-space: pre; overflow-wrap: normal; }
.cd-no { flex: none; width: 4ch; padding-right: 6px; text-align: right; color: var(--fg-muted); user-select: none; }
.cd-word { background: var(--diff-word-bg, rgba(127, 127, 127, 0.28)); border-radius: 2px; }
.cd-gap { display: flex; align-items: center; gap: 6px; width: 100%; padding: 4px 10px; border: none; background: transparent; color: var(--fg-muted); font: inherit; font-size: 12px; cursor: pointer; text-align: left; }
.cd-gap:hover { background: var(--bg-hover); }
.cd-hunk + .cd-hunk { border-top: 1px dashed var(--border); }
.cd-sbs { display: grid; grid-template-columns: 1fr 1fr; }
.cd-sbs .diff-line { min-width: 0; }
.cd-sbs .empty { background: var(--bg-card); }
.cd-head-button { display: inline-flex; align-items: center; gap: 4px; max-width: 100%; padding: 2px 8px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; cursor: pointer; overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
.cd-head-button:hover { background: var(--bg-hover); }
.cd-commits { flex: none; max-height: 40%; overflow-y: auto; border-top: 1px solid var(--border); padding: 4px; }
.cd-commits-title { display: flex; align-items: center; gap: 6px; padding: 4px 6px; color: var(--fg-muted); font-size: 12px; cursor: pointer; user-select: none; }
.cd-commit { display: block; width: 100%; padding: 4px 6px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; text-align: left; cursor: pointer; }
.cd-commit:hover, .cd-commit.selected { background: var(--bg-hover); }
.cd-commit .subject { display: block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.cd-commit .meta { display: block; color: var(--fg-muted); font-size: 11px; }
.cd-search { position: absolute; z-index: 5; right: 8px; top: 40px; width: 320px; max-width: calc(100% - 16px); background: var(--bg-card); border: 1px solid var(--border); border-radius: var(--radius); box-shadow: 0 6px 24px rgba(0, 0, 0, 0.18); }
.cd-search input { width: 100%; box-sizing: border-box; padding: 8px 10px; border: none; border-bottom: 1px solid var(--border); background: transparent; color: var(--fg); font: inherit; font-size: 12px; outline: none; }
.cd-search-list { max-height: 260px; overflow-y: auto; padding: 4px; }
```

If `--diff-word-bg` is not defined in the theme, the fallback value is used; add a real value to both the light and the dark token blocks of `theme.css` (find `--diff-added-bg` and put `--diff-word-bg` next to it, a slightly stronger tint of the same colour family: for example `rgba(46, 160, 67, 0.28)` in light and `rgba(63, 185, 80, 0.32)` in dark; neutral grey is acceptable because one class marks both added and removed words).

- [ ] **Step 2: Build check**

Run: `cargo build -p forge-shared`
Expected: PASS (CSS is a string asset).

- [ ] **Step 3: Commit**

```bash
git add crates/forge-shared
git commit -m "style: Changes panel stack, tree, commits and search"
```

---

### Task 9: The panel on the new data: stack and tree column

**Files:**
- Create: `crates/forge-widgets/src/changes_stack.rs`
- Create: `crates/forge-widgets/src/changes_tree.rs`
- Modify: `crates/forge-widgets/src/changes_panel.rs` (rewrite)
- Modify: `crates/forge-widgets/src/lib.rs` (`mod changes_stack; mod changes_tree;`)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: Tasks 2, 3, 6, 7.
- Produces: `DiffStack { id, cwd, source, files, order, prefs, session, on_menu }`, `scroll_to_diff(path: &str)` (in `changes_stack.rs`), `ChangesTree { id, files, prefs, closed, on_pick, on_menu, commits }` where `commits: Element` is drawn under the tree (Task 11 fills it), and the new `ChangesPanel`.

Tests: the Dioxus components are checked in the real window (Task 15). The pure parts they call are tested in Tasks 2 and 6. Run `cargo build -p forge-widgets` after each step below to keep it compiling.

- [ ] **Step 1: `changes_stack.rs`.**

```rust
use std::collections::HashMap;
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_entities::{gap_after, gap_lines, icon_of, paired_spans, parse_patch, side_by_side, split_by_spans, DiffLineKind, PatchHunk, PatchLine, Span};
use forge_features::{is_folded, run_blocking, split_folder, toggle_fold, use_workspace, ChangesSession};
use forge_ports::{ChangeSource, ChangeStatus, DiffOptions, FileChange, PatchState};
use forge_shared::tr;

use crate::files_icons::FileTypeIcon;

/// Scrolls the stack to a file's section.
pub(crate) fn scroll_to_diff(path: &str) {
    let literal = serde_json::to_string(path).unwrap_or_default();
    document::eval(&format!(
        "(function(p){{const el=[...document.querySelectorAll('[data-diff-path]')].find(e=>e.dataset.diffPath===p);if(el){{el.scrollIntoView({{block:'start'}});}}}})({literal});"
    ));
}

/// Every changed file one under the other, each foldable. `order` is the order of `files` to draw.
#[component]
pub fn DiffStack(
    id: LiveSessionId,
    cwd: PathBuf,
    source: ChangeSource,
    files: Vec<FileChange>,
    order: Vec<usize>,
    prefs: ChangesViewPrefs,
    session: ChangesSession,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    rsx! {
        div { class: "cd-stack",
            for (position, index) in order.iter().copied().enumerate() {
                FileSection {
                    key: "{files[index].file.path}",
                    id,
                    cwd: cwd.clone(),
                    source: source.clone(),
                    change: files[index].clone(),
                    prefs: prefs.clone(),
                    folded: is_folded(&session, &files[index].file.path, position),
                    on_menu,
                }
            }
        }
    }
}

fn line_class(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => "context",
        DiffLineKind::Added => "added",
        DiffLineKind::Removed => "removed",
    }
}

fn sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    }
}

fn number(value: Option<u32>) -> String {
    value.map(|n| n.to_string()).unwrap_or_default()
}

fn line_row(line: &PatchLine, spans: &[Span], highlight: bool) -> Element {
    let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
    rsx! {
        div { class: "diff-line {line_class(line.kind)}",
            span { class: "cd-no", "{number(line.old_no)}" }
            span { class: "cd-no", "{number(line.new_no)}" }
            span { class: "diff-sign", "{sign(line.kind)}" }
            span { class: "diff-text",
                for (piece , hot) in pieces {
                    span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                }
            }
        }
    }
}

fn cell(line: Option<&PatchLine>, spans: &[Span], highlight: bool) -> Element {
    match line {
        Some(line) => {
            let pieces = if highlight && !spans.is_empty() { split_by_spans(&line.text, spans) } else { vec![(line.text.clone(), false)] };
            let number = if line.kind == DiffLineKind::Added { line.new_no } else { line.old_no.or(line.new_no) };
            rsx! {
                div { class: "diff-line {line_class(line.kind)}",
                    span { class: "cd-no", "{self::number(number)}" }
                    span { class: "diff-sign", "{sign(line.kind)}" }
                    span { class: "diff-text",
                        for (piece , hot) in pieces {
                            span { class: if hot { "cd-word" } else { "" }, "{piece}" }
                        }
                    }
                }
            }
        }
        None => rsx! { div { class: "diff-line empty" } },
    }
}

fn hunk_view(hunk: &PatchHunk, prefs: &ChangesViewPrefs) -> Element {
    let spans = if prefs.highlight_words { paired_spans(&hunk.lines) } else { Vec::new() };
    let none: Vec<Span> = Vec::new();
    let spans_of = |index: usize| spans.get(index).unwrap_or(&none);
    if prefs.side_by_side {
        let rows = side_by_side(&hunk.lines);
        rsx! {
            div { class: "cd-hunk cd-sbs",
                for (left , right) in rows {
                    {cell(left.map(|i| &hunk.lines[i]), left.map(spans_of).unwrap_or(&none), prefs.highlight_words)}
                    {cell(right.map(|i| &hunk.lines[i]), right.map(spans_of).unwrap_or(&none), prefs.highlight_words)}
                }
            }
        }
    } else {
        rsx! {
            div { class: "cd-hunk",
                for (index , line) in hunk.lines.iter().enumerate() {
                    {line_row(line, spans_of(index), prefs.highlight_words)}
                }
            }
        }
    }
}

#[component]
fn FileSection(
    id: LiveSessionId,
    cwd: PathBuf,
    source: ChangeSource,
    change: FileChange,
    prefs: ChangesViewPrefs,
    folded: bool,
    on_menu: EventHandler<(String, bool, f64, f64)>,
) -> Element {
    let workspace = use_workspace();
    let path = change.file.path.clone();
    let deleted = change.file.status == ChangeStatus::Deleted;
    let (folder, name) = split_folder(&path);
    // The text of a file that was too large, once asked for.
    let mut loaded = use_signal(|| None::<Result<String, String>>);
    // The unchanged lines a click on a gap row read, by the hunk before the gap.
    let mut gaps = use_signal(HashMap::<usize, Vec<PatchLine>>::new);
    let text: Option<String> = match (&change.patch, loaded()) {
        (PatchState::Text(text), _) => Some(text.clone()),
        (_, Some(Ok(text))) => Some(text),
        _ => None,
    };
    let hunks = text.as_deref().map(parse_patch).unwrap_or_default();
    let (added, removed) = (change.file.added.unwrap_or(0), change.file.removed.unwrap_or(0));
    let wrap_class = if prefs.wrap { "cd-lines" } else { "cd-lines nowrap" };

    let body = if folded {
        rsx! {}
    } else {
        match (&change.patch, loaded()) {
            (PatchState::Binary, _) => rsx! { div { class: "cd-note", {tr!("No textual changes to show")} } },
            (PatchState::TooLarge, None) => {
                let (cwd, source, path) = (cwd.clone(), source.clone(), path.clone());
                rsx! {
                    div { class: "cd-note",
                        button {
                            class: "cd-gap",
                            r#type: "button",
                            onclick: move |_| {
                                let (git, cwd, source, path) = (workspace.git(), cwd.clone(), source.clone(), path.clone());
                                let options = DiffOptions { ignore_whitespace: workspace.changes_view_prefs().hide_whitespace };
                                spawn(async move {
                                    if let Some(result) = run_blocking(move || git.file_patch(&cwd, &source, &options, &path)).await {
                                        loaded.set(Some(result.map_err(|error| error.to_string())));
                                    }
                                });
                            },
                            {tr!("Load diff")}
                        }
                    }
                }
            }
            (PatchState::TooLarge, Some(Err(error))) => rsx! { div { class: "cd-note error", "{error}" } },
            _ if hunks.is_empty() => rsx! { div { class: "cd-note", {tr!("No textual changes to show")} } },
            _ => rsx! {
                div { class: "{wrap_class}",
                    for (index , hunk) in hunks.iter().enumerate() {
                        {hunk_view(hunk, &prefs)}
                        if let Some(lines) = gaps.read().get(&index) {
                            div { class: "cd-hunk",
                                for line in lines.iter() {
                                    {line_row(line, &[], false)}
                                }
                            }
                        } else if gap_after(&hunks, index).is_some() {
                            {
                                let (cwd, source, path, hunks) = (cwd.clone(), source.clone(), path.clone(), hunks.clone());
                                rsx! {
                                    button {
                                        class: "cd-gap",
                                        r#type: "button",
                                        onclick: move |_| {
                                            let (git, cwd, source, path, hunks) = (workspace.git(), cwd.clone(), source.clone(), path.clone(), hunks.clone());
                                            spawn(async move {
                                                let Some(read) = run_blocking(move || git.file_lines(&cwd, &source, &path)).await else { return };
                                                // A file that cannot be read has no more lines to show.
                                                let lines = gap_lines(&hunks, index, &read.unwrap_or_default());
                                                gaps.write().insert(index, lines);
                                            });
                                        },
                                        span { class: "cd-chevron", "⌄" }
                                        {tr!("Show more lines")}
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
    };

    rsx! {
        section { class: "cd-file", "data-diff-path": "{path}",
            div {
                class: "cd-file-head",
                role: "button",
                tabindex: "0",
                onclick: {
                    let path = path.clone();
                    move |_| workspace.update_changes_session(id, |session| toggle_fold(session, &path, folded))
                },
                oncontextmenu: {
                    let path = path.clone();
                    move |event: MouseEvent| {
                        event.prevent_default();
                        let point = event.client_coordinates();
                        on_menu.call((path.clone(), deleted, point.x, point.y));
                    }
                },
                span { class: "cd-chevron", if folded { "▸" } else { "▾" } }
                FileTypeIcon { icon: icon_of(&path) }
                span { class: "cd-name", "{name}" }
                if !folder.is_empty() {
                    span { class: "cd-folder", "{folder}" }
                }
                span { class: "cd-counts",
                    span { class: "cd-add", "+{added}" }
                    " "
                    span { class: "cd-del", "−{removed}" }
                }
            }
            {body}
        }
    }
}
```

`PatchHunk` must be `Clone` (it derives `Clone`). `ChangesSession` and `FileChange` need `PartialEq` for component props (they derive it). If the compiler asks for `ChangeSource: Clone` it derives it already.

Add to `ko.rs` (sorted): `("Load diff", "diff 불러오기")`, `("Show more lines", "더 많은 줄 보기")`, `("No textual changes to show", ...)` already exists.

- [ ] **Step 2: `changes_tree.rs`.**

```rust
use std::collections::HashSet;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_entities::icon_of;
use forge_features::{tree_rows, use_workspace, TreeRow, ROW_LIMIT};
use forge_ports::{ChangeStatus, ChangedFile};
use forge_shared::tr;

use crate::files_icons::FileTypeIcon;

/// The right column: the changed files as a folder tree (or flat), then whatever `commits` draws.
#[component]
pub fn ChangesTree(
    id: LiveSessionId,
    files: Vec<ChangedFile>,
    prefs: ChangesViewPrefs,
    closed: HashSet<String>,
    on_pick: EventHandler<String>,
    on_menu: EventHandler<(String, bool, f64, f64)>,
    commits: Element,
) -> Element {
    let workspace = use_workspace();
    let rows = tree_rows(&files, prefs.group_by_folder, &closed);
    let hidden = rows.len().saturating_sub(ROW_LIMIT);
    rsx! {
        div { class: "cd-side",
            ul { class: "cd-tree",
                for row in rows.into_iter().take(ROW_LIMIT) {
                    match row {
                        TreeRow::Folder { path, name, depth, open } => rsx! {
                            li { key: "dir:{path}",
                                button {
                                    class: "cd-row",
                                    r#type: "button",
                                    style: "padding-left: {depth * 14 + 6}px;",
                                    onclick: {
                                        let path = path.clone();
                                        move |_| {
                                            workspace.update_changes_session(id, |session| {
                                                if !session.closed_folders.remove(&path) {
                                                    session.closed_folders.insert(path.clone());
                                                }
                                            })
                                        }
                                    },
                                    span { class: "cd-chevron", if open { "▾" } else { "▸" } }
                                    span { class: "cd-name", "{name}" }
                                }
                            }
                        },
                        TreeRow::File { path, name, depth, added, removed, status, .. } => {
                            let deleted = status == ChangeStatus::Deleted;
                            let counts = match (added, removed) {
                                (Some(added), Some(removed)) => rsx! {
                                    span { class: "cd-counts",
                                        span { class: "cd-add", "+{added}" }
                                        " "
                                        span { class: "cd-del", "−{removed}" }
                                    }
                                },
                                _ => rsx! {},
                            };
                            rsx! {
                                li { key: "file:{path}",
                                    button {
                                        class: "cd-row",
                                        r#type: "button",
                                        title: "{path}",
                                        style: "padding-left: {depth * 14 + 6}px;",
                                        onclick: {
                                            let path = path.clone();
                                            move |_| on_pick.call(path.clone())
                                        },
                                        oncontextmenu: {
                                            let path = path.clone();
                                            move |event: MouseEvent| {
                                                event.prevent_default();
                                                let point = event.client_coordinates();
                                                on_menu.call((path.clone(), deleted, point.x, point.y));
                                            }
                                        },
                                        FileTypeIcon { icon: icon_of(&path) }
                                        span { class: "cd-name", "{name}" }
                                        {counts}
                                    }
                                }
                            }
                        }
                    }
                }
                if hidden > 0 {
                    li { class: "cd-note", {tr!("… {count} more files", count = hidden)} }
                }
            }
            {commits}
        }
    }
}
```

(`… {count} more files` already exists in `ko.rs`.)

- [ ] **Step 3: Rewrite `changes_panel.rs`.**

```rust
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{run_blocking, source_for, stack_order, use_workspace, FileAction, MenuTarget, Panel};
use forge_ports::{BranchInfo, ChangeSource, CommitInfo, DiffOptions, FileChange, GitError};

use crate::changes_stack::{scroll_to_diff, DiffStack};
use crate::changes_tree::ChangesTree;
use crate::context_menu::ContextMenu;
use crate::files_state::{absolute, copy_text_for};
use crate::panel_frame::PanelFrame;
use forge_shared::tr;

/// The Changes panel: every changed file as a stacked diff and a folder tree of the files at the
/// right. It is a viewer; it reloads on Refresh, when a tool call of the session finishes, and when
/// what is compared changes. See the spec `2026-10-17-changes-panel-design.md`.
#[component]
pub fn ChangesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut refresh = use_signal(|| 0u32);
    let mut list_open = use_signal(|| true);
    // ⇧⌘Y shows or hides the list.
    use_effect(move || {
        if workspace.changes_list_request() {
            workspace.clear_changes_list_request();
            let shown = *list_open.peek();
            list_open.set(!shown);
        }
    });
    let mut info = use_signal(|| None::<Result<BranchInfo, GitError>>);
    let mut commits = use_signal(|| None::<Vec<CommitInfo>>);
    let mut result = use_signal(|| None::<Result<Vec<FileChange>, GitError>>);
    // The base that was asked for and was not there; the list then shows the uncommitted changes.
    let mut missing_base = use_signal(|| None::<String>);
    // The count changes only when a tool call finishes; reading the session's view directly in an
    // effect would rerun it on every streamed chunk of a reply.
    let finished = use_memo(move || workspace.finished_tool_calls(id));
    // Each run takes a ticket; a result is kept only if no later run started meanwhile.
    let mut info_ticket = use_signal(|| 0u64);
    let mut changes_ticket = use_signal(|| 0u64);
    let mut commits_ticket = use_signal(|| 0u64);

    let session = workspace.changes_session(id);
    let prefs = workspace.changes_view_prefs();
    let base: Option<String> = session
        .base
        .clone()
        .or_else(|| workspace.worktree_base(&cwd))
        .or_else(|| info().and_then(|read| read.ok()).and_then(|branches| branches.default_base));
    let source = source_for(&session.view, base.as_deref());

    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = info_ticket.peek().wrapping_add(1);
            info_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.branches(&cwd)).await else { return };
                if *info_ticket.peek() == ticket {
                    info.set(Some(read));
                }
            });
        });
    }
    {
        let (cwd, source) = (cwd.clone(), source.clone());
        let hide = prefs.hide_whitespace;
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = changes_ticket.peek().wrapping_add(1);
            changes_ticket.set(ticket);
            let (git, cwd, source) = (workspace.git(), cwd.clone(), source.clone());
            let options = DiffOptions { ignore_whitespace: hide };
            spawn(async move {
                let Some((read, missing)) = run_blocking(move || match git.changes(&cwd, &source, &options) {
                    Err(GitError::BaseNotFound(base)) if !matches!(source, ChangeSource::Commit { .. }) => {
                        (git.changes(&cwd, &ChangeSource::Uncommitted, &options), Some(base))
                    }
                    other => (other, None),
                })
                .await
                else {
                    return;
                };
                if *changes_ticket.peek() == ticket {
                    missing_base.set(missing);
                    result.set(Some(read));
                }
            });
        });
    }
    {
        let (cwd, base) = (cwd.clone(), base.clone());
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = commits_ticket.peek().wrapping_add(1);
            commits_ticket.set(ticket);
            let Some(base) = base.clone() else {
                commits.set(None);
                return;
            };
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(read) = run_blocking(move || git.commits(&cwd, &base)).await else { return };
                if *commits_ticket.peek() == ticket {
                    commits.set(read.ok());
                }
            });
        });
    }

    let mut menu = use_signal(|| None::<(String, bool, f64, f64)>);

    let tools = rsx! {
        button {
            class: if list_open() { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: tr!("File list"),
            "aria-label": "File list",
            "aria-pressed": "{list_open()}",
            onclick: move |_| list_open.set(!list_open()),
            "☰"
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: tr!("Refresh"),
            "aria-label": "Refresh",
            onclick: move |_| refresh.set(refresh() + 1),
            "⟳"
        }
    };

    let menu_view = match menu() {
        Some((path, deleted, x, y)) => {
            let (cwd, run_path) = (cwd.clone(), path.clone());
            rsx! {
                ContextMenu {
                    entries: forge_features::file_menu(MenuTarget::Change { deleted }),
                    x,
                    y,
                    on_close: move |_| menu.set(None),
                    on_select: move |action: FileAction| {
                        menu.set(None);
                        if let Some(text) = copy_text_for(action, &cwd, &run_path) {
                            forge_entities::copy_text(&text);
                            return;
                        }
                        let full = absolute(&cwd, &run_path);
                        match action {
                            FileAction::Attach => workspace.queue_file(id, full),
                            FileAction::OpenFile => workspace.open_file(id, full.to_string_lossy().to_string()),
                            FileAction::OpenVsCode => workspace.open_with_app(&full, Some("Visual Studio Code")),
                            FileAction::OpenDefault => workspace.open_with_app(&full, None),
                            FileAction::Reveal => workspace.reveal(&full),
                            _ => {}
                        }
                    },
                }
            }
        }
        None => rsx! {},
    };

    let body = match result() {
        None => rsx! { div { class: "dock-note", {tr!("Loading…")} } },
        Some(Err(GitError::NotARepository)) => rsx! { div { class: "dock-note", {tr!("No changes to show")} } },
        Some(Err(error)) => rsx! { div { class: "dock-note error", "{error}" } },
        Some(Ok(files)) if files.is_empty() => rsx! { div { class: "dock-note", {tr!("No changes to show")} } },
        Some(Ok(files)) => {
            let listed: Vec<_> = files.iter().map(|change| change.file.clone()).collect();
            let order = stack_order(&listed, prefs.group_by_folder);
            let notice = missing_base().map(|name| tr!("Base branch {name} was not found; showing uncommitted changes", name = name));
            rsx! {
                div { class: "changes-body",
                    div { class: "cd-main",
                        if let Some(text) = notice {
                            div { class: "cd-note", "{text}" }
                        }
                        DiffStack {
                            id,
                            cwd: cwd.clone(),
                            source: source.clone(),
                            files: files.clone(),
                            order,
                            prefs: prefs.clone(),
                            session: session.clone(),
                            on_menu: move |target| menu.set(Some(target)),
                        }
                    }
                    if list_open() {
                        ChangesTree {
                            id,
                            files: listed,
                            prefs: prefs.clone(),
                            closed: session.closed_folders.clone(),
                            on_pick: move |path: String| scroll_to_diff(&path),
                            on_menu: move |target| menu.set(Some(target)),
                            commits: rsx! {},
                        }
                    }
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Changes, tools,
            {menu_view}
            {body}
        }
    }
}
```

If `workspace.queue_file`/`open_with_app`/`reveal` signatures differ from the calls above (`queue_file(id, PathBuf)`, `open_with_app(&Path, Option<&'static str>)`, `reveal(&Path)`; verified at plan time), follow the compiler. Add `mod changes_stack; mod changes_tree;` to `forge-widgets/src/lib.rs`. Add to `ko.rs`: `("Base branch {name} was not found; showing uncommitted changes", "기준 브랜치 {name}을(를) 찾지 못해 커밋되지 않은 변경 사항을 표시합니다")`.

- [ ] **Step 4: Build and run the unit tests**

Run: `cargo build -p forge-app && cargo test -p forge-widgets -p forge-shared`
Expected: PASS. The old `status`/`diff` are unused but still exist.

- [ ] **Step 5: Quick window check.** Follow `scripts/harness/README.md`: make a fixture repository (`git init -b main`, two files, a branch with a commit, an edited and an untracked file) as the fixture's session folder, launch with `scripts/harness/launch.sh`, open the `±` panel, and look at the capture: stacked diffs with numbers, the tree at the right, a click on a tree file scrolls, a header click folds. Fix what the window shows before going on.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: Changes panel shows stacked diffs and a folder tree from the new git data"
```

---

### Task 10: The branch button, its menus and the file search

**Files:**
- Create: `crates/forge-widgets/src/changes_header.rs`
- Modify: `crates/forge-widgets/src/changes_panel.rs` (put the header into `tools`, keep the panel's `position: relative` for the popover)
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `ChangeView`, `BranchInfo`, `CommitInfo`, `ChangedFile`, `filter_files`, `scroll_to_diff`.
- Produces: `BranchButton { id, info: Option<BranchInfo>, base: Option<String>, commits: Vec<CommitInfo>, view: ChangeView }` and `FileSearch { files: Vec<ChangedFile>, on_pick: EventHandler<String>, on_close: EventHandler<()> }`.

- [ ] **Step 1: A tested label function.** In `changes_header.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{filter_files, use_workspace, ChangeView};
use forge_ports::{BranchInfo, ChangedFile, CommitInfo};
use forge_shared::tr;

/// The text of the branch button.
pub(crate) fn button_label(view: &ChangeView, info: Option<&BranchInfo>, base: Option<&str>, commits: &[CommitInfo]) -> String {
    match view {
        ChangeView::Uncommitted => tr!("Uncommitted changes").to_string(),
        ChangeView::Commit(sha) => {
            let subject = commits.iter().find(|commit| &commit.sha == sha).map(|commit| commit.subject.as_str()).unwrap_or("");
            format!("{} {}", &sha[..sha.len().min(7)], subject).trim().to_string()
        }
        ChangeView::All => match (base, info) {
            (Some(base), Some(info)) => format!("{base} → {}", info.head_label),
            _ => tr!("Uncommitted changes").to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> BranchInfo {
        BranchInfo { current: Some("feature".to_string()), head_label: "feature".to_string(), default_base: Some("main".to_string()), branches: vec![] }
    }

    fn commit(sha: &str, subject: &str) -> CommitInfo {
        CommitInfo { sha: sha.to_string(), subject: subject.to_string(), author: "a".to_string(), time: 0 }
    }

    #[test]
    fn all_changes_name_the_base_and_the_branch() {
        assert_eq!(button_label(&ChangeView::All, Some(&info()), Some("main"), &[]), "main → feature");
    }

    #[test]
    fn all_changes_with_no_base_read_as_uncommitted() {
        assert_eq!(button_label(&ChangeView::All, Some(&info()), None, &[]), tr!("Uncommitted changes"));
    }

    #[test]
    fn a_commit_is_named_by_its_short_sha_and_subject() {
        let commits = vec![commit("d5bfbad1234", "Update a")];
        assert_eq!(button_label(&ChangeView::Commit("d5bfbad1234".to_string()), None, None, &commits), "d5bfbad Update a");
    }
}
```

- [ ] **Step 2: Run the label tests**

Run: `cargo test -p forge-widgets changes_header`
Expected: PASS.

- [ ] **Step 3: The components.** Append to `changes_header.rs`:

```rust
/// `base → branch ⌄`: a menu of the two modes and the base list. A pick is kept in the session.
#[component]
pub fn BranchButton(id: LiveSessionId, info: Option<BranchInfo>, base: Option<String>, commits: Vec<CommitInfo>, view: ChangeView) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let mut choosing_base = use_signal(|| false);
    let mut query = use_signal(String::new);
    let label = button_label(&view, info.as_ref(), base.as_deref(), &commits);
    let branches: Vec<String> = info
        .as_ref()
        .map(|info| info.branches.iter().filter(|name| query().is_empty() || name.to_lowercase().contains(&query().to_lowercase())).cloned().collect())
        .unwrap_or_default();
    let default_base = info.as_ref().and_then(|info| info.default_base.clone());
    rsx! {
        div { class: "cd-branch",
            button {
                class: "cd-head-button",
                r#type: "button",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    open.set(!open());
                    choosing_base.set(false);
                },
                span { "{label}" }
                span { class: "cd-chevron", "⌄" }
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div { class: "picker-menu cd-menu", role: "menu",
                    if choosing_base() {
                        input {
                            class: "cd-menu-search",
                            r#type: "text",
                            placeholder: tr!("Search branches"),
                            value: "{query}",
                            oninput: move |event| query.set(event.value()),
                            onmounted: move |mounted| async move {
                                let _ = mounted.set_focus(true).await;
                            },
                        }
                        for name in branches {
                            button {
                                key: "{name}",
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitem",
                                onclick: {
                                    let name = name.clone();
                                    move |_| {
                                        workspace.update_changes_session(id, |session| {
                                            session.base = Some(name.clone());
                                            session.view = ChangeView::All;
                                        });
                                        open.set(false);
                                    }
                                },
                                span { class: "picker-text", "{name}" }
                                if base.as_deref() == Some(name.as_str()) {
                                    span { class: "picker-hint", "✓" }
                                }
                                if default_base.as_deref() == Some(name.as_str()) {
                                    span { class: "picker-hint", {tr!("Default")} }
                                }
                            }
                        }
                    } else {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                workspace.update_changes_session(id, |session| session.view = ChangeView::All);
                                open.set(false);
                            },
                            span { class: "picker-text", {tr!("All changes")} }
                            if view == ChangeView::All {
                                span { class: "picker-hint", "✓" }
                            }
                            if let Some(name) = base.clone() {
                                span { class: "picker-hint", {tr!("vs {base}", base = name)} }
                            }
                        }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                workspace.update_changes_session(id, |session| session.view = ChangeView::Uncommitted);
                                open.set(false);
                            },
                            span { class: "picker-text", {tr!("Uncommitted changes")} }
                            if view == ChangeView::Uncommitted {
                                span { class: "picker-hint", "✓" }
                            }
                        }
                        div { class: "picker-divider" }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                query.set(String::new());
                                choosing_base.set(true);
                            },
                            span { class: "picker-text", {tr!("Compare to")} }
                            span { class: "picker-hint", "{base.clone().unwrap_or_default()} ›" }
                        }
                    }
                }
            }
        }
    }
}

/// The popover of the search icon: a field that filters the changed files; a pick jumps to the file.
#[component]
pub fn FileSearch(files: Vec<ChangedFile>, on_pick: EventHandler<String>, on_close: EventHandler<()>) -> Element {
    let mut query = use_signal(String::new);
    let shown = filter_files(&files, &query());
    rsx! {
        div { class: "picker-backdrop", onclick: move |_| on_close.call(()) }
        div {
            class: "cd-search",
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    on_close.call(());
                }
            },
            input {
                r#type: "text",
                placeholder: tr!("Search changed files"),
                value: "{query}",
                oninput: move |event| query.set(event.value()),
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
            }
            div { class: "cd-search-list",
                for index in shown {
                    {
                        let file = files[index].clone();
                        let (folder, name) = forge_features::split_folder(&file.path);
                        let path = file.path.clone();
                        rsx! {
                            button {
                                key: "{file.path}",
                                class: "cd-row",
                                r#type: "button",
                                onclick: move |_| {
                                    on_pick.call(path.clone());
                                    on_close.call(());
                                },
                                span { class: "cd-name", "{name}" }
                                if !folder.is_empty() {
                                    span { class: "cd-folder", "{folder}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

`ChangeView` needs `PartialEq` for the `view == ChangeView::All` comparisons and component props (it derives it). Add the `cd-menu` and `cd-menu-search` styles to `theme.css` next to the other `.cd-*` rules (`.cd-menu { min-width: 260px; max-height: 320px; overflow-y: auto; }` and `.cd-menu-search { width: 100%; box-sizing: border-box; padding: 6px 8px; border: none; border-bottom: 1px solid var(--border); background: transparent; color: var(--fg); font: inherit; font-size: 12px; outline: none; }`, plus `.cd-branch { position: relative; min-width: 0; }`).

`ko.rs` strings: `("All changes", "모든 변경사항")`, `("Compare to", "비교 대상")`, `("Default", "기본값")`, `("Search branches", "브랜치 검색")`, `("Search changed files", "변경된 파일 검색")`, `("Uncommitted changes", "커밋되지 않은 변경사항")`, `("vs {base}", "{base} 대비")`.

- [ ] **Step 4: Wire into the panel.** In `changes_panel.rs`: add `let mut searching = use_signal(|| false);`; put at the START of `tools`:

```rust
        BranchButton {
            id,
            info: info().and_then(|read| read.ok()),
            base: base.clone(),
            commits: commits().unwrap_or_default(),
            view: session.view.clone(),
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: tr!("Search changed files"),
            "aria-label": "Search changed files",
            onclick: move |_| searching.set(true),
            "⌕"
        }
```

and in `rsx!` after `{menu_view}`:

```rust
            if searching() {
                if let Some(Ok(files)) = result() {
                    FileSearch {
                        files: files.iter().map(|change| change.file.clone()).collect::<Vec<_>>(),
                        on_pick: move |path: String| scroll_to_diff(&path),
                        on_close: move |_| searching.set(false),
                    }
                }
            }
```

Add the imports (`use crate::changes_header::{BranchButton, FileSearch};`) and `mod changes_header;` in `lib.rs`. Make `.dock-panel` position relative in `theme.css` if it is not already (check; the search popover is `position: absolute`).

- [ ] **Step 5: Build, test, window check**

Run: `cargo build -p forge-app && cargo test -p forge-widgets -p forge-shared`
Expected: PASS. In the window: the button reads `main → feature`; its menu switches modes; `Compare to` opens a searchable branch list; a pick changes the list; the search icon opens the popover; a pick scrolls.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: branch button with mode and base menus, and the changed file search"
```

---

### Task 11: The `Commits N` section

**Files:**
- Modify: `crates/forge-widgets/src/changes_tree.rs` (a `CommitsSection` component) and `changes_panel.rs` (fill the `commits` slot)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Produces: `CommitsSection { id, commits: Vec<CommitInfo>, view: ChangeView }`; a helper `commit_meta(commit: &CommitInfo, now: OffsetDateTime) -> String` (tested).

- [ ] **Step 1: Write the failing test** in a `#[cfg(test)]` module of `changes_tree.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::CommitInfo;
    use time::OffsetDateTime;

    #[test]
    fn the_meta_line_has_the_short_sha_the_author_and_the_age() {
        let commit = CommitInfo { sha: "d5bfbad1234567".to_string(), subject: "s".to_string(), author: "myhency".to_string(), time: 1_000_000 };
        let now = OffsetDateTime::from_unix_timestamp(1_000_000 + 4 * 60).unwrap();
        assert_eq!(commit_meta(&commit, now), format!("d5bfbad · myhency · {}", tr!("{n}m ago", n = 4)));
    }
}
```

- [ ] **Step 2: Run to see failure**

Run: `cargo test -p forge-widgets the_meta_line`
Expected: FAIL to compile.

- [ ] **Step 3: Implement.** In `changes_tree.rs` add:

```rust
use forge_features::ChangeView;
use forge_ports::CommitInfo;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// `d5bfbad · author · 4m ago`.
pub(crate) fn commit_meta(commit: &CommitInfo, now: OffsetDateTime) -> String {
    let ago = OffsetDateTime::from_unix_timestamp(commit.time)
        .ok()
        .and_then(|then| then.format(&Rfc3339).ok())
        .map(|text| forge_entities::relative_time(&text, now))
        .unwrap_or_default();
    format!("{} · {} · {}", &commit.sha[..commit.sha.len().min(7)], commit.author, ago)
}

/// `Commits N ⌄`: the two modes and one row per commit of the branch.
#[component]
pub fn CommitsSection(id: LiveSessionId, commits: Vec<CommitInfo>, view: ChangeView) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| true);
    let now = OffsetDateTime::now_utc();
    rsx! {
        div { class: "cd-commits",
            div { class: "cd-commits-title", onclick: move |_| open.set(!open()),
                span { class: "cd-chevron", if open() { "▾" } else { "▸" } }
                span { {tr!("Commits")} }
                span { style: "margin-left: auto;", "{commits.len()}" }
            }
            if open() {
                button {
                    class: if view == ChangeView::All { "cd-commit selected" } else { "cd-commit" },
                    r#type: "button",
                    onclick: move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::All),
                    span { class: "subject", {tr!("All changes")} }
                }
                button {
                    class: if view == ChangeView::Uncommitted { "cd-commit selected" } else { "cd-commit" },
                    r#type: "button",
                    onclick: move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::Uncommitted),
                    span { class: "subject", {tr!("Uncommitted changes")} }
                }
                for commit in commits.iter() {
                    button {
                        key: "{commit.sha}",
                        class: if view == ChangeView::Commit(commit.sha.clone()) { "cd-commit selected" } else { "cd-commit" },
                        r#type: "button",
                        title: "{commit.subject}",
                        onclick: {
                            let sha = commit.sha.clone();
                            move |_| workspace.update_changes_session(id, |session| session.view = ChangeView::Commit(sha.clone()))
                        },
                        span { class: "subject", "{commit.subject}" }
                        span { class: "meta", "{commit_meta(commit, now)}" }
                    }
                }
            }
        }
    }
}
```

Is `relative_time` exported from `forge_entities`? It is `pub fn relative_time` in `relative_time.rs`; check `forge-entities/src/lib.rs` for its `pub use`, and use that path. In `changes_panel.rs` replace `commits: rsx! {}` with:

```rust
                            commits: if let Some(list) = commits() {
                                rsx! { CommitsSection { id, commits: list, view: session.view.clone() } }
                            } else {
                                rsx! {}
                            },
```

(import `CommitsSection`). `ko.rs`: `("Commits", "커밋")`.

- [ ] **Step 4: Run tests and check the window**

Run: `cargo test -p forge-widgets && cargo build -p forge-app`
Expected: PASS. In the window: the section appears at the foot of the right column when the base exists; a commit row shows that commit's files and the button names it; `All changes` goes back.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: Commits section in the Changes panel's right column"
```

---

### Task 12: The `⋮` menu with the viewing options

**Files:**
- Modify: `crates/forge-widgets/src/changes_header.rs` (a `ViewMenu` component)
- Modify: `crates/forge-widgets/src/changes_panel.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Produces: `ViewMenu { id, prefs: ChangesViewPrefs, paths: Vec<String>, list_open: bool, on_toggle_list: EventHandler<()>, on_refresh: EventHandler<()> }`.

- [ ] **Step 1: Implement.** Append to `changes_header.rs`:

```rust
use forge_domain::ChangesViewPrefs;
use forge_features::{fold_all, unfold_all};

fn check_item(label: &'static str, on: bool, onclick: impl FnMut(MouseEvent) + 'static) -> Element {
    rsx! {
        button { class: "picker-item", r#type: "button", role: "menuitemcheckbox", "aria-checked": "{on}", onclick,
            span { class: "picker-text", "{label}" }
            if on {
                span { class: "picker-hint", "✓" }
            }
        }
    }
}

/// The panel's `⋮`: what the diff looks like, fold all, refresh. A click on a check item does not
/// close the menu, as in the desktop app.
#[component]
pub fn ViewMenu(id: LiveSessionId, prefs: ChangesViewPrefs, paths: Vec<String>, list_open: bool, on_toggle_list: EventHandler<()>, on_refresh: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let (fold_paths, unfold_paths) = (paths.clone(), paths);
    rsx! {
        div { class: "cd-branch",
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("More"),
                "aria-label": "More",
                "aria-haspopup": "menu",
                onclick: move |_| open.set(!open()),
                "⋮"
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                div { class: "picker-menu cd-menu", role: "menu", style: "right: 0; left: auto;",
                    {check_item(tr!("Show files"), list_open, move |_| on_toggle_list.call(()))}
                    {check_item(tr!("Group files by folder"), prefs.group_by_folder, move |_| workspace.update_changes_view_prefs(|p| p.group_by_folder = !p.group_by_folder))}
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            workspace.update_changes_session(id, |session| fold_all(session, &fold_paths));
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Collapse all files")} }
                    }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            workspace.update_changes_session(id, |session| unfold_all(session, &unfold_paths));
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Expand all files")} }
                    }
                    div { class: "picker-divider" }
                    {check_item(tr!("Side by side"), prefs.side_by_side, move |_| workspace.update_changes_view_prefs(|p| p.side_by_side = !p.side_by_side))}
                    {check_item(tr!("Wrap lines"), prefs.wrap, move |_| workspace.update_changes_view_prefs(|p| p.wrap = !p.wrap))}
                    {check_item(tr!("Highlight changed words"), prefs.highlight_words, move |_| workspace.update_changes_view_prefs(|p| p.highlight_words = !p.highlight_words))}
                    {check_item(tr!("Hide whitespace changes"), prefs.hide_whitespace, move |_| workspace.update_changes_view_prefs(|p| p.hide_whitespace = !p.hide_whitespace))}
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: move |_| {
                            on_refresh.call(());
                            open.set(false);
                        },
                        span { class: "picker-text", {tr!("Refresh")} }
                    }
                }
            }
        }
    }
}
```

`Escape` closes it: add `onkeydown` like `FileSearch` if the existing picker pattern does not already do it (check how `filter_menu.rs` closes on Escape and copy that exactly). In `changes_panel.rs` replace the old `☰` and `⟳` buttons in `tools` with:

```rust
        ViewMenu {
            id,
            prefs: prefs.clone(),
            paths: result().and_then(|read| read.ok()).map(|files| files.iter().map(|change| change.file.path.clone()).collect()).unwrap_or_default(),
            list_open: list_open(),
            on_toggle_list: move |_| list_open.set(!list_open()),
            on_refresh: move |_| refresh.set(refresh() + 1),
        }
```

`ko.rs` strings: `("Collapse all files", "모든 파일 접기")`, `("Expand all files", "모든 파일 펼치기")`, `("Group files by folder", "폴더별 파일 그룹화")`, `("Hide whitespace changes", "공백 변경 숨기기")`, `("Highlight changed words", "변경된 단어 강조")`, `("Show files", "파일 표시")`, `("Side by side", "나란히")`, `("Wrap lines", "자동 줄 바꿈")`; `More` and `Refresh` exist (check; add if the i18n test says so).

- [ ] **Step 2: Build, test, window check**

Run: `cargo test -p forge-widgets -p forge-shared && cargo build -p forge-app`
Expected: PASS. In the window: each switch changes the picture at once; `Hide whitespace changes` reloads the list (a whitespace-only file leaves); the options survive a restart (`~/.forge` config or the harness config path shows `changes_view`); Collapse/Expand all work.

- [ ] **Step 3: Commit**

```bash
git add crates
git commit -m "feat: Changes panel options menu: grouping, fold all, side by side, wrap, word highlight, whitespace"
```

---

### Task 13: Remove the old read-only calls

**Files:**
- Modify: `crates/forge-ports/src/git_port.rs` (remove `status` and `diff` from the trait)
- Modify: `crates/forge-adapter-git/src/lib.rs` (remove `status`, `diff`, `parse_status`, `parse_numstat`, `status_of`, their tests)

- [ ] **Step 1: Find the users.** Run `grep -rn "\.status(\|\.diff(" crates --include=*.rs | grep -v "adapter-git\|git_port"`. Expected: none (the panel uses `changes` now).

- [ ] **Step 2: Port the robustness tests.** For each old adapter test below, change `GitCli::new().status(path)` to `GitCli::new().changes(path, &ChangeSource::Uncommitted, &DiffOptions::default())` and compare `.file` fields; keep the test bodies otherwise:
  `a_clean_repository_has_no_changes`, `modified_added_deleted_and_untracked_files_are_listed_with_counts`, `names_with_spaces_and_non_ascii_characters_and_renames_are_read` (a rename is now a deleted plus an added file: change the expectation, and say so in the test name: `a_rename_shows_as_a_deleted_and_an_added_file`), `untracked_files_inside_a_new_folder_are_listed_one_by_one`, `a_path_with_glob_characters_is_a_name_not_a_pattern` (check the patch text is non-empty), `a_git_that_hangs_is_stopped_at_the_timeout`, `a_folder_that_is_gone_is_not_reported_as_a_missing_git`. Delete the tests that only read `diff` output (`the_diff_of_*`, `a_path_that_looks_like_an_option_is_only_a_path`): `changes` takes no path from the caller, and `file_patch` gets one test:

```rust
    #[test]
    fn a_file_patch_of_an_option_like_name_is_only_a_patch() {
        let dir = repo();
        write(dir.path(), "-n.txt", "x\n");
        let patch = GitCli::new().file_patch(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default(), "-n.txt").unwrap();
        assert!(patch.contains("+x"));
    }
```

- [ ] **Step 3: Remove the code and run everything**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: PASS. (If the project's clippy config is not `-D warnings`, run `cargo clippy --workspace --all-targets` and fix new warnings only.)

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "refactor: drop the one-file status and diff calls the Changes panel no longer uses"
```

---

### Task 14: Documents, then the real window

**Files:**
- Modify: `docs/superpowers/specs/2026-10-17-changes-panel-design.md` (the amendments)
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-02 row, README order table)
- Modify: `docs/superpowers/roadmap/observations.md` (what the window showed)

- [ ] **Step 1: Amend the spec** with an "As built" section listing every difference: `FileDiff` unchanged and the `Patch*` types instead; `--no-renames` (a rename shows as deleted plus added); the branch button sits in the panel header's tools beside the title `Changes` (the desktop app has no title); the search popover; the base default order (worktree record, `origin/HEAD`, `main`, `master`); fold state in `ChangesSession`; what was checked and what was not.

- [ ] **Step 2: Real window checks** (`scripts/harness/README.md`; Korean UI). Use a fixture repository with `main`, a `claude/x` branch with two commits, an edited file, an untracked file, a file in a folder, a whitespace-only change, a 3 MB generated file and a binary file. Check and record each:
  1. Header `main → claude/x ⌄`; its menu (both modes, `Compare to` list with search and the default badge); a pick changes the list.
  2. The right column tree and flat list; a folder click folds; a file click scrolls the stack; the `Commits N` section; a commit row shows its diff and the button names it; `All changes` returns.
  3. Folding by header click; collapse all; expand all; the 3 MB file shows `Load diff` and loads on click; the binary file note; `Show more lines` after a hunk.
  4. Side by side, wrap off (long line scrolls sideways), word highlight on and off, hide whitespace (the whitespace-only file leaves).
  5. The search popover; Escape closes it; a pick scrolls.
  6. The row menu on a tree row and on a header: Attach (a chip appears in the composer), Copy, Open file (Files panel), Open in, Show in Finder; a deleted file's menu has only Copy.
  7. A missing base (rename the base branch while the panel is open, then Refresh): the notice line and the uncommitted list.
  8. Narrow window (Plan 26 layout) with the panel open; dark mode looks right.
  9. Reopen the app: the `⋮` options are kept.
  10. A comparison capture of the desktop app for the same repository (`scripts/observe/claude_app.py`) beside Forge's.

  Anything that fails is fixed in a new commit before this step is ticked.

- [ ] **Step 3: Record.** In `observations.md` add what step 10 showed that differs. In `backlog.md` set PNL-02's Plan cell to `Plan 21B (built); Plan 48 (built)` and add to its notes what is still not built (see the spec's "Not built"); add the README tentative-order row `| 48 | Changes panel parity (built) | PNL-02 | Spec 2026-10-17-changes-panel-design.md |`. Add the memory file `plan-48-changes-panel.md` and its line in `MEMORY.md`.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: Plan 48 built: Changes panel parity (PNL-02)"
```

---

## Self-Review

**Spec coverage:** header and branch button with both modes and `Compare to` (Task 10); commits list and a commit's diff (Tasks 5, 10, 11); stacked, foldable diffs with numbers (Tasks 2, 9); `More lines` gaps (Tasks 2, 9); tree, flat list and folder text (Tasks 6, 9); viewing options and persistence (Tasks 1, 7, 12); search popover (Task 10); row menu (Tasks 7, 9); errors, missing base, detached HEAD, binary, too large, `-w` (Tasks 5, 9); 500-row and 50-section caps (Tasks 6, 9); strings in Korean (each task); window checks (Task 14). Left out as the spec says: `File list position`, test/build/generated split, `Commits ›` submenu, long `Copy ›` list, commit button.

**Placeholder scan:** none. Two spots tell the implementer to follow the compiler where an external signature was verified at plan time (`queue_file`, `open_with_app`, `reveal`; the `relative_time` re-export path); the checks and expected types are named.

**Type consistency:** `ChangeSource`, `DiffOptions`, `FileChange`, `PatchState`, `BranchInfo`, `CommitInfo` are defined in Task 3 and used with the same names in Tasks 5, 9, 10, 11. `ChangeView`/`ChangesSession` (Task 6) are used in Tasks 7, 9, 10, 11, 12 with the fields defined there (`view`, `base`, `folded_files`, `expanded_files`, `closed_folders`). `PatchHunk`/`PatchLine`/`Span` (Task 2) match the widget code in Task 9 (`side_by_side` returns indexes, as used). `MenuTarget::Change { deleted }` and `FileAction::OpenFile` (Task 7) match Task 9's menu code.

**Review Focus coverage:** each line has a named test in Tasks 2, 5, 6 or a window check in Task 14.
