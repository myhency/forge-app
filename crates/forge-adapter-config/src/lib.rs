use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::AppConfig;
use forge_ports::{ConfigError, ConfigRepositoryPort};

pub fn default_config_path() -> Option<PathBuf> {
    directories::ProjectDirs::from("", "", "Forge")
        .map(|dirs| dirs.config_dir().join("config.json"))
}

pub struct ConfigFileAdapter {
    path: PathBuf,
}

impl ConfigFileAdapter {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

#[async_trait]
impl ConfigRepositoryPort for ConfigFileAdapter {
    async fn load(&self) -> Result<AppConfig, ConfigError> {
        match tokio::fs::read_to_string(&self.path).await {
            Ok(contents) => {
                serde_json::from_str(&contents).map_err(|e| ConfigError::ParseFailed(e.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(AppConfig::default()),
            Err(e) => Err(ConfigError::ReadFailed(e.to_string())),
        }
    }

    async fn save(&self, _config: &AppConfig) -> Result<(), ConfigError> {
        unimplemented!("implemented in Task 2")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn load_returns_default_config_when_file_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = ConfigFileAdapter::new(dir.path().join("does-not-exist.json"));

        let config = adapter.load().await.unwrap();
        assert_eq!(config, AppConfig::default());
    }

    #[tokio::test]
    async fn load_returns_parse_failed_for_malformed_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        tokio::fs::write(&path, "not valid json").await.unwrap();
        let adapter = ConfigFileAdapter::new(path);

        let result = adapter.load().await;
        assert!(matches!(result, Err(ConfigError::ParseFailed(_))));
    }

    #[tokio::test]
    async fn load_parses_an_existing_valid_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        tokio::fs::write(
            &path,
            r#"{"base_url":"https://api.anthropic.com","api_key":"sk-ant-test","enabled_skills":["brainstorming"],"enabled_plugins":[],"enabled_mcp_servers":[]}"#,
        )
        .await
        .unwrap();
        let adapter = ConfigFileAdapter::new(path);

        let config = adapter.load().await.unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://api.anthropic.com"));
        assert_eq!(config.enabled_skills, vec!["brainstorming".to_string()]);
    }

    #[test]
    fn default_config_path_ends_with_forge_config_json() {
        let path = default_config_path().expect("a config dir should be resolvable on this platform");
        assert_eq!(path.file_name().unwrap(), "config.json");
        assert!(path.to_string_lossy().contains("Forge") || path.to_string_lossy().contains("forge"));
    }
}
