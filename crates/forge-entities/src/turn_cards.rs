//! The two tools the desktop app gives Claude, as Forge draws them: the task list folded from the
//! `update_plan` calls and the end-of-turn card read from a `wrap_up` call. No Dioxus in here.

use forge_domain::ToolCall;
use serde_json::Value;

const SERVER_PREFIX: &str = "mcp__ccd_turn__";
pub const MAX_STEPS: usize = 12;
pub const MAX_ACTIONS: usize = 2;
pub const MAX_LABEL_CHARS: usize = 24;
pub const MAX_MESSAGE_CHARS: usize = 300;
pub const MAX_RECAP_CHARS: usize = 600;
pub const MAX_ASK_CHARS: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnTool {
    UpdatePlan,
    WrapUp,
}

/// Which of the two tools a call's name is. Only the desktop app's full name counts.
pub fn turn_tool(name: &str) -> Option<TurnTool> {
    match name.strip_prefix(SERVER_PREFIX)? {
        "update_plan" => Some(TurnTool::UpdatePlan),
        "wrap_up" => Some(TurnTool::WrapUp),
        _ => None,
    }
}

fn refused(call: &ToolCall) -> bool {
    call.result.as_ref().is_some_and(|result| result.is_error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    InProgress,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub content: String,
    pub state: StepState,
}

fn step_of(item: &Value) -> Option<Step> {
    let content = item.get("content")?.as_str()?.trim();
    if content.is_empty() {
        return None;
    }
    let state = match item.get("status").and_then(Value::as_str) {
        Some("in_progress") => StepState::InProgress,
        Some("completed") => StepState::Done,
        _ => StepState::Pending,
    };
    Some(Step { content: content.to_string(), state })
}

/// Applies one `update_plan` input to the list. `items` replaces it; otherwise `advance` completes the
/// step in progress and starts the first pending one, `done` completes the 1-based numbers, and `start`
/// makes one 1-based step the only one in progress. A number outside the list changes nothing.
pub fn apply(steps: &mut Vec<Step>, input: &Value) {
    if let Some(items) = input.get("items").and_then(Value::as_array) {
        *steps = items.iter().filter_map(step_of).take(MAX_STEPS).collect();
        return;
    }
    if input.get("advance").and_then(Value::as_bool) == Some(true) {
        if let Some(current) = steps.iter().position(|step| step.state == StepState::InProgress) {
            steps[current].state = StepState::Done;
        }
        if let Some(next) = steps.iter().position(|step| step.state == StepState::Pending) {
            steps[next].state = StepState::InProgress;
        }
    }
    if let Some(done) = input.get("done").and_then(Value::as_array) {
        for number in done.iter().filter_map(Value::as_u64) {
            if let Some(step) = number.checked_sub(1).and_then(|index| steps.get_mut(index as usize)) {
                step.state = StepState::Done;
            }
        }
    }
    if let Some(number) = input.get("start").and_then(Value::as_u64) {
        let target = number.checked_sub(1).map(|index| index as usize).filter(|index| *index < steps.len());
        if let Some(target) = target {
            for step in steps.iter_mut().filter(|step| step.state == StepState::InProgress) {
                step.state = StepState::Pending;
            }
            steps[target].state = StepState::InProgress;
        }
    }
}

/// The plan the calls build, in order; refused calls are skipped. `None` when there is no
/// `update_plan` call or the list ends up empty.
pub fn plan_of<'a>(calls: impl IntoIterator<Item = &'a ToolCall>) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    let mut seen = false;
    for call in calls {
        if turn_tool(&call.name) == Some(TurnTool::UpdatePlan) && !refused(call) {
            seen = true;
            apply(&mut steps, &call.input);
        }
    }
    (seen && !steps.is_empty()).then_some(steps)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardAction {
    pub label: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndCard {
    /// The id of the `wrap_up` call: what the per-session handled marks are keyed by.
    pub id: String,
    pub recap: String,
    pub ask: String,
    pub actions: Vec<CardAction>,
}

/// The first `max` characters, then trimmed (so a cut that ends in a space still matches its trimmed reply).
fn clip(text: &str, max: usize) -> String {
    text.chars().take(max).collect::<String>().trim().to_string()
}

/// What became of a card that is not answered yet: the person closed it, or a button (or `⌘⏎`) already sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardMark {
    Dismissed,
    Sent,
}

