use forge_domain::SlashCommand;

/// The text after the `/` while the input is still just a command name: it starts with `/` and has no
/// whitespace. `None` means the menu is closed.
pub fn command_query(input: &str) -> Option<&str> {
    let rest = input.strip_prefix('/')?;
    if rest.chars().any(char::is_whitespace) {
        None
    } else {
        Some(rest)
    }
}

/// Commands whose name starts with `query`, then those that contain it; case-insensitive, each group
/// in the order the SDK gave.
pub fn filter_commands<'a>(commands: &'a [SlashCommand], query: &str) -> Vec<&'a SlashCommand> {
    let query = query.to_lowercase();
    let mut prefix = Vec::new();
    let mut inner = Vec::new();
    for command in commands {
        let name = command.name.to_lowercase();
        if name.starts_with(&query) {
            prefix.push(command);
        } else if name.contains(&query) {
            inner.push(command);
        }
    }
    prefix.extend(inner);
    prefix
}

/// The question of a `/btw` message: the text after `/btw `, trimmed. `None` for any other text and
/// for `/btw` with nothing after it.
pub fn btw_question(input: &str) -> Option<&str> {
    let rest = input.trim_start().strip_prefix("/btw")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let question = rest.trim();
    (!question.is_empty()).then_some(question)
}

/// Whether the message is `/btw` with no question.
pub fn is_bare_btw(input: &str) -> bool {
    input.trim() == "/btw"
}

/// Whether the message is just `/rewind`.
pub fn is_rewind(input: &str) -> bool {
    input.trim() == "/rewind"
}

/// The names the desktop app lists first, in its order.
const DESKTOP_ORDER: [&str; 13] =
    ["btw", "ultrareview", "rewind", "fork", "subtask", "mcp", "plugin", "feedback", "model", "effort", "fast", "plan", "permissions"];

/// The list the menu shows: Forge's own commands (`local`, which win over a same-named one from the
/// SDK), then the desktop app's usual ones in its order, then everything else as the SDK gave it.
pub fn arrange_commands(sdk: &[SlashCommand], local: Vec<SlashCommand>) -> Vec<SlashCommand> {
    let mut all = local;
    for command in sdk {
        if !all.iter().any(|known| known.name == command.name) {
            all.push(command.clone());
        }
    }
    // A stable sort: the names the desktop knows go first, the rest keep their place.
    all.sort_by_key(|command| DESKTOP_ORDER.iter().position(|name| *name == command.name).unwrap_or(DESKTOP_ORDER.len()));
    all
}

/// The selected row after moving `delta` rows, wrapping around; always inside `0..len` (0 for no rows).
pub fn move_selection(current: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let current = current.min(len - 1) as isize;
    (current + delta).rem_euclid(len as isize) as usize
}

/// What the message box holds after a command is picked.
pub fn insert_text(command: &SlashCommand) -> String {
    format!("/{} ", command.name)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKey {
    Up,
    Down,
    Enter,
    Tab,
    Escape,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    Move(isize),
    Pick,
    Close,
}

/// What an open menu does with a key. Enter or Tab that confirms an IME composition (Korean, Japanese,
/// Chinese) belongs to the composition, and with Shift held Enter adds a line, so neither picks.
pub fn menu_action(key: MenuKey, composing: bool, shift: bool) -> Option<MenuAction> {
    match key {
        MenuKey::Up => Some(MenuAction::Move(-1)),
        MenuKey::Down => Some(MenuAction::Move(1)),
        MenuKey::Enter | MenuKey::Tab if !composing && !shift => Some(MenuAction::Pick),
        MenuKey::Escape => Some(MenuAction::Close),
        _ => None,
    }
}

/// The popup's rows; empty means closed. It is open only for text the person typed (`typed` is that
/// text, `None` when the box's text changed some other way since): text that arrives another way, like
/// a failed send put back in the box, must not open it.
pub fn menu_rows<'a>(commands: &'a [SlashCommand], value: &str, typed: Option<&str>, dismissed: bool) -> Vec<&'a SlashCommand> {
    if dismissed || typed != Some(value) {
        return Vec::new();
    }
    match command_query(value) {
        Some(query) => filter_commands(commands, query),
        None => Vec::new(),
    }
}

