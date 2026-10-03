use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_entities::TranscriptView;

/// The view mode of each open session. It lives in memory only: a session that is opened again
/// starts in Normal.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TranscriptViews {
    modes: HashMap<LiveSessionId, TranscriptView>,
}

impl TranscriptViews {
    pub fn get(&self, id: LiveSessionId) -> TranscriptView {
        self.modes.get(&id).copied().unwrap_or_default()
    }

    pub fn set(&mut self, id: LiveSessionId, view: TranscriptView) {
        if view == TranscriptView::default() {
            self.modes.remove(&id);
        } else {
            self.modes.insert(id, view);
        }
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
    fn a_session_starts_in_normal() {
        assert_eq!(TranscriptViews::default().get(LiveSessionId(1)), TranscriptView::Normal);
    }

    #[test]
    fn each_session_keeps_its_own_mode() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Thinking);
        views.set(LiveSessionId(2), TranscriptView::Detailed);

        assert_eq!(views.get(LiveSessionId(1)), TranscriptView::Thinking);
        assert_eq!(views.get(LiveSessionId(2)), TranscriptView::Detailed);
        assert_eq!(views.get(LiveSessionId(3)), TranscriptView::Normal);
    }

    #[test]
    fn setting_normal_again_stores_nothing() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Detailed);
        views.set(LiveSessionId(1), TranscriptView::Normal);

        assert_eq!(views, TranscriptViews::default());
    }

    #[test]
    fn a_closed_session_is_forgotten() {
        let mut views = TranscriptViews::default();
        views.set(LiveSessionId(1), TranscriptView::Thinking);
        views.forget(LiveSessionId(1));

        assert_eq!(views.get(LiveSessionId(1)), TranscriptView::Normal);
    }
}
