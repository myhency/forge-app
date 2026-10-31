use std::collections::{HashMap, HashSet};

use forge_ports::FileVersion;

use crate::editor_doc::text_hash;
use crate::editor_tabs::EditorTabs;

/// Text typed in a tab that is not on disk, and what the file was when it was typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub text: String,
    /// The version and the content hash of the file the text was typed against.
    pub base: FileVersion,
    pub base_hash: u64,
    /// The tab showed the conflict banner when this was typed: it stays a conflict until the person
    /// answers it (Overwrite writes the text, Cancel drops it).
    pub conflict: bool,
}

/// What a tab shows when its file is read again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restore {
    /// No draft, or one equal to the text on disk: the text on disk.
    FromDisk,
    /// The draft, and the file is as it was when the text was typed.
    Draft(String),
    /// The draft, but the file changed on disk since (or the tab was already in conflict): the
    /// conflict banner goes with it.
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
    /// as `\n`), `base` the version it was read at, `conflict` whether the tab shows the conflict banner.
    pub fn note_text(&mut self, path: &str, text: &str, disk_hash: u64, base: FileVersion, conflict: bool) {
        if text_hash(text) == disk_hash {
            self.drafts.remove(path);
            return;
        }
        let conflict = conflict || self.drafts.get(path).is_some_and(|draft| draft.conflict);
        self.drafts.insert(path.to_string(), Draft { text: text.to_string(), base, base_hash: disk_hash, conflict });
    }

    /// A write of `text` to `path` succeeded and left the file at `version`. The draft of that text is
    /// done; text typed while the write ran stays, now against what was written.
    pub fn written(&mut self, path: &str, text: &str, version: FileVersion) {
        let written_hash = text_hash(text);
        if let Some(draft) = self.drafts.get_mut(path) {
            if text_hash(&draft.text) == written_hash {
                self.drafts.remove(path);
            } else {
                draft.base = version;
                draft.base_hash = written_hash;
                draft.conflict = false;
            }
        }
    }

    pub fn clear_draft(&mut self, path: &str) {
        self.drafts.remove(path);
    }

    pub fn has_draft(&self, path: &str) -> bool {
        self.drafts.contains_key(path)
    }

    pub fn draft_text(&self, path: &str) -> Option<String> {
        self.drafts.get(path).map(|draft| draft.text.clone())
    }

    /// What to show for `path` now that its file was read at version `fresh` with content hash
    /// `fresh_hash`.
    pub fn restore(&self, path: &str, fresh: FileVersion, fresh_hash: u64) -> Restore {
        match self.drafts.get(path) {
            None => Restore::FromDisk,
            Some(draft) if text_hash(&draft.text) == fresh_hash => Restore::FromDisk,
            Some(draft) if draft.conflict || draft.base != fresh || draft.base_hash != fresh_hash => {
                Restore::DraftWithConflict(draft.text.clone())
            }
            Some(draft) => Restore::Draft(draft.text.clone()),
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

    fn h(text: &str) -> u64 {
        text_hash(text)
    }

    #[test]
    fn a_text_that_differs_from_disk_becomes_a_draft_and_one_that_matches_clears_it() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", h("disk"), v(1), false);
        assert!(store.has_draft("a.py"));
        store.note_text("a.py", "disk", h("disk"), v(1), false);
        assert!(!store.has_draft("a.py"));
    }

    #[test]
    fn an_empty_text_that_differs_from_the_disk_is_a_draft() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "", h("disk"), v(1), false);
        assert_eq!(store.restore("a.py", v(1), h("disk")), Restore::Draft(String::new()));
    }

    #[test]
    fn no_draft_means_the_disk_text() {
        assert_eq!(FilesStore::default().restore("a.py", v(1), h("disk")), Restore::FromDisk);
    }

    #[test]
    fn a_draft_comes_back_as_it_was_when_the_disk_is_unchanged() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", h("disk"), v(1), false);
        assert_eq!(store.restore("a.py", v(1), h("disk")), Restore::Draft("typed".to_string()));
    }

    #[test]
    fn a_draft_comes_back_with_a_conflict_when_the_disk_changed_meanwhile() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", h("disk"), v(1), false);
        assert_eq!(store.restore("a.py", v(2), h("other")), Restore::DraftWithConflict("typed".to_string()));
    }

    #[test]
    fn a_change_that_kept_the_version_but_not_the_content_is_a_conflict_too() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", h("disk"), v(1), false);
        assert_eq!(store.restore("a.py", v(1), h("sneaky")), Restore::DraftWithConflict("typed".to_string()));
    }

    #[test]
    fn a_draft_typed_under_a_conflict_keeps_its_conflict_when_the_base_moves() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "typed", h("old"), v(1), false);
        // The panel came back: the doc is at the fresh version v2 with the banner, and the page reports again.
        store.note_text("a.py", "typed", h("new"), v(2), true);
        assert_eq!(store.restore("a.py", v(2), h("new")), Restore::DraftWithConflict("typed".to_string()));
        // Still a conflict on a later note that does not know about the banner (it is sticky).
        store.note_text("a.py", "typed more", h("new"), v(2), false);
        assert_eq!(store.restore("a.py", v(2), h("new")), Restore::DraftWithConflict("typed more".to_string()));
    }

    #[test]
    fn a_draft_equal_to_the_disk_text_is_not_a_draft_on_restore() {
        let mut store = FilesStore::default();
        // The write finished on disk but the panel went away before the follow up ran.
        store.note_text("a.py", "typed", h("disk"), v(1), false);
        assert_eq!(store.restore("a.py", v(2), h("typed")), Restore::FromDisk);
    }

    #[test]
    fn a_finished_write_clears_the_draft_of_that_text_but_keeps_newer_typing() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "text1", h("disk"), v(1), false);
        store.written("a.py", "text1", v(2));
        assert!(!store.has_draft("a.py"));

        store.note_text("a.py", "text1", h("disk"), v(1), true);
        // text2 was typed while the write of text1 ran.
        store.note_text("a.py", "text2", h("disk"), v(1), true);
        store.written("a.py", "text1", v(2));
        assert_eq!(store.draft_text("a.py"), Some("text2".to_string()));
        // The write succeeded, so the new draft is against the new version and the content just written.
        assert_eq!(store.restore("a.py", v(2), h("text1")), Restore::Draft("text2".to_string()));
    }

    #[test]
    fn drafts_are_per_file() {
        let mut store = FilesStore::default();
        store.note_text("a.py", "x", h("a"), v(1), false);
        assert!(!store.has_draft("b.py"));
        store.clear_draft("b.py");
        assert!(store.has_draft("a.py"));
        assert_eq!(store.draft_text("b.py"), None);
    }
}
