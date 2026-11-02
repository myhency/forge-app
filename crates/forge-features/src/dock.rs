use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_shared::tr;

pub const DEFAULT_COLUMN_WIDTH: f32 = 480.0;
pub const MIN_COLUMN_WIDTH: f32 = 280.0;
/// The session rail is a narrow strip of cards (the desktop app's is about 310 pt wide).
pub const RAIL_COLUMN_WIDTH: f32 = 320.0;
/// A column grows until the chat is this wide (the desktop app stops there; there is no fixed largest width).
pub const MIN_CHAT_WHILE_RESIZING: f32 = 100.0;

/// A panel that can sit in the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Terminal,
    Changes,
    Preview,
    Files,
    Tasks,
    Details,
    Plan,
    /// The `/btw` side chat.
    Side,
}

impl Panel {
    pub fn title(self) -> &'static str {
        match self {
            Panel::Terminal => tr!("Terminal"),
            Panel::Changes => tr!("Changes"),
            Panel::Preview => tr!("Preview"),
            Panel::Files => tr!("Files"),
            Panel::Tasks => tr!("Background tasks"),
            Panel::Details => tr!("Environment"),
            Panel::Plan => tr!("Plan"),
            Panel::Side => tr!("Side question"),
        }
    }

    /// The width a column opens at when this panel is the one that opens it.
    pub fn default_width(self) -> f32 {
        match self {
            Panel::Details => RAIL_COLUMN_WIDTH,
            _ => DEFAULT_COLUMN_WIDTH,
        }
    }

    /// Whether the panel can share a column with one other (Files, the Terminal, Changes and Preview can; the rest always stand alone).
    fn can_stack(self) -> bool {
        matches!(self, Panel::Files | Panel::Terminal | Panel::Changes | Panel::Preview)
    }
}

/// One column of the dock: the panels stacked in it, top to bottom, and its width in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub panels: Vec<Panel>,
    pub width: f32,
}

/// The dock of one session: columns right of the chat, left to right, and the panel that fills
/// the whole area right of the sidebar, if any.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockState {
    pub columns: Vec<Column>,
    pub expanded: Option<Panel>,
}

impl DockState {
    pub fn is_open(&self, panel: Panel) -> bool {
        self.columns.iter().any(|column| column.panels.contains(&panel))
    }

