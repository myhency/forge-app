use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use forge_ports::{FileVersion, WriteError};

/// A hash of a text, to tell whether the editor holds what is on disk without keeping a second copy.
pub fn text_hash(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// What the banner over the editor says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Banner {
    /// The file changed on disk since the editor read it.
    Conflict,
    SaveFailed(String),
}

impl Banner {
    pub fn message(&self) -> String {
        match self {
            Banner::Conflict => "This file has changed on disk since you started editing.".to_string(),
            Banner::SaveFailed(reason) => format!("Could not save: {reason}"),
        }
    }
}

/// What Rust knows about one open text tab: the file version it last read or wrote, a hash of the
/// text that is on disk, and the banner. The text itself lives in the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabDoc {
    pub loaded: FileVersion,
    pub clean: u64,
    pub banner: Option<Banner>,
}

impl TabDoc {
    pub fn new(loaded: FileVersion, text: &str) -> Self {
        Self { loaded, clean: text_hash(text), banner: None }
    }

    /// The editor reports its first change since a read or a write. `disk` is the file's version
    /// now: a different one is a conflict; a file that cannot be looked at is a failed save.
    pub fn on_dirty(&mut self, disk: Result<FileVersion, String>) {
        if self.banner.is_some() {
            return;
        }
        self.banner = match disk {
            Ok(version) if version == self.loaded => None,
            Ok(_) => Some(Banner::Conflict),
            Err(reason) => Some(Banner::SaveFailed(reason)),
        };
    }

    /// Whether the timer's save may write: auto save is on and no banner waits for an answer.
    pub fn may_write(&self, auto_save: bool) -> bool {
        auto_save && self.banner.is_none()
    }

    /// Whether `text` (what the editor holds now) is not what is on disk.
    pub fn differs(&self, text: &str) -> bool {
        text_hash(text) != self.clean
    }

    /// A write of `text` succeeded.
    pub fn written(&mut self, version: FileVersion, text: &str) {
        self.loaded = version;
        self.clean = text_hash(text);
        self.banner = None;
    }

    pub fn write_failed(&mut self, error: &WriteError) {
        self.banner = Some(match error {
            WriteError::Changed => Banner::Conflict,
            WriteError::Outside => Banner::SaveFailed(WriteError::Outside.to_string()),
            WriteError::Failed(reason) => Banner::SaveFailed(reason.clone()),
        });
    }

    /// The file was read again (Cancel on the conflict banner, or a quiet refresh).
    pub fn reloaded(&mut self, version: FileVersion, text: &str) {
        self.loaded = version;
        self.clean = text_hash(text);
        self.banner = None;
    }

    pub fn clear_banner(&mut self) {
        self.banner = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(ns: u128, len: u64) -> FileVersion {
        FileVersion { modified_ns: ns, len }
    }

    #[test]
    fn a_freshly_read_text_is_clean_and_a_different_text_differs() {
        let doc = TabDoc::new(version(1, 3), "abc");
        assert!(!doc.differs("abc"));
        assert!(doc.differs("abcd"));
        assert!(doc.differs(""));
    }

    #[test]
    fn the_first_edit_on_an_unchanged_disk_raises_no_banner() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(1, 3)));
        assert_eq!(doc.banner, None);
        assert!(doc.may_write(true));
    }

    #[test]
    fn the_first_edit_after_the_disk_changed_raises_the_conflict_banner_and_pauses_auto_save() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        assert_eq!(doc.banner, Some(Banner::Conflict));
        assert!(!doc.may_write(true));
    }

    #[test]
    fn a_file_that_cannot_be_looked_at_raises_a_save_failed_banner_with_the_reason() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Err("No such file or directory".to_string()));
        assert_eq!(doc.banner, Some(Banner::SaveFailed("No such file or directory".to_string())));
        assert!(!doc.may_write(true));
    }

    #[test]
    fn a_banner_already_shown_is_not_replaced_by_a_later_check() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        doc.on_dirty(Ok(version(1, 3)));
        assert_eq!(doc.banner, Some(Banner::Conflict));
    }

    #[test]
    fn with_auto_save_off_nothing_may_be_written_by_the_timer() {
        let doc = TabDoc::new(version(1, 3), "abc");
        assert!(!doc.may_write(false));
    }

    #[test]
    fn a_successful_write_makes_that_text_the_clean_one_and_keeps_later_typing_dirty() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.written(version(5, 4), "abcd");
        assert_eq!(doc.loaded, version(5, 4));
        assert!(!doc.differs("abcd"));
        // The person typed one more letter while the write was in flight.
        assert!(doc.differs("abcde"), "text typed during a write must still be saved by the next change");
    }

    #[test]
    fn a_write_that_found_the_file_changed_raises_the_conflict_banner() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Changed);
        assert_eq!(doc.banner, Some(Banner::Conflict));
    }

    #[test]
    fn a_write_that_failed_keeps_the_reason_and_the_clean_text() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Failed("Permission denied".to_string()));
        assert_eq!(doc.banner, Some(Banner::SaveFailed("Permission denied".to_string())));
        assert!(doc.differs("abcd"), "the clean text is still the one on disk");
        doc.write_failed(&WriteError::Outside);
        assert!(matches!(doc.banner, Some(Banner::SaveFailed(_))));
    }

    #[test]
    fn reloading_from_disk_clears_the_banner_and_takes_the_new_text() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.on_dirty(Ok(version(2, 9)));
        doc.reloaded(version(2, 9), "disk text");
        assert_eq!(doc.banner, None);
        assert_eq!(doc.loaded, version(2, 9));
        assert!(!doc.differs("disk text"));
    }

    #[test]
    fn clearing_the_banner_lets_auto_save_go_on() {
        let mut doc = TabDoc::new(version(1, 3), "abc");
        doc.write_failed(&WriteError::Failed("x".to_string()));
        doc.clear_banner();
        assert!(doc.may_write(true));
    }

    #[test]
    fn the_banner_messages_are_plain_english() {
        assert_eq!(Banner::Conflict.message(), "This file has changed on disk since you started editing.");
        assert_eq!(Banner::SaveFailed("Permission denied".to_string()).message(), "Could not save: Permission denied");
    }
}
