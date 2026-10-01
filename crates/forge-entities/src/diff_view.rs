use dioxus::prelude::*;

use crate::diff::{DiffLineKind, FileDiff};

pub(crate) fn diff_line_class(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => "context",
        DiffLineKind::Added => "added",
        DiffLineKind::Removed => "removed",
    }
}

pub(crate) fn diff_line_sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    }
}

pub(crate) fn diff_counts_label(diff: &FileDiff) -> String {
    format!("+{} −{}", diff.added, diff.removed)
}

/// A file diff: a header with the path and counts, then the lines of each hunk with a `+` / `-`
/// sign. Shown under a tool call in the chat and inside a permission request.
#[component]
pub fn DiffView(diff: FileDiff) -> Element {
    let counts = diff_counts_label(&diff);
    rsx! {
        div { class: "diff",
            div { class: "diff-header",
                span { class: "diff-path", "{diff.path}" }
                if diff.new_content {
                    span { class: "diff-note", "new content" }
                }
                span { class: "diff-counts", "{counts}" }
            }
            for (index, hunk) in diff.hunks.iter().enumerate() {
                div { key: "{index}", class: "diff-hunk",
                    for (line_index, line) in hunk.iter().enumerate() {
                        div { key: "{line_index}", class: "diff-line {diff_line_class(line.kind)}",
                            span { class: "diff-sign", "{diff_line_sign(line.kind)}" }
                            span { class: "diff-text", "{line.text}" }
                        }
                    }
                }
            }
            if diff.hidden_lines > 0 {
                div { class: "diff-more", "… {diff.hidden_lines} more lines" }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::FileDiff;

    #[test]
    fn each_line_kind_has_a_class_and_a_sign() {
        assert_eq!(diff_line_class(DiffLineKind::Context), "context");
        assert_eq!(diff_line_class(DiffLineKind::Added), "added");
        assert_eq!(diff_line_class(DiffLineKind::Removed), "removed");
        assert_eq!(diff_line_sign(DiffLineKind::Context), " ");
        assert_eq!(diff_line_sign(DiffLineKind::Added), "+");
        assert_eq!(diff_line_sign(DiffLineKind::Removed), "-");
    }

    #[test]
    fn the_counts_label_uses_the_untruncated_totals() {
        let diff = FileDiff {
            path: "f".to_string(),
            new_content: false,
            added: 3,
            removed: 1,
            hunks: vec![],
            hidden_lines: 0,
        };
        assert_eq!(diff_counts_label(&diff), "+3 −1");
    }
}
