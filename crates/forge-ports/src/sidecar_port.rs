use std::collections::BTreeMap;

use async_trait::async_trait;
use forge_domain::{ActiveExtensions, Attachment, AvailableModel, EffortLevel, PermissionDecision, PermissionMode, SessionDefaults, SlashCommand, TokenUsage};
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
    /// Permission rules that need no asking (the SDK's `allowedTools`): `Read`, `Bash(npm test:*)`.
    pub allowed_tools: Vec<String>,
    /// Variables the session's process runs with, over Forge's own environment. Values may be secrets.
    pub env: forge_domain::EnvVars,
    /// What the Code tab of Settings decides for this start.
    pub defaults: SessionDefaults,
}

/// One row of the SDK's context breakdown. `kind` is `used`, `free`, `buffer` or `deferred`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCategory {
    pub name: String,
    pub tokens: u64,
    pub kind: String,
}

/// An MCP tool (with its server) or an agent (no server) and what it costs in the context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextItemReport {
    #[serde(default)]
    pub server: Option<String>,
    pub name: String,
    pub tokens: u64,
}

/// One MCP server as the SDK reports it. `status` is the SDK's word, mapped later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerReport {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default, rename = "type")]
    pub transport: Option<String>,
    #[serde(default)]
    pub tool_count: u64,
    #[serde(default)]
    pub error: Option<String>,
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
    PermissionRequest {
        permission_request_id: String,
        tool_name: String,
        input: serde_json::Value,
        /// What "always allow" would add, as `allowed_tools` strings; empty when the SDK suggested none.
        #[serde(default)]
        rules: Vec<String>,
    },
    /// The SDK withdrew a request that was open (a stop denied it); its card goes. A finished turn
    /// does not do this: a workflow's agents keep asking after the turn that started it.
    PermissionCancelled { permission_request_id: String },
    /// What the turn added, per model.
    TurnComplete {
        #[serde(default)]
        usage: BTreeMap<String, TokenUsage>,
    },
    /// How full the context window is, sent after each `TurnComplete` when the SDK could say.
    ContextUsage { total_tokens: u64, max_tokens: u64 },
    /// The breakdown behind the context ring, sent right after `ContextUsage` when the SDK listed categories.
    ContextDetail {
        categories: Vec<ContextCategory>,
        #[serde(default)]
        mcp_tools: Vec<ContextItemReport>,
        #[serde(default)]
        agents: Vec<ContextItemReport>,
    },
    /// The state of every MCP server, sent after each `TurnComplete`; each event replaces the earlier one.
    McpStatus { servers: Vec<McpServerReport> },
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
    /// The SDK's whole set of background tasks; each event replaces the earlier one.
    BackgroundTasks { tasks: Vec<forge_domain::BackgroundTaskInfo> },
    /// A background task ended. The set still comes from `BackgroundTasks`; this says how it ended.
    /// `tokens` and `duration_ms` are what the SDK reported (a Bash command reports neither).
    TaskFinished {
        task_id: String,
        status: String,
        summary: String,
        #[serde(default)]
        tokens: Option<u64>,
        #[serde(default)]
        duration_ms: Option<u64>,
    },
    /// How many tokens a running background task has used so far.
    TaskProgress { task_id: String, total_tokens: u64 },
    /// The SDK's guess at what the person will ask next.
    PromptSuggestion { text: String },
    /// The conversation was compacted (by `/compact` or because the context was full).
    Compacted {
        #[serde(default)]
        pre_tokens: Option<u64>,
        #[serde(default)]
        post_tokens: Option<u64>,
    },
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
        reason: Option<String>,
    ) -> Result<(), SidecarError>;
    /// Answers a pending `AskUserQuestion` request: each question's text maps to the chosen
    /// label(s) (several labels joined by `,`) or the text typed under "Other".
    async fn answer_question(
        &self,
        permission_request_id: PermissionRequestId,
        answers: BTreeMap<String, String>,
    ) -> Result<(), SidecarError>;
    async fn interrupt(&self) -> Result<(), SidecarError>;
    /// Stops one background task. A refusal comes back as a `SettingFailed` event.
    async fn stop_task(&self, task_id: String) -> Result<(), SidecarError>;
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
                    value: "m".to_string(), resolved_model: None,
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
                allowed_tools: Vec::new(),
                env: forge_domain::EnvVars::default(),
                defaults: forge_domain::SessionDefaults::default(),
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
    fn context_detail_event_parses() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"context_detail","categories":[{"name":"Messages","tokens":20100,"kind":"used"}],"mcp_tools":[{"server":"Claude Docs","name":"Make","tokens":152}],"agents":[{"name":"Explore","tokens":80}]}"#,
        )
        .unwrap();
        assert_eq!(
            event,
            SidecarEvent::ContextDetail {
                categories: vec![ContextCategory { name: "Messages".to_string(), tokens: 20_100, kind: "used".to_string() }],
                mcp_tools: vec![ContextItemReport { server: Some("Claude Docs".to_string()), name: "Make".to_string(), tokens: 152 }],
                agents: vec![ContextItemReport { server: None, name: "Explore".to_string(), tokens: 80 }],
            }
        );
    }

    #[test]
    fn mcp_status_event_parses() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"mcp_status","servers":[{"name":"figma","status":"connected","scope":"user","source":"user","type":"http","tool_count":12,"error":null},{"name":"x","status":"failed","tool_count":0}]}"#,
        )
        .unwrap();
        assert_eq!(
            event,
            SidecarEvent::McpStatus {
                servers: vec![
                    McpServerReport {
                        name: "figma".to_string(),
                        status: "connected".to_string(),
                        scope: Some("user".to_string()),
                        source: Some("user".to_string()),
                        transport: Some("http".to_string()),
                        tool_count: 12,
                        error: None,
                    },
                    McpServerReport {
                        name: "x".to_string(),
                        status: "failed".to_string(),
                        scope: None,
                        source: None,
                        transport: None,
                        tool_count: 0,
                        error: None,
                    },
                ]
            }
        );
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

    #[test]
    fn background_task_and_suggestion_events_read_from_the_wire() {
        let tasks: SidecarEvent = serde_json::from_str(
            r#"{"type":"background_tasks","tasks":[{"task_id":"b1","description":"Sleep","task_type":"local_bash"}]}"#,
        )
        .unwrap();
        assert_eq!(
            tasks,
            SidecarEvent::BackgroundTasks {
                tasks: vec![forge_domain::BackgroundTaskInfo { id: "b1".to_string(), description: "Sleep".to_string(), task_type: "local_bash".to_string() }]
            }
        );
        let none: SidecarEvent = serde_json::from_str(r#"{"type":"background_tasks","tasks":[]}"#).unwrap();
        assert_eq!(none, SidecarEvent::BackgroundTasks { tasks: vec![] });
        let finished: SidecarEvent =
            serde_json::from_str(r#"{"type":"task_finished","task_id":"b1","status":"stopped","summary":"s"}"#).unwrap();
        assert_eq!(
            finished,
            SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "stopped".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None }
        );
        let timed: SidecarEvent =
            serde_json::from_str(r#"{"type":"task_finished","task_id":"w1","status":"failed","summary":"x","tokens":157469,"duration_ms":70914}"#).unwrap();
        assert_eq!(
            timed,
            SidecarEvent::TaskFinished { task_id: "w1".to_string(), status: "failed".to_string(), summary: "x".to_string(), tokens: Some(157_469), duration_ms: Some(70_914) }
        );
        let progress: SidecarEvent = serde_json::from_str(r#"{"type":"task_progress","task_id":"w1","total_tokens":99100}"#).unwrap();
        assert_eq!(progress, SidecarEvent::TaskProgress { task_id: "w1".to_string(), total_tokens: 99_100 });
        let suggestion: SidecarEvent = serde_json::from_str(r#"{"type":"prompt_suggestion","text":"run it"}"#).unwrap();
        assert_eq!(suggestion, SidecarEvent::PromptSuggestion { text: "run it".to_string() });
    }
}
