use crate::dock::Panel;
use forge_shared::{msg, tr};

/// What a shortcut does. `GoToSession` counts from 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    NewSessionHere,
    NextSession,
    PreviousSession,
    CloseSession,
    OpenSettings,
    OpenSearch,
    ToggleSidebar,
    ShowShortcuts,
    GoToSession(usize),
    ReopenClosed,
    ArchiveActive,
    ForkActive,
    RenameActive,
    TogglePanel(Panel),
}

/// A key press: the key's code in the table's spelling (`"KeyN"`) and the modifiers held. `match_code`
/// makes the code: the letter on the keycap when that is a plain ASCII letter (so AZERTY and Dvorak
/// keep their shortcuts), the physical key otherwise (Option changes the typed character on a Mac and
/// an input method such as Korean replaces it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyPress<'a> {
    pub code: &'a str,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

/// One key of a shortcut. `command` is Cmd on macOS and Ctrl elsewhere; `ctrl` is the Control key
/// itself on every platform. `code` `DIGITS` stands for the keys 1 to 9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub code: &'static str,
    pub command: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

pub const DIGITS: &str = "Digit*";

const fn binding(code: &'static str, command: bool, ctrl: bool, alt: bool, shift: bool) -> Binding {
    Binding { code, command, ctrl, alt, shift }
}
const fn cmd(code: &'static str) -> Binding {
    binding(code, true, false, false, false)
}
const fn cmd_shift(code: &'static str) -> Binding {
    binding(code, true, false, false, true)
}
const fn cmd_alt(code: &'static str) -> Binding {
    binding(code, true, false, true, false)
}
const fn ctrl(code: &'static str) -> Binding {
    binding(code, false, true, false, false)
}
const fn ctrl_shift(code: &'static str) -> Binding {
    binding(code, false, true, false, true)
}
const fn alt(code: &'static str) -> Binding {
    binding(code, false, false, true, false)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    General,
    Panels,
}

impl Group {
    pub fn title(self) -> &'static str {
        match self {
            Group::General => tr!("General"),
            Group::Panels => tr!("Panels"),
        }
    }
}

/// One row of the shortcut list and the action it stands for.
#[derive(Debug, Clone, Copy)]
pub struct ShortcutRow {
    pub group: Group,
    pub label: &'static str,
    pub action: Shortcut,
    pub keys: &'static [Binding],
}

/// Every shortcut Forge has. The key handler and the shortcut list both read this table, so a key
/// that is not here is not bound and not shown.
pub static SHORTCUTS: &[ShortcutRow] = &[
    ShortcutRow { group: Group::General, label: msg!("Search sessions"), action: Shortcut::OpenSearch, keys: &[cmd("KeyK")] },
    ShortcutRow { group: Group::General, label: msg!("New session"), action: Shortcut::NewSession, keys: &[cmd("KeyN")] },
    ShortcutRow { group: Group::General, label: msg!("New session in this folder"), action: Shortcut::NewSessionHere, keys: &[cmd_shift("KeyN")] },
    ShortcutRow { group: Group::General, label: msg!("Close the session"), action: Shortcut::CloseSession, keys: &[cmd("KeyW")] },
    ShortcutRow { group: Group::General, label: msg!("Settings"), action: Shortcut::OpenSettings, keys: &[cmd("Comma")] },
    ShortcutRow { group: Group::General, label: msg!("Keyboard shortcuts"), action: Shortcut::ShowShortcuts, keys: &[cmd("Slash")] },
    ShortcutRow { group: Group::General, label: msg!("Show or hide the sidebar"), action: Shortcut::ToggleSidebar, keys: &[cmd("KeyB")] },
    ShortcutRow {
        group: Group::General,
        label: msg!("Next session"),
        action: Shortcut::NextSession,
        keys: &[ctrl("Tab"), cmd_shift("BracketRight"), alt("ArrowDown")],
    },
    ShortcutRow {
        group: Group::General,
        label: msg!("Previous session"),
        action: Shortcut::PreviousSession,
        keys: &[ctrl_shift("Tab"), cmd_shift("BracketLeft"), alt("ArrowUp")],
    },
    ShortcutRow { group: Group::General, label: msg!("Go to session 1…9"), action: Shortcut::GoToSession(0), keys: &[cmd(DIGITS)] },
    ShortcutRow { group: Group::General, label: msg!("Reopen the last closed session"), action: Shortcut::ReopenClosed, keys: &[cmd_shift("KeyT")] },
    ShortcutRow { group: Group::General, label: msg!("Archive or unarchive the session"), action: Shortcut::ArchiveActive, keys: &[cmd_alt("KeyA")] },
    ShortcutRow { group: Group::General, label: msg!("Fork the session"), action: Shortcut::ForkActive, keys: &[cmd_alt("KeyO")] },
    ShortcutRow { group: Group::General, label: msg!("Rename the session"), action: Shortcut::RenameActive, keys: &[cmd_alt("KeyR")] },
    ShortcutRow { group: Group::Panels, label: msg!("Changes"), action: Shortcut::TogglePanel(Panel::Changes), keys: &[cmd_shift("KeyD")] },
    ShortcutRow { group: Group::Panels, label: msg!("Files"), action: Shortcut::TogglePanel(Panel::Files), keys: &[cmd_shift("KeyF")] },
    ShortcutRow { group: Group::Panels, label: msg!("Terminal"), action: Shortcut::TogglePanel(Panel::Terminal), keys: &[cmd("KeyJ"), ctrl("Backquote")] },
    ShortcutRow { group: Group::Panels, label: msg!("Preview"), action: Shortcut::TogglePanel(Panel::Preview), keys: &[cmd_shift("KeyB")] },
];