    /// Opens a closed panel, or closes an open one. Opening a panel never closes another. The session rail opens in a new
    /// column at the far left. Files, the Terminal, Changes and Preview join the rightmost column below what is there when
    /// that column holds one of them alone (a column holds two; seen in the desktop app: whichever opened last is lower). With that
    /// column full, or any other panel there, they open a new column at the far right, as the other panels always do.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.close(panel);
            return;
        }
        if panel.can_stack() {
            if let Some(column) = self.columns.last_mut().filter(|column| column.panels.len() == 1 && column.panels[0].can_stack()) {
                column.panels.push(panel);
                return;
            }
        }
        let column = Column { panels: vec![panel], width: panel.default_width() };
        // The session rail always stands leftmost, next to the chat; every other panel opens at the far right.
        if panel == Panel::Details {
            self.columns.insert(0, column);
        } else {
            self.columns.push(column);
        }
    }

    /// Opens a closed panel as `toggle` does; an open one stays open.
    pub fn open(&mut self, panel: Panel) {
        if !self.is_open(panel) {
            self.toggle(panel);
        }
    }

    pub fn close(&mut self, panel: Panel) {
        for column in &mut self.columns {
            column.panels.retain(|open| *open != panel);
        }
        self.columns.retain(|column| !column.panels.is_empty());
        if self.expanded == Some(panel) {
            self.expanded = None;
        }
    }

    /// Makes an open panel fill the area right of the sidebar. A closed panel is ignored.
    pub fn expand(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.expanded = Some(panel);
        }
    }

    pub fn collapse(&mut self) {
        self.expanded = None;
    }

    /// Puts an expanded panel back, or expands the first panel of the newest column. Nothing
    /// happens when no panel is open.
    pub fn toggle_expanded(&mut self) {
        if self.expanded.is_some() {
            self.collapse();
        } else if let Some(panel) = self.columns.last().and_then(|column| column.panels.first().copied()) {
            self.expand(panel);
        }
    }

    /// The panel whose file list a key shows or hides: Files when it is open, else Changes.
    pub fn file_list_panel(&self) -> Option<Panel> {
        [Panel::Files, Panel::Changes].into_iter().find(|panel| self.is_open(*panel))
    }

    /// Whether a column is the session rail (a fixed width: it has no handle).
    pub fn is_rail(&self, column: usize) -> bool {
        self.columns.get(column).is_some_and(|column| column.panels.contains(&Panel::Details))
    }

    /// Whether the left edge of a column is the edge next to the chat: no column but the rail stands left of it.
    pub fn is_chat_edge(&self, column: usize) -> bool {
        !self.is_rail(column) && (0..column).all(|before| self.is_rail(before))
    }

    /// Sets the width of the column next to the chat. It is at least `MIN_COLUMN_WIDTH`, and grows until the chat is
    /// `MIN_CHAT_WHILE_RESIZING` wide beside it and the other columns. (The rail does not count: it floats over the chat as a card.) A column that does not exist and a width that is not a number are ignored.
    pub fn resize_chat_edge(&mut self, column: usize, width: f32, pane_width: f32) {
        if !width.is_finite() || !pane_width.is_finite() || column >= self.columns.len() {
            return;
        }
        let others: f32 = self.columns.iter().enumerate().filter(|(index, other)| *index != column && !other.panels.contains(&Panel::Details)).map(|(_, other)| other.width).sum();
        let largest = (pane_width - MIN_CHAT_WHILE_RESIZING - others).max(MIN_COLUMN_WIDTH);
        self.columns[column].width = width.clamp(MIN_COLUMN_WIDTH, largest);
    }

    /// Moves the edge between column `left` and the next one by `delta` (right is positive) from the widths they had when it
    /// was grabbed. The two share what they had: each keeps at least `MIN_COLUMN_WIDTH`, and the chat does not change.
    pub fn move_boundary(&mut self, left: usize, start_left: f32, start_right: f32, delta: f32) {
        if !delta.is_finite() || left + 1 >= self.columns.len() {
            return;
        }
        let total = start_left + start_right;
        let new_left = (start_left + delta).clamp(MIN_COLUMN_WIDTH, (total - MIN_COLUMN_WIDTH).max(MIN_COLUMN_WIDTH));
        self.columns[left].width = new_left;
        self.columns[left + 1].width = total - new_left;
    }
}

/// The dock of each open session. It lives in memory only (spec: panel state is not saved).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockStates {
    states: HashMap<LiveSessionId, DockState>,
}

impl DockStates {
    pub fn get(&self, id: LiveSessionId) -> DockState {
        self.states.get(&id).cloned().unwrap_or_default()
    }

    pub fn change(&mut self, id: LiveSessionId, change: impl FnOnce(&mut DockState)) {
        let state = self.states.entry(id).or_default();
        change(state);
        if *state == DockState::default() {
            self.states.remove(&id);
        }
    }

    /// Drops what is kept for a session that was closed.
    pub fn forget(&mut self, id: LiveSessionId) {
        self.states.remove(&id);
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_expand_key_expands_the_newest_column_and_puts_it_back() {
        let mut dock = DockState::default();
        dock.toggle_expanded();
        assert_eq!(dock.expanded, None, "nothing is open");
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Tasks);
        dock.toggle_expanded();
        assert_eq!(dock.expanded, Some(Panel::Tasks));
        dock.toggle_expanded();
        assert_eq!(dock.expanded, None);
    }

    #[test]
    fn open_changes_panel_does_not_close_an_open_panel() {
        let mut dock = DockState::default();
        dock.open(Panel::Changes);
        assert!(dock.is_open(Panel::Changes));
        dock.open(Panel::Changes);
        assert!(dock.is_open(Panel::Changes));
        assert_eq!(dock.columns.len(), 1);
    }

