use async_trait::async_trait;
use forge_domain::ProjectSummary;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum ProjectRepositoryError {
    #[error("failed to list projects: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ProjectRepositoryPort: Send + Sync {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_project_repository_port_can_record_a_list_projects_call() {
        let mut mock = MockProjectRepositoryPort::new();
        mock.expect_list_projects().times(1).returning(|| Ok(vec![]));

        let result = mock.list_projects().await;

        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn list_failed_message_names_the_cause() {
        let error = ProjectRepositoryError::ListFailed("boom".to_string());
        assert_eq!(error.to_string(), "failed to list projects: boom");
    }
}
