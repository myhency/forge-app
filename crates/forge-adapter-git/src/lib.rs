use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_domain::WorktreeRecord;
use forge_ports::{BranchInfo, BranchList, ChangeSource, ChangeStatus, ChangedFile, CommitInfo, DiffOptions, FileChange, GitError, GitPort, PatchState, WorktreeError, WorktreePort, WorktreeState};

mod changes;
use changes::{is_quoted_name, parse_counts, parse_name_status, split_patch, safe_ref, untracked_patch, Count, EMPTY_TREE, PATCH_BUDGET};

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

    fn checked(&self, output: Output) -> Result<Output, GitError> {
        if output.success() {
            Ok(output)
        } else {
            Err(failure(&output.stderr))
        }
    }

    fn has_commit(&self, root: &Path, name: &str) -> bool {
        safe_ref(name)
            && self
                .run(root, &["rev-parse", "--verify", "--quiet", &format!("{name}^{{commit}}")])
                .is_ok_and(|output| output.success())
    }

    fn diff_cmd(&self, root: &Path, flags: &[&str], from: &str, to: Option<&str>, only: Option<&str>) -> Result<Output, GitError> {
        let mut args = vec!["-c", "core.quotepath=off", "diff"];
        args.extend_from_slice(flags);
        args.push(from);
        if let Some(to) = to {
            args.push(to);
        }
        if let Some(path) = only {
            args.extend(["--", path]);
        }
        self.run(root, &args)
    }

    // What `source` compares: the starting tree, the end (`None` is the working tree) and whether
    // untracked files belong to it.
    fn range(&self, root: &Path, source: &ChangeSource) -> Result<(String, Option<String>, bool), GitError> {
        let has_head = self.has_commit(root, "HEAD");
        match source {
            ChangeSource::Uncommitted => Ok((if has_head { "HEAD".to_string() } else { EMPTY_TREE.to_string() }, None, true)),
            ChangeSource::All { base } => {
                if !self.has_commit(root, base) {
                    return Err(GitError::BaseNotFound(base.clone()));
                }
                if !has_head {
                    return Ok((EMPTY_TREE.to_string(), None, true));
                }
                // Unrelated histories have no merge base: compare with the base itself.
                let from = match self.run(root, &["merge-base", base, "HEAD"]) {
                    Ok(output) if output.success() => String::from_utf8_lossy(&output.stdout).trim().to_string(),
                    _ => base.clone(),
                };
                Ok((from, None, true))
            }
            ChangeSource::Commit { sha } => {
                if !self.has_commit(root, sha) {
                    return Err(GitError::BaseNotFound(sha.clone()));
                }
                let from = if self.has_commit(root, &format!("{sha}^")) { format!("{sha}^") } else { EMPTY_TREE.to_string() };
                Ok((from, Some(sha.clone()), false))
            }
        }
    }

    // `changes` and `file_patch`: `only` limits it to one path, `budget` is the patch text allowed.
    fn collect(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, only: Option<&str>, mut budget: usize) -> Result<Vec<FileChange>, GitError> {
        let root = self.root(cwd)?;
        let (from, to, untracked) = self.range(&root, source)?;
        let to = to.as_deref();
        let names = self.checked(self.diff_cmd(&root, &["--name-status", "-z", "--no-renames"], &from, to, only)?)?;
        let counts = self.checked(self.diff_cmd(&root, &["--numstat", "-z", "--no-renames"], &from, to, only)?)?;
        let mut flags = vec!["--no-color", "--no-ext-diff", "--no-renames", "-U3"];
        if options.ignore_whitespace {
            flags.push("-w");
        }
        let patch = self.checked(self.diff_cmd(&root, &flags, &from, to, only)?)?;
        let mut patches = split_patch(&String::from_utf8_lossy(&patch.stdout));
        let counts = parse_counts(&counts.stdout);
        let mut files = Vec::new();
        for (status, path) in parse_name_status(&names.stdout) {
            let (added, removed, binary) = match counts.get(&path) {
                Some(Count::Lines(added, removed)) => (Some(*added), Some(*removed), false),
                Some(Count::Binary) => (None, None, true),
                None => (None, None, false),
            };
            let state = if binary {
                PatchState::Binary
            } else {
                match patches.remove(&path) {
                    Some(text) if text.len() <= budget => {
                        budget -= text.len();
                        PatchState::Text(text)
                    }
                    Some(_) => PatchState::TooLarge,
                    // `-w` leaves out a file whose only change is whitespace.
                    // A name git quotes has no readable chunk, so it is not a whitespace-only file.
                    None if is_quoted_name(&path) => PatchState::Binary,
                    None if options.ignore_whitespace => continue,
                    None => PatchState::Text(String::new()),
                }
            };
            files.push(FileChange { file: ChangedFile { path, status, added, removed }, patch: state });
        }
        if untracked {
            let mut args = vec!["ls-files", "--others", "--exclude-standard", "-z"];
            if let Some(path) = only {
                args.extend(["--", path]);
            }
            let others = self.checked(self.run(&root, &args)?)?;
            let mut paths: Vec<String> = String::from_utf8_lossy(&others.stdout).split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect();
            paths.sort();
            for path in paths {
                // Only a regular file is read, never followed: a link, a FIFO or a folder reads as binary,
                // and a file over the budget is not read at all.
                let (state, added) = match std::fs::symlink_metadata(root.join(&path)) {
                    Ok(meta) if meta.is_file() && meta.len() > budget as u64 => (PatchState::TooLarge, 0),
                    Ok(meta) if meta.is_file() => match std::fs::read(root.join(&path)) {
                        Ok(bytes) => untracked_patch(&bytes, budget),
                        Err(_) => (PatchState::Binary, 0),
                    },
                    _ => (PatchState::Binary, 0),
                };
                if let PatchState::Text(text) = &state {
                    budget -= text.len();
                }
                let counts = (!matches!(state, PatchState::Binary | PatchState::TooLarge)).then_some((added, 0));
                files.push(FileChange {
                    file: ChangedFile { path, status: ChangeStatus::Untracked, added: counts.map(|c| c.0), removed: counts.map(|c| c.1) },
                    patch: state,
                });
            }
        }
        files.sort_by(|a, b| a.file.path.cmp(&b.file.path));
        Ok(files)
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
        GitError::BaseNotFound(name) => WorktreeError::Failed(format!("Base branch \"{name}\" was not found")),
        GitError::Failed(message) => WorktreeError::Failed(message),
    }
}

