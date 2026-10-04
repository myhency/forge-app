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

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: &str = "Message Claude…";

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