/// What counts as typed once the box's text is `value`: the typed text if it is still what the box
/// holds, otherwise nothing (a pick, a send that cleared the box, a failed send put back).
pub fn typed_after_change(typed: Option<&str>, value: &str) -> Option<String> {
    typed.filter(|typed| *typed == value).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str) -> SlashCommand {
        SlashCommand { name: name.to_string(), description: None, argument_hint: None }
    }

    #[test]
    fn a_btw_without_a_question_is_bare() {
        assert!(is_bare_btw("/btw") && is_bare_btw(" /btw \n"));
        assert!(!is_bare_btw("/btw hi") && !is_bare_btw("/btwx"));
    }

    #[test]
    fn rewind_is_the_whole_message() {
        assert!(is_rewind("/rewind"));
        assert!(is_rewind("  /rewind \n"));
        assert!(!is_rewind("/rewind now"));
        assert!(!is_rewind("/rewinds"));
    }

    #[test]
    fn btw_takes_the_text_after_it_as_the_question() {
        assert_eq!(btw_question("/btw what is 2+2?"), Some("what is 2+2?"));
        assert_eq!(btw_question("  /btw   spaced  "), Some("spaced"));
        assert_eq!(btw_question("/btw"), None, "nothing to ask");
        assert_eq!(btw_question("/btw "), None);
        assert_eq!(btw_question("/btwx hi"), None, "another command");
        assert_eq!(btw_question("tell me /btw"), None);
        assert_eq!(btw_question("/btw\nline two"), Some("line two"));
    }

    #[test]
    fn arranged_commands_put_forges_own_first_then_the_desktops_usual_ones_then_the_rest() {
        let sdk = vec![cmd("design"), cmd("model"), cmd("compact"), cmd("btw"), cmd("mcp"), cmd("code-review")];
        let local = vec![SlashCommand { name: "btw".to_string(), description: Some("side".to_string()), argument_hint: None }, cmd("rewind")];
        let names: Vec<_> = arrange_commands(&sdk, local).into_iter().map(|c| c.name).collect();
        assert_eq!(names, vec!["btw", "rewind", "mcp", "model", "design", "compact", "code-review"]);
        let kept = arrange_commands(&sdk, vec![SlashCommand { name: "btw".to_string(), description: Some("side".to_string()), argument_hint: None }]);
        assert_eq!(kept[0].description.as_deref(), Some("side"), "Forge's own entry replaces the SDK's");
    }

    #[test]
    fn the_query_is_the_text_after_the_slash_while_it_is_one_word() {
        assert_eq!(command_query("/"), Some(""));
        assert_eq!(command_query("/com"), Some("com"));
        assert_eq!(command_query("/compact now"), None, "a space ends the command name");
        assert_eq!(command_query("/a\nb"), None);
        assert_eq!(command_query("hello /x"), None);
        assert_eq!(command_query(""), None);
        assert_eq!(command_query("/가나"), Some("가나"));
    }

    #[test]
    fn prefix_matches_come_before_substring_matches_in_the_given_order() {
        let commands = vec![cmd("recompact"), cmd("compact"), cmd("context"), cmd("review")];
        let names: Vec<_> = filter_commands(&commands, "co").iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["compact", "context", "recompact"]);
    }

    #[test]
    fn filtering_ignores_case_and_an_empty_query_lists_everything() {
        let commands = vec![cmd("Compact"), cmd("review")];
        assert_eq!(filter_commands(&commands, "COM").len(), 1);
        assert_eq!(filter_commands(&commands, "").len(), 2);
        assert!(filter_commands(&commands, "zzz").is_empty());
    }

    #[test]
    fn the_selection_wraps_in_both_directions_and_survives_an_empty_list() {
        assert_eq!(move_selection(0, 3, 1), 1);
        assert_eq!(move_selection(2, 3, 1), 0);
        assert_eq!(move_selection(0, 3, -1), 2);
        assert_eq!(move_selection(5, 3, 0), 2, "a stale index is brought back into range");
        assert_eq!(move_selection(0, 0, 1), 0);
    }

    #[test]
    fn a_picked_command_becomes_a_slash_name_and_a_space() {
        assert_eq!(insert_text(&cmd("compact")), "/compact ");
    }

    #[test]
    fn keys_map_to_menu_actions_and_enter_during_an_ime_composition_does_nothing() {
        assert_eq!(menu_action(MenuKey::Up, false, false), Some(MenuAction::Move(-1)));
        assert_eq!(menu_action(MenuKey::Down, false, false), Some(MenuAction::Move(1)));
        assert_eq!(menu_action(MenuKey::Enter, false, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Tab, false, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Escape, false, false), Some(MenuAction::Close));
        assert_eq!(menu_action(MenuKey::Other, false, false), None);
        assert_eq!(menu_action(MenuKey::Enter, true, false), None);
        assert_eq!(menu_action(MenuKey::Tab, true, false), None);
    }

    #[test]
    fn enter_and_tab_with_shift_held_do_not_pick() {
        assert_eq!(menu_action(MenuKey::Enter, false, true), None, "Shift+Enter adds a line");
        assert_eq!(menu_action(MenuKey::Tab, false, true), None);
        assert_eq!(menu_action(MenuKey::Down, false, true), Some(MenuAction::Move(1)));
    }

    #[test]
    fn the_popup_rows_are_the_matches_for_text_the_person_typed() {
        let commands = vec![cmd("compact"), cmd("review")];
        let names = |rows: Vec<&SlashCommand>| rows.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(menu_rows(&commands, "/c", Some("/c"), false)), vec!["compact"]);
        assert_eq!(names(menu_rows(&commands, "/", Some("/"), false)), vec!["compact", "review"]);
    }

    #[test]
    fn the_popup_is_closed_after_escape_for_text_put_back_and_after_a_space() {
        let commands = vec![cmd("compact")];
        assert!(menu_rows(&commands, "/c", Some("/c"), true).is_empty(), "dismissed with Esc");
        assert!(menu_rows(&commands, "/compact", None, false).is_empty(), "nothing was typed since the text last changed by itself");
        assert!(menu_rows(&commands, "/compact", Some(""), false).is_empty(), "text put back differs from what was typed");
        assert!(menu_rows(&commands, "/compact now", Some("/compact now"), false).is_empty());
        assert!(menu_rows(&commands, "hello", Some("hello"), false).is_empty());
    }

    #[test]
    fn what_was_typed_is_forgotten_as_soon_as_the_text_changes_without_a_keystroke() {
        // Typed "/compact", picked it ("/compact "), sent (text cleared), the send failed and the text came back.
        assert_eq!(typed_after_change(Some("/compact"), "/compact"), Some("/compact".to_string()));
        assert_eq!(typed_after_change(Some("/compact"), "/compact "), None, "a pick");
        assert_eq!(typed_after_change(Some("/compact"), ""), None, "a send cleared the box");
        assert_eq!(typed_after_change(None, "/compact"), None, "so the text put back is not typed text");
        assert_eq!(typed_after_change(None, ""), None);
    }
}
