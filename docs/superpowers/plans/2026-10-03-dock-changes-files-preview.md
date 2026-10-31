# Plan 21B: Changes, Files and Preview panels — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Three more panels in the right-hand dock: Changes (git status and diff, read only), Files (tree, filter, `?` content search, read only viewer) and Preview (tabs, address bar, history; the page opens in the system browser).

**Architecture:** Same layers as Plan 21A. Pure state in `forge-features` (`Panel` variants and the Changes+Preview shared column, `FileTree`, `PreviewTabs`). Three new ports in `forge-ports` (`GitPort`, `FilesPort`, `BrowserPort`) with adapters `forge-adapter-git` (the `git` CLI), `forge-adapter-files` (`std::fs`) and `forge-adapter-browser` (`open`). `forge-entities` gains `parse_unified_diff` (git's diff text into the existing `FileDiff`, drawn by the existing `DiffView`) and `code_html` (the existing highlighter, public). Widgets `ChangesPanel`, `FilesPanel`, `PreviewPanel` sit in the existing `PanelFrame`. Port calls are blocking, so panels run them through `run_blocking` (`tokio::task::spawn_blocking`) and never on the UI thread.

**Tech Stack:** Rust, Dioxus 0.7.10 desktop, `tokio`, `tempfile` (dev), the `git` CLI, `similar` and `syntect` (already in `forge-entities`).

**Spec:** `docs/superpowers/specs/2026-10-03-right-dock-panels-design.md` (sections "Changes", "Files", "Preview"; the Preview section was amended after the spike).

## Global Constraints

- Layers as in the repo: pure logic in `forge-features`/`forge-entities`, I/O only in adapters, the composition root (`forge-app/src/main.rs`) is the only place that names concrete adapters. `forge-widgets` never names an adapter.
- UI text is English. Strings, exactly: `No changes to show`, `Cannot show this file`, `Folder is empty`, `Refresh`, `Open in browser`, `New tab`, `Loading…`.
- Everything read only: no stage, revert, commit, no file writes, no editor.
- `git` is run with `--no-optional-locks`, `GIT_TERMINAL_PROMPT=0`, `LC_ALL=C`, a 10 s timeout, and never with a shell.
- Files: `.git` and `node_modules` are skipped; a file over 1 MB (`READ_LIMIT` = 1_048_576 bytes) or with a NUL byte in its first 8000 bytes is "Cannot show this file"; a path that resolves outside the session folder (`..`, absolute, symlink) is refused in the adapter.
- Preview: only `http://` and `https://` are ever handed to the system browser. A bare `localhost`, `127.0.0.1` or `[::1]` gets `http://`, any other bare host gets `https://`. Nothing is drawn inside the panel (the `<iframe>` spike failed: `dioxus-desktop` opens every `http(s)` navigation in the system browser).
- Changes and Preview share one dock column when both are open; Files and Terminal get their own column.
- The shortcuts ⇧⌘F and ⇧⌘Y from the spec are not built (the shortcut table rejects Shift today); Files is opened from the `⋮` menu. They go to the backlog (Task 12).
- The diff limit is the existing `FileDiff` truncation (300 lines, "… N more lines"), not a separate "Diff too large to show" message.
- Tests first for pure logic and adapters; widgets are checked in the real app (Task 11). Every task ends with `cargo test` for the crates it touched passing and a commit with the trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Work stays on branch `plan-21` (base branch `master`).

## Review Focus

1. A session folder inside a git repository (a subfolder, not its root): Changes still lists the root-relative paths and the diff of each opens (Task 3 test, Task 8, Task 11).
2. A file name with spaces or non-ASCII characters, a renamed file, and a repository with no commit yet (Task 3 tests).
3. A folder that is not a git repository, or has no changes: "No changes to show", not an error banner (Task 3 test, Task 8).
4. A symlink inside the folder that points outside it, and a `..` path: refused, never read (Task 4 tests).
5. A binary file, a file over 1 MB and a very long single-line file: "Cannot show this file" or shown without freezing the UI (Task 4 tests, Task 9).
6. Preview input that is not a web address (`javascript:alert(1)`, `file:///etc/passwd`, `foo bar`): nothing is opened (Task 5, Task 6 tests).
7. A diff that is empty or binary (`Binary files differ`): "No textual changes to show", no panic (Task 2 test, Task 8).

---

### Task 1: Dock state for the new panels

**Files:**
- Modify: `crates/forge-features/src/dock.rs`
- Modify: `crates/forge-widgets/src/dock.rs` (one match arm, so it still compiles)

**Interfaces:**
- Produces: `Panel::{Terminal, Changes, Preview, Files}` and `Panel::title()` (`"Terminal"`, `"Changes"`, `"Preview"`, `"Files"`); `DockState::toggle` puts `Changes` and `Preview` in one column when the other is open.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/forge-features/src/dock.rs`:

```rust
    #[test]
    fn preview_joins_the_changes_column_and_changes_joins_the_preview_column() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Preview);
        assert_eq!(dock.columns.len(), 1);
        assert_eq!(dock.columns[0].panels, vec![Panel::Changes, Panel::Preview]);

        let mut dock = DockState::default();
        dock.toggle(Panel::Preview);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.columns.len(), 1);
        assert_eq!(dock.columns[0].panels, vec![Panel::Preview, Panel::Changes]);
    }

    #[test]
    fn files_and_the_terminal_get_their_own_columns() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Files);
        dock.toggle(Panel::Terminal);
        assert_eq!(dock.columns.len(), 3);
    }

    #[test]
    fn closing_one_of_a_shared_pair_keeps_the_column_for_the_other() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Changes);
        dock.toggle(Panel::Preview);
        dock.toggle(Panel::Changes);
        assert_eq!(dock.columns[0].panels, vec![Panel::Preview]);
        dock.toggle(Panel::Preview);
        assert!(dock.columns.is_empty());
    }

    #[test]
    fn the_new_panels_have_titles() {
        assert_eq!(Panel::Changes.title(), "Changes");
        assert_eq!(Panel::Preview.title(), "Preview");
        assert_eq!(Panel::Files.title(), "Files");
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features dock`
Expected: FAIL to compile (`no variant named Changes`).

- [ ] **Step 3: Implement**

In `crates/forge-features/src/dock.rs` replace the `Panel` enum, its `title`, and `toggle`:

```rust
/// A panel that can sit in the dock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Terminal,
    Changes,
    Preview,
    Files,
}

impl Panel {
    pub fn title(self) -> &'static str {
        match self {
            Panel::Terminal => "Terminal",
            Panel::Changes => "Changes",
            Panel::Preview => "Preview",
            Panel::Files => "Files",
        }
    }

    // Changes and Preview share one column.
    fn partner(self) -> Option<Panel> {
        match self {
            Panel::Changes => Some(Panel::Preview),
            Panel::Preview => Some(Panel::Changes),
            Panel::Terminal | Panel::Files => None,
        }
    }
}
```

```rust
    /// Opens a closed panel in a new column at the far right, or in its partner's column when the
    /// partner is open (Changes and Preview), or closes an open one. Opening a panel never closes
    /// another.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.close(panel);
            return;
        }
        if let Some(partner) = panel.partner() {
            if let Some(column) = self.columns.iter_mut().find(|column| column.panels.contains(&partner)) {
                column.panels.push(panel);
                return;
            }
        }
        self.columns.push(Column { panels: vec![panel], width: DEFAULT_COLUMN_WIDTH });
    }
```

In `crates/forge-widgets/src/dock.rs`, in the `match panel` add after the Terminal arm (each later task replaces its own name in this arm):

```rust
                                    Panel::Changes | Panel::Preview | Panel::Files => rsx! {},
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-features && cargo build -p forge-widgets`
Expected: PASS, builds.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/dock.rs crates/forge-widgets/src/dock.rs
git commit -m "feat: dock state knows Changes, Preview and Files, and Changes and Preview share a column

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Ports, the diff parser and the public highlighter

**Files:**
- Create: `crates/forge-ports/src/git_port.rs`, `crates/forge-ports/src/files_port.rs`, `crates/forge-ports/src/browser_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`
- Modify: `crates/forge-entities/src/diff.rs`, `crates/forge-entities/src/highlight.rs`, `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces (ports): `ChangeStatus` (`Modified, Added, Deleted, Renamed, Untracked, Conflicted`; `letter() -> &'static str` = `M A D R U C`), `ChangedFile { path: String, status: ChangeStatus, added: Option<usize>, removed: Option<usize> }`, `GitError { NotARepository, Failed(String) }` (Display: `Not a git repository` / the message), `trait GitPort { fn status(&self, cwd: &Path) -> Result<Vec<ChangedFile>, GitError>; fn diff(&self, cwd: &Path, path: &str) -> Result<String, GitError>; }`. `DirEntry { name: String, is_dir: bool }`, `FileContent { Text(String), Binary, TooLarge }`, `ContentHit { path: String, line: usize, text: String }`, `FilesError { Outside, Failed(String) }`, `READ_LIMIT: usize = 1_048_576`, `trait FilesPort { fn list(&self, root: &Path, dir: &str) -> Result<Vec<DirEntry>, FilesError>; fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>; fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<ContentHit>, FilesError>; }` (paths are relative to `root`, `/`-separated, `""` is the root itself). `trait BrowserPort { fn open(&self, url: &str) -> Result<(), String>; }`. All three with `Mock…` under the `mocks` feature.
- Produces (entities): `parse_unified_diff(path: &str, text: &str) -> FileDiff`, `code_html(language: &str, code: &str) -> String`, `language_of(path: &str) -> &str`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `crates/forge-entities/src/diff.rs`:

