use std::path::Path;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Opens an address in the person's own browser.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait BrowserPort: Send + Sync {
    /// Only `http://` and `https://` addresses are opened. The error is a message for the person.
    fn open(&self, url: &str) -> Result<(), String>;
    /// Shows a file or folder in Finder (selected in its folder). The path must be absolute.
    fn reveal(&self, path: &Path) -> Result<(), String>;
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

    #[test]
    fn the_mock_can_refuse_to_reveal() {
        let mut mock = MockBrowserPort::new();
        mock.expect_reveal().returning(|_| Err("no finder".to_string()));
        assert_eq!(mock.reveal(std::path::Path::new("/tmp/a.txt")), Err("no finder".to_string()));
    }
}
