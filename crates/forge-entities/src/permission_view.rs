use forge_domain::PermissionRequest;
use serde_json::Value;

use crate::tool_summary::tool_input_summary;
use crate::turn_view::{file_name, tool_kind, ToolKind};
use crate::workflow_script::workflow_meta;
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
    /// Set when the request is Claude's plan (ExitPlanMode): its card shows the plan text.
    pub plan: Option<PlanAsk>,
}

/// The plan a plan card shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanAsk {
    pub text: String,
}

/// What a workflow's own card adds to the title: the one-line description from the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowAsk {
    pub description: Option<String>,
}

/// The tool Claude calls to run a workflow.
const WORKFLOW_TOOL: &str = "Workflow";

/// The tool Claude calls to propose a plan.
const PLAN_TOOL: &str = "ExitPlanMode";

/// The plan text of a request, when it is an `ExitPlanMode` request that carries one.
pub fn plan_text_of_request(request: &PermissionRequest) -> Option<String> {
    permission_view(&request.tool_name, &request.input).plan.map(|plan| plan.text)
}

pub fn permission_view(tool_name: &str, input: &Value) -> PermissionView {
    let text = |key: &str| input.get(key).and_then(Value::as_str).filter(|text| !text.is_empty()).map(str::to_string);
    if tool_name == WORKFLOW_TOOL {
        let script = input.get("script").and_then(Value::as_str).unwrap_or("");
        if let Some(meta) = workflow_meta(script) {
            return PermissionView {
                title: tr!("Allow Claude to run the workflow {name}?", name = meta.name),
                subject: meta.name,
                path: None,
                command: None,
                workflow: Some(WorkflowAsk { description: meta.description }),
                plan: None,
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
    let plan = (tool_name == PLAN_TOOL).then(|| text("plan")).flatten().map(|text| PlanAsk { text });
    PermissionView { title, subject, path, command, workflow: None, plan }
}

/// Whether a card that just appeared may take keyboard focus: not while the person is typing.
/// `typed_len` is the length of the text in the focused text field, or -1 when no text field has
/// focus. Taking focus from typed text would turn the next digit into a decision.
pub fn card_takes_focus(typed_len: i64) -> bool {
    typed_len <= 0
}

/// Whether the plan card takes focus when it appears: only when no text field has focus (`-1`).
/// An empty message box keeps focus too, so a ⌘↩ meant to send never accepts a plan.
pub fn plan_card_takes_focus(typed_len: i64) -> bool {
    typed_len < 0
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

/// What a key does on a plan card.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanKey {
    /// Esc: ask for changes.
    Revise,
    /// ⇧⌘↩: accept the plan.
    Accept,
    /// ⌘↩: accept the plan and run in auto mode.
    AcceptAuto,
}

/// The decision a key press stands for on a plan card, from the physical key (`code`) only.
/// Esc revises, ⌘↩ accepts in auto mode, ⇧⌘↩ accepts. Esc with ⌘, ⌃ or ⌥ held does nothing.
pub fn plan_key(code: &str, shift: bool, meta: bool, other_modifier: bool) -> Option<PlanKey> {
    if matches!(code, "Enter" | "NumpadEnter") {
        if other_modifier {
            return None;
        }
        return match (meta, shift) {
            (true, true) => Some(PlanKey::Accept),
            (true, false) => Some(PlanKey::AcceptAuto),
            _ => None,
        };
    }
    if code == "Escape" && !meta && !other_modifier {
        return Some(PlanKey::Revise);
    }
    None
}

/// How the person accepts a plan: the permission mode the session runs in afterwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanChoice {
    Accept,
    AcceptEdits,
    AcceptAuto,
}

impl PlanChoice {
    pub fn mode(self) -> forge_domain::PermissionMode {
        match self {
            PlanChoice::Accept => forge_domain::PermissionMode::Default,
            PlanChoice::AcceptEdits => forge_domain::PermissionMode::AcceptEdits,
            PlanChoice::AcceptAuto => forge_domain::PermissionMode::Auto,
        }
    }
}

/// What Claude is told when the person turns a plan down without saying why. It goes to the model,
/// not the screen.
pub const DEFAULT_PLAN_REFUSAL: &str =
    "The user declined this plan or wants changes to it. Stay in plan mode and ask the user what they would like to change.";

/// The reason sent with a refusal: the person's own words, else the default.
pub fn plan_reason(text: &str) -> String {
    let text = text.trim();
    if text.is_empty() { DEFAULT_PLAN_REFUSAL.to_string() } else { text.to_string() }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_plan_request_gives_its_plan_text_and_other_requests_none() {
        let plan = PermissionRequest { id: "p".into(), tool_name: "ExitPlanMode".into(), input: json!({"plan": "# Plan"}), rules: Vec::new() };
        assert_eq!(plan_text_of_request(&plan), Some("# Plan".to_string()));
        let bash = PermissionRequest { id: "b".into(), tool_name: "Bash".into(), input: json!({"command": "ls"}), rules: Vec::new() };
        assert_eq!(plan_text_of_request(&bash), None);
    }

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
    fn the_plan_card_takes_focus_only_when_no_text_field_has_it() {
        assert!(plan_card_takes_focus(-1));
        assert!(!plan_card_takes_focus(0));
        assert!(!plan_card_takes_focus(1));
        assert!(!plan_card_takes_focus(120));
    }

    #[test]
    fn plan_enter_with_another_modifier_is_nothing() {
        assert_eq!(plan_key("Enter", false, true, true), None);
        assert_eq!(plan_key("Enter", true, true, true), None);
    }

    #[test]
    fn a_digit_with_another_modifier_held_is_not_a_decision() {
        assert_eq!(permission_key("Digit1", false, false, true), None);
        assert_eq!(permission_key("Digit3", false, true, false), None);
    }

    #[test]
    fn exit_plan_mode_carries_the_plan_text() {
        let view = permission_view("ExitPlanMode", &json!({"plan": "# P"}));
        assert_eq!(view.plan, Some(PlanAsk { text: "# P".to_string() }));
        for input in [json!({}), json!({"plan": ""}), json!({"plan": 3})] {
            let view = permission_view("ExitPlanMode", &input);
            assert_eq!(view.plan, None);
            assert_eq!(view.title, "Claude wants to use ExitPlanMode?");
        }
        assert_eq!(permission_view("Bash", &json!({"plan": "x"})).plan, None);
    }

    #[test]
    fn plan_keys_go_by_physical_key() {
        assert_eq!(plan_key("Escape", false, false, false), Some(PlanKey::Revise));
        assert_eq!(plan_key("Enter", false, true, false), Some(PlanKey::AcceptAuto));
        assert_eq!(plan_key("Enter", true, true, false), Some(PlanKey::Accept));
        assert_eq!(plan_key("Digit1", false, false, false), None);
        assert_eq!(plan_key("Enter", false, false, false), None);
        assert_eq!(plan_key("Escape", false, true, false), None);
    }

    #[test]
    fn plan_choices_map_to_modes() {
        use forge_domain::PermissionMode;
        assert_eq!(PlanChoice::Accept.mode(), PermissionMode::Default);
        assert_eq!(PlanChoice::AcceptEdits.mode(), PermissionMode::AcceptEdits);
        assert_eq!(PlanChoice::AcceptAuto.mode(), PermissionMode::Auto);
    }

    #[test]
    fn plan_reason_is_the_trimmed_text_or_the_default() {
        assert_eq!(plan_reason("  "), DEFAULT_PLAN_REFUSAL);
        assert_eq!(plan_reason(""), DEFAULT_PLAN_REFUSAL);
        assert_eq!(plan_reason(" no "), "no");
    }
}
