/// One tab of the Files panel. Tabs are keyed by the file's path inside the session folder, so two
/// files with the same name in different folders are two tabs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorTab {
    pub path: String,
    /// An italic tab that the next file replaces, until it is kept or edited.
    pub preview: bool,
}

/// The tabs of one Files panel and which one is selected.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EditorTabs {
    tabs: Vec<EditorTab>,
    selected: Option<String>,
}

impl EditorTabs {
    /// Opens a file: an open one is selected; a new one takes the place of the preview tab when
    /// `preview_tabs` is on (and there is one), else goes at the right. Returns the path of the
    /// preview tab it replaced, so the caller can forget that file's state.
    pub fn open(&mut self, path: &str, preview_tabs: bool) -> Option<String> {
        if self.contains(path) {
            self.selected = Some(path.to_string());
            return None;
        }
        let tab = EditorTab { path: path.to_string(), preview: preview_tabs };
        let mut replaced = None;
        match self.tabs.iter().position(|existing| existing.preview).filter(|_| preview_tabs) {
            Some(index) => replaced = Some(std::mem::replace(&mut self.tabs[index], tab).path),
            None => self.tabs.push(tab),
        }
        self.selected = Some(path.to_string());
        replaced
    }

    /// A preview tab becomes a normal one (`Keep open`, or its first edit).
    pub fn keep(&mut self, path: &str) {
        if let Some(tab) = self.tabs.iter_mut().find(|tab| tab.path == path) {
            tab.preview = false;
        }
    }

    pub fn select(&mut self, path: &str) {
        if self.contains(path) {
            self.selected = Some(path.to_string());
        }
    }

    /// Closing the selected tab selects its right neighbour, else the left one.
    pub fn close(&mut self, path: &str) {
        let Some(position) = self.tabs.iter().position(|tab| tab.path == path) else {
            return;
        };
        self.tabs.remove(position);
        if self.selected.as_deref() == Some(path) {
            self.selected = self.tabs.get(position).or_else(|| self.tabs.last()).map(|tab| tab.path.clone());
        }
    }

    pub fn close_others(&mut self, keep: &str) {
        self.tabs.retain(|tab| tab.path == keep);
        self.selected = self.tabs.first().map(|tab| tab.path.clone());
    }

    pub fn close_all(&mut self) {
        self.tabs.clear();
        self.selected = None;
    }

    /// Moves the `from` tab to the place of the `to` tab. The selection does not change.
    pub fn reorder(&mut self, from: &str, to: &str) {
        let position = |path: &str| self.tabs.iter().position(|tab| tab.path == path);
        let (Some(a), Some(b)) = (position(from), position(to)) else {
            return;
        };
        if a != b {
            let tab = self.tabs.remove(a);
            self.tabs.insert(b, tab);
        }
    }

    pub fn contains(&self, path: &str) -> bool {
        self.tabs.iter().any(|tab| tab.path == path)
    }

    pub fn tabs(&self) -> &[EditorTab] {
        &self.tabs
    }

    pub fn paths(&self) -> Vec<String> {
        self.tabs.iter().map(|tab| tab.path.clone()).collect()
    }

    pub fn selected(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(tabs: &EditorTabs) -> Vec<&str> {
        tabs.tabs().iter().map(|tab| tab.path.as_str()).collect()
    }

    #[test]
    fn a_new_file_opens_in_a_preview_tab_and_is_selected() {
        let mut tabs = EditorTabs::default();
        assert_eq!(tabs.open("a.py", true), None);
        assert_eq!(paths(&tabs), vec!["a.py"]);
        assert!(tabs.tabs()[0].preview);
        assert_eq!(tabs.selected(), Some("a.py"));
    }

    #[test]
    fn the_next_file_replaces_the_preview_tab_in_place_and_says_which_it_replaced() {
        let mut tabs = EditorTabs::default();
        tabs.open("kept.py", true);
        tabs.keep("kept.py");
        tabs.open("a.py", true);
        tabs.open("tail.py", false);

        assert_eq!(tabs.open("b.py", true), Some("a.py".to_string()));

        assert_eq!(paths(&tabs), vec!["kept.py", "b.py", "tail.py"]);
        assert_eq!(tabs.selected(), Some("b.py"));
    }

    #[test]
    fn opening_an_open_file_selects_it_and_changes_nothing_else() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", true);
        tabs.keep("a.py");
        tabs.open("b.py", true);

        assert_eq!(tabs.open("a.py", true), None);

        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
        assert_eq!(tabs.selected(), Some("a.py"));
        assert!(tabs.tabs()[1].preview, "b is still the preview tab");
    }

