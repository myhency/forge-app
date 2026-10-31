use forge_domain::{GroupBy, SidebarPrefs, SortBy, StatusFilter};
use forge_shared::tr;

pub const STATUS_ITEMS: [StatusFilter; 3] = [StatusFilter::Active, StatusFilter::Archived, StatusFilter::All];
pub const SORT_ITEMS: [SortBy; 3] = [SortBy::Name, SortBy::Created, SortBy::Activity];
pub const GROUP_ITEMS: [GroupBy; 3] = [GroupBy::Folder, GroupBy::Date, GroupBy::Flat];

/// The sort submenu: the three sort keys, then the direction (descending, then ascending).
pub const SORT_SUBMENU_LEN: usize = SORT_ITEMS.len() + 2;

pub fn status_label(status: StatusFilter) -> &'static str {
    match status {
        StatusFilter::Active => tr!("Active"),
        StatusFilter::Archived => tr!("Archived"),
        StatusFilter::All => tr!("All"),
    }
}

pub fn sort_label(sort_by: SortBy) -> &'static str {
    match sort_by {
        SortBy::Name => tr!("Name"),
        SortBy::Created => tr!("Created"),
        SortBy::Activity => tr!("Activity"),
    }
}

/// The wording of a direction: `reversed` off is the sidebar's usual order (newest first), shown as descending.
pub fn direction_label(reversed: bool) -> &'static str {
    if reversed {
        tr!("Ascending")
    } else {
        tr!("Descending")
    }
}

pub fn group_label(group_by: GroupBy) -> &'static str {
    match group_by {
        GroupBy::Folder => tr!("Folder"),
        GroupBy::Date => tr!("Date"),
        GroupBy::Flat => tr!("None"),
    }
}

/// The top-level rows of the filter menu, in drawing order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterRow {
    Status,
    Sort,
    Group,
    ShowEmpty,
    Reset,
}

pub const FILTER_ROWS: [FilterRow; 5] = [FilterRow::Status, FilterRow::Sort, FilterRow::Group, FilterRow::ShowEmpty, FilterRow::Reset];

impl FilterRow {
    /// How many items the row's submenu has; a row with none is a toggle or a button.
    pub fn submenu_len(self) -> usize {
        match self {
            FilterRow::Status => STATUS_ITEMS.len(),
            FilterRow::Sort => SORT_SUBMENU_LEN,
            FilterRow::Group => GROUP_ITEMS.len(),
            FilterRow::ShowEmpty | FilterRow::Reset => 0,
        }
    }
}

/// What picking an item changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterChoice {
    Status(StatusFilter),
    Sort(SortBy),
    Group(GroupBy),
    ToggleShowEmpty,
    /// Newest first or A to Z (`false`), or the other way round (`true`).
    Reverse(bool),
    /// Sort, group, status and empty groups go back to their defaults; the width, the collapsed groups and
    /// whether the sidebar is hidden are left alone.
    Reset,
}

