use std::future::pending;
use std::sync::Arc;

use forge_domain::{ActiveExtensions, Attachment, EffortLevel, PermissionDecision, PermissionMode, SessionDefaults};
use forge_ports::{SidecarEvent, SidecarFactoryPort, SidecarPort, StartSessionParams};
use tokio::sync::{mpsc, watch};

use crate::event::{apply_sidecar_event, SessionUpdate};
use crate::live_session::{LiveCommand, LiveSessionView, Phase};
use crate::use_cases::{AnswerQuestionUseCase, ApprovePermissionUseCase, StopTaskUseCase, ExtensionsUseCase, InterruptTurnUseCase, SendMessageUseCase};

const CANNOT_SEND: &str = "the project folder no longer exists, so this session can only be viewed";

/// The one task that owns a session. It `select!`s between UI commands and the sidecar's event
/// stream, so nothing is shared and no lock is needed.
pub(crate) struct Actor {
    view: LiveSessionView,
    view_tx: watch::Sender<LiveSessionView>,
    commands: mpsc::UnboundedReceiver<LiveCommand>,
    factory: Arc<dyn SidecarFactoryPort>,
    model: Option<String>,
    effort: Option<EffortLevel>,
    extensions: Arc<ExtensionsUseCase>,
    /// What the first successful start resolved. An open session keeps it across restarts (spec
    /// §17.4); a failed resolution is not stored, so the next start resolves again.
    active: Option<ActiveExtensions>,
    /// What Settings decided at the first successful start; kept across restarts like `active`
    /// (a change in Settings applies to the next new session).
    defaults: Option<SessionDefaults>,
    sidecar: Option<Arc<dyn SidecarPort>>,
    /// Extra folders for the next start (the SDK reads them once, at start).
    additional_directories: Vec<String>,
    /// Permission rules for the next start (the SDK's `allowedTools`).
    allowed_tools: Vec<String>,
    env: forge_domain::EnvVars,
    /// The sidecar was started by a warm-up and nothing has been sent to it yet. A failure then is
    /// dropped quietly: nobody asked for anything, and the first send starts a sidecar again.
    warm_only: bool,
    /// `last_error` holds the SDK's refusal of a mode, model or effort change. A later change that the SDK
    /// accepts clears it; any other message stays.
    refusal_shown: bool,
    /// Tasks the task set dropped before their finish notification arrived; the notification takes
    /// their tokens and time from here.
    vanished: std::collections::HashMap<String, forge_domain::RunningTask>,
    /// The view changed since it was last published.
    dirty: bool,
}

