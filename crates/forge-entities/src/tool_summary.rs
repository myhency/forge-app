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

/// One `key: value` line of a tool call's input, as the opened row shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputLine {
    pub key: String,
    pub value: String,
    /// An absolute path without spaces: shown blue and underlined.
    pub is_path: bool,
}

// The keys a person wants first, in this order; every other key follows alphabetically.
const FIRST_KEYS: [&str; 7] = ["file_path", "notebook_path", "path", "command", "description", "pattern", "url"];

/// The input of a tool call as `key: value` lines (no JSON braces). Strings are shown as they are,
/// anything else as compact JSON; a value is cut to 4000 characters. Input that is not an object
/// has no lines.
pub fn tool_input_lines(input: &Value) -> Vec<InputLine> {
    let Some(object) = input.as_object() else {
        return Vec::new();
    };
    let mut entries: Vec<(&String, &Value)> = object.iter().collect();
    entries.sort_by_key(|(key, _)| (FIRST_KEYS.iter().position(|first| first == &key.as_str()).unwrap_or(FIRST_KEYS.len()), key.to_string()));
    entries
        .into_iter()
        .map(|(key, value)| {
            let text = match value {
                Value::String(text) => text.clone(),
                other => other.to_string(),
            };
            let is_path = matches!(value, Value::String(text) if text.starts_with('/') && !text.contains(char::is_whitespace));
            InputLine { key: key.clone(), value: tool_detail_text(&text), is_path }
        })
        .collect()
}

/// The clock of a running turn: "4s", "1m 21s", "1h 2m".
pub fn elapsed_label(seconds: u64) -> String {
    match seconds {
        s if s < 60 => format!("{s}s"),
        s if s < 3600 => format!("{}m {}s", s / 60, s % 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
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

    #[test]
    fn input_lines_put_the_useful_keys_first_and_flag_paths() {
        let lines = tool_input_lines(&json!({"content": "x", "file_path": "/private/tmp/obs.txt", "mode": 3}));
        let keys: Vec<&str> = lines.iter().map(|line| line.key.as_str()).collect();
        assert_eq!(keys, vec!["file_path", "content", "mode"]);
        assert_eq!(lines[0], InputLine { key: "file_path".to_string(), value: "/private/tmp/obs.txt".to_string(), is_path: true });
        assert!(!lines[1].is_path);
        assert_eq!(lines[2].value, "3", "a non-string value is written as JSON");
    }

    #[test]
    fn input_lines_of_a_path_with_spaces_or_a_relative_value_are_not_paths() {
        let lines = tool_input_lines(&json!({"a": "/has space", "b": "relative/x", "c": "/ok"}));
        let flags: Vec<bool> = lines.iter().map(|line| line.is_path).collect();
        assert_eq!(flags, vec![false, false, true]);
    }

    #[test]
    fn input_lines_of_something_that_is_not_an_object_are_empty() {
        for input in [json!(null), json!("text"), json!([1, 2]), json!({})] {
            assert!(tool_input_lines(&input).is_empty(), "{input}");
        }
    }

    #[test]
    fn a_nested_value_is_written_compactly_and_a_long_one_is_cut() {
        let lines = tool_input_lines(&json!({"edits": [{"old": "a"}], "big": "x".repeat(5000)}));
        let edits = lines.iter().find(|line| line.key == "edits").unwrap();
        assert_eq!(edits.value, r#"[{"old":"a"}]"#);
        let big = lines.iter().find(|line| line.key == "big").unwrap();
        assert!(big.value.ends_with("… 1000 more characters"));
    }

    #[test]
    fn the_clock_reads_seconds_minutes_and_hours() {
        assert_eq!(elapsed_label(0), "0s");
        assert_eq!(elapsed_label(4), "4s");
        assert_eq!(elapsed_label(59), "59s");
        assert_eq!(elapsed_label(81), "1m 21s");
        assert_eq!(elapsed_label(3600), "1h 0m");
        assert_eq!(elapsed_label(3720), "1h 2m");
    }
}