fn worktree_failure(stderr: &str) -> WorktreeError {
    worktree_error(failure(stderr))
}

fn lines_of(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes).lines().map(str::trim).filter(|line| !line.is_empty()).map(str::to_string).collect()
}

impl GitPort for GitCli {
    fn branches(&self, cwd: &Path) -> Result<BranchInfo, GitError> {
        let root = self.root(cwd)?;
        let refs = self.checked(self.run(&root, &["for-each-ref", "--format=%(refname:short)", "--sort=refname", "refs/heads"])?)?;
        let branches = lines_of(&refs.stdout);
        let current = self
            .run(&root, &["symbolic-ref", "--short", "-q", "HEAD"])
            .ok()
            .filter(|output| output.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|name| !name.is_empty());
        let head_label = match &current {
            Some(name) => name.clone(),
            None => self
                .run(&root, &["rev-parse", "--short", "HEAD"])
                .ok()
                .filter(|output| output.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
                .unwrap_or_default(),
        };
        let remote_head = self
            .run(&root, &["symbolic-ref", "--short", "-q", "refs/remotes/origin/HEAD"])
            .ok()
            .filter(|output| output.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .map(|name| name.strip_prefix("origin/").filter(|local| branches.iter().any(|b| b == local)).map(str::to_string).unwrap_or(name))
            .filter(|name| !name.is_empty());
        let default_base = remote_head.or_else(|| ["main", "master"].into_iter().find(|name| branches.iter().any(|b| b == name)).map(str::to_string));
        Ok(BranchInfo { current, head_label, default_base, branches })
    }

    fn commits(&self, cwd: &Path, base: &str) -> Result<Vec<CommitInfo>, GitError> {
        let root = self.root(cwd)?;
        if !self.has_commit(&root, base) {
            return Err(GitError::BaseNotFound(base.to_string()));
        }
        if !self.has_commit(&root, "HEAD") {
            return Ok(Vec::new());
        }
        let range = format!("{base}..HEAD");
        let output = self.checked(self.run(&root, &["log", "-z", "-n", "200", "--format=%H%x1f%an%x1f%at%x1f%s", &range])?)?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(text
            .split('\0')
            .filter_map(|entry| {
                let mut fields = entry.trim_start_matches('\n').splitn(4, '\x1f');
                Some(CommitInfo {
                    sha: fields.next()?.to_string(),
                    author: fields.next()?.to_string(),
                    time: fields.next()?.parse().ok()?,
                    subject: fields.next()?.to_string(),
                })
            })
            .collect())
    }

    fn changes(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions) -> Result<Vec<FileChange>, GitError> {
        self.collect(cwd, source, options, None, PATCH_BUDGET)
    }

    fn file_lines(&self, cwd: &Path, source: &ChangeSource, path: &str) -> Result<Vec<String>, GitError> {
        // Only a plain relative path inside the repository: no root, drive prefix or `..`.
        let escapes = std::path::Path::new(path).components().any(|part| {
            matches!(part, std::path::Component::ParentDir | std::path::Component::Prefix(_) | std::path::Component::RootDir)
        });
        if escapes || path.split('/').any(|part| part == "..") {
            return Err(GitError::Failed("Path is outside the repository".to_string()));
        }
        let root = self.root(cwd)?;
        let bytes = match source {
            ChangeSource::Commit { sha } => {
                if !self.has_commit(&root, sha) {
                    return Err(GitError::BaseNotFound(sha.clone()));
                }
                self.checked(self.run(&root, &["show", &format!("{sha}:{path}")])?)?.stdout
            }
            _ => std::fs::read(root.join(path)).map_err(|error| GitError::Failed(error.to_string()))?,
        };
        Ok(String::from_utf8_lossy(&bytes).lines().map(str::to_string).collect())
    }

    fn file_patch(&self, cwd: &Path, source: &ChangeSource, options: &DiffOptions, path: &str) -> Result<String, GitError> {
        let files = self.collect(cwd, source, options, Some(path), usize::MAX)?;
        Ok(match files.into_iter().next().map(|change| change.patch) {
            Some(PatchState::Text(text)) => text,
            _ => String::new(),
        })
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
        // Commits of the branch, and commits made on a detached HEAD inside the worktree.
        let on_branch = self.commits_beyond(&record.repo, &format!("{}..{}", record.base_branch, record.branch))?;
        let on_head = self.commits_beyond(&record.path, &format!("{}..HEAD", record.base_branch))?;
        let count = on_branch.max(on_head);
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

impl GitCli {
    fn commits_beyond(&self, dir: &Path, range: &str) -> Result<u64, WorktreeError> {
        let counted = self.run(dir, &["rev-list", "--count", range]).map_err(worktree_error)?;
        if !counted.success() {
            return Err(worktree_failure(&counted.stderr));
        }
        String::from_utf8_lossy(&counted.stdout).trim().parse().map_err(|_| WorktreeError::Failed("git rev-list gave no count".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use forge_domain::WorktreeRecord;
    use forge_ports::{ChangeSource, ChangeStatus, DiffOptions, FileChange, GitError, GitPort, PatchState, WorktreeError, WorktreePort};
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

    fn uncommitted(dir: &Path) -> Vec<FileChange> {
        GitCli::new().changes(dir, &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap()
    }

    #[test]
    fn a_clean_repository_has_no_changes() {
        assert_eq!(uncommitted(repo().path()), vec![]);
    }

    #[test]
    fn modified_added_deleted_and_untracked_files_are_listed_with_counts() {
        let dir = repo();
        fs::write(dir.path().join("a.txt"), "a\nB\nc\nd\n").unwrap();
        fs::write(dir.path().join("new.txt"), "x\n").unwrap();
        git(dir.path(), &["add", "new.txt"]);
        fs::write(dir.path().join("loose.txt"), "y\n").unwrap();
        let files = uncommitted(dir.path());
        let modified = &named(&files, "a.txt").file;
        assert_eq!((modified.status, modified.added, modified.removed), (ChangeStatus::Modified, Some(2), Some(1)));
        assert_eq!(named(&files, "new.txt").file.status, ChangeStatus::Added);
        assert_eq!(named(&files, "loose.txt").file.status, ChangeStatus::Untracked);

        fs::remove_file(dir.path().join("a.txt")).unwrap();
        let files = uncommitted(dir.path());
        assert_eq!(named(&files, "a.txt").file.status, ChangeStatus::Deleted);
    }

    #[test]
    fn names_with_spaces_and_non_ascii_characters_are_read_and_a_rename_shows_as_a_deleted_and_an_added_file() {
        let dir = repo();
        fs::write(dir.path().join("my file.txt"), "1\n").unwrap();
        fs::write(dir.path().join("한글.txt"), "2\n").unwrap();
        git(dir.path(), &["mv", "a.txt", "renamed.txt"]);
        let files = uncommitted(dir.path());
        assert_eq!(named(&files, "my file.txt").file.status, ChangeStatus::Untracked);
        assert_eq!(named(&files, "한글.txt").file.status, ChangeStatus::Untracked);
        assert_eq!(named(&files, "renamed.txt").file.status, ChangeStatus::Added);
        assert_eq!(named(&files, "a.txt").file.status, ChangeStatus::Deleted);
    }

    #[test]
    fn a_rename_shows_as_a_deleted_and_an_added_file() {
        let dir = repo();
        git(dir.path(), &["mv", "a.txt", "renamed.txt"]);
        let files = uncommitted(dir.path());
        assert_eq!(files.len(), 2, "{files:?}");
        assert_eq!(named(&files, "renamed.txt").file.status, ChangeStatus::Added);
        assert_eq!(named(&files, "a.txt").file.status, ChangeStatus::Deleted);
    }

    #[test]
    fn untracked_files_inside_a_new_folder_are_listed_one_by_one() {
        let dir = repo();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub/one.txt"), "1\n").unwrap();
        let files = uncommitted(dir.path());
        assert_eq!(named(&files, "sub/one.txt").file.status, ChangeStatus::Untracked);
    }

    #[test]
    fn a_missing_git_program_is_reported_as_a_message() {
        let error = GitCli::with_program("forge-no-such-git", Duration::from_secs(1)).changes(Path::new("/tmp"), &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap_err();
        assert!(error.to_string().contains("not installed"), "{error}");
    }

    #[test]
    fn a_git_that_hangs_is_stopped_at_the_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("slow-git");
        fs::write(&script, "#!/bin/sh\nsleep 5\n").unwrap();
        Command::new("chmod").args(["+x"]).arg(&script).status().unwrap();
        let started = std::time::Instant::now();
        let error = GitCli::with_program(script.to_string_lossy(), Duration::from_millis(200))
            .changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default())
            .unwrap_err();
        assert!(error.to_string().contains("timed out"), "{error}");
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_folder_that_is_gone_is_not_reported_as_a_missing_git() {
        let dir = tempfile::tempdir().unwrap();
        let gone = dir.path().join("gone");
        let error = GitCli::new().changes(&gone, &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap_err();
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
        let files = uncommitted(dir.path());
        assert_eq!(files.len(), 2, "{files:?}");
        assert!(matches!(&named(&files, "app/[id]/page.tsx").patch, PatchState::Text(text) if text.contains("+ONE") && !text.contains("TWO")));
        assert!(matches!(&named(&files, "app/i/page.tsx").patch, PatchState::Text(text) if text.contains("+TWO") && !text.contains("ONE")));
    }

    #[test]
    fn a_file_patch_of_an_option_like_name_is_only_a_patch() {
        let dir = repo();
        write(dir.path(), "-n.txt", "x\n");
        let patch = GitCli::new().file_patch(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default(), "-n.txt").unwrap();
        assert!(patch.contains("+x"));
    }

    // A worktree of `repo` made the way the use case makes it.
    fn worktree_of(repo: &TempDir, name: &str) -> WorktreeRecord {
        let cli = GitCli::new();
        let base = WorktreePort::branches(&cli, repo.path()).unwrap().current.unwrap();
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
        let list = WorktreePort::branches(&GitCli::new(), repo.path()).unwrap();
        assert!(list.current.is_some());
        assert!(list.local.contains(&"feature".to_string()));
        assert!(list.local.contains(list.current.as_ref().unwrap()));
    }

    #[test]
    fn a_detached_head_has_no_current_branch_and_no_entry_in_the_list() {
        let repo = repo();
        git(repo.path(), &["checkout", "-q", "--detach"]);
        let list = WorktreePort::branches(&GitCli::new(), repo.path()).unwrap();
        assert_eq!(list.current, None);
        assert!(list.local.iter().all(|name| !name.starts_with('(')), "{list:?}");
    }

    #[test]
    fn a_folder_that_is_not_a_repository_has_no_branches() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(WorktreePort::branches(&GitCli::new(), dir.path()), Err(WorktreeError::NotARepository));
    }

    #[test]
    fn creating_a_worktree_makes_the_folder_and_the_branch_from_the_base() {
        let repo = repo();
        let record = worktree_of(&repo, "fix-1a2b3c");
        assert!(record.path.join("a.txt").is_file());
        let branches = WorktreePort::branches(&GitCli::new(), repo.path()).unwrap();
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
    fn a_commit_on_a_detached_head_in_the_worktree_is_its_own_commit() {
        let repo = repo();
        let record = worktree_of(&repo, "detached-000009");
        git(&record.path, &["checkout", "-q", "--detach"]);
        fs::write(record.path.join("d.txt"), "d").unwrap();
        git(&record.path, &["add", "."]);
        git(&record.path, &["commit", "-q", "-m", "detached"]);
        let state = GitCli::new().inspect(&record).unwrap();
        assert_eq!((state.dirty, state.own_commits), (false, true));
    }

    #[test]
    fn removing_a_clean_worktree_deletes_its_folder_and_its_branch() {
        let repo = repo();
        let record = worktree_of(&repo, "gone-000004");
        GitCli::new().remove(&record).unwrap();
        assert!(!record.path.exists());
        assert!(!WorktreePort::branches(&GitCli::new(), repo.path()).unwrap().local.contains(&record.branch));
    }

    #[test]
    fn a_worktree_with_changes_is_not_removed() {
        let repo = repo();
        let record = worktree_of(&repo, "kept-000005");
        fs::write(record.path.join("a.txt"), "changed").unwrap();
        assert!(GitCli::new().remove(&record).is_err());
        assert!(record.path.join("a.txt").is_file());
    }

    fn write(dir: &Path, name: &str, text: &str) {
        let path = dir.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn commit(dir: &Path, message: &str) {
        git(dir, &["add", "-A"]);
        git(dir, &["commit", "-q", "-m", message]);
    }

    fn head(dir: &Path) -> String {
        let output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(dir).output().unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    }

    // `main` holds `a.txt`. `feature` changes `a.txt` and adds `b.txt` in two commits; then `b.txt`
    // is edited and `c.txt` is untracked. Returns the repository and the sha of the second commit.
    fn feature_repo() -> (TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        write(dir.path(), "a.txt", "a\nb\nc\n");
        commit(dir.path(), "first");
        git(dir.path(), &["checkout", "-q", "-b", "feature"]);
        write(dir.path(), "a.txt", "a\nB\nc\n");
        commit(dir.path(), "change a");
        write(dir.path(), "b.txt", "one\n");
        commit(dir.path(), "add b");
        let second = head(dir.path());
        write(dir.path(), "b.txt", "one\ntwo\n");
        write(dir.path(), "c.txt", "new\n");
        (dir, second)
    }

    fn all(base: &str) -> ChangeSource {
        ChangeSource::All { base: base.to_string() }
    }

    fn list(dir: &Path, source: &ChangeSource) -> Vec<FileChange> {
        GitCli::new().changes(dir, source, &DiffOptions::default()).unwrap()
    }

    fn named<'a>(files: &'a [FileChange], path: &str) -> &'a FileChange {
        files.iter().find(|change| change.file.path == path).unwrap_or_else(|| panic!("{path} in {files:?}"))
    }

    #[test]
    fn all_changes_run_from_the_merge_base_to_the_working_tree() {
        let (dir, _) = feature_repo();
        let files = list(dir.path(), &all("main"));
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "b.txt", "c.txt"]);
        assert_eq!(named(&files, "a.txt").file.status, ChangeStatus::Modified);
        assert_eq!((named(&files, "a.txt").file.added, named(&files, "a.txt").file.removed), (Some(1), Some(1)));
        assert_eq!(named(&files, "b.txt").file.status, ChangeStatus::Added);
        assert_eq!(named(&files, "c.txt").file.status, ChangeStatus::Untracked);
        assert!(matches!(&named(&files, "a.txt").patch, PatchState::Text(text) if text.contains("-b\n+B\n")));
        assert_eq!(named(&files, "c.txt").patch, PatchState::Text("@@ -0,0 +1,1 @@\n+new\n".to_string()));
    }

    #[test]
    fn uncommitted_changes_are_against_head() {
        let (dir, _) = feature_repo();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["b.txt", "c.txt"]);
        assert_eq!(named(&files, "b.txt").file.status, ChangeStatus::Modified);
    }

    #[test]
    fn one_commit_is_against_its_parent() {
        let (dir, second) = feature_repo();
        let files = list(dir.path(), &ChangeSource::Commit { sha: second });
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file.path, "b.txt");
        assert_eq!(files[0].file.status, ChangeStatus::Added);
    }

    #[test]
    fn a_root_commit_is_against_the_empty_tree() {
        let (dir, _) = feature_repo();
        let output = Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(dir.path()).output().unwrap();
        let root = String::from_utf8(output.stdout).unwrap().trim().to_string();
        let files = list(dir.path(), &ChangeSource::Commit { sha: root });
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].file.status, ChangeStatus::Added);
    }

    #[test]
    fn a_missing_base_is_reported_as_base_not_found() {
        let (dir, _) = feature_repo();
        let result = GitCli::new().changes(dir.path(), &all("nope"), &DiffOptions::default());
        assert_eq!(result, Err(GitError::BaseNotFound("nope".to_string())));
        let option_like = GitCli::new().changes(dir.path(), &all("-x"), &DiffOptions::default());
        assert_eq!(option_like, Err(GitError::BaseNotFound("-x".to_string())));
        assert!(matches!(GitCli::new().commits(dir.path(), "nope"), Err(GitError::BaseNotFound(_))));
    }

    #[test]
    fn names_with_spaces_and_korean_letters_get_their_patch() {
        let (dir, _) = feature_repo();
        write(dir.path(), "dir/my file.txt", "x\n");
        write(dir.path(), "한글 이름.txt", "y\n");
        git(dir.path(), &["add", "-A"]);
        write(dir.path(), "dir/my file.txt", "x\nz\n");
        write(dir.path(), "-dash.txt", "d\n");
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        for name in ["dir/my file.txt", "한글 이름.txt", "-dash.txt"] {
            assert!(matches!(&named(&files, name).patch, PatchState::Text(text) if !text.is_empty()), "{name}");
        }
    }

    #[test]
    fn hiding_whitespace_drops_a_file_that_only_changed_in_whitespace() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        write(dir.path(), "w.txt", "a\n  b\n");
        write(dir.path(), "k.txt", "k\n");
        commit(dir.path(), "first");
        write(dir.path(), "w.txt", "a\n    b\n");
        write(dir.path(), "k.txt", "k\nmore\n");
        let shown = GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap();
        assert_eq!(shown.len(), 2);
        let hidden = GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions { ignore_whitespace: true }).unwrap();
        assert_eq!(hidden.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["k.txt"]);
    }

