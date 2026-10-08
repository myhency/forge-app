use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_domain::{
    ActiveExtensions, AppConfig, McpServerDefinition, PluginDefinition, SkillDefinition,
};
use forge_ports::{
    ConfigError, ConfigRepositoryPort, McpRegistryError, McpRegistryPort, PluginRepositoryError,
    PluginRepositoryPort, SkillRepositoryError, SkillRepositoryPort,
};
use thiserror::Error;

use super::config_lock::ConfigLock;
use super::toggles::{ToggleMcpServerUseCase, TogglePluginUseCase, ToggleSkillUseCase};

#[derive(Debug, Error)]
pub enum ExtensionsError {
    #[error("failed to read the Forge config: {0}")]
    Config(#[from] ConfigError),
    #[error("{0}")]
    Skills(#[from] SkillRepositoryError),
    #[error("{0}")]
    Plugins(#[from] PluginRepositoryError),
    #[error("{0}")]
    Mcp(#[from] McpRegistryError),
}

/// Everything about skills, plugins and MCP servers that the app needs: what a new session starts
/// with, what the settings tabs list, and the toggles. Definitions are read from the shared
/// `~/.claude` locations; which of them are on is Forge's own `enabled_*` lists (spec §4.3).
pub struct ExtensionsUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
    skills: Arc<dyn SkillRepositoryPort>,
    plugins: Arc<dyn PluginRepositoryPort>,
    mcp: Arc<dyn McpRegistryPort>,
    /// Read as "the project" when no project is active, so only global definitions show.
    global_scope: PathBuf,
    lock: ConfigLock,
}

impl ExtensionsUseCase {
    pub fn new(
        config: Arc<dyn ConfigRepositoryPort>,
        skills: Arc<dyn SkillRepositoryPort>,
        plugins: Arc<dyn PluginRepositoryPort>,
        mcp: Arc<dyn McpRegistryPort>,
        global_scope: PathBuf,
    ) -> Self {
        Self {
            config,
            skills,
            plugins,
            mcp,
            global_scope,
            lock: ConfigLock::default(),
        }
    }

    /// Shares the lock the other config writers hold (see `ConfigLock`).
    pub fn with_config_lock(mut self, lock: ConfigLock) -> Self {
        self.lock = lock;
        self
    }

    /// The enabled set for a session in `project`. A reader is only called when something of its
    /// kind is enabled, so a broken reader cannot block sessions for someone who uses none; when
    /// one is needed and fails, so does the start (a session never starts with fewer tools than
    /// were switched on).
    pub async fn resolve_active(
        &self,
        project: &Path,
    ) -> Result<ActiveExtensions, ExtensionsError> {
        let config = self.config.load().await?;
        let skills = if config.enabled_skills.is_empty() {
            Vec::new()
        } else {
            self.skills.list_skills(project).await?
        };
        let plugins = if config.enabled_plugins.is_empty() {
            Vec::new()
        } else {
            self.plugins.list_plugins().await?
        };
        let servers = if config.enabled_mcp_servers.is_empty() {
            Vec::new()
        } else {
            self.mcp.list_servers(project).await?
        };
        Ok(ActiveExtensions::select(
            &config, &skills, &plugins, &servers,
        ))
    }

    pub async fn load_config(&self) -> Result<AppConfig, ConfigError> {
        self.config.load().await
    }

    pub async fn list_skills(
        &self,
        project: Option<&Path>,
    ) -> Result<Vec<SkillDefinition>, SkillRepositoryError> {
        self.skills
            .list_skills(project.unwrap_or(&self.global_scope))
            .await
    }

    pub async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        self.plugins.list_plugins().await
    }

    pub async fn list_mcp_servers(
        &self,
        project: Option<&Path>,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        self.mcp
            .list_servers(project.unwrap_or(&self.global_scope))
            .await
    }

    pub async fn toggle_skill(&self, name: &str) -> Result<AppConfig, ConfigError> {
        ToggleSkillUseCase::new(self.config.clone())
            .with_config_lock(self.lock.clone())
            .execute(name)
            .await
    }

    /// `key` is `PluginDefinition::key()`.
    pub async fn toggle_plugin(&self, key: &str) -> Result<AppConfig, ConfigError> {
        TogglePluginUseCase::new(self.config.clone())
            .with_config_lock(self.lock.clone())
            .execute(key)
            .await
    }

    pub async fn toggle_mcp_server(&self, name: &str) -> Result<AppConfig, ConfigError> {
        ToggleMcpServerUseCase::new(self.config.clone())
            .with_config_lock(self.lock.clone())
            .execute(name)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
    use forge_ports::{
        MockConfigRepositoryPort, MockMcpRegistryPort, MockPluginRepositoryPort,
        MockSkillRepositoryPort,
    };

    fn skill(name: &str) -> SkillDefinition {
        SkillDefinition {
            name: name.to_string(),
            path: PathBuf::from(format!("/skills/{name}")),
            description: None,
        }
    }

    fn config_returning(config: AppConfig) -> MockConfigRepositoryPort {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(move || Ok(config.clone()));
        mock
    }

    fn use_case(
        config: MockConfigRepositoryPort,
        skills: MockSkillRepositoryPort,
        plugins: MockPluginRepositoryPort,
        mcp: MockMcpRegistryPort,
    ) -> ExtensionsUseCase {
        ExtensionsUseCase::new(
            Arc::new(config),
            Arc::new(skills),
            Arc::new(plugins),
            Arc::new(mcp),
            PathBuf::from("/home/me"),
        )
    }

    #[tokio::test]
    async fn resolve_active_keeps_only_what_is_enabled() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .times(1)
            .returning(|_| Ok(vec![skill("pdf"), skill("docx")]));
        let mut plugins = MockPluginRepositoryPort::new();
        plugins.expect_list_plugins().times(1).returning(|| {
            Ok(vec![PluginDefinition {
                name: "hud".to_string(),
                marketplace: Some("official".to_string()),
                path: PathBuf::from("/plugins/hud"),
            }])
        });
        let mut mcp = MockMcpRegistryPort::new();
        mcp.expect_list_servers().times(1).returning(|_| {
            Ok(vec![McpServerDefinition {
                name: "files".to_string(),
                config: serde_json::json!({"command": "npx"}),
            }])
        });

        let active = use_case(config_returning(config), skills, plugins, mcp)
            .resolve_active(Path::new("/work/a"))
            .await
            .unwrap();

        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/plugins/hud")]);
        assert_eq!(active.mcp_servers["files"]["command"], "npx");
    }

    #[tokio::test]
    async fn resolve_active_with_nothing_enabled_reads_no_definitions() {
        // The reader mocks have no expectations: any call would panic.
        let active = use_case(
            config_returning(AppConfig::default()),
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap();

        assert_eq!(active, ActiveExtensions::default());
    }

    #[tokio::test]
    async fn resolve_active_only_reads_the_kinds_that_have_something_enabled() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            ..AppConfig::default()
        };
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .times(1)
            .returning(|_| Ok(vec![skill("pdf")]));

        let active = use_case(
            config_returning(config),
            skills,
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap();

        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
    }

    #[tokio::test]
    async fn resolve_active_fails_when_a_needed_reader_fails() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            ..AppConfig::default()
        };
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .returning(|_| Err(SkillRepositoryError::ListFailed("boom".to_string())));

        let error = use_case(
            config_returning(config),
            skills,
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap_err();

        assert!(matches!(error, ExtensionsError::Skills(_)));
        assert!(error.to_string().contains("boom"), "{error}");
    }

    #[tokio::test]
    async fn resolve_active_fails_when_the_config_cannot_be_read() {
        let mut config = MockConfigRepositoryPort::new();
        config
            .expect_load()
            .returning(|| Err(ConfigError::ParseFailed("bad json".to_string())));

        let error = use_case(
            config,
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .resolve_active(Path::new("/work/a"))
        .await
        .unwrap_err();

        assert!(matches!(error, ExtensionsError::Config(_)));
    }

    #[tokio::test]
    async fn listing_without_a_project_reads_the_global_scope() {
        let mut skills = MockSkillRepositoryPort::new();
        skills
            .expect_list_skills()
            .withf(|path| path == Path::new("/home/me"))
            .times(1)
            .returning(|_| Ok(vec![]));
        let mut mcp = MockMcpRegistryPort::new();
        mcp.expect_list_servers()
            .withf(|path| path == Path::new("/work/a"))
            .times(1)
            .returning(|_| Ok(vec![]));

        let extensions = use_case(
            config_returning(AppConfig::default()),
            skills,
            MockPluginRepositoryPort::new(),
            mcp,
        );
        extensions.list_skills(None).await.unwrap();
        extensions
            .list_mcp_servers(Some(Path::new("/work/a")))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn toggling_a_plugin_stores_its_key_and_keeps_the_other_lists() {
        let mut config = MockConfigRepositoryPort::new();
        config.expect_load().returning(|| {
            Ok(AppConfig {
                enabled_skills: vec!["pdf".to_string()],
                ..AppConfig::default()
            })
        });
        config
            .expect_save()
            .withf(|c: &AppConfig| {
                c.enabled_plugins == vec!["hud@official".to_string()]
                    && c.enabled_skills == vec!["pdf".to_string()]
            })
            .times(1)
            .returning(|_| Ok(()));

        let saved = use_case(
            config,
            MockSkillRepositoryPort::new(),
            MockPluginRepositoryPort::new(),
            MockMcpRegistryPort::new(),
        )
        .toggle_plugin("hud@official")
        .await
        .unwrap();

        assert_eq!(saved.enabled_plugins, vec!["hud@official".to_string()]);
    }
}