impl Binding {
    // The Meta and Ctrl keys this binding asks for on the platform.
    fn wanted(&self, mac: bool) -> (bool, bool) {
        if self.command {
            if mac { (true, false) } else { (false, true) }
        } else {
            (false, self.ctrl)
        }
    }

    fn modifiers_match(&self, press: &KeyPress, mac: bool) -> bool {
        let (meta, ctrl) = self.wanted(mac);
        press.meta == meta && press.ctrl == ctrl && press.alt == self.alt && press.shift == self.shift
    }

    /// The key as the list shows it: `⌥⌘A` on a Mac, `Ctrl+Alt+A` elsewhere.
    pub fn display(&self, mac: bool) -> String {
        let (meta, ctrl) = self.wanted(mac);
        let key = key_label(self.code);
        if mac {
            let mut text = String::new();
            if ctrl { text.push('⌃'); }
            if self.alt { text.push('⌥'); }
            if self.shift { text.push('⇧'); }
            if meta { text.push('⌘'); }
            text.push_str(&key);
            text
        } else {
            let mut parts = Vec::new();
            if ctrl { parts.push("Ctrl".to_string()); }
            if self.alt { parts.push("Alt".to_string()); }
            if self.shift { parts.push("Shift".to_string()); }
            parts.push(key);
            parts.join("+")
        }
    }
}

fn key_label(code: &str) -> String {
    match code {
        DIGITS => "1…9".to_string(),
        "ArrowDown" => "↓".to_string(),
        "ArrowUp" => "↑".to_string(),
        "Comma" => ",".to_string(),
        "Slash" => "/".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "Backquote" => "`".to_string(),
        other => other.strip_prefix("Key").unwrap_or(other).to_string(),
    }
}

// The 0-based place a `DigitN` code stands for (`Digit1` is 0); 0 and anything else is none.
fn digit_index(code: &str) -> Option<usize> {
    let number: usize = code.strip_prefix("Digit")?.parse().ok()?;
    (1..=9).contains(&number).then(|| number - 1)
}

/// The code a key press is matched by, in `event.code()` spelling. The letter the key is labelled with
/// wins when it is an ASCII letter, one of the table's unshifted punctuation keys, or a digit 1 to 9,
/// so Cmd+Z stays on the key marked Z on AZERTY. Everything else uses `code`: Option or Alt
/// combinations (the character is changed), an input method's "Process" or Hangul, shifted symbols,
/// arrows, Tab, Escape.
pub fn match_code(key: &str, code: &str, alt: bool) -> String {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() && !alt {
            return format!("Key{}", c.to_ascii_uppercase());
        }
        if matches!(c, '1'..='9') {
            return format!("Digit{c}");
        }
        let punctuation = match c {
            ',' => Some("Comma"),
            '/' => Some("Slash"),
            '[' => Some("BracketLeft"),
            ']' => Some("BracketRight"),
            '`' => Some("Backquote"),
            _ => None,
        };
        if let (Some(name), false) = (punctuation, alt) {
            return name.to_string();
        }
    }
    code.to_string()
}

