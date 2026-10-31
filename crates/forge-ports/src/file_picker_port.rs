use std::path::PathBuf;

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Asks the person to choose files. An empty list means they cancelled.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait FilePickerPort: Send + Sync {
    async fn pick_files(&self) -> Vec<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_file_picker_returns_the_chosen_files_or_nothing() {
        let mut mock = MockFilePickerPort::new();
        let mut answers = vec![Vec::new(), vec![PathBuf::from("/a.png"), PathBuf::from("/b.txt")]];
        mock.expect_pick_files().times(2).returning(move || answers.pop().unwrap());

        assert_eq!(mock.pick_files().await.len(), 2);
        assert!(mock.pick_files().await.is_empty());
    }
}
