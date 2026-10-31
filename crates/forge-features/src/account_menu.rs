//! The account menu at the bottom of the sidebar, as data the widget draws.

use forge_domain::{Language, KEY_SETTINGS, KEY_SHORTCUTS};
use forge_shared::msg;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountAction {
    OpenSettings,
    SetLanguage(Language),
    ShowShortcuts,
    /// A row that is shown but does nothing yet: it only closes the menu.
    Close,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountRow {
    Heading(String),
    Divider,
    /// `label` and `keys` are text for `i18n::text`; `check` is `Some` when the item has a check column.
    Item { label: &'static str, keys: Option<&'static str>, action: AccountAction, check: Option<bool> },
    Submenu { label: &'static str, rows: Vec<AccountRow> },
}

fn item(label: &'static str, action: AccountAction) -> AccountRow {
    AccountRow::Item { label, keys: None, action, check: None }
}

fn language_item(language: Language, current: Language) -> AccountRow {
    // Each language is named in its own language; the label is never translated.
    AccountRow::Item { label: language.label(), keys: None, action: AccountAction::SetLanguage(language), check: Some(language == current) }
}

/// The menu for `user`, with the check on `current`.
pub fn account_rows(user: &str, current: Language) -> Vec<AccountRow> {
    vec![
        AccountRow::Heading(user.to_string()),
        AccountRow::Divider,
        AccountRow::Item { label: msg!("Settings"), keys: Some(KEY_SETTINGS), action: AccountAction::OpenSettings, check: None },
        item(msg!("Usage"), AccountAction::Close),
        AccountRow::Submenu {
            label: msg!("Language"),
            rows: vec![language_item(Language::Korean, current), language_item(Language::English, current)],
        },
        item(msg!("Get help"), AccountAction::Close),
        AccountRow::Divider,
        AccountRow::Submenu {
            label: msg!("Learn more"),
            rows: vec![
                item(msg!("Usage policy"), AccountAction::Close),
                item(msg!("Privacy policy"), AccountAction::Close),
                AccountRow::Divider,
                AccountRow::Item { label: msg!("Keyboard shortcuts"), keys: Some(KEY_SHORTCUTS), action: AccountAction::ShowShortcuts, check: None },
            ],
        },
        AccountRow::Divider,
        item(msg!("Get API key"), AccountAction::Close),
    ]
}

/// The name shown in the account area: the OS user (`USER`, on Windows `USERNAME`), else "Forge".
pub fn user_name(user: Option<&str>, username: Option<&str>) -> String {
    [user, username]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|name| !name.is_empty())
        .unwrap_or("Forge")
        .to_string()
}

/// The letter in the avatar circle.
pub fn user_initial(name: &str) -> String {
    name.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "F".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // One line per row: heading text, `—` for a divider, `label [keys]`, `label >` then its rows indented.
    fn lines(rows: &[AccountRow], indent: &str, out: &mut Vec<String>) {
        for row in rows {
            match row {
                AccountRow::Heading(text) => out.push(format!("{indent}# {text}")),
                AccountRow::Divider => out.push(format!("{indent}—")),
                AccountRow::Item { label, keys, check, .. } => {
                    let mark = match check { Some(true) => " ✓", _ => "" };
                    let keys = keys.map(|k| format!(" [{k}]")).unwrap_or_default();
                    out.push(format!("{indent}{label}{keys}{mark}"));
                }
                AccountRow::Submenu { label, rows } => {
                    out.push(format!("{indent}{label} >"));
                    lines(rows, &format!("{indent}  "), out);
                }
            }
        }
    }

    fn shown(current: Language) -> Vec<String> {
        let mut out = Vec::new();
        lines(&account_rows("james", current), "", &mut out);
        out
    }

    #[test]
    fn the_menu_has_the_rows_in_order() {
        assert_eq!(
            shown(Language::English),
            [
                "# james".to_string(),
                "—".into(),
                format!("Settings [{}]", forge_domain::KEY_SETTINGS),
                "Usage".into(),
                "Language >".into(),
                "  한국어".into(),
                "  English ✓".into(),
                "Get help".into(),
                "—".into(),
                "Learn more >".into(),
                "  Usage policy".into(),
                "  Privacy policy".into(),
                "  —".into(),
                format!("  Keyboard shortcuts [{}]", forge_domain::KEY_SHORTCUTS),
                "—".into(),
                "Get API key".into(),
            ]
        );
    }

    #[test]
    fn the_check_follows_the_current_language() {
        assert!(shown(Language::Korean).contains(&"  한국어 ✓".to_string()));
        assert!(!shown(Language::Korean).contains(&"  English ✓".to_string()));
    }

    fn find(rows: &[AccountRow], wanted: &str) -> AccountAction {
        for row in rows {
            match row {
                AccountRow::Item { label, action, .. } if *label == wanted => return *action,
                AccountRow::Submenu { rows, .. } => {
                    if let found @ (AccountAction::OpenSettings | AccountAction::SetLanguage(_) | AccountAction::ShowShortcuts) = find(rows, wanted) {
                        return found;
                    }
                }
                _ => {}
            }
        }
        AccountAction::Close
    }

    #[test]
    fn the_rows_do_what_the_spec_says() {
        let rows = account_rows("james", Language::English);
        assert_eq!(find(&rows, "Settings"), AccountAction::OpenSettings);
        assert_eq!(find(&rows, "Keyboard shortcuts"), AccountAction::ShowShortcuts);
        assert_eq!(find(&rows, "한국어"), AccountAction::SetLanguage(Language::Korean));
        assert_eq!(find(&rows, "English"), AccountAction::SetLanguage(Language::English));
        for shown_only in ["Usage", "Get help", "Get API key", "Usage policy", "Privacy policy"] {
            assert_eq!(find(&rows, shown_only), AccountAction::Close, "{shown_only}");
        }
    }

    #[test]
    fn the_user_name_comes_from_the_environment_with_a_fallback() {
        assert_eq!(user_name(Some("james"), Some("other")), "james");
        assert_eq!(user_name(None, Some("james")), "james");
        assert_eq!(user_name(Some(""), Some("  ")), "Forge");
        assert_eq!(user_name(None, None), "Forge");
        assert_eq!(user_name(Some("  제임스 "), None), "제임스");
    }

    #[test]
    fn the_avatar_letter_is_the_first_character_in_capitals() {
        assert_eq!(user_initial("james"), "J");
        assert_eq!(user_initial("제임스"), "제");
        assert_eq!(user_initial("ñandú"), "Ñ");
        assert_eq!(user_initial(""), "F");
    }
}
