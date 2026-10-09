use serde_json::Value;

use crate::tool_summary::tool_input_summary;
use crate::turn_view::{file_name, tool_kind, ToolKind};
use forge_shared::tr;

/// What a permission card says about a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionView {
    /// "Claude wants to create obs.txt?"
    pub title: String,
    /// The file name, the command's description (else the command), or the tool name.
    pub subject: String,
    /// The full path of a file tool.
    pub path: Option<String>,
    /// The full command of a Bash call.
    pub command: Option<String>,
    /// Set when the request is to run a workflow: its card has its own text and two choices.
    pub workflow: Option<WorkflowAsk>,
}

/// What a workflow's own card adds to the title: the one-line description from the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAsk {
    pub description: Option<String>,
}

/// The tool Claude calls to run a workflow.
const WORKFLOW_TOOL: &str = "Workflow";

/// The string literal after `key:` in the script's text, the first time `key` stands alone as a
/// word. Reads `'...'`, `"..."` and backtick strings with backslash escapes; `None` when the key is
/// absent or its value is not a plain string. A script is JavaScript, so this reads only what the
/// `meta` block needs, never runs or parses the rest.
fn script_literal(script: &str, key: &str) -> Option<String> {
    let mut from = 0;
    while let Some(found) = script[from..].find(key) {
        let start = from + found;
        let end = start + key.len();
        from = end;
        let before_ok = script[..start].chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_'));
        let rest = script[end..].trim_start();
        let Some(rest) = rest.strip_prefix(':') else { continue };
        let mut chars = rest.trim_start().chars();
        let Some(quote) = chars.next().filter(|c| matches!(c, '\'' | '"' | '`')) else { continue };
        if !before_ok {
            continue;
        }
        let mut value = String::new();
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(other) => value.push(other),
                    None => return None,
                },
                c if c == quote => return Some(value).filter(|value| !value.is_empty()),
                c => value.push(c),
            }
        }
        return None;
    }
    None
}

pub fn permission_view(tool_name: &str, input: &Value) -> PermissionView {
    let text = |key: &str| input.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string);
    if tool_name == WORKFLOW_TOOL {
        // The meta block comes first in a script; stop before the body so a string in a later
        // agent prompt is never taken for the workflow's name.
        let script = input.get("script").and_then(Value::as_str).unwrap_or("");
        let meta = script.split("\n}").next().unwrap_or(script);
        if let Some(name) = script_literal(meta, "name") {
            let description = script_literal(meta, "description");
            return PermissionView {
                title: tr!("Allow Claude to run the workflow {name}?", name = name),
                subject: name,
                path: None,
                command: None,
                workflow: Some(WorkflowAsk { description }),
            };
        }
    }
    let kind = tool_kind(tool_name);
    let (verb, subject, path, command) = match kind {
        ToolKind::Created | ToolKind::Edited | ToolKind::Read => {
            let path = text("file_path").or_else(|| text("notebook_path"));
            let subject = path.as_deref().map(file_name).unwrap_or_else(|| tr!("a file").to_string());
            (kind, subject, path, None)
        }
        ToolKind::Command => {
            let command = text("command");
            let subject = text("description").or_else(|| command.clone()).unwrap_or_else(|| tool_input_summary(input));
            (kind, subject, None, command)
        }
        ToolKind::Other => (kind, tool_name.to_string(), None, None),
    };
    let title = match verb {
        ToolKind::Created => tr!("Claude wants to create {subject}?", subject = subject),
        ToolKind::Edited => tr!("Claude wants to edit {subject}?", subject = subject),
        ToolKind::Read => tr!("Claude wants to read {subject}?", subject = subject),
        ToolKind::Command => tr!("Claude wants to run {subject}?", subject = subject),
        ToolKind::Other => tr!("Claude wants to use {subject}?", subject = subject),
    };
    PermissionView { title, subject, path, command, workflow: None }
}