```rust
    const MODIFIED: &str = "diff --git a/a.txt b/a.txt\nindex 1..2 100644\n--- a/a.txt\n+++ b/a.txt\n@@ -1,3 +1,3 @@\n a\n-b\n+B\n c\n@@ -10,2 +10,3 @@\n x\n+y\n z\n";

    #[test]
    fn a_unified_diff_becomes_hunks_with_counts() {
        let diff = parse_unified_diff("a.txt", MODIFIED);
        assert_eq!(diff.path, "a.txt");
        assert!(!diff.new_content);
        assert_eq!((diff.added, diff.removed, diff.hidden_lines), (2, 1, 0));
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!(
            kinds(&diff),
            vec![
                (DiffLineKind::Context, "a"),
                (DiffLineKind::Removed, "b"),
                (DiffLineKind::Added, "B"),
                (DiffLineKind::Context, "c"),
                (DiffLineKind::Context, "x"),
                (DiffLineKind::Added, "y"),
                (DiffLineKind::Context, "z"),
            ]
        );
    }

    #[test]
    fn a_new_file_is_marked_and_the_header_lines_are_not_shown() {
        let text = "diff --git a/n.txt b/n.txt\nnew file mode 100644\nindex 0..1\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1,2 @@\n+one\n+two\n";
        let diff = parse_unified_diff("n.txt", text);
        assert!(diff.new_content);
        assert_eq!((diff.added, diff.removed), (2, 0));
        assert_eq!(kinds(&diff), vec![(DiffLineKind::Added, "one"), (DiffLineKind::Added, "two")]);
    }

    #[test]
    fn a_binary_or_empty_diff_has_no_hunks() {
        assert!(parse_unified_diff("b.png", "diff --git a/b.png b/b.png\nBinary files a/b.png and b/b.png differ\n").hunks.is_empty());
        assert!(parse_unified_diff("x", "").hunks.is_empty());
    }

    #[test]
    fn no_newline_markers_and_blank_context_lines_are_handled() {
        let text = "@@ -1,2 +1,2 @@\n-a\n\\ No newline at end of file\n+b\n\n";
        let diff = parse_unified_diff("f", text);
        assert_eq!(kinds(&diff), vec![(DiffLineKind::Removed, "a"), (DiffLineKind::Added, "b"), (DiffLineKind::Context, "")]);
    }

    #[test]
    fn a_long_diff_is_cut_like_a_tool_call_diff() {
        let mut text = String::from("@@ -0,0 +1,400 @@\n");
        for i in 0..400 {
            text.push_str(&format!("+line {i}\n"));
        }
        let diff = parse_unified_diff("big", &text);
        assert_eq!(diff.added, 400);
        assert_eq!(diff.hidden_lines, 100);
    }
```

Add to the `tests` module of `crates/forge-entities/src/highlight.rs`:

```rust
    #[test]
    fn code_html_highlights_known_languages_and_escapes_the_rest() {
        assert!(code_html("rs", "fn main() {}\n").contains("<span class=\"hl-"));
        assert_eq!(code_html("", "a < b"), "a &lt; b");
        assert_eq!(code_html("nosuchlang", "<x>"), "&lt;x&gt;");
    }

    #[test]
    fn the_language_is_the_file_extension() {
        assert_eq!(language_of("src/main.rs"), "rs");
        assert_eq!(language_of("a.b.py"), "py");
        assert_eq!(language_of("Makefile"), "");
        assert_eq!(language_of("dir.d/file"), "");
        assert_eq!(language_of(".gitignore"), "");
    }
```

Create `crates/forge-ports/src/git_port.rs`:

```rust
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
```

Create `crates/forge-ports/src/files_port.rs`:

```rust
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
```

Create `crates/forge-ports/src/browser_port.rs`:

