use forge_domain::ToolCall;
use serde_json::{Map, Value};
use similar::{ChangeTag, TextDiff};

// A diff shows at most this many lines in total; the rest is summarised as "… N more lines".
const MAX_DIFF_LINES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// What an Edit, MultiEdit or Write tool call changes. There are no line numbers: the tool
/// input does not say where in the file the change lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    /// A Write: the content is all there is to show; the input does not say what the file held before.
    pub new_content: bool,
    /// Changed lines before truncation.
    pub added: usize,
    pub removed: usize,
    /// One hunk per edit, at most `MAX_DIFF_LINES` lines in total.
    pub hunks: Vec<Vec<DiffLine>>,
    pub hidden_lines: usize,
}

pub fn diff_for_tool_call(tool_call: &ToolCall) -> Option<FileDiff> {
    diff_for_tool(&tool_call.name, &tool_call.input)
}

/// `None` for any other tool and for input that lacks the expected string fields.
pub fn diff_for_tool(name: &str, input: &Value) -> Option<FileDiff> {
    let object = input.as_object()?;
    let path = object.get("file_path")?.as_str()?.to_string();
    let (mut hunks, new_content) = match name {
        "Edit" => (vec![edit_hunk(object)?], false),
        "MultiEdit" => {
            let mut hunks = Vec::new();
            for edit in object.get("edits")?.as_array()? {
                hunks.push(edit_hunk(edit.as_object()?)?);
            }
            (hunks, false)
        }
        "Write" => (vec![line_diff("", string_field(object, "content")?)], true),
        _ => return None,
    };
    let count = |kind| hunks.iter().flatten().filter(|line| line.kind == kind).count();
    let (added, removed) = (count(DiffLineKind::Added), count(DiffLineKind::Removed));
    let hidden_lines = truncate_hunks(&mut hunks, MAX_DIFF_LINES);
    Some(FileDiff { path, new_content, added, removed, hunks, hidden_lines })
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key)?.as_str()
}

fn edit_hunk(object: &Map<String, Value>) -> Option<Vec<DiffLine>> {
    Some(line_diff(string_field(object, "old_string")?, string_field(object, "new_string")?))
}

fn line_diff(old: &str, new: &str) -> Vec<DiffLine> {
    TextDiff::from_lines(old, new)
        .iter_all_changes()
        .map(|change| DiffLine {
            kind: match change.tag() {
                ChangeTag::Equal => DiffLineKind::Context,
                ChangeTag::Insert => DiffLineKind::Added,
                ChangeTag::Delete => DiffLineKind::Removed,
            },
            text: change.value().trim_end_matches(['\n', '\r']).to_string(),
        })
        .collect()
}

