use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use forge_domain::{PermissionDecision, PermissionRequest, Session, SessionId};
use forge_ports::SidecarFactoryPort;
use tokio::sync::{mpsc, watch};

use crate::live_session_actor::Actor;

/// App-local handle for an open session. Claude's own `SessionId` is unknown for a brand-new
/// session until the sidecar reports `session_started`, so it cannot be the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LiveSessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No turn in flight (a sidecar may or may not exist).
    Idle,
    Running,
    /// The last sidecar failed; the next `send` respawns one.
    Failed,
}

/// Everything the UI needs to draw one open session.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSessionView {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    /// `session.id` is empty until the Claude session id is known.
    pub session: Session,
    pub claude_session_id: Option<SessionId>,
    pub phase: Phase,
    /// Oldest first; the front is the one the UI shows.
    pub pending_permissions: VecDeque<PermissionRequest>,
    pub last_error: Option<String>,
    /// `false` when the project folder no longer exists: the history is readable only.
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiveCommand {
    Send(String),
    Approve { id: String, decision: PermissionDecision },
    Interrupt,
    Close,
}

/// A cheap, cloneable remote control for one open session. Every command is fire-and-forget: the
/// outcome shows up in the view.
#[derive(Clone)]
pub struct LiveSessionHandle {
    id: LiveSessionId,
    commands: mpsc::UnboundedSender<LiveCommand>,
    view: watch::Receiver<LiveSessionView>,
}

impl LiveSessionHandle {
    pub fn id(&self) -> LiveSessionId {
        self.id
    }

    pub fn send(&self, text: String) {
        let _ = self.commands.send(LiveCommand::Send(text));
    }

    pub fn approve(&self, permission_request_id: String, decision: PermissionDecision) {
        let _ = self.commands.send(LiveCommand::Approve {
            id: permission_request_id,
            decision,
        });
    }

    pub fn interrupt(&self) {
        let _ = self.commands.send(LiveCommand::Interrupt);
    }

    pub fn close(&self) {
        let _ = self.commands.send(LiveCommand::Close);
    }

    /// A receiver that wakes on every published change and reports `Err` once the session is closed.
    pub fn subscribe(&self) -> watch::Receiver<LiveSessionView> {
        self.view.clone()
    }

    pub fn snapshot(&self) -> LiveSessionView {
        self.view.borrow().clone()
    }

    /// The Claude session id, once known (immediately for a resumed session). Cheaper than
    /// `snapshot`, which clones the whole history.
    pub fn claude_session_id(&self) -> Option<SessionId> {
        self.view.borrow().claude_session_id.clone()
    }

    /// `true` once the session's task has ended (after `close`).
    pub fn is_closed(&self) -> bool {
        self.commands.is_closed()
    }
}

pub(crate) struct LiveSessionInit {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub session: Session,
    pub claude_session_id: Option<SessionId>,
    pub model: Option<String>,
    pub can_send: bool,
    pub factory: Arc<dyn SidecarFactoryPort>,
}