```rust
#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Opens an address in the person's own browser.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait BrowserPort: Send + Sync {
    /// Only `http://` and `https://` addresses are opened. The error is a message for the person.
    fn open(&self, url: &str) -> Result<(), String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_refuse() {
        let mut mock = MockBrowserPort::new();
        mock.expect_open().returning(|_| Err("no browser".to_string()));
        assert_eq!(mock.open("http://localhost"), Err("no browser".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-entities && cargo test -p forge-ports`
Expected: FAIL to compile (`parse_unified_diff`, `code_html`, `language_of` not found; ports not declared yet).

- [ ] **Step 3: Implement**

In `crates/forge-ports/src/lib.rs` add `mod browser_port; mod files_port; mod git_port;` (alphabetical among the `mod` lines), and

```rust
pub use browser_port::BrowserPort;
pub use files_port::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};
pub use git_port::{ChangeStatus, ChangedFile, GitError, GitPort};
#[cfg(any(test, feature = "mocks"))]
pub use browser_port::MockBrowserPort;
#[cfg(any(test, feature = "mocks"))]
pub use files_port::MockFilesPort;
#[cfg(any(test, feature = "mocks"))]
pub use git_port::MockGitPort;
```

In `crates/forge-entities/src/diff.rs` add above `fn string_field`:

```rust
/// Git's unified diff of one file as a `FileDiff`. Lines before the first `@@` (the `diff --git`,
/// `index`, `---` and `+++` header) are not shown; each `@@` starts a hunk. A binary file or empty
/// text gives no hunks.
pub fn parse_unified_diff(path: &str, text: &str) -> FileDiff {
    let new_content = text.lines().any(|line| line.starts_with("new file mode"));
    let mut hunks: Vec<Vec<DiffLine>> = Vec::new();
    for line in text.lines() {
        if line.starts_with("@@") {
            hunks.push(Vec::new());
            continue;
        }
        let Some(hunk) = hunks.last_mut() else { continue };
        let (kind, rest) = match line.chars().next() {
            Some('+') => (DiffLineKind::Added, &line[1..]),
            Some('-') => (DiffLineKind::Removed, &line[1..]),
            Some(' ') => (DiffLineKind::Context, &line[1..]),
            None => (DiffLineKind::Context, ""),
            // "\ No newline at end of file" and anything else.
            Some(_) => continue,
        };
        hunk.push(DiffLine { kind, text: rest.trim_end_matches('\r').to_string() });
    }
    let count = |kind| hunks.iter().flatten().filter(|line| line.kind == kind).count();
    let (added, removed) = (count(DiffLineKind::Added), count(DiffLineKind::Removed));
    let hidden_lines = truncate_hunks(&mut hunks, MAX_DIFF_LINES);
    FileDiff { path: path.to_string(), new_content, added, removed, hunks, hidden_lines }
}
```

In `crates/forge-entities/src/highlight.rs` add above `#[cfg(test)]`:

```rust
/// The code as HTML for a `<pre>`: highlighted for the language when one matches, else only
/// escaped.
pub fn code_html(language: &str, code: &str) -> String {
    highlight_code(language, code).unwrap_or_else(|| escape_html(code))
}

/// The language token of a file path: its extension, or `""` for a name without one (a dotfile
/// like `.gitignore` has none).
pub fn language_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(0) | None => "",
        Some(dot) => &name[dot + 1..],
    }
}
```

In `crates/forge-entities/src/lib.rs` change the `diff` and add `highlight` exports:

```rust
pub use diff::{diff_for_tool, diff_for_tool_call, parse_unified_diff, DiffLine, DiffLineKind, FileDiff};
pub use highlight::{code_html, language_of};
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-entities && cargo test -p forge-ports`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports crates/forge-entities
git commit -m "feat: git, files and browser ports, a unified diff parser and a public highlighter

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The git adapter

**Files:**
- Create: `crates/forge-adapter-git/Cargo.toml`, `crates/forge-adapter-git/src/lib.rs`

**Interfaces:**
- Consumes: `GitPort`, `ChangedFile`, `ChangeStatus`, `GitError` from Task 2.
- Produces: `GitCli::new()`, `GitCli::with_program(program: impl Into<String>, timeout: Duration)`, `impl GitPort for GitCli`.

- [ ] **Step 1: Write the crate with the failing tests**

`crates/forge-adapter-git/Cargo.toml`:

```toml
[package]
name = "forge-adapter-git"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }

[dev-dependencies]
tempfile = "3"
```

`crates/forge-adapter-git/src/lib.rs` — start with only this test module (the code comes in Step 3):

```rust
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
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-git`
Expected: FAIL to compile (`GitCli` not found).

- [ ] **Step 3: Implement**

Put this above the `tests` module in `crates/forge-adapter-git/src/lib.rs`:

```rust
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
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-adapter-git`
Expected: PASS. (They run real `git`; if the machine's git config forces signing or hooks, the helper's `-c` flags already turn signing off.)

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-git Cargo.lock
git commit -m "feat: git adapter reads status and diffs with the git command

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The files adapter

**Files:**
- Create: `crates/forge-adapter-files/Cargo.toml`, `crates/forge-adapter-files/src/lib.rs`

**Interfaces:**
- Consumes: `FilesPort`, `DirEntry`, `FileContent`, `ContentHit`, `FilesError`, `READ_LIMIT` from Task 2.
- Produces: `LocalFiles` (unit struct, `LocalFiles::new()`), `impl FilesPort for LocalFiles`.

- [ ] **Step 1: Write the crate with the failing tests**

`crates/forge-adapter-files/Cargo.toml`:

```toml
[package]
name = "forge-adapter-files"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }

[dev-dependencies]
tempfile = "3"
```

`crates/forge-adapter-files/src/lib.rs` — the test module first:

```rust
#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use forge_ports::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};
    use tempfile::TempDir;

    use super::*;

    fn tree() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("README.md"), "# Title\nhello World\n").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        fs::write(root.join("src/deep/notes.txt"), "nothing here\nHELLO again\n").unwrap();
        fs::write(root.join(".git/config"), "hello\n").unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "hello\n").unwrap();
        dir
    }

    fn names(entries: &[DirEntry]) -> Vec<String> {
        entries.iter().map(|entry| format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" })).collect()
    }

    #[test]
    fn a_folder_lists_folders_first_then_by_name_without_git_or_node_modules() {
        let dir = tree();
        fs::write(dir.path().join("Zed.txt"), "").unwrap();
        fs::write(dir.path().join("apple.txt"), "").unwrap();
        let entries = LocalFiles::new().list(dir.path(), "").unwrap();
        assert_eq!(names(&entries), vec!["src/", "apple.txt", "README.md", "Zed.txt"]);
    }

    #[test]
    fn a_subfolder_is_listed_by_its_relative_path() {
        let dir = tree();
        let entries = LocalFiles::new().list(dir.path(), "src").unwrap();
        assert_eq!(names(&entries), vec!["deep/", "main.rs"]);
    }

    #[test]
    fn an_empty_folder_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(LocalFiles::new().list(dir.path(), "").unwrap(), vec![]);
    }

    #[test]
    fn a_text_file_is_read_with_its_content() {
        let dir = tree();
        assert_eq!(LocalFiles::new().read(dir.path(), "README.md").unwrap(), FileContent::Text("# Title\nhello World\n".to_string()));
    }

    #[test]
    fn a_file_with_a_nul_byte_is_binary_and_a_big_file_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.bin"), [1u8, 2, 0, 3]).unwrap();
        fs::write(dir.path().join("big.txt"), vec![b'a'; READ_LIMIT + 1]).unwrap();
        fs::write(dir.path().join("exact.txt"), vec![b'a'; READ_LIMIT]).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.read(dir.path(), "b.bin").unwrap(), FileContent::Binary);
        assert_eq!(files.read(dir.path(), "big.txt").unwrap(), FileContent::TooLarge);
        assert!(matches!(files.read(dir.path(), "exact.txt").unwrap(), FileContent::Text(_)));
    }

    #[test]
    fn text_that_is_not_utf8_is_read_lossily() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("l.txt"), [b'a', 0xff, b'b']).unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "l.txt").unwrap(), FileContent::Text("a\u{fffd}b".to_string()));
    }

    #[test]
    fn paths_that_leave_the_folder_are_refused() {
        let dir = tree();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path().join("secret.txt"), dir.path().join("link.txt")).unwrap();
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.read(dir.path(), "../secret.txt"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "src/../../x"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "/etc/passwd"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "link.txt"), Err(FilesError::Outside));
        assert_eq!(files.list(dir.path(), "linkdir"), Err(FilesError::Outside));
        assert_eq!(files.list(dir.path(), ".."), Err(FilesError::Outside));
    }

    #[test]
    fn a_missing_file_is_a_message_not_a_panic() {
        let dir = tree();
        assert!(matches!(LocalFiles::new().read(dir.path(), "nope.txt"), Err(FilesError::Failed(_))));
        assert!(matches!(LocalFiles::new().list(&dir.path().join("gone"), ""), Err(FilesError::Failed(_))));
    }

    #[test]
    fn search_finds_lines_in_any_case_in_path_order_and_skips_git_and_node_modules() {
        let dir = tree();
        let hits = LocalFiles::new().search(dir.path(), "hello", 50).unwrap();
        let expected = vec![
            ContentHit { path: "README.md".to_string(), line: 2, text: "hello World".to_string() },
            ContentHit { path: "src/deep/notes.txt".to_string(), line: 2, text: "HELLO again".to_string() },
            ContentHit { path: "src/main.rs".to_string(), line: 2, text: "println!(\"hello\");".to_string() },
        ];
        assert_eq!(hits, expected);
    }

    #[test]
    fn search_stops_at_the_limit_and_ignores_an_empty_query_binary_files_and_symlinks() {
        let dir = tree();
        fs::write(dir.path().join("b.bin"), b"hello\0hello").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("o.txt"), "hello").unwrap();
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.search(dir.path(), "hello", 2).unwrap().len(), 2);
        assert_eq!(files.search(dir.path(), "", 10).unwrap(), vec![]);
        let all = files.search(dir.path(), "hello", 50).unwrap();
        assert!(all.iter().all(|hit| hit.path != "b.bin" && !hit.path.starts_with("linkdir")), "{all:?}");
    }

    #[test]
    fn a_very_long_line_is_shortened_in_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("long.txt"), format!("needle {}\n", "x".repeat(5000))).unwrap();
        let hits = LocalFiles::new().search(dir.path(), "needle", 5).unwrap();
        assert_eq!(hits[0].text.chars().count(), 200);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-files`
Expected: FAIL to compile (`LocalFiles` not found).

- [ ] **Step 3: Implement**

Above the tests in `crates/forge-adapter-files/src/lib.rs`:

```rust
use std::fs;
use std::path::{Component, Path, PathBuf};

use forge_ports::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};

const SKIPPED: [&str; 2] = [".git", "node_modules"];
// A NUL byte in this many first bytes makes a file binary.
const BINARY_SNIFF: usize = 8000;
// A search looks at no more files than this, so a huge tree cannot keep it busy for long.
const SEARCH_FILE_LIMIT: usize = 20_000;
const HIT_TEXT_CHARS: usize = 200;

/// Reads files under a root with `std::fs`. A path is resolved (symlinks included) and must stay
/// inside the root.
#[derive(Default)]
pub struct LocalFiles;

impl LocalFiles {
    pub fn new() -> Self {
        Self
    }
}

fn failed(error: impl std::fmt::Display) -> FilesError {
    FilesError::Failed(error.to_string())
}

fn resolve(root: &Path, relative: &str) -> Result<PathBuf, FilesError> {
    let root = root.canonicalize().map_err(failed)?;
    let relative = Path::new(relative);
    if relative.is_absolute() || relative.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err(FilesError::Outside);
    }
    let resolved = root.join(relative).canonicalize().map_err(failed)?;
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(FilesError::Outside)
    }
}

fn is_skipped(name: &str) -> bool {
    SKIPPED.contains(&name)
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0)
}

// A folder's entries sorted folders first, then by lower-case name.
fn sorted_entries(dir: &Path) -> Result<Vec<DirEntry>, FilesError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_skipped(&name) {
            continue;
        }
        // `metadata` follows a symlink; a broken one counts as a file.
        let is_dir = fs::metadata(entry.path()).map(|meta| meta.is_dir()).unwrap_or(false);
        entries.push(DirEntry { name, is_dir });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

impl FilesPort for LocalFiles {
    fn list(&self, root: &Path, dir: &str) -> Result<Vec<DirEntry>, FilesError> {
        sorted_entries(&resolve(root, dir)?)
    }

    fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError> {
        let file = resolve(root, path)?;
        let length = fs::metadata(&file).map_err(failed)?.len();
        if length > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        if looks_binary(&bytes) {
            return Ok(FileContent::Binary);
        }
        Ok(FileContent::Text(String::from_utf8_lossy(&bytes).into_owned()))
    }

    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<ContentHit>, FilesError> {
        let root = root.canonicalize().map_err(failed)?;
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        if needle.is_empty() || limit == 0 {
            return Ok(hits);
        }
        let mut visited = 0;
        search_dir(&root, "", &needle, limit, &mut visited, &mut hits);
        Ok(hits)
    }
}

// Depth first in name order; symlinks are not followed, so a link cannot lead out of the root.
fn search_dir(dir: &Path, prefix: &str, needle: &str, limit: usize, visited: &mut usize, hits: &mut Vec<ContentHit>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if hits.len() >= limit || *visited >= SEARCH_FILE_LIMIT {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if is_skipped(&name) || kind.is_symlink() {
            continue;
        }
        let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if kind.is_dir() {
            search_dir(&entry.path(), &path, needle, limit, visited, hits);
            continue;
        }
        *visited += 1;
        if entry.metadata().map(|meta| meta.len() > READ_LIMIT as u64).unwrap_or(true) {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else { continue };
        if looks_binary(&bytes) {
            continue;
        }
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.to_lowercase().contains(needle) {
                let text: String = line.trim().chars().take(HIT_TEXT_CHARS).collect();
                hits.push(ContentHit { path: path.clone(), line: index + 1, text });
                if hits.len() >= limit {
                    return;
                }
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-adapter-files`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-files Cargo.lock
git commit -m "feat: files adapter lists, reads and searches inside the session folder

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The browser adapter

**Files:**
- Create: `crates/forge-adapter-browser/Cargo.toml`, `crates/forge-adapter-browser/src/lib.rs`

**Interfaces:**
- Consumes: `BrowserPort` from Task 2.
- Produces: `SystemBrowser::new()` (runs macOS `open`), `SystemBrowser::with_program(program: impl Into<String>)`, `impl BrowserPort for SystemBrowser`.

- [ ] **Step 1: Write the crate with the failing tests**

`crates/forge-adapter-browser/Cargo.toml`:

```toml
[package]
name = "forge-adapter-browser"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }
```

`crates/forge-adapter-browser/src/lib.rs` — test module first:

```rust
#[cfg(test)]
mod tests {
    use forge_ports::BrowserPort;

    use super::*;

    #[test]
    fn only_web_addresses_are_opened() {
        // The program does not exist, so an error that mentions http can only come from the check
        // that runs before any program starts.
        let browser = SystemBrowser::with_program("forge-no-such-program");
        for url in ["file:///etc/passwd", "javascript:alert(1)", "ftp://host/x", "localhost:3000", "", "http://", "https://a b"] {
            let error = browser.open(url).unwrap_err();
            assert!(error.contains("http"), "{url}: {error}");
        }
    }

    #[test]
    fn a_web_address_is_passed_to_the_program_as_one_argument() {
        assert_eq!(SystemBrowser::with_program("true").open("http://localhost:3000/a?b=1&c=2"), Ok(()));
    }

    #[test]
    fn a_program_that_fails_or_is_missing_is_reported_as_a_message() {
        assert!(SystemBrowser::with_program("false").open("https://example.com").is_err());
        let error = SystemBrowser::with_program("forge-no-such-program").open("https://example.com").unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-browser`
Expected: FAIL to compile (`SystemBrowser` not found).

- [ ] **Step 3: Implement**

Above the tests:

```rust
use std::process::{Command, Stdio};

use forge_ports::BrowserPort;

/// Opens addresses with the macOS `open` command, which hands them to the default browser.
pub struct SystemBrowser {
    program: String,
}

impl SystemBrowser {
    pub fn new() -> Self {
        Self::with_program("open")
    }

    pub fn with_program(program: impl Into<String>) -> Self {
        Self { program: program.into() }
    }
}

impl Default for SystemBrowser {
    fn default() -> Self {
        Self::new()
    }
}

fn is_web_address(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    matches!(rest, Some(rest) if !rest.is_empty() && !url.chars().any(char::is_whitespace))
}

impl BrowserPort for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        if !is_web_address(url) {
            return Err("Only http and https addresses can be opened".to_string());
        }
        // The address is one argument, never part of a shell command line.
        let status = Command::new(&self.program)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not run {}: {error}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} could not open the address", self.program))
        }
    }
}
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-adapter-browser`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-browser Cargo.lock
git commit -m "feat: browser adapter opens web addresses in the default browser

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Pure state: file tree, preview tabs, tool activity, run_blocking

**Files:**
- Create: `crates/forge-features/src/file_tree.rs`, `crates/forge-features/src/preview_tabs.rs`, `crates/forge-features/src/tool_activity.rs`, `crates/forge-features/src/blocking.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/Cargo.toml`

**Interfaces:**
- Consumes: `forge_ports::DirEntry`; `forge_domain::{Session, Message, MessageRole, ToolCall, ToolResult}`.
- Produces: `FilesQuery<'a> { Filter(&'a str), Content(&'a str) }`, `parse_files_query(input: &str) -> FilesQuery<'_>`; `TreeRow { path, name, depth, is_dir, expanded }`; `FileTree` (`set_children(&mut self, dir: &str, entries: Vec<DirEntry>)`, `is_loaded(&self, dir: &str) -> bool`, `toggle(&mut self, dir: &str) -> bool` returns true when the folder is now open and its entries are not loaded, `rows(&self, filter: &str) -> Vec<TreeRow>`); `normalize_url(input: &str) -> Option<String>`; `PreviewTabId(pub u64)`; `PreviewTabs` (`open() -> PreviewTabId`, `close(id)`, `select(id)`, `ids() -> Vec<PreviewTabId>`, `active() -> Option<PreviewTabId>`, `is_empty()`, `navigate(id, url: String)`, `back(id) -> Option<String>`, `forward(id) -> Option<String>`, `current(id) -> Option<String>`, `can_back(id)`, `can_forward(id)`, `label(id) -> String`); `finished_tool_calls(session: &Session) -> usize`; `async fn run_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T>`.

- [ ] **Step 1: Write the failing tests**

`crates/forge-features/src/file_tree.rs` (tests only for now):

```rust
#[cfg(test)]
mod tests {
    use forge_ports::DirEntry;

    use super::*;

    fn dir(name: &str) -> DirEntry {
        DirEntry { name: name.to_string(), is_dir: true }
    }

    fn file(name: &str) -> DirEntry {
        DirEntry { name: name.to_string(), is_dir: false }
    }

    fn paths(rows: &[TreeRow]) -> Vec<(&str, usize)> {
        rows.iter().map(|row| (row.path.as_str(), row.depth)).collect()
    }

    fn sample() -> FileTree {
        let mut tree = FileTree::default();
        tree.set_children("", vec![dir("src"), file("README.md")]);
        tree
    }

    #[test]
    fn the_root_entries_show_and_folders_start_closed() {
        let tree = sample();
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(!tree.rows("")[0].expanded);
    }

    #[test]
    fn opening_a_folder_asks_for_its_entries_once() {
        let mut tree = sample();
        assert!(tree.toggle("src"), "not loaded yet");
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)], "nothing to show until loaded");
        tree.set_children("src", vec![file("main.rs"), dir("deep")]);
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("src/main.rs", 1), ("src/deep", 1), ("README.md", 0)]);
        assert!(!tree.toggle("src"), "closing needs no load");
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(!tree.toggle("src"), "opening again: already loaded");
        assert_eq!(tree.rows("").len(), 4);
    }

    #[test]
    fn an_empty_folder_stays_open_with_no_rows_under_it() {
        let mut tree = sample();
        tree.toggle("src");
        tree.set_children("src", vec![]);
        assert!(tree.is_loaded("src"));
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(tree.rows("")[0].expanded);
    }

    #[test]
    fn the_filter_keeps_names_that_match_in_any_case_and_the_folders_that_hold_them() {
        let mut tree = sample();
        tree.toggle("src");
        tree.set_children("src", vec![file("main.rs"), file("lib.rs")]);
        assert_eq!(paths(&tree.rows("MAIN")), vec![("src", 0), ("src/main.rs", 1)]);
        assert_eq!(paths(&tree.rows("readme")), vec![("README.md", 0)]);
        assert_eq!(paths(&tree.rows("nothing")), Vec::<(&str, usize)>::new());
    }

    #[test]
    fn a_filter_shows_matches_in_folders_that_were_closed() {
        let mut tree = sample();
        tree.set_children("src", vec![file("main.rs")]);
        let rows = tree.rows("main");
        assert_eq!(paths(&rows), vec![("src", 0), ("src/main.rs", 1)]);
        assert!(rows[0].expanded);
    }

    #[test]
    fn a_question_mark_starts_a_content_search_and_anything_else_is_a_name_filter() {
        assert_eq!(parse_files_query("?hello world "), FilesQuery::Content("hello world"));
        assert_eq!(parse_files_query("  ?x"), FilesQuery::Content("x"));
        assert_eq!(parse_files_query("?"), FilesQuery::Content(""));
        assert_eq!(parse_files_query(" main "), FilesQuery::Filter("main"));
        assert_eq!(parse_files_query(""), FilesQuery::Filter(""));
    }
}
```

`crates/forge-features/src/preview_tabs.rs` (tests only for now):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hosts_get_http_and_other_hosts_get_https() {
        assert_eq!(normalize_url("localhost:3000").as_deref(), Some("http://localhost:3000"));
        assert_eq!(normalize_url("localhost").as_deref(), Some("http://localhost"));
        assert_eq!(normalize_url("LOCALHOST:8080/a?b=1").as_deref(), Some("http://LOCALHOST:8080/a?b=1"));
        assert_eq!(normalize_url("127.0.0.1:5173").as_deref(), Some("http://127.0.0.1:5173"));
        assert_eq!(normalize_url("[::1]:3000").as_deref(), Some("http://[::1]:3000"));
        assert_eq!(normalize_url("example.com").as_deref(), Some("https://example.com"));
        assert_eq!(normalize_url("  example.com/path  ").as_deref(), Some("https://example.com/path"));
    }

    #[test]
    fn an_address_with_a_web_scheme_is_kept_as_typed() {
        assert_eq!(normalize_url("http://example.com").as_deref(), Some("http://example.com"));
        assert_eq!(normalize_url("HTTPS://example.com/x").as_deref(), Some("HTTPS://example.com/x"));
    }

    #[test]
    fn input_that_is_not_a_web_address_gives_none() {
        for input in ["", "   ", "file:///etc/passwd", "javascript:alert(1)", "ftp://x", "foo bar", "mailto:a@b.c", "http://", "a:b"] {
            assert_eq!(normalize_url(input), None, "{input:?}");
        }
    }

    #[test]
    fn a_new_tab_is_empty_and_active() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        assert_eq!(tabs.active(), Some(tab));
        assert_eq!(tabs.current(tab), None);
        assert_eq!(tabs.label(tab), "New tab");
        assert!(!tabs.can_back(tab) && !tabs.can_forward(tab));
    }

    #[test]
    fn navigating_builds_a_history_that_back_and_forward_walk() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        tabs.navigate(tab, "http://a.test/".to_string());
        tabs.navigate(tab, "http://b.test/".to_string());
        assert_eq!(tabs.current(tab).as_deref(), Some("http://b.test/"));
        assert!(tabs.can_back(tab) && !tabs.can_forward(tab));
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a.test/"));
        assert!(!tabs.can_back(tab) && tabs.can_forward(tab));
        assert_eq!(tabs.back(tab), None, "already at the start");
        assert_eq!(tabs.forward(tab).as_deref(), Some("http://b.test/"));
        assert_eq!(tabs.forward(tab), None, "already at the end");
    }

    #[test]
    fn navigating_after_going_back_drops_the_forward_pages_and_the_same_address_twice_is_one_entry() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        for url in ["http://a/", "http://b/", "http://c/"] {
            tabs.navigate(tab, url.to_string());
        }
        tabs.back(tab);
        tabs.back(tab);
        tabs.navigate(tab, "http://d/".to_string());
        assert!(!tabs.can_forward(tab));
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a/"));
        tabs.forward(tab);
        tabs.navigate(tab, "http://d/".to_string());
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a/"), "no duplicate entry");
    }

    #[test]
    fn the_label_is_the_host_of_the_current_address() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        tabs.navigate(tab, "http://localhost:3000/app?x=1".to_string());
        assert_eq!(tabs.label(tab), "localhost:3000");
        tabs.navigate(tab, "https://example.com".to_string());
        assert_eq!(tabs.label(tab), "example.com");
    }

    #[test]
    fn closing_the_active_tab_selects_a_neighbour_and_unknown_tabs_are_ignored() {
        let mut tabs = PreviewTabs::default();
        let (a, b, c) = (tabs.open(), tabs.open(), tabs.open());
        tabs.select(b);
        tabs.close(b);
        assert_eq!(tabs.active(), Some(c));
        tabs.close(c);
        assert_eq!(tabs.active(), Some(a));
        tabs.close(PreviewTabId(999));
        tabs.navigate(PreviewTabId(999), "http://x/".to_string());
        assert_eq!(tabs.back(PreviewTabId(999)), None);
        tabs.close(a);
        assert!(tabs.is_empty());
        assert_eq!(tabs.active(), None);
    }
}
```

`crates/forge-features/src/tool_activity.rs`:

```rust
use forge_domain::Session;

