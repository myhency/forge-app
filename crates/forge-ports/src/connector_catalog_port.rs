use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::{ConnectorSnapshot, PluginEntry};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectorCatalogError {
    /// A file could not be read or is not JSON: the message names it.
    #[error("{0}")]
    Io(String),
}

/// The connectors the Customize page shows. Read-only and without side effects.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ConnectorCatalogPort: Send + Sync {
    /// Servers of the user's `~/.claude.json`, the project's `.mcp.json` and local scope (when there is a
    /// project), the installed plugins in `plugins`, and the plugins that marketplaces offer and are not installed.
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_a_snapshot_call() {
        let mut mock = MockConnectorCatalogPort::new();
        mock.expect_snapshot().times(1).returning(|_, _| Ok(ConnectorSnapshot::default()));
        assert!(mock.snapshot(None, vec![]).await.unwrap().connectors.is_empty());
    }
}
