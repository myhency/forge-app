use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::{PermissionDecision, PermissionMode};
use forge_ports::{SidecarError, SidecarEvent, SidecarPort, StartSessionParams};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, Mutex};

use crate::protocol::SidecarCommand;

/// How long `shutdown` waits for the sidecar to exit on its own (so it can remove its temp files)
/// before the process is killed.
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(3);

pub struct SidecarProcess {
    child: Mutex<Child>,
    shutdown_timeout: Duration,
    stdin_tx: mpsc::UnboundedSender<String>,
    events_rx: Mutex<mpsc::UnboundedReceiver<SidecarEvent>>,
    next_id: AtomicU64,
}

impl SidecarProcess {
    pub fn spawn(
        node_binary: &str,
        script_path: &str,
        env: &[(String, String)],
    ) -> Result<Self, SidecarError> {
        let mut child = Command::new(node_binary)
            .arg(script_path)
            .envs(env.iter().map(|(name, value)| (name.as_str(), value.as_str())))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;

        let mut stdin = child.stdin.take().expect("stdin was piped");
        let stdout = child.stdout.take().expect("stdout was piped");

        let (stdin_tx, mut stdin_rx) = mpsc::unbounded_channel::<String>();
        tokio::spawn(async move {
            while let Some(line) = stdin_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
            }
        });

        let (events_tx, events_rx) = mpsc::unbounded_channel::<SidecarEvent>();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<SidecarEvent>(trimmed) {
                            Ok(event) => {
                                if events_tx.send(event).is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                if events_tx
                                    .send(SidecarEvent::Error {
                                        message: format!("failed to parse sidecar event: {err} (line: {trimmed})"),
                                    })
                                    .is_err()
                                {
                                    break;
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        });

        Ok(Self {
            child: Mutex::new(child),
            shutdown_timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            stdin_tx,
            events_rx: Mutex::new(events_rx),
            next_id: AtomicU64::new(1),
        })
    }

    /// Overrides how long `shutdown` waits for the process to exit before killing it.
    pub fn with_shutdown_timeout(mut self, timeout: Duration) -> Self {
        self.shutdown_timeout = timeout;
        self
    }

    fn next_id(&self) -> String {
        self.next_id.fetch_add(1, Ordering::Relaxed).to_string()
    }

    fn send_command(&self, command: SidecarCommand) -> Result<(), SidecarError> {
        let json = serde_json::to_string(&command).map_err(|e| SidecarError::WriteFailed(e.to_string()))?;
        self.stdin_tx.send(json).map_err(|_| SidecarError::ProcessExited)
    }
}

#[async_trait]
impl SidecarPort for SidecarProcess {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::start_session(id, params))
    }

    async fn send_message(&self, text: String) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::SendMessage { id, text })
    }

    async fn respond_to_permission(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::PermissionResponse {
            id,
            permission_request_id,
            decision: decision.into(),
        })
    }

    async fn interrupt(&self) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::Interrupt { id })
    }

    async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::SetPermissionMode { id, mode: mode.into() })
    }

    async fn set_model(&self, model: Option<String>) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::SetModel { id, model })
    }

    async fn shutdown(&self) -> Result<(), SidecarError> {
        let id = self.next_id();
        self.send_command(SidecarCommand::Shutdown { id })?;
        // Wait for the sidecar to finish its cleanup and exit by itself. `kill_on_drop` stays the
        // backstop, and on a timeout the process is killed here so the caller never waits longer.
        let mut child = self.child.lock().await;
        if tokio::time::timeout(self.shutdown_timeout, child.wait()).await.is_err() {
            let _ = child.kill().await;
        }
        Ok(())
    }

    async fn next_event(&self) -> Option<SidecarEvent> {
        let mut rx = self.events_rx.lock().await;
        rx.recv().await
    }
}
