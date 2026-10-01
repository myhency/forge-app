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
/// Chinese) belongs to the composition, so it does nothing here.
pub fn menu_action(key: MenuKey, composing: bool) -> Option<MenuAction> {
    match key {
        MenuKey::Up => Some(MenuAction::Move(-1)),
        MenuKey::Down => Some(MenuAction::Move(1)),
        MenuKey::Enter | MenuKey::Tab if !composing => Some(MenuAction::Pick),
        MenuKey::Escape => Some(MenuAction::Close),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(name: &str) -> SlashCommand {
        SlashCommand { name: name.to_string(), description: None, argument_hint: None }
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
        assert_eq!(menu_action(MenuKey::Up, false), Some(MenuAction::Move(-1)));
        assert_eq!(menu_action(MenuKey::Down, false), Some(MenuAction::Move(1)));
        assert_eq!(menu_action(MenuKey::Enter, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Tab, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Escape, false), Some(MenuAction::Close));
        assert_eq!(menu_action(MenuKey::Other, false), None);
        assert_eq!(menu_action(MenuKey::Enter, true), None);
        assert_eq!(menu_action(MenuKey::Tab, true), None);
    }
}