    #[test]
    fn the_file_list_key_prefers_files_over_changes() {
        let mut dock = DockState::default();
        assert_eq!(dock.file_list_panel(), None);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.file_list_panel(), Some(Panel::Changes));
        dock.toggle(Panel::Files);
        assert_eq!(dock.file_list_panel(), Some(Panel::Files));
    }

    #[test]
    fn the_session_rail_always_opens_left_of_the_other_panels() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Files);
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Details);
        let order: Vec<Vec<Panel>> = dock.columns.iter().map(|column| column.panels.clone()).collect();
        assert_eq!(order, vec![vec![Panel::Details], vec![Panel::Files, Panel::Terminal]]);
        // Closed and opened again after another panel: still leftmost.
        dock.toggle(Panel::Details);
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Details);
        assert_eq!(dock.columns[0].panels, vec![Panel::Details]);
    }

    #[test]
    fn the_session_rail_opens_narrow_and_the_other_panels_at_the_default_width() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Details);
        dock.toggle(Panel::Files);
        assert_eq!(dock.columns[0].width, RAIL_COLUMN_WIDTH);
        assert_eq!(dock.columns[1].width, DEFAULT_COLUMN_WIDTH);
        assert!(RAIL_COLUMN_WIDTH >= MIN_COLUMN_WIDTH && RAIL_COLUMN_WIDTH < DEFAULT_COLUMN_WIDTH);
    }

    #[test]
    fn toggling_a_closed_panel_opens_it_in_a_new_column_at_the_default_width() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        assert!(dock.is_open(Panel::Terminal));
        assert_eq!(dock.columns, vec![Column { panels: vec![Panel::Terminal], width: DEFAULT_COLUMN_WIDTH }]);
    }

    #[test]
    fn toggling_an_open_panel_closes_it_and_its_column() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Terminal);
        assert!(!dock.is_open(Panel::Terminal));
        assert!(dock.columns.is_empty());
    }

    #[test]
    fn expanding_a_closed_panel_does_nothing() {
        let mut dock = DockState::default();
        dock.expand(Panel::Terminal);
        assert_eq!(dock.expanded, None);
    }

    #[test]
    fn an_open_panel_expands_and_collapses() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.expand(Panel::Terminal);
        assert_eq!(dock.expanded, Some(Panel::Terminal));
        dock.collapse();
        assert_eq!(dock.expanded, None);
    }

    #[test]
    fn closing_the_expanded_panel_collapses_first() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.expand(Panel::Terminal);
        dock.close(Panel::Terminal);
        assert_eq!(dock, DockState::default());
    }

    #[test]
    fn the_chat_edge_column_grows_until_the_chat_is_100_wide_and_shrinks_to_280() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize_chat_edge(0, 10.0, 1000.0);
        assert_eq!(dock.columns[0].width, MIN_COLUMN_WIDTH);
        // The desktop app at 1700 pt: 1320 of the 1420 pt of the pane, the chat keeps 100.
        dock.resize_chat_edge(0, 5000.0, 1420.0);
        assert_eq!(dock.columns[0].width, 1320.0);
        dock.resize_chat_edge(0, 500.0, 1420.0);
        assert_eq!(dock.columns[0].width, 500.0);
    }

    #[test]
    fn the_rail_does_not_count_for_room_and_has_no_handle() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Details);
        dock.toggle(Panel::Files);
        assert!(dock.is_rail(0) && !dock.is_rail(1));
        assert!(!dock.is_chat_edge(0));
        assert!(dock.is_chat_edge(1), "the first column right of the rail is next to the chat");
        dock.resize_chat_edge(1, 5000.0, 1200.0);
        assert_eq!(dock.columns[1].width, 1200.0 - MIN_CHAT_WHILE_RESIZING);
        assert_eq!(dock.columns[0].width, RAIL_COLUMN_WIDTH);
    }

    #[test]
    fn a_boundary_between_two_columns_shares_their_width_and_keeps_280_each() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Files);
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Plan);
        // Files + Terminal share a column, so Plan is the second.
        assert_eq!(dock.columns.len(), 2);
        let (a, b) = (dock.columns[0].width, dock.columns[1].width);
        assert!(!dock.is_chat_edge(1));
        dock.move_boundary(0, a, b, -5000.0);
        assert_eq!((dock.columns[0].width, dock.columns[1].width), (MIN_COLUMN_WIDTH, a + b - MIN_COLUMN_WIDTH));
        dock.move_boundary(0, a, b, 5000.0);
        assert_eq!((dock.columns[0].width, dock.columns[1].width), (a + b - MIN_COLUMN_WIDTH, MIN_COLUMN_WIDTH));
        dock.move_boundary(0, a, b, 40.0);
        assert_eq!(dock.columns[0].width + dock.columns[1].width, a + b);
    }

    #[test]
    fn resizing_ignores_a_missing_column_and_a_width_that_is_not_a_number() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize_chat_edge(3, 500.0, 1200.0);
        dock.resize_chat_edge(0, f32::NAN, 1200.0);
        dock.move_boundary(0, 400.0, 400.0, f32::NAN);
        assert_eq!(dock.columns[0].width, DEFAULT_COLUMN_WIDTH);
    }

    #[test]
    fn each_session_has_its_own_dock_and_forgets_it_on_close() {
        let mut docks = DockStates::default();
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        assert!(docks.get(LiveSessionId(1)).is_open(Panel::Terminal));
        assert!(!docks.get(LiveSessionId(2)).is_open(Panel::Terminal));
        docks.forget(LiveSessionId(1));
        assert_eq!(docks.get(LiveSessionId(1)), DockState::default());
    }

    #[test]
    fn a_dock_that_is_back_to_empty_is_not_kept() {
        let mut docks = DockStates::default();
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        assert!(docks.is_empty());
    }

    #[test]
    fn a_second_panel_joins_the_rightmost_column_below_the_first_whichever_it_is() {
        // Seen in the desktop app, each pair opened in both orders where checked: the panel opened last is the lower one.
        for (first, second) in [(Panel::Changes, Panel::Terminal), (Panel::Terminal, Panel::Changes), (Panel::Preview, Panel::Changes), (Panel::Changes, Panel::Preview), (Panel::Files, Panel::Terminal)] {
            let mut dock = DockState::default();
            dock.toggle(first);
            dock.toggle(second);
            assert_eq!(dock.columns.len(), 1);
            assert_eq!(dock.columns[0].panels, vec![first, second]);
        }
    }

    #[test]
    fn a_third_panel_opens_a_new_column_and_the_fourth_joins_it() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Files);
        assert_eq!(dock.columns.len(), 2);
        assert_eq!(dock.columns[1].panels, vec![Panel::Files]);
        dock.toggle(Panel::Preview);
        assert_eq!(dock.columns.len(), 2);
        assert_eq!(dock.columns[1].panels, vec![Panel::Files, Panel::Preview]);
    }

    #[test]
    fn a_panel_with_a_column_of_its_own_is_never_joined() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Plan);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.columns.len(), 3, "Plan stands last, so Changes does not join the Terminal behind it");
        let mut dock = DockState::default();
        dock.toggle(Panel::Tasks);
        dock.toggle(Panel::Files);
        assert_eq!(dock.columns.len(), 2);
    }

    #[test]
    fn closing_one_of_a_shared_pair_keeps_the_column_for_the_other() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Preview);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.columns[0].panels, vec![Panel::Preview]);
        dock.toggle(Panel::Preview);
        assert!(dock.columns.is_empty());
    }

    #[test]
    fn the_new_panels_have_titles() {
        assert_eq!(Panel::Changes.title(), "Changes");
        assert_eq!(Panel::Preview.title(), "Preview");
        assert_eq!(Panel::Files.title(), "Files");
    }

    #[test]
    fn the_plan_panel_opens_in_its_own_column_and_closes_again() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Plan);
        assert_eq!(dock.columns.len(), 2);
        assert_eq!(dock.columns[1].panels, vec![Panel::Plan]);
        assert_eq!(Panel::Plan.title(), "Plan");
        assert_eq!(Panel::Plan.can_stack(), false);
        dock.toggle(Panel::Plan);
        assert!(!dock.is_open(Panel::Plan));
        assert_eq!(dock.columns.len(), 1);
    }

    #[test]
    fn tasks_open_in_their_own_column_and_close_again() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Tasks);
        assert_eq!(dock.columns.len(), 2);
        assert_eq!(dock.columns[1].panels, vec![Panel::Tasks]);
        assert!(dock.is_open(Panel::Tasks));
        assert_eq!(Panel::Tasks.title(), "Background tasks");
        dock.toggle(Panel::Tasks);
        assert!(!dock.is_open(Panel::Tasks));
        assert_eq!(dock.columns.len(), 1);
    }

    #[test]
    fn details_opens_in_its_own_column_and_closes() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Details);
        assert_eq!(dock.columns.len(), 1);
        assert_eq!(dock.columns[0].panels, vec![Panel::Details]);
        dock.toggle(Panel::Details);
        assert!(dock.columns.is_empty());
    }

    #[test]
    fn details_has_the_environment_title() {
        assert_eq!(Panel::Details.title(), "Environment");
    }
}
