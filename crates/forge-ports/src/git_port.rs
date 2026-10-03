use std::path::Path;

use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    Untracked,
    Conflicted,
}

impl ChangeStatus {
    pub fn letter(self) -> &'static str {
        match self {
            ChangeStatus::Modified => "M",
            ChangeStatus::Added => "A",
            ChangeStatus::Deleted => "D",
            ChangeStatus::Renamed => "R",
            ChangeStatus::Untracked => "U",
            ChangeStatus::Conflicted => "C",
        }
    }
}

/// One changed file: its path from the repository root and the lines it adds and removes
/// (`None` when git has no count, as for an untracked or binary file).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedFile {
    pub path: String,
    pub status: ChangeStatus,
    pub added: Option<usize>,
    pub removed: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GitError {
    #[error("Not a git repository")]
    NotARepository,
    #[error("{0}")]
    Failed(String),
}

/// Reads the state of a git working tree. It never changes it.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait GitPort: Send + Sync {
    /// The changed files of the repository that holds `cwd`, against `HEAD`, untracked files included.
    fn status(&self, cwd: &Path) -> Result<Vec<ChangedFile>, GitError>;
    /// The unified diff of one file (a path from `status`) against `HEAD`; an untracked file is
    /// all added. Empty text when there is nothing to show.
    fn diff(&self, cwd: &Path, path: &str) -> Result<String, GitError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_status_has_its_own_letter() {
        let letters: Vec<_> = [
            ChangeStatus::Modified,
            ChangeStatus::Added,
            ChangeStatus::Deleted,
            ChangeStatus::Renamed,
            ChangeStatus::Untracked,
            ChangeStatus::Conflicted,
        ]
        .iter()
        .map(|status| status.letter())
        .collect();
        assert_eq!(letters, vec!["M", "A", "D", "R", "U", "C"]);
    }

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(GitError::NotARepository.to_string(), "Not a git repository");
        assert_eq!(GitError::Failed("boom".to_string()).to_string(), "boom");
    }
}
