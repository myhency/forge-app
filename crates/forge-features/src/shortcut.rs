#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    NextSession,
    PreviousSession,
    CloseSession,
    OpenSettings,
    OpenSearch,
}

/// Claude Code desktop's session shortcuts: Cmd+N new session, Cmd+W close the session, Cmd+, open settings, Cmd+K search the sessions,
/// Ctrl+Tab / Ctrl+Shift+Tab cycle. "Cmd" is Ctrl on platforms other than macOS. `key` is the
/// pressed key: `"Tab"`, or a character in any case.
pub fn shortcut_for(key: &str, ctrl: bool, meta: bool, shift: bool, mac: bool) -> Option<Shortcut> {
    if key == "Tab" && ctrl {
        return Some(if shift { Shortcut::PreviousSession } else { Shortcut::NextSession });
    }
    let command = if mac { meta && !ctrl } else { ctrl && !meta };
    if !command || shift {
        return None;
    }
    match key.to_lowercase().as_str() {
        "n" => Some(Shortcut::NewSession),
        "w" => Some(Shortcut::CloseSession),
        "," => Some(Shortcut::OpenSettings),
        "k" => Some(Shortcut::OpenSearch),
        _ => None,
    }
}

/// Whether `shortcut` may act while the settings overlay is open. Only opening settings does: the
/// session shortcuts would change the session hidden behind the overlay (Plan 6 found ⌘W closing it).
pub fn shortcut_allowed(shortcut: Shortcut, overlay_open: bool) -> bool {
    !overlay_open || shortcut == Shortcut::OpenSettings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_n_and_cmd_w_on_a_mac() {
        assert_eq!(shortcut_for("n", false, true, false, true), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for("w", false, true, false, true), Some(Shortcut::CloseSession));
        assert_eq!(shortcut_for("N", false, true, false, true), Some(Shortcut::NewSession), "caps lock");
    }

    #[test]
    fn ctrl_n_and_ctrl_w_elsewhere() {
        assert_eq!(shortcut_for("n", true, false, false, false), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for("w", true, false, false, false), Some(Shortcut::CloseSession));
    }

    #[test]
    fn the_wrong_modifier_for_the_platform_does_nothing() {
        assert_eq!(shortcut_for("n", true, false, false, true), None, "Ctrl+N on a Mac");
        assert_eq!(shortcut_for("w", false, true, false, false), None, "Meta+W elsewhere");
    }

    #[test]
    fn plain_keys_and_shifted_commands_do_nothing() {
        assert_eq!(shortcut_for("n", false, false, false, true), None);
        assert_eq!(shortcut_for("n", false, true, true, true), None, "Cmd+Shift+N");
        assert_eq!(shortcut_for("x", false, true, false, true), None);
    }

    #[test]
    fn ctrl_tab_cycles_on_every_platform_and_shift_reverses() {
        for mac in [true, false] {
            assert_eq!(shortcut_for("Tab", true, false, false, mac), Some(Shortcut::NextSession));
            assert_eq!(shortcut_for("Tab", true, false, true, mac), Some(Shortcut::PreviousSession));
        }
    }

    #[test]
    fn tab_without_ctrl_is_left_alone() {
        assert_eq!(shortcut_for("Tab", false, false, false, true), None);
        assert_eq!(shortcut_for("Tab", false, true, false, true), None);
    }

    #[test]
    fn cmd_comma_opens_settings_on_a_mac_and_ctrl_comma_elsewhere() {
        assert_eq!(shortcut_for(",", false, true, false, true), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(",", true, false, false, false), Some(Shortcut::OpenSettings));
        assert_eq!(shortcut_for(",", true, false, false, true), None, "Ctrl+, on a Mac");
        assert_eq!(shortcut_for(",", false, true, true, true), None, "Cmd+Shift+,");
    }

    #[test]
    fn while_the_overlay_is_open_only_opening_settings_is_allowed() {
        for shortcut in [Shortcut::NewSession, Shortcut::NextSession, Shortcut::PreviousSession, Shortcut::CloseSession, Shortcut::OpenSearch] {
            assert!(shortcut_allowed(shortcut, false), "{shortcut:?} with the overlay closed");
            assert!(!shortcut_allowed(shortcut, true), "{shortcut:?} behind the overlay");
        }
        assert!(shortcut_allowed(Shortcut::OpenSettings, true));
        assert!(shortcut_allowed(Shortcut::OpenSettings, false));
    }

    #[test]
    fn cmd_k_opens_the_search_on_a_mac_and_ctrl_k_elsewhere() {
        assert_eq!(shortcut_for("k", false, true, false, true), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for("K", false, true, false, true), Some(Shortcut::OpenSearch), "caps lock");
        assert_eq!(shortcut_for("k", true, false, false, false), Some(Shortcut::OpenSearch));
        assert_eq!(shortcut_for("k", true, false, false, true), None, "Ctrl+K on a Mac");
        assert_eq!(shortcut_for("k", false, true, true, true), None, "Cmd+Shift+K");
    }
}
