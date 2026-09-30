use std::sync::Arc;

use forge_domain::{normalize_setting, AppConfig};
use forge_ports::{ConfigError, ConfigRepositoryPort};

/// Reads and changes the settings the UI owns (`base_url`, `api_key`, `model`). Every save loads
/// the current config first, so the `enabled_*` lists are never lost.
pub struct SettingsUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl SettingsUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn load(&self) -> Result<AppConfig, ConfigError> {
        self.config.load().await
    }

    /// The model a new session should start with (`None`: the SDK chooses).
    pub async fn saved_model(&self) -> Result<Option<String>, ConfigError> {
        Ok(self.config.load().await?.model)
    }

    pub async fn save_connection(&self, base_url: &str, api_key: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.load_for_update().await?;
        config.base_url = normalize_setting(base_url);
        config.api_key = normalize_setting(api_key);
        self.config.save(&config).await?;
        Ok(config)
    }

    pub async fn save_model(&self, model: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.load_for_update().await?;
        config.model = normalize_setting(model);
        self.config.save(&config).await?;
        Ok(config)
    }

    // A file that is not valid JSON cannot be repaired by the settings screen, so saving replaces
    // it; any other failure (for example a permission error) is reported, never overwritten.
    async fn load_for_update(&self) -> Result<AppConfig, ConfigError> {
        match self.config.load().await {
            Err(ConfigError::ParseFailed(_)) => Ok(AppConfig::default()),
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockConfigRepositoryPort;

    fn with_lists() -> AppConfig {
        AppConfig {
            base_url: Some("https://old.test".to_string()),
            api_key: Some("old-key".to_string()),
            model: Some("old-model".to_string()),
            enabled_skills: vec!["brainstorming".to_string()],
            enabled_plugins: vec!["p@m".to_string()],
            enabled_mcp_servers: vec!["playwright".to_string()],
        }
    }

    #[tokio::test]
    async fn save_connection_replaces_only_the_url_and_the_key() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.base_url.as_deref() == Some("https://new.test")
                    && c.api_key.as_deref() == Some("new-key")
                    && c.model.as_deref() == Some("old-model")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
                    && c.enabled_plugins == vec!["p@m".to_string()]
                    && c.enabled_mcp_servers == vec!["playwright".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        let saved = SettingsUseCase::new(Arc::new(mock))
            .save_connection(" https://new.test ", "new-key\n")
            .await
            .unwrap();
        assert_eq!(saved.api_key.as_deref(), Some("new-key"));
    }

    #[tokio::test]
    async fn a_blank_field_clears_the_value() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.base_url.is_none() && c.api_key.is_none())
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_connection("", "  ").await.unwrap();
    }

    #[tokio::test]
    async fn save_model_replaces_only_the_model() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.model.as_deref() == Some("claude-opus-5-5")
                    && c.api_key.as_deref() == Some("old-key")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_model("claude-opus-5-5").await.unwrap();
    }

    #[tokio::test]
    async fn an_empty_model_means_the_sdk_default() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save().withf(|c: &AppConfig| c.model.is_none()).times(1).returning(|_| Ok(()));

        let saved = SettingsUseCase::new(Arc::new(mock)).save_model("").await.unwrap();
        assert_eq!(saved.model, None);
    }

    #[tokio::test]
    async fn a_config_file_that_does_not_parse_is_replaced_on_save() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("expected value".to_string())));
        mock.expect_save()
            .withf(|c: &AppConfig| c.api_key.as_deref() == Some("k") && c.enabled_skills.is_empty())
            .times(1)
            .returning(|_| Ok(()));

        SettingsUseCase::new(Arc::new(mock)).save_connection("", "k").await.unwrap();
    }

    #[tokio::test]
    async fn a_read_error_is_returned_and_nothing_is_saved() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Err(ConfigError::ReadFailed("denied".to_string())));
        mock.expect_save().times(0);

        assert!(SettingsUseCase::new(Arc::new(mock)).save_model("m").await.is_err());
    }

    #[tokio::test]
    async fn a_write_error_is_returned() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .times(1)
            .returning(|_| Err(ConfigError::WriteFailed("disk full".to_string())));

        assert!(SettingsUseCase::new(Arc::new(mock)).save_model("m").await.is_err());
    }

    #[tokio::test]
    async fn saved_model_reads_the_model_from_the_config() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));

        let model = SettingsUseCase::new(Arc::new(mock)).saved_model().await.unwrap();
        assert_eq!(model.as_deref(), Some("old-model"));
    }

    #[tokio::test]
    async fn saved_model_fails_when_the_config_cannot_be_loaded() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("bad".to_string())));

        assert!(SettingsUseCase::new(Arc::new(mock)).saved_model().await.is_err());
    }
}
