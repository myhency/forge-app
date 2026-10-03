use std::collections::BTreeMap;

use async_trait::async_trait;
use forge_domain::{ActiveExtensions, PermissionDecision, PermissionMode, SlashCommand, TokenUsage};
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
    /// The skills, plugins and MCP servers the session starts with.
    pub extensions: ActiveExtensions,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarEvent {
    SessionStarted { session_id: String },
    AssistantDelta { text: String },
    /// A piece of the model's thinking, streamed before its answer.
    ThinkingDelta { text: String },
    ToolUse { tool_call_id: String, name: String, input: serde_json::Value },
    ToolResult { tool_call_id: String, output: String, is_error: bool },
    PermissionRequest { permission_request_id: String, tool_name: String, input: serde_json::Value },
    /// What the turn added, per model.
    TurnComplete {
        #[serde(default)]
        usage: BTreeMap<String, TokenUsage>,
    },
    /// How full the context window is, sent after each `TurnComplete` when the SDK could say.
    ContextUsage { total_tokens: u64, max_tokens: u64 },
    /// The slash commands the SDK reports; each event replaces the earlier list.
    CommandsAvailable { commands: Vec<SlashCommand> },
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
                extensions: ActiveExtensions::default(),
            })
            .await;

        assert!(result.is_ok());
    }

    #[test]
    fn turn_complete_carries_per_model_usage() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"turn_complete","usage":{"claude-test":{"input":12,"output":34,"cache_read":5,"cache_write_5m":6,"cache_write_1h":0,"web_search_requests":2}}}"#,
        )
        .unwrap();
        let SidecarEvent::TurnComplete { usage } = event else { panic!("not a turn_complete") };
        assert_eq!(
            usage["claude-test"],
            forge_domain::TokenUsage { input: 12, output: 34, cache_read: 5, cache_write_5m: 6, cache_write_1h: 0, web_search_requests: 2 }
        );
    }

    #[test]
    fn turn_complete_with_no_usage_is_still_a_turn_complete() {
        let event: SidecarEvent = serde_json::from_str(r#"{"type":"turn_complete","usage":{}}"#).unwrap();
        assert_eq!(event, SidecarEvent::TurnComplete { usage: Default::default() });
    }

    #[test]
    fn context_usage_event_parses() {
        let event: SidecarEvent =
            serde_json::from_str(r#"{"type":"context_usage","total_tokens":76200,"max_tokens":200000}"#).unwrap();
        assert_eq!(event, SidecarEvent::ContextUsage { total_tokens: 76_200, max_tokens: 200_000 });
    }

    #[test]
    fn commands_available_parses_with_and_without_the_optional_fields() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"commands_available","commands":[{"name":"compact","description":"Free up context","argument_hint":"[text]"},{"name":"review"}]}"#,
        )
        .unwrap();
        assert_eq!(
            event,
            SidecarEvent::CommandsAvailable {
                commands: vec![
                    forge_domain::SlashCommand {
                        name: "compact".to_string(),
                        description: Some("Free up context".to_string()),
                        argument_hint: Some("[text]".to_string()),
                    },
                    forge_domain::SlashCommand { name: "review".to_string(), description: None, argument_hint: None },
                ]
            }
        );
    }
}
