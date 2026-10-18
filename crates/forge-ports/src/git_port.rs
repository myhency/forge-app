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

/// The message of loading the diff of a file that is too large to read.
pub const TOO_LARGE_TO_SHOW: &str = "This file is too large to show";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GitError {
    #[error("Not a git repository")]
    NotARepository,
    /// The branch or commit asked for as a base does not exist.
    #[error("Base branch \"{0}\" was not found")]
    BaseNotFound(String),
    #[error("{0}")]
    Failed(String),
}

/// What a change list compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChangeSource {
    /// Where the branch left `base` (the merge base) against the working tree, untracked files included.
    All { base: String },
    /// `HEAD` against the working tree, untracked files included.
    Uncommitted,
    /// One commit against its parent (a root commit against the empty tree).
    Commit { sha: String },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiffOptions {
    /// `git diff -w`: a file whose only change is whitespace is left out.
    pub ignore_whitespace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchInfo {
    /// The checked-out branch, `None` while HEAD is detached.
    pub current: Option<String>,
    /// The branch name, or the short sha of a detached HEAD.
    pub head_label: String,
    /// `origin/HEAD`'s branch, else `main`, else `master`, if there is one.
    pub default_base: Option<String>,
    pub branches: Vec<String>,
    /// The repository root. Git prints paths relative to it, which is not always the session folder.
    pub root: std::path::PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitInfo {
    pub sha: String,
    pub subject: String,
    pub author: String,
    /// Seconds since the epoch.
    pub time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchState {
    /// The unified diff of the file, from its `diff --git` line on; empty for a mode-only change.
    Text(String),
    Binary,
    /// Over the budget of one `changes` call; `file_patch` reads it alone.
    TooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    pub file: ChangedFile,
    pub patch: PatchState,
}

/// Reads the state of a git working tree. It never changes it.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait GitPort: Send + Sync {
    /// The local branches, the checked-out one and the default base of the repository holding `cwd`.
    fn branches(&self, cwd: &Path) -> Result<BranchInfo, GitError>;
    /// The commits of `base..HEAD`, newest first, at most 200. Empty when there is no commit yet.
    /// `BaseNotFound` when `base` does not exist.
    fn commits(&self, cwd: &Path, base: &str) -> Result<Vec<CommitInfo>, GitError>;
    /// The changed files of `source` with their patches, in path order. `BaseNotFound` for an `All`
    /// whose base does not exist.
    fn changes(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions) -> Result<Vec<FileChange>, GitError>;
    /// The lines of a file on the new side of `source` (the working tree, or the commit's blob).
    fn file_lines(&self, cwd: &Path, source: &ChangeSource, path: &str) -> Result<Vec<String>, GitError>;
    /// The patch of one file with no size budget, for a file `changes` called `TooLarge`.
    fn file_patch(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, path: &str) -> Result<String, GitError>;
    /// Whether the working tree has a change or an untracked file (an ignored file is neither). The
    /// panel hides its `Uncommitted changes` row when this is false.
    fn is_dirty(&self, cwd: &Path) -> Result<bool, GitError>;
    /// Whether `sha` names a commit of the repository, wherever it sits in the history. `commits`
    /// stops at a cap, so a selected commit beyond it is not in that list but still exists.
    fn commit_exists(&self, cwd: &Path, sha: &str) -> Result<bool, GitError>;
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
    fn the_new_error_names_the_base() {
        assert_eq!(GitError::BaseNotFound("dev".to_string()).to_string(), "Base branch \"dev\" was not found");
    }

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(GitError::NotARepository.to_string(), "Not a git repository");
        assert_eq!(GitError::Failed("boom".to_string()).to_string(), "boom");
    }
}
