use forge_domain::PermissionMode;
use forge_shared::{msg, tr};

const MODES: [(PermissionMode, &str, &str); 5] = [
    (PermissionMode::Default, "default", msg!("Ask before edits")),
    (PermissionMode::AcceptEdits, "accept_edits", msg!("Accept edits")),
    (PermissionMode::Plan, "plan", msg!("Plan mode")),
    (PermissionMode::BypassPermissions, "bypass_permissions", msg!("Bypass permissions")),
    (PermissionMode::Auto, "auto", msg!("Auto")),
];

/// `(value, label)` pairs for the mode selector. Bypass is left out unless `include_bypass`.
pub fn mode_options(include_bypass: bool) -> Vec<(String, String)> {
    MODES
        .iter()
        .filter(|(mode, _, _)| include_bypass || *mode != PermissionMode::BypassPermissions)
        .map(|(_, value, label)| (value.to_string(), forge_shared::i18n::text(label).to_string()))
        .collect()
}

pub fn mode_value(mode: PermissionMode) -> &'static str {
    MODES
        .iter()
        .find(|(candidate, _, _)| *candidate == mode)
        .map(|(_, value, _)| *value)
        .unwrap_or("default")
}

/// An unknown value falls back to `Default`, the safest mode.
pub fn parse_mode(value: &str) -> PermissionMode {
    MODES
        .iter()
        .find(|(_, candidate, _)| *candidate == value)
        .map(|(mode, _, _)| *mode)
        .unwrap_or(PermissionMode::Default)
}

/// One row of the mode picker in the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeRow {
    pub mode: PermissionMode,
    pub label: &'static str,
    pub description: &'static str,
    /// The number key that picks the row (1 to 4); the bypass row has none.
    pub key: Option<usize>,
    /// A divider is drawn above the row.
    pub divider_before: bool,
}

/// The modes in Claude Code desktop's order: four with number keys, then bypass below a divider.
/// The bypass row is left out unless `include_bypass` (the Settings switch, or the session is
/// already in that mode).
pub fn mode_rows(include_bypass: bool) -> Vec<ModeRow> {
    let row = |mode, label, description, key, divider_before| ModeRow { mode, label, description, key, divider_before };
    let mut rows = vec![
        row(PermissionMode::Auto, tr!("Auto"), tr!("Claude handles permission decisions"), Some(1), false),
        row(PermissionMode::Default, tr!("Ask before edits"), tr!("Always ask before making changes"), Some(2), false),
        row(PermissionMode::AcceptEdits, tr!("Accept edits"), tr!("Automatically accept all file edits"), Some(3), false),
        row(PermissionMode::Plan, tr!("Plan mode"), tr!("Make a plan before changing anything"), Some(4), false),
    ];
    if include_bypass {
        rows.push(row(PermissionMode::BypassPermissions, tr!("Bypass permissions"), tr!("Skip every permission check"), None, true));
    }
    rows
}

/// Whether the new-session screen offers a one-press "Turn on Auto mode": only while the session would
/// ask before edits. Plan, accept-edits and bypass are choices the person made on purpose, and Auto
/// needs no offer.
pub fn auto_mode_offered(mode: PermissionMode) -> bool {
    mode == PermissionMode::Default
}

/// A confirmation that was asked for stays valid only while the picker still has the row (the
/// Settings switch may have been turned off in between).
pub fn confirmation_still_valid(rows: &[ModeRow], mode: PermissionMode) -> bool {
    rows.iter().any(|row| row.mode == mode)
}

/// What choosing a mode does. Bypass turns every check off, so it is not applied by one click: the
/// picker asks first (Claude Code desktop has a separate "활성화" button for it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeChoice {
    Apply,
    AskFirst,
}

pub fn mode_choice(mode: PermissionMode) -> ModeChoice {
    if mode == PermissionMode::BypassPermissions {
        ModeChoice::AskFirst
    } else {
        ModeChoice::Apply
    }
}

