use std::collections::HashMap;

use forge_application::LiveSessionId;

pub const DEFAULT_COLUMN_WIDTH: f32 = 480.0;
pub const MIN_COLUMN_WIDTH: f32 = 280.0;
pub const MAX_COLUMN_WIDTH: f32 = 900.0;

/// A panel that can sit in the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Terminal,
    Changes,
    Preview,
    Files,
    Tasks,
}

impl Panel {
    pub fn title(self) -> &'static str {
        match self {
            Panel::Terminal => "Terminal",
            Panel::Changes => "Changes",
            Panel::Preview => "Preview",
            Panel::Files => "Files",
            Panel::Tasks => "Background tasks",
        }
    }

    // Changes and Preview share one column.
    fn partner(self) -> Option<Panel> {
        match self {
            Panel::Changes => Some(Panel::Preview),
            Panel::Preview => Some(Panel::Changes),
            Panel::Terminal | Panel::Files | Panel::Tasks => None,
        }
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

    /// Opens a closed panel in a new column at the far right, or in its partner's column when the
    /// partner is open (Changes and Preview), or closes an open one. Opening a panel never closes
    /// another.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.close(panel);
            return;
        }
        if let Some(partner) = panel.partner() {
            if let Some(column) = self.columns.iter_mut().find(|column| column.panels.contains(&partner)) {
                column.panels.push(panel);
                return;
            }
        }
        self.columns.push(Column { panels: vec![panel], width: DEFAULT_COLUMN_WIDTH });
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

    /// Sets a column's width, kept between the minimum and the maximum. A column that does not
    /// exist and a width that is not a number are ignored.
    pub fn resize(&mut self, column: usize, width: f32) {
        if !width.is_finite() {
            return;
        }
        if let Some(column) = self.columns.get_mut(column) {
            column.width = width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }
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
    fn resize_clamps_to_the_minimum_and_maximum() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize(0, 10.0);
        assert_eq!(dock.columns[0].width, MIN_COLUMN_WIDTH);
        dock.resize(0, 5000.0);
        assert_eq!(dock.columns[0].width, MAX_COLUMN_WIDTH);
        dock.resize(0, 500.0);
        assert_eq!(dock.columns[0].width, 500.0);
    }

    #[test]
    fn resize_ignores_a_missing_column_and_a_width_that_is_not_a_number() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize(3, 500.0);
        dock.resize(0, f32::NAN);
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
    fn preview_joins_the_changes_column_and_changes_joins_the_preview_column() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Preview);
        assert_eq!(dock.columns.len(), 1);
        assert_eq!(dock.columns[0].panels, vec![Panel::Changes, Panel::Preview]);

        let mut dock = DockState::default();
        dock.toggle(Panel::Preview);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.columns.len(), 1);
        assert_eq!(dock.columns[0].panels, vec![Panel::Preview, Panel::Changes]);
    }

    #[test]
    fn files_and_the_terminal_get_their_own_columns() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Files);
        dock.toggle(Panel::Terminal);
        assert_eq!(dock.columns.len(), 3);
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
}
