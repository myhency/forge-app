use std::collections::{HashMap, HashSet};

use forge_application::LiveSessionId;

/// What the Background tasks panel keeps for one session: whether its `Finished` fold is open and
/// which finished cards the person cleared with the trash icon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskListView {
    pub finished_open: bool,
    pub cleared: HashSet<String>,
}

/// The task list state of each open session. In memory only: nothing is deleted from the session's
/// record, so after a restart the finished cards are back.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskListViews {
    by_session: HashMap<LiveSessionId, TaskListView>,
}

impl TaskListViews {
    pub fn get(&self, id: LiveSessionId) -> TaskListView {
        self.by_session.get(&id).cloned().unwrap_or_default()
    }

    pub fn toggle_finished(&mut self, id: LiveSessionId) {
        let view = self.by_session.entry(id).or_default();
        view.finished_open = !view.finished_open;
    }

    /// Hides these finished tasks. A task that ends later is not in the set and shows.
    pub fn clear(&mut self, id: LiveSessionId, task_ids: Vec<String>) {
        self.by_session.entry(id).or_default().cleared.extend(task_ids);
    }

    /// Drops what is kept for a session that was closed.
    pub fn forget(&mut self, id: LiveSessionId) {
        self.by_session.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_starts_closed_with_nothing_cleared() {
        let views = TaskListViews::default();
        let view = views.get(LiveSessionId(1));
        assert!(!view.finished_open);
        assert!(view.cleared.is_empty());
    }

    #[test]
    fn the_fold_toggles_per_session() {
        let mut views = TaskListViews::default();
        views.toggle_finished(LiveSessionId(1));
        assert!(views.get(LiveSessionId(1)).finished_open);
        assert!(!views.get(LiveSessionId(2)).finished_open);
        views.toggle_finished(LiveSessionId(1));
        assert!(!views.get(LiveSessionId(1)).finished_open);
    }

    #[test]
    fn clearing_hides_only_the_ids_given_and_adds_up() {
        let mut views = TaskListViews::default();
        views.clear(LiveSessionId(1), vec!["a".to_string()]);
        views.clear(LiveSessionId(1), vec!["b".to_string()]);
        let cleared = views.get(LiveSessionId(1)).cleared;
        assert!(cleared.contains("a") && cleared.contains("b") && !cleared.contains("c"));
        assert!(views.get(LiveSessionId(2)).cleared.is_empty());
    }

    #[test]
    fn a_closed_session_is_forgotten() {
        let mut views = TaskListViews::default();
        views.toggle_finished(LiveSessionId(1));
        views.clear(LiveSessionId(1), vec!["a".to_string()]);
        views.forget(LiveSessionId(1));
        assert_eq!(views.get(LiveSessionId(1)), TaskListView::default());
    }
}
