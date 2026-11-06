use crate::{AppConfig, AutoModeModel, OutputStyle};

/// What the Code tab of Settings decides for a session at the moment its sidecar starts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionDefaults {
    pub output_style: Option<OutputStyle>,
    /// The model that approves tool calls in auto mode (`None`: Claude Code decides).
    pub auto_mode_model: Option<AutoModeModel>,
    pub prompt_suggestions: bool,
    /// Whether the SDK may enter bypass-permissions mode in this session.
    pub allow_bypass: bool,
    /// Whether the SDK gets the Workflow tool in this session.
    pub dynamic_workflows: bool,
    /// Whether the session keeps notes between sessions (the SDK's auto memory).
    pub auto_memory: bool,
    /// Whether the session gets the task list and next-step tools.
    pub turn_tools: bool,
    /// Whether the session gets the tool that draws an interactive page in the chat.
    pub generative_ui: bool,
    /// Whether the session gets the tool that reads and presses other apps (each call asks first).
    pub desktop_control: bool,
    /// What Claude does when the person asks for something unrelated to the session's work.
    pub unrelated_task: UnrelatedTask,
    /// Claude sorts the session into blocked, ready for review or done after each turn.
    pub classify_status: bool,
}

/// The desktop app's `관련 없는 작업을 요청할 때` (Settings → Claude Code, observed 2026-11-06):
/// `질문하기` (ask whether to start a separate session), `항상 여기서 실행`, `항상 새 세션 시작`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnrelatedTask {
    #[default]
    Ask,
    Here,
    NewSession,
}

impl UnrelatedTask {
    pub const ALL: [UnrelatedTask; 3] = [UnrelatedTask::Ask, UnrelatedTask::Here, UnrelatedTask::NewSession];

    pub fn value(self) -> &'static str {
        match self {
            UnrelatedTask::Ask => "ask",
            UnrelatedTask::Here => "here",
            UnrelatedTask::NewSession => "new_session",
        }
    }

    pub fn from_value(value: &str) -> Option<UnrelatedTask> {
        UnrelatedTask::ALL.into_iter().find(|choice| choice.value() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            UnrelatedTask::Ask => crate::msg!("Ask"),
            UnrelatedTask::Here => crate::msg!("Always run here"),
            UnrelatedTask::NewSession => crate::msg!("Always start a new session"),
        }
    }
}

impl Default for SessionDefaults {
    fn default() -> Self {
        Self { output_style: None, auto_mode_model: None, prompt_suggestions: true, allow_bypass: false, dynamic_workflows: false, auto_memory: true, turn_tools: true, generative_ui: true, desktop_control: false, unrelated_task: UnrelatedTask::Ask, classify_status: true }
    }
}

impl SessionDefaults {
    pub fn from_config(config: &AppConfig) -> Self {
        Self { output_style: config.output_style.clone(), auto_mode_model: config.auto_mode_model, prompt_suggestions: config.prompt_suggestions, allow_bypass: config.allow_bypass_mode, dynamic_workflows: config.dynamic_workflows, auto_memory: config.auto_memory, turn_tools: config.turn_tools, generative_ui: config.generative_ui, desktop_control: config.desktop_control, unrelated_task: config.unrelated_task, classify_status: config.classify_sessions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_config_gives_the_default_defaults() {
        assert_eq!(SessionDefaults::from_config(&AppConfig::default()), SessionDefaults::default());
    }

    #[test]
    fn auto_memory_is_carried_and_default_on() {
        assert!(SessionDefaults::default().auto_memory);
        let config = AppConfig { auto_memory: false, ..AppConfig::default() };
        assert!(!SessionDefaults::from_config(&config).auto_memory);
    }

    #[test]
    fn turn_tools_are_carried_and_default_on() {
        assert!(SessionDefaults::default().turn_tools);
        let config = AppConfig { turn_tools: false, ..AppConfig::default() };
        assert!(!SessionDefaults::from_config(&config).turn_tools);
    }

    #[test]
    fn the_config_values_are_carried() {
        let config = AppConfig { output_style: Some(OutputStyle::Learning), auto_mode_model: None, prompt_suggestions: false, allow_bypass_mode: true, dynamic_workflows: true, ..AppConfig::default() };
        assert_eq!(
            SessionDefaults::from_config(&config),
            SessionDefaults { output_style: Some(OutputStyle::Learning), auto_mode_model: None, prompt_suggestions: false, allow_bypass: true, dynamic_workflows: true, auto_memory: true, turn_tools: true, generative_ui: true, desktop_control: false, unrelated_task: UnrelatedTask::Ask, classify_status: true }
        );
    }
}
