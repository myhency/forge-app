use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct AppConfig {
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    /// The model new sessions start with; `None` lets the SDK choose.
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub enabled_skills: Vec<String>,
    #[serde(default)]
    pub enabled_plugins: Vec<String>,
    #[serde(default)]
    pub enabled_mcp_servers: Vec<String>,
}

/// What a settings text field is stored as: trimmed, and `None` when nothing is left.
pub fn normalize_setting(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_config_round_trips_through_json() {
        let config = AppConfig {
            base_url: Some("https://api.anthropic.com".to_string()),
            api_key: Some("sk-ant-test".to_string()),
            model: None,
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec![],
            enabled_mcp_servers: vec!["playwright".to_string()],
        };
        let json = serde_json::to_string(&config).unwrap();
        let back: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, back);
    }

    #[test]
    fn app_config_defaults_to_empty_lists_and_no_credentials() {
        let config = AppConfig::default();
        assert_eq!(config.base_url, None);
        assert_eq!(config.api_key, None);
        assert!(config.enabled_skills.is_empty());
        assert!(config.enabled_plugins.is_empty());
        assert!(config.enabled_mcp_servers.is_empty());
    }

    #[test]
    fn app_config_deserializes_from_json_missing_optional_lists() {
        let json = r#"{"base_url": null, "api_key": null}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert!(config.enabled_skills.is_empty());
    }

    #[test]
    fn a_config_file_without_a_model_loads_with_none() {
        let json = r#"{"base_url": null, "api_key": null, "enabled_skills": ["a"]}"#;
        let config: AppConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.model, None);
        assert_eq!(config.enabled_skills, vec!["a".to_string()]);
    }

    #[test]
    fn the_model_round_trips_through_json() {
        let config = AppConfig { model: Some("claude-sonnet-5-5".to_string()), ..AppConfig::default() };
        let back: AppConfig = serde_json::from_str(&serde_json::to_string(&config).unwrap()).unwrap();
        assert_eq!(back.model.as_deref(), Some("claude-sonnet-5-5"));
    }

    #[test]
    fn normalize_setting_trims_and_turns_blank_into_none() {
        assert_eq!(normalize_setting(""), None);
        assert_eq!(normalize_setting("   \n"), None);
        assert_eq!(normalize_setting(" sk-ant-1 \n"), Some("sk-ant-1".to_string()));
        assert_eq!(normalize_setting("https://x.test"), Some("https://x.test".to_string()));
    }
}
