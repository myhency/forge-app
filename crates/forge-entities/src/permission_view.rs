use serde_json::Value;

use crate::tool_summary::tool_input_summary;
use crate::turn_view::{file_name, tool_kind, ToolKind};

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
}

pub fn permission_view(tool_name: &str, input: &Value) -> PermissionView {
    let text = |key: &str| input.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string);
    let kind = tool_kind(tool_name);
    let (verb, subject, path, command) = match kind {
        ToolKind::Created | ToolKind::Edited | ToolKind::Read => {
            let verb = match kind {
                ToolKind::Created => "create",
                ToolKind::Edited => "edit",
                _ => "read",
            };
            let path = text("file_path").or_else(|| text("notebook_path"));
            let subject = path.as_deref().map(file_name).unwrap_or_else(|| "a file".to_string());
            (verb, subject, path, None)
        }
        ToolKind::Command => {
            let command = text("command");
            let subject = text("description").or_else(|| command.clone()).unwrap_or_else(|| tool_input_summary(input));
            ("run", subject, None, command)
        }
        ToolKind::Other => ("use", tool_name.to_string(), None, None),
    };
    PermissionView { title: format!("Claude wants to {verb} {subject}?"), subject, path, command }
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