// Keeps the first `max` lines across all hunks and returns how many were cut.
fn truncate_hunks(hunks: &mut Vec<Vec<DiffLine>>, max: usize) -> usize {
    let mut remaining = max;
    let mut hidden = 0;
    for hunk in hunks.iter_mut() {
        let kept = hunk.len().min(remaining);
        hidden += hunk.len() - kept;
        hunk.truncate(kept);
        remaining -= kept;
    }
    hunks.retain(|hunk| !hunk.is_empty());
    hidden
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kinds(diff: &FileDiff) -> Vec<(DiffLineKind, &str)> {
        diff.hunks.iter().flatten().map(|line| (line.kind, line.text.as_str())).collect()
    }

    #[test]
    fn an_edit_shows_context_removed_and_added_lines() {
        let input = json!({ "file_path": "src/a.rs", "old_string": "a\nb\nc\n", "new_string": "a\nB\nc\n" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!(diff.path, "src/a.rs");
        assert!(!diff.new_content);
        assert_eq!((diff.added, diff.removed, diff.hidden_lines), (1, 1, 0));
        assert_eq!(
            kinds(&diff),
            vec![
                (DiffLineKind::Context, "a"),
                (DiffLineKind::Removed, "b"),
                (DiffLineKind::Added, "B"),
                (DiffLineKind::Context, "c"),
            ]
        );
    }

    #[test]
    fn text_without_a_final_newline_and_crlf_lose_their_line_endings() {
        let input = json!({ "file_path": "f", "old_string": "x\r\ny", "new_string": "x\r\nz" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!(
            kinds(&diff),
            vec![(DiffLineKind::Context, "x"), (DiffLineKind::Removed, "y"), (DiffLineKind::Added, "z")]
        );
    }

    #[test]
    fn an_edit_with_an_empty_old_string_is_all_additions() {
        let input = json!({ "file_path": "f", "old_string": "", "new_string": "one\ntwo\n" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!((diff.added, diff.removed), (2, 0));
    }

    #[test]
    fn a_multi_edit_has_one_hunk_per_edit_in_order() {
        let input = json!({
            "file_path": "f",
            "edits": [
                { "old_string": "a", "new_string": "b" },
                { "old_string": "c", "new_string": "d" }
            ]
        });
        let diff = diff_for_tool("MultiEdit", &input).unwrap();
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!((diff.added, diff.removed), (2, 2));
        assert_eq!(diff.hunks[1][0].text, "c");
    }

    #[test]
    fn a_write_is_all_additions_and_marked_as_new_content() {
        let input = json!({ "file_path": "new.txt", "content": "l1\nl2\nl3" });
        let diff = diff_for_tool("Write", &input).unwrap();
        assert!(diff.new_content);
        assert_eq!((diff.added, diff.removed), (3, 0));
        assert!(kinds(&diff).iter().all(|(kind, _)| *kind == DiffLineKind::Added));
    }

    #[test]
    fn an_empty_write_has_a_diff_with_no_lines() {
        let diff = diff_for_tool("Write", &json!({ "file_path": "e", "content": "" })).unwrap();
        assert_eq!((diff.added, diff.removed, diff.hidden_lines), (0, 0, 0));
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn a_huge_write_is_cut_to_the_line_limit() {
        let content = "line\n".repeat(1000);
        let diff = diff_for_tool("Write", &json!({ "file_path": "big", "content": content })).unwrap();
        assert_eq!(diff.added, 1000);
        assert_eq!(diff.hunks.iter().map(Vec::len).sum::<usize>(), 300);
        assert_eq!(diff.hidden_lines, 700);
    }

    #[test]
    fn the_limit_is_shared_across_the_hunks_of_a_multi_edit() {
        let block = "x\n".repeat(200);
        let input = json!({
            "file_path": "f",
            "edits": [
                { "old_string": "", "new_string": block },
                { "old_string": "", "new_string": block }
            ]
        });
        let diff = diff_for_tool("MultiEdit", &input).unwrap();
        assert_eq!(diff.hunks.iter().map(Vec::len).sum::<usize>(), 300);
        assert_eq!(diff.hidden_lines, 100);
    }

    #[test]
    fn other_tools_have_no_diff() {
        assert_eq!(diff_for_tool("Bash", &json!({ "command": "ls" })), None);
        assert_eq!(diff_for_tool("Read", &json!({ "file_path": "f" })), None);
    }

    #[test]
    fn malformed_input_has_no_diff() {
        assert_eq!(diff_for_tool("Edit", &json!({ "file_path": "f", "old_string": "a" })), None);
        assert_eq!(diff_for_tool("Edit", &json!({ "old_string": "a", "new_string": "b" })), None);
        assert_eq!(diff_for_tool("Edit", &json!({ "file_path": 7, "old_string": "a", "new_string": "b" })), None);
        assert_eq!(diff_for_tool("Edit", &json!("not an object")), None);
        assert_eq!(diff_for_tool("MultiEdit", &json!({ "file_path": "f", "edits": "nope" })), None);
        assert_eq!(diff_for_tool("MultiEdit", &json!({ "file_path": "f", "edits": [{ "old_string": "a" }] })), None);
        assert_eq!(diff_for_tool("Write", &json!({ "file_path": "f" })), None);
    }

    #[test]
    fn the_tool_call_wrapper_reads_name_and_input() {
        let call = forge_domain::ToolCall {
            id: "t".to_string(),
            name: "Write".to_string(),
            input: json!({ "file_path": "f", "content": "x" }),
            result: None,
        };
        assert!(diff_for_tool_call(&call).is_some());
    }
}
