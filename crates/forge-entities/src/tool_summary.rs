use forge_domain::ToolCall;
use serde_json::Value;

// The first of these keys with a string value is what a person wants to see about a tool call.
const SUMMARY_KEYS: [&str; 7] = [
    "command",
    "file_path",
    "path",
    "pattern",
    "url",
    "query",
    "description",
];
const MAX_SUMMARY_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Done,
    Failed,
}

/// A one-line, at most 80-character description of a tool call's input.
pub fn tool_input_summary(input: &Value) -> String {
    let preferred = input
        .as_object()
        .and_then(|object| SUMMARY_KEYS.iter().find_map(|key| object.get(*key)?.as_str()))
        .map(str::to_string);
    let text = preferred.unwrap_or_else(|| match input {
        Value::Null => String::new(),
        Value::Object(object) if object.is_empty() => String::new(),
        other => other.to_string(),
    });
    truncate_chars(&text.replace('\n', " "), MAX_SUMMARY_CHARS)
}

// Counts characters, not bytes, so multi-byte text is never cut inside a character.
fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}

const MAX_DETAIL_CHARS: usize = 4000;

/// A tool call's input or output for the detailed view: cut to 4000 characters with a note of how
/// much was left out, so one large result cannot fill the chat.
pub fn tool_detail_text(text: &str) -> String {
    let total = text.chars().count();
    if total <= MAX_DETAIL_CHARS {
        return text.to_string();
    }
    let kept: String = text.chars().take(MAX_DETAIL_CHARS).collect();
    format!("{kept}\n… {} more characters", total - MAX_DETAIL_CHARS)
}

pub fn tool_call_status(tool_call: &ToolCall) -> ToolCallStatus {
    match &tool_call.result {
        None => ToolCallStatus::Running,
        Some(result) if result.is_error => ToolCallStatus::Failed,
        Some(_) => ToolCallStatus::Done,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::ToolResult;
    use serde_json::json;

    fn call(result: Option<ToolResult>) -> ToolCall {
        ToolCall {
            id: "t-1".to_string(),
            name: "Bash".to_string(),
            input: json!({}),
            result,
        }
    }

    #[test]
    fn summary_prefers_the_command() {
        let input = json!({ "file_path": "a.rs", "command": "cargo test" });
        assert_eq!(tool_input_summary(&input), "cargo test");
    }

    #[test]
    fn summary_falls_back_through_the_known_keys() {
        assert_eq!(tool_input_summary(&json!({ "file_path": "src/lib.rs" })), "src/lib.rs");
        assert_eq!(tool_input_summary(&json!({ "pattern": "fn main" })), "fn main");
    }

    #[test]
    fn summary_falls_back_to_compact_json_for_unknown_shapes() {
        assert_eq!(tool_input_summary(&json!({ "n": 1 })), r#"{"n":1}"#);
        assert_eq!(tool_input_summary(&json!("plain")), r#""plain""#);
    }

    #[test]
    fn summary_of_nothing_is_empty() {
        assert_eq!(tool_input_summary(&Value::Null), "");
        assert_eq!(tool_input_summary(&json!({})), "");
    }

    #[test]
    fn summary_is_a_single_line() {
        assert_eq!(tool_input_summary(&json!({ "command": "a\nb" })), "a b");
    }

    #[test]
    fn summary_is_cut_to_80_characters_with_an_ellipsis() {
        let summary = tool_input_summary(&json!({ "command": "x".repeat(200) }));
        assert_eq!(summary.chars().count(), 80);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn summary_never_cuts_inside_a_multi_byte_character() {
        let summary = tool_input_summary(&json!({ "command": "한".repeat(200) }));
        assert_eq!(summary.chars().count(), 80);
        assert!(summary.starts_with('한'));
    }

    #[test]
    fn a_short_summary_is_left_alone() {
        assert_eq!(tool_input_summary(&json!({ "command": "ls" })), "ls");
    }

    #[test]
    fn detail_text_keeps_a_short_text_whole() {
        assert_eq!(tool_detail_text("line 1\nline 2"), "line 1\nline 2");
    }

    #[test]
    fn detail_text_is_cut_with_a_count_of_what_is_left() {
        let text = "한".repeat(4100);
        let detail = tool_detail_text(&text);
        assert!(detail.starts_with(&"한".repeat(4000)));
        assert!(detail.ends_with("… 100 more characters"));
    }

    #[test]
    fn status_follows_the_result() {
        assert_eq!(tool_call_status(&call(None)), ToolCallStatus::Running);
        assert_eq!(
            tool_call_status(&call(Some(ToolResult { output: String::new(), is_error: false }))),
            ToolCallStatus::Done
        );
        assert_eq!(
            tool_call_status(&call(Some(ToolResult { output: String::new(), is_error: true }))),
            ToolCallStatus::Failed
        );
    }
}
