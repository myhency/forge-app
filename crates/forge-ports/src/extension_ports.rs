use std::path::Path;

use async_trait::async_trait;
use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum SkillRepositoryError {
    #[error("failed to list skills: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SkillRepositoryPort: Send + Sync {
    async fn list_skills(&self, project_path: &Path) -> Result<Vec<SkillDefinition>, SkillRepositoryError>;
}

#[derive(Debug, Error)]
pub enum PluginRepositoryError {
    #[error("failed to list plugins: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait PluginRepositoryPort: Send + Sync {
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError>;
}

#[derive(Debug, Error)]
pub enum McpRegistryError {
    #[error("failed to list mcp servers: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait McpRegistryPort: Send + Sync {
    async fn list_servers(&self, project_path: &Path) -> Result<Vec<McpServerDefinition>, McpRegistryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn mock_skill_repository_port_can_record_a_list_skills_call() {
        let mut mock = MockSkillRepositoryPort::new();
        mock.expect_list_skills().times(1).returning(|_| Ok(vec![]));
        let result = mock.list_skills(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_plugin_repository_port_can_record_a_list_plugins_call() {
        let mut mock = MockPluginRepositoryPort::new();
        mock.expect_list_plugins().times(1).returning(|| Ok(vec![]));
        let result = mock.list_plugins().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_mcp_registry_port_can_record_a_list_servers_call() {
        let mut mock = MockMcpRegistryPort::new();
        mock.expect_list_servers().times(1).returning(|_| Ok(vec![]));
        let result = mock.list_servers(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }
}