    #[test]
    fn a_patch_over_the_budget_is_too_large_until_asked_for() {
        let dir = repo();
        write(dir.path(), "big.txt", &"abcdef\n".repeat(400_000));
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        let big = named(&files, "big.txt");
        assert_eq!(big.patch, PatchState::TooLarge);
        // Too big to be read for a count.
        assert_eq!(big.file.added, None);
        let patch = GitCli::new().file_patch(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default(), "big.txt").unwrap();
        assert!(patch.len() > 2 * 1024 * 1024);
    }

    #[test]
    fn an_untracked_binary_file_is_binary() {
        let dir = repo();
        fs::write(dir.path().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(named(&files, "pic.bin").patch, PatchState::Binary);
    }

    #[test]
    fn a_tracked_binary_file_is_binary() {
        let dir = repo();
        fs::write(dir.path().join("pic.bin"), [0u8, 1, 2, 3]).unwrap();
        commit(dir.path(), "pic");
        fs::write(dir.path().join("pic.bin"), [0u8, 9, 9, 9]).unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(named(&files, "pic.bin").patch, PatchState::Binary);
    }

    #[test]
    fn a_repository_with_no_commit_lists_its_changes() {
        let dir = tempfile::tempdir().unwrap();
        git(dir.path(), &["init", "-q"]);
        fs::write(dir.path().join("a.txt"), "a\n").unwrap();
        git(dir.path(), &["add", "a.txt"]);
        fs::write(dir.path().join("b.txt"), "b\n").unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert_eq!(files.iter().map(|c| c.file.path.as_str()).collect::<Vec<_>>(), vec!["a.txt", "b.txt"]);
        // A repository with no commit has no `main` to compare with.
        assert_eq!(GitCli::new().commits(dir.path(), "main"), Err(GitError::BaseNotFound("main".to_string())));
    }

    #[test]
    fn a_subfolder_sees_root_relative_paths() {
        let (dir, _) = feature_repo();
        write(dir.path(), "sub/x.txt", "x\n");
        let files = list(&dir.path().join("sub"), &ChangeSource::Uncommitted);
        assert!(files.iter().any(|c| c.file.path == "sub/x.txt"));
    }

    #[test]
    fn commits_come_newest_first_with_subject_and_author() {
        let (dir, second) = feature_repo();
        let commits = GitCli::new().commits(dir.path(), "main").unwrap();
        assert_eq!(commits.iter().map(|c| c.subject.as_str()).collect::<Vec<_>>(), vec!["add b", "change a"]);
        assert_eq!(commits[0].sha, second);
        assert_eq!(commits[0].author, "t");
        assert!(commits[0].time > 0);
    }

    #[test]
    fn branches_name_the_current_branch_and_the_default_base() {
        let (dir, _) = feature_repo();
        let info = GitPort::branches(&GitCli::new(), dir.path()).unwrap();
        assert_eq!(info.current.as_deref(), Some("feature"));
        assert_eq!(info.head_label, "feature");
        assert_eq!(info.default_base.as_deref(), Some("main"));
        assert_eq!(info.branches, vec!["feature".to_string(), "main".to_string()]);
    }

    #[test]
    fn a_detached_head_is_labelled_with_its_sha() {
        let (dir, second) = feature_repo();
        git(dir.path(), &["checkout", "-q", "--detach", &second]);
        let info = GitPort::branches(&GitCli::new(), dir.path()).unwrap();
        assert_eq!(info.current, None);
        assert!(second.starts_with(&info.head_label) && info.head_label.len() >= 7);
    }

    #[test]
    fn file_lines_read_the_working_tree_or_the_commit() {
        let (dir, second) = feature_repo();
        let now = GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "b.txt").unwrap();
        assert_eq!(now, vec!["one".to_string(), "two".to_string()]);
        let then = GitCli::new().file_lines(dir.path(), &ChangeSource::Commit { sha: second }, "b.txt").unwrap();
        assert_eq!(then, vec!["one".to_string()]);
        assert!(GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "../x").is_err());
        assert!(GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "/etc/hosts").is_err());
        assert!(GitCli::new().file_lines(dir.path(), &ChangeSource::Uncommitted, "a/../../x").is_err());
    }

    #[test]
    fn a_folder_that_is_not_a_repository_says_so_for_changes() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()), Err(GitError::NotARepository));
    }

    #[test]
    fn a_missing_git_is_reported_as_a_message_for_changes() {
        let dir = repo();
        let cli = GitCli::with_program("forge-no-such-git", Duration::from_secs(5));
        let error = cli.changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions::default()).unwrap_err();
        assert_eq!(error, GitError::Failed("git is not installed".to_string()));
    }

    #[test]
    fn a_quoted_file_name_does_not_corrupt_the_previous_patch() {
        let dir = repo();
        write(dir.path(), "a.txt", "a\nB\nc\n");
        write(dir.path(), "q\"t.txt", "x\n");
        git(dir.path(), &["add", "-N", "q\"t.txt"]);
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        assert!(matches!(&named(&files, "a.txt").patch, PatchState::Text(text) if !text.contains("q\\\"t") && !text.contains("+x")));
        assert_eq!(named(&files, "q\"t.txt").patch, PatchState::Binary);
        let hidden = GitCli::new().changes(dir.path(), &ChangeSource::Uncommitted, &DiffOptions { ignore_whitespace: true }).unwrap();
        assert!(hidden.iter().any(|c| c.file.path == "q\"t.txt"));
    }

    #[cfg(unix)]
    #[test]
    fn an_untracked_symlink_is_never_followed() {
        let dir = repo();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "SECRET\n").unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret.txt"), dir.path().join("link.txt")).unwrap();
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        let link = named(&files, "link.txt");
        assert_eq!(link.patch, PatchState::Binary);
        assert_eq!((link.file.added, link.file.removed), (None, None));
        assert_eq!(link.file.status, ChangeStatus::Untracked);
    }

    #[test]
    fn an_untracked_file_over_the_budget_is_not_read() {
        let dir = repo();
        write(dir.path(), "big.txt", &"abcdef\n".repeat(400_000));
        let files = list(dir.path(), &ChangeSource::Uncommitted);
        let big = named(&files, "big.txt");
        assert_eq!(big.patch, PatchState::TooLarge);
        assert_eq!(big.file.added, None);
        assert_eq!(big.file.status, ChangeStatus::Untracked);
    }
}
