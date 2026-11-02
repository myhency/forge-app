use std::path::{Path, PathBuf};

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// What a saved report holds, for the page to tell the person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReportSummary {
    /// The names of the files inside the zip.
    pub entries: Vec<String>,
    /// How many places in the text were hidden (paths, keys, e-mail addresses).
    pub hidden: usize,
}

/// Packs Forge's logs, the computer's own logs and its CPU and memory details into one zip the person can attach
/// to a report. Everything personal is hidden on the way in.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait LogReportPort: Send + Sync {
    /// A file name for the zip, with the time in it, such as `forge-report-20261102-1530.zip`.
    fn suggested_name(&self) -> String;
    /// The folder with Forge's own log files.
    fn logs_dir(&self) -> PathBuf;
    /// Writes the report to `dest`; with `conversations` the newest conversations are in it too. Slow (it asks the
    /// system for its logs), so a caller keeps it off the UI thread.
    fn write_report(&self, dest: &Path, conversations: bool) -> Result<ReportSummary, String>;
}

/// Asks the person where to save a file. `None` means they cancelled.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait FileSavePort: Send + Sync {
    async fn pick_save_path(&self, file_name: &str) -> Option<PathBuf>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_save_dialog_can_be_cancelled() {
        let mut mock = MockFileSavePort::new();
        mock.expect_pick_save_path().returning(|_| None);
        assert_eq!(mock.pick_save_path("a.zip").await, None);
    }
}