    #[test]
    fn with_preview_tabs_off_every_file_gets_a_normal_tab() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", false);
        tabs.open("b.py", false);
        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
        assert!(tabs.tabs().iter().all(|tab| !tab.preview));
    }

    #[test]
    fn keeping_a_preview_tab_makes_it_normal_so_the_next_file_gets_its_own_tab() {
        let mut tabs = EditorTabs::default();
        tabs.open("a.py", true);
        tabs.keep("a.py");
        assert_eq!(tabs.open("b.py", true), None);
        assert_eq!(paths(&tabs), vec!["a.py", "b.py"]);
    }

    #[test]
    fn closing_the_selected_tab_selects_its_right_neighbour_else_the_left() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.select("b");
        tabs.close("b");
        assert_eq!(tabs.selected(), Some("c"));
        tabs.close("c");
        assert_eq!(tabs.selected(), Some("a"));
        tabs.close("a");
        assert_eq!(tabs.selected(), None);
        assert!(tabs.tabs().is_empty());
    }

    #[test]
    fn closing_another_tab_keeps_the_selection() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.select("a");
        tabs.close("c");
        assert_eq!(tabs.selected(), Some("a"));
        tabs.close("missing");
        assert_eq!(paths(&tabs), vec!["a", "b"]);
    }

    #[test]
    fn close_others_keeps_one_tab_and_selects_it() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c"] {
            tabs.open(name, false);
        }
        tabs.close_others("b");
        assert_eq!(paths(&tabs), vec!["b"]);
        assert_eq!(tabs.selected(), Some("b"));
    }

    #[test]
    fn close_all_leaves_nothing() {
        let mut tabs = EditorTabs::default();
        tabs.open("a", false);
        tabs.close_all();
        assert!(tabs.tabs().is_empty());
        assert_eq!(tabs.selected(), None);
    }

    #[test]
    fn dragging_a_tab_onto_another_puts_it_at_that_place_and_keeps_the_selection() {
        let mut tabs = EditorTabs::default();
        for name in ["a", "b", "c", "d"] {
            tabs.open(name, false);
        }
        tabs.select("c");
        tabs.reorder("a", "d");
        assert_eq!(paths(&tabs), vec!["b", "c", "d", "a"]);
        assert_eq!(tabs.selected(), Some("c"));
        tabs.reorder("a", "b");
        assert_eq!(paths(&tabs), vec!["a", "b", "c", "d"]);
        tabs.reorder("a", "a");
        tabs.reorder("a", "missing");
        assert_eq!(paths(&tabs), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn two_files_with_the_same_name_in_different_folders_are_two_tabs() {
        let mut tabs = EditorTabs::default();
        tabs.open("a/x.py", false);
        tabs.open("b/x.py", false);
        assert_eq!(paths(&tabs), vec!["a/x.py", "b/x.py"]);
        tabs.close("a/x.py");
        assert_eq!(paths(&tabs), vec!["b/x.py"]);
    }

    #[test]
    fn selecting_a_path_that_is_not_open_does_nothing() {
        let mut tabs = EditorTabs::default();
        tabs.open("a", false);
        tabs.select("nope");
        assert_eq!(tabs.selected(), Some("a"));
    }
}
