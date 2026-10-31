use std::path::PathBuf;

use forge_domain::SessionId;

/// The sessions closed during this run of the app, newest last, for "reopen the last closed
/// session". Kept in memory only.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClosedSessions {
    items: Vec<(PathBuf, SessionId)>,
}

impl ClosedSessions {
    pub const MAX: usize = 20;

    /// Remembers a closed session. One that is already remembered moves to the newest place; past
    /// `MAX` the oldest is forgotten.
    pub fn push(&mut self, project: PathBuf, session: SessionId) {
        self.items.retain(|(path, id)| !(*path == project && *id == session));
        self.items.push((project, session));
        if self.items.len() > Self::MAX {
            self.items.remove(0);
        }
    }

    /// The most recently closed session, forgotten from now on.
    pub fn pop(&mut self) -> Option<(PathBuf, SessionId)> {
        self.items.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(text: &str) -> SessionId {
        SessionId(text.to_string())
    }

    #[test]
    fn the_newest_comes_back_first() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/a"), id("two"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("two"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
        assert_eq!(closed.pop(), None);
    }

    #[test]
    fn nothing_closed_gives_nothing() {
        assert_eq!(ClosedSessions::default().pop(), None);
    }

    #[test]
    fn closing_the_same_session_twice_keeps_it_once_at_the_newest_place() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/a"), id("two"));
        closed.push(PathBuf::from("/a"), id("one"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("two"))));
        assert_eq!(closed.pop(), None);
    }

    #[test]
    fn the_same_id_in_another_folder_is_another_session() {
        let mut closed = ClosedSessions::default();
        closed.push(PathBuf::from("/a"), id("one"));
        closed.push(PathBuf::from("/b"), id("one"));
        assert_eq!(closed.pop(), Some((PathBuf::from("/b"), id("one"))));
        assert_eq!(closed.pop(), Some((PathBuf::from("/a"), id("one"))));
    }

    #[test]
    fn only_the_newest_twenty_are_kept() {
        let mut closed = ClosedSessions::default();
        for n in 0..25 {
            closed.push(PathBuf::from("/a"), id(&format!("s{n}")));
        }
        let mut popped = Vec::new();
        while let Some((_, session)) = closed.pop() {
            popped.push(session.0);
        }
        assert_eq!(popped.len(), ClosedSessions::MAX);
        assert_eq!(popped.first().map(String::as_str), Some("s24"));
        assert_eq!(popped.last().map(String::as_str), Some("s5"));
    }
}