/// How many tool calls in the session have a result. It grows each time a tool call finishes, so
/// a panel that shows the files on disk reloads when it changes.
pub fn finished_tool_calls(session: &Session) -> usize {
    session.messages.iter().flat_map(|message| &message.tool_calls).filter(|call| call.result.is_some()).count()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_domain::{Message, MessageRole, PermissionMode, SessionId, SessionUsage, ToolCall, ToolResult};

    use super::*;

    fn empty_session() -> Session {
        Session {
            id: SessionId(String::new()),
            project_path: PathBuf::from("/work/a"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: SessionUsage::default(),
        }
    }

    fn call(result: bool) -> ToolCall {
        ToolCall {
            id: "t".to_string(),
            name: "Edit".to_string(),
            input: serde_json::json!({}),
            result: result.then(|| ToolResult { output: "ok".to_string(), is_error: false }),
        }
    }

    #[test]
    fn only_tool_calls_with_a_result_count() {
        let mut session = empty_session();
        assert_eq!(finished_tool_calls(&session), 0);
        let mut message = Message::new(MessageRole::Assistant);
        message.tool_calls = vec![call(true), call(false), call(true)];
        session.messages.push(message);
        assert_eq!(finished_tool_calls(&session), 2);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features`
Expected: FAIL to compile (modules not declared).

- [ ] **Step 3: Implement**

`crates/forge-features/Cargo.toml`: change the tokio line to `tokio = { version = "1", features = ["time", "rt"] }` and add under `[dev-dependencies]` nothing new (`serde_json` is there).

`crates/forge-features/src/blocking.rs`:

```rust
/// Runs blocking work (a `git` process, a folder read) off the UI thread and waits for it without
/// blocking. `None` if the work panicked.
pub async fn run_blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    tokio::task::spawn_blocking(work).await.ok()
}
```

Above the tests in `file_tree.rs`:

```rust
use std::collections::{HashMap, HashSet};

use forge_ports::DirEntry;

/// What the Files filter field holds: a name filter, or, after a leading `?`, a text to search for
/// inside the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesQuery<'a> {
    Filter(&'a str),
    Content(&'a str),
}

pub fn parse_files_query(input: &str) -> FilesQuery<'_> {
    match input.trim_start().strip_prefix('?') {
        Some(rest) => FilesQuery::Content(rest.trim()),
        None => FilesQuery::Filter(input.trim()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// From the session folder, `/`-separated.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

/// The folders the Files panel has read so far and which of them are open. Folders are read when
/// they are first opened; the root (`""`) is always open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileTree {
    children: HashMap<String, Vec<DirEntry>>,
    open: HashSet<String>,
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

impl FileTree {
    pub fn set_children(&mut self, dir: &str, entries: Vec<DirEntry>) {
        self.children.insert(dir.to_string(), entries);
    }

    pub fn is_loaded(&self, dir: &str) -> bool {
        self.children.contains_key(dir)
    }

    /// Opens a closed folder or closes an open one. True when the folder is now open and its
    /// entries have not been read yet, so the caller must read them.
    pub fn toggle(&mut self, dir: &str) -> bool {
        if self.open.remove(dir) {
            return false;
        }
        self.open.insert(dir.to_string());
        !self.is_loaded(dir)
    }

    /// The visible rows, depth first. With a filter, only the entries whose name contains it (any
    /// case) and the folders that hold such an entry among what has been read; those folders show
    /// open.
    pub fn rows(&self, filter: &str) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.collect("", 0, &filter.to_lowercase(), &mut rows);
        rows
    }

    fn matches(name: &str, filter: &str) -> bool {
        name.to_lowercase().contains(filter)
    }

    // Whether anything read below `dir` has a name that matches.
    fn holds_match(&self, dir: &str, filter: &str) -> bool {
        self.children.get(dir).is_some_and(|entries| {
            entries.iter().any(|entry| Self::matches(&entry.name, filter) || (entry.is_dir && self.holds_match(&join(dir, &entry.name), filter)))
        })
    }

    fn collect(&self, dir: &str, depth: usize, filter: &str, rows: &mut Vec<TreeRow>) {
        let Some(entries) = self.children.get(dir) else { return };
        for entry in entries {
            let path = join(dir, &entry.name);
            let (show, expanded) = if filter.is_empty() {
                (true, entry.is_dir && self.open.contains(&path))
            } else {
                let holds = entry.is_dir && self.holds_match(&path, filter);
                (Self::matches(&entry.name, filter) || holds, holds)
            };
            if !show {
                continue;
            }
            rows.push(TreeRow { path: path.clone(), name: entry.name.clone(), depth, is_dir: entry.is_dir, expanded });
            if expanded {
                self.collect(&path, depth + 1, filter, rows);
            }
        }
    }
}
```

Above the tests in `preview_tabs.rs`:

```rust
/// A web address from what was typed in the address bar: an address with an `http` or `https`
/// scheme is kept; `localhost`, `127.0.0.1` and `[::1]` get `http://`, any other bare host
/// `https://`. Anything else (another scheme, spaces, nothing) gives `None`.
pub fn normalize_url(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some((scheme, rest)) = text.split_once("://") {
        let web = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
        return (web && !rest.is_empty()).then(|| text.to_string());
    }
    let authority = text.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    // A port is digits; `javascript:alert(1)` and `mailto:a@b.c` have a colon but no port.
    if !authority.starts_with('[') {
        if let Some((_, port)) = authority.rsplit_once(':') {
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
        }
    }
    let host = authority.split(':').next().unwrap_or("");
    let local = host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || authority.starts_with("[::1]");
    Some(format!("{}://{text}", if local { "http" } else { "https" }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewTabId(pub u64);

#[derive(Debug, Clone, PartialEq)]
struct PreviewTab {
    id: PreviewTabId,
    history: Vec<String>,
    // Index into `history` of the current address; meaningless while the history is empty.
    position: usize,
}

/// The tabs of one Preview panel, each with its own history. The page itself opens in the
/// system browser, so a history is only the addresses that were opened from the tab.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewTabs {
    tabs: Vec<PreviewTab>,
    active: Option<PreviewTabId>,
    next: u64,
}

impl PreviewTabs {
    /// Adds an empty tab at the right and makes it the active one.
    pub fn open(&mut self) -> PreviewTabId {
        let id = PreviewTabId(self.next);
        self.next += 1;
        self.tabs.push(PreviewTab { id, history: Vec::new(), position: 0 });
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: PreviewTabId) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else { return };
        self.tabs.remove(index);
        if self.active == Some(id) {
            self.active = self.tabs.get(index).or_else(|| self.tabs.last()).map(|tab| tab.id);
        }
    }

    pub fn select(&mut self, id: PreviewTabId) {
        if self.tabs.iter().any(|tab| tab.id == id) {
            self.active = Some(id);
        }
    }

    pub fn ids(&self) -> Vec<PreviewTabId> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    pub fn active(&self) -> Option<PreviewTabId> {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    fn tab(&self, id: PreviewTabId) -> Option<&PreviewTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn tab_mut(&mut self, id: PreviewTabId) -> Option<&mut PreviewTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// Makes `url` the tab's current address, dropping any forward pages. The address it already
    /// has is not added again.
    pub fn navigate(&mut self, id: PreviewTabId, url: String) {
        let Some(tab) = self.tab_mut(id) else { return };
        if tab.history.get(tab.position) == Some(&url) {
            return;
        }
        if !tab.history.is_empty() {
            tab.history.truncate(tab.position + 1);
        }
        tab.history.push(url);
        tab.position = tab.history.len() - 1;
    }

    pub fn current(&self, id: PreviewTabId) -> Option<String> {
        let tab = self.tab(id)?;
        tab.history.get(tab.position).cloned()
    }

    pub fn can_back(&self, id: PreviewTabId) -> bool {
        self.tab(id).is_some_and(|tab| !tab.history.is_empty() && tab.position > 0)
    }

    pub fn can_forward(&self, id: PreviewTabId) -> bool {
        self.tab(id).is_some_and(|tab| tab.position + 1 < tab.history.len())
    }

    /// Steps back and returns the address now current, or `None` at the start.
    pub fn back(&mut self, id: PreviewTabId) -> Option<String> {
        if !self.can_back(id) {
            return None;
        }
        self.tab_mut(id)?.position -= 1;
        self.current(id)
    }

    pub fn forward(&mut self, id: PreviewTabId) -> Option<String> {
        if !self.can_forward(id) {
            return None;
        }
        self.tab_mut(id)?.position += 1;
        self.current(id)
    }

    /// The host (and port) of the current address, or "New tab".
    pub fn label(&self, id: PreviewTabId) -> String {
        let Some(url) = self.current(id) else { return "New tab".to_string() };
        let rest = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
        rest.split(['/', '?', '#']).next().unwrap_or(rest).to_string()
    }
}
```

In `crates/forge-features/src/lib.rs` add `mod blocking; mod file_tree; mod preview_tabs; mod tool_activity;` and

```rust
pub use blocking::run_blocking;
pub use file_tree::{parse_files_query, FileTree, FilesQuery, TreeRow};
pub use preview_tabs::{normalize_url, PreviewTabId, PreviewTabs};
pub use tool_activity::finished_tool_calls;
```

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-features`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features Cargo.lock
git commit -m "feat: file tree, preview tabs and tool activity state, and a helper to run blocking work

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Wire the services into the workspace and the app

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (`WorkspaceServices`, accessors)
- Modify: `crates/forge-app/src/main.rs`, `crates/forge-app/Cargo.toml`

**Interfaces:**
- Consumes: `GitPort`, `FilesPort`, `BrowserPort`; `GitCli`, `LocalFiles`, `SystemBrowser`; `finished_tool_calls`.
- Produces: `WorkspaceServices { git: Arc<dyn GitPort>, files: Arc<dyn FilesPort>, browser: Arc<dyn BrowserPort>, .. }`; `Workspace::git()`, `Workspace::files()`, `Workspace::browser()`, `Workspace::finished_tool_calls(id: LiveSessionId) -> usize`.

- [ ] **Step 1: Implement** (no new behaviour to test; the build and the existing suite are the check)

In `workspace.rs` change the ports import to `use forge_ports::{BrowserPort, FilePickerPort, FilesPort, FolderPickerPort, GitPort, ProjectRepositoryPort, SessionRepositoryPort, SleepPort, TerminalPort};`, add `use crate::tool_activity::finished_tool_calls;`, and add to `WorkspaceServices` after `pub sleep: Arc<dyn SleepPort>,`:

```rust
    pub git: Arc<dyn GitPort>,
    pub files: Arc<dyn FilesPort>,
    pub browser: Arc<dyn BrowserPort>,
```

After `pub fn terminal(&self)`:

```rust
    pub fn git(&self) -> Arc<dyn GitPort> {
        self.services.peek().git.clone()
    }

    pub fn files(&self) -> Arc<dyn FilesPort> {
        self.services.peek().files.clone()
    }

    pub fn browser(&self) -> Arc<dyn BrowserPort> {
        self.services.peek().browser.clone()
    }

    /// How many tool calls of an open session have finished. A panel that shows files on disk
    /// reads this inside its loader, so it reloads when a tool call finishes. 0 for a session
    /// that is not open.
    pub fn finished_tool_calls(&self, id: LiveSessionId) -> usize {
        self.live_view(id).map(|view| finished_tool_calls(&view.read().session)).unwrap_or(0)
    }
```

`crates/forge-app/Cargo.toml`: add
```toml
forge-adapter-browser = { path = "../forge-adapter-browser" }
forge-adapter-files = { path = "../forge-adapter-files" }
forge-adapter-git = { path = "../forge-adapter-git" }
```
In `main.rs` add the three `use` lines (`forge_adapter_browser::SystemBrowser`, `forge_adapter_files::LocalFiles`, `forge_adapter_git::GitCli`) and in `build_services` after `sleep:`:

```rust
        git: Arc::new(GitCli::new()),
        files: Arc::new(LocalFiles::new()),
        browser: Arc::new(SystemBrowser::new()),
```

- [ ] **Step 2: Build and test**

Run: `cargo build -p forge-app && cargo test -p forge-features -p forge-widgets`
Expected: builds, PASS. (`cargo build` is where a mismatched `view.read().session` would show; `LiveSessionView.session` is used the same way in `chat_window.rs`.)

- [ ] **Step 3: Commit**

```bash
git add crates/forge-features crates/forge-app Cargo.lock
git commit -m "feat: the workspace has the git, files and browser services

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The Changes panel

**Files:**
- Create: `crates/forge-widgets/src/changes_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/dock.rs`, `crates/forge-widgets/src/session_tools.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `GitPort` via `workspace.git()`, `run_blocking`, `finished_tool_calls`, `parse_unified_diff`, `DiffView`, `PanelFrame`.
- Produces: `ChangesPanel { id: LiveSessionId, cwd: PathBuf }`; the toolbar button for Changes.

- [ ] **Step 1: Write the panel**

`crates/forge-widgets/src/changes_panel.rs`:

```rust
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{parse_unified_diff, DiffView};
use forge_features::{run_blocking, use_workspace, Panel};
use forge_ports::{ChangedFile, GitError};

use crate::panel_frame::PanelFrame;

/// The files git says changed in the session's repository, with the diff of the one picked. Read
/// only. It reloads when "Refresh" is pressed and whenever a tool call of the session finishes.
#[component]
pub fn ChangesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut refresh = use_signal(|| 0u32);
    let mut selected = use_signal(|| None::<String>);
    let mut list_open = use_signal(|| true);
    let mut status = use_signal(|| None::<Result<Vec<ChangedFile>, GitError>>);
    let mut diff = use_signal(|| None::<(String, Result<String, GitError>)>);

    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = workspace.finished_tool_calls(id);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(result) = run_blocking(move || git.status(&cwd)).await else { return };
                if let Ok(files) = &result {
                    // Keep the picked file while it is still changed, else pick the first one.
                    let keep = selected.peek().as_ref().is_some_and(|path| files.iter().any(|file| &file.path == path));
                    if !keep {
                        selected.set(files.first().map(|file| file.path.clone()));
                    }
                }
                status.set(Some(result));
            });
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = workspace.finished_tool_calls(id);
            let Some(path) = selected() else {
                diff.set(None);
                return;
            };
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let shown = path.clone();
                let Some(result) = run_blocking(move || git.diff(&cwd, &path)).await else { return };
                diff.set(Some((shown, result)));
            });
        });
    }

    let tools = rsx! {
        button {
            class: if list_open() { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: "File list",
            "aria-label": "File list",
            "aria-pressed": "{list_open()}",
            onclick: move |_| list_open.set(!list_open()),
            "☰"
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: "Refresh",
            "aria-label": "Refresh",
            onclick: move |_| refresh.set(refresh() + 1),
            "⟳"
        }
    };

    let body = match status() {
        None => rsx! { div { class: "dock-note", "Loading…" } },
        Some(Err(GitError::NotARepository)) => rsx! { div { class: "dock-note", "No changes to show" } },
        Some(Err(error)) => rsx! { div { class: "dock-note error", "{error}" } },
        Some(Ok(files)) if files.is_empty() => rsx! { div { class: "dock-note", "No changes to show" } },
        Some(Ok(files)) => {
            let picked = selected();
            let shown = match diff() {
                Some((path, Ok(text))) if Some(&path) == picked.as_ref() => {
                    let parsed = parse_unified_diff(&path, &text);
                    if parsed.hunks.is_empty() {
                        rsx! { div { class: "dock-note", "No textual changes to show" } }
                    } else {
                        rsx! { DiffView { diff: parsed } }
                    }
                }
                Some((path, Err(error))) if Some(&path) == picked.as_ref() => rsx! { div { class: "dock-note error", "{error}" } },
                _ => rsx! { div { class: "dock-note", "Loading…" } },
            };
            rsx! {
                div { class: "changes-body",
                    if list_open() {
                        ul { class: "changes-list",
                            for file in files.iter() {
                                {
                                    let path = file.path.clone();
                                    let letter = file.status.letter();
                                    let counts = match (file.added, file.removed) {
                                        (Some(added), Some(removed)) => format!("+{added} −{removed}"),
                                        _ => String::new(),
                                    };
                                    rsx! {
                                        li { key: "{file.path}",
                                            button {
                                                class: if picked.as_deref() == Some(file.path.as_str()) { "changes-row selected" } else { "changes-row" },
                                                r#type: "button",
                                                title: "{file.path}",
                                                onclick: {
                                                    let path = path.clone();
                                                    move |_| selected.set(Some(path.clone()))
                                                },
                                                span { class: "change-letter change-{letter}", "{letter}" }
                                                span { class: "change-path", "{file.path}" }
                                                span { class: "change-counts", "{counts}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    div { class: "changes-diff", {shown} }
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Changes, tools, {body} }
    }
}
```

- [ ] **Step 2: Wire it**

`lib.rs`: add `mod changes_panel;` (alphabetical) — no `pub use` needed because only `dock.rs` uses it.

`dock.rs`: add `use crate::changes_panel::ChangesPanel;` and replace the shared arm so Changes has its own:

```rust
                                    Panel::Changes => rsx! { ChangesPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                    Panel::Preview | Panel::Files => rsx! {},
```

`session_tools.rs`: after the terminal button (inside `div { class: "pane-tools"`), add a Changes button with the same shape and gate (`if terminal`), and the open state:

```rust
    let changes_open = workspace.dock(id).is_open(Panel::Changes);
```
```rust
            if terminal {
                button {
                    class: if changes_open { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: "Changes",
                    "aria-label": "Changes",
                    "aria-pressed": "{changes_open}",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.toggle(Panel::Changes)),
                    "±"
                }
            }
```
(The `terminal` prop is false for an archived session, which has no dock at all, so it gates every panel button. Rename it to `panels` in all three call sites if you like; do not change behaviour.)

Append to `crates/forge-shared/src/theme.css`:

```css
/* Panels that show files: a note, a list at the left and the content at the right. */
.dock-note { padding: 12px; color: var(--fg-muted); font-size: 13px; }
.dock-note.error { color: var(--danger); }
.changes-body { flex: 1; min-height: 0; display: flex; }
.changes-list { flex: none; width: 40%; margin: 0; padding: 4px; list-style: none; overflow-y: auto; border-right: 1px solid var(--border); box-sizing: border-box; }
.changes-row { display: flex; align-items: baseline; gap: 6px; width: 100%; padding: 4px 6px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; text-align: left; cursor: pointer; }
.changes-row:hover { background: var(--bg-hover); }
.changes-row.selected { background: var(--bg-hover); }
.change-letter { flex: none; width: 1.2em; font-family: var(--mono); font-weight: 600; color: var(--fg-muted); }
.change-letter.change-A, .change-letter.change-U { color: var(--hl-string); }
.change-letter.change-D, .change-letter.change-C { color: var(--danger); }
.change-path { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; direction: rtl; text-align: left; }
.change-counts { flex: none; color: var(--fg-muted); font-size: 11px; }
.changes-diff { flex: 1; min-width: 0; overflow: auto; padding: 8px; }
.changes-diff .diff { max-width: none; margin: 0; }
```

- [ ] **Step 3: Build**

Run: `cargo build -p forge-app && cargo test -p forge-widgets`
Expected: builds, PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Changes panel shows git changes and their diffs

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The Files panel

**Files:**
- Create: `crates/forge-widgets/src/files_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/dock.rs`, `crates/forge-widgets/src/session_tools.rs`, `crates/forge-widgets/Cargo.toml`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `FilesPort` via `workspace.files()`, `FileTree`, `parse_files_query`, `FilesQuery`, `run_blocking`, `code_html`, `language_of`.
- Produces: `FilesPanel { id: LiveSessionId, cwd: PathBuf }`; the Files item in the `⋮` menu.

- [ ] **Step 1: Write the panel**

`crates/forge-widgets/Cargo.toml`: tokio line becomes `tokio = { version = "1", features = ["macros", "sync", "time"] }`.

`crates/forge-widgets/src/files_panel.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{code_html, language_of};
use forge_features::{parse_files_query, run_blocking, use_workspace, FileTree, FilesQuery, Panel};
use forge_ports::{ContentHit, FileContent, FilesError, FilesPort};

use crate::panel_frame::PanelFrame;

// Typing waits this long before the files are searched.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 100;
// Highlighting a file larger than this would keep the UI busy, so it is shown as plain text.
const HIGHLIGHT_LIMIT: usize = 200_000;

async fn load_dir(files: Arc<dyn FilesPort>, root: PathBuf, dir: String, mut tree: Signal<FileTree>, mut problem: Signal<Option<String>>) {
    let key = dir.clone();
    match run_blocking(move || files.list(&root, &dir)).await {
        Some(Ok(entries)) => tree.write().set_children(&key, entries),
        Some(Err(error)) => problem.set(Some(error.to_string())),
        None => {}
    }
}

/// A file tree with a filter field, and a read only viewer for the picked file. A filter that
/// starts with `?` searches inside the files instead.
#[component]
pub fn FilesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut tree = use_signal(FileTree::default);
    let mut problem = use_signal(|| None::<String>);
    let mut filter = use_signal(String::new);
    let mut tree_open = use_signal(|| true);
    let mut opened = use_signal(|| None::<String>);
    let mut content = use_signal(|| None::<Result<FileContent, FilesError>>);
    let mut hits = use_signal(|| None::<Vec<ContentHit>>);

    {
        let cwd = cwd.clone();
        use_future(move || {
            let cwd = cwd.clone();
            async move { load_dir(workspace.files(), cwd, String::new(), tree, problem).await }
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let typed = filter();
            match parse_files_query(&typed) {
                FilesQuery::Content(query) if !query.is_empty() => {
                    let query = query.to_string();
                    let (files, root) = (workspace.files(), cwd.clone());
                    spawn(async move {
                        tokio::time::sleep(SEARCH_DEBOUNCE).await;
                        if filter.peek().as_str() != typed {
                            return;
                        }
                        if let Some(Ok(found)) = run_blocking(move || files.search(&root, &query, SEARCH_LIMIT)).await {
                            if filter.peek().as_str() == typed {
                                hits.set(Some(found));
                            }
                        }
                    });
                }
                _ => hits.set(None),
            }
        });
    }

    let open_file = {
        let cwd = cwd.clone();
        move |path: String| {
            opened.set(Some(path.clone()));
            content.set(None);
            let (files, root) = (workspace.files(), cwd.clone());
            spawn(async move {
                let wanted = path.clone();
                let Some(result) = run_blocking(move || files.read(&root, &path)).await else { return };
                if opened.peek().as_deref() == Some(wanted.as_str()) {
                    content.set(Some(result));
                }
            });
        }
    };

    let tools = rsx! {
        button {
            class: if tree_open() { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: "File tree",
            "aria-label": "File tree",
            "aria-pressed": "{tree_open()}",
            onclick: move |_| tree_open.set(!tree_open()),
            "☰"
        }
    };

    let typed = filter();
    let query = parse_files_query(&typed);
    let list = if let FilesQuery::Content(text) = query {
        match hits() {
            _ if text.is_empty() => rsx! { div { class: "dock-note", "Type the text to search for after ?" } },
            None => rsx! { div { class: "dock-note", "Searching…" } },
            Some(found) if found.is_empty() => rsx! { div { class: "dock-note", "No matches" } },
            Some(found) => rsx! {
                ul { class: "files-list",
                    for (index, hit) in found.iter().enumerate() {
                        li { key: "{index}",
                            button {
                                class: "files-hit",
                                r#type: "button",
                                onclick: {
                                    let path = hit.path.clone();
                                    let mut open_file = open_file.clone();
                                    move |_| open_file(path.clone())
                                },
                                span { class: "files-hit-path", "{hit.path}:{hit.line}" }
                                span { class: "files-hit-text", "{hit.text}" }
                            }
                        }
                    }
                }
            },
        }
    } else if let Some(message) = problem() {
        rsx! { div { class: "dock-note error", "{message}" } }
    } else if !tree.read().is_loaded("") {
        rsx! { div { class: "dock-note", "Loading…" } }
    } else {
        let rows = tree.read().rows(&typed);
        if rows.is_empty() {
            let note = if typed.trim().is_empty() { "Folder is empty" } else { "No files match" };
            rsx! { div { class: "dock-note", "{note}" } }
        } else {
            let current = opened();
            rsx! {
                ul { class: "files-list",
                    for row in rows {
                        {
                            let indent = format!("padding-left: {}px", 6 + row.depth * 14);
                            let mark = if !row.is_dir { " " } else if row.expanded { "▾" } else { "▸" };
                            let selected = current.as_deref() == Some(row.path.as_str());
                            let path = row.path.clone();
                            let (is_dir, cwd) = (row.is_dir, cwd.clone());
                            let mut open_file = open_file.clone();
                            rsx! {
                                li { key: "{row.path}",
                                    button {
                                        class: if selected { "files-row selected" } else { "files-row" },
                                        r#type: "button",
                                        style: "{indent}",
                                        onclick: move |_| {
                                            if is_dir {
                                                if tree.write().toggle(&path) {
                                                    spawn(load_dir(workspace.files(), cwd.clone(), path.clone(), tree, problem));
                                                }
                                            } else {
                                                open_file(path.clone());
                                            }
                                        },
                                        span { class: "files-mark", "{mark}" }
                                        span { "{row.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let viewer = match (opened(), content()) {
        (None, _) => rsx! { div { class: "dock-note", "Pick a file" } },
        (Some(_), None) => rsx! { div { class: "dock-note", "Loading…" } },
        (Some(_), Some(Err(error))) => rsx! { div { class: "dock-note error", "{error}" } },
        (Some(_), Some(Ok(FileContent::Binary | FileContent::TooLarge))) => rsx! { div { class: "dock-note", "Cannot show this file" } },
        (Some(path), Some(Ok(FileContent::Text(text)))) => {
            let language = if text.len() > HIGHLIGHT_LIMIT { "" } else { language_of(&path) };
            let html = code_html(language, &text);
            let numbers = (1..=text.lines().count().max(1)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
            rsx! {
                div { class: "file-viewer", "aria-label": "{path}",
                    pre { class: "file-gutter", "{numbers}" }
                    pre { class: "file-code", dangerous_inner_html: "{html}" }
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Files, tools,
            div { class: "files-filter",
                input {
                    class: "files-filter-input",
                    r#type: "text",
                    placeholder: "Filter, or start with ? to search inside files",
                    value: "{filter}",
                    oninput: move |event| filter.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                }
            }
            div { class: "changes-body",
                if tree_open() {
                    div { class: "files-side", {list} }
                }
                div { class: "changes-diff", {viewer} }
            }
        }
    }
}
```

(`open_file` is a closure over `Copy` signals and a `PathBuf`, so it is `Clone`, not `Copy`: every handler takes its own clone, as above. If the compiler asks for `Fn`/`FnMut` changes, keep the clones and mark the local copies `mut`.)

- [ ] **Step 2: Wire it**

`lib.rs`: add `mod files_panel;`. `dock.rs`: `use crate::files_panel::FilesPanel;`, arms:

```rust
                                    Panel::Files => rsx! { FilesPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                    Panel::Preview => rsx! {},
```

`session_tools.rs`: add `let files_open = workspace.dock(id).is_open(Panel::Files);` and, in the `⋮` menu `div` before the Prevent sleep button (inside `if terminal` so an archived session, which has no dock, does not offer it):

```rust
                        if terminal {
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{files_open}",
                                onclick: move |_| {
                                    workspace.change_dock(id, |dock| dock.toggle(Panel::Files));
                                    open.set(false);
                                },
                                span { "Files" }
                            }
                        }
```

Append to `theme.css`:

```css
.files-filter { flex: none; padding: 6px; border-bottom: 1px solid var(--border); }
.files-filter-input { width: 100%; box-sizing: border-box; padding: 5px 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font: inherit; font-size: 12px; }
.files-side { flex: none; width: 40%; overflow-y: auto; border-right: 1px solid var(--border); box-sizing: border-box; }
.files-list { margin: 0; padding: 4px; list-style: none; }
.files-row, .files-hit { display: flex; align-items: baseline; gap: 4px; width: 100%; padding: 3px 6px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; text-align: left; cursor: pointer; }
.files-row:hover, .files-hit:hover, .files-row.selected { background: var(--bg-hover); }
.files-mark { flex: none; width: 1em; color: var(--fg-muted); }
.files-hit { flex-direction: column; align-items: flex-start; gap: 0; }
.files-hit-path { color: var(--fg-muted); font-size: 11px; }
.files-hit-text { font-family: var(--mono); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 100%; }
.file-viewer { display: flex; font-family: var(--mono); font-size: 12px; line-height: 1.5; min-width: max-content; }
.file-gutter { flex: none; margin: 0; padding: 0 8px 0 0; text-align: right; color: var(--fg-muted); user-select: none; }
.file-code { margin: 0; white-space: pre; }
```

- [ ] **Step 3: Build**

Run: `cargo build -p forge-app && cargo test -p forge-widgets`
Expected: builds, PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-widgets crates/forge-shared Cargo.lock
git commit -m "feat: Files panel with a tree, a filter, content search and a read only viewer

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The Preview panel

**Files:**
- Create: `crates/forge-widgets/src/preview_panel.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/dock.rs`, `crates/forge-widgets/src/session_tools.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `PreviewTabs`, `normalize_url`, `BrowserPort` via `workspace.browser()`.
- Produces: `PreviewPanel { id: LiveSessionId }`; the Preview toolbar button.

- [ ] **Step 1: Write the panel**

`crates/forge-widgets/src/preview_panel.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{normalize_url, use_workspace, Panel, PreviewTabs};

use crate::panel_frame::PanelFrame;

/// Tabs, back, forward, reload and an address bar. The page itself opens in the system browser:
/// the window's web view opens every web address outside, so it cannot show a page inside the
/// panel. Each tab keeps the addresses that were opened from it.
#[component]
pub fn PreviewPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut tabs = use_signal(|| {
        let mut tabs = PreviewTabs::default();
        tabs.open();
        tabs
    });
    let mut input = use_signal(String::new);
    let mut notice = use_signal(|| None::<String>);

    let go = move |url: String| match workspace.browser().open(&url) {
        Ok(()) => notice.set(None),
        Err(message) => notice.set(Some(message)),
    };
    let mut submit = move || {
        let Some(url) = normalize_url(&input()) else {
            notice.set(Some("Enter a web address, such as localhost:3000".to_string()));
            return;
        };
        let active = tabs.read().active();
        let Some(tab) = active else { return };
        tabs.write().navigate(tab, url.clone());
        input.set(url.clone());
        go(url);
    };
    let step = move |forward: bool| {
        let active = tabs.read().active();
        let Some(tab) = active else { return };
        let url = if forward { tabs.write().forward(tab) } else { tabs.write().back(tab) };
        if let Some(url) = url {
            input.set(url.clone());
            go(url);
        }
    };

    let active = tabs.read().active();
    let ids = tabs.read().ids();
    let current = active.and_then(|tab| tabs.read().current(tab));
    let (can_back, can_forward) = match active {
        Some(tab) => (tabs.read().can_back(tab), tabs.read().can_forward(tab)),
        None => (false, false),
    };

    let tools = rsx! {
        for tab in ids.iter().copied() {
            span { key: "{tab.0}", class: if Some(tab) == active { "terminal-tab active" } else { "terminal-tab" },
                button {
                    class: "terminal-tab-label",
                    r#type: "button",
                    onclick: move |_| {
                        tabs.write().select(tab);
                        let url = tabs.read().current(tab).unwrap_or_default();
                        input.set(url);
                        notice.set(None);
                    },
                    "{tabs.read().label(tab)}"
                }
                button {
                    class: "terminal-tab-close",
                    r#type: "button",
                    title: "Close tab",
                    "aria-label": "Close tab",
                    onclick: move |_| {
                        tabs.write().close(tab);
                        if tabs.read().is_empty() {
                            workspace.change_dock(id, |dock| dock.close(Panel::Preview));
                        } else {
                            let url = tabs.read().active().and_then(|active| tabs.read().current(active)).unwrap_or_default();
                            input.set(url);
                        }
                    },
                    "×"
                }
            }
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: "New tab",
            "aria-label": "New tab",
            onclick: move |_| {
                tabs.write().open();
                input.set(String::new());
                notice.set(None);
            },
            "+"
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Preview, tools,
            div { class: "preview-bar",
                button { class: "icon-button", r#type: "button", title: "Back", "aria-label": "Back", disabled: !can_back, onclick: move |_| step(false), "‹" }
                button { class: "icon-button", r#type: "button", title: "Forward", "aria-label": "Forward", disabled: !can_forward, onclick: move |_| step(true), "›" }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: "Reload",
                    "aria-label": "Reload",
                    disabled: current.is_none(),
                    onclick: {
                        let current = current.clone();
                        move |_| {
                            if let Some(url) = current.clone() {
                                go(url);
                            }
                        }
                    },
                    "⟳"
                }
                input {
                    class: "preview-url",
                    r#type: "text",
                    placeholder: "Enter a URL",
                    value: "{input}",
                    oninput: move |event| input.set(event.value()),
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Enter {
                            submit();
                        }
                    },
                }
                button { class: "preview-open", r#type: "button", onclick: move |_| submit(), "Open in browser" }
            }
            div { class: "dock-note",
                if let Some(message) = notice() {
                    span { class: "error", "{message}" }
                } else if let Some(url) = current {
                    span { "{url}" }
                    br {}
                    span { "This page is open in your browser." }
                } else {
                    span { "Enter an address above. The page opens in your browser." }
                }
            }
        }
    }
}
```

Note `go` is `Copy` (it captures `workspace` and `notice`, both `Copy`) and is used by `submit`, `step` and the reload handler. If the compiler reports that a closure is not `Copy`, make `go` a small `fn open_url(workspace: Workspace, mut notice: Signal<Option<String>>, url: String)` instead and call that.

- [ ] **Step 2: Wire it**

`lib.rs`: `mod preview_panel;`. `dock.rs`: `use crate::preview_panel::PreviewPanel;` and the last arm:

```rust
                                    Panel::Preview => rsx! { PreviewPanel { key: "{panel:?}", id } },
```
(The match is now exhaustive; delete the leftover `rsx! {}` arm.)

`session_tools.rs`: a third button after Changes, same shape, `Panel::Preview`, title `"Preview"`, glyph `"◎"`, state `preview_open`.

Append to `theme.css`:

```css
.preview-bar { flex: none; display: flex; align-items: center; gap: 4px; padding: 4px 6px; border-bottom: 1px solid var(--border); }
.preview-bar .icon-button:disabled { opacity: 0.4; cursor: default; }
.preview-url { flex: 1; min-width: 0; padding: 4px 8px; border: 1px solid var(--border); border-radius: 6px; background: var(--bg); color: var(--fg); font: inherit; font-size: 12px; }
.preview-open { flex: none; padding: 4px 8px; border: 1px solid var(--border); border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 12px; cursor: pointer; }
.preview-open:hover { background: var(--bg-hover); }
.dock-note .error { color: var(--danger); }
```

- [ ] **Step 3: Build**

Run: `cargo build -p forge-app && cargo test`
Expected: builds; the whole workspace test suite PASSES.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Preview panel with tabs, history and an address bar that opens the browser

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Check the panels in the real app

**Files:** none (a check; fix what it finds in the file it belongs to and re-commit).

Use the harness (`scripts/harness`, see its README and the memory note "driving the Forge app visually"): `launch.sh`, `capture.py` then Read the PNG (it may show scaled; multiply by the factor before `click.py X Y`), `stop.sh` at the end. Do not launch twice.

- [ ] **Step 1: Prepare a repository in a fixture folder**

`launch.sh` builds the fixture; its project folders are under `/private/tmp` (see `scripts/harness/build_fixture.py`, `my-app`). Find the folder with `ls -d /private/tmp/*/my-app /private/tmp/my-app 2>/dev/null` or read the "Folder" field on the new-session screen. In it:

```bash
cd <that my-app folder>
git init -q && printf 'a\nb\nc\n' > a.txt && mkdir -p src && printf 'fn main() {\n    println!("hello");\n}\n' > src/main.rs
git add . && git -c user.name=t -c user.email=t@t commit -qm first
printf 'a\nB\nc\nd\n' > a.txt && printf 'new\n' > new.txt && printf 'x\0y' > data.bin
```

- [ ] **Step 2: Walk through**

Open the session "Read the config" (project `my-app`), then check and capture each:
1. Click `±`: Changes opens at the right; the list shows `M a.txt +2 −1`, `U new.txt`, `U data.bin`; `a.txt` is picked and its diff shows `-b` / `+B` / `+d` with context. Click `new.txt`: all added. Click `data.bin`: "No textual changes to show".
2. Edit `a.txt` again from a terminal; press ⟳: the counts change. (A tool call finishing refreshes it too; not driven here, it needs a real model turn.)
3. Click `◎` while Changes is open: Preview joins the same column under Changes. Type `localhost:3000`, Enter: the default browser opens `http://localhost:3000`; the panel shows the address and "This page is open in your browser."; `+` makes a second tab; back and forward walk the history. Type `javascript:alert(1)`: the message "Enter a web address, such as localhost:3000" and nothing opens.
4. `⋮` → Files: a new column. The tree shows `src/`, `a.txt`, … (no `.git`). Open `src`, click `main.rs`: highlighted, with line numbers. Click `data.bin`: "Cannot show this file". Type `main` in the filter: only matching rows and their folders. Type `?hello`: a row `src/main.rs:2`; click it to open the file.
5. Expand and collapse each panel; close each; drag a column edge; the terminal still opens next to them.
6. Switch to another session and back; open the `my_app` session whose folder is missing: Changes says "No changes to show" or the git message, not a crash.
7. A folder that is not a repository (`Session in a/app` if its folder exists, else create one): Changes says "No changes to show".

- [ ] **Step 3: Fix, stop, and commit any fixes**

Run `scripts/harness/stop.sh`. For each defect found, write a failing test first when the defect is in pure code or an adapter, fix it, and commit with a message that says what was wrong.

---

### Task 12: Docs: spec, backlog, roadmap

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-right-dock-panels-design.md`, `docs/superpowers/roadmap/backlog.md`

- [ ] **Step 1: Write what was built**

Append a section `## As built (Plan 21B)` to the spec: built as designed (Changes, Files, Preview, the new ports and adapters, `run_blocking`), and the differences — the Preview page opens in the system browser (spike); the shortcuts ⇧⌘F and ⇧⌘Y are not built; "Refresh" is a button in the header, not in a `⋮`; the diff limit is the existing 300 line cut; `GitPort` roots at the repository's top folder; `parse_unified_diff` and `code_html` live in `forge-entities`; Files is opened from the `⋮` menu. Plus whatever Task 11 found. In the same file fix the table row "forge-features | `git_status.rs` (parse)" to say the parser lives in `forge-adapter-git`.

In `backlog.md` set PNL-02, PNL-03 and PNL-04 to built (with what is left): Changes stage/revert/commit; Files editor, auto save and options, ⇧⌘F, ⇧⌘Y; Preview in-panel page (a `forge://` proxy into an `<iframe>`, or a child web view), cookie import, element picker, layout icon. Mark OBS-04 as to repeat with the real app when a session has real changes.

- [ ] **Step 2: Commit**

```bash
git add docs
git commit -m "docs: Plan 21B as built, backlog updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