/// Whether a card that just appeared may take keyboard focus: not while the person is typing.
/// `typed_len` is the length of the text in the focused text field, or -1 when no text field has
/// focus. Taking focus from typed text would turn the next digit into a decision.
pub fn card_takes_focus(typed_len: i64) -> bool {
    typed_len <= 0
}

/// What a key does on a permission card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionKey {
    Deny,
    AllowAlways,
    AllowOnce,
}

/// The decision a key press stands for, from the physical key (`code`), never the character: the
/// character depends on the input method. `1` and Esc deny, `2` and ⇧⌘↩ allow always, `3` and ⌘↩
/// allow once. A digit with ⌘, ⌃ or ⌥ held (`other_modifier`) is not a decision.
pub fn permission_key(code: &str, shift: bool, meta: bool, other_modifier: bool) -> Option<PermissionKey> {
    if matches!(code, "Enter" | "NumpadEnter") {
        return match (meta, shift) {
            (true, true) => Some(PermissionKey::AllowAlways),
            (true, false) => Some(PermissionKey::AllowOnce),
            _ => None,
        };
    }
    if meta || other_modifier {
        return None;
    }
    match code {
        "Escape" => Some(PermissionKey::Deny),
        "Digit1" | "Numpad1" => Some(PermissionKey::Deny),
        "Digit2" | "Numpad2" => Some(PermissionKey::AllowAlways),
        "Digit3" | "Numpad3" => Some(PermissionKey::AllowOnce),
        _ => None,
    }
}

