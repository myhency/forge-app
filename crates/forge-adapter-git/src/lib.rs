use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_ports::{ChangeStatus, ChangedFile, GitError, GitPort};

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

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
        let deadline = Instant::now() + self.timeout;
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

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use forge_ports::{ChangeStatus, ChangedFile, GitError, GitPort};
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
}
