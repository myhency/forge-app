use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_domain::WorktreeRecord;
use forge_ports::{BranchList, ChangeStatus, ChangedFile, GitError, GitPort, WorktreeError, WorktreePort, WorktreeState};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);
const CREATE_TIMEOUT: Duration = Duration::from_secs(120);

struct Output {
    code: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl Output {
    fn success(&self) -> bool {
        self.code == Some(0)
    }
}

/// Reads a working tree by running the `git` command, read only. Every command has a timeout, and
/// paths are always passed after `--` so a name that looks like an option is only a name.
pub struct GitCli {
    program: String,
    timeout: Duration,
}

impl GitCli {
    pub fn new() -> Self {
        Self::with_program("git", DEFAULT_TIMEOUT)
    }

    pub fn with_program(program: impl Into<String>, timeout: Duration) -> Self {
        Self { program: program.into(), timeout }
    }

    fn run(&self, cwd: &Path, args: &[&str]) -> Result<Output, GitError> {
        self.run_for(cwd, args, self.timeout)
    }

    fn run_for(&self, cwd: &Path, args: &[&str], timeout: Duration) -> Result<Output, GitError> {
        // A folder that is gone also makes the spawn fail with "not found", which must not read
        // as a missing git.
        if !cwd.is_dir() {
            return Err(GitError::Failed(format!("Folder \"{}\" does not exist", cwd.display())));
        }
        let mut child = Command::new(&self.program)
            .arg("--no-optional-locks")
            .args(args)
            .current_dir(cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("LC_ALL", "C")
            // Paths are names, not patterns: `app/[id]/page.tsx` is one file.
            .env("GIT_LITERAL_PATHSPECS", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    GitError::Failed("git is not installed".to_string())
                } else {
                    GitError::Failed(error.to_string())
                }
            })?;
        // Both pipes are drained on their own threads so a large output cannot fill a pipe and
        // stop git while we wait for it.
        let mut stdout = child.stdout.take().expect("piped");
        let mut stderr = child.stderr.take().expect("piped");
        let out_thread = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            bytes
        });
        let err_thread = thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stderr.read_to_end(&mut bytes);
            bytes
        });
        let deadline = Instant::now() + timeout;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(GitError::Failed("git timed out".to_string()));
                }
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => return Err(GitError::Failed(error.to_string())),
            }
        };
        let stdout = out_thread.join().unwrap_or_default();
        let stderr = String::from_utf8_lossy(&err_thread.join().unwrap_or_default()).into_owned();
        Ok(Output { code: status.code(), stdout, stderr })
    }

    // The root of the repository that holds `cwd`. Status and diff run there, so the paths git
    // prints (relative to the root) are the paths we pass back.
    fn root(&self, cwd: &Path) -> Result<std::path::PathBuf, GitError> {
        let output = self.run(cwd, &["rev-parse", "--show-toplevel"])?;
        if output.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            return Ok(std::path::PathBuf::from(text.trim_end_matches('\n')));
        }
        Err(failure(&output.stderr))
    }
}

impl Default for GitCli {
    fn default() -> Self {
        Self::new()
    }
}

fn failure(stderr: &str) -> GitError {
    if stderr.contains("not a git repository") {
        GitError::NotARepository
    } else {
        GitError::Failed(stderr.lines().next().unwrap_or("git failed").to_string())
    }
}

fn worktree_error(error: GitError) -> WorktreeError {
    match error {
        GitError::NotARepository => WorktreeError::NotARepository,
        GitError::Failed(message) => WorktreeError::Failed(message),
    }
}

fn worktree_failure(stderr: &str) -> WorktreeError {
    worktree_error(failure(stderr))
}

fn lines_of(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes).lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}

