use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use forge_domain::{Attachment, AvailableModel, ContextBreakdown, ContextUsage, McpServerState, EffortLevel, PermissionDecision, PermissionMode, PermissionRequest, Session, SessionId, SlashCommand};
use forge_ports::SidecarFactoryPort;
use tokio::sync::{mpsc, watch};

use crate::live_session_actor::Actor;
use crate::use_cases::ExtensionsUseCase;

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
    /// The model this session runs on; `None` is the SDK's default. Changes when the SDK accepts a
    /// change (or at once, while no sidecar is running).
    pub model: Option<String>,
    /// The effort of this session; `None` is the SDK's default. Changes when the SDK accepts a change
    /// (or at once, while no sidecar is running). A transcript does not record it, so a resumed
    /// session starts on `None`.
    pub effort: Option<EffortLevel>,
    /// The models the SDK listed for this session, with the effort levels each takes; empty until the
    /// sidecar has reported.
    pub available_models: Vec<AvailableModel>,
    pub phase: Phase,
    /// When the running turn was sent; `None` while no turn runs.
    pub turn_started: Option<std::time::SystemTime>,
    /// The background tasks the SDK reports as running, with when each was first seen.
    pub background_tasks: Vec<forge_domain::RunningTask>,
    /// The SDK's guess at the next prompt; cleared by a send.
    pub suggestion: Option<String>,
    /// Oldest first; the front is the one the UI shows.
    pub pending_permissions: VecDeque<PermissionRequest>,
    pub last_error: Option<String>,
    /// `false` when the project folder no longer exists: the history is readable only.
    pub can_send: bool,
    /// The text of the last message that could not be delivered, so the UI can put it back in the
    /// message box. Cleared by the next send that succeeds.
    pub unsent: Option<String>,
    /// The files and images of that message, so the UI can attach them again.
    pub unsent_attachments: Vec<Attachment>,
    /// How many sends have failed so far. The UI restores `unsent` when this number changes.
    pub failed_sends: u32,
    /// How full the context window was after the last finished turn. `None` until a turn has
    /// finished in this run: the sidecar starts on the first send, so an opened-but-idle session
    /// has no value yet.
    pub context: Option<ContextUsage>,
    /// The last non-empty context breakdown; a restarted sidecar leaves it until a new one arrives.
    pub context_detail: Option<ContextBreakdown>,
    /// The last MCP server states; `Some(vec![])` is "no servers", `None` is "not reported yet".
    pub mcp_servers: Option<Vec<McpServerState>>,
    /// The slash commands the SDK reported; empty until the first report.
    pub commands: Vec<SlashCommand>,
    /// The rules of every request the person answered "always allow" in this session, in order.
    pub allowed_rules: Vec<String>,
    /// Messages the person asked to send once the running turn ends, oldest first. The session
    /// sends the front one when a turn ends.
    pub queued: Vec<QueuedMessage>,
    /// The account's usage limit state as the SDK last reported it.
    pub rate_limit: Option<forge_domain::RateLimit>,
    /// The `/btw` questions and answers, oldest first; they are not part of the session.
    pub side_thread: Vec<SideQuestion>,
    /// The `/rewind` the person is looking at: a preview of what would change, then the result.
    pub rewind: Option<RewindState>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RewindState {
    pub request_id: String,
    /// The user message the files go back to (they are put as they were before it).
    pub message_uuid: String,
    /// `true`: only asked what would change.
    pub dry_run: bool,
    /// `None` until the SDK has answered.
    pub outcome: Option<forge_domain::RewindOutcome>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SideQuestion {
    pub id: String,
    pub question: String,
    pub answer: SideAnswer,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SideAnswer {
    Pending,
    Done(String),
    /// Why there is no answer (empty when the SDK gave none).
    Failed(String),
}

/// A message waiting for the running turn to end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedMessage {
    pub text: String,
    pub attachments: Vec<Attachment>,
}

#[derive(Debug)]
pub enum LiveCommand {
    Send { text: String, attachments: Vec<Attachment> },
    /// Keeps the message until the running turn ends; with no turn running it is sent at once.
    Queue { text: String, attachments: Vec<Attachment> },
    /// Takes the queued message at `index` out; `send_now` sends it into the running turn instead
    /// of dropping it.
    Dequeue { index: usize, send_now: bool },
    /// Asks about the conversation without adding the question to it (`/btw`).
    AskSide { question: String },
    /// Forgets the `/btw` thread.
    DismissSide,
    /// Asks the SDK to put the files back as they were before the user message (`dry_run`: only
    /// to say what would change).
    Rewind { message_uuid: String, dry_run: bool },
    CloseRewind,
    AddDirectories(Vec<PathBuf>),
    /// Makes the folder the session's working folder from the next start on.
    MoveTo(PathBuf),
    /// Stops the sidecar of an idle session; answers whether it did (see `LiveSessionHandle::suspend`).
    Suspend(tokio::sync::oneshot::Sender<bool>),
    /// Permission rules the next start allows without asking.
    AllowTools(Vec<String>),
    /// Variables the next start runs with.
    SetEnv(forge_domain::EnvVars),
    WarmUp,
    Approve { id: String, decision: PermissionDecision, reason: Option<String> },
    AnswerQuestion { id: String, answers: std::collections::BTreeMap<String, String> },
    StopTask { task_id: String },
    Interrupt,
    SetTitle(String),
    SetMode(PermissionMode),
    SetModel(Option<String>),
    SetEffort(Option<EffortLevel>),
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
        self.send_with(text, Vec::new());
    }

    /// Sends a message with files and images attached.
    pub fn send_with(&self, text: String, attachments: Vec<Attachment>) {
        let _ = self.commands.send(LiveCommand::Send { text, attachments });
    }

    /// Holds a message until the running turn ends (see `LiveCommand::Queue`).
    pub fn queue(&self, text: String, attachments: Vec<Attachment>) {
        let _ = self.commands.send(LiveCommand::Queue { text, attachments });
    }

    /// `/btw`: a question about the conversation that is not added to it.
    pub fn ask_side(&self, question: String) {
        let _ = self.commands.send(LiveCommand::AskSide { question });
    }

    /// `/rewind`: previews (`dry_run`) or does putting the files back as they were before a message.
    pub fn rewind_files(&self, message_uuid: String, dry_run: bool) {
        let _ = self.commands.send(LiveCommand::Rewind { message_uuid, dry_run });
    }

    pub fn close_rewind(&self) {
        let _ = self.commands.send(LiveCommand::CloseRewind);
    }

    pub fn dismiss_side(&self) {
        let _ = self.commands.send(LiveCommand::DismissSide);
    }

    /// Removes the queued message at `index` (`send_now`: and sends it now).
    pub fn dequeue(&self, index: usize, send_now: bool) {
        let _ = self.commands.send(LiveCommand::Dequeue { index, send_now });
    }

    /// Folders the session may use besides its own. The SDK reads them at start, so a running
    /// sidecar is dropped and the next send starts one with the longer list (resuming the session).
    /// While a turn runs the call is ignored.
    pub fn add_directories(&self, directories: Vec<PathBuf>) {
        let _ = self.commands.send(LiveCommand::AddDirectories(directories));
    }

    /// Moves the session to another working folder; the next send resumes it there. Ignored while a
    /// sidecar is running a turn or background tasks; after `suspend` it always applies.
    pub fn move_to(&self, folder: PathBuf) {
        let _ = self.commands.send(LiveCommand::MoveTo(folder));
    }

    /// Stops the sidecar so nothing writes to the transcript any more; the next send resumes the
    /// session on a new one. False (and nothing done) while a turn or a background task runs, or
    /// when the session is gone.
    pub async fn suspend(&self) -> bool {
        let (answer, answered) = tokio::sync::oneshot::channel();
        if self.commands.send(LiveCommand::Suspend(answer)).is_err() {
            return false;
        }
        answered.await.unwrap_or(false)
    }

    /// Permission rules the next start allows without asking (the SDK reads them once, at start).
    pub fn allow_tools(&self, tools: Vec<String>) {
        let _ = self.commands.send(LiveCommand::AllowTools(tools));
    }

    /// Variables the session's process runs with (the SDK reads them once, at start). Like folders,
    /// only a session whose sidecar has not started takes them.
    pub fn set_env(&self, env: forge_domain::EnvVars) {
        let _ = self.commands.send(LiveCommand::SetEnv(env));
    }

    /// Starts the session's sidecar now instead of at the first send, so its slash commands are known
    /// before the person types. A failure is not shown: the first send starts the sidecar again.
    pub fn warm_up(&self) {
        let _ = self.commands.send(LiveCommand::WarmUp);
    }

    pub fn approve(&self, permission_request_id: String, decision: PermissionDecision) {
        let _ = self.commands.send(LiveCommand::Approve {
            id: permission_request_id,
            decision,
            reason: None,
        });
    }

    /// Refuses a request and tells the model why (the sidecar uses `reason` as the deny message).
    pub fn refuse_with_reason(&self, permission_request_id: String, reason: Option<String>) {
        let _ = self.commands.send(LiveCommand::Approve {
            id: permission_request_id,
            decision: PermissionDecision::Deny,
            reason,
        });
    }

    /// Answers an `AskUserQuestion` request (see `SidecarPort::answer_question`).
    pub fn answer_question(&self, permission_request_id: String, answers: std::collections::BTreeMap<String, String>) {
        let _ = self.commands.send(LiveCommand::AnswerQuestion { id: permission_request_id, answers });
    }

    /// Stops one background task (see `SidecarPort::stop_task`).
    pub fn stop_task(&self, task_id: String) {
        let _ = self.commands.send(LiveCommand::StopTask { task_id });
    }

    /// Shows `title` as the session's title. Called after the new title is stored, so the view
    /// never claims a name the transcript does not have.
    pub fn set_title(&self, title: String) {
        let _ = self.commands.send(LiveCommand::SetTitle(title));
    }

    pub fn interrupt(&self) {
        let _ = self.commands.send(LiveCommand::Interrupt);
    }

    /// Changes the permission mode from the next turn on. With a sidecar running the view follows
    /// once the SDK accepts it; without one the view changes now and the next start uses it.
    pub fn set_permission_mode(&self, mode: PermissionMode) {
        let _ = self.commands.send(LiveCommand::SetMode(mode));
    }

    /// Changes the model (`None`: the default) the same way.
    pub fn set_model(&self, model: Option<String>) {
        let _ = self.commands.send(LiveCommand::SetModel(model));
    }

    /// Changes the effort (`None`: the default) the same way.
    pub fn set_effort(&self, effort: Option<EffortLevel>) {
        let _ = self.commands.send(LiveCommand::SetEffort(effort));
    }

    pub fn close(&self) {
        let _ = self.commands.send(LiveCommand::Close);
    }

    /// A receiver that wakes on every published change and reports `Err` once the session is closed.
    pub fn subscribe(&self) -> watch::Receiver<LiveSessionView> {
        self.view.clone()
    }

    /// Reads the current state in place, without copying the whole transcript the way `snapshot` does. Keep
    /// `read` short: the session's task cannot publish while it runs.
    pub fn with_view<R>(&self, read: impl FnOnce(&LiveSessionView) -> R) -> R {
        read(&self.view.borrow())
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
    pub effort: Option<EffortLevel>,
    pub can_send: bool,
    pub commands: Vec<SlashCommand>,
    pub factory: Arc<dyn SidecarFactoryPort>,
    pub extensions: Arc<ExtensionsUseCase>,
}

/// Starts the session's task. Must be called from inside a tokio runtime.
pub(crate) fn spawn_live_session(init: LiveSessionInit) -> LiveSessionHandle {
    let view = LiveSessionView {
        id: init.id,
        project_path: init.project_path,
        session: init.session,
        claude_session_id: init.claude_session_id,
        model: init.model.clone(),
        effort: init.effort,
        available_models: Vec::new(),
        phase: Phase::Idle,
        turn_started: None,
        background_tasks: Vec::new(),
        suggestion: None,
        pending_permissions: VecDeque::new(),
        last_error: None,
        can_send: init.can_send,
        unsent: None,
        unsent_attachments: Vec::new(),
        failed_sends: 0,
        context: None,
        context_detail: None,
        mcp_servers: None,
        commands: init.commands,
        allowed_rules: Vec::new(),
        queued: Vec::new(),
        rate_limit: None,
        side_thread: Vec::new(),
        rewind: None,
    };
    let (view_tx, view_rx) = watch::channel(view.clone());
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let actor = Actor::new(view, view_tx, command_rx, init.factory, init.model, init.effort, init.extensions);
    tokio::spawn(actor.run());
    LiveSessionHandle {
        id: init.id,
        commands: command_tx,
        view: view_rx,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use forge_domain::{ActiveExtensions, AppConfig, ContextUsage, MessageRole, OutputStyle, PermissionMode, SessionDefaults, SkillDefinition, TokenUsage};
    use forge_ports::{ContextCategory, ContextItemReport, McpServerReport, SidecarError, SidecarEvent, StartSessionParams};

    use super::*;
    use crate::test_support::{
        counting_extensions, extensions_changing_config, extensions_with, factory_returning, failing_extensions, fake_sidecar, no_extensions, wait_closed, wait_for, Call,
    };

    fn empty_session() -> Session {
        Session {
            finished_tasks: Vec::new(),
            id: SessionId(String::new()),
            project_path: PathBuf::from("/work/a"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::AcceptEdits,
            usage: forge_domain::SessionUsage::default(),
        }
    }

    fn new_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        LiveSessionInit {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: empty_session(),
            claude_session_id: None,
            model: Some("model-x".to_string()),
            effort: None,
            can_send: true,
            commands: Vec::new(),
            factory,
            extensions: no_extensions(),
        }
    }

    fn resumed_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        let mut session = empty_session();
        session.id = SessionId("claude-1".to_string());
        session.messages.push(forge_domain::Message::user("earlier"));
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
            effort: None,
            extensions: ActiveExtensions::default(),
            additional_directories: Vec::new(),
            allowed_tools: Vec::new(),
            env: forge_domain::EnvVars::default(),
            defaults: SessionDefaults::default(),
        }
    }

    fn code_tab_config() -> AppConfig {
        AppConfig {
            output_style: Some(OutputStyle::Concise),
            prompt_suggestions: false,
            allow_bypass_mode: true,
            dynamic_workflows: true,
            ..AppConfig::default()
        }
    }

    fn started_defaults(calls: Vec<Call>) -> SessionDefaults {
        match calls.into_iter().next() {
            Some(Call::Start(params)) => params.defaults,
            other => panic!("expected a start, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn the_first_start_carries_the_code_tab_defaults_from_the_config() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = new_init(factory);
        init.extensions = extensions_with(code_tab_config(), Vec::new());
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(
            started_defaults(control.calls()),
            SessionDefaults { output_style: Some(OutputStyle::Concise), prompt_suggestions: false, allow_bypass: true, dynamic_workflows: true }
        );
    }

    #[tokio::test]
    async fn a_restarted_session_keeps_the_defaults_it_first_started_with() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let mut init = new_init(factory);
        // The person turns everything back to the stock values while the session is open.
        init.extensions = extensions_changing_config(code_tab_config(), AppConfig::default());
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        let first_defaults = started_defaults(first_control.calls());
        assert_eq!(first_defaults.output_style, Some(OutputStyle::Concise));
        assert_eq!(started_defaults(second_control.calls()), first_defaults);
    }

    #[tokio::test]
    async fn a_restart_after_leaving_bypass_caches_the_configured_switch_not_the_or_ed_one() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let mut init = resumed_init(factory);
        init.session.permission_mode = PermissionMode::BypassPermissions;
        // The default config: the switch is off.
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        wait_for(&mut view, "the first turn", |v| v.phase == Phase::Running).await;
        assert!(started_defaults(first_control.calls()).allow_bypass, "bypass is allowed while the session is in it");

        handle.set_permission_mode(PermissionMode::AcceptEdits);
        first_control.push(SidecarEvent::PermissionModeSet { mode: PermissionMode::AcceptEdits });
        wait_for(&mut view, "the mode change", |v| v.session.permission_mode == PermissionMode::AcceptEdits).await;
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert!(!started_defaults(second_control.calls()).allow_bypass, "the cached value is the configured one");
    }

    #[tokio::test]
    async fn a_failed_start_caches_nothing_so_the_next_start_reads_the_config_again() {
        let (first, first_control) = fake_sidecar();
        first_control.fail_start();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let mut init = new_init(factory);
        init.extensions = extensions_changing_config(code_tab_config(), AppConfig::default());
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(started_defaults(first_control.calls()).output_style, Some(OutputStyle::Concise));
        assert_eq!(started_defaults(second_control.calls()), SessionDefaults::default(), "the second config, not the first");
    }

    #[tokio::test]
    async fn a_session_already_in_bypass_mode_may_start_with_the_switch_off() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = resumed_init(factory);
        init.session.permission_mode = PermissionMode::BypassPermissions;
        // The default config: the switch is off.
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert!(started_defaults(control.calls()).allow_bypass, "the SDK refuses the mode unless it is allowed up front");
    }

    #[tokio::test]
    async fn a_session_in_another_mode_does_not_get_bypass_when_the_switch_is_off() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert!(!started_defaults(control.calls()).allow_bypass);
    }

    fn permission_request(id: &str) -> SidecarEvent {
        SidecarEvent::PermissionRequest {
            permission_request_id: id.to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({ "command": "cargo test" }),
            rules: Vec::new(),
        }
    }

    fn delta(text: &str) -> SidecarEvent {
        SidecarEvent::AssistantDelta { text: text.to_string() }
    }

    fn turn_complete() -> SidecarEvent {
        SidecarEvent::TurnComplete { usage: BTreeMap::new() }
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
        assert_eq!(seen.session.messages[0].text(), "hello");
    }

    #[tokio::test]
    async fn a_message_with_attachments_reaches_the_sidecar_and_the_history() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        let attachment = forge_domain::Attachment { path: "/x/a b.png".into(), kind: forge_domain::AttachmentKind::Image };

        handle.send_with("look".to_string(), vec![attachment.clone()]);
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::SendAttached("look".to_string(), vec![attachment.clone()])]
        );
        assert_eq!(seen.session.messages[0].attachments, vec![attachment]);
    }

    #[tokio::test]
    async fn added_directories_go_to_the_first_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.add_directories(vec!["/extra".into(), "/extra".into()]);
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.additional_directories = vec!["/extra".to_string()];
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }

    #[tokio::test]
    async fn with_view_reads_the_current_state_without_copying_it() {
        let (sidecar, _control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let (phase, messages) = handle.with_view(|view| (view.phase, view.session.messages.len()));
        assert_eq!((phase, messages), (Phase::Idle, 0));
        assert_eq!(handle.with_view(|view| view.project_path.clone()), handle.snapshot().project_path);
    }

    #[tokio::test]
    async fn env_goes_to_the_first_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.set_env(forge_domain::EnvVars(vec![("A".to_string(), "1".to_string())]));
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.env = forge_domain::EnvVars(vec![("A".to_string(), "1".to_string())]);
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }

    #[tokio::test]
    async fn allowed_tools_go_to_the_first_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.allow_tools(vec!["Read".to_string(), "Read".to_string()]);
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.allowed_tools = vec!["Read".to_string()];
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }

    #[tokio::test]
    async fn an_always_allow_answer_adds_the_requests_rules_to_the_view_and_a_single_allow_does_not() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("run tests".to_string());
        control.push(SidecarEvent::PermissionRequest {
            permission_request_id: "p1".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({ "command": "cargo test" }),
            rules: vec!["Bash(cargo test:*)".to_string()],
        });
        wait_for(&mut view, "one queued request", |v| v.pending_permissions.len() == 1).await;
        handle.approve("p1".to_string(), PermissionDecision::AllowAlways);
        let after = wait_for(&mut view, "the request answered", |v| v.pending_permissions.is_empty()).await;
        assert_eq!(after.allowed_rules, vec!["Bash(cargo test:*)".to_string()]);

        control.push(SidecarEvent::PermissionRequest {
            permission_request_id: "p2".to_string(),
            tool_name: "Write".to_string(),
            input: serde_json::json!({}),
            rules: vec!["Write".to_string()],
        });
        wait_for(&mut view, "the second request", |v| v.pending_permissions.len() == 1).await;
        handle.approve("p2".to_string(), PermissionDecision::AllowOnce);
        let after = wait_for(&mut view, "the second answered", |v| v.pending_permissions.is_empty()).await;
        assert_eq!(after.allowed_rules, vec!["Bash(cargo test:*)".to_string()], "a single allow adds nothing");
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
    async fn a_session_shows_the_model_it_was_opened_with() {
        let (factory, _spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(new_init(factory));

        assert_eq!(handle.snapshot().model, Some("model-x".to_string()));
    }

    #[tokio::test]
    async fn before_a_sidecar_exists_a_mode_and_model_change_apply_at_once_and_to_the_next_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.set_permission_mode(PermissionMode::Plan);
        handle.set_model(None);
        let seen = wait_for(&mut view, "the changes", |v| v.session.permission_mode == PermissionMode::Plan && v.model.is_none()).await;
        assert_eq!(seen.model, None);

        handle.send("go".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let started = control.calls().into_iter().find_map(|call| match call {
            Call::Start(params) => Some(params),
            _ => None,
        });
        let started = started.expect("a sidecar was started");
        assert_eq!(started.permission_mode, PermissionMode::Plan);
        assert_eq!(started.model, None);
    }

    #[tokio::test]
    async fn with_a_sidecar_the_change_is_sent_and_shown_only_when_the_sidecar_answers() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.set_permission_mode(PermissionMode::Auto);
        handle.set_model(Some("model-y".to_string()));
        // Both were forwarded, and neither is shown yet: the SDK has not said it accepted them.
        for _ in 0..50 {
            if control.calls().len() >= 4 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let calls = control.calls();
        assert!(calls.contains(&Call::SetMode(PermissionMode::Auto)));
        assert!(calls.contains(&Call::SetModel(Some("model-y".to_string()))));
        assert_eq!(handle.snapshot().session.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(handle.snapshot().model, Some("model-x".to_string()));

        control.push(SidecarEvent::PermissionModeSet { mode: PermissionMode::Auto });
        control.push(SidecarEvent::ModelSet { model: Some("model-y".to_string()) });
        let seen = wait_for(&mut view, "the answers", |v| {
            v.session.permission_mode == PermissionMode::Auto && v.model.as_deref() == Some("model-y")
        })
        .await;
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn a_refused_change_shows_a_message_keeps_the_old_value_and_the_session_goes_on() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.set_permission_mode(PermissionMode::Auto);
        control.push(SidecarEvent::SettingFailed { message: "auto is not available".to_string() });
        let seen = wait_for(&mut view, "the message", |v| v.last_error.is_some()).await;

        assert_eq!(seen.last_error.as_deref(), Some("auto is not available"));
        assert_eq!(seen.session.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(seen.phase, Phase::Running, "a refusal does not end the turn or the session");
    }

    #[tokio::test]
    async fn a_refusal_message_goes_away_when_a_later_change_is_accepted() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::SettingFailed { message: "no such model".to_string() });
        wait_for(&mut view, "the refusal", |v| v.last_error.is_some()).await;
        control.push(SidecarEvent::ModelSet { model: Some("model-y".to_string()) });
        let seen = wait_for(&mut view, "the change", |v| v.model.as_deref() == Some("model-y")).await;

        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn before_a_sidecar_exists_an_effort_change_applies_at_once_and_to_the_next_start() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.set_effort(Some(forge_domain::EffortLevel::High));
        wait_for(&mut view, "the change", |v| v.effort == Some(forge_domain::EffortLevel::High)).await;
        handle.send("go".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let started = control.calls().into_iter().find_map(|call| match call {
            Call::Start(params) => Some(params),
            _ => None,
        });
        assert_eq!(started.expect("a sidecar was started").effort, Some(forge_domain::EffortLevel::High));
    }

    #[tokio::test]
    async fn with_a_sidecar_the_effort_is_sent_and_shown_only_when_the_sidecar_answers() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.set_effort(Some(forge_domain::EffortLevel::Max));
        for _ in 0..50 {
            if control.calls().contains(&Call::SetEffort(Some(forge_domain::EffortLevel::Max))) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(control.calls().contains(&Call::SetEffort(Some(forge_domain::EffortLevel::Max))));
        assert_eq!(handle.snapshot().effort, None, "not shown before the answer");

        control.push(SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::Max) });
        let seen = wait_for(&mut view, "the answer", |v| v.effort == Some(forge_domain::EffortLevel::Max)).await;
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn a_refused_effort_change_keeps_the_old_level_and_the_refusal_goes_away_when_a_change_is_accepted() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::SettingFailed { message: "capped".to_string() });
        let refused = wait_for(&mut view, "the refusal", |v| v.last_error.is_some()).await;
        assert_eq!(refused.effort, None);
        assert_eq!(refused.phase, Phase::Running);

        control.push(SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::Low) });
        let seen = wait_for(&mut view, "the change", |v| v.effort == Some(forge_domain::EffortLevel::Low)).await;
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn the_models_the_sdk_lists_are_kept_in_the_view() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let models = vec![forge_domain::AvailableModel {
            value: "model-x".to_string(), resolved_model: None,
            display_name: "X".to_string(),
            effort_levels: vec![forge_domain::EffortLevel::Low, forge_domain::EffortLevel::High],
        }];
        control.push(SidecarEvent::ModelsAvailable { models: models.clone() });
        let seen = wait_for(&mut view, "the list", |v| !v.available_models.is_empty()).await;

        assert_eq!(seen.available_models, models);
    }

    #[tokio::test]
    async fn an_accepted_change_leaves_a_message_that_is_not_a_refusal() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::SettingFailed { message: "no such model".to_string() });
        wait_for(&mut view, "the refusal", |v| v.last_error.is_some()).await;
        // A later send replaces the banner with its own state; a change accepted after that must
        // not clear something else.
        control.push(turn_complete());
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        handle.send("again".to_string());
        wait_for(&mut view, "the banner to clear", |v| v.phase == Phase::Running && v.last_error.is_none()).await;
        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let failed = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(failed.last_error.as_deref(), Some("boom"));
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
        assert_eq!(reply.text(), "Hello");
        assert_eq!(reply.tool_calls().count(), 1);
        assert_eq!(reply.tool_calls().nth(0).unwrap().result.as_ref().unwrap().output, "contents");
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
            .contains(&Call::Permission("p-1".to_string(), PermissionDecision::AllowOnce, None)));
    }

    #[tokio::test]
    async fn refuse_with_reason_reaches_the_sidecar_as_a_deny_with_the_reason() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("plan it".to_string());
        control.push(permission_request("p-1"));
        wait_for(&mut view, "the request", |v| v.pending_permissions.len() == 1).await;

        handle.refuse_with_reason("p-1".to_string(), Some("add tests".to_string()));
        wait_for(&mut view, "the request gone", |v| v.pending_permissions.is_empty()).await;

        assert!(control.calls().contains(&Call::Permission(
            "p-1".to_string(),
            PermissionDecision::Deny,
            Some("add tests".to_string())
        )));
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

    fn question_request(id: &str) -> SidecarEvent {
        SidecarEvent::PermissionRequest {
            permission_request_id: id.to_string(),
            tool_name: "AskUserQuestion".to_string(),
            input: serde_json::json!({ "questions": [] }),
            rules: Vec::new(),
        }
    }

    #[tokio::test]
    async fn answering_a_question_pops_it_and_reaches_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("ask me".to_string());
        control.push(question_request("q-1"));
        wait_for(&mut view, "the question", |v| v.pending_permissions.len() == 1).await;

        let answers = BTreeMap::from([("Q?".to_string(), "A".to_string())]);
        handle.answer_question("q-1".to_string(), answers.clone());
        wait_for(&mut view, "the question answered", |v| v.pending_permissions.is_empty()).await;

        assert!(control.calls().contains(&Call::Answer("q-1".to_string(), answers)));
    }

    #[tokio::test]
    async fn answering_an_unknown_question_does_not_reach_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.answer_question("nope".to_string(), BTreeMap::new());
        handle.interrupt(); // commands run in order
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(!control.calls().iter().any(|call| matches!(call, Call::Answer(..))));
    }

    #[tokio::test]
    async fn the_turn_clock_starts_on_send_and_stops_when_the_turn_ends() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert!(handle.snapshot().turn_started.is_none());

        handle.send("hi".to_string());
        let running = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        assert!(running.turn_started.is_some());

        control.push(turn_complete());
        let idle = wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        assert!(idle.turn_started.is_none());
    }

    #[tokio::test]
    async fn a_queued_message_is_sent_when_the_turn_ends_and_only_then() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("first".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        handle.queue("second".to_string(), Vec::new());
        handle.queue("third".to_string(), Vec::new());
        let queued = wait_for(&mut view, "two queued", |v| v.queued.len() == 2).await;
        assert_eq!(queued.queued[0].text, "second");
        assert!(!control.calls().contains(&Call::Send("second".to_string())), "nothing is sent while the turn runs");

        control.push(turn_complete());
        let after = wait_for(&mut view, "the front one to go out", |v| v.queued.len() == 1).await;
        assert_eq!(after.queued[0].text, "third");
        assert_eq!(after.phase, Phase::Running, "the sent one starts a turn of its own");
        assert!(control.calls().contains(&Call::Send("second".to_string())));
        assert!(!control.calls().contains(&Call::Send("third".to_string())));
    }

    #[tokio::test]
    async fn a_side_question_is_asked_without_a_turn_and_its_answer_fills_the_card() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.ask_side("what was the number?".to_string());
        let asked = wait_for(&mut view, "the question to show", |v| !v.side_thread.is_empty()).await;
        let side = asked.side_thread[0].clone();
        assert_eq!(side.question, "what was the number?");
        assert_eq!(side.answer, SideAnswer::Pending);
        assert_eq!(asked.phase, Phase::Idle, "a side question starts no turn");
        assert!(asked.session.messages.is_empty(), "and adds nothing to the session");
        assert!(control.calls().contains(&Call::SideQuestion(side.id.clone(), "what was the number?".to_string())));

        control.push(SidecarEvent::SideAnswer { request_id: "stale".to_string(), answer: Some("no".to_string()), error: None });
        control.push(SidecarEvent::SideAnswer { request_id: side.id.clone(), answer: Some("41".to_string()), error: None });
        let answered = wait_for(&mut view, "the answer", |v| v.side_thread.first().is_some_and(|q| q.answer != SideAnswer::Pending)).await;
        assert_eq!(answered.side_thread[0].answer, SideAnswer::Done("41".to_string()));

        handle.dismiss_side();
        wait_for(&mut view, "the thread to go", |v| v.side_thread.is_empty()).await;
    }

    #[tokio::test]
    async fn a_follow_up_carries_the_earlier_questions_and_answers_to_the_sdk() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.ask_side("first?".to_string());
        let asked = wait_for(&mut view, "the first question", |v| !v.side_thread.is_empty()).await;
        control.push(SidecarEvent::SideAnswer { request_id: asked.side_thread[0].id.clone(), answer: Some("one".to_string()), error: None });
        wait_for(&mut view, "the first answer", |v| v.side_thread[0].answer == SideAnswer::Done("one".to_string())).await;

        handle.ask_side("and then?".to_string());
        let two = wait_for(&mut view, "the second question", |v| v.side_thread.len() == 2).await;
        assert_eq!(two.side_thread[1].question, "and then?", "the thread shows what the person typed");
        let sent = control.calls().into_iter().filter_map(|call| if let Call::SideQuestion(_, text) = call { Some(text) } else { None }).collect::<Vec<_>>();
        assert_eq!(sent[0], "first?");
        assert!(sent[1].contains("first?") && sent[1].contains("one") && sent[1].ends_with("and then?"), "{}", sent[1]);
    }

    #[tokio::test]
    async fn a_side_question_that_fails_says_why() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.ask_side("q".to_string());
        let asked = wait_for(&mut view, "the question to show", |v| !v.side_thread.is_empty()).await;
        let id = asked.side_thread[0].id.clone();
        control.push(SidecarEvent::SideAnswer { request_id: id, answer: None, error: Some("boom".to_string()) });
        let failed = wait_for(&mut view, "the failure", |v| v.side_thread.first().is_some_and(|q| q.answer != SideAnswer::Pending)).await;
        assert_eq!(failed.side_thread[0].answer, SideAnswer::Failed("boom".to_string()));
    }

    #[tokio::test]
    async fn a_generated_title_names_an_untitled_session_and_never_replaces_a_title() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::SessionTitle { title: "A short name".to_string() });
        let named = wait_for(&mut view, "the title", |v| v.session.title.is_some()).await;
        assert_eq!(named.session.title.as_deref(), Some("A short name"));

        control.push(SidecarEvent::SessionTitle { title: "Another".to_string() });
        control.push(SidecarEvent::UserMessage { uuid: "u".to_string() });
        wait_for(&mut view, "a later event", |v| v.session.messages.first().is_some_and(|m| m.uuid.is_some())).await;
        assert_eq!(handle.snapshot().session.title.as_deref(), Some("A short name"));
    }

    #[tokio::test]
    async fn rewinding_previews_then_applies_and_the_answer_fills_the_state() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.rewind_files("m-1".to_string(), true);
        let asked = wait_for(&mut view, "the preview to be asked", |v| v.rewind.is_some()).await;
        let state = asked.rewind.unwrap();
        assert!(state.dry_run && state.outcome.is_none());
        assert!(control.calls().contains(&Call::Rewind(state.request_id.clone(), "m-1".to_string(), true)));

        control.push(SidecarEvent::RewindResult { request_id: "stale".to_string(), can_rewind: false, error: None, files_changed: vec![], insertions: 0, deletions: 0 });
        control.push(SidecarEvent::RewindResult {
            request_id: state.request_id.clone(),
            can_rewind: true,
            error: None,
            files_changed: vec!["/a/b.txt".to_string()],
            insertions: 1,
            deletions: 2,
        });
        let previewed = wait_for(&mut view, "the preview", |v| matches!(&v.rewind, Some(r) if r.outcome.is_some())).await;
        let outcome = previewed.rewind.unwrap().outcome.unwrap();
        assert!(outcome.can_rewind);
        assert_eq!((outcome.files, outcome.insertions, outcome.deletions), (vec!["/a/b.txt".to_string()], 1, 2));

        handle.rewind_files("m-1".to_string(), false);
        let applying = wait_for(&mut view, "the real rewind to be asked", |v| matches!(&v.rewind, Some(r) if !r.dry_run)).await;
        assert!(applying.rewind.unwrap().outcome.is_none(), "the old preview is gone");

        handle.close_rewind();
        wait_for(&mut view, "the state to go", |v| v.rewind.is_none()).await;
    }

    #[tokio::test]
    async fn a_user_message_gets_the_id_the_sidecar_announces() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        control.push(SidecarEvent::UserMessage { uuid: "u-1".to_string() });
        let seen = wait_for(&mut view, "the id", |v| v.session.messages.first().is_some_and(|m| m.uuid.is_some())).await;
        assert_eq!(seen.session.messages[0].uuid.as_deref(), Some("u-1"));
    }

    #[tokio::test]
    async fn queueing_while_idle_sends_at_once() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.queue("now".to_string(), Vec::new());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        assert!(control.calls().contains(&Call::Send("now".to_string())));
        assert!(handle.snapshot().queued.is_empty());
    }

    #[tokio::test]
    async fn a_queued_message_can_be_removed_or_sent_now() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("first".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        handle.queue("a".to_string(), Vec::new());
        handle.queue("b".to_string(), Vec::new());
        handle.queue("c".to_string(), Vec::new());
        wait_for(&mut view, "three queued", |v| v.queued.len() == 3).await;

        handle.dequeue(0, false);
        handle.dequeue(1, true); // "c" now, after "a" was removed
        let after = wait_for(&mut view, "two taken out", |v| v.queued.len() == 1).await;
        assert_eq!(after.queued[0].text, "b");
        assert!(control.calls().contains(&Call::Send("c".to_string())));
        assert!(!control.calls().contains(&Call::Send("a".to_string())));
        handle.dequeue(9, false); // out of range: nothing happens
        assert_eq!(handle.snapshot().queued.len(), 1);
    }

    fn task(id: &str) -> forge_domain::BackgroundTaskInfo {
        forge_domain::BackgroundTaskInfo { id: id.to_string(), description: format!("task {id}"), task_type: "local_bash".to_string() }
    }

    #[tokio::test]
    async fn stop_task_reaches_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.stop_task("b1".to_string());
        handle.interrupt(); // commands run in order
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(control.calls().contains(&Call::StopTask("b1".to_string())));
    }

    #[tokio::test]
    async fn the_task_set_is_replaced_and_a_staying_task_keeps_its_first_seen_time() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        let first = wait_for(&mut view, "one task", |v| v.background_tasks.len() == 1).await;
        let seen = first.background_tasks[0].seen_at;

        tokio::time::sleep(Duration::from_millis(20)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1"), task("b2")] });
        let second = wait_for(&mut view, "two tasks", |v| v.background_tasks.len() == 2).await;
        assert_eq!(second.background_tasks[0].seen_at, seen, "b1 keeps its time");
        assert!(second.background_tasks[1].seen_at > seen, "b2 is new");

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![] });
        wait_for(&mut view, "no tasks", |v| v.background_tasks.is_empty()).await;
    }

    #[tokio::test]
    async fn a_running_tasks_tokens_survive_a_new_task_set() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 1234 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(1234)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1"), task("b2")] });
        let second = wait_for(&mut view, "two tasks", |v| v.background_tasks.len() == 2).await;
        assert_eq!(second.background_tasks[0].tokens, Some(1234));
        assert_eq!(second.background_tasks[1].tokens, None);
    }

    #[tokio::test]
    async fn a_finished_task_gets_the_tokens_and_time_its_notification_left_out() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 500 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(500)).await;
        control.push(SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None });
        let done = wait_for(&mut view, "finished", |v| v.session.finished_tasks.len() == 1 && v.session.finished_tasks[0].tokens.is_some()).await;
        assert_eq!(done.session.finished_tasks[0].tokens, Some(500));
        assert!(done.session.finished_tasks[0].duration_ms.is_some());
    }

    #[tokio::test]
    async fn the_task_set_may_drop_a_task_before_its_notification_arrives() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        control.push(SidecarEvent::TaskProgress { task_id: "b1".to_string(), total_tokens: 500 });
        wait_for(&mut view, "tokens", |v| v.background_tasks.first().and_then(|t| t.tokens) == Some(500)).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![] });
        wait_for(&mut view, "no tasks", |v| v.background_tasks.is_empty()).await;
        control.push(SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None });
        let done = wait_for(&mut view, "finished", |v| v.session.finished_tasks.len() == 1 && v.session.finished_tasks[0].tokens.is_some()).await;
        assert_eq!(done.session.finished_tasks[0].tokens, Some(500));
    }

    #[tokio::test]
    async fn a_resumed_task_leaves_the_finished_list() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());

        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        wait_for(&mut view, "one task", |v| v.background_tasks.len() == 1).await;
        control.push(SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None });
        wait_for(&mut view, "finished", |v| v.session.finished_tasks.len() == 1).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![] });
        wait_for(&mut view, "no tasks", |v| v.background_tasks.is_empty()).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        let resumed = wait_for(&mut view, "resumed", |v| v.background_tasks.len() == 1 && v.session.finished_tasks.is_empty()).await;
        assert_eq!(resumed.background_tasks[0].info.id, "b1");
    }

    #[tokio::test]
    async fn the_task_set_empties_when_the_sidecar_fails() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("go".to_string());
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        wait_for(&mut view, "one task", |v| v.background_tasks.len() == 1).await;

        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let failed = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;
        assert!(failed.background_tasks.is_empty());
    }

    #[tokio::test]
    async fn a_suggestion_is_kept_until_the_next_send() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        control.push(turn_complete());
        control.push(SidecarEvent::PromptSuggestion { text: "run the tests".to_string() });
        let with = wait_for(&mut view, "the suggestion", |v| v.suggestion.is_some()).await;
        assert_eq!(with.suggestion.as_deref(), Some("run the tests"));

        handle.send("two".to_string());
        let cleared = wait_for(&mut view, "the suggestion cleared", |v| v.suggestion.is_none() && v.phase == Phase::Running).await;
        assert!(cleared.suggestion.is_none());
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
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
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
        assert_eq!(seen.session.messages[0].text(), "earlier");
    }

    #[tokio::test]
    async fn a_spawn_failure_keeps_the_unsent_text_so_the_ui_can_put_it_back() {
        let (factory, _spawns) = factory_returning(vec![Err(SidecarError::SpawnFailed("no node".to_string()))]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("my long message".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.unsent, Some("my long message".to_string()));
        assert_eq!(seen.failed_sends, 1);
    }

    #[tokio::test]
    async fn a_failed_write_keeps_the_unsent_text_and_counts_the_failure() {
        let (sidecar, control) = fake_sidecar();
        control.fail_send();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("lost?".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.unsent, Some("lost?".to_string()));
        assert_eq!(seen.failed_sends, 1);
    }

    #[tokio::test]
    async fn a_failed_send_keeps_the_attachments_too_and_a_good_one_clears_them() {
        let (sidecar, control) = fake_sidecar();
        control.fail_send();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        let attachment = forge_domain::Attachment { path: "/x/a.png".into(), kind: forge_domain::AttachmentKind::Image };

        handle.send_with(String::new(), vec![attachment.clone()]);
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.unsent, Some(String::new()), "a chips-only message has no text to bring back");
        assert_eq!(seen.unsent_attachments, vec![attachment]);
        assert_eq!(seen.failed_sends, 1);
    }

    #[tokio::test]
    async fn a_successful_send_after_a_failure_clears_the_unsent_text_but_keeps_the_count() {
        let (first, _first_control) = fake_sidecar();
        let (second, _second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![
            Err(SidecarError::SpawnFailed("no node".to_string())),
            Ok(second),
        ]);
        let _ = first;
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("one".to_string());
        let seen = wait_for(&mut view, "the retry to start", |v| v.phase == Phase::Running).await;

        assert_eq!(seen.unsent, None);
        assert_eq!(seen.failed_sends, 1);
    }

    #[tokio::test]
    async fn a_completed_turn_drops_permission_requests_that_can_no_longer_be_answered() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("run it".to_string());
        control.push(permission_request("p-1"));
        wait_for(&mut view, "a queued request", |v| !v.pending_permissions.is_empty()).await;

        // Stop: the sidecar denies the open request itself, says so, and the turn ends.
        control.push(SidecarEvent::PermissionCancelled { permission_request_id: "p-1".to_string() });
        control.push(turn_complete());
        let seen = wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;

        assert!(seen.pending_permissions.is_empty(), "a stale card would hide the next request");
    }

    #[tokio::test]
    async fn a_request_still_open_when_the_turn_ends_stays_until_it_is_cancelled_or_answered() {
        // A workflow's agents ask after the turn that started it has ended; their cards must stay.
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("run a workflow".to_string());
        control.push(permission_request("p-1"));
        control.push(permission_request("p-2"));
        wait_for(&mut view, "two queued requests", |v| v.pending_permissions.len() == 2).await;

        control.push(turn_complete());
        let idle = wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        assert_eq!(idle.pending_permissions.len(), 2, "the turn ending does not withdraw them");

        control.push(SidecarEvent::PermissionCancelled { permission_request_id: "p-1".to_string() });
        let after = wait_for(&mut view, "one request left", |v| v.pending_permissions.len() == 1).await;
        assert_eq!(after.pending_permissions[0].id, "p-2");

        control.push(SidecarEvent::PermissionCancelled { permission_request_id: "unknown".to_string() });
        handle.approve("p-2".to_string(), PermissionDecision::AllowOnce);
        wait_for(&mut view, "none left", |v| v.pending_permissions.is_empty()).await;
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
    async fn the_started_session_gets_exactly_the_enabled_extensions() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let config = AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() };
        let skills = vec![
            SkillDefinition { name: "pdf".to_string(), path: PathBuf::from("/skills/pdf"), description: None },
            SkillDefinition { name: "docx".to_string(), path: PathBuf::from("/skills/docx"), description: None },
        ];
        let mut init = new_init(factory);
        init.extensions = extensions_with(config, skills);
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.extensions = ActiveExtensions {
            skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
            ..ActiveExtensions::default()
        };
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_reader_failure_spawns_nothing_and_keeps_the_typed_text() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = new_init(factory);
        init.extensions = failing_extensions();
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("skill reader is down"));
        assert_eq!(seen.unsent.as_deref(), Some("hello"));
        assert_eq!(spawns.load(Ordering::SeqCst), 0, "no process for a session that cannot start");
        assert!(control.calls().is_empty());
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
    async fn close_shuts_the_sidecar_down_before_dropping_it() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(control.lifecycle(), vec!["shutdown", "dropped"]);
    }

    #[tokio::test]
    async fn an_unexpected_session_ended_event_shuts_the_still_running_sidecar_down() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());

        control.push(SidecarEvent::SessionEnded);
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(control.lifecycle(), vec!["shutdown", "dropped"]);
    }

    #[tokio::test]
    async fn a_restarted_session_keeps_the_extensions_it_first_started_with() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let (extensions, loads) = counting_extensions();
        let mut init = new_init(factory);
        init.extensions = extensions;
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(loads.load(Ordering::SeqCst), 1, "the toggles are resolved once per open session");
        let started = |calls: Vec<Call>| match calls.into_iter().next() {
            Some(Call::Start(params)) => params.extensions,
            other => panic!("expected a start, got {other:?}"),
        };
        let first_extensions = started(first_control.calls());
        assert_eq!(first_extensions.skills.len(), 1);
        assert_eq!(started(second_control.calls()), first_extensions);
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
            v.phase == Phase::Idle && v.session.messages.iter().map(|m| m.text().len()).sum::<usize>() >= 106
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
        let total: usize = seen.session.messages.iter().map(|m| m.text().len()).sum();
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
        assert_eq!(last.session.messages[1].text().len(), 1000);
        assert!(updates <= 5, "1001 events were published {updates} times");
    }

    #[tokio::test]
    async fn a_finished_turn_adds_its_usage_to_the_totals_that_came_with_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = resumed_init(factory);
        init.session.usage.add("claude-test", TokenUsage { input: 100, ..Default::default() });
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("go".to_string());
        control.push(SidecarEvent::TurnComplete {
            usage: BTreeMap::from([("claude-test".to_string(), TokenUsage { input: 10, output: 5, ..Default::default() })]),
        });
        let seen = wait_for(&mut view, "the usage to grow", |v| v.session.usage.total().output == 5).await;

        assert_eq!(seen.session.usage.total().input, 110);
        assert_eq!(seen.phase, Phase::Idle);
    }

    #[tokio::test]
    async fn a_context_update_replaces_the_last_one() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert_eq!(handle.snapshot().context, None, "nothing is measured before the first turn");

        handle.send("go".to_string());
        control.push(SidecarEvent::ContextUsage { total_tokens: 10, max_tokens: 100 });
        control.push(SidecarEvent::ContextUsage { total_tokens: 20, max_tokens: 100 });
        let seen = wait_for(&mut view, "the second context value", |v| {
            v.context == Some(ContextUsage { total_tokens: 20, max_tokens: 100 })
        })
        .await;

        assert_eq!(seen.context.map(|c| c.percent()), Some(20));
    }

    fn detail(kind: &str, name: &str) -> SidecarEvent {
        SidecarEvent::ContextDetail {
            categories: vec![ContextCategory { name: name.to_string(), tokens: 5, kind: kind.to_string() }],
            mcp_tools: vec![ContextItemReport { server: Some("figma".to_string()), name: "t".to_string(), tokens: 3 }],
            agents: vec![ContextItemReport { server: None, name: "Explore".to_string(), tokens: 2 }],
        }
    }

    fn server(name: &str, status: &str, transport: &str) -> McpServerReport {
        McpServerReport {
            name: name.to_string(),
            status: status.to_string(),
            scope: Some("user".to_string()),
            source: None,
            transport: Some(transport.to_string()),
            tool_count: 4,
            error: None,
        }
    }

    #[tokio::test]
    async fn a_context_detail_event_is_kept_and_replaced() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert_eq!(handle.snapshot().context_detail, None);

        handle.send("go".to_string());
        control.push(detail("used", "Messages"));
        control.push(detail("free", "Free space"));
        let seen = wait_for(&mut view, "the second breakdown", |v| {
            v.context_detail.as_ref().is_some_and(|d| d.rows[0].name == "Free space")
        })
        .await;
        let breakdown = seen.context_detail.unwrap();
        assert_eq!(breakdown.rows[0].kind, forge_domain::ContextKind::Free);
        assert_eq!(breakdown.mcp_tools[0].group.as_deref(), Some("figma"));
        assert_eq!(breakdown.agents[0].group, None);

        // An empty list, or only rows of an unknown kind, is no value and keeps the good one.
        control.push(SidecarEvent::ContextDetail { categories: vec![], mcp_tools: vec![], agents: vec![] });
        control.push(detail("weird", "Mystery"));
        control.push(SidecarEvent::ContextUsage { total_tokens: 1, max_tokens: 2 });
        let seen = wait_for(&mut view, "the marker", |v| v.context.is_some()).await;
        assert_eq!(seen.context_detail.as_ref().unwrap().rows[0].name, "Free space");

        // A weird row next to a valid one: only the valid row survives.
        control.push(SidecarEvent::ContextDetail {
            categories: vec![
                ContextCategory { name: "Mystery".to_string(), tokens: 1, kind: "weird".to_string() },
                ContextCategory { name: "Autocompact buffer".to_string(), tokens: 9, kind: "buffer".to_string() },
            ],
            mcp_tools: vec![],
            agents: vec![],
        });
        let seen = wait_for(&mut view, "the buffer breakdown", |v| {
            v.context_detail.as_ref().is_some_and(|d| d.rows[0].name == "Autocompact buffer")
        })
        .await;
        assert_eq!(seen.context_detail.unwrap().rows.len(), 1);
    }

    #[tokio::test]
    async fn an_mcp_status_event_replaces_the_server_list() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert_eq!(handle.snapshot().mcp_servers, None);

        handle.send("go".to_string());
        control.push(SidecarEvent::McpStatus { servers: vec![server("a", "connected", "stdio"), server("b", "needs-auth", "http")] });
        control.push(SidecarEvent::McpStatus { servers: vec![server("c", "failed", "sse")] });
        let seen = wait_for(&mut view, "the second list", |v| v.mcp_servers.as_ref().is_some_and(|s| s[0].name == "c")).await;
        let servers = seen.mcp_servers.unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].status, forge_domain::McpStatus::Failed);
        assert_eq!(servers[0].remote, Some(true));
    }

    #[tokio::test]
    async fn an_empty_server_list_is_kept_as_empty() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("go".to_string());
        control.push(SidecarEvent::McpStatus { servers: vec![] });
        let seen = wait_for(&mut view, "an empty list", |v| v.mcp_servers.is_some()).await;
        assert_eq!(seen.mcp_servers, Some(vec![]));
    }

    #[tokio::test]
    async fn a_commands_update_replaces_the_last_list() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert!(handle.snapshot().commands.is_empty());

        handle.send("go".to_string());
        let named = |name: &str| forge_domain::SlashCommand { name: name.to_string(), description: None, argument_hint: None };
        control.push(SidecarEvent::CommandsAvailable { commands: vec![named("a"), named("b")] });
        control.push(SidecarEvent::CommandsAvailable { commands: vec![named("c")] });
        let seen = wait_for(&mut view, "the second list", |v| v.commands == vec![named("c")]).await;

        assert_eq!(seen.commands.len(), 1);
    }

    #[tokio::test]
    async fn warming_up_starts_the_sidecar_before_any_send_and_the_commands_arrive() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        let command = forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None };
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command.clone()] });
        let seen = wait_for(&mut view, "the commands", |v| !v.commands.is_empty()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None))]);
        assert_eq!(seen.commands, vec![command]);
        assert_eq!(seen.phase, Phase::Idle);
        assert_eq!(seen.last_error, None);
    }

    #[tokio::test]
    async fn the_first_send_after_a_warm_up_reuses_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.send("hello".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn directories_added_after_a_warm_up_restart_the_sidecar_so_the_sdk_gets_them() {
        // `open_new` warms the session up before the new-session screen can add its folders.
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.add_directories(vec!["/extra".into()]);
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(first_control.calls(), vec![Call::Start(start_params(None)), Call::Shutdown]);
        let mut expected = start_params(None);
        expected.additional_directories = vec!["/extra".to_string()];
        assert_eq!(second_control.calls(), vec![Call::Start(expected), Call::Send("hi".to_string())]);
    }

    #[tokio::test]
    async fn a_folder_added_after_the_first_send_restarts_the_sidecar_on_the_next_send_with_the_longer_list() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.add_directories(vec!["/a".into()]);
        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        first_control.push(SidecarEvent::SessionStarted { session_id: "claude-9".to_string() });
        first_control.push(turn_complete());
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle && v.claude_session_id.is_some()).await;
        handle.add_directories(vec!["/x".into()]);
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert!(first_control.calls().contains(&Call::Shutdown));
        let mut expected = start_params(Some("claude-9"));
        expected.additional_directories = vec!["/a".to_string(), "/x".to_string()];
        assert_eq!(second_control.calls(), vec![Call::Start(expected), Call::Send("two".to_string())]);
    }

    #[tokio::test]
    async fn a_folder_added_while_a_turn_runs_is_ignored() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        handle.add_directories(vec!["/late".into()]);
        handle.send("two".to_string());
        wait_for(&mut view, "the second message", |v| v.session.messages.len() == 2).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::Send("one".to_string()), Call::Send("two".to_string())]
        );
    }

    #[tokio::test]
    async fn a_folder_added_twice_is_listed_once() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        first_control.push(turn_complete());
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        handle.add_directories(vec!["/x".into()]);
        handle.add_directories(vec!["/x".into()]);
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(None);
        expected.additional_directories = vec!["/x".to_string()];
        assert_eq!(second_control.calls(), vec![Call::Start(expected), Call::Send("two".to_string())]);
    }

    #[tokio::test]
    async fn moving_changes_the_working_folder_and_the_next_send_resumes_there() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        first_control.push(SidecarEvent::SessionStarted { session_id: "claude-9".to_string() });
        first_control.push(turn_complete());
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle && v.claude_session_id.is_some()).await;
        handle.move_to(PathBuf::from("/new"));
        let seen = wait_for(&mut view, "the move", |v| v.project_path == PathBuf::from("/new")).await;
        assert_eq!(seen.session.project_path, PathBuf::from("/new"));
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert!(first_control.calls().contains(&Call::Shutdown));
        let mut expected = start_params(Some("claude-9"));
        expected.cwd = "/new".to_string();
        assert_eq!(second_control.calls(), vec![Call::Start(expected), Call::Send("two".to_string())]);
    }

    #[tokio::test]
    async fn moving_while_a_turn_runs_is_ignored() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        handle.move_to(PathBuf::from("/new"));
        handle.send("two".to_string());
        let seen = wait_for(&mut view, "the second message", |v| v.session.messages.len() == 2).await;

        assert_eq!(seen.project_path, PathBuf::from("/work/a"));
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::Send("one".to_string()), Call::Send("two".to_string())]
        );
    }

    // One finished turn on the first sidecar, which named the Claude session `claude-9`.
    async fn after_one_turn(handle: &LiveSessionHandle, view: &mut watch::Receiver<LiveSessionView>, control: &crate::test_support::FakeControl) {
        handle.send("one".to_string());
        wait_for(view, "the turn to start", |v| v.phase == Phase::Running).await;
        control.push(SidecarEvent::SessionStarted { session_id: "claude-9".to_string() });
        control.push(turn_complete());
        wait_for(view, "the turn to end", |v| v.phase == Phase::Idle && v.claude_session_id.is_some()).await;
    }

    #[tokio::test]
    async fn suspend_stops_an_idle_sidecar_and_the_next_send_resumes_on_a_new_one() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        after_one_turn(&handle, &mut view, &first_control).await;

        assert!(handle.suspend().await);
        assert!(first_control.calls().contains(&Call::Shutdown));
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(second_control.calls(), vec![Call::Start(start_params(Some("claude-9"))), Call::Send("two".to_string())]);
    }

    #[tokio::test]
    async fn suspend_is_refused_while_a_turn_runs() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert!(!handle.suspend().await);
        assert!(!control.calls().contains(&Call::Shutdown));
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn a_running_background_task_refuses_suspend_folders_and_moves() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        after_one_turn(&handle, &mut view, &control).await;
        control.push(SidecarEvent::BackgroundTasks { tasks: vec![task("b1")] });
        wait_for(&mut view, "the task", |v| v.background_tasks.len() == 1).await;

        assert!(!handle.suspend().await);
        handle.add_directories(vec!["/x".into()]);
        handle.move_to(PathBuf::from("/new"));
        handle.send("two".to_string());
        let seen = wait_for(&mut view, "the second message", |v| v.session.messages.len() == 2).await;

        assert_eq!(seen.project_path, PathBuf::from("/work/a"));
        assert_eq!(seen.background_tasks.len(), 1, "the task is still there");
        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(!control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn a_move_after_suspend_applies_and_the_next_send_resumes_in_the_new_folder() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        after_one_turn(&handle, &mut view, &first_control).await;

        assert!(handle.suspend().await);
        handle.move_to(PathBuf::from("/new"));
        wait_for(&mut view, "the move", |v| v.project_path == PathBuf::from("/new")).await;
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        let mut expected = start_params(Some("claude-9"));
        expected.cwd = "/new".to_string();
        assert_eq!(second_control.calls(), vec![Call::Start(expected), Call::Send("two".to_string())]);
    }

    #[tokio::test]
    async fn adding_a_folder_the_session_already_has_keeps_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.add_directories(vec!["/x".into()]);
        after_one_turn(&handle, &mut view, &control).await;

        handle.add_directories(vec!["/x".into()]);
        handle.send("two".to_string());
        wait_for(&mut view, "the second message", |v| v.session.messages.len() == 2).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(!control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn moving_to_the_folder_the_session_is_in_keeps_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        after_one_turn(&handle, &mut view, &control).await;

        handle.move_to(PathBuf::from("/work/a"));
        handle.send("two".to_string());
        wait_for(&mut view, "the second message", |v| v.session.messages.len() == 2).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(!control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn moving_drops_the_cached_extensions_so_they_are_resolved_again_for_the_new_folder() {
        let (first, first_control) = fake_sidecar();
        let (second, _second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let (extensions, asked) = crate::test_support::path_recording_extensions();
        let mut init = new_init(factory);
        init.extensions = extensions;
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        first_control.push(turn_complete());
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle).await;
        handle.move_to(PathBuf::from("/new"));
        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(*asked.lock().unwrap(), vec![PathBuf::from("/work/a"), PathBuf::from("/new")]);
    }

    #[tokio::test]
    async fn a_failed_warm_up_shows_no_error_and_the_first_send_starts_the_sidecar_again() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Err(SidecarError::SpawnFailed("no node".to_string())), Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(seen.last_error, None);
        assert_eq!(control.calls(), vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_session_that_cannot_send_does_not_warm_up() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(LiveSessionInit { can_send: false, ..new_init(factory) });
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closing_right_after_a_warm_up_ends_the_session() {
        let (sidecar, _control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.close();

        wait_closed(&mut view).await;
    }

    #[tokio::test]
    async fn a_warm_sidecar_that_fails_before_any_send_is_dropped_without_an_error_and_the_next_send_starts_again() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        first_control.push(SidecarEvent::Error { message: "the SDK failed to start".to_string() });
        tokio::time::sleep(Duration::from_millis(150)).await;
        let quiet = handle.snapshot();
        assert_eq!(quiet.last_error, None, "no banner for a failure nobody asked for");
        assert_eq!(quiet.phase, Phase::Idle);

        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(seen.last_error, None);
        assert_eq!(second_control.calls(), vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]);
    }

    #[tokio::test]
    async fn a_sidecar_that_fails_after_a_send_still_fails_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.warm_up();
        handle.send("hello".to_string());
        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.last_error.as_deref(), Some("boom"));
    }
}
