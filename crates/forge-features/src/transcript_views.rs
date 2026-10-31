use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_entities::TranscriptView;

/// The view mode of each open session. It lives in memory only: a session that is opened again
/// starts in the default view.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptViews {
    modes: HashMap<LiveSessionId, TranscriptView>,
}

impl TranscriptViews {
    /// The session's own choice, else `default` (the view Settings says new sessions open in).
    pub fn get(&self, id: LiveSessionId, default: TranscriptView) -> TranscriptView {
        self.modes.get(&id).copied().unwrap_or(default)
    }

    /// Always stores: with a default that is not Normal, a chosen Normal must stay a choice.
    pub fn set(&mut self, id: LiveSessionId, view: TranscriptView) {
        self.modes.insert(id, view);
    }

    /// Drops what is kept for a session that was closed.
    pub fn forget(&mut self, id: LiveSessionId) {
        self.modes.remove(&id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_session_without_a_choice_follows_the_default() {
        let views = TranscriptViews::default();
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Normal), TranscriptView::Normal);
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Detailed), TranscriptView::Detailed);
    }

    #[test]
    fn each_session_keeps_its_own_mode() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Thinking);
        views.set(LiveSessionId(2), TranscriptView::Detailed);

        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Normal), TranscriptView::Thinking);
        assert_eq!(views.get(LiveSessionId(2), TranscriptView::Normal), TranscriptView::Detailed);
        assert_eq!(views.get(LiveSessionId(3), TranscriptView::Normal), TranscriptView::Normal);
    }

    #[test]
    fn a_chosen_normal_sticks_when_the_default_is_something_else() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Normal);
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Thinking), TranscriptView::Normal);
    }

    #[test]
    fn a_closed_session_goes_back_to_the_default() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Detailed);
        views.forget(LiveSessionId(1));
        assert_eq!(views.get(LiveSessionId(1), TranscriptView::Thinking), TranscriptView::Thinking);
    }
}