impl FilterChoice {
    pub fn apply(self, prefs: &mut SidebarPrefs) {
        match self {
            FilterChoice::Status(status) => prefs.status = status,
            FilterChoice::Sort(sort_by) => prefs.sort_by = sort_by,
            FilterChoice::Group(group_by) => prefs.group_by = group_by,
            FilterChoice::ToggleShowEmpty => prefs.show_empty = !prefs.show_empty,
            FilterChoice::Reverse(reversed) => prefs.reversed = reversed,
            FilterChoice::Reset => {
                let defaults = SidebarPrefs::default();
                prefs.sort_by = defaults.sort_by;
                prefs.group_by = defaults.group_by;
                prefs.status = defaults.status;
                prefs.show_empty = defaults.show_empty;
                prefs.reversed = defaults.reversed;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
}

/// The key a physical key code stands for (`event.code()`), if the menu uses it.
pub fn filter_key(code: &str) -> Option<FilterKey> {
    match code {
        "ArrowUp" => Some(FilterKey::Up),
        "ArrowDown" => Some(FilterKey::Down),
        "ArrowLeft" => Some(FilterKey::Left),
        "ArrowRight" => Some(FilterKey::Right),
        "Enter" | "NumpadEnter" => Some(FilterKey::Enter),
        "Escape" => Some(FilterKey::Escape),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOutcome {
    Stay,
    /// Apply it, then close the menu.
    Choose(FilterChoice),
    Close,
}

/// Which row is highlighted, and which item of an open submenu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterMenu {
    pub top: usize,
    pub sub: Option<usize>,
}

/// The position of the current value among the items of a row's submenu.
pub fn current_index(row: FilterRow, prefs: &SidebarPrefs) -> usize {
    match row {
        FilterRow::Status => STATUS_ITEMS.iter().position(|item| *item == prefs.status).unwrap_or(0),
        FilterRow::Sort => SORT_ITEMS.iter().position(|item| *item == prefs.sort_by).unwrap_or(0),
        FilterRow::Group => GROUP_ITEMS.iter().position(|item| *item == prefs.group_by).unwrap_or(0),
        FilterRow::ShowEmpty | FilterRow::Reset => 0,
    }
}

impl FilterMenu {
    pub fn key(&mut self, key: FilterKey, prefs: &SidebarPrefs) -> FilterOutcome {
        let row = FILTER_ROWS[self.top];
        match (self.sub, key) {
            (Some(index), FilterKey::Up) => self.sub = Some(index.saturating_sub(1)),
            (Some(index), FilterKey::Down) => self.sub = Some((index + 1).min(row.submenu_len().saturating_sub(1))),
            (Some(_), FilterKey::Left | FilterKey::Escape) => self.sub = None,
            (Some(_), FilterKey::Right) => {}
            (Some(index), FilterKey::Enter) => {
                let choice = match row {
                    FilterRow::Status => FilterChoice::Status(STATUS_ITEMS[index]),
                    FilterRow::Sort if index < SORT_ITEMS.len() => FilterChoice::Sort(SORT_ITEMS[index]),
                    // After the three keys: descending is the first direction row, ascending the second.
                    FilterRow::Sort => FilterChoice::Reverse(index > SORT_ITEMS.len()),
                    FilterRow::Group => FilterChoice::Group(GROUP_ITEMS[index]),
                    FilterRow::ShowEmpty => FilterChoice::ToggleShowEmpty,
                    FilterRow::Reset => FilterChoice::Reset,
                };
                return FilterOutcome::Choose(choice);
            }
            (None, FilterKey::Up) => self.top = self.top.saturating_sub(1),
            (None, FilterKey::Down) => self.top = (self.top + 1).min(FILTER_ROWS.len() - 1),
            (None, FilterKey::Left) => {}
            (None, FilterKey::Right) => {
                if row.submenu_len() > 0 {
                    self.sub = Some(current_index(row, prefs));
                }
            }
            (None, FilterKey::Enter) => match row {
                FilterRow::ShowEmpty => return FilterOutcome::Choose(FilterChoice::ToggleShowEmpty),
                FilterRow::Reset => return FilterOutcome::Choose(FilterChoice::Reset),
                _ => self.sub = Some(current_index(row, prefs)),
            },
            (None, FilterKey::Escape) => return FilterOutcome::Close,
        }
        FilterOutcome::Stay
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::{GroupBy, SidebarPrefs, SortBy, StatusFilter};

    use super::*;

    fn press(menu: &mut FilterMenu, keys: &[FilterKey], prefs: &SidebarPrefs) -> FilterOutcome {
        let mut last = FilterOutcome::Stay;
        for key in keys {
            last = menu.key(*key, prefs);
        }
        last
    }

    #[test]
    fn up_and_down_move_over_the_five_rows_and_stop_at_the_ends() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Up], &prefs);
        assert_eq!(menu.top, 0);
        press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(menu.top, 4);
    }

    #[test]
    fn right_opens_the_submenu_on_the_current_value() {
        let prefs = SidebarPrefs { sort_by: SortBy::Created, ..SidebarPrefs::default() };
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Down, FilterKey::Right], &prefs);

        assert_eq!(menu.sub, Some(1));
    }

