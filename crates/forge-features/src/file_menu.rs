/// What a menu item does. The Files panel performs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    Attach,
    CopyAbsolute,
    CopyRelative,
    CopyName,
    OpenVsCode,
    OpenDefault,
    Reveal,
    ShowInTree,
    FindInFile,
    CopyContents,
    Reload,
    Close,
    KeepOpen,
    CloseOthers,
    CloseAll,
}

/// Where the menu was asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuTarget {
    Tree,
    Tab { preview: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuEntry {
    Action { label: &'static str, shortcut: Option<&'static str>, action: FileAction, enabled: bool },
    Submenu { label: &'static str, entries: Vec<MenuEntry> },
    Divider,
}

fn item(label: &'static str, action: FileAction) -> MenuEntry {
    MenuEntry::Action { label, shortcut: None, action, enabled: true }
}

fn copy_menu() -> MenuEntry {
    MenuEntry::Submenu {
        label: "Copy",
        entries: vec![
            item("Absolute path", FileAction::CopyAbsolute),
            item("Relative path", FileAction::CopyRelative),
            item("File name", FileAction::CopyName),
        ],
    }
}

pub fn file_menu(target: MenuTarget) -> Vec<MenuEntry> {
    match target {
        MenuTarget::Tree => vec![
            item("Attach as context", FileAction::Attach),
            copy_menu(),
            MenuEntry::Divider,
            MenuEntry::Submenu {
                label: "Open in",
                entries: vec![item("VS Code", FileAction::OpenVsCode), MenuEntry::Divider, item("Default app", FileAction::OpenDefault)],
            },
            item(forge_domain::SHOW_IN_FILE_MANAGER, FileAction::Reveal),
            MenuEntry::Divider,
            item("Show in file tree", FileAction::ShowInTree),
            MenuEntry::Divider,
            MenuEntry::Action { label: "Find in file", shortcut: Some(forge_domain::KEY_FIND), action: FileAction::FindInFile, enabled: true },
            item("Copy file contents", FileAction::CopyContents),
            item("Reload file", FileAction::Reload),
            MenuEntry::Divider,
            item("Close file", FileAction::Close),
        ],
        MenuTarget::Tab { preview } => vec![
            item("Attach as context", FileAction::Attach),
            copy_menu(),
            MenuEntry::Divider,
            item("Show in file tree", FileAction::ShowInTree),
            MenuEntry::Divider,
            MenuEntry::Action { label: "Keep open", shortcut: None, action: FileAction::KeepOpen, enabled: preview },
            item("Close file", FileAction::Close),
            item("Close others", FileAction::CloseOthers),
            item("Close all", FileAction::CloseAll),
        ],
    }
}

pub const MENU_ITEM_HEIGHT: f64 = 34.0;
pub const MENU_DIVIDER_HEIGHT: f64 = 9.0;
pub const MENU_PADDING: f64 = 10.0;

/// About how tall a menu is, in CSS px, so it can be kept inside the window (a submenu is not counted:
/// it opens to the side).
pub fn menu_height(entries: &[MenuEntry]) -> f64 {
    MENU_PADDING
        + entries
            .iter()
            .map(|entry| match entry {
                MenuEntry::Divider => MENU_DIVIDER_HEIGHT,
                _ => MENU_ITEM_HEIGHT,
            })
            .sum::<f64>()
}

/// Whether a submenu opens to the left of its menu: when the window has no room for the menu and a
/// submenu of the same width to the right of the menu's left edge.
pub fn submenu_opens_left(menu_x: f64, window_width: f64, menu_width: f64) -> bool {
    menu_x + 2.0 * menu_width > window_width
}

#[cfg(test)]
mod tests {
    use super::*;

    // One line per entry: the label, `—` for a divider, `Label >` followed by its items for a submenu.
    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        let mut out = Vec::new();
        for entry in entries {
            match entry {
                MenuEntry::Divider => out.push("—".to_string()),
                MenuEntry::Action { label, shortcut, .. } => {
                    // The expected lists below are written for macOS.
                    let label = label.replace(forge_domain::SHOW_IN_FILE_MANAGER, "Show in Finder");
                    out.push(match shortcut {
                        Some(key) => format!("{label} [{}]", key.replace(forge_domain::KEY_FIND, "⌘F")),
                        None => label,
                    })
                }
                MenuEntry::Submenu { label, entries } => {
                    out.push(format!("{label} >"));
                    out.extend(labels(entries).into_iter().map(|l| format!("  {l}")));
                }
            }
        }
        out
    }

    #[test]
    fn the_tree_row_menu_has_the_observed_items_in_order() {
        assert_eq!(
            labels(&file_menu(MenuTarget::Tree)),
            vec![
                "Attach as context", "Copy >", "  Absolute path", "  Relative path", "  File name", "—",
                "Open in >", "  VS Code", "  —", "  Default app", "Show in Finder", "—",
                "Show in file tree", "—",
                "Find in file [⌘F]", "Copy file contents", "Reload file", "—",
                "Close file",
            ]
        );
    }

    #[test]
    fn the_tab_menu_has_the_observed_items_in_order() {
        assert_eq!(
            labels(&file_menu(MenuTarget::Tab { preview: true })),
            vec![
                "Attach as context", "Copy >", "  Absolute path", "  Relative path", "  File name", "—",
                "Show in file tree", "—",
                "Keep open", "Close file", "Close others", "Close all",
            ]
        );
    }

    fn enabled(entries: &[MenuEntry], wanted: FileAction) -> Option<bool> {
        entries.iter().find_map(|entry| match entry {
            MenuEntry::Action { action, enabled, .. } if *action == wanted => Some(*enabled),
            _ => None,
        })
    }

    #[test]
    fn keep_open_is_enabled_only_for_a_preview_tab() {
        assert_eq!(enabled(&file_menu(MenuTarget::Tab { preview: true }), FileAction::KeepOpen), Some(true));
        assert_eq!(enabled(&file_menu(MenuTarget::Tab { preview: false }), FileAction::KeepOpen), Some(false));
    }

    #[test]
    fn the_submenu_flips_left_when_there_is_no_room_on_the_right() {
        assert!(!submenu_opens_left(100.0, 1200.0, 230.0));
        assert!(submenu_opens_left(900.0, 1200.0, 230.0));
        // Just enough room for the menu and the submenu: stays on the right.
        assert!(!submenu_opens_left(740.0, 1200.0, 230.0));
        assert!(submenu_opens_left(741.0, 1200.0, 230.0));
    }

    #[test]
    fn the_height_follows_the_entries_so_a_menu_can_be_kept_inside_the_window() {
        let tree = menu_height(&file_menu(MenuTarget::Tree));
        let tab = menu_height(&file_menu(MenuTarget::Tab { preview: true }));
        assert!(tab < tree, "{tab} {tree}");
        // 9 entries and 4 dividers in the tree menu.
        assert_eq!(tree, 9.0 * MENU_ITEM_HEIGHT + 4.0 * MENU_DIVIDER_HEIGHT + MENU_PADDING);
        assert_eq!(menu_height(&[]), MENU_PADDING);
    }
}