/// The keys of a workflow's card, which has two choices: `1` and Esc deny, `2` and ⌘↩ allow once.
/// There is no always-allow and no third choice, so `3` and ⇧⌘↩ do nothing.
pub fn workflow_permission_key(code: &str, shift: bool, meta: bool, other_modifier: bool) -> Option<PermissionKey> {
    match permission_key(code, shift, meta, other_modifier)? {
        PermissionKey::Deny => Some(PermissionKey::Deny),
        PermissionKey::AllowOnce if matches!(code, "Enter" | "NumpadEnter") => Some(PermissionKey::AllowOnce),
        PermissionKey::AllowAlways if matches!(code, "Digit2" | "Numpad2") => Some(PermissionKey::AllowOnce),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_write_asks_to_create_the_file_by_name() {
        let view = permission_view("Write", &json!({"file_path": "/private/tmp/forge-observe/obs.txt", "content": "x"}));
        assert_eq!(view.title, "Claude wants to create obs.txt?");
        assert_eq!(view.subject, "obs.txt");
        assert_eq!(view.path.as_deref(), Some("/private/tmp/forge-observe/obs.txt"));
        assert_eq!(view.command, None);
    }

    #[test]
    fn edits_reads_and_notebooks_use_their_own_verb() {
        assert_eq!(permission_view("Edit", &json!({"file_path": "/p/a.py"})).title, "Claude wants to edit a.py?");
        assert_eq!(permission_view("MultiEdit", &json!({"file_path": "/p/a.py"})).title, "Claude wants to edit a.py?");
        assert_eq!(permission_view("NotebookEdit", &json!({"notebook_path": "/p/n.ipynb"})).title, "Claude wants to edit n.ipynb?");
        assert_eq!(permission_view("Read", &json!({"file_path": "/p/a.py"})).title, "Claude wants to read a.py?");
    }

    #[test]
    fn a_command_asks_to_run_its_description_and_carries_the_command() {
        let view = permission_view("Bash", &json!({"command": "python3 fizz.py", "description": "Run fizz.py"}));
        assert_eq!(view.title, "Claude wants to run Run fizz.py?");
        assert_eq!(view.command.as_deref(), Some("python3 fizz.py"));
        assert_eq!(view.path, None);
        let bare = permission_view("Bash", &json!({"command": "ls -la"}));
        assert_eq!(bare.title, "Claude wants to run ls -la?");
    }

    #[test]
    fn any_other_tool_is_used_by_name_even_with_an_empty_input() {
        for input in [json!({}), json!(null), json!("x")] {
            let view = permission_view("mcp__thing__go", &input);
            assert_eq!(view.title, "Claude wants to use mcp__thing__go?");
            assert_eq!(view.path, None);
            assert_eq!(view.command, None);
        }
    }

    fn workflow_input() -> Value {
        json!({"script": "export const meta = {\n  name: 'sample-risk-review',\n  description: \"Three parallel agents review sample/domain; a fourth merges\",\n  phases: [{ title: 'Analyze' }],\n}\nconst x = await agent('name: not this one')"})
    }

    #[test]
    fn a_workflow_asks_by_its_own_name_and_shows_its_description() {
        let view = permission_view("Workflow", &workflow_input());
        assert_eq!(view.title, "Allow Claude to run the workflow sample-risk-review?");
        let workflow = view.workflow.expect("a workflow card");
        assert_eq!(workflow.description.as_deref(), Some("Three parallel agents review sample/domain; a fourth merges"));
        assert_eq!(view.command, None);
    }

    #[test]
    fn a_workflow_without_a_readable_meta_falls_back_to_the_plain_tool_card() {
        for input in [json!({}), json!({"script": "const nothing = 1"}), json!({"script": 7})] {
            let view = permission_view("Workflow", &input);
            assert_eq!(view.title, "Claude wants to use Workflow?");
            assert_eq!(view.workflow, None);
        }
    }

    #[test]
    fn a_workflow_name_with_an_escaped_quote_reads_whole() {
        let view = permission_view("Workflow", &json!({"script": "export const meta = { name: 'it\\'s fine', description: 'd' }"}));
        assert_eq!(view.title, "Allow Claude to run the workflow it's fine?");
    }

    #[test]
    fn a_workflow_card_has_two_choices_and_no_always() {
        assert_eq!(workflow_permission_key("Digit1", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(workflow_permission_key("Escape", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(workflow_permission_key("Digit2", false, false, false), Some(PermissionKey::AllowOnce));
        assert_eq!(workflow_permission_key("Enter", false, true, false), Some(PermissionKey::AllowOnce));
        assert_eq!(workflow_permission_key("Digit3", false, false, false), None, "there is no third choice");
        assert_eq!(workflow_permission_key("Enter", true, true, false), None, "there is no always allow");
        assert_eq!(workflow_permission_key("Digit2", false, true, false), None, "a digit with another modifier is not a decision");
    }

    #[test]
    fn a_write_with_no_path_still_has_a_title() {
        let view = permission_view("Write", &json!({"content": "x"}));
        assert_eq!(view.title, "Claude wants to create a file?");
        assert_eq!(view.path, None);
    }

    #[test]
    fn digits_and_escape_pick_by_physical_key() {
        assert_eq!(permission_key("Digit1", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Numpad1", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Escape", false, false, false), Some(PermissionKey::Deny));
        assert_eq!(permission_key("Digit2", false, false, false), Some(PermissionKey::AllowAlways));
        assert_eq!(permission_key("Digit3", false, false, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("Digit4", false, false, false), None);
        assert_eq!(permission_key("KeyA", false, false, false), None);
    }

    #[test]
    fn command_return_allows_once_and_with_shift_always() {
        assert_eq!(permission_key("Enter", false, true, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("NumpadEnter", false, true, false), Some(PermissionKey::AllowOnce));
        assert_eq!(permission_key("Enter", true, true, false), Some(PermissionKey::AllowAlways));
        assert_eq!(permission_key("Enter", false, false, false), None, "plain Return is not a decision");
    }

    #[test]
    fn a_card_never_takes_focus_from_text_the_person_is_typing() {
        // -1: no text field has focus; otherwise the length of its text.
        assert!(card_takes_focus(-1));
        assert!(card_takes_focus(0), "an empty message box is fine to leave");
        assert!(!card_takes_focus(1));
        assert!(!card_takes_focus(120));
    }

    #[test]
    fn a_digit_with_another_modifier_held_is_not_a_decision() {
        assert_eq!(permission_key("Digit1", false, false, true), None);
        assert_eq!(permission_key("Digit3", false, true, false), None);
    }
}