fn status_of(x: u8, y: u8) -> ChangeStatus {
    match (x, y) {
        (b'?', b'?') => ChangeStatus::Untracked,
        (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => ChangeStatus::Conflicted,
        (b'R', _) | (_, b'R') | (b'C', _) | (_, b'C') => ChangeStatus::Renamed,
        (b'A', _) | (_, b'A') => ChangeStatus::Added,
        (b'D', _) | (_, b'D') => ChangeStatus::Deleted,
        _ => ChangeStatus::Modified,
    }
}

// `git status --porcelain=v1 -z`: entries `XY path`, each ended by a NUL; a rename or copy is
// followed by one more NUL-ended entry, the old name.
fn parse_status(bytes: &[u8]) -> Vec<ChangedFile> {
    let text = String::from_utf8_lossy(bytes);
    let mut parts = text.split('\0');
    let mut files = Vec::new();
    while let Some(entry) = parts.next() {
        let (Some(codes), Some(path)) = (entry.get(..3), entry.get(3..)) else { continue };
        let codes = codes.as_bytes();
        let (x, y) = (codes[0], codes[1]);
        if matches!(x, b'R' | b'C') || matches!(y, b'R' | b'C') {
            parts.next();
        }
        files.push(ChangedFile { path: path.to_string(), status: status_of(x, y), added: None, removed: None });
    }
    files
}

// `git diff --numstat -z --no-renames`: `added<TAB>removed<TAB>path` ended by a NUL; a binary
// file has `-` for both counts and is left out.
fn parse_numstat(bytes: &[u8]) -> HashMap<String, (usize, usize)> {
    let text = String::from_utf8_lossy(bytes);
    let mut counts = HashMap::new();
    for entry in text.split('\0') {
        let mut fields = entry.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (fields.next(), fields.next(), fields.next()) else { continue };
        if let (Ok(added), Ok(removed)) = (added.parse(), removed.parse()) {
            counts.insert(path.to_string(), (added, removed));
        }
    }
    counts
}

impl GitPort for GitCli {
    fn status(&self, cwd: &Path) -> Result<Vec<ChangedFile>, GitError> {
        let root = self.root(cwd)?;
        let output = self.run(&root, &["status", "--porcelain=v1", "-z", "-uall"])?;
        if !output.success() {
            return Err(failure(&output.stderr));
        }
        let mut files = parse_status(&output.stdout);
        // A repository with no commit has no HEAD: the counts are then simply missing.
        let counts = match self.run(&root, &["diff", "HEAD", "--numstat", "-z", "--no-renames"]) {
            Ok(output) if output.success() => parse_numstat(&output.stdout),
            _ => HashMap::new(),
        };
        for file in &mut files {
            if let Some((added, removed)) = counts.get(&file.path) {
                file.added = Some(*added);
                file.removed = Some(*removed);
            }
        }
        Ok(files)
    }

    fn diff(&self, cwd: &Path, path: &str) -> Result<String, GitError> {
        let root = self.root(cwd)?;
        let options = ["--no-color", "--no-ext-diff", "--no-renames", "-U3"];
        let mut tracked = self.run(&root, &[&["diff", "HEAD"][..], &options, &["--", path]].concat())?;
        if !tracked.success() {
            // No HEAD yet: what is staged is the change.
            tracked = self.run(&root, &[&["diff", "--cached"][..], &options, &["--", path]].concat())?;
        }
        if tracked.success() && !tracked.stdout.is_empty() {
            return Ok(String::from_utf8_lossy(&tracked.stdout).into_owned());
        }
        // An untracked file is shown as all added; `--no-index` exits with 1 when it differs.
        let others = self.run(&root, &["ls-files", "--others", "--exclude-standard", "-z", "--", path])?;
        if others.success() && !others.stdout.is_empty() {
            let shown = self.run(&root, &[&["diff", "--no-index"][..], &options, &["--", "/dev/null", path]].concat())?;
            if matches!(shown.code, Some(0 | 1)) {
                return Ok(String::from_utf8_lossy(&shown.stdout).into_owned());
            }
            return Err(failure(&shown.stderr));
        }
        Ok(String::new())
    }
}

impl WorktreePort for GitCli {
    fn branches(&self, repo: &Path) -> Result<BranchList, WorktreeError> {
        let listed = self.run(repo, &["branch", "--format=%(refname:short)"]).map_err(worktree_error)?;
        if !listed.success() {
            return Err(worktree_failure(&listed.stderr));
        }
        // A detached HEAD is listed as "(HEAD detached at ...)": not a branch.
        let local: Vec<String> = lines_of(&listed.stdout).into_iter().filter(|name| !name.starts_with('(')).collect();
        let head = self.run(repo, &["symbolic-ref", "--short", "-q", "HEAD"]).map_err(worktree_error)?;
        let current = if head.success() { lines_of(&head.stdout).into_iter().next() } else { None };
        Ok(BranchList { current, local })
    }

    fn create(&self, repo: &Path, dir: &Path, branch: &str, base: &str) -> Result<(), WorktreeError> {
        if dir.exists() {
            return Err(WorktreeError::NameTaken);
        }
        let exists = self.run(repo, &["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}")]).map_err(worktree_error)?;
        if exists.success() {
            return Err(WorktreeError::NameTaken);
        }
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|error| WorktreeError::Failed(error.to_string()))?;
        }
        let target = dir.to_string_lossy();
        let output = self
            .run_for(repo, &["worktree", "add", "-b", branch, "--", &target, base], CREATE_TIMEOUT)
            .map_err(worktree_error)?;
        if output.success() {
            Ok(())
        } else {
            Err(worktree_failure(&output.stderr))
        }
    }

    fn inspect(&self, record: &WorktreeRecord) -> Result<WorktreeState, WorktreeError> {
        let status = self.run(&record.path, &["status", "--porcelain"]).map_err(worktree_error)?;
        if !status.success() {
            return Err(worktree_failure(&status.stderr));
        }
        let range = format!("{}..{}", record.base_branch, record.branch);
        let counted = self.run(&record.repo, &["rev-list", "--count", &range]).map_err(worktree_error)?;
        if !counted.success() {
            return Err(worktree_failure(&counted.stderr));
        }
        let count: u64 = String::from_utf8_lossy(&counted.stdout).trim().parse().map_err(|_| WorktreeError::Failed("git rev-list gave no count".to_string()))?;
        Ok(WorktreeState { dirty: !status.stdout.is_empty(), own_commits: count > 0 })
    }

    fn remove(&self, record: &WorktreeRecord) -> Result<(), WorktreeError> {
        let target = record.path.to_string_lossy();
        let removed = self.run(&record.repo, &["worktree", "remove", "--", &target]).map_err(worktree_error)?;
        if !removed.success() {
            return Err(worktree_failure(&removed.stderr));
        }
        // `-d` only: a branch that is not merged stays, which loses nothing.
        let _ = self.run(&record.repo, &["branch", "-d", "--", &record.branch]);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use forge_domain::WorktreeRecord;
    use forge_ports::{ChangeStatus, ChangedFile, GitError, GitPort, WorktreeError, WorktreePort};
    use tempfile::TempDir;

    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?}");
    }

    // A repository with one commit holding `a.txt` ("a\nb\nc\n").
    fn repo() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        fs::write(dir.path().join("a.txt"), "a\nb\nc\n").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "first"]);
        dir
    }

    fn find<'a>(files: &'a [ChangedFile], path: &str) -> &'a ChangedFile {
        files.iter().find(|file| file.path == path).unwrap_or_else(|| panic!("{path} in {files:?}"))
    }

    #[test]
    fn a_clean_repository_has_no_changes() {
        assert_eq!(GitCli::new().status(repo().path()).unwrap(), vec![]);
    }

    #[test]
    fn modified_added_deleted_and_untracked_files_are_listed_with_counts() {
        let dir = repo();
        fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\n").unwrap();
        fs::write(dir.path().join("new.txt"), "x\n").unwrap();
        git(dir.path(), &["add", "new.txt"]);
        fs::write(dir.path().join("loose.txt"), "y\n").unwrap();
        let files = GitCli::new().status(dir.path()).unwrap();
        let modified = find(&files, "a.txt");
        assert_eq!((modified.status, modified.added, modified.removed), (ChangeStatus::Modified, Some(2), Some(1)));
        assert_eq!(find(&files, "new.txt").status, ChangeStatus::Added);
        assert_eq!(find(&files, "loose.txt").status, ChangeStatus::Untracked);
        assert_eq!(find(&files, "loose.txt").added, None);

        fs::remove_file(dir.path().join("a.txt")).unwrap();
        let files = GitCli::new().status(dir.path()).unwrap();
        assert_eq!(find(&files, "a.txt").status, ChangeStatus::Deleted);
    }

    #[test]
    fn names_with_spaces_and_non_ascii_characters_and_renames_are_read() {
        let dir = repo();
        fs::write(dir.path().join("my file.txt"), "1\n").unwrap();
        fs::write(dir.path().join("한글.txt"), "2\n").unwrap();
        git(dir.path(), &["mv", "a.txt", "renamed.txt"]);
        let files = GitCli::new().status(dir.path()).unwrap();
        assert_eq!(find(&files, "my file.txt").status, ChangeStatus::Untracked);
        assert_eq!(find(&files, "한글.txt").status, ChangeStatus::Untracked);
        assert_eq!(find(&files, "renamed.txt").status, ChangeStatus::Renamed);
        assert!(files.iter().all(|file| file.path != "a.txt"), "the old name is not a row: {files:?}");
    }

    #[test]
    fn untracked_files_inside_a_new_folder_are_listed_one_by_one() {
        let dir = repo();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/one.txt"), "1\n").unwrap();
        let files = GitCli::new().status(dir.path()).unwrap();
        assert_eq!(find(&files, "sub/one.txt").status, ChangeStatus::Untracked);
    }

    #[test]
    fn a_repository_with_no_commit_lists_its_files() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        fs::write(dir.path().join("f.txt"), "1\n").unwrap();
        let files = GitCli::new().status(dir.path()).unwrap();
        assert_eq!(find(&files, "f.txt").status, ChangeStatus::Untracked);
        assert!(GitCli::new().diff(dir.path(), "f.txt").unwrap().contains("+1"));
    }

    #[test]
    fn a_folder_that_is_not_a_repository_says_so() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitCli::new().status(dir.path()), Err(GitError::NotARepository));
        assert_eq!(GitCli::new().diff(dir.path(), "x"), Err(GitError::NotARepository));
    }

    #[test]
    fn a_subfolder_of_the_repository_sees_root_relative_paths_and_their_diffs() {
        let dir = repo();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("a.txt"), "a\nB\nc\n").unwrap();
        let sub = dir.path().join("sub");
        let files = GitCli::new().status(&sub).unwrap();
        assert_eq!(find(&files, "a.txt").status, ChangeStatus::Modified);
        assert!(GitCli::new().diff(&sub, "a.txt").unwrap().contains("+B"));
    }

    #[test]
    fn the_diff_of_a_modified_file_is_a_unified_diff() {
        let dir = repo();
        fs::write(dir.path().join("a.txt"), "a\nB\nc\n").unwrap();
        let text = GitCli::new().diff(dir.path(), "a.txt").unwrap();
        assert!(text.contains("@@"), "{text}");
        assert!(text.contains("-b") && text.contains("+B"), "{text}");
    }

    #[test]
    fn the_diff_of_an_untracked_file_is_all_added() {
        let dir = repo();
        fs::write(dir.path().join("new.txt"), "one\ntwo\n").unwrap();
        let text = GitCli::new().diff(dir.path(), "new.txt").unwrap();
        assert!(text.contains("new file mode"), "{text}");
        assert!(text.contains("+one") && text.contains("+two"), "{text}");
    }

    #[test]
    fn the_diff_of_an_unchanged_file_is_empty() {
        assert_eq!(GitCli::new().diff(repo().path(), "a.txt").unwrap(), "");
    }

    #[test]
    fn a_path_that_looks_like_an_option_is_only_a_path() {
        let dir = repo();
        assert_eq!(GitCli::new().diff(dir.path(), "--output=/tmp/forge-should-not-exist").unwrap(), "");
        assert!(!Path::new("/tmp/forge-should-not-exist").exists());
    }

    #[test]
    fn a_missing_git_program_is_reported_as_a_message() {
        let error = GitCli::with_program("forge-no-such-git", Duration::from_secs(1)).status(Path::new("/tmp")).unwrap_err();
        assert!(error.to_string().contains("not installed"), "{error}");
    }

    #[test]
    fn a_git_that_hangs_is_stopped_at_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("slow-git");
        fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
        Command::new("chmod").args(["+x"]).arg(&script).status().unwrap();
        let started = std::time::Instant::now();
        let error = GitCli::with_program(script.to_string_lossy(), Duration::from_millis(200)).status(dir.path()).unwrap_err();
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_folder_that_is_gone_is_not_reported_as_a_missing_git() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        let error = GitCli::new().status(&gone).unwrap_err();
        assert!(!error.to_string().contains("not installed"), "{error}");
        assert!(error.to_string().contains("gone"), "{error}");
    }

    #[test]
    fn a_path_with_glob_characters_is_a_name_not_a_pattern() {
        let dir = repo();
        fs::create_dir_all(dir.path().join("app/[id]")).unwrap();
        fs::create_dir_all(dir.path().join("app/i")).unwrap();
        fs::write(dir.path().join("app/[id]/page.tsx"), "one\n").unwrap();
        fs::write(dir.path().join("app/i/page.tsx"), "two\n").unwrap();
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "pages"]);
        fs::write(dir.path().join("app/[id]/page.tsx"), "ONE\n").unwrap();
        fs::write(dir.path().join("app/i/page.tsx"), "TWO\n").unwrap();
        let text = GitCli::new().diff(dir.path(), "app/[id]/page.tsx").unwrap();
        assert!(text.contains("+ONE"), "{text}");
        assert!(!text.contains("TWO"), "the other file leaked into the diff: {text}");
    }

    // A worktree of `repo` made the way the use case makes it.
    fn worktree_of(repo: &TempDir, name: &str) -> WorktreeRecord {
        let cli = GitCli::new();
        let base = cli.branches(repo.path()).unwrap().current.unwrap();
        let dir = repo.path().join(".claude").join("worktrees").join(name);
        let branch = format!("claude/{name}");
        cli.create(repo.path(), &dir, &branch, &base).unwrap();
        WorktreeRecord {
            name: name.to_string(),
            path: dir,
            repo: repo.path().to_path_buf(),
            branch,
            base_branch: base,
            created_at: 0,
        }
    }

    #[test]
    fn the_branches_of_a_repository_are_listed_with_the_current_one() {
        let repo = repo();
        git(repo.path(), &["branch", "feature"]);
        let list = GitCli::new().branches(repo.path()).unwrap();
        assert!(list.current.is_some());
        assert!(list.local.contains(&"feature".to_string()));
        assert!(list.local.contains(list.current.as_ref().unwrap()));
    }

    #[test]
    fn a_detached_head_has_no_current_branch_and_no_entry_in_the_list() {
        let repo = repo();
        git(repo.path(), &["checkout", "-q", "--detach"]);
        let list = GitCli::new().branches(repo.path()).unwrap();
        assert_eq!(list.current, None);
        assert!(list.local.iter().all(|name| !name.starts_with('(')), "{list:?}");
    }

    #[test]
    fn a_folder_that_is_not_a_repository_has_no_branches() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitCli::new().branches(dir.path()), Err(WorktreeError::NotARepository));
    }

    #[test]
    fn creating_a_worktree_makes_the_folder_and_the_branch_from_the_base() {
        let repo = repo();
        let record = worktree_of(&repo, "fix-1a2b3c");
        assert!(record.path.join("a.txt").is_file());
        let branches = GitCli::new().branches(repo.path()).unwrap();
        assert!(branches.local.contains(&"claude/fix-1a2b3c".to_string()));
    }

    #[test]
    fn a_name_whose_folder_or_branch_exists_is_taken() {
        let repo = repo();
        let record = worktree_of(&repo, "fix-1a2b3c");
        let cli = GitCli::new();
        let again = cli.create(repo.path(), &record.path, &record.branch, &record.base_branch);
        assert_eq!(again, Err(WorktreeError::NameTaken));
        let other_dir = repo.path().join(".claude").join("worktrees").join("other");
        let same_branch = cli.create(repo.path(), &other_dir, &record.branch, &record.base_branch);
        assert_eq!(same_branch, Err(WorktreeError::NameTaken));
        assert!(!other_dir.exists());
    }

    #[test]
    fn a_repository_with_no_commit_cannot_have_a_worktree() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        let target = dir.path().join(".claude").join("worktrees").join("x");
        let result = GitCli::new().create(dir.path(), &target, "claude/x", "master");
        assert!(matches!(result, Err(WorktreeError::Failed(_))), "{result:?}");
    }

    #[test]
    fn a_new_worktree_is_clean_with_no_commits_of_its_own() {
        let repo = repo();
        let record = worktree_of(&repo, "clean-000001");
        let state = GitCli::new().inspect(&record).unwrap();
        assert_eq!((state.dirty, state.own_commits), (false, false));
    }

    #[test]
    fn an_untracked_file_makes_a_worktree_dirty() {
        let repo = repo();
        let record = worktree_of(&repo, "dirty-000002");
        fs::write(record.path.join("new.txt"), "x").unwrap();
        assert!(GitCli::new().inspect(&record).unwrap().dirty);
    }

    #[test]
    fn a_commit_on_the_branch_is_its_own_commit() {
        let repo = repo();
        let record = worktree_of(&repo, "commit-000003");
        fs::write(record.path.join("b.txt"), "b").unwrap();
        git(&record.path, &["add", "."]);
        git(&record.path, &["commit", "-q", "-m", "second"]);
        let state = GitCli::new().inspect(&record).unwrap();
        assert_eq!((state.dirty, state.own_commits), (false, true));
    }

    #[test]
    fn removing_a_clean_worktree_deletes_its_folder_and_its_branch() {
        let repo = repo();
        let record = worktree_of(&repo, "gone-000004");
        GitCli::new().remove(&record).unwrap();
        assert!(!record.path.exists());
        assert!(!GitCli::new().branches(repo.path()).unwrap().local.contains(&record.branch));
    }

    #[test]
    fn a_worktree_with_changes_is_not_removed() {
        let repo = repo();
        let record = worktree_of(&repo, "kept-000005");
        fs::write(record.path.join("a.txt"), "changed").unwrap();
        assert!(GitCli::new().remove(&record).is_err());
        assert!(record.path.join("a.txt").is_file());
    }
}
