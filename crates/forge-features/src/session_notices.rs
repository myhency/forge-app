use std::collections::HashMap;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_domain::NotificationPrefs;

use crate::sidebar::LiveSummary;
use crate::workspace::{Active, Workspace};
use crate::settings::Settings;

/// What one look at a session found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub phase: Phase,
    /// Permission requests waiting for an answer.
    pub waiting: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    ReplyDone,
    PermissionRequest,
    Failed,
}

impl Notice {
    /// Whether the person is needed (the Dock icon bounces for these), not just told.
    pub fn needs_attention(self) -> bool {
        matches!(self, Notice::PermissionRequest | Notice::Failed)
    }
}

/// What changed between two looks at one session that the person should hear about.
pub fn notice_between(before: &Snapshot, after: &Snapshot) -> Option<Notice> {
    if before.phase != Phase::Failed && after.phase == Phase::Failed {
        return Some(Notice::Failed);
    }
    if before.waiting == 0 && after.waiting > 0 {
        return Some(Notice::PermissionRequest);
    }
    if before.phase == Phase::Running && after.phase == Phase::Idle && after.waiting == 0 {
        return Some(Notice::ReplyDone);
    }
    None
}

/// A notice goes out when its switch is on and the person is not looking at that session.
pub fn should_send(notice: Notice, prefs: &NotificationPrefs, in_view: bool) -> bool {
    !in_view
        && match notice {
            Notice::ReplyDone => prefs.reply_done,
            Notice::PermissionRequest => prefs.permission_request,
            Notice::Failed => prefs.code_notice,
        }
}

pub fn text(notice: Notice, session_title: &str) -> (String, String) {
    match notice {
        Notice::ReplyDone => ("Reply done".to_string(), format!("{session_title} finished its reply")),
        Notice::PermissionRequest => ("Approval needed".to_string(), format!("{session_title} is waiting for your approval")),
        Notice::Failed => ("Session stopped".to_string(), format!("{session_title} could not continue")),
    }
}

const POLL: Duration = Duration::from_millis(500);

/// Watches every open session and sends the notifications the Notifications switches allow. A
/// routine's session is left to the routines timer, which tells about it already.
pub fn use_session_notices(workspace: Workspace, settings: Settings) {
    use_hook(move || {
        let notify = workspace.notify_port();
        dioxus::core::spawn_forever(async move {
            let mut before: HashMap<LiveSessionId, Snapshot> = HashMap::new();
            loop {
                let views = workspace.live_views();
                before.retain(|id, _| views.iter().any(|(open, _)| open == id));
                for (id, view) in views {
                    if workspace.is_routine_session(id) {
                        continue;
                    }
                    let (now, title) = {
                        let view = view.peek();
                        (Snapshot { phase: view.phase, waiting: view.pending_permissions.len() }, LiveSummary::from_view(&view).display_title())
                    };
                    let Some(notice) = before.insert(id, now).and_then(|previous| notice_between(&previous, &now)) else {
                        continue;
                    };
                    let config = settings.saved_config();
                    let focused = document::eval("return document.hasFocus();").join::<bool>().await.unwrap_or(true);
                    let in_view = focused && workspace.active() == Active::Live(id);
                    if !should_send(notice, &config.notifications, in_view) {
                        continue;
                    }
                    let (heading, body) = text(notice, &title);
                    let bounce = config.attention_bounce && notice.needs_attention();
                    let notify = notify.clone();
                    // A notification is a process spawn: off the UI thread. One that cannot be sent is not worth a banner.
                    tokio::task::spawn_blocking(move || {
                        let _ = notify.notify(&heading, &body);
                        if bounce {
                            let _ = notify.request_attention();
                        }
                    });
                }
                tokio::time::sleep(POLL).await;
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(phase: Phase, waiting: usize) -> Snapshot {
        Snapshot { phase, waiting }
    }

    #[test]
    fn a_turn_that_ends_is_a_reply_done() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Idle, 0)), Some(Notice::ReplyDone));
    }

    #[test]
    fn a_session_that_starts_waiting_notifies_once_not_on_every_update() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Running, 1)), Some(Notice::PermissionRequest));
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Running, 1)), None);
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Running, 2)), None, "a second request while one waits is not a new notice");
    }

    #[test]
    fn a_waiting_session_that_ends_its_turn_is_not_a_reply_done_while_a_request_remains() {
        assert_eq!(notice_between(&snap(Phase::Running, 1), &snap(Phase::Idle, 1)), None);
    }

    #[test]
    fn a_session_that_fails_notifies() {
        assert_eq!(notice_between(&snap(Phase::Running, 0), &snap(Phase::Failed, 0)), Some(Notice::Failed));
        assert_eq!(notice_between(&snap(Phase::Failed, 0), &snap(Phase::Failed, 0)), None, "a failure is told once");
    }

    #[test]
    fn an_idle_session_staying_idle_says_nothing() {
        assert_eq!(notice_between(&snap(Phase::Idle, 0), &snap(Phase::Idle, 0)), None);
    }

    #[test]
    fn a_notice_is_sent_only_when_its_switch_is_on_and_the_session_is_not_in_view() {
        let on = NotificationPrefs::default();
        assert!(should_send(Notice::ReplyDone, &on, false));
        assert!(!should_send(Notice::ReplyDone, &on, true), "the person is looking at it");
        assert!(!should_send(Notice::ReplyDone, &NotificationPrefs { reply_done: false, ..on }, false));
        assert!(!should_send(Notice::PermissionRequest, &NotificationPrefs { permission_request: false, ..on }, false));
        assert!(!should_send(Notice::Failed, &NotificationPrefs { code_notice: false, ..on }, false));
    }

    #[test]
    fn only_notices_that_need_the_person_bounce_the_dock() {
        assert!(Notice::PermissionRequest.needs_attention() && Notice::Failed.needs_attention());
        assert!(!Notice::ReplyDone.needs_attention());
    }

    #[test]
    fn the_text_names_the_session() {
        let (title, body) = text(Notice::PermissionRequest, "Fix login");
        assert_eq!(title, "Approval needed");
        assert!(body.contains("Fix login"));
    }
}
