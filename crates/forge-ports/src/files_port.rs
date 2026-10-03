use std::path::Path;

use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// The most bytes of a file that `FilesPort::read` shows.
pub const READ_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    Text(String),
    /// The file has a NUL byte near its start.
    Binary,
    /// The file is over `READ_LIMIT`.
    TooLarge,
}

/// A line of a file that holds the searched text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHit {
    pub path: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilesError {
    #[error("That path is outside the session folder")]
    Outside,
    #[error("{0}")]
    Failed(String),
}

/// Reads the files under a root folder. Every path is relative to the root, `/`-separated, with
/// `""` for the root itself; a path that leaves the root is refused.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait FilesPort: Send + Sync {
    /// The entries of a folder, folders first, then by name; `.git` and `node_modules` are left out.
    fn list(&self, root: &Path, dir: &str) -> Result<Vec<DirEntry>, FilesError>;
    fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>;
    /// Lines that contain `query` (any case), at most `limit`, in path order.
    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<ContentHit>, FilesError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(FilesError::Outside.to_string(), "That path is outside the session folder");
        assert_eq!(FilesError::Failed("denied".to_string()).to_string(), "denied");
    }
}
