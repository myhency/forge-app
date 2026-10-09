#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Tells the person something while Forge is in the background. An error is a message for the log;
/// nothing depends on a notification arriving.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait NotifyPort: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_records_a_notification() {
        let mut mock = MockNotifyPort::new();
        mock.expect_notify().withf(|title, body| title == "Routine waiting" && body == "Review needs Bash").times(1).returning(|_, _| Ok(()));
        assert_eq!(mock.notify("Routine waiting", "Review needs Bash"), Ok(()));
    }
}
