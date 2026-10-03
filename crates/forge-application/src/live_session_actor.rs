use std::future::pending;
use std::sync::Arc;

use forge_domain::{ActiveExtensions, Attachment, PermissionDecision, PermissionMode};
use forge_ports::{SidecarEvent, SidecarFactoryPort, SidecarPort, StartSessionParams};
use tokio::sync::{mpsc, watch};

use crate::event::{apply_sidecar_event, SessionUpdate};
use crate::live_session::{LiveCommand, LiveSessionView, Phase};
use crate::use_cases::{ApprovePermissionUseCase, ExtensionsUseCase, InterruptTurnUseCase, SendMessageUseCase};

const CANNOT_SEND: &str = "the project folder no longer exists, so this session can only be viewed";

/// The one task that owns a session. It `select!`s between UI commands and the sidecar's event
/// stream, so nothing is shared and no lock is needed.
pub(crate) struct Actor {
    view: LiveSessionView,
    view_tx: watch::Sender<LiveSessionView>,
    commands: mpsc::UnboundedReceiver<LiveCommand>,
    factory: Arc<dyn SidecarFactoryPort>,
    model: Option<String>,
    extensions: Arc<ExtensionsUseCase>,
    /// What the first successful start resolved. An open session keeps it across restarts (spec
    /// §17.4); a failed resolution is not stored, so the next start resolves again.
    active: Option<ActiveExtensions>,
    sidecar: Option<Arc<dyn SidecarPort>>,
    /// Extra folders for the next start (the SDK reads them once, at start).
    additional_directories: Vec<String>,
    /// The sidecar was started by a warm-up and nothing has been sent to it yet. A failure then is
    /// dropped quietly: nobody asked for anything, and the first send starts a sidecar again.
    warm_only: bool,
    /// `last_error` holds the SDK's refusal of a mode or model change. A later change that the SDK
    /// accepts clears it; any other message stays.
    refusal_shown: bool,
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
        extensions: Arc<ExtensionsUseCase>,
    ) -> Self {
        Self {
            view,
            view_tx,
            commands,
            factory,
            model,
            extensions,
            active: None,
            sidecar: None,
            additional_directories: Vec::new(),
            warm_only: false,
            refusal_shown: false,
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
            LiveCommand::WarmUp => self.warm_up().await,
            LiveCommand::Approve { id, decision } => self.approve(id, decision).await,
            LiveCommand::Interrupt => self.interrupt().await,
            LiveCommand::SetMode(mode) => self.set_mode(mode).await,
            LiveCommand::SetModel(model) => self.set_model(model).await,
            LiveCommand::SetTitle(title) => {
                self.view.session.title = Some(title);
                self.dirty = true;
            }
            LiveCommand::Close => {}
        }
    }

    // The SDK reads the folders once, at start. A sidecar that only a warm-up started (nothing sent
    // yet, and `open_new` warms every new session up before its folders can be added) is dropped so
    // the first send starts one that has them. After a message was sent they are ignored.
    async fn add_directories(&mut self, directories: Vec<std::path::PathBuf>) {
        if self.sidecar.is_some() && !self.warm_only {
            return;
        }
        for directory in directories {
            let directory = directory.to_string_lossy().to_string();
            if !self.additional_directories.contains(&directory) {
                self.additional_directories.push(directory);
            }
        }
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
        let extensions = match &self.active {
            Some(active) => active.clone(),
            None => self
                .extensions
                .resolve_active(&self.view.project_path)
                .await
                .map_err(|e| e.to_string())?,
        };
        let sidecar = self.factory.spawn().await.map_err(|e| e.to_string())?;
        sidecar
            .start_session(StartSessionParams {
                cwd: self.view.project_path.to_string_lossy().to_string(),
                resume_session_id: self.view.claude_session_id.as_ref().map(|id| id.0.clone()),
                permission_mode: self.view.session.permission_mode,
                model: self.model.clone(),
                extensions: extensions.clone(),
                additional_directories: self.additional_directories.clone(),
            })
            .await
            .map_err(|e| e.to_string())?;
        self.active = Some(extensions);
        self.sidecar = Some(sidecar);
        Ok(())
    }

    async fn approve(&mut self, id: String, decision: PermissionDecision) {
        let Some(position) = self.view.pending_permissions.iter().position(|request| request.id == id) else {
            return;
        };
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
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

    async fn apply_event(&mut self, event: SidecarEvent) {
        match apply_sidecar_event(&mut self.view.session, &event) {
            SessionUpdate::SessionStarted { session_id } => {
                self.view.session.id = session_id.clone();
                self.view.claude_session_id = Some(session_id);
            }
            SessionUpdate::PermissionRequested(request) => {
                self.view.pending_permissions.push_back(request);
            }
            // A finished turn has no open permission request any more (Stop makes the sidecar deny
            // it silently), and a stale card would hide the next real one.
            SessionUpdate::TurnCompleted => {
                self.view.phase = Phase::Idle;
                self.view.pending_permissions.clear();
            }
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
        self.view.last_error = Some(message);
        self.refusal_shown = false;
        self.view.pending_permissions.clear();
        self.dirty = true;
    }

    async fn discard_sidecar(&mut self, shutdown: bool) {
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