/// Whether a key press typed a character instead of asking for a shortcut. On Windows AltGr arrives as
/// Ctrl+Alt, and AltGr+A types "ą" on a Polish layout; Ctrl+Alt with a plain ASCII letter or digit is
/// the shortcut. On a Mac Option text is expected and the shortcuts still apply.
pub fn types_text(key: &str, ctrl: bool, alt: bool, mac: bool) -> bool {
    if mac || !ctrl || !alt {
        return false;
    }
    let mut chars = key.chars();
    matches!((chars.next(), chars.next()), (Some(c), None) if !c.is_ascii_alphanumeric() && !c.is_control())
}

/// Whether the key press of a shortcut that `shortcut_allowed` refused is swallowed (default prevented).
/// The session-cycling keys are not: Alt+Up and Alt+Down belong to the text field behind them.
pub fn swallowed_behind_overlay(shortcut: Shortcut) -> bool {
    !matches!(shortcut, Shortcut::NextSession | Shortcut::PreviousSession)
}

/// The shortcut a key press is, if any. "Cmd" is Ctrl on platforms other than macOS.
pub fn shortcut_for(press: &KeyPress, mac: bool) -> Option<Shortcut> {
    for row in SHORTCUTS {
        for binding in row.keys {
            if !binding.modifiers_match(press, mac) {
                continue;
            }
            if binding.code == DIGITS {
                if let Some(index) = digit_index(press.code) {
                    return Some(Shortcut::GoToSession(index));
                }
            } else if binding.code == press.code {
                return Some(row.action);
            }
        }
    }
    None
}

/// Whether `shortcut` may act while an overlay (Settings, the search palette, the shortcut list) is
/// open. Only opening settings and the list do: the session shortcuts would change the session
/// hidden behind the overlay (Plan 6 found ⌘W closing it).
pub fn shortcut_allowed(shortcut: Shortcut, overlay_open: bool) -> bool {
    !overlay_open || matches!(shortcut, Shortcut::OpenSettings | Shortcut::ShowShortcuts)
}

/// One group of the shortcut list: rows of (label, keys text).
#[derive(Debug, Clone, PartialEq)]
pub struct ShortcutSection {
    pub title: &'static str,
    pub rows: Vec<(String, String)>,
}

/// The shortcut list as the dialog draws it, groups in order, alternatives joined with " or ".
pub fn shortcut_sections(mac: bool) -> Vec<ShortcutSection> {
    [Group::General, Group::Panels]
        .into_iter()
        .map(|group| ShortcutSection {
            title: group.title(),
            rows: SHORTCUTS
                .iter()
                .filter(|row| row.group == group)
                .map(|row| (forge_shared::i18n::text(row.label).to_string(), row.keys.iter().map(|binding| binding.display(mac)).collect::<Vec<_>>().join(tr!(" or "))))
                .collect(),
        })
        .collect()
}

/// Whether the platform's command key is held: Cmd on macOS, Ctrl elsewhere (Meta there is the Windows key).
pub fn command_pressed(ctrl: bool, meta: bool, mac: bool) -> bool {
    if mac { meta } else { ctrl }
}

