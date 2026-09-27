use std::sync::Arc;

use forge_domain::AppConfig;
use forge_ports::{ConfigError, ConfigRepositoryPort};

fn toggle_membership(list: &mut Vec<String>, name: &str) {
    if let Some(pos) = list.iter().position(|existing| existing == name) {
        list.remove(pos);
    } else {
        list.push(name.to_string());
    }
}

pub struct ToggleSkillUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl ToggleSkillUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_skills, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

pub struct TogglePluginUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl TogglePluginUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_plugins, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

pub struct ToggleMcpServerUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
}

impl ToggleMcpServerUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config }
    }

    pub async fn execute(&self, name: &str) -> Result<AppConfig, ConfigError> {
        let mut config = self.config.load().await?;
        toggle_membership(&mut config.enabled_mcp_servers, name);
        self.config.save(&config).await?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockConfigRepositoryPort;

    #[tokio::test]
    async fn toggle_skill_use_case_adds_a_skill_that_was_not_enabled() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_skills == vec!["brainstorming".to_string()])
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        let result = use_case.execute("brainstorming").await.unwrap();
        assert_eq!(result.enabled_skills, vec!["brainstorming".to_string()]);
    }

    #[tokio::test]
    async fn toggle_skill_use_case_removes_a_skill_that_was_already_enabled() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| {
            Ok(AppConfig {
                enabled_skills: vec!["brainstorming".to_string()],
                ..AppConfig::default()
            })
        });
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_skills.is_empty())
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        let result = use_case.execute("brainstorming").await.unwrap();
        assert!(result.enabled_skills.is_empty());
    }

    #[tokio::test]
    async fn toggle_plugin_use_case_only_touches_enabled_plugins() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| {
                c.enabled_plugins == vec!["playwright".to_string()] && c.enabled_skills.is_empty()
            })
            .times(1)
            .returning(|_| Ok(()));

        let use_case = TogglePluginUseCase::new(Arc::new(mock));
        let result = use_case.execute("playwright").await.unwrap();
        assert_eq!(result.enabled_plugins, vec!["playwright".to_string()]);
    }

    #[tokio::test]
    async fn toggle_mcp_server_use_case_only_touches_enabled_mcp_servers() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .withf(|c: &AppConfig| c.enabled_mcp_servers == vec!["playwright-mcp".to_string()])
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ToggleMcpServerUseCase::new(Arc::new(mock));
        let result = use_case.execute("playwright-mcp").await.unwrap();
        assert_eq!(result.enabled_mcp_servers, vec!["playwright-mcp".to_string()]);
    }

    #[tokio::test]
    async fn toggle_skill_use_case_propagates_a_load_error_without_calling_save() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load()
            .times(1)
            .returning(|| Err(ConfigError::ReadFailed("disk error".to_string())));
        mock.expect_save().times(0);

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        assert!(use_case.execute("brainstorming").await.is_err());
    }

    #[tokio::test]
    async fn toggle_skill_use_case_propagates_a_save_error() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));
        mock.expect_save()
            .times(1)
            .returning(|_| Err(ConfigError::WriteFailed("disk full".to_string())));

        let use_case = ToggleSkillUseCase::new(Arc::new(mock));
        assert!(use_case.execute("brainstorming").await.is_err());
    }
}
