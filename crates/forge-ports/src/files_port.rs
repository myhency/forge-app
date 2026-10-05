use std::path::Path;

use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// The most bytes of a file that `FilesPort::read` shows: 10 MiB (the real app opened a 10 MiB
/// text file and refused a 15 MiB one).
pub const READ_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// When a file was last changed and how long it is. Two reads that give the same version saw the
/// same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersion {
    pub modified_ns: u128,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    /// `lossy` is true when the bytes were not valid UTF-8 and some were replaced; such a file must
    /// not be written back.
    Text { text: String, version: FileVersion, lossy: bool },
    /// A picture, by its extension; `mime` is for a `data:` address.
    Image { bytes: Vec<u8>, mime: String },
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
    /// Char offsets of the first match inside `text`.
    pub start: usize,
    pub end: usize,
}

/// How a content search reads its query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    /// The query is a regular expression instead of plain text.
    pub regex: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilesError {
    #[error("That path is outside the session folder")]
    Outside,
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    InvalidPattern(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
    #[error("The file changed on disk since it was read")]
    Changed,
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
    /// The file's bytes as text even when it looks binary (invalid UTF-8 is replaced). `TooLarge`
    /// over `READ_LIMIT`. For "Show as text anyway"; the result must be shown read only.
    fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>;
    /// The file's version now.
    fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError>;
    /// Replaces the file's text. With `Some(version)` it first checks the file is still at that
    /// version and gives `WriteError::Changed` when it is not; with `None` it overwrites. The text
    /// goes to a temporary file in the same folder that is renamed over the target, so a crash
    /// leaves the old or the new file whole. Returns the version after the write.
    fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError>;
    /// One hit per line that holds a match of `query`, at most `limit`, in path order.
    fn search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError>;
    /// The last `max_bytes` of the file at an absolute `path`, as text (bytes that are not UTF-8 are
    /// replaced). Unlike `read` it is not confined to a root: the caller must have checked the path.
    fn read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(FilesError::Outside.to_string(), "That path is outside the session folder");
        assert_eq!(FilesError::Failed("denied".to_string()).to_string(), "denied");
    }

    #[test]
    fn write_errors_read_as_messages() {
        assert_eq!(WriteError::Changed.to_string(), "The file changed on disk since it was read");
        assert_eq!(WriteError::Failed("denied".to_string()).to_string(), "denied");
    }
}
