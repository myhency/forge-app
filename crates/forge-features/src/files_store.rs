use std::collections::{HashMap, HashSet};

use forge_ports::FileVersion;

use crate::editor_doc::text_hash;
use crate::editor_tabs::EditorTabs;

/// Text typed in a tab that is not on disk, and the version of the file it was typed against.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    pub base: FileVersion,
}

/// What a tab shows when its file is read again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// No draft: the text on disk.
    FromDisk,
    /// The draft, and the file is as it was when the text was typed.
    Draft(String),
    /// The draft, but the file changed on disk since: the conflict banner goes with it.
    DraftWithConflict(String),
}

/// What the Files panel of one session keeps while the panel is not on screen: its tabs, the
/// markdown files shown as source, and the unsaved text of tabs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilesStore {
    pub tabs: EditorTabs,
    pub source: HashSet<String>,
    drafts: HashMap<String, Draft>,
}

impl FilesStore {
    /// The editor reported `text` for `path`. `disk_hash` is the hash of the text on disk (line ends
    /// as `\n`), `base` the version it was read at.
    pub fn note_text(&mut self, path: &str, text: &str, disk_hash: u64, base: FileVersion) {
        if text_hash(text) == disk_hash {
            self.drafts.remove(path);
        } else {
            self.drafts.insert(path.to_string(), Draft { text: text.to_string(), base });
        }
    }

    pub fn clear_draft(&mut self, path: &str) {
        self.drafts.remove(path);
    }

    pub fn has_draft(&self, path: &str) -> bool {
        self.drafts.contains_key(path)
    }

    /// What to show for `path` now that its file was read at version `fresh`.
    pub fn restore(&self, path: &str, fresh: FileVersion) -> Restore {
        match self.drafts.get(path) {
            None => Restore::FromDisk,
            Some(draft) if draft.base == fresh => Restore::Draft(draft.text.clone()),
            Some(draft) => Restore::DraftWithConflict(draft.text.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_doc::text_hash;

    fn v(n: u128) -> FileVersion {
        FileVersion { modified_ns: n, len: n as u64 }
    }

    #[test]
    fn a_text_that_differs_from_disk_becomes_a_draft_and_one_that_matches_clears_it() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert!(store.has_draft("a.py"));
        store.note_text("a.py", "disk", text_hash("disk"), v(1));
        assert!(!store.has_draft("a.py"));
    }

    #[test]
    fn an_empty_text_that_differs_from_the_disk_is_a_draft() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(1)), Restore::Draft(String::new()));
    }

    #[test]
    fn no_draft_means_the_disk_text() {
        assert_eq!(FilesStore::default().restore("a.py", v(1)), Restore::FromDisk);
    }

    #[test]
    fn a_draft_comes_back_as_it_was_when_the_disk_is_unchanged() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(1)), Restore::Draft("typed".to_string()));
    }

    #[test]
    fn a_draft_comes_back_with_a_conflict_when_the_disk_changed_meanwhile() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        assert_eq!(store.restore("a.py", v(2)), Restore::DraftWithConflict("typed".to_string()));
    }

    #[test]
    fn a_save_then_a_stale_note_order_never_resurrects_old_text() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", text_hash("disk"), v(1));
        // The write finished: the disk now holds "typed", and the next report equals it.
        store.clear_draft("a.py");
        store.note_text("a.py", "typed", text_hash("typed"), v(2));
        assert_eq!(store.restore("a.py", v(2)), Restore::FromDisk);
    }

    #[test]
    fn drafts_are_per_file() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "x", text_hash("a"), v(1));
        assert!(!store.has_draft("b.py"));
        store.clear_draft("b.py");
        assert!(store.has_draft("a.py"));
    }
}
