use std::path::Path;

use async_trait::async_trait;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{0}")]
pub struct ScratchError(pub String);

/// The plain folder work behind scratch folders (the empty folders made for sessions that have no
/// project folder).
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ScratchPort: Send + Sync {
    /// Makes the folder and any missing parents.
    async fn create_dir(&self, path: &Path) -> Result<(), ScratchError>;
    /// True for a folder with no entries; false for a file, a missing path or a folder with entries.
    async fn is_empty_dir(&self, path: &Path) -> Result<bool, ScratchError>;
    /// Removes an empty folder. Fails on a folder that has entries.
    async fn remove_empty_dir(&self, path: &Path) -> Result<(), ScratchError>;
}
