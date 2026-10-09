#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Tells the person something while Forge is in the background. An error is a message for the log;
/// nothing depends on a notification arriving.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait NotifyPort: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;

    /// Bounces the Dock icon until the person comes back. Only macOS has one.
    fn request_attention(&self) -> Result<(), String> {
        Err("attention requests are not available here".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_request_for_attention_says_it_is_not_available() {
        struct Plain;
        impl NotifyPort for Plain {
            fn notify(&self, _: &str, _: &str) -> Result<(), String> {
                Ok(())
            }
        }
        assert!(Plain.request_attention().is_err());
    }

    #[test]
    fn the_mock_records_a_notification() {
        let mut mock = MockNotifyPort::new();
        mock.expect_notify().withf(|title, body| title == "Routine waiting" && body == "Review needs Bash").times(1).returning(|_, _| Ok(()));
        assert_eq!(mock.notify("Routine waiting", "Review needs Bash"), Ok(()));
    }
}
