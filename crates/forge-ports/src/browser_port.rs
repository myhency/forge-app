#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Opens an address in the person's own browser.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait BrowserPort: Send + Sync {
    /// Only `http://` and `https://` addresses are opened. The error is a message for the person.
    fn open(&self, url: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_refuse() {
        let mut mock = MockBrowserPort::new();
        mock.expect_open().returning(|_| Err("no browser".to_string()));
        assert_eq!(mock.open("http://localhost"), Err("no browser".to_string()));
    }
}
