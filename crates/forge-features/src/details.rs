use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_ports::ChangedFile;

/// The cards of a session's Details panel; each folds on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailsCard {
    Environment,
    Context,
    Connectors,
    Outputs,
    Changes,
}

/// What the Changes card shows: the files with uncommitted changes and the lines they add and remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeCounts {
    pub files: usize,
    pub added: usize,
    pub removed: usize,
}

/// A request that is applied once: `take` hands it out and forgets it, a new `set` replaces the old.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneShot<T>(Option<T>);

impl<T: Copy> OneShot<T> {
    pub fn set(&mut self, value: T) {
        self.0 = Some(value);
    }

    pub fn take(&mut self) -> Option<T> {
        self.0.take()
    }

    pub fn pending(&self) -> bool {
        self.0.is_some()
    }
}

impl<T> Default for OneShot<T> {
    fn default() -> Self {
        Self(None)
    }
}

/// `None` when nothing changed. A file git gave no line count for (binary, untracked) is still a
/// file and adds nothing to the sums.
pub fn change_counts(files: &[ChangedFile]) -> Option<ChangeCounts> {
    if files.is_empty() {
        return None;
    }
    Some(ChangeCounts {
        files: files.len(),
        added: files.iter().filter_map(|file| file.added).sum(),
        removed: files.iter().filter_map(|file| file.removed).sum(),
    })
}

/// Whether a card is folded; cards start open.
pub fn card_folded(flags: &HashMap<(LiveSessionId, DetailsCard), bool>, id: LiveSessionId, card: DetailsCard) -> bool {
    flags.get(&(id, card)).copied().unwrap_or(false)
}

/// Drops the fold flags of every card of a session that closed.
pub fn forget_card_flags(flags: &mut HashMap<(LiveSessionId, DetailsCard), bool>, id: LiveSessionId) {
    flags.retain(|(session, _), _| *session != id);
}

#[cfg(test)]
mod tests {
    use forge_ports::ChangeStatus;

    use super::*;

    fn file(added: Option<usize>, removed: Option<usize>) -> ChangedFile {
        ChangedFile { path: "a.rs".into(), status: ChangeStatus::Modified, added, removed }
    }

    #[test]
    fn change_counts_sums_lines_and_counts_files() {
        let files = [file(Some(3), Some(1)), file(Some(10), Some(0))];
        assert_eq!(change_counts(&files), Some(ChangeCounts { files: 2, added: 13, removed: 1 }));
    }

    #[test]
    fn change_counts_is_none_for_no_files() {
        assert_eq!(change_counts(&[]), None);
    }

    #[test]
    fn a_missing_line_count_counts_as_zero() {
        let files = [file(None, None), file(Some(2), None)];
        assert_eq!(change_counts(&files), Some(ChangeCounts { files: 2, added: 2, removed: 0 }));
    }

    #[test]
    fn a_one_shot_request_is_taken_once_and_a_new_one_replaces_the_old() {
        let mut request = OneShot::default();
        assert!(!request.pending());
        request.set(1);
        request.set(2);
        assert!(request.pending());
        assert_eq!(request.take(), Some(2));
        assert_eq!(request.take(), None);
        assert!(!request.pending());
    }

    #[test]
    fn folding_one_card_leaves_the_others() {
        let mut flags = HashMap::new();
        let id = LiveSessionId(1);
        flags.insert((id, DetailsCard::Context), true);
        assert!(card_folded(&flags, id, DetailsCard::Context));
        assert!(!card_folded(&flags, id, DetailsCard::Environment));
        assert!(!card_folded(&flags, LiveSessionId(2), DetailsCard::Context));
    }

    #[test]
    fn a_closed_sessions_card_flags_are_dropped() {
        let mut flags = HashMap::new();
        let (a, b) = (LiveSessionId(1), LiveSessionId(2));
        for card in [DetailsCard::Environment, DetailsCard::Changes] {
            flags.insert((a, card), true);
        }
        flags.insert((b, DetailsCard::Changes), true);
        forget_card_flags(&mut flags, a);
        assert_eq!(flags.len(), 1);
        assert!(card_folded(&flags, b, DetailsCard::Changes));
    }
}
