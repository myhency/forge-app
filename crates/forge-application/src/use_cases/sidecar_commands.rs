use std::sync::Arc;

use forge_domain::{Attachment, PermissionDecision, Session};
use forge_ports::{SidecarError, SidecarPort};

use crate::event::push_user_message;

pub struct SendMessageUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl SendMessageUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        session: &mut Session,
        text: String,
        attachments: Vec<Attachment>,
    ) -> Result<(), SidecarError> {
        push_user_message(session, text.clone(), attachments.clone());
        self.sidecar.send_message(text, attachments).await
    }
}

pub struct ApprovePermissionUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl ApprovePermissionUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        self.sidecar.respond_to_permission(permission_request_id, decision).await
    }
}

pub struct InterruptTurnUseCase {
    sidecar: Arc<dyn SidecarPort>,
}

impl InterruptTurnUseCase {
    pub fn new(sidecar: Arc<dyn SidecarPort>) -> Self {
        Self { sidecar }
    }

    pub async fn execute(&self) -> Result<(), SidecarError> {
        self.sidecar.interrupt().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::MockSidecarPort;

    fn empty_session() -> Session {
        Session {
            id: forge_domain::SessionId("s-1".to_string()),
            project_path: std::path::PathBuf::from("/tmp/project"),
            title: None,
            messages: Vec::new(),
            permission_mode: forge_domain::PermissionMode::Default,
            usage: forge_domain::SessionUsage::default(),
        }
    }

    #[tokio::test]
    async fn send_message_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_send_message()
            .withf(|text: &String, _| text == "hello")
            .times(1)
            .returning(|_, _| Ok(()));

        let use_case = SendMessageUseCase::new(Arc::new(mock));
        let mut session = empty_session();
        assert!(use_case.execute(&mut session, "hello".to_string(), Vec::new()).await.is_ok());
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, forge_domain::MessageRole::User);
        assert_eq!(session.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn send_message_use_case_propagates_a_sidecar_error() {
        let mut mock = MockSidecarPort::new();
        mock.expect_send_message().returning(|_, _| Err(SidecarError::ProcessExited));

        let use_case = SendMessageUseCase::new(Arc::new(mock));
        let mut session = empty_session();
        assert!(matches!(
            use_case.execute(&mut session, "hi".to_string(), Vec::new()).await,
            Err(SidecarError::ProcessExited)
        ));
    }

    #[tokio::test]
    async fn approve_permission_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_respond_to_permission()
            .withf(|id: &String, decision: &PermissionDecision| {
                id == "perm-1" && *decision == PermissionDecision::AllowOnce
            })
            .times(1)
            .returning(|_, _| Ok(()));

        let use_case = ApprovePermissionUseCase::new(Arc::new(mock));
        assert!(use_case
            .execute("perm-1".to_string(), PermissionDecision::AllowOnce)
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn interrupt_turn_use_case_delegates_to_the_sidecar_port() {
        let mut mock = MockSidecarPort::new();
        mock.expect_interrupt().times(1).returning(|| Ok(()));

        let use_case = InterruptTurnUseCase::new(Arc::new(mock));
        assert!(use_case.execute().await.is_ok());
    }
}