/// Starts the session's task. Must be called from inside a tokio runtime.
pub(crate) fn spawn_live_session(init: LiveSessionInit) -> LiveSessionHandle {
    let view = LiveSessionView {
        id: init.id,
        project_path: init.project_path,
        session: init.session,
        claude_session_id: init.claude_session_id,
        phase: Phase::Idle,
        pending_permissions: VecDeque::new(),
        last_error: None,
        can_send: init.can_send,
    };
    let (view_tx, view_rx) = watch::channel(view.clone());
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let actor = Actor::new(view, view_tx, command_rx, init.factory, init.model);
    tokio::spawn(actor.run());
    LiveSessionHandle {
        id: init.id,
        commands: command_tx,
        view: view_rx,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use forge_domain::{MessageRole, PermissionMode};
    use forge_ports::{SidecarError, SidecarEvent, StartSessionParams};

    use super::*;
    use crate::test_support::{factory_returning, fake_sidecar, wait_closed, wait_for, Call};

    fn empty_session() -> Session {
        Session {
            id: SessionId(String::new()),
            project_path: PathBuf::from("/work/a"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::AcceptEdits,
        }
    }

    fn new_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        LiveSessionInit {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: empty_session(),
            claude_session_id: None,
            model: Some("model-x".to_string()),
            can_send: true,
            factory,
        }
    }

    fn resumed_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        let mut session = empty_session();
        session.id = SessionId("claude-1".to_string());
        session.messages.push(forge_domain::Message {
            role: MessageRole::User,
            content: "earlier".to_string(),
            tool_calls: Vec::new(),
        });
        LiveSessionInit {
            claude_session_id: Some(SessionId("claude-1".to_string())),
            session,
            ..new_init(factory)
        }
    }

    fn start_params(resume: Option<&str>) -> StartSessionParams {
        StartSessionParams {
            cwd: "/work/a".to_string(),
            resume_session_id: resume.map(str::to_string),
            permission_mode: PermissionMode::AcceptEdits,
            model: Some("model-x".to_string()),
        }
    }

    fn permission_request(id: &str) -> SidecarEvent {
        SidecarEvent::PermissionRequest {
            permission_request_id: id.to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({ "command": "cargo test" }),
        }
    }

    fn delta(text: &str) -> SidecarEvent {
        SidecarEvent::AssistantDelta { text: text.to_string() }
    }

    fn turn_complete() -> SidecarEvent {
        SidecarEvent::TurnComplete { input_tokens: 1, output_tokens: 2 }
    }

    #[tokio::test]
    async fn opening_a_session_spawns_no_sidecar() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert_eq!(handle.snapshot().session.messages.len(), 1);
        assert_eq!(handle.snapshot().phase, Phase::Idle);
    }

    #[tokio::test]
    async fn the_first_send_spawns_a_sidecar_starts_the_session_then_sends_the_message() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]
        );
        assert_eq!(seen.session.messages.len(), 1);
        assert_eq!(seen.session.messages[0].role, MessageRole::User);
        assert_eq!(seen.session.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn resuming_an_existing_session_passes_its_id_as_the_resume_session_id() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.send("continue".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(Some("claude-1"))), Call::Send("continue".to_string())]
        );
        assert_eq!(seen.session.messages.len(), 2, "history plus the new message");
    }

    #[tokio::test]
    async fn session_started_records_the_claude_session_id() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(SidecarEvent::SessionStarted { session_id: "claude-9".to_string() });
        let seen = wait_for(&mut view, "the session id", |v| v.claude_session_id.is_some()).await;

        assert_eq!(seen.claude_session_id, Some(SessionId("claude-9".to_string())));
        assert_eq!(seen.session.id, SessionId("claude-9".to_string()));
    }

    #[tokio::test]
    async fn streamed_events_build_the_assistant_message_and_turn_complete_returns_to_idle() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(delta("Hel"));
        control.push(delta("lo"));
        control.push(SidecarEvent::ToolUse {
            tool_call_id: "t-1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({ "file_path": "a.rs" }),
        });
        control.push(SidecarEvent::ToolResult {
            tool_call_id: "t-1".to_string(),
            output: "contents".to_string(),
            is_error: false,
        });
        control.push(turn_complete());
        let seen = wait_for(&mut view, "the turn to complete", |v| {
            v.phase == Phase::Idle && v.session.messages.len() == 2
        })
        .await;

        let reply = &seen.session.messages[1];
        assert_eq!(reply.role, MessageRole::Assistant);
        assert_eq!(reply.content, "Hello");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].result.as_ref().unwrap().output, "contents");
    }

    #[tokio::test]
    async fn permission_requests_queue_in_order_and_approving_answers_the_sidecar_and_pops_it() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("run tests".to_string());
        control.push(permission_request("p-1"));
        control.push(permission_request("p-2"));
        let queued = wait_for(&mut view, "two queued requests", |v| v.pending_permissions.len() == 2).await;
        assert_eq!(queued.pending_permissions[0].id, "p-1");

        handle.approve("p-1".to_string(), PermissionDecision::AllowOnce);
        let after = wait_for(&mut view, "one request left", |v| v.pending_permissions.len() == 1).await;

        assert_eq!(after.pending_permissions[0].id, "p-2");
        assert!(control
            .calls()
            .contains(&Call::Permission("p-1".to_string(), PermissionDecision::AllowOnce)));
    }

    #[tokio::test]
    async fn approving_an_unknown_request_does_not_reach_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.approve("nope".to_string(), PermissionDecision::Deny);
        handle.interrupt(); // commands run in order, so once this lands the approval was handled
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        assert!(control.calls().contains(&Call::Interrupt));
        assert!(!control
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Permission(..))));
    }

    #[tokio::test]
    async fn interrupt_reaches_the_sidecar_and_is_a_no_op_without_one() {
        let (factory_idle, spawns) = factory_returning(vec![]);
        let idle = spawn_live_session(new_init(factory_idle));
        let mut idle_view = idle.subscribe();
        idle.interrupt();
        idle.close();
        wait_closed(&mut idle_view).await;
        assert_eq!(spawns.load(Ordering::SeqCst), 0);

        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.interrupt();
        handle.close();
        wait_closed(&mut view).await;

        assert!(control.calls().contains(&Call::Interrupt));
    }

    #[tokio::test]
    async fn an_error_event_shuts_the_sidecar_down_clears_permissions_and_fails_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(permission_request("p-1"));
        wait_for(&mut view, "a queued request", |v| !v.pending_permissions.is_empty()).await;
        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.last_error, Some("boom".to_string()));
        assert!(seen.pending_permissions.is_empty());
        assert!(control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn sending_after_a_failure_respawns_and_resumes_with_the_known_session_id() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::SessionStarted { session_id: "claude-1".to_string() });
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        let seen = wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(
            second_control.calls(),
            vec![Call::Start(start_params(Some("claude-1"))), Call::Send("two".to_string())]
        );
        assert_eq!(seen.last_error, None, "a successful send clears the error");
    }

    #[tokio::test]
    async fn sending_after_a_failure_that_came_before_any_session_id_starts_fresh() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(second_control.calls()[0], Call::Start(start_params(None)));
    }

    #[tokio::test]
    async fn a_spawn_failure_fails_the_session_and_leaves_the_history_untouched() {
        let (factory, spawns) = factory_returning(vec![Err(SidecarError::SpawnFailed("no node".to_string()))]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(seen.last_error.unwrap().contains("no node"));
        assert!(seen.session.messages.is_empty());
    }

    #[tokio::test]
    async fn a_start_session_failure_fails_the_session_and_never_sends_the_message() {
        let (sidecar, control) = fake_sidecar();
        control.fail_start();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("fake start failure"));
        assert!(seen.session.messages.is_empty());
        assert_eq!(control.calls(), vec![Call::Start(start_params(None))], "the message was never sent");
    }

    #[tokio::test]
    async fn a_failed_write_removes_the_undelivered_message_from_the_history() {
        let (sidecar, control) = fake_sidecar();
        control.fail_send();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.send("lost".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.session.messages.len(), 1, "only the earlier history remains");
        assert_eq!(seen.session.messages[0].content, "earlier");
    }

    #[tokio::test]
    async fn the_event_stream_ending_fails_the_session_without_asking_the_dead_process_to_shut_down() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.end_stream();
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("exited unexpectedly"));
        assert!(!control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn an_unexpected_session_ended_event_fails_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());

        control.push(SidecarEvent::SessionEnded);
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("ended"));
    }

    #[tokio::test]
    async fn sending_to_a_session_whose_folder_is_missing_sets_an_error_and_never_spawns() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(LiveSessionInit {
            can_send: false,
            ..resumed_init(factory)
        });
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the error", |v| v.last_error.is_some()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert_eq!(seen.phase, Phase::Idle);
        assert!(!seen.can_send);
        assert_eq!(seen.session.messages.len(), 1, "the history stays readable");
    }

    #[tokio::test]
    async fn close_shuts_the_sidecar_down_and_ends_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.close();
        wait_closed(&mut view).await;

        assert!(control.calls().contains(&Call::Shutdown));
        assert!(handle.is_closed());
    }

    #[tokio::test]
    async fn closing_without_a_sidecar_just_ends_the_session() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn commands_and_events_interleave_without_losing_either() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        for _ in 0..50 {
            control.push(delta("a"));
        }
        handle.send("two".to_string());
        for _ in 0..50 {
            control.push(delta("b"));
        }
        control.push(turn_complete());
        let seen = wait_for(&mut view, "both turns' events", |v| {
            v.phase == Phase::Idle && v.session.messages.iter().map(|m| m.content.len()).sum::<usize>() >= 106
        })
        .await;

        assert_eq!(
            control.calls(),
            vec![
                Call::Start(start_params(None)),
                Call::Send("one".to_string()),
                Call::Send("two".to_string()),
            ]
        );
        let total: usize = seen.session.messages.iter().map(|m| m.content.len()).sum();
        assert_eq!(total, "one".len() + "two".len() + 100);
    }

    #[tokio::test]
    async fn a_burst_of_events_is_applied_completely_but_published_only_a_few_times() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("go".to_string());
        for _ in 0..1000 {
            control.push(delta("x"));
        }
        control.push(turn_complete());

        let mut updates = 0;
        loop {
            match tokio::time::timeout(Duration::from_millis(300), view.changed()).await {
                Ok(Ok(())) => {
                    updates += 1;
                    if view.borrow().phase == Phase::Idle {
                        break;
                    }
                }
                _ => break,
            }
        }

        let last = handle.snapshot();
        assert_eq!(last.phase, Phase::Idle);
        assert_eq!(last.session.messages[1].content.len(), 1000);
        assert!(updates <= 5, "1001 events were published {updates} times");
    }
}