    #[test]
    fn enter_in_a_submenu_chooses_and_the_choice_changes_the_preferences() {
        let mut prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        let outcome = press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Right, FilterKey::Down, FilterKey::Enter], &prefs);

        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Group(GroupBy::Date)));
        if let FilterOutcome::Choose(choice) = outcome {
            choice.apply(&mut prefs);
        }
        assert_eq!(prefs.group_by, GroupBy::Date);
    }

    #[test]
    fn left_and_escape_close_the_submenu_first_and_then_escape_closes_the_menu() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Right], &prefs);
        assert!(menu.sub.is_some());
        assert_eq!(press(&mut menu, &[FilterKey::Left], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);

        press(&mut menu, &[FilterKey::Right], &prefs);
        assert_eq!(press(&mut menu, &[FilterKey::Escape], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);
        assert_eq!(press(&mut menu, &[FilterKey::Escape], &prefs), FilterOutcome::Close);
    }

    #[test]
    fn enter_on_show_empty_toggles_it_and_right_does_nothing_there() {
        let mut prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(press(&mut menu, &[FilterKey::Right], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);
        let outcome = press(&mut menu, &[FilterKey::Enter], &prefs);

        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::ToggleShowEmpty));
        if let FilterOutcome::Choose(choice) = outcome {
            choice.apply(&mut prefs);
        }
        assert!(prefs.show_empty);
    }

    #[test]
    fn the_sort_submenu_has_the_three_keys_then_the_two_directions() {
        assert_eq!(FilterRow::Sort.submenu_len(), 5);
        let mut prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();
        // Down to Sort, open it on the current key, then to the last row (ascending).
        press(&mut menu, &[FilterKey::Down, FilterKey::Right, FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(menu.sub, Some(4));

        let outcome = press(&mut menu, &[FilterKey::Enter], &prefs);
        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Reverse(true)));
        if let FilterOutcome::Choose(choice) = outcome {
            choice.apply(&mut prefs);
        }
        assert!(prefs.reversed);

        // The row before it is descending, which turns the reversal off again.
        let outcome = press(&mut menu, &[FilterKey::Up, FilterKey::Enter], &prefs);
        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Reverse(false)));
    }

    #[test]
    fn choosing_a_sort_key_keeps_the_direction() {
        let mut prefs = SidebarPrefs { reversed: true, ..SidebarPrefs::default() };

        FilterChoice::Sort(SortBy::Name).apply(&mut prefs);

        assert_eq!(prefs.sort_by, SortBy::Name);
        assert!(prefs.reversed);
        assert_eq!(direction_label(false), "Descending");
        assert_eq!(direction_label(true), "Ascending");
    }

    #[test]
    fn reset_restores_the_defaults_but_not_the_window_state() {
        let mut prefs = SidebarPrefs {
            sort_by: SortBy::Name,
            group_by: GroupBy::Date,
            status: StatusFilter::Archived,
            show_empty: true,
            reversed: true,
            collapsed: vec!["folder:/a".to_string()],
            width: 300,
            hidden: true,
        };

        FilterChoice::Reset.apply(&mut prefs);

        let defaults = SidebarPrefs::default();
        assert_eq!((prefs.sort_by, prefs.group_by, prefs.status, prefs.show_empty, prefs.reversed), (defaults.sort_by, defaults.group_by, defaults.status, defaults.show_empty, defaults.reversed));
        assert_eq!((prefs.collapsed.clone(), prefs.width, prefs.hidden), (vec!["folder:/a".to_string()], 300, true));
    }

    #[test]
    fn enter_on_reset_resets_and_right_does_nothing_there() {
        let prefs = SidebarPrefs::default();
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Down, FilterKey::Down, FilterKey::Down, FilterKey::Down], &prefs);
        assert_eq!(press(&mut menu, &[FilterKey::Right], &prefs), FilterOutcome::Stay);
        assert_eq!(menu.sub, None);
        assert_eq!(press(&mut menu, &[FilterKey::Enter], &prefs), FilterOutcome::Choose(FilterChoice::Reset));
    }

    #[test]
    fn keys_are_read_from_the_physical_key() {
        assert_eq!(filter_key("ArrowDown"), Some(FilterKey::Down));
        assert_eq!(filter_key("ArrowUp"), Some(FilterKey::Up));
        assert_eq!(filter_key("ArrowLeft"), Some(FilterKey::Left));
        assert_eq!(filter_key("ArrowRight"), Some(FilterKey::Right));
        assert_eq!(filter_key("Enter"), Some(FilterKey::Enter));
        assert_eq!(filter_key("NumpadEnter"), Some(FilterKey::Enter));
        assert_eq!(filter_key("Escape"), Some(FilterKey::Escape));
        assert_eq!(filter_key("KeyR"), None);
    }

    #[test]
    fn the_labels_are_the_ones_the_menu_shows() {
        assert_eq!(sort_label(SortBy::Activity), "Activity");
        assert_eq!(group_label(GroupBy::Flat), "None");
    }

    #[test]
    fn the_first_row_is_status_and_it_opens_on_the_current_value() {
        let prefs = SidebarPrefs { status: StatusFilter::Archived, ..SidebarPrefs::default() };
        let mut menu = FilterMenu::default();

        press(&mut menu, &[FilterKey::Right], &prefs);

        assert_eq!(menu.sub, Some(1));
        let outcome = press(&mut menu, &[FilterKey::Down, FilterKey::Enter], &prefs);
        assert_eq!(outcome, FilterOutcome::Choose(FilterChoice::Status(StatusFilter::All)));
    }

    #[test]
    fn choosing_a_status_changes_the_preferences() {
        let mut prefs = SidebarPrefs::default();

        FilterChoice::Status(StatusFilter::Archived).apply(&mut prefs);

        assert_eq!(prefs.status, StatusFilter::Archived);
        assert_eq!(status_label(StatusFilter::Active), "Active");
        assert_eq!(status_label(StatusFilter::Archived), "Archived");
        assert_eq!(status_label(StatusFilter::All), "All");
    }
}
