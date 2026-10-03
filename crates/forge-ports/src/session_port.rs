use std::path::Path;

use async_trait::async_trait;
use forge_domain::{SearchHit, Session, SessionId, SessionSummary};
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
    #[error("failed to rename session: {0}")]
    RenameFailed(String),
    #[error("failed to delete session: {0}")]
    DeleteFailed(String),
    #[error("failed to fork session: {0}")]
    ForkFailed(String),
    #[error("failed to search sessions: {0}")]
    SearchFailed(String),
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
    /// Gives the session a title of the user's choosing. The title is stored in the session's own
    /// transcript, so Claude Code shows it too.
    async fn rename_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<(), SessionRepositoryError>;
    /// Moves the session's transcript (and the data kept beside it) to the OS trash. Nothing is
    /// removed permanently.
    async fn delete_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
    ) -> Result<(), SessionRepositoryError>;
    /// Copies the session's whole transcript into a new session of the same project and gives the
    /// copy `title`. The source is not touched. Returns the new session's id.
    async fn fork_session(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<SessionId, SessionRepositoryError>;
    /// Sessions of every project whose title or message text contains `query` (any case), newest
    /// first, at most `limit`. A blank query finds nothing. Only what the user and the assistant
    /// wrote is searched, not tool input, tool results or thinking.
    async fn search_sessions(&self, query: &str, limit: usize) -> Result<Vec<SearchHit>, SessionRepositoryError>;
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

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_rename_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_rename_session()
            .withf(|p: &Path, id: &SessionId, title: &str| {
                p == Path::new("/tmp/project") && id.0 == "s-1" && title == "New title"
            })
            .times(1)
            .returning(|_, _, _| Ok(()));

        let result = mock
            .rename_session(&PathBuf::from("/tmp/project"), &SessionId("s-1".to_string()), "New title")
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_delete_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_delete_session().times(1).returning(|_, _| Ok(()));

        let result = mock
            .delete_session(&PathBuf::from("/tmp/project"), &SessionId("s-1".to_string()))
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn delete_failed_message_names_the_cause() {
        let error = SessionRepositoryError::DeleteFailed("no trash".to_string());
        assert_eq!(error.to_string(), "failed to delete session: no trash");
    }

    #[test]
    fn rename_failed_message_names_the_cause() {
        let error = SessionRepositoryError::RenameFailed("disk full".to_string());
        assert_eq!(error.to_string(), "failed to rename session: disk full");
    }

    #[test]
    fn search_failed_message_names_the_cause() {
        let error = SessionRepositoryError::SearchFailed("no access".to_string());
        assert_eq!(error.to_string(), "failed to search sessions: no access");
    }

    #[tokio::test]
    async fn mock_session_repository_port_can_record_a_search_call() {
        let mut mock = MockSessionRepositoryPort::new();
        mock.expect_search_sessions()
            .withf(|query: &str, limit: &usize| query == "borrow" && *limit == 50)
            .times(1)
            .returning(|_, _| Ok(vec![]));

        assert!(mock.search_sessions("borrow", 50).await.unwrap().is_empty());
    }
}
