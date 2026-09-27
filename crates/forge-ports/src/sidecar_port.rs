use async_trait::async_trait;
use forge_domain::{PermissionDecision, PermissionMode};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

pub type PermissionRequestId = String;

#[derive(Debug, Clone, PartialEq)]
pub struct StartSessionParams {
    pub cwd: String,
    pub resume_session_id: Option<String>,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarEvent {
    SessionStarted { session_id: String },
    AssistantDelta { text: String },
    ToolUse { tool_call_id: String, name: String, input: serde_json::Value },
    ToolResult { tool_call_id: String, output: String, is_error: bool },
    PermissionRequest { permission_request_id: String, tool_name: String, input: serde_json::Value },
    TurnComplete { input_tokens: u32, output_tokens: u32 },
    Error { message: String },
    SessionEnded,
}

#[derive(Debug, Error)]
pub enum SidecarError {
    #[error("failed to spawn sidecar process: {0}")]
    SpawnFailed(String),
    #[error("sidecar process exited unexpectedly")]
    ProcessExited,
    #[error("failed to write command to sidecar: {0}")]
    WriteFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SidecarPort: Send + Sync {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError>;
    async fn send_message(&self, text: String) -> Result<(), SidecarError>;
    async fn respond_to_permission(
        &self,
        permission_request_id: PermissionRequestId,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError>;
    async fn interrupt(&self) -> Result<(), SidecarError>;
    async fn shutdown(&self) -> Result<(), SidecarError>;
    async fn next_event(&self) -> Option<SidecarEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_sidecar_port_can_record_a_start_session_call() {
        let mut mock = MockSidecarPort::new();
        mock.expect_start_session()
            .withf(|p: &StartSessionParams| p.cwd == "/tmp/project")
            .times(1)
            .returning(|_| Ok(()));

        let result = mock
            .start_session(StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
            })
            .await;

        assert!(result.is_ok());
    }
}
