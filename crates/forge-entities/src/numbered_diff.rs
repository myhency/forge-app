//! The change an Edit, MultiEdit or Write call would make, placed in the file it changes: line
//! numbers, a few unchanged lines around each change, and the changed words marked. The desktop app's
//! edit permission card shows a file change this way (round 37); `FileDiff` has only the edit's own
//! text, because the tool input does not say where in the file it lands.

use std::time::Duration;

use dioxus::prelude::*;
use serde_json::{Map, Value};
use similar::{ChangeTag, TextDiff};

use crate::diff::DiffLineKind;
use crate::diff_view::diff_line_class;
use crate::highlight::{code_html, language_of};

/// Unchanged lines shown before and after each change.
const CONTEXT_LINES: usize = 3;
/// At most this many lines in total; the rest is summarised.
const MAX_LINES: usize = 300;
const DIFF_DEADLINE: Duration = Duration::from_millis(100);

/// One line of a numbered diff. `segments` are the line's text in pieces; `true` marks a piece that
/// changed (a removed or added word), which the card highlights more strongly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedLine {
    pub kind: DiffLineKind,
    /// The line's number: in the new file for unchanged and added lines, in the old file for removed ones.
    pub number: usize,
    pub segments: Vec<(bool, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NumberedDiff {
    pub path: String,
    pub hunks: Vec<Vec<NumberedLine>>,
    pub hidden_lines: usize,
}

/// The diff of what the tool call would do to `current` (the file's text now, `None` when it does not
/// exist). `None` when the call is not a file edit, or an edit's `old_string` is not in the file (it
/// changed since, or the input is incomplete): the caller falls back to the plain diff.
pub fn numbered_diff_for_tool(name: &str, input: &Value, current: Option<&str>) -> Option<NumberedDiff> {
    let object = input.as_object()?;
    let path = object.get("file_path")?.as_str()?.to_string();
    let old = current.unwrap_or("");
    let new = match name {
        "Edit" => apply_edit(current?, object)?,
        "MultiEdit" => {
            let mut text = current?.to_string();
            for edit in object.get("edits")?.as_array()? {
                text = apply_edit(&text, edit.as_object()?)?;
            }
            text
        }
        "Write" => object.get("content")?.as_str()?.to_string(),
        _ => return None,
    };
    Some(numbered_diff(path, old, &new))
}

fn apply_edit(text: &str, edit: &Map<String, Value>) -> Option<String> {
    let old = edit.get("old_string")?.as_str()?;
    let new = edit.get("new_string")?.as_str()?;
    if old.is_empty() || !text.contains(old) {
        return None;
    }
    let all = edit.get("replace_all").and_then(Value::as_bool).unwrap_or(false);
    Some(if all { text.replace(old, new) } else { text.replacen(old, new, 1) })
}

/// The line diff of `old` → `new`, grouped into hunks with `CONTEXT_LINES` around each change.
pub fn numbered_diff(path: String, old: &str, new: &str) -> NumberedDiff {
    let diff = TextDiff::configure().timeout(DIFF_DEADLINE).diff_lines(old, new);
    let mut hunks = Vec::new();
    let mut shown = 0;
    let mut hidden_lines = 0;
    for group in diff.grouped_ops(CONTEXT_LINES) {
        let mut hunk = Vec::new();
        for op in &group {
            for change in diff.iter_inline_changes(op) {
                if shown >= MAX_LINES {
                    hidden_lines += 1;
                    continue;
                }
                let (kind, index) = match change.tag() {
                    ChangeTag::Equal => (DiffLineKind::Context, change.new_index()),
                    ChangeTag::Insert => (DiffLineKind::Added, change.new_index()),
                    ChangeTag::Delete => (DiffLineKind::Removed, change.old_index()),
                };
                let mut segments: Vec<(bool, String)> = change
                    .iter_strings_lossy()
                    .map(|(emphasized, text)| (emphasized, text.into_owned()))
                    .collect();
                trim_line_end(&mut segments);
                // A line where every piece changed has nothing to set apart.
                if segments.iter().all(|(emphasized, _)| *emphasized) {
                    for segment in &mut segments {
                        segment.0 = false;
                    }
                }
                hunk.push(NumberedLine { kind, number: index.map_or(0, |i| i + 1), segments });
                shown += 1;
            }
        }
        if !hunk.is_empty() {
            hunks.push(hunk);
        }
    }
    NumberedDiff { path, hunks, hidden_lines }
}

// The last piece carries the line break; the card draws one line per row.
fn trim_line_end(segments: &mut Vec<(bool, String)>) {
    while let Some(last) = segments.last_mut() {
        let trimmed = last.1.trim_end_matches(['\n', '\r']).len();
        last.1.truncate(trimmed);
        if last.1.is_empty() {
            segments.pop();
        } else {
            break;
        }
    }
}

/// A line's HTML: syntax colours for the file's language, and changed words wrapped in
/// `<span class="diff-word">`.
pub(crate) fn line_html(language: &str, line: &NumberedLine) -> String {
    let changed = line.segments.iter().any(|(emphasized, _)| *emphasized);
    if !changed {
        let text: String = line.segments.iter().map(|(_, text)| text.as_str()).collect();
        return code_html(language, &text);
    }
    line.segments
        .iter()
        .map(|(emphasized, text)| {
            let html = code_html(language, text);
            if *emphasized {
                format!("<span class=\"diff-word\">{html}</span>")
            } else {
                html
            }
        })
        .collect()
}

/// The numbered diff as the desktop app's edit card draws it: one row per line with a coloured
/// gutter mark, the line number and the coloured text; a dashed rule between hunks.
#[component]
pub fn NumberedDiffView(diff: NumberedDiff) -> Element {
    let language = language_of(&diff.path).to_string();
    rsx! {
        div { class: "ndiff",
            for (index, hunk) in diff.hunks.iter().enumerate() {
                div { key: "{index}", class: "ndiff-hunk",
                    for (line_index, line) in hunk.iter().enumerate() {
                        div { key: "{line_index}", class: "ndiff-line {diff_line_class(line.kind)}",
                            span { class: "ndiff-mark" }
                            span { class: "ndiff-number", "{line.number}" }
                            span { class: "ndiff-text", dangerous_inner_html: "{line_html(&language, line)}" }
                        }
                    }
                }
            }
            if diff.hidden_lines > 0 {
                div { class: "diff-more", {forge_shared::tr!("… {count} more lines", count = diff.hidden_lines)} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn texts(line: &NumberedLine) -> String {
        line.segments.iter().map(|(_, text)| text.as_str()).collect()
    }

    const FILE: &str = "def mean(xs):\n    return sum(xs) / (len(xs) - 1)\n\n\ndef median(xs):\n    s = sorted(xs)\n";

    #[test]
    fn an_edit_is_placed_in_the_file_with_numbers_and_context() {
        let input = json!({"file_path": "/p/stats.py", "old_string": "(len(xs) - 1)", "new_string": "len(xs)"});
        let diff = numbered_diff_for_tool("Edit", &input, Some(FILE)).unwrap();
        assert_eq!(diff.hunks.len(), 1);
        let rows: Vec<(DiffLineKind, usize, String)> = diff.hunks[0].iter().map(|l| (l.kind, l.number, texts(l))).collect();
        assert_eq!(rows[0], (DiffLineKind::Context, 1, "def mean(xs):".to_string()));
        assert_eq!(rows[1], (DiffLineKind::Removed, 2, "    return sum(xs) / (len(xs) - 1)".to_string()));
        assert_eq!(rows[2], (DiffLineKind::Added, 2, "    return sum(xs) / len(xs)".to_string()));
        // Three lines of context after the change, then the hunk ends.
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[5], (DiffLineKind::Context, 5, "def median(xs):".to_string()));
    }

    #[test]
    fn the_changed_words_are_marked_and_the_rest_is_not() {
        let input = json!({"file_path": "/p/stats.py", "old_string": "(len(xs) - 1)", "new_string": "len(xs)"});
        let diff = numbered_diff_for_tool("Edit", &input, Some(FILE)).unwrap();
        let removed = &diff.hunks[0][1];
        assert!(removed.segments.iter().any(|(e, _)| *e));
        assert!(removed.segments.iter().any(|(e, t)| !*e && t.contains("return")));
    }

    #[test]
    fn an_edit_whose_text_is_gone_gives_none() {
        let input = json!({"file_path": "/p/stats.py", "old_string": "not there", "new_string": "x"});
        assert_eq!(numbered_diff_for_tool("Edit", &input, Some(FILE)), None);
        assert_eq!(numbered_diff_for_tool("Edit", &input, None), None);
    }

    #[test]
    fn a_write_to_a_new_file_is_all_added_from_line_one() {
        let input = json!({"file_path": "/p/summary.md", "content": "# a\nb\n"});
        let diff = numbered_diff_for_tool("Write", &input, None).unwrap();
        let rows: Vec<(DiffLineKind, usize)> = diff.hunks[0].iter().map(|l| (l.kind, l.number)).collect();
        assert_eq!(rows, vec![(DiffLineKind::Added, 1), (DiffLineKind::Added, 2)]);
        // A wholly new line has nothing to set apart.
        assert!(diff.hunks[0].iter().all(|l| l.segments.iter().all(|(e, _)| !*e)));
    }

    #[test]
    fn replace_all_and_multi_edit_apply_every_change() {
        let all = json!({"file_path": "/p/a", "old_string": "xs", "new_string": "ys", "replace_all": true});
        let diff = numbered_diff_for_tool("Edit", &all, Some(FILE)).unwrap();
        let added = diff.hunks.iter().flatten().filter(|l| l.kind == DiffLineKind::Added).count();
        // `xs` is on lines 1, 2, 5 and 6.
        assert_eq!(added, 4);
        let multi = json!({"file_path": "/p/a", "edits": [
            {"old_string": "def mean", "new_string": "def average"},
            {"old_string": "def median", "new_string": "def middle"}
        ]});
        let diff = numbered_diff_for_tool("MultiEdit", &multi, Some(FILE)).unwrap();
        let added: Vec<usize> = diff.hunks.iter().flatten().filter(|l| l.kind == DiffLineKind::Added).map(|l| l.number).collect();
        assert_eq!(added, vec![1, 5]);
    }

    #[test]
    fn other_tools_give_none() {
        assert_eq!(numbered_diff_for_tool("Bash", &json!({"command": "ls"}), None), None);
    }

    #[test]
    fn a_line_html_wraps_changed_words() {
        let line = NumberedLine { kind: DiffLineKind::Removed, number: 1, segments: vec![(false, "a ".into()), (true, "b".into())] };
        let html = line_html("", &line);
        assert_eq!(html, "a <span class=\"diff-word\">b</span>");
    }
}
