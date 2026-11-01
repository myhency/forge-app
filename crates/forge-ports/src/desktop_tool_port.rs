use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Whether the `agent-desktop` program (which lets Claude read and press other apps) can be run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DesktopToolStatus {
    Missing,
    Installed { version: String },
}

/// Checks for the program and installs it where Forge keeps its tools. The error is a sentence for the person.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait DesktopToolPort: Send + Sync {
    async fn status(&self) -> Result<DesktopToolStatus, String>;
    async fn install(&self) -> Result<DesktopToolStatus, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_desktop_tool_can_report_missing_then_installed() {
        let mut mock = MockDesktopToolPort::new();
        mock.expect_status().returning(|| Ok(DesktopToolStatus::Missing));
        mock.expect_install().returning(|| Ok(DesktopToolStatus::Installed { version: "1.0.2".into() }));
        assert_eq!(mock.status().await, Ok(DesktopToolStatus::Missing));
        assert_eq!(mock.install().await, Ok(DesktopToolStatus::Installed { version: "1.0.2".into() }));
    }
}
