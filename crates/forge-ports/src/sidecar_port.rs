use std::collections::BTreeMap;

use async_trait::async_trait;
use forge_domain::{ActiveExtensions, Attachment, AvailableModel, EffortLevel, PermissionDecision, PermissionMode, SlashCommand, TokenUsage};
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
    /// The effort the session starts with; `None` is the SDK's default.
    pub effort: Option<EffortLevel>,
    /// The skills, plugins and MCP servers the session starts with.
    pub extensions: ActiveExtensions,
    /// Folders the session may use besides its own (the SDK's `additionalDirectories`).
    pub additional_directories: Vec<String>,
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
    /// The SDK accepted a `set_permission_mode`.
    PermissionModeSet { mode: PermissionMode },
    /// The SDK accepted a `set_model`; `None` is its default model.
    ModelSet { model: Option<String> },
    /// The SDK accepted a `set_effort`; `None` is its default.
    EffortSet { effort: Option<EffortLevel> },
    /// The models the SDK lists for this session and the effort levels each takes. Sent once after the start.
    ModelsAvailable { models: Vec<AvailableModel> },
    /// The SDK refused a `set_permission_mode`, `set_model` or `set_effort`. The session goes on.
    SettingFailed { message: String },
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
    async fn send_message(&self, text: String, attachments: Vec<Attachment>) -> Result<(), SidecarError>;
    async fn respond_to_permission(
        &self,
        permission_request_id: PermissionRequestId,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError>;
    async fn interrupt(&self) -> Result<(), SidecarError>;
    /// Asks for a mode change in the running session. The answer comes back as an event.
    async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), SidecarError>;
    /// Asks for a model change in the running session (`None`: the default). The answer comes back
    /// as an event.
    async fn set_model(&self, model: Option<String>) -> Result<(), SidecarError>;
    /// Asks for an effort change in the running session (`None`: the default). The answer comes back
    /// as an event.
    async fn set_effort(&self, effort: Option<EffortLevel>) -> Result<(), SidecarError>;
    async fn shutdown(&self) -> Result<(), SidecarError>;
    async fn next_event(&self) -> Option<SidecarEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_events_read_from_the_wire() {
        let set: SidecarEvent = serde_json::from_str(r#"{"type":"effort_set","effort":"xhigh"}"#).unwrap();
        assert_eq!(set, SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::XHigh) });
        let cleared: SidecarEvent = serde_json::from_str(r#"{"type":"effort_set","effort":null}"#).unwrap();
        assert_eq!(cleared, SidecarEvent::EffortSet { effort: None });
        let models: SidecarEvent = serde_json::from_str(
            r#"{"type":"models_available","models":[{"value":"m","display_name":"M","effort_levels":["low","high"]}]}"#,
        )
        .unwrap();
        assert_eq!(
            models,
            SidecarEvent::ModelsAvailable {
                models: vec![forge_domain::AvailableModel {
                    value: "m".to_string(),
                    display_name: "M".to_string(),
                    effort_levels: vec![forge_domain::EffortLevel::Low, forge_domain::EffortLevel::High],
                }]
            }
        );
    }

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
                effort: None,
                extensions: ActiveExtensions::default(),
                additional_directories: Vec::new(),
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
    fn the_answers_to_a_mode_or_model_change_parse() {
        let mode: SidecarEvent = serde_json::from_str(r#"{"type":"permission_mode_set","mode":"accept_edits"}"#).unwrap();
        let model: SidecarEvent = serde_json::from_str(r#"{"type":"model_set","model":"claude-opus-5-5"}"#).unwrap();
        let default: SidecarEvent = serde_json::from_str(r#"{"type":"model_set","model":null}"#).unwrap();
        let failed: SidecarEvent = serde_json::from_str(r#"{"type":"setting_failed","message":"nope"}"#).unwrap();

        assert_eq!(mode, SidecarEvent::PermissionModeSet { mode: PermissionMode::AcceptEdits });
        assert_eq!(model, SidecarEvent::ModelSet { model: Some("claude-opus-5-5".to_string()) });
        assert_eq!(default, SidecarEvent::ModelSet { model: None });
        assert_eq!(failed, SidecarEvent::SettingFailed { message: "nope".to_string() });
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
