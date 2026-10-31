use std::path::PathBuf;

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Asks the person to choose a folder. `None` means they cancelled.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait FolderPickerPort: Send + Sync {
    async fn pick_folder(&self) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_folder_picker_can_return_a_chosen_folder_or_a_cancel() {
        let mut mock = MockFolderPickerPort::new();
        let mut answers = vec![None, Some(PathBuf::from("/work/a"))];
        mock.expect_pick_folder().times(2).returning(move || answers.pop().unwrap());

        assert_eq!(mock.pick_folder().await, Some(PathBuf::from("/work/a")));
        assert_eq!(mock.pick_folder().await, None);
    }
}