/// The zero-based row a number key picks, from the physical key (`"Digit2"`, `"Numpad2"`): the
/// character it types depends on the input method. Only 1 to 4 are keys.
pub fn menu_digit_row(code: &str) -> Option<usize> {
    let digit = code.strip_prefix("Digit").or_else(|| code.strip_prefix("Numpad"))?;
    match digit {
        "1" => Some(0),
        "2" => Some(1),
        "3" => Some(2),
        "4" => Some(3),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_rows_follow_the_desktop_order_with_keys_one_to_four() {
        let rows = mode_rows(true);
        let modes: Vec<_> = rows.iter().map(|row| row.mode).collect();
        assert_eq!(
            modes,
            vec![
                PermissionMode::Auto,
                PermissionMode::Default,
                PermissionMode::AcceptEdits,
                PermissionMode::Plan,
                PermissionMode::BypassPermissions
            ]
        );
        let keys: Vec<_> = rows.iter().map(|row| row.key).collect();
        assert_eq!(keys, vec![Some(1), Some(2), Some(3), Some(4), None]);
        assert!(rows[4].divider_before && rows[..4].iter().all(|row| !row.divider_before));
        assert!(rows.iter().all(|row| !row.description.is_empty()));
    }

    #[test]
    fn auto_mode_is_offered_only_while_the_session_asks_before_edits() {
        assert!(auto_mode_offered(PermissionMode::Default));
        for mode in [PermissionMode::Auto, PermissionMode::AcceptEdits, PermissionMode::Plan, PermissionMode::BypassPermissions] {
            assert!(!auto_mode_offered(mode), "{mode:?}");
        }
    }

    #[test]
    fn without_the_switch_the_picker_has_no_bypass_row() {
        let rows = mode_rows(false);
        assert_eq!(rows.len(), 4);
        assert!(rows.iter().all(|row| row.mode != PermissionMode::BypassPermissions));
    }

    #[test]
    fn with_the_switch_bypass_is_the_last_row_below_a_divider() {
        let rows = mode_rows(true);
        let last = rows.last().unwrap();
        assert_eq!(last.mode, PermissionMode::BypassPermissions);
        assert!(last.divider_before);
    }

    #[test]
    fn the_four_modes_keep_their_number_keys_with_or_without_bypass() {
        for include in [false, true] {
            let keys: Vec<_> = mode_rows(include).iter().filter_map(|row| row.key).collect();
            assert_eq!(keys, vec![1, 2, 3, 4]);
        }
    }

    #[test]
    fn every_mode_is_in_the_rows_and_the_selector_agrees() {
        for (mode, _, label) in MODES {
            let row = mode_rows(true).into_iter().find(|row| row.mode == mode).expect("a row for the mode");
            assert_eq!(row.label, label, "the picker and the selector name {mode:?} alike");
        }
    }

    #[test]
    fn only_bypass_asks_first() {
        for row in mode_rows(true) {
            let expected = if row.mode == PermissionMode::BypassPermissions { ModeChoice::AskFirst } else { ModeChoice::Apply };
            assert_eq!(mode_choice(row.mode), expected, "{:?}", row.mode);
        }
    }

    #[test]
    fn number_keys_one_to_four_pick_rows_by_the_physical_key() {
        assert_eq!(menu_digit_row("Digit1"), Some(0));
        assert_eq!(menu_digit_row("Digit4"), Some(3));
        assert_eq!(menu_digit_row("Numpad2"), Some(1));
        assert_eq!(menu_digit_row("Digit5"), None);
        assert_eq!(menu_digit_row("Digit0"), None);
        assert_eq!(menu_digit_row("KeyR"), None);
        assert_eq!(menu_digit_row("NumpadAdd"), None);
    }

    #[test]
    fn every_mode_round_trips_through_its_value() {
        for mode in [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::Plan,
            PermissionMode::BypassPermissions,
            PermissionMode::Auto,
        ] {
            assert_eq!(parse_mode(mode_value(mode)), mode);
        }
    }

    #[test]
    fn an_unknown_value_falls_back_to_default() {
        assert_eq!(parse_mode("nonsense"), PermissionMode::Default);
        assert_eq!(parse_mode(""), PermissionMode::Default);
    }

    #[test]
    fn the_selector_offers_all_five_modes_with_distinct_values() {
        let options = mode_options(true);
        assert_eq!(options.len(), 5);
        let mut values: Vec<&String> = options.iter().map(|(value, _)| value).collect();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), 5);
        assert_eq!(options[0].0, "default", "the safest mode is listed first");
    }

    #[test]
    fn the_selector_leaves_out_bypass_when_not_allowed() {
        let options = mode_options(false);
        assert!(options.iter().all(|(value, _)| value != "bypass_permissions"));
        let with: Vec<_> = mode_options(true).into_iter().filter(|(value, _)| value != "bypass_permissions").collect();
        assert_eq!(options, with, "the other options keep their order");
        assert!(mode_options(true).iter().any(|(value, _)| value == "bypass_permissions"));
    }

    #[test]
    fn a_pending_confirmation_is_valid_only_while_its_row_exists() {
        assert!(confirmation_still_valid(&mode_rows(true), PermissionMode::BypassPermissions));
        assert!(!confirmation_still_valid(&mode_rows(false), PermissionMode::BypassPermissions));
        assert!(confirmation_still_valid(&mode_rows(false), PermissionMode::Plan));
    }
}
