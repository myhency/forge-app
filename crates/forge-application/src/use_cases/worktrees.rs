use std::path::{Path, PathBuf};
use std::sync::Arc;

use forge_domain::{branch_name, worktree_dir, worktree_name, WorktreeRecord};
use forge_ports::{BranchList, ConfigError, WorktreeError, WorktreePort};
use thiserror::Error;

use super::settings::SettingsUseCase;

/// How many names are tried before a clash is given up on.
const MAX_ATTEMPTS: u32 = 8;

#[derive(Debug, Error)]
pub enum WorktreeUseCaseError {
    #[error("{0}")]
    Git(#[from] WorktreeError),
    #[error("failed to load the Forge config: {0}")]
    Config(#[from] ConfigError),
}

/// What archiving did to a session's worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Release {
    /// The folder and its branch are gone and the record is dropped.
    Removed,
    /// The worktree stays on disk.
    Kept(Kept),
    /// Forge did not make this folder; it is left alone.
    NotTracked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kept {
    Changes,
    Commits,
}

/// Makes the worktree a session starts in and removes it when the session is archived. Every
/// worktree it makes is recorded in the Forge config.
pub struct WorktreesUseCase {
    git: Arc<dyn WorktreePort>,
    settings: Arc<SettingsUseCase>,
}

// Git runs as a process: off the async threads.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, WorktreeError> + Send + 'static) -> Result<T, WorktreeError> {
    tokio::task::spawn_blocking(work).await.unwrap_or_else(|error| Err(WorktreeError::Failed(error.to_string())))
}

impl WorktreesUseCase {
    pub fn new(git: Arc<dyn WorktreePort>, settings: Arc<SettingsUseCase>) -> Self {
        Self { git, settings }
    }

    /// The local branches of `repo`; `NotARepository` when it is not one.
    pub async fn branches(&self, repo: PathBuf) -> Result<BranchList, WorktreeUseCaseError> {
        let git = self.git.clone();
        Ok(blocking(move || git.branches(&repo)).await?)
    }

    /// Makes a worktree for a session whose first message is `message`, from `base` (the checked-out
    /// branch when `None`), and records it. `seed` makes the name's suffix.
    pub async fn create(
        &self,
        repo: &Path,
        base: Option<String>,
        message: &str,
        seed: u64,
        created_at: i64,
    ) -> Result<WorktreeRecord, WorktreeUseCaseError> {
        let config = self.settings.load().await?;
        let listed = self.branches(repo.to_path_buf()).await?;
        let base = base
            .or(listed.current)
            .ok_or_else(|| WorktreeError::Failed("HEAD is detached: choose a branch first".to_string()))?;
        for attempt in 0..MAX_ATTEMPTS {
            let name = worktree_name(message, seed, attempt);
            let dir = worktree_dir(repo, &name, &config.worktree_location);
            let branch = branch_name(&name);
            let git = self.git.clone();
            let (repo_owned, dir_owned, branch_owned, base_owned) = (repo.to_path_buf(), dir.clone(), branch.clone(), base.clone());
            match blocking(move || git.create(&repo_owned, &dir_owned, &branch_owned, &base_owned)).await {
                Ok(()) => {
                    let record = WorktreeRecord { name, path: dir, repo: repo.to_path_buf(), branch, base_branch: base, created_at };
                    let kept = record.clone();
                    self.settings.save_code_settings(move |config| config.worktrees.push(kept)).await?;
                    return Ok(record);
                }
                Err(WorktreeError::NameTaken) => continue,
                Err(error) => return Err(error.into()),
            }
        }
        Err(WorktreeError::NameTaken.into())
    }

