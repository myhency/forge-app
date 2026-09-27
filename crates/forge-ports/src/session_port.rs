use std::path::Path;

use async_trait::async_trait;
use forge_domain::{Session, SessionId, SessionSummary};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum SessionRepositoryError {
    #[error("failed to list sessions: {0}")]
    ListFailed(String),
    #[error("session not found: {0:?}")]
    NotFound(SessionId),
    #[error("failed to load session: {0}")]
    LoadFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SessionRepositoryPort: Send + Sync {
    async fn list_sessions(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SessionSummary>, SessionRepositoryError>;
    async fn load_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<Session, SessionRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_list_sessions_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_list_sessions()
            .withf(|p: &Path| p == Path::new("/tmp/project"))
            .times(1)
            .returning(|_| Ok(vec![]));

        let result = mock.list_sessions(&PathBuf::from("/tmp/project")).await;
        assert!(result.is_ok());
    }
}
