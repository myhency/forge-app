use async_trait::async_trait;
use forge_domain::PluginSnapshot;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginManagerError {
    /// The CLI ran and failed: its own message, as it is.
    #[error("{0}")]
    Cli(String),
    /// The `claude` program that ships with the SDK is not where it should be.
    #[error("The Claude program that comes with Forge was not found ({0}).")]
    BinaryMissing(String),
    #[error("The command took too long and was stopped.")]
    TimedOut,
    /// The CLI printed something that is not the JSON this app expects.
    #[error("Could not read the output of claude: {0}")]
    BadOutput(String),
}

/// Plugins and marketplaces, managed by the bundled Claude Code (the same state the terminal's
/// Claude Code uses). `id` is `name@marketplace`. Install scope is always the user's.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait PluginManagerPort: Send + Sync {
    async fn snapshot(&self) -> Result<PluginSnapshot, PluginManagerError>;
    /// `source` is `owner/repo`, a Git URL or a folder.
    async fn add_marketplace(&self, source: &str) -> Result<(), PluginManagerError>;
    async fn remove_marketplace(&self, name: &str) -> Result<(), PluginManagerError>;
    async fn update_marketplace(&self, name: &str) -> Result<(), PluginManagerError>;
    async fn install(&self, id: &str) -> Result<(), PluginManagerError>;
    async fn uninstall(&self, id: &str) -> Result<(), PluginManagerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_an_install_call() {
        let mut mock = MockPluginManagerPort::new();
        mock.expect_install().withf(|id| id == "a@b").times(1).returning(|_| Ok(()));
        assert!(mock.install("a@b").await.is_ok());
    }

    #[test]
    fn a_cli_error_shows_its_own_text() {
        assert_eq!(PluginManagerError::Cli("Path does not exist: /x".into()).to_string(), "Path does not exist: /x");
    }
}
