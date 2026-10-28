/// What the message box says when it is empty: the SDK's suggested next prompt while no turn runs,
/// else `default`.
pub fn composer_placeholder(suggestion: Option<&str>, draft_empty: bool, running: bool, default: &str) -> String {
    match usable(suggestion) {
        Some(text) if draft_empty && !running => text.to_string(),
        _ => default.to_string(),
    }
}

/// The text Tab puts in the box: the suggestion, when the box is empty and no turn runs. Tab is
/// Forge's own choice of key (the desktop app's was not observed).
pub fn suggestion_to_accept(code: &str, suggestion: Option<&str>, draft_empty: bool, running: bool) -> Option<String> {
    if code != "Tab" || !draft_empty || running {
        return None;
    }
    usable(suggestion).map(str::to_string)
}

fn usable(suggestion: Option<&str>) -> Option<&str> {
    suggestion.filter(|text| !text.trim().is_empty())
}

/// The message `⌘⏎` sends while a next-step card is live: its first button's, when the box is empty and
/// no turn runs. With text in the box `⌘⏎` keeps queueing it.
pub fn end_card_reply(card: Option<&forge_entities::EndCard>, draft_empty: bool, running: bool) -> Option<String> {
    if !draft_empty || running {
        return None;
    }
    card?.actions.first().map(|action| action.message.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "Message Claude…";

    fn card(actions: &[(&str, &str)]) -> forge_entities::EndCard {
        forge_entities::EndCard {
            id: "w".to_string(),
            recap: "r".to_string(),
            ask: "q".to_string(),
            actions: actions.iter().map(|(label, message)| forge_entities::CardAction { label: label.to_string(), message: message.to_string() }).collect(),
        }
    }

    #[test]
    fn command_enter_in_an_empty_idle_box_sends_the_first_button_of_a_live_card() {
        let with = card(&[("Commit", "commit it"), ("Not now", "not now")]);
        assert_eq!(end_card_reply(Some(&with), true, false).as_deref(), Some("commit it"));
    }

    #[test]
    fn it_does_nothing_with_text_in_the_box_a_running_turn_no_card_or_no_buttons() {
        let with = card(&[("Commit", "commit it")]);
        assert_eq!(end_card_reply(Some(&with), false, false), None);
        assert_eq!(end_card_reply(Some(&with), true, true), None);
        assert_eq!(end_card_reply(None, true, false), None);
        assert_eq!(end_card_reply(Some(&card(&[])), true, false), None);
    }

    #[test]
    fn the_placeholder_is_the_suggestion_only_when_the_box_is_empty_and_nothing_runs() {
        assert_eq!(composer_placeholder(Some("run the tests"), true, false, DEFAULT), "run the tests");
        assert_eq!(composer_placeholder(Some("run the tests"), false, false, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(Some("run the tests"), true, true, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(None, true, false, DEFAULT), DEFAULT);
    }

    #[test]
    fn a_blank_suggestion_is_not_shown() {
        assert_eq!(composer_placeholder(Some("   "), true, false, DEFAULT), DEFAULT);
        assert_eq!(composer_placeholder(Some(""), true, false, DEFAULT), DEFAULT);
    }

    #[test]
    fn tab_in_an_empty_box_takes_the_suggestion() {
        assert_eq!(suggestion_to_accept("Tab", Some("run the tests"), true, false).as_deref(), Some("run the tests"));
    }

    #[test]
    fn no_other_key_and_no_other_state_takes_it() {
        assert_eq!(suggestion_to_accept("Enter", Some("x"), true, false), None);
        assert_eq!(suggestion_to_accept("Tab", Some("x"), false, false), None, "the box has text");
        assert_eq!(suggestion_to_accept("Tab", Some("x"), true, true), None, "a turn is running");
        assert_eq!(suggestion_to_accept("Tab", None, true, false), None);
        assert_eq!(suggestion_to_accept("Tab", Some("  "), true, false), None);
    }
}
