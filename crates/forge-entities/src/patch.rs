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
/// up to the end of the file (`u32::MAX`, the file decides) unless the hunk already reaches it. `None`
/// when nothing lies between, and for a file that was removed.
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
        // The diff is made with 3 lines of context (`-U3`, the git adapter's context size): a last
        // hunk with fewer trailing context lines has reached the end of the file, so nothing is
        // left to show below it.
        None => {
            let trailing = hunk.lines.iter().rev().take_while(|line| line.kind == DiffLineKind::Context).count();
            (trailing >= 3).then_some((from, u32::MAX))
        }
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

/// The hunks cut to their first `limit` lines in all (a hunk cut in the middle keeps its first
/// lines; the hunks after the cut are dropped) and how many lines were left out. A web view stops
/// laying out lines past about 65,536 children, so a very large diff is drawn a part at a time.
pub fn cap_lines(hunks: &[PatchHunk], limit: usize) -> (Vec<PatchHunk>, usize) {
    let mut left = limit;
    let mut shown = Vec::new();
    let mut hidden = 0;
    for hunk in hunks {
        let take = hunk.lines.len().min(left);
        hidden += hunk.lines.len() - take;
        if take > 0 {
            left -= take;
            shown.push(PatchHunk { old_start: hunk.old_start, new_start: hunk.new_start, lines: hunk.lines[..take].to_vec() });
        }
    }
    (shown, hidden)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffLineKind::{Added, Context, Removed};

    const PATCH: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -2,3 +2,4 @@ fn x\n b\n-c\n+C\n+d\n e\n@@ -20,2 +21,2 @@\n y\n-z\n+Z\n";

    // The same, but the last hunk keeps 3 lines of context after its change: the file goes on.
    const PATCH_MORE: &str = "@@ -2,3 +2,4 @@\n b\n-c\n+C\n+d\n e\n@@ -20,5 +21,5 @@\n y\n-z\n+Z\n t1\n t2\n t3\n";

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
        let hunks = parse_patch(PATCH_MORE);
        assert_eq!(gap_after(&hunks, 1), Some((26, u32::MAX)));
    }

    #[test]
    fn a_last_hunk_that_reaches_the_end_has_no_gap() {
        // No trailing context, and two lines of it: the change is at the end of the file.
        assert_eq!(gap_after(&parse_patch(PATCH), 1), None);
        let hunks = parse_patch("@@ -20,4 +21,4 @@\n y\n-z\n+Z\n t1\n t2\n");
        assert_eq!(gap_after(&hunks, 0), None);
        let created = parse_patch("@@ -0,0 +1,2 @@\n+one\n+two\n");
        assert_eq!(gap_after(&created, 0), None);
        let three = parse_patch("@@ -20,5 +21,5 @@\n y\n-z\n+Z\n t1\n t2\n t3\n");
        assert_eq!(gap_after(&three, 0), Some((26, u32::MAX)));
    }

    #[test]
    fn a_line_cap_cuts_inside_a_hunk_and_counts_what_it_left_out() {
        let hunks = parse_patch(PATCH);
        // 5 + 3 lines; a cap of 6 keeps hunk 0 and the first line of hunk 1.
        let (shown, hidden) = cap_lines(&hunks, 6);
        assert_eq!(shown.len(), 2);
        assert_eq!(shown[1].lines.len(), 1);
        assert_eq!(shown[1].new_start, 21);
        assert_eq!(hidden, 2);
        let (shown, hidden) = cap_lines(&hunks, 3);
        assert_eq!((shown.len(), shown[0].lines.len(), hidden), (1, 3, 5));
        assert_eq!(cap_lines(&hunks, 100), (hunks.clone(), 0));
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
        let hunks = parse_patch(PATCH_MORE);
        let file: Vec<String> = (1..=27).map(|n| format!("line {n}")).collect();
        let lines = gap_lines(&hunks, 1, &file);
        assert_eq!(lines.iter().map(|l| l.new_no.unwrap()).collect::<Vec<_>>(), vec![26, 27]);
        // A file that ends inside the hunk has no more lines.
        let short: Vec<String> = (1..=24).map(|n| format!("line {n}")).collect();
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