/// The card, unless it was handled (closed or already sent) according to the session's marks.
pub fn offered(card: Option<EndCard>, marks: &std::collections::HashMap<String, CardMark>) -> Option<EndCard> {
    card.filter(|card| !marks.contains_key(&card.id))
}

fn text_of<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key)?.as_str().map(str::trim).filter(|text| !text.is_empty())
}

fn action_of(item: &Value) -> Option<CardAction> {
    let (label, message) = match (text_of(item, "label"), text_of(item, "message")) {
        (None, None) => return None,
        (Some(only), None) | (None, Some(only)) => (only, only),
        (Some(label), Some(message)) => (label, message),
    };
    let message = clip(message, MAX_MESSAGE_CHARS);
    // A reply that starts with `/` or `!` would run a command, not talk: the model's text is never trusted with that.
    if message.starts_with('/') || message.starts_with('!') {
        return None;
    }
    Some(CardAction { label: clip(label, MAX_LABEL_CHARS), message })
}

/// The card a `wrap_up` call asks for. No card without a recap or without a question (the model's own
/// rule: a turn with nothing pending ends with no box), nor for a refused call.
pub fn end_card_of(call: &ToolCall) -> Option<EndCard> {
    if turn_tool(&call.name) != Some(TurnTool::WrapUp) || refused(call) {
        return None;
    }
    let recap = clip(text_of(&call.input, "recap")?, MAX_RECAP_CHARS);
    let ask = clip(text_of(&call.input, "ask")?, MAX_ASK_CHARS);
    let actions = call
        .input
        .get("actions")
        .and_then(Value::as_array)
        .map(|list| list.iter().filter_map(action_of).take(MAX_ACTIONS).collect())
        .unwrap_or_default();
    Some(EndCard { id: call.id.clone(), recap, ask, actions })
}

/// The button a user message answers: its message equals the text (spaces around it ignored, case not).
pub fn chosen_action<'a>(card: &'a EndCard, user_text: &str) -> Option<&'a CardAction> {
    let sent = user_text.trim();
    card.actions.iter().find(|action| action.message == sent)
}

