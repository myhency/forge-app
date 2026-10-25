use std::collections::HashMap;
use std::path::Path;

use forge_application::LiveSessionId;

/// Where a path asked to open belongs: in the session folder (relative to it) or outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPlace {
    Inside(String),
    Outside,
}

pub fn open_place(folder: &Path, absolute_path: &str) -> OpenPlace {
    match forge_entities::relative_to_folder(folder, absolute_path) {
        Some(relative) => OpenPlace::Inside(relative),
        None => OpenPlace::Outside,
    }
}

/// The paths for which `exists` holds, in their order.
pub fn existing_outputs(paths: Vec<String>, exists: impl Fn(&str) -> bool) -> Vec<String> {
    paths.into_iter().filter(|path| exists(path)).collect()
}

/// Whether the dialog offers the default program for the file: not for a program.
pub fn offers_default_program(path: &str) -> bool {
    !forge_entities::is_program(path)
}

/// The Outputs card's files per session, with the ticket of each session's latest read.
#[derive(Debug, Default)]
pub struct OutputsStore {
    files: HashMap<LiveSessionId, Vec<String>>,
    tickets: HashMap<LiveSessionId, u64>,
}

impl OutputsStore {
    pub fn files(&self, id: LiveSessionId) -> Vec<String> {
        self.files.get(&id).cloned().unwrap_or_default()
    }

    /// Starts a read; a result is kept only if it carries the latest ticket.
    pub fn begin(&mut self, id: LiveSessionId) -> u64 {
        let ticket = self.tickets.get(&id).copied().unwrap_or(0).wrapping_add(1);
        self.tickets.insert(id, ticket);
        ticket
    }

    /// Whether a finished read would change anything: not for a stale ticket (overtaken, or the
    /// session closed), nor for the same list as before.
    pub fn would_change(&self, id: LiveSessionId, ticket: u64, kept: &[String]) -> bool {
        if self.tickets.get(&id) != Some(&ticket) {
            return false;
        }
        match self.files.get(&id) {
            Some(old) => old != kept,
            None => !kept.is_empty(),
        }
    }

    /// Keeps a finished read; `false` when it changed nothing. An empty list removes the entry.
    pub fn finish(&mut self, id: LiveSessionId, ticket: u64, kept: Vec<String>) -> bool {
        if !self.would_change(id, ticket, &kept) {
            return false;
        }
        if kept.is_empty() {
            self.files.remove(&id);
        } else {
            self.files.insert(id, kept);
        }
        true
    }

    pub fn forget(&mut self, id: LiveSessionId) {
        self.files.remove(&id);
        self.tickets.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> LiveSessionId {
        LiveSessionId(n)
    }

    fn list(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn a_result_that_a_later_read_overtook_is_dropped() {
        let mut store = OutputsStore::default();
        let (first, second) = (store.begin(id(1)), store.begin(id(1)));
        assert!(!store.finish(id(1), first, list(&["/a"])));
        assert!(store.files(id(1)).is_empty());
        assert!(store.finish(id(1), second, list(&["/b"])));
        assert_eq!(store.files(id(1)), ["/b"]);
    }

    #[test]
    fn a_result_after_the_session_closed_is_dropped() {
        let mut store = OutputsStore::default();
        let ticket = store.begin(id(1));
        store.forget(id(1));
        assert!(!store.finish(id(1), ticket, list(&["/a"])));
        assert!(store.files(id(1)).is_empty());
    }

    #[test]
    fn an_empty_result_removes_the_entry_and_the_same_list_changes_nothing() {
        let mut store = OutputsStore::default();
        let t = store.begin(id(1));
        assert!(!store.finish(id(1), t, Vec::new()), "no entry and no files: nothing to write");
        let t = store.begin(id(1));
        assert!(store.finish(id(1), t, list(&["/a"])));
        let t = store.begin(id(1));
        assert!(!store.finish(id(1), t, list(&["/a"])), "same list");
        let t = store.begin(id(1));
        assert!(store.finish(id(1), t, Vec::new()));
        assert!(store.files(id(1)).is_empty());
    }

    #[test]
    fn forgetting_a_session_leaves_the_others() {
        let mut store = OutputsStore::default();
        for n in [1, 2] {
            let t = store.begin(id(n));
            store.finish(id(n), t, list(&["/a"]));
        }
        store.forget(id(1));
        assert!(store.files(id(1)).is_empty());
        assert_eq!(store.files(id(2)), ["/a"]);
    }

    #[test]
    fn the_default_program_is_offered_for_a_document_not_for_a_program() {
        assert!(offers_default_program("/a/report.docx"));
        assert!(!offers_default_program("/a/tool.exe"));
        assert!(!offers_default_program("/a/tool"));
        assert!(!offers_default_program("C:\\a\\run.ps1"));
    }

    #[test]
    fn a_path_outside_the_folder_is_placed_outside() {
        assert_eq!(open_place(Path::new("/w/p"), "/Users/me/report.docx"), OpenPlace::Outside);
    }

    #[test]
    fn a_path_inside_the_folder_is_placed_inside_with_its_relative_path() {
        assert_eq!(open_place(Path::new("/w/p"), "/w/p/src/a.rs"), OpenPlace::Inside("src/a.rs".to_string()));
    }

    #[test]
    fn outputs_drop_paths_that_do_not_exist() {
        let paths = vec!["/a/one.xlsx".to_string(), "/a/gone.xlsx".to_string(), "/a/two.docx".to_string()];
        let kept = existing_outputs(paths, |path| !path.contains("gone"));
        assert_eq!(kept, ["/a/one.xlsx", "/a/two.docx"]);
    }
}
