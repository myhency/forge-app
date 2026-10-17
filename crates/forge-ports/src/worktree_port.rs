use std::path::Path;

use forge_domain::WorktreeRecord;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// The local branches of a repository and the one checked out (`None` while HEAD is detached).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchList {
    pub current: Option<String>,
    pub local: Vec<String>,
}

/// What a worktree holds that removing it would lose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorktreeState {
    /// Changed or untracked files.
    pub dirty: bool,
    /// Commits on the worktree's branch that its base branch does not have.
    pub own_commits: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorktreeError {
    #[error("Not a git repository")]
    NotARepository,
    /// The folder or the branch for that name exists already.
    #[error("A worktree with that name already exists")]
    NameTaken,
    #[error("{0}")]
    Failed(String),
}

/// Makes, inspects and removes git worktrees. Unlike `GitPort` it changes the repository.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait WorktreePort: Send + Sync {
    /// The local branches of the repository at `repo`; `NotARepository` for any other folder.
    fn branches(&self, repo: &Path) -> Result<BranchList, WorktreeError>;
    /// `git worktree add -b <branch> <dir> <base>` in `repo`. `NameTaken` when `dir` or `branch` exists.
    fn create(&self, repo: &Path, dir: &Path, branch: &str, base: &str) -> Result<(), WorktreeError>;
    fn inspect(&self, record: &WorktreeRecord) -> Result<WorktreeState, WorktreeError>;
    /// Removes the worktree's folder and then its branch. Git itself refuses a folder with changes;
    /// a branch that cannot be deleted safely is left in place.
    fn remove(&self, record: &WorktreeRecord) -> Result<(), WorktreeError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(WorktreeError::NotARepository.to_string(), "Not a git repository");
        assert_eq!(WorktreeError::NameTaken.to_string(), "A worktree with that name already exists");
        assert_eq!(WorktreeError::Failed("boom".to_string()).to_string(), "boom");
    }

    #[test]
    fn an_empty_branch_list_has_no_current_branch() {
        assert_eq!(BranchList::default().current, None);
        assert!(BranchList::default().local.is_empty());
    }
}
