use std::future::pending;
use std::sync::Arc;

use forge_domain::PermissionDecision;
use forge_ports::{SidecarEvent, SidecarFactoryPort, SidecarPort, StartSessionParams};
use tokio::sync::{mpsc, watch};

use crate::event::{apply_sidecar_event, SessionUpdate};
use crate::live_session::{LiveCommand, LiveSessionView, Phase};
use crate::use_cases::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};

const CANNOT_SEND: &str = "the project folder no longer exists, so this session can only be viewed";

/// The one task that owns a session. It `select!`s between UI commands and the sidecar's event
/// stream, so nothing is shared and no lock is needed.
pub(crate) struct Actor {
    view: LiveSessionView,
    view_tx: watch::Sender<LiveSessionView>,
    commands: mpsc::UnboundedReceiver<LiveCommand>,
    factory: Arc<dyn SidecarFactoryPort>,
    model: Option<String>,
    sidecar: Option<Arc<dyn SidecarPort>>,
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
    ) -> Self {
        Self {
            view,
            view_tx,
            commands,
            factory,
            model,
            sidecar: None,
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
                    None => self.fail("the sidecar process exited unexpectedly".to_string(), false).await,
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
            LiveCommand::Send(text) => self.send(text).await,
            LiveCommand::Approve { id, decision } => self.approve(id, decision).await,
            LiveCommand::Interrupt => self.interrupt().await,
            LiveCommand::Close => {}
        }
    }

    async fn send(&mut self, text: String) {
        if !self.view.can_send {
            self.view.last_error = Some(CANNOT_SEND.to_string());
            self.dirty = true;
            return;
        }
        if self.sidecar.is_none() {
            if let Err(message) = self.start_sidecar().await {
                self.fail(message, false).await;
                return;
            }
        }
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };

        let history_len = self.view.session.messages.len();
        match SendMessageUseCase::new(sidecar).execute(&mut self.view.session, text).await {
            Ok(()) => {
                self.view.phase = Phase::Running;
                self.view.last_error = None;
                self.dirty = true;
            }
            Err(error) => {
                // The use-case records the message before writing it; a message that was never
                // delivered must not stay in the history.
                self.view.session.messages.truncate(history_len);
                self.fail(error.to_string(), false).await;
            }
        }
    }

    // Spawns a sidecar and starts (or resumes) the session on it. The sidecar is only kept once
    // both steps succeed; otherwise it is dropped, which ends its process.
    async fn start_sidecar(&mut self) -> Result<(), String> {
        let sidecar = self.factory.spawn().await.map_err(|e| e.to_string())?;
        sidecar
            .start_session(StartSessionParams {
                cwd: self.view.project_path.to_string_lossy().to_string(),
                resume_session_id: self.view.claude_session_id.as_ref().map(|id| id.0.clone()),
                permission_mode: self.view.session.permission_mode,
                model: self.model.clone(),
            })
            .await
            .map_err(|e| e.to_string())?;
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

    async fn apply_event(&mut self, event: SidecarEvent) {
        match apply_sidecar_event(&mut self.view.session, &event) {
            SessionUpdate::SessionStarted { session_id } => {
                self.view.session.id = session_id.clone();
                self.view.claude_session_id = Some(session_id);
            }
            SessionUpdate::PermissionRequested(request) => {
                self.view.pending_permissions.push_back(request);
            }
            SessionUpdate::TurnCompleted { .. } => self.view.phase = Phase::Idle,
            // The sidecar protocol does not distinguish fatal from non-fatal errors, so every
            // `Error` ends this sidecar: its query is over and `start_session` cannot be re-sent.
            SessionUpdate::Errored { message } => self.fail(message, true).await,
            SessionUpdate::SessionEnded => {
                self.fail("the session ended unexpectedly".to_string(), false).await;
            }
            SessionUpdate::MessageDelta { .. }
            | SessionUpdate::ToolCallStarted { .. }
            | SessionUpdate::ToolCallCompleted { .. } => {}
        }
        self.dirty = true;
    }

    // Marks the session failed and drops its sidecar. Permission requests that can no longer be
    // answered are cleared. `shutdown` asks a still-running sidecar to exit first.
    async fn fail(&mut self, message: String, shutdown: bool) {
        self.discard_sidecar(shutdown).await;
        self.view.phase = Phase::Failed;
        self.view.last_error = Some(message);
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
