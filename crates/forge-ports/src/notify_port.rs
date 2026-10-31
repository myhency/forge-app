#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// What gets the `target` of a clicked notification. (An alias, because the mock generator cannot read `Fn` types.)
pub type ClickHandler = Box<dyn Fn(String) + Send + Sync>;

/// Tells the person something while Forge is in the background. An error is a message for the log;
/// nothing depends on a notification arriving.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait NotifyPort: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<(), String>;

    /// Like `notify`, and the notification remembers `target` (an opaque text the caller reads back in
    /// `on_click`). A port that cannot tell about clicks sends a plain notification.
    fn notify_target(&self, title: &str, body: &str, _target: &str) -> Result<(), String> {
        self.notify(title, body)
    }

    /// `handler` gets the `target` of every notification the person clicks. A port whose notifications
    /// cannot be clicked never calls it.
    fn on_click(&self, _handler: ClickHandler) {}

    /// Bounces the Dock icon until the person comes back. Only macOS has one.
    fn request_attention(&self) -> Result<(), String> {
        Err("attention requests are not available here".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_port_without_clicks_sends_a_targeted_notice_as_a_plain_one_and_ignores_a_click_handler() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        struct Counting(AtomicUsize);
        impl NotifyPort for Counting {
            fn notify(&self, _: &str, _: &str) -> Result<(), String> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let port = Counting(AtomicUsize::new(0));
        assert_eq!(port.notify_target("t", "b", "live:1"), Ok(()));
        assert_eq!(port.0.load(Ordering::SeqCst), 1, "the default falls back to notify");
        port.on_click(Box::new(|_| panic!("a port that cannot be clicked never calls it")));
    }

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
