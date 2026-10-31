use async_trait::async_trait;
use forge_domain::AppConfig;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    ReadFailed(String),
    #[error("failed to write config file: {0}")]
    WriteFailed(String),
    #[error("failed to parse config file: {0}")]
    ParseFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ConfigRepositoryPort: Send + Sync {
    async fn load(&self) -> Result<AppConfig, ConfigError>;
    async fn save(&self, config: &AppConfig) -> Result<(), ConfigError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_config_repository_port_can_record_a_load_call() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().times(1).returning(|| Ok(AppConfig::default()));

        let result = mock.load().await;
        assert!(result.is_ok());
    }
}
