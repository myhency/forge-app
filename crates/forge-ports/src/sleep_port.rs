#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Keeps the computer awake while something holds it. Each holder has its own key, so two
/// sessions can hold at once and each releases only its own.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait SleepPort: Send + Sync {
    /// Starts holding for `key`; holding twice for the same key is one hold. The error is a
    /// message for the person.
    fn hold(&self, key: u64) -> Result<(), String>;
    fn release(&self, key: u64);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_refuse_to_hold() {
        let mut mock = MockSleepPort::new();
        mock.expect_hold().returning(|_| Err("no caffeinate".to_string()));
        assert_eq!(mock.hold(1), Err("no caffeinate".to_string()));
    }
}