    /// Archiving the last session of the worktree at `path`: removes it when it holds no work.
    pub async fn release(&self, path: &Path) -> Result<Release, WorktreeUseCaseError> {
        let config = self.settings.load().await?;
        let Some(record) = config.worktrees.iter().find(|record| record.path == path).cloned() else {
            return Ok(Release::NotTracked);
        };
        if record.path.exists() {
            let git = self.git.clone();
            let probe = record.clone();
            let state = blocking(move || git.inspect(&probe)).await?;
            if state.dirty {
                return Ok(Release::Kept(Kept::Changes));
            }
            if state.own_commits {
                return Ok(Release::Kept(Kept::Commits));
            }
            let git = self.git.clone();
            let doomed = record.clone();
            blocking(move || git.remove(&doomed)).await?;
        }
        // A folder that is already gone only needs its record dropped.
        let gone = record.path.clone();
        self.settings.save_code_settings(move |config| config.worktrees.retain(|record| record.path != gone)).await?;
        Ok(Release::Removed)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::AppConfig;
    use forge_ports::{MockConfigRepositoryPort, MockWorktreePort, WorktreeState};
    use std::sync::Mutex;

    fn usecase(git: MockWorktreePort, config: AppConfig, saved: Arc<Mutex<Vec<AppConfig>>>) -> WorktreesUseCase {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(move || Ok(config.clone()));
        mock.expect_save().returning(move |config| {
            saved.lock().unwrap().push(config.clone());
            Ok(())
        });
        WorktreesUseCase::new(Arc::new(git), Arc::new(SettingsUseCase::new(Arc::new(mock))))
    }

    fn on_master() -> BranchList {
        BranchList { current: Some("master".to_string()), local: vec!["master".to_string(), "dev".to_string()] }
    }

    fn record(path: &str) -> WorktreeRecord {
        WorktreeRecord {
            name: "x-000001".to_string(),
            path: path.into(),
            repo: "/work/app".into(),
            branch: "claude/x-000001".to_string(),
            base_branch: "master".to_string(),
            created_at: 1,
        }
    }

    #[tokio::test]
    async fn creating_a_worktree_names_it_from_the_message_and_saves_the_record() {
        let mut git = MockWorktreePort::new();
        git.expect_branches().returning(|_| Ok(on_master()));
        git.expect_create()
            .withf(|repo, dir, branch, base| {
                repo == Path::new("/work/app")
                    && dir.to_string_lossy().starts_with("/work/app/.claude/worktrees/fix-the-bug-")
                    && branch.starts_with("claude/fix-the-bug-")
                    && base == "master"
            })
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let usecase = usecase(git, AppConfig::default(), saved.clone());

        let made = usecase.create(Path::new("/work/app"), None, "Fix the bug\nplease", 7, 99).await.unwrap();

        assert_eq!(made.repo, PathBuf::from("/work/app"));
        assert_eq!(made.base_branch, "master");
        assert_eq!(made.created_at, 99);
        assert_eq!(made.branch, format!("claude/{}", made.name));
        assert_eq!(saved.lock().unwrap().last().unwrap().worktrees, vec![made]);
    }

    #[tokio::test]
    async fn a_chosen_branch_is_the_base_and_a_custom_folder_is_used() {
        let mut git = MockWorktreePort::new();
        git.expect_branches().returning(|_| Ok(on_master()));
        git.expect_create()
            .withf(|_, dir, _, base| dir.starts_with("/trees/app/") && base == "dev")
            .times(1)
            .returning(|_, _, _, _| Ok(()));
        let config = AppConfig { worktree_location: forge_domain::WorktreeLocation::Folder("/trees".into()), ..AppConfig::default() };
        let usecase = usecase(git, config, Arc::new(Mutex::new(Vec::new())));
        let made = usecase.create(Path::new("/work/app"), Some("dev".to_string()), "x", 1, 0).await.unwrap();
        assert_eq!(made.base_branch, "dev");
    }

    #[tokio::test]
    async fn a_taken_name_is_made_again_with_the_next_attempt() {
        let mut git = MockWorktreePort::new();
        git.expect_branches().returning(|_| Ok(on_master()));
        let calls = Arc::new(Mutex::new(Vec::<String>::new()));
        let seen = calls.clone();
        git.expect_create().times(2).returning(move |_, dir, _, _| {
            let mut calls = seen.lock().unwrap();
            calls.push(dir.to_string_lossy().to_string());
            if calls.len() == 1 { Err(WorktreeError::NameTaken) } else { Ok(()) }
        });
        let usecase = usecase(git, AppConfig::default(), Arc::new(Mutex::new(Vec::new())));
        usecase.create(Path::new("/work/app"), None, "x", 5, 0).await.unwrap();
        let calls = calls.lock().unwrap();
        assert_ne!(calls[0], calls[1]);
    }

    #[tokio::test]
    async fn a_detached_head_without_a_chosen_branch_cannot_start_a_worktree() {
        let mut git = MockWorktreePort::new();
        git.expect_branches().returning(|_| Ok(BranchList { current: None, local: vec!["dev".to_string()] }));
        let usecase = usecase(git, AppConfig::default(), Arc::new(Mutex::new(Vec::new())));
        let result = usecase.create(Path::new("/work/app"), None, "x", 1, 0).await;
        assert!(matches!(result, Err(WorktreeUseCaseError::Git(WorktreeError::Failed(_)))), "{result:?}");
    }

    #[tokio::test]
    async fn a_git_failure_is_returned_and_no_record_is_saved() {
        let mut git = MockWorktreePort::new();
        git.expect_branches().returning(|_| Ok(on_master()));
        git.expect_create().returning(|_, _, _, _| Err(WorktreeError::Failed("fatal: bad".to_string())));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let usecase = usecase(git, AppConfig::default(), saved.clone());
        let result = usecase.create(Path::new("/work/app"), None, "x", 1, 0).await;
        assert_eq!(result.unwrap_err().to_string(), "fatal: bad");
        assert!(saved.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn releasing_a_clean_worktree_removes_it_and_drops_its_record() {
        let here = tempfile::tempdir().unwrap();
        let path = here.path().to_string_lossy().to_string();
        let mut git = MockWorktreePort::new();
        git.expect_inspect().returning(|_| Ok(WorktreeState { dirty: false, own_commits: false }));
        git.expect_remove().times(1).returning(|_| Ok(()));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let config = AppConfig { worktrees: vec![record(&path)], ..AppConfig::default() };
        let usecase = usecase(git, config, saved.clone());

        assert_eq!(usecase.release(Path::new(&path)).await.unwrap(), Release::Removed);
        assert!(saved.lock().unwrap().last().unwrap().worktrees.is_empty());
    }

    #[tokio::test]
    async fn a_worktree_with_changes_or_commits_is_kept_and_its_record_stays() {
        for (state, expected) in [
            (WorktreeState { dirty: true, own_commits: false }, Kept::Changes),
            (WorktreeState { dirty: false, own_commits: true }, Kept::Commits),
            (WorktreeState { dirty: true, own_commits: true }, Kept::Changes),
        ] {
            let here = tempfile::tempdir().unwrap();
            let path = here.path().to_string_lossy().to_string();
            let mut git = MockWorktreePort::new();
            git.expect_inspect().returning(move |_| Ok(state));
            // No expect_remove: a removal would panic.
            let saved = Arc::new(Mutex::new(Vec::new()));
            let config = AppConfig { worktrees: vec![record(&path)], ..AppConfig::default() };
            let usecase = usecase(git, config, saved.clone());
            assert_eq!(usecase.release(Path::new(&path)).await.unwrap(), Release::Kept(expected));
            assert!(saved.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn a_folder_that_is_not_a_recorded_worktree_is_not_touched() {
        let git = MockWorktreePort::new();
        let usecase = usecase(git, AppConfig::default(), Arc::new(Mutex::new(Vec::new())));
        assert_eq!(usecase.release(Path::new("/work/app/.claude/worktrees/theirs")).await.unwrap(), Release::NotTracked);
    }

    #[tokio::test]
    async fn a_worktree_folder_deleted_by_hand_only_loses_its_record() {
        let git = MockWorktreePort::new();
        let saved = Arc::new(Mutex::new(Vec::new()));
        let config = AppConfig { worktrees: vec![record("/definitely/not/here")], ..AppConfig::default() };
        let usecase = usecase(git, config, saved.clone());
        assert_eq!(usecase.release(Path::new("/definitely/not/here")).await.unwrap(), Release::Removed);
        assert!(saved.lock().unwrap().last().unwrap().worktrees.is_empty());
    }

    #[tokio::test]
    async fn a_failed_removal_is_returned_and_the_record_stays() {
        let here = tempfile::tempdir().unwrap();
        let path = here.path().to_string_lossy().to_string();
        let mut git = MockWorktreePort::new();
        git.expect_inspect().returning(|_| Ok(WorktreeState { dirty: false, own_commits: false }));
        git.expect_remove().returning(|_| Err(WorktreeError::Failed("locked".to_string())));
        let saved = Arc::new(Mutex::new(Vec::new()));
        let config = AppConfig { worktrees: vec![record(&path)], ..AppConfig::default() };
        let usecase = usecase(git, config, saved.clone());
        assert_eq!(usecase.release(Path::new(&path)).await.unwrap_err().to_string(), "locked");
        assert!(saved.lock().unwrap().is_empty());
    }
}