/// `command_pressed` for the key press `modifiers` describe, on the platform Forge runs on.
pub fn command_held(modifiers: dioxus::prelude::Modifiers) -> bool {
    command_pressed(modifiers.ctrl(), modifiers.meta(), cfg!(target_os = "macos"))
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    fn press(code: &str, ctrl: bool, meta: bool, alt: bool, shift: bool) -> KeyPress<'_> {
        KeyPress { code, ctrl, meta, alt, shift }
    }

    /// The press a binding stands for on `mac`.
    fn press_of(binding: &Binding, code: &'static str, mac: bool) -> KeyPress<'static> {
        let (meta, ctrl) = if binding.command { if mac { (true, false) } else { (false, true) } } else { (false, binding.ctrl) };
        KeyPress { code, ctrl, meta, alt: binding.alt, shift: binding.shift }
    }

    #[test]
    fn the_command_key_is_meta_on_a_mac_and_ctrl_elsewhere() {
        assert!(command_pressed(false, true, true));
        assert!(!command_pressed(true, false, true));
        assert!(command_pressed(true, false, false));
        assert!(!command_pressed(false, true, false), "the Windows key is not the command key");
    }

    #[test]
    fn cmd_n_and_cmd_w_on_a_mac() {
        assert_eq!(shortcut_for(&press("KeyN", false, true, false, false), true), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for(&press("KeyW", false, true, false, false), true), Some(Shortcut::CloseSession));
    }

    #[test]
    fn ctrl_n_and_ctrl_w_elsewhere() {
        assert_eq!(shortcut_for(&press("KeyN", true, false, false, false), false), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for(&press("KeyW", true, false, false, false), false), Some(Shortcut::CloseSession));
    }

    #[test]
    fn the_wrong_modifier_for_the_platform_does_nothing() {
        assert_eq!(shortcut_for(&press("KeyN", true, false, false, false), true), None, "Ctrl+N on a Mac");
        assert_eq!(shortcut_for(&press("KeyW", false, true, false, false), false), None, "Meta+W elsewhere");
    }

    #[test]
    fn plain_keys_and_unbound_commands_do_nothing() {
        assert_eq!(shortcut_for(&press("KeyN", false, false, false, false), true), None);
        assert_eq!(shortcut_for(&press("KeyX", false, true, false, false), true), None);
        assert_eq!(shortcut_for(&press("KeyN", false, true, true, false), true), None, "an extra Option turns Cmd+N into something else");
    }

    #[test]
    fn shift_cmd_n_starts_a_session_in_this_folder() {
        assert_eq!(shortcut_for(&press("KeyN", false, true, false, true), true), Some(Shortcut::NewSessionHere));
    }

    #[test]
    fn ctrl_tab_cycles_on_every_platform_and_shift_reverses() {
        for mac in [true, false] {
            assert_eq!(shortcut_for(&press("Tab", true, false, false, false), mac), Some(Shortcut::NextSession));
            assert_eq!(shortcut_for(&press("Tab", true, false, false, true), mac), Some(Shortcut::PreviousSession));
        }
    }

    #[test]
    fn tab_without_ctrl_is_left_alone() {
        assert_eq!(shortcut_for(&press("Tab", false, false, false, false), true), None);
        assert_eq!(shortcut_for(&press("Tab", false, true, false, false), true), None);
    }

    #[test]
    fn the_other_keys_for_next_and_previous_session() {
        for mac in [true, false] {
            assert_eq!(shortcut_for(&press("ArrowDown", false, false, true, false), mac), Some(Shortcut::NextSession), "Option+Down");
            assert_eq!(shortcut_for(&press("ArrowUp", false, false, true, false), mac), Some(Shortcut::PreviousSession), "Option+Up");
        }
        assert_eq!(shortcut_for(&press("BracketRight", false, true, false, true), true), Some(Shortcut::NextSession));
        assert_eq!(shortcut_for(&press("BracketLeft", false, true, false, true), true), Some(Shortcut::PreviousSession));
        assert_eq!(shortcut_for(&press("ArrowDown", false, false, false, false), true), None, "plain Down is the caret's");
    }

    #[test]
    fn cmd_comma_opens_settings_and_cmd_k_the_search() {
        assert_eq!(shortcut_for(&press("Comma", false, true, false, false), true), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(&press("Comma", true, false, false, false), false), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(&press("Comma", true, false, false, false), true), None, "Ctrl+, on a Mac");
        assert_eq!(shortcut_for(&press("KeyK", false, true, false, false), true), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for(&press("KeyK", true, false, false, false), false), Some(Shortcut::OpenSearch));
    }

    #[test]
    fn the_digits_go_to_the_session_at_that_place() {
        assert_eq!(shortcut_for(&press("Digit1", false, true, false, false), true), Some(Shortcut::GoToSession(0)));
        assert_eq!(shortcut_for(&press("Digit9", false, true, false, false), true), Some(Shortcut::GoToSession(8)));
        assert_eq!(shortcut_for(&press("Digit0", false, true, false, false), true), None);
        assert_eq!(shortcut_for(&press("Digit1", false, false, false, false), true), None, "a typed digit");
    }

    #[test]
    fn option_combinations_match_by_the_physical_key() {
        // On a Mac Option+Cmd+A types "å"; the press carries the key's code, not the character.
        assert_eq!(shortcut_for(&press("KeyA", false, true, true, false), true), Some(Shortcut::ArchiveActive));
        assert_eq!(shortcut_for(&press("KeyO", false, true, true, false), true), Some(Shortcut::ForkActive));
        assert_eq!(shortcut_for(&press("KeyR", false, true, true, false), true), Some(Shortcut::RenameActive));
        assert_eq!(shortcut_for(&press("KeyA", true, false, true, false), false), Some(Shortcut::ArchiveActive), "Ctrl+Alt+A elsewhere");
    }

    #[test]
    fn the_panel_keys() {
        use crate::dock::Panel;
        assert_eq!(shortcut_for(&press("KeyD", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Changes)));
        assert_eq!(shortcut_for(&press("KeyF", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Files)));
        assert_eq!(shortcut_for(&press("KeyJ", false, true, false, false), true), Some(Shortcut::TogglePanel(Panel::Terminal)));
        assert_eq!(shortcut_for(&press("Backquote", true, false, false, false), true), Some(Shortcut::TogglePanel(Panel::Terminal)));
        assert_eq!(shortcut_for(&press("KeyB", false, true, false, true), true), Some(Shortcut::TogglePanel(Panel::Preview)));
        assert_eq!(shortcut_for(&press("KeyB", false, true, false, false), true), Some(Shortcut::ToggleSidebar));
    }

    #[test]
    fn the_rest_of_the_general_keys() {
        assert_eq!(shortcut_for(&press("Slash", false, true, false, false), true), Some(Shortcut::ShowShortcuts));
        assert_eq!(shortcut_for(&press("KeyT", false, true, false, true), true), Some(Shortcut::ReopenClosed));
    }

    #[test]
    fn every_binding_of_every_row_gives_that_row_back_on_both_platforms() {
        for mac in [true, false] {
            for row in SHORTCUTS {
                for binding in row.keys {
                    if binding.code == DIGITS {
                        continue;
                    }
                    let found = shortcut_for(&press_of(binding, binding.code, mac), mac);
                    assert_eq!(found, Some(row.action), "{} on mac={mac}", row.label);
                }
            }
        }
    }

    #[test]
    fn no_two_bindings_share_a_key() {
        for mac in [true, false] {
            let mut seen = HashSet::new();
            for row in SHORTCUTS {
                for binding in row.keys {
                    let key = (binding.code, binding.wanted(mac), binding.alt, binding.shift);
                    assert!(seen.insert(key), "{} repeats a key (mac={mac})", row.label);
                }
            }
        }
    }

    #[test]
    fn match_code_follows_the_letter_on_the_keycap_not_the_place_of_the_key() {
        assert_eq!(match_code("z", "KeyW", false), "KeyZ", "AZERTY");
        assert_eq!(match_code("n", "KeyB", false), "KeyN", "Dvorak");
        assert_eq!(match_code("N", "KeyN", false), "KeyN", "uppercase");
        assert_eq!(match_code(",", "Comma", false), "Comma");
        assert_eq!(match_code("/", "Slash", false), "Slash");
        assert_eq!(match_code("[", "BracketLeft", false), "BracketLeft");
        assert_eq!(match_code("]", "BracketRight", false), "BracketRight");
        assert_eq!(match_code("`", "Backquote", false), "Backquote");
    }

    #[test]
    fn match_code_falls_back_to_the_physical_key() {
        assert_eq!(match_code("ㅊ", "KeyC", false), "KeyC", "Korean IME");
        assert_eq!(match_code("Process", "KeyC", false), "KeyC", "IME in progress");
        assert_eq!(match_code("å", "KeyA", true), "KeyA", "Option changes the character");
        assert_eq!(match_code("a", "KeyQ", true), "KeyQ", "with Alt held the code decides");
        assert_eq!(match_code("{", "BracketLeft", false), "BracketLeft", "shifted symbol");
        assert_eq!(match_code("ArrowUp", "ArrowUp", false), "ArrowUp");
        assert_eq!(match_code("Tab", "Tab", false), "Tab");
        assert_eq!(match_code("Escape", "Escape", false), "Escape");
    }

    #[test]
    fn match_code_digits() {
        assert_eq!(match_code("1", "Digit1", false), "Digit1");
        assert_eq!(match_code("&", "Digit1", false), "Digit1", "AZERTY");
        assert_eq!(match_code("0", "Digit0", false), "Digit0", "0 is no shortcut, the code is kept");
    }

    #[test]
    fn altgr_text_is_not_a_shortcut_off_a_mac() {
        assert!(types_text("ą", true, true, false), "Polish AltGr+A");
        assert!(types_text("ó", true, true, false), "Polish AltGr+O");
        assert!(types_text("@", true, true, false), "AltGr+Q on a German layout");
        assert!(!types_text("a", true, true, false), "plain Ctrl+Alt+A");
        assert!(!types_text("A", true, true, false));
        assert!(!types_text("ArrowUp", true, true, false), "not a single character");
        assert!(!types_text("ą", false, true, false), "Alt alone");
        assert!(!types_text("ą", true, false, false), "Ctrl alone");
        assert_eq!(shortcut_for(&press("KeyA", true, false, true, false), false), Some(Shortcut::ArchiveActive));
    }

    #[test]
    fn option_text_is_expected_on_a_mac() {
        assert!(!types_text("å", true, true, true));
        assert!(!types_text("å", false, true, true), "⌥⌘A types å on a Mac and is still the shortcut");
    }

    #[test]
    fn a_key_the_overlay_blocks_is_swallowed_except_the_session_cycling_keys() {
        use crate::dock::Panel;
        assert!(!swallowed_behind_overlay(Shortcut::NextSession));
        assert!(!swallowed_behind_overlay(Shortcut::PreviousSession));
        assert!(swallowed_behind_overlay(Shortcut::CloseSession));
        assert!(swallowed_behind_overlay(Shortcut::GoToSession(0)));
        assert!(swallowed_behind_overlay(Shortcut::TogglePanel(Panel::Files)));
    }

    #[test]
    fn every_row_has_a_label_and_at_least_one_key() {
        for row in SHORTCUTS {
            assert!(!row.label.is_empty());
            assert!(!row.keys.is_empty(), "{}", row.label);
        }
    }

    #[test]
    fn while_an_overlay_is_open_only_opening_settings_and_this_list_are_allowed() {
        use crate::dock::Panel;
        let mut actions: Vec<Shortcut> = SHORTCUTS.iter().map(|row| row.action).collect();
        actions.push(Shortcut::GoToSession(3));
        actions.push(Shortcut::TogglePanel(Panel::Files));
        for action in actions {
            assert!(shortcut_allowed(action, false), "{action:?} with no overlay");
            let allowed = matches!(action, Shortcut::OpenSettings | Shortcut::ShowShortcuts);
            assert_eq!(shortcut_allowed(action, true), allowed, "{action:?} behind an overlay");
        }
    }

    #[test]
    fn keys_are_shown_the_platform_way() {
        let find = |label: &str| SHORTCUTS.iter().find(|row| row.label == label).unwrap();
        assert_eq!(find("Archive or unarchive the session").keys[0].display(true), "⌥⌘A");
        assert_eq!(find("Archive or unarchive the session").keys[0].display(false), "Ctrl+Alt+A");
        assert_eq!(find("New session in this folder").keys[0].display(true), "⇧⌘N");
        assert_eq!(find("New session in this folder").keys[0].display(false), "Ctrl+Shift+N");
        assert_eq!(find("Go to session 1…9").keys[0].display(true), "⌘1…9");
        assert_eq!(find("Next session").keys[0].display(true), "⌃Tab");
        assert_eq!(find("Next session").keys[0].display(false), "Ctrl+Tab");
        assert_eq!(find("Next session").keys[2].display(true), "⌥↓");
        assert_eq!(find("Terminal").keys[1].display(true), "⌃`");
    }

    #[test]
    fn the_dialog_lists_every_row_under_its_group_with_alternatives_joined() {
        let sections = shortcut_sections(true);
        assert_eq!(sections.iter().map(|section| section.title).collect::<Vec<_>>(), vec!["General", "Panels"]);
        let total: usize = sections.iter().map(|section| section.rows.len()).sum();
        assert_eq!(total, SHORTCUTS.len());
        let next = sections[0].rows.iter().find(|(label, _)| label == "Next session").unwrap();
        assert_eq!(next.1, "⌃Tab or ⇧⌘] or ⌥↓");
    }
}
