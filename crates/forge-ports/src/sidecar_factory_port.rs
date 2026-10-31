use std::sync::Arc;

use async_trait::async_trait;

use crate::sidecar_port::{SidecarError, SidecarPort};

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Starts a fresh sidecar process for one session. `SidecarPort` itself stays single-session
/// (spec §3: one sidecar per open session); this port is how the application layer gets a new one.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SidecarFactoryPort: Send + Sync {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockSidecarPort;

    #[tokio::test]
    async fn mock_sidecar_factory_port_can_hand_out_a_sidecar() {
        let mut factory = MockSidecarFactoryPort::new();
        factory
            .expect_spawn()
            .times(1)
            .returning(|| Ok(Arc::new(MockSidecarPort::new())));

        let sidecar = factory.spawn().await;

        assert!(sidecar.is_ok());
    }

    #[tokio::test]
    async fn mock_sidecar_factory_port_can_fail_to_spawn() {
        let mut factory = MockSidecarFactoryPort::new();
        factory
            .expect_spawn()
            .times(1)
            .returning(|| Err(SidecarError::SpawnFailed("no node".to_string())));

        let result = factory.spawn().await;

        assert!(matches!(result, Err(SidecarError::SpawnFailed(_))));
    }
}
