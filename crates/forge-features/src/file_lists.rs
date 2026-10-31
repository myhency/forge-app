//! The `@` mention's file list per session: the folder walk's last result, whether a walk is in
//! flight, and when the list was walked, so a refresh starts only when it is due and never twice.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use forge_application::LiveSessionId;
use forge_ports::FileList;

/// The most files one walk keeps.
pub const FILE_LIST_LIMIT: usize = 5_000;
/// A list older than this is walked again the next time the list opens.
pub const FILE_LIST_MAX_AGE: Duration = Duration::from_secs(30);

/// What the mention list can show for a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileListState {
    /// Never walked.
    Idle,
    /// A walk is in flight; the list from the walk before, if there was one, stays visible.
    Loading(Option<Vec<String>>),
    Ready { files: Vec<String>, cut: bool },
}

#[derive(Debug, Default)]
struct Entry {
    list: Option<FileList>,
    in_flight: Option<u64>,
    /// When the last walk started, and the turn mark it started at.
    walked: Option<(Instant, Option<usize>)>,
}

/// The file lists of the open sessions.
#[derive(Debug, Default)]
pub struct FileListStore {
    entries: HashMap<LiveSessionId, Entry>,
    next_ticket: u64,
}

impl FileListStore {
    pub fn state(&self, id: LiveSessionId) -> FileListState {
        let Some(entry) = self.entries.get(&id) else { return FileListState::Idle };
        match (&entry.list, entry.in_flight) {
            (old, Some(_)) => FileListState::Loading(old.as_ref().map(|list| list.files.clone())),
            (Some(list), None) => FileListState::Ready { files: list.files.clone(), cut: list.cut },
            (None, None) => FileListState::Idle,
        }
    }

    /// Whether the list should be walked now: never walked, older than `FILE_LIST_MAX_AGE`, or a
    /// turn ended since (`turn_mark` is the idle session's message count, `None` while a turn
    /// runs). Never while a walk is in flight.
    pub fn needs_walk(&self, id: LiveSessionId, now: Instant, turn_mark: Option<usize>) -> bool {
        let Some(entry) = self.entries.get(&id) else { return true };
        if entry.in_flight.is_some() {
            return false;
        }
        match entry.walked {
            None => true,
            Some((at, mark)) => now.saturating_duration_since(at) >= FILE_LIST_MAX_AGE || turn_mark.is_some_and(|now_mark| Some(now_mark) != mark),
        }
    }

    /// Starts a walk and gives its ticket; `None` when one is in flight already.
    pub fn begin(&mut self, id: LiveSessionId, now: Instant, turn_mark: Option<usize>) -> Option<u64> {
        let entry = self.entries.entry(id).or_default();
        if entry.in_flight.is_some() {
            return None;
        }
        self.next_ticket = self.next_ticket.wrapping_add(1);
        entry.in_flight = Some(self.next_ticket);
        entry.walked = Some((now, turn_mark));
        Some(self.next_ticket)
    }

    /// Whether `ticket` is the walk in flight for the session.
    pub fn is_current(&self, id: LiveSessionId, ticket: u64) -> bool {
        self.entries.get(&id).is_some_and(|entry| entry.in_flight == Some(ticket))
    }

    /// Keeps a finished walk; `false` for a stale ticket (the session closed or moved meanwhile).
    pub fn finish(&mut self, id: LiveSessionId, ticket: u64, list: FileList) -> bool {
        let Some(entry) = self.entries.get_mut(&id) else { return false };
        if entry.in_flight != Some(ticket) {
            return false;
        }
        entry.in_flight = None;
        entry.list = Some(list);
        true
    }

    pub fn forget(&mut self, id: LiveSessionId) {
        self.entries.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(n: u64) -> LiveSessionId {
        LiveSessionId(n)
    }

    fn list(names: &[&str], cut: bool) -> FileList {
        FileList { files: names.iter().map(|name| name.to_string()).collect(), cut }
    }

    #[test]
    fn a_session_never_walked_is_idle_and_needs_a_walk() {
        let store = FileListStore::default();
        assert_eq!(store.state(id(1)), FileListState::Idle);
        assert!(store.needs_walk(id(1), Instant::now(), Some(0)));
    }

    #[test]
    fn a_walk_in_flight_is_not_started_twice() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        assert!(store.begin(id(1), now, Some(0)).is_some());
        assert_eq!(store.state(id(1)), FileListState::Loading(None));
        assert!(!store.needs_walk(id(1), now + FILE_LIST_MAX_AGE * 2, Some(9)));
        assert_eq!(store.begin(id(1), now, Some(0)), None);
        assert!(store.begin(id(2), now, Some(0)).is_some(), "another session walks on its own");
    }

    #[test]
    fn a_finished_walk_is_ready_and_fresh() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        let ticket = store.begin(id(1), now, Some(3)).unwrap();
        assert!(store.finish(id(1), ticket, list(&["a.txt"], true)));
        assert_eq!(store.state(id(1)), FileListState::Ready { files: vec!["a.txt".into()], cut: true });
        assert!(!store.needs_walk(id(1), now + Duration::from_secs(5), Some(3)));
        assert!(!store.needs_walk(id(1), now + Duration::from_secs(5), None), "a turn running is no reason");
    }

    #[test]
    fn a_list_is_walked_again_after_thirty_seconds_or_when_a_turn_ended() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        let ticket = store.begin(id(1), now, Some(3)).unwrap();
        store.finish(id(1), ticket, list(&["a.txt"], false));
        assert!(store.needs_walk(id(1), now + FILE_LIST_MAX_AGE, Some(3)));
        assert!(store.needs_walk(id(1), now + Duration::from_secs(1), Some(5)), "the turn added messages");
    }

    #[test]
    fn a_refresh_keeps_the_old_list_visible_until_the_new_one_arrives() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        let first = store.begin(id(1), now, Some(0)).unwrap();
        store.finish(id(1), first, list(&["old.txt"], false));
        let second = store.begin(id(1), now + FILE_LIST_MAX_AGE, Some(0)).unwrap();
        assert_eq!(store.state(id(1)), FileListState::Loading(Some(vec!["old.txt".into()])));
        assert!(store.finish(id(1), second, list(&["new.txt"], false)));
        assert_eq!(store.state(id(1)), FileListState::Ready { files: vec!["new.txt".into()], cut: false });
    }

    #[test]
    fn a_walk_finishing_after_the_session_closed_is_dropped_and_the_cache_goes() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        let ticket = store.begin(id(1), now, Some(0)).unwrap();
        store.forget(id(1));
        assert!(!store.finish(id(1), ticket, list(&["a.txt"], false)));
        assert_eq!(store.state(id(1)), FileListState::Idle);
        let ticket = store.begin(id(1), now, Some(0)).unwrap();
        store.finish(id(1), ticket, list(&["a.txt"], false));
        store.forget(id(1));
        assert_eq!(store.state(id(1)), FileListState::Idle, "closing drops the list");
    }

    #[test]
    fn a_stale_ticket_from_before_a_forget_does_not_finish_the_new_walk() {
        let mut store = FileListStore::default();
        let now = Instant::now();
        let old = store.begin(id(1), now, Some(0)).unwrap();
        store.forget(id(1));
        let new = store.begin(id(1), now, Some(0)).unwrap();
        assert_ne!(old, new);
        assert!(!store.is_current(id(1), old) && store.is_current(id(1), new));
        assert!(!store.finish(id(1), old, list(&["stale.txt"], false)));
        assert_eq!(store.state(id(1)), FileListState::Loading(None));
        assert!(store.finish(id(1), new, list(&["fresh.txt"], false)));
    }
}