impl Actor {
    pub(crate) fn new(
        view: LiveSessionView,
        view_tx: watch::Sender<LiveSessionView>,
        commands: mpsc::UnboundedReceiver<LiveCommand>,
        factory: Arc<dyn SidecarFactoryPort>,
        model: Option<String>,
        effort: Option<EffortLevel>,
        extensions: Arc<ExtensionsUseCase>,
    ) -> Self {
        Self {
            view,
            view_tx,
            commands,
            factory,
            model,
            effort,
            extensions,
            active: None,
            defaults: None,
            sidecar: None,
            additional_directories: Vec::new(),
            allowed_tools: Vec::new(),
            env: forge_domain::EnvVars::default(),
            warm_only: false,
            refusal_shown: false,
            vanished: Default::default(),
            dirty: false,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                // Priority order: commands, then sidecar events, then publishing. The view is
                // only published when neither of the others is immediately ready, so a burst of
                // events is applied first and published once.
                biased;
                command = self.commands.recv() => match command {
                    None | Some(LiveCommand::Close) => {
                        self.discard_sidecar(true).await;
                        break;
                    }
                    Some(command) => self.handle_command(command).await,
                },
                event = next_event(&self.sidecar) => match event {
                    Some(event) => self.apply_event(event).await,
                    None => self.sidecar_gone("the sidecar process exited unexpectedly".to_string(), false).await,
                },
                _ = std::future::ready(()), if self.dirty => self.publish(),
            }
        }
    }

    fn publish(&mut self) {
        self.view_tx.send_replace(self.view.clone());
        self.dirty = false;
    }

    async fn handle_command(&mut self, command: LiveCommand) {
        match command {
            LiveCommand::Send { text, attachments } => self.send(text, attachments).await,
            LiveCommand::AddDirectories(directories) => self.add_directories(directories).await,
            LiveCommand::MoveTo(folder) => self.move_to(folder).await,
            LiveCommand::Suspend(answer) => {
                let done = self.suspend().await;
                let _ = answer.send(done);
            }
            LiveCommand::AllowTools(tools) => self.allow_tools(tools).await,
            LiveCommand::SetEnv(env) => self.set_env(env).await,
            LiveCommand::WarmUp => self.warm_up().await,
            LiveCommand::Approve { id, decision } => self.approve(id, decision).await,
            LiveCommand::AnswerQuestion { id, answers } => self.answer_question(id, answers).await,
            LiveCommand::StopTask { task_id } => self.stop_task(task_id).await,
            LiveCommand::Interrupt => self.interrupt().await,
            LiveCommand::SetMode(mode) => self.set_mode(mode).await,
            LiveCommand::SetModel(model) => self.set_model(model).await,
            LiveCommand::SetEffort(effort) => self.set_effort(effort).await,
            LiveCommand::SetTitle(title) => {
                self.view.session.title = Some(title);
                self.dirty = true;
            }
            LiveCommand::Close => {}
        }
    }

    // The SDK reads the folders once, at start. A sidecar that only a warm-up started (nothing sent
    // yet, and `open_new` warms every new session up before its folders can be added) is dropped so
    // the first send starts one that has them. A sidecar that has served a turn is dropped too, once
    // the turn is over: the next send resumes the session on a new one. While a turn runs the
    // folders are ignored.
    // Background tasks live in the sidecar too, so a session running any is busy as well.
    async fn add_directories(&mut self, directories: Vec<std::path::PathBuf>) {
        if self.sidecar_busy() {
            return;
        }
        let mut grew = false;
        for directory in directories {
            let directory = directory.to_string_lossy().to_string();
            if !self.additional_directories.contains(&directory) {
                self.additional_directories.push(directory);
                grew = true;
            }
        }
        if self.sidecar.is_some() && (grew || self.warm_only) {
            self.warm_only = false;
            self.restart_on_next_send("a folder was added").await;
        }
    }

    // The next send resumes the session in the new folder. The extensions that depend on the folder
    // are resolved again then. With no sidecar (after `suspend`) nothing can refuse it.
    async fn move_to(&mut self, folder: std::path::PathBuf) {
        if self.sidecar_busy() || folder == self.view.project_path {
            return;
        }
        self.view.project_path = folder.clone();
        self.view.session.project_path = folder;
        self.active = None;
        self.defaults = None;
        self.warm_only = false;
        self.restart_on_next_send("the session moved to another folder").await;
        self.dirty = true;
    }

    async fn suspend(&mut self) -> bool {
        if self.sidecar_busy() {
            return false;
        }
        self.warm_only = false;
        self.restart_on_next_send("the session's transcript is about to move").await;
        true
    }

    fn sidecar_busy(&self) -> bool {
        self.sidecar.is_some() && (self.view.phase == Phase::Running || !self.view.background_tasks.is_empty())
    }

    // The sidecar's stderr goes to the app's, so this line shows when a restart was asked for.
    async fn restart_on_next_send(&mut self, reason: &str) {
        if self.sidecar.is_some() {
            eprintln!("forge: session {} stops its sidecar ({reason}); the next send resumes it", self.view.id.0);
        }
        self.discard_sidecar(true).await;
    }

    // Like folders, the SDK reads these once, at start: a warm-up sidecar is dropped so the first send
    // starts one that has them; after a message was sent they are ignored.
    async fn allow_tools(&mut self, tools: Vec<String>) {
        if self.sidecar.is_some() && !self.warm_only {
            return;
        }
        for tool in tools {
            if !self.allowed_tools.contains(&tool) {
                self.allowed_tools.push(tool);
            }
        }
        if self.warm_only {
            self.warm_only = false;
            self.discard_sidecar(true).await;
        }
    }

    async fn set_env(&mut self, env: forge_domain::EnvVars) {
        if self.sidecar.is_some() && !self.warm_only {
            return;
        }
        self.env = env;
        if self.warm_only {
            self.warm_only = false;
            self.discard_sidecar(true).await;
        }
    }

    async fn warm_up(&mut self) {
        if !self.view.can_send || self.sidecar.is_some() {
            return;
        }
        // Not `fail`: nothing was asked of the session yet, so there is nothing to report. The first
        // send resolves and spawns again and shows its own error if it fails too.
        self.warm_only = self.start_sidecar().await.is_ok();
    }

    // The sidecar's process or session ended on its own. After a warm-up with nothing sent it is just
    // dropped; otherwise the session fails.
    async fn sidecar_gone(&mut self, message: String, shutdown: bool) {
        if self.warm_only {
            self.warm_only = false;
            self.discard_sidecar(shutdown).await;
        } else {
            self.fail(message, shutdown).await;
        }
    }

    async fn send(&mut self, text: String, attachments: Vec<Attachment>) {
        if !self.view.can_send {
            self.view.last_error = Some(CANNOT_SEND.to_string());
            self.refusal_shown = false;
            self.dirty = true;
            return;
        }
        self.warm_only = false;
        if self.sidecar.is_none() {
            if let Err(message) = self.start_sidecar().await {
                self.keep_unsent(text, attachments);
                self.fail(message, false).await;
                return;
            }
        }
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };

        let history_len = self.view.session.messages.len();
        match SendMessageUseCase::new(sidecar).execute(&mut self.view.session, text.clone(), attachments.clone()).await {
            Ok(()) => {
                self.view.phase = Phase::Running;
                self.view.turn_started = Some(std::time::SystemTime::now());
                self.view.suggestion = None;
                self.view.last_error = None;
                self.refusal_shown = false;
                self.view.unsent = None;
                self.view.unsent_attachments.clear();
                self.dirty = true;
            }
            Err(error) => {
                // The use-case records the message before writing it; a message that was never
                // delivered must not stay in the history.
                self.view.session.messages.truncate(history_len);
                self.keep_unsent(text, attachments);
                self.fail(error.to_string(), false).await;
            }
        }
    }

    // The message never reached the sidecar: hand its text back through the view so the UI can put
    // it in the message box again instead of losing what the person typed.
    fn keep_unsent(&mut self, text: String, attachments: Vec<Attachment>) {
        self.view.unsent = Some(text);
        self.view.unsent_attachments = attachments;
        self.view.failed_sends += 1;
    }

    // Resolves what is switched on, spawns a sidecar and starts (or resumes) the session on it.
    // Resolving comes first so a failing reader spawns no process. The sidecar is only kept once
    // both steps succeed; otherwise it is dropped, which ends its process.
    async fn start_sidecar(&mut self) -> Result<(), String> {
        let (extensions, configured) = match (&self.active, self.defaults) {
            (Some(active), Some(defaults)) => (active.clone(), defaults),
            _ => self.extensions.resolve_start(&self.view.project_path).await.map_err(|e| e.to_string())?,
        };
        // A session already in bypass mode (resumed, or switched before the switch went off) must
        // still start: the SDK refuses that mode unless it is allowed up front.
        let mut defaults = configured;
        defaults.allow_bypass |= self.view.session.permission_mode == PermissionMode::BypassPermissions;
        let sidecar = self.factory.spawn().await.map_err(|e| e.to_string())?;
        sidecar
            .start_session(StartSessionParams {
                cwd: self.view.project_path.to_string_lossy().to_string(),
                resume_session_id: self.view.claude_session_id.as_ref().map(|id| id.0.clone()),
                permission_mode: self.view.session.permission_mode,
                model: self.model.clone(),
                effort: self.effort,
                extensions: extensions.clone(),
                additional_directories: self.additional_directories.clone(),
                allowed_tools: self.allowed_tools.clone(),
                env: self.env.clone(),
                defaults,
            })
            .await
            .map_err(|e| e.to_string())?;
        self.active = Some(extensions);
        self.defaults = Some(configured);
        self.sidecar = Some(sidecar);
        Ok(())
    }

    async fn stop_task(&mut self, task_id: String) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        if let Err(error) = StopTaskUseCase::new(sidecar).execute(task_id).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn answer_question(&mut self, id: String, answers: std::collections::BTreeMap<String, String>) {
        let Some(position) = self.view.pending_permissions.iter().position(|request| request.id == id) else {
            return;
        };
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        self.view.pending_permissions.remove(position);
        self.dirty = true;
        if let Err(error) = AnswerQuestionUseCase::new(sidecar).execute(id, answers).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn approve(&mut self, id: String, decision: PermissionDecision) {
        let Some(position) = self.view.pending_permissions.iter().position(|request| request.id == id) else {
            return;
        };
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        if decision == PermissionDecision::AllowAlways {
            for rule in self.view.pending_permissions[position].rules.clone() {
                if !self.view.allowed_rules.contains(&rule) {
                    self.view.allowed_rules.push(rule);
                }
            }
        }
        self.view.pending_permissions.remove(position);
        self.dirty = true;
        if let Err(error) = ApprovePermissionUseCase::new(sidecar).execute(id, decision).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn interrupt(&mut self) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        if let Err(error) = InterruptTurnUseCase::new(sidecar).execute().await {
            self.fail(error.to_string(), false).await;
        }
    }

    // With a sidecar the SDK decides: the command goes out and the view changes when the answer
    // comes. Without one nothing can refuse, so the view and the next start take the value now.
    async fn set_mode(&mut self, mode: PermissionMode) {
        let Some(sidecar) = self.sidecar.clone() else {
            self.view.session.permission_mode = mode;
            self.dirty = true;
            return;
        };
        if let Err(error) = sidecar.set_permission_mode(mode).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn set_model(&mut self, model: Option<String>) {
        let Some(sidecar) = self.sidecar.clone() else {
            self.model = model.clone();
            self.view.model = model;
            self.dirty = true;
            return;
        };
        if let Err(error) = sidecar.set_model(model).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn set_effort(&mut self, effort: Option<EffortLevel>) {
        let Some(sidecar) = self.sidecar.clone() else {
            self.effort = effort;
            self.view.effort = effort;
            self.dirty = true;
            return;
        };
        if let Err(error) = sidecar.set_effort(effort).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn apply_event(&mut self, event: SidecarEvent) {
        match apply_sidecar_event(&mut self.view.session, &event) {
            SessionUpdate::SessionStarted { session_id } => {
                self.view.session.id = session_id.clone();
                self.view.claude_session_id = Some(session_id);
            }
            SessionUpdate::PermissionRequested(request) => {
                self.view.pending_permissions.push_back(request);
            }
            // Stop makes the sidecar deny an open request and say so; a stale card would hide the
            // next real one. A finished turn does not clear them: a workflow's agents are still
            // asking after the turn that started it ended.
            SessionUpdate::PermissionCancelled { id } => {
                self.view.pending_permissions.retain(|request| request.id != id);
            }
            SessionUpdate::TurnCompleted => {
                self.view.phase = Phase::Idle;
                self.view.turn_started = None;
            }
            SessionUpdate::BackgroundTasks(tasks) => {
                let now = std::time::SystemTime::now();
                let previous = std::mem::take(&mut self.view.background_tasks);
                self.view.background_tasks = tasks
                    .into_iter()
                    .map(|info| {
                        let before = previous.iter().find(|task| task.info.id == info.id);
                        forge_domain::RunningTask { seen_at: before.map_or(now, |task| task.seen_at), tokens: before.and_then(|task| task.tokens), info }
                    })
                    .collect();
                // A task that is new to the set and has an old end is a resumed one: it runs again.
                let resumed: Vec<String> = self.view.background_tasks.iter().filter(|task| !previous.iter().any(|old| old.info.id == task.info.id)).map(|task| task.info.id.clone()).collect();
                for id in resumed {
                    self.view.session.finished_tasks.retain(|item| item.task_id != id);
                    self.vanished.remove(&id);
                }
                let gone: Vec<_> = previous.into_iter().filter(|old| !self.view.background_tasks.iter().any(|task| task.info.id == old.info.id)).collect();
                for gone in gone {
                    if !self.fill_finished(&gone) {
                        self.vanished.insert(gone.info.id.clone(), gone);
                    }
                }
            }
            SessionUpdate::TaskProgress { task_id, total_tokens } => {
                if let Some(task) = self.view.background_tasks.iter_mut().find(|task| task.info.id == task_id) {
                    task.tokens = Some(total_tokens);
                }
            }
            SessionUpdate::TaskFinished { task_id } => {
                let seen = self.view.background_tasks.iter().find(|task| task.info.id == task_id).cloned().or_else(|| self.vanished.remove(&task_id));
                if let Some(task) = seen {
                    self.fill_finished(&task);
                }
            }
            SessionUpdate::PromptSuggestion(text) => self.view.suggestion = Some(text),
            SessionUpdate::ContextUsage(context) => self.view.context = Some(context),
            SessionUpdate::CommandsAvailable(commands) => self.view.commands = commands,
            // The session already holds the new mode (`apply_sidecar_event`); the model lives here
            // too, so a restarted sidecar starts on it.
            SessionUpdate::PermissionModeChanged { .. } => self.clear_refusal(),
            SessionUpdate::ModelChanged { model } => {
                self.model = model.clone();
                self.view.model = model;
                self.clear_refusal();
            }
            SessionUpdate::EffortChanged { effort } => {
                self.effort = effort;
                self.view.effort = effort;
                self.clear_refusal();
            }
            SessionUpdate::ModelsAvailable(models) => self.view.available_models = models,
            // A refusal is not a failure of the sidecar: the banner shows it and the session goes on.
            SessionUpdate::SettingFailed { message } => {
                self.view.last_error = Some(message);
                self.refusal_shown = true;
            }
            // The sidecar protocol does not distinguish fatal from non-fatal errors, so every
            // `Error` ends this sidecar: its query is over and `start_session` cannot be re-sent.
            SessionUpdate::Errored { message } => self.sidecar_gone(message, true).await,
            SessionUpdate::SessionEnded => {
                // The sidecar is still alive and cleaning up, so it is asked to exit and awaited.
                self.sidecar_gone("the session ended unexpectedly".to_string(), true).await;
            }
            SessionUpdate::MessageDelta { .. }
            | SessionUpdate::ThinkingDelta { .. }
            | SessionUpdate::ToolCallStarted { .. }
            | SessionUpdate::ToolCallCompleted { .. } => {}
        }
        self.dirty = true;
    }

    /// What a finished task's notification left out (its tokens, its time) is taken from what the
    /// view saw of it while it ran. False when no finished entry exists yet.
    fn fill_finished(&mut self, task: &forge_domain::RunningTask) -> bool {
        let Some(entry) = self.view.session.finished_tasks.iter_mut().find(|item| item.task_id == task.info.id) else { return false };
        if entry.tokens.is_none() {
            entry.tokens = task.tokens;
        }
        if entry.duration_ms.is_none() {
            entry.duration_ms = task.seen_at.elapsed().ok().map(|elapsed| elapsed.as_millis() as u64);
        }
        true
    }

    fn clear_refusal(&mut self) {
        if self.refusal_shown {
            self.view.last_error = None;
            self.refusal_shown = false;
        }
    }

    // Marks the session failed and drops its sidecar. Permission requests that can no longer be
    // answered are cleared. `shutdown` asks a still-running sidecar to exit first; the port's
    // `shutdown` waits (bounded) for it, so its cleanup runs before the process is dropped.
    async fn fail(&mut self, message: String, shutdown: bool) {
        self.discard_sidecar(shutdown).await;
        self.view.phase = Phase::Failed;
        self.view.turn_started = None;
        self.view.last_error = Some(message);
        self.refusal_shown = false;
        self.view.pending_permissions.clear();
        self.dirty = true;
    }

    async fn discard_sidecar(&mut self, shutdown: bool) {
        self.view.background_tasks.clear();
        self.vanished.clear();
        if let Some(sidecar) = self.sidecar.take() {
            if shutdown {
                let _ = sidecar.shutdown().await;
            }
        }
    }
}

// Waits for the next sidecar event; with no sidecar it never resolves, so `select!` ignores it.
async fn next_event(sidecar: &Option<Arc<dyn SidecarPort>>) -> Option<SidecarEvent> {
    match sidecar {
        Some(sidecar) => sidecar.next_event().await,
        None => pending().await,
    }
}
