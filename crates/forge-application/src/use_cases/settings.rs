use std::sync::Arc;

use forge_domain::{normalize_setting, AppConfig, ArchivedRef, SidebarPrefs};
use forge_ports::{ConfigError, ConfigRepositoryPort};

/// Reads and changes the settings the UI owns (`base_url`, `api_key`, `model`). Every save loads
/// the current config first, so the `enabled_*` lists are never lost.
pub struct SettingsUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
    // Every save reads the config, changes one part and writes it back; two at once would each
    // start from the same file and the later write would drop the earlier change.
    write_lock: tokio::sync::Mutex<()>,
}

impl SettingsUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config, write_lock: tokio::sync::Mutex::new(()) }
    }

    pub async fn load(&self) -> Result<AppConfig, ConfigError> {
        self.config.load().await
    }

    /// The model a new session should start with (`None`: the SDK chooses).
    pub async fn saved_model(&self) -> Result<Option<String>, ConfigError> {
        Ok(self.config.load().await?.model)
    }

    pub async fn save_connection(&self, base_url: &str, api_key: &str) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.load_for_update().await?;
        config.base_url = normalize_setting(base_url);
        config.api_key = normalize_setting(api_key);
        self.config.save(&config).await?;
        Ok(config)
    }

    pub async fn save_model(&self, model: &str) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.load_for_update().await?;
        config.model = normalize_setting(model);
        self.config.save(&config).await?;
        Ok(config)
    }

    /// Saves how the sidebar looks. Unlike the settings screen's saves, a config that cannot be read
    /// is never replaced: this runs on a click in the sidebar, and replacing the file would drop the
    /// API key without the person having asked for anything of the kind.
    pub async fn save_sidebar(&self, prefs: &SidebarPrefs) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.config.load().await?;
        config.sidebar = prefs.clone();
        self.config.save(&config).await?;
        Ok(config)
    }

    /// Saves which sessions are archived. Like `save_sidebar`, a config that cannot be read is never
    /// replaced, and it shares the write lock so a quick archive and filter change both survive.
    pub async fn save_archived(&self, archived: &[ArchivedRef]) -> Result<AppConfig, ConfigError> {
        let _saving = self.write_lock.lock().await;
        let mut config = self.config.load().await?;
        config.archived = archived.to_vec();
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
            ..AppConfig::default()
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

    #[tokio::test]
    async fn save_sidebar_replaces_only_the_sidebar_preferences() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.sidebar.sort_by == forge_domain::SortBy::Name
                    && c.sidebar.hidden
                    && c.base_url.as_deref() == Some("https://old.test")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));
        let usecase = SettingsUseCase::new(Arc::new(mock));
        let prefs = forge_domain::SidebarPrefs {
            sort_by: forge_domain::SortBy::Name,
            hidden: true,
            ..Default::default()
        };

        let saved = usecase.save_sidebar(&prefs).await.unwrap();

        assert_eq!(saved.sidebar, prefs);
    }

    #[tokio::test]
    async fn save_sidebar_does_not_replace_a_config_it_cannot_read() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("trailing comma".to_string())));
        // No `expect_save`: a save would panic. A sidebar click must never overwrite a config with
        // an API key in it just because the file could not be read this once.
        let usecase = SettingsUseCase::new(Arc::new(mock));

        let result = usecase.save_sidebar(&forge_domain::SidebarPrefs::default()).await;

        assert!(matches!(result, Err(ConfigError::ParseFailed(_))));
    }

    // A config kept in memory whose load and save each yield once, so two saves really interleave.
    struct SharedConfig {
        stored: std::sync::Mutex<AppConfig>,
    }

    #[async_trait::async_trait]
    impl forge_ports::ConfigRepositoryPort for SharedConfig {
        async fn load(&self) -> Result<AppConfig, ConfigError> {
            let snapshot = self.stored.lock().unwrap().clone();
            tokio::task::yield_now().await;
            Ok(snapshot)
        }

        async fn save(&self, config: &AppConfig) -> Result<(), ConfigError> {
            tokio::task::yield_now().await;
            *self.stored.lock().unwrap() = config.clone();
            Ok(())
        }
    }

    #[tokio::test]
    async fn two_saves_at_once_both_survive() {
        let shared = Arc::new(SharedConfig { stored: std::sync::Mutex::new(AppConfig::default()) });
        let usecase = SettingsUseCase::new(shared.clone());
        let prefs = forge_domain::SidebarPrefs { hidden: true, ..Default::default() };

        let (model, sidebar) = tokio::join!(usecase.save_model("claude-opus-5-5"), usecase.save_sidebar(&prefs));
        model.unwrap();
        sidebar.unwrap();

        let stored = shared.stored.lock().unwrap().clone();
        assert_eq!(stored.model.as_deref(), Some("claude-opus-5-5"), "the model save was lost");
        assert_eq!(stored.sidebar, prefs, "the sidebar save was lost");
    }

    #[tokio::test]
    async fn save_archived_replaces_only_the_archived_list() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(with_lists()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.archived.len() == 1
                    && c.archived[0].session_id == "s1"
                    && c.base_url.as_deref() == Some("https://old.test")
                    && c.enabled_skills == vec!["brainstorming".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));
        let usecase = SettingsUseCase::new(Arc::new(mock));
        let archived = vec![forge_domain::ArchivedRef { project: "/p".into(), session_id: "s1".to_string() }];

        let saved = usecase.save_archived(&archived).await.unwrap();

        assert_eq!(saved.archived, archived);
    }

    #[tokio::test]
    async fn save_archived_does_not_replace_a_config_it_cannot_read() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ParseFailed("bad".to_string())));
        mock.expect_save().times(0);

        let result = SettingsUseCase::new(Arc::new(mock)).save_archived(&[]).await;

        assert!(result.is_err());
    }
}
