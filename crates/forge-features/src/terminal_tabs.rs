#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// The tabs of one terminal panel. The panel owns it for as long as it is open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerminalTabs {
    tabs: Vec<TabId>,
    active: Option<TabId>,
    next: u64,
}

impl TerminalTabs {
    /// Adds a tab at the right and makes it the active one.
    pub fn open(&mut self) -> TabId {
        let id = TabId(self.next);
        self.next += 1;
        self.tabs.push(id);
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: TabId) {
        let Some(position) = self.tabs.iter().position(|tab| *tab == id) else {
            return;
        };
        self.tabs.remove(position);
        if self.active == Some(id) {
            self.active = self.tabs.get(position).or_else(|| self.tabs.last()).copied();
        }
    }

    pub fn select(&mut self, id: TabId) {
        if self.tabs.contains(&id) {
            self.active = Some(id);
        }
    }

    pub fn tabs(&self) -> &[TabId] {
        &self.tabs
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tab_becomes_the_active_one() {
        let mut tabs = TerminalTabs::default();
        let first = tabs.open();
        let second = tabs.open();
        assert_eq!(tabs.tabs(), &[first, second]);
        assert_eq!(tabs.active(), Some(second));
    }

    #[test]
    fn ids_are_not_reused_after_a_close() {
        let mut tabs = TerminalTabs::default();
        let first = tabs.open();
        tabs.close(first);
        let second = tabs.open();
        assert_ne!(first, second);
    }

    #[test]
    fn closing_the_active_tab_activates_its_right_neighbour_else_the_left() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        let b = tabs.open();
        let c = tabs.open();
        tabs.select(b);
        tabs.close(b);
        assert_eq!(tabs.active(), Some(c));
        tabs.close(c);
        assert_eq!(tabs.active(), Some(a));
        tabs.close(a);
        assert_eq!(tabs.active(), None);
        assert!(tabs.is_empty());
    }

    #[test]
    fn closing_a_tab_that_is_not_active_keeps_the_active_one() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        let b = tabs.open();
        tabs.close(a);
        assert_eq!(tabs.active(), Some(b));
    }

    #[test]
    fn selecting_or_closing_a_tab_that_does_not_exist_is_ignored() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        tabs.select(TabId(99));
        tabs.close(TabId(99));
        assert_eq!(tabs.tabs(), &[a]);
        assert_eq!(tabs.active(), Some(a));
    }
}
