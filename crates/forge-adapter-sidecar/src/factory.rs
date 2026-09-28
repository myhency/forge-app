use std::sync::Arc;

use async_trait::async_trait;
use forge_domain::AppConfig;
use forge_ports::{ConfigRepositoryPort, SidecarError, SidecarFactoryPort, SidecarPort};

use crate::process::SidecarProcess;

/// Spawns one Node sidecar per session, with Forge's own credentials in its environment.
pub struct SidecarProcessFactory {
    node_binary: String,
    script_path: String,
    config: Arc<dyn ConfigRepositoryPort>,
}

impl SidecarProcessFactory {
    pub fn new(node_binary: String, script_path: String, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { node_binary, script_path, config }
    }
}

#[async_trait]
impl SidecarFactoryPort for SidecarProcessFactory {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError> {
        // Loaded on every spawn, so an edit to config.json applies to the next session opened.
        // A missing config file is not an error (the adapter returns `AppConfig::default()`); a
        // malformed one is, so a broken config is reported instead of silently ignored.
        let config = self
            .config
            .load()
            .await
            .map_err(|e| SidecarError::SpawnFailed(format!("failed to load the Forge config: {e}")))?;
        let process = SidecarProcess::spawn(&self.node_binary, &self.script_path, &credentials_env(&config))?;
        Ok(Arc::new(process))
    }
}

// Only values that are set and non-empty are injected; everything else is inherited from the
// parent environment. Never format `config` itself (its `Debug` output contains the API key).
pub(crate) fn credentials_env(config: &AppConfig) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(api_key) = config.api_key.as_deref().filter(|value| !value.is_empty()) {
        env.push(("ANTHROPIC_API_KEY".to_string(), api_key.to_string()));
    }
    if let Some(base_url) = config.base_url.as_deref().filter(|value| !value.is_empty()) {
        env.push(("ANTHROPIC_BASE_URL".to_string(), base_url.to_string()));
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(api_key: Option<&str>, base_url: Option<&str>) -> AppConfig {
        AppConfig {
            api_key: api_key.map(str::to_string),
            base_url: base_url.map(str::to_string),
            ..AppConfig::default()
        }
    }

    #[test]
    fn credentials_env_injects_both_values_when_both_are_set() {
        let env = credentials_env(&config(Some("sk-test"), Some("https://proxy.example.test")));

        assert_eq!(
            env,
            vec![
                ("ANTHROPIC_API_KEY".to_string(), "sk-test".to_string()),
                ("ANTHROPIC_BASE_URL".to_string(), "https://proxy.example.test".to_string()),
            ]
        );
    }

    #[test]
    fn credentials_env_injects_nothing_for_a_default_config() {
        assert!(credentials_env(&AppConfig::default()).is_empty());
    }

    #[test]
    fn credentials_env_treats_empty_strings_as_unset() {
        assert!(credentials_env(&config(Some(""), Some(""))).is_empty());
    }

    #[test]
    fn credentials_env_injects_only_the_value_that_is_set() {
        let env = credentials_env(&config(None, Some("https://proxy.example.test")));

        assert_eq!(
            env,
            vec![("ANTHROPIC_BASE_URL".to_string(), "https://proxy.example.test".to_string())]
        );
    }
}
