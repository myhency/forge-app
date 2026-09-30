use std::path::Path;
use std::sync::Arc;

use forge_domain::{PermissionMode, Session, SessionId};
use forge_ports::{
    SessionRepositoryError, SessionRepositoryPort, SidecarError, SidecarPort, StartSessionParams,
};
use thiserror::Error;

pub struct StartSessionUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl StartSessionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        cwd: String,
        permission_mode: PermissionMode,
        model: Option<String>,
    ) -> Result<(), SidecarError> {
        self.sidecar
            .start_session(StartSessionParams {
                cwd,
                resume_session_id: None,
                permission_mode,
                model,
                extensions: Default::default(),
            })
            .await
    }
}

#[derive(Debug, Error)]
pub enum ResumeSessionError {
    #[error("failed to load session history: {0}")]
    SessionRepository(#[from] SessionRepositoryError),
    #[error("failed to resume sidecar session: {0}")]
    Sidecar(#[from] SidecarError),
}

pub struct ResumeSessionUseCase {
    sidecar: Arc<dyn SidecarPort>,
    sessions: Arc<dyn SessionRepositoryPort>,
}

impl ResumeSessionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>, sessions: Arc<dyn SessionRepositoryPort>) -> Self {
        Self { sidecar, sessions }
    }

    pub async fn execute(
        &self,
        project_path: &Path,
        session_id: SessionId,
        permission_mode: PermissionMode,
    ) -> Result<Session, ResumeSessionError> {
        let mut session = self.sessions.load_session(project_path, &session_id).await?;

        self.sidecar
            .start_session(StartSessionParams {
                cwd: project_path.display().to_string(),
                resume_session_id: Some(session_id.0.clone()),
                permission_mode,
                model: None,
                extensions: Default::default(),
            })
            .await?;

        session.permission_mode = permission_mode;
        Ok(session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::{MockSessionRepositoryPort, MockSidecarPort};
    use std::path::PathBuf;

    #[tokio::test]
    async fn start_session_use_case_delegates_to_the_sidecar_port_with_no_resume_id() {
        let mut mock = MockSidecarPort::new();
        mock.expect_start_session()
            .withf(|p: &StartSessionParams| p.cwd == "/tmp/project" && p.resume_session_id.is_none())
            .times(1)
            .returning(|_| Ok(()));

        let use_case = StartSessionUseCase::new(Arc::new(mock));
        assert!(use_case
            .execute("/tmp/project".to_string(), PermissionMode::Default, None)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn resume_session_use_case_loads_history_then_starts_the_sidecar_with_the_resume_id() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock.expect_load_session().times(1).returning(|_, id| {
            Ok(Session {
                id: id.clone(),
                project_path: PathBuf::from("/tmp/project"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            })
        });

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock
            .expect_start_session()
            .withf(|p: &StartSessionParams| p.resume_session_id.as_deref() == Some("s-1"))
            .times(1)
            .returning(|_| Ok(()));

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().id, SessionId("s-1".to_string()));
    }

    #[tokio::test]
    async fn resume_session_use_case_does_not_start_the_sidecar_if_history_fails_to_load() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock
            .expect_load_session()
            .times(1)
            .returning(|_, id| Err(SessionRepositoryError::NotFound(id.clone())));

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock.expect_start_session().times(0);

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(matches!(result, Err(ResumeSessionError::SessionRepository(_))));
    }

    #[tokio::test]
    async fn resume_session_use_case_returns_a_sidecar_error_even_after_history_loads_successfully() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock.expect_load_session().times(1).returning(|_, id| {
            Ok(Session {
                id: id.clone(),
                project_path: PathBuf::from("/tmp/project"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            })
        });

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock
            .expect_start_session()
            .times(1)
            .returning(|_| Err(SidecarError::ProcessExited));

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::Default)
            .await;

        assert!(matches!(result, Err(ResumeSessionError::Sidecar(_))));
    }

    #[tokio::test]
    async fn resume_session_use_case_overwrites_the_returned_session_permission_mode() {
        let mut sessions_mock = MockSessionRepositoryPort::new();
        sessions_mock.expect_load_session().times(1).returning(|_, id| {
            Ok(Session {
                id: id.clone(),
                project_path: PathBuf::from("/tmp/project"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Plan,
            })
        });

        let mut sidecar_mock = MockSidecarPort::new();
        sidecar_mock.expect_start_session().times(1).returning(|_| Ok(()));

        let use_case = ResumeSessionUseCase::new(Arc::new(sidecar_mock), Arc::new(sessions_mock));
        let result = use_case
            .execute(&PathBuf::from("/tmp/project"), SessionId("s-1".to_string()), PermissionMode::AcceptEdits)
            .await
            .unwrap();

        assert_eq!(result.permission_mode, PermissionMode::AcceptEdits);
    }
}