#[cfg(test)]
mod tests {
    use forge_domain::{ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    fn call(name: &str, input: serde_json::Value, error: bool) -> ToolCall {
        ToolCall { id: "t".to_string(), name: name.to_string(), input, result: Some(ToolResult { output: "ok".to_string(), is_error: error }) }
    }
    fn plan(input: serde_json::Value) -> ToolCall {
        call("mcp__ccd_turn__update_plan", input, false)
    }
    fn wrap(input: serde_json::Value) -> ToolCall {
        call("mcp__ccd_turn__wrap_up", input, false)
    }
    fn states(steps: &[Step]) -> Vec<StepState> {
        steps.iter().map(|step| step.state).collect()
    }
    fn three() -> ToolCall {
        plan(json!({"items": [
            {"content": "Look", "status": "in_progress"},
            {"content": "Fix", "status": "pending"},
            {"content": "Test", "status": "pending"},
        ]}))
    }

    #[test]
    fn only_the_prefixed_names_are_the_two_tools() {
        assert_eq!(turn_tool("mcp__ccd_turn__update_plan"), Some(TurnTool::UpdatePlan));
        assert_eq!(turn_tool("mcp__ccd_turn__wrap_up"), Some(TurnTool::WrapUp));
        assert_eq!(turn_tool("update_plan"), None);
        assert_eq!(turn_tool("mcp__other__wrap_up"), None);
        assert_eq!(turn_tool("mcp__ccd_turn__other"), None);
    }

    #[test]
    fn items_make_the_plan_and_a_new_list_replaces_it() {
        let steps = plan_of([&three()]).unwrap();
        assert_eq!(steps.iter().map(|s| s.content.as_str()).collect::<Vec<_>>(), ["Look", "Fix", "Test"]);
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending]);
        let again = plan(json!({"items": [{"content": "Only", "status": "completed"}]}));
        let steps = plan_of([&three(), &again]).unwrap();
        assert_eq!(steps, vec![Step { content: "Only".to_string(), state: StepState::Done }]);
    }

    #[test]
    fn advance_completes_the_current_step_and_starts_the_next() {
        let advance = plan(json!({"advance": true}));
        let steps = plan_of([&three(), &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::InProgress, StepState::Pending]);
        let steps = plan_of([&three(), &advance, &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::Done, StepState::InProgress]);
    }

    #[test]
    fn advance_after_the_last_step_leaves_everything_done() {
        let advance = plan(json!({"advance": true}));
        let steps = plan_of([&three(), &advance, &advance, &advance, &advance]).unwrap();
        assert_eq!(states(&steps), [StepState::Done, StepState::Done, StepState::Done]);
    }

    #[test]
    fn done_and_start_use_one_based_numbers_and_skip_numbers_outside_the_list() {
        let out_of_order = plan(json!({"done": [2], "start": 3}));
        let steps = plan_of([&three(), &out_of_order]).unwrap();
        assert_eq!(states(&steps), [StepState::Pending, StepState::Done, StepState::InProgress]);
        let outside = plan(json!({"done": [0, 9], "start": 7}));
        let steps = plan_of([&three(), &outside]).unwrap();
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending], "nothing moved");
    }

    #[test]
    fn a_failed_call_does_not_change_the_plan_and_an_unknown_status_reads_as_pending() {
        let failed = call("mcp__ccd_turn__update_plan", json!({"advance": true}), true);
        let steps = plan_of([&three(), &failed]).unwrap();
        assert_eq!(states(&steps), [StepState::InProgress, StepState::Pending, StepState::Pending]);
        let odd = plan(json!({"items": [{"content": "A", "status": "weird"}, {"content": "  ", "status": "pending"}]}));
        let steps = plan_of([&odd]).unwrap();
        assert_eq!(steps, vec![Step { content: "A".to_string(), state: StepState::Pending }], "an empty step is dropped");
    }

    #[test]
    fn no_plan_call_or_an_empty_list_is_no_plan() {
        assert_eq!(plan_of([&call("Bash", json!({}), false)]), None);
        assert_eq!(plan_of([&plan(json!({"items": []}))]), None);
        assert_eq!(plan_of(std::iter::empty()), None);
    }

    #[test]
    fn at_most_twelve_steps_are_kept() {
        let items: Vec<_> = (0..15).map(|n| json!({"content": format!("s{n}"), "status": "pending"})).collect();
        assert_eq!(plan_of([&plan(json!({"items": items}))]).unwrap().len(), 12);
    }

    #[test]
    fn a_wrap_up_with_a_question_is_a_card_with_its_buttons() {
        let card = end_card_of(&wrap(json!({
            "recap": " Built it. ",
            "ask": "Commit?",
            "actions": [{"label": "Commit", "message": "commit it"}, {"label": "Not now"}],
        })))
        .unwrap();
        assert_eq!(card.recap, "Built it.");
        assert_eq!(card.ask, "Commit?");
        assert_eq!(
            card.actions,
            vec![
                CardAction { label: "Commit".to_string(), message: "commit it".to_string() },
                CardAction { label: "Not now".to_string(), message: "Not now".to_string() },
            ]
        );
    }

    #[test]
    fn a_wrap_up_without_a_question_or_a_recap_draws_no_card() {
        assert_eq!(end_card_of(&wrap(json!({"recap": "Done."}))), None, "no ask, no box");
        assert_eq!(end_card_of(&wrap(json!({"recap": "Done.", "ask": "  "}))), None);
        assert_eq!(end_card_of(&wrap(json!({"ask": "Commit?"}))), None);
        assert_eq!(end_card_of(&wrap(json!({"recap": "  ", "ask": "Commit?"}))), None);
        assert_eq!(end_card_of(&call("mcp__ccd_turn__wrap_up", json!({"recap": "x", "ask": "y"}), true)), None, "a refused call");
        assert_eq!(end_card_of(&call("Bash", json!({"recap": "x", "ask": "y"}), false)), None);
    }

    #[test]
    fn buttons_are_limited_and_cleaned() {
        let long_label = "L".repeat(40);
        let long_message = "M".repeat(400);
        let card = end_card_of(&wrap(json!({
            "recap": "r", "ask": "q",
            "actions": [{}, {"label": long_label, "message": long_message}, {"message": "only a message"}, {"label": "third"}],
        })))
        .unwrap();
        assert_eq!(card.actions.len(), 2, "the empty action is dropped, then at most two");
        assert_eq!(card.actions[0].label.chars().count(), 24);
        assert_eq!(card.actions[0].message.chars().count(), 300);
        assert_eq!(card.actions[1], CardAction { label: "only a message".to_string(), message: "only a message".to_string() });
    }

    #[test]
    fn a_button_is_chosen_by_an_exact_reply_ignoring_spaces_around_it() {
        let card = end_card_of(&wrap(json!({"recap": "r", "ask": "q", "actions": [{"label": "Yes", "message": "Yes, fix 1"}]}))).unwrap();
        assert_eq!(chosen_action(&card, "Yes, fix 1").map(|a| a.label.as_str()), Some("Yes"));
        assert_eq!(chosen_action(&card, "  Yes, fix 1\n").map(|a| a.label.as_str()), Some("Yes"));
        assert_eq!(chosen_action(&card, "yes, fix 1"), None);
        assert_eq!(chosen_action(&card, "something else"), None);
    }

    #[test]
    fn the_card_carries_the_wrap_up_call_id() {
        let mut call = wrap(json!({"recap": "r", "ask": "q"}));
        call.id = "toolu_9".to_string();
        assert_eq!(end_card_of(&call).unwrap().id, "toolu_9");
    }

    #[test]
    fn a_handled_card_is_not_offered_again() {
        let mut call = wrap(json!({"recap": "r", "ask": "q"}));
        call.id = "w1".to_string();
        let card = end_card_of(&call);
        let mut marks = std::collections::HashMap::new();
        assert!(offered(card.clone(), &marks).is_some());
        marks.insert("other".to_string(), CardMark::Dismissed);
        assert!(offered(card.clone(), &marks).is_some(), "another call's mark does not count");
        marks.insert("w1".to_string(), CardMark::Dismissed);
        assert_eq!(offered(card.clone(), &marks), None);
        marks.insert("w1".to_string(), CardMark::Sent);
        assert_eq!(offered(card, &marks), None);
        assert_eq!(offered(None, &marks), None);
    }

    #[test]
    fn a_cut_message_is_trimmed_after_the_cut() {
        let message = format!("{} tail", "a".repeat(299));
        let card = end_card_of(&wrap(json!({"recap": "r", "ask": "q", "actions": [{"label": "Go", "message": message}]}))).unwrap();
        assert_eq!(card.actions[0].message, "a".repeat(299), "no space left at the end");
        assert_eq!(chosen_action(&card, &card.actions[0].message.clone()).map(|a| a.label.as_str()), Some("Go"));
    }

    #[test]
    fn the_recap_and_the_question_are_clipped() {
        let card = end_card_of(&wrap(json!({"recap": "R".repeat(700), "ask": "Q".repeat(150)}))).unwrap();
        assert_eq!(card.recap.chars().count(), 600);
        assert_eq!(card.ask.chars().count(), 100);
    }

    #[test]
    fn a_button_that_would_send_a_command_is_dropped() {
        let card = end_card_of(&wrap(json!({"recap": "r", "ask": "q", "actions": [
            {"label": "Rewind", "message": "/rewind"},
            {"label": "Run", "message": "  !rm -rf x"},
            {"label": "Yes", "message": "Yes"},
        ]})))
        .unwrap();
        assert_eq!(card.actions, vec![CardAction { label: "Yes".to_string(), message: "Yes".to_string() }]);
        let only_label = end_card_of(&wrap(json!({"recap": "r", "ask": "q", "actions": [{"label": "/compact"}]}))).unwrap();
        assert!(only_label.actions.is_empty(), "a label sent as the message counts too");
    }
}
