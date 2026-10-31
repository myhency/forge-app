use async_trait::async_trait;
use forge_domain::{Connection, NewConnector};
use thiserror::Error;

use crate::PluginManagerError;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpManagerError {
    /// The CLI ran and failed: its own message, with the values of the connector taken out.
    #[error("{0}")]
    Cli(String),
    #[error("The Claude program that comes with Forge was not found ({0}).")]
    BinaryMissing(String),
    #[error("The command took too long and was stopped.")]
    TimedOut,
    #[error("Could not read the output of claude: {0}")]
    BadOutput(String),
}

impl From<PluginManagerError> for McpManagerError {
    fn from(error: PluginManagerError) -> Self {
        match error {
            PluginManagerError::Cli(text) => McpManagerError::Cli(text),
            PluginManagerError::BinaryMissing(path) => McpManagerError::BinaryMissing(path),
            PluginManagerError::TimedOut => McpManagerError::TimedOut,
            PluginManagerError::BadOutput(text) => McpManagerError::BadOutput(text),
        }
    }
}

/// Adds, removes and checks MCP servers through the bundled Claude Code (the state the terminal's
/// Claude Code uses). Scope is always the user's.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait McpManagerPort: Send + Sync {
    async fn add(&self, connector: &NewConnector) -> Result<(), McpManagerError>;
    async fn remove(&self, name: &str) -> Result<(), McpManagerError>;
    /// One line about the connection. The output of the CLI is not kept: it prints header and env values.
    async fn check(&self, name: &str) -> Result<Connection, McpManagerError>;
    /// The shell line that starts the Claude Code Forge uses, with the config folder it uses. Typed into a
    /// terminal, so the person's `/mcp` sign-in lands in the state the checks read.
    fn launch_line(&self) -> Result<String, McpManagerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_a_remove_call() {
        let mut mock = MockMcpManagerPort::new();
        mock.expect_remove().withf(|name| name == "sentry").times(1).returning(|_| Ok(()));
        assert!(mock.remove("sentry").await.is_ok());
    }

    #[test]
    fn a_plugin_manager_error_keeps_its_meaning() {
        assert_eq!(McpManagerError::from(PluginManagerError::TimedOut), McpManagerError::TimedOut);
        assert_eq!(McpManagerError::from(PluginManagerError::Cli("x".into())).to_string(), "x");
    }
}
