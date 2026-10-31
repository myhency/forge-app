# Worktree sessions, worktree location and auto archive Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A git folder on the new-session screen can start its session in its own git worktree; archiving removes a worktree that holds no work; Settings gets `Worktree location` (SET-08) and `Archive inactive sessions` (SET-07).

**Architecture:** Pure rules (names, paths, origin of a worktree, which sessions are due) live in `forge-domain`. A new write port `WorktreePort` (implemented on the existing `GitCli`) runs `git worktree` commands. A `WorktreesUseCase` in `forge-application` creates and releases worktrees and keeps a record of each in the Forge config (`AppConfig.worktrees`). `forge-features`'s `Workspace` calls it when a first message is sent and when a session is archived, and merges a worktree's sessions into the main repository's sidebar group; `forge-widgets` adds the branch chip, the worktree row and the two settings rows.

**Tech Stack:** Rust 2021, Dioxus 0.6-style signals (`Signal`, `spawn_forever`), `mockall` ports, `tempfile` for real-repo tests, `git` on PATH.

**Spec:** `docs/superpowers/specs/2026-10-17-worktree-sessions-design.md` (deviations from it are listed under "Spec changes made while planning" and are written into the spec in Task 1).

## Global Constraints

- Worktree folder: `<repo>/.claude/worktrees/<name>`; branch `claude/<name>`; `<name>` = `<slug>-<6 lowercase hex>`; slug = first non-empty line of the first message, lower-cased, runs of non-ASCII-alphanumerics become one `-`, cut to 40 characters, `session` when nothing is left.
- `Worktree location`: `Project` (default) or `Folder(path)`; a folder equal to or inside the repository is ignored (the default is used); a custom folder holds `<folder>/<repo folder name>/<name>`.
- Archive removes a worktree only when it has no changes (tracked or untracked) and no commits of its own, only for worktrees Forge recorded, never with `--force` or `-D`, and never while another session still uses the same folder.
- Auto archive menu: None, 1 day, 2 days, 7 days, 14 days, 30 days; default None; a busy session (open and running, waiting for approval, or with background work) is never archived.
- All user-visible text goes through `tr!`, and every new sentence needs its Korean text in `crates/forge-shared/src/ko.rs` (the tests `every_sentence_in_the_sources_has_a_korean_text`, `every_korean_entry_is_used_in_the_sources` and `the_table_is_sorted_and_has_no_duplicates` enforce it). Use `python3 scripts/i18n_add.py` (Task 1) to insert entries at their sorted place.
- Blocking work (git, folder reads) runs off the UI thread: `spawn_blocking` in the use case, `crate::blocking::run_blocking` in features.
- Saves and disk mutations that must finish use `dioxus::core::spawn_forever`; config writes go through `SettingsUseCase::save_code_settings` (it holds the config lock and never replaces an unreadable config).
- Commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (use two `-m` arguments).
- Run commands from `/Users/james/orca/workspaces/forge/features` on branch `plan-47`.

## Review Focus

- A first message with no ASCII letters or digits (Korean only, emoji, punctuation) still gives a valid name (`session-xxxxxx`): tested in Task 1.
- A repository with a detached HEAD, or with no commit yet: the branch list shows no current branch, the worktree cannot be created, the notice shows git's text and the typed message comes back into the box: tested in Tasks 3 and 4, checked in Task 7.
- Archiving one of two sessions that share a worktree (a fork keeps the folder): the worktree stays: tested in Task 8 (`worktree_in_use`).
- A worktree with only an untracked file, or with a commit, is kept on archive: tested in Tasks 3 and 4.
- A config written by an older Forge (no new fields), or with a broken `worktree_location`, still loads and keeps everything else: tested in Task 1.
- A custom worktree folder inside the repository, a name that already exists (folder or branch), and a worktree folder deleted by hand before archive: tested in Tasks 1, 3 and 4.

---

## Spec changes made while planning

Written into the spec by Task 1 so the two documents agree:

1. The branch row and the worktree switch of the desktop folder menu become: a **Branch chip** beside the folder chip (a normal picker list) and a **checkable `Worktree` row** in the folder menu. The existing `Picker` has no submenu or switch, and a new picker kind is not worth it for one screen.
2. The record `created_at` is epoch milliseconds (`i64`), like routines.
3. Removal on archive only applies to worktrees Forge recorded (a worktree the desktop app made has no recorded base branch to check commits against). Such a worktree still groups under its repository (by the `.claude/worktrees/<name>` path rule) and still gets the `Worktree` tag, without a branch.
4. `main_repo_of` is a path rule only (no check that the folder exists).
5. When creation fails the text comes back into the box; attachments and extra folders do not (they were taken out of the box when the message was sent).
6. The search palette and the Date grouping label a worktree session's row with the worktree's folder name (the name Forge made), not the repository's.

## File Structure

Create:
- `crates/forge-domain/src/worktree.rs`: `WorktreeRecord`, `WorktreeLocation`, naming and path rules, `origin_of`, `main_repo_of`.
- `crates/forge-domain/src/auto_archive.rs`: `ArchiveCandidate`, `sessions_to_archive` (Task 10).
- `crates/forge-ports/src/worktree_port.rs`: `WorktreePort`, `BranchList`, `WorktreeState`, `WorktreeError`.
- `crates/forge-application/src/use_cases/worktrees.rs`: `WorktreesUseCase`, `Release`, `Kept`, `WorktreeUseCaseError`.
- `crates/forge-features/src/worktree_view.rs`: `RepoInfo`, `NewWorktree`, `folder_choices`, `worktree_in_use`, `WorktreeTag`.
- `scripts/i18n_add.py`: inserts sentences into `ko.rs` in sorted order.

Modify:
- `crates/forge-domain/src/config.rs`, `lib.rs`: three `AppConfig` fields and exports.
- `crates/forge-ports/src/lib.rs`: export the port and its mock.
- `crates/forge-adapter-git/src/lib.rs`, `Cargo.toml`: `impl WorktreePort for GitCli`.
- `crates/forge-application/src/use_cases/mod.rs`, `src/lib.rs`: export the use case.
- `crates/forge-features/src/arrange.rs`: `arrange_with_origins`.
- `crates/forge-features/src/workspace.rs`, `new_session.rs`, `lib.rs`, `settings.rs`, `settings_pages.rs`.
- `crates/forge-widgets/src/new_session_controls.rs`, `new_session_panel.rs`, `chat_window.rs`, `settings_overlay.rs`; `crates/forge-shared/src/theme.css`, `ko.rs`.
- `crates/forge-app/src/main.rs`: build and inject the use case.
- Docs: `docs/superpowers/roadmap/backlog.md`, the spec's "As built", the memory file.

---

## Stage A: worktree sessions

### Task 1: Domain types, naming and paths, config fields, i18n helper

**Files:**
- Create: `crates/forge-domain/src/worktree.rs`
- Create: `scripts/i18n_add.py`
- Modify: `crates/forge-domain/src/config.rs` (the `AppConfig` struct, `Default`, tests), `crates/forge-domain/src/lib.rs`
- Modify: `docs/superpowers/specs/2026-10-17-worktree-sessions-design.md` (spec changes)

**Interfaces:**
- Produces (all `pub`, re-exported from `forge_domain`):
  - `struct WorktreeRecord { name: String, path: PathBuf, repo: PathBuf, branch: String, base_branch: String, created_at: i64 }`
  - `enum WorktreeLocation { Project, Folder(PathBuf) }` (`Default` = `Project`)
  - `fn first_line(text: &str) -> &str`, `fn slug(text: &str) -> String`, `fn suffix(seed: u64, attempt: u32) -> String`
  - `fn worktree_name(message: &str, seed: u64, attempt: u32) -> String`, `fn branch_name(name: &str) -> String`
  - `fn worktree_dir(repo: &Path, name: &str, location: &WorktreeLocation) -> PathBuf`
  - `fn main_repo_of(path: &Path) -> Option<PathBuf>`, `fn origin_of(records: &[WorktreeRecord], path: &Path) -> PathBuf`
  - `AppConfig.worktrees: Vec<WorktreeRecord>`, `AppConfig.worktree_location: WorktreeLocation`, `AppConfig.auto_archive_days: Option<u32>`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/worktree.rs` with only the tests first (the types and functions come in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn record(path: &str, repo: &str) -> WorktreeRecord {
        WorktreeRecord {
            name: "n".to_string(),
            path: PathBuf::from(path),
            repo: PathBuf::from(repo),
            branch: "claude/n".to_string(),
            base_branch: "master".to_string(),
            created_at: 0,
        }
    }

    #[test]
    fn a_slug_keeps_ascii_letters_and_digits_in_lower_case_joined_by_one_dash() {
        assert_eq!(slug("Fix the login bug!"), "fix-the-login-bug");
        assert_eq!(slug("  --Hello,   World--  "), "hello-world");
        assert_eq!(slug("v2.0 release"), "v2-0-release");
    }

    #[test]
    fn a_slug_is_cut_at_forty_characters_without_a_trailing_dash() {
        assert_eq!(slug(&"a".repeat(50)), "a".repeat(40));
        let text = format!("{} b", "a".repeat(39));
        assert_eq!(slug(&text), "a".repeat(39));
    }

    #[test]
    fn a_message_with_no_ascii_letter_or_digit_is_called_session() {
        assert_eq!(slug("안녕하세요"), "session");
        assert_eq!(slug("😀 !!!"), "session");
        assert_eq!(slug(""), "session");
    }

    #[test]
    fn the_name_comes_from_the_first_non_empty_line() {
        assert_eq!(first_line("\n  \n Add tests \nsecond"), "Add tests");
        assert_eq!(first_line(""), "");
        assert!(worktree_name("\nAdd tests\nmore", 1, 0).starts_with("add-tests-"));
    }

    #[test]
    fn the_suffix_is_six_lower_case_hex_digits_and_changes_with_the_attempt() {
        let first = suffix(12345, 0);
        assert_eq!(first.len(), 6);
        assert!(first.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{first}");
        assert_ne!(first, suffix(12345, 1));
        assert_eq!(first, suffix(12345, 0));
    }

    #[test]
    fn the_branch_is_the_name_under_claude() {
        assert_eq!(branch_name("fix-1a2b3c"), "claude/fix-1a2b3c");
    }

    #[test]
    fn a_project_worktree_lives_under_the_repository() {
        let dir = worktree_dir(Path::new("/work/app"), "fix-1a2b3c", &WorktreeLocation::Project);
        assert_eq!(dir, PathBuf::from("/work/app/.claude/worktrees/fix-1a2b3c"));
    }

    #[test]
    fn a_custom_folder_holds_one_folder_per_repository() {
        let location = WorktreeLocation::Folder(PathBuf::from("/trees"));
        assert_eq!(worktree_dir(Path::new("/work/app"), "fix-1a2b3c", &location), PathBuf::from("/trees/app/fix-1a2b3c"));
    }

    #[test]
    fn a_custom_folder_inside_the_repository_is_ignored() {
        let inside = WorktreeLocation::Folder(PathBuf::from("/work/app/trees"));
        let same = WorktreeLocation::Folder(PathBuf::from("/work/app"));
        let expected = PathBuf::from("/work/app/.claude/worktrees/x-000000");
        assert_eq!(worktree_dir(Path::new("/work/app"), "x-000000", &inside), expected);
        assert_eq!(worktree_dir(Path::new("/work/app"), "x-000000", &same), expected);
    }

    #[test]
    fn the_repository_of_a_default_worktree_path_is_read_from_the_path() {
        assert_eq!(main_repo_of(Path::new("/work/app/.claude/worktrees/fix-1a2b3c")), Some(PathBuf::from("/work/app")));
        assert_eq!(main_repo_of(Path::new("/work/app")), None);
        assert_eq!(main_repo_of(Path::new("/work/app/.claude/worktrees")), None);
        assert_eq!(main_repo_of(Path::new("/.claude/worktrees/x")), Some(PathBuf::from("/")));
        assert_eq!(main_repo_of(Path::new(".claude/worktrees/x")), None);
    }

    #[test]
    fn the_origin_is_the_recorded_repository_then_the_path_rule_then_the_path_itself() {
        let records = vec![record("/trees/app/x-1", "/work/app")];
        assert_eq!(origin_of(&records, Path::new("/trees/app/x-1")), PathBuf::from("/work/app"));
        assert_eq!(origin_of(&records, Path::new("/work/other/.claude/worktrees/y")), PathBuf::from("/work/other"));
        assert_eq!(origin_of(&records, Path::new("/work/app")), PathBuf::from("/work/app"));
    }

    #[test]
    fn the_location_round_trips_through_json() {
        let folder = WorktreeLocation::Folder(PathBuf::from("/trees"));
        let text = serde_json::to_string(&folder).unwrap();
        assert_eq!(serde_json::from_str::<WorktreeLocation>(&text).unwrap(), folder);
        let project = serde_json::to_string(&WorktreeLocation::Project).unwrap();
        assert_eq!(serde_json::from_str::<WorktreeLocation>(&project).unwrap(), WorktreeLocation::Project);
    }
}
```

Add these tests at the end of the `tests` module in `crates/forge-domain/src/config.rs`:

```rust
    #[test]
    fn a_config_from_an_older_forge_loads_with_no_worktrees_and_no_auto_archive() {
        let config: AppConfig = serde_json::from_str(r#"{"base_url":null,"api_key":null}"#).unwrap();
        assert!(config.worktrees.is_empty());
        assert_eq!(config.worktree_location, crate::WorktreeLocation::Project);
        assert_eq!(config.auto_archive_days, None);
    }

    #[test]
    fn a_broken_worktree_setting_reads_as_the_default_and_keeps_the_rest() {
        let text = r#"{"base_url":"https://x.test","api_key":null,"worktree_location":{"kind":"nonsense"},"worktrees":"oops","auto_archive_days":"soon"}"#;
        let config: AppConfig = serde_json::from_str(text).unwrap();
        assert_eq!(config.base_url.as_deref(), Some("https://x.test"));
        assert_eq!(config.worktree_location, crate::WorktreeLocation::Project);
        assert!(config.worktrees.is_empty());
        assert_eq!(config.auto_archive_days, None);
    }

    #[test]
    fn worktree_records_and_the_settings_are_saved_and_read_back() {
        let config = AppConfig {
            worktrees: vec![crate::WorktreeRecord {
                name: "x-1".to_string(),
                path: "/w/.claude/worktrees/x-1".into(),
                repo: "/w".into(),
                branch: "claude/x-1".to_string(),
                base_branch: "master".to_string(),
                created_at: 5,
            }],
            worktree_location: crate::WorktreeLocation::Folder("/trees".into()),
            auto_archive_days: Some(7),
            ..AppConfig::default()
        };
        let text = serde_json::to_string(&config).unwrap();
        assert_eq!(serde_json::from_str::<AppConfig>(&text).unwrap(), config);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-domain worktree 2>&1 | tail -15`
Expected: FAIL to compile (`cannot find ... WorktreeRecord`, `slug`, ...).

- [ ] **Step 3: Write the implementation**

Put this at the top of `crates/forge-domain/src/worktree.rs`, above the tests:

```rust
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

const SLUG_MAX: usize = 40;

/// A worktree Forge made for a session, kept in the config so the sidebar can group the session
/// under its repository and archive can tell what it may remove.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeRecord {
    pub name: String,
    pub path: PathBuf,
    /// The repository the worktree was made from (the main checkout).
    pub repo: PathBuf,
    pub branch: String,
    /// The branch the worktree started from.
    pub base_branch: String,
    /// Epoch milliseconds.
    pub created_at: i64,
}

/// Where new worktrees are kept. An unknown stored value reads as `Project` (see `AppConfig`).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", content = "path", rename_all = "snake_case")]
pub enum WorktreeLocation {
    /// `<repository>/.claude/worktrees`.
    #[default]
    Project,
    /// `<folder>/<repository folder name>`.
    Folder(PathBuf),
}

/// The first line of `text` that is not blank, trimmed; empty when there is none.
pub fn first_line(text: &str) -> &str {
    text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or("")
}

/// A folder-safe name from free text: ASCII letters and digits in lower case, every run of other
/// characters one dash, at most 40 characters, `session` when nothing is left.
pub fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            if dash && !out.is_empty() {
                out.push('-');
            }
            dash = false;
            out.push(c.to_ascii_lowercase());
        } else {
            dash = true;
        }
    }
    out.truncate(SLUG_MAX);
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "session".to_string()
    } else {
        out
    }
}

/// Six lower-case hex digits taken from `seed` and the attempt number (a name that was taken is
/// made again with the next attempt).
pub fn suffix(seed: u64, attempt: u32) -> String {
    let mixed = seed ^ u64::from(attempt).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let mixed = (mixed ^ (mixed >> 29)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    format!("{:06x}", mixed >> 40)
}

/// The worktree's name: the slug of the message's first line and the suffix.
pub fn worktree_name(message: &str, seed: u64, attempt: u32) -> String {
    format!("{}-{}", slug(first_line(message)), suffix(seed, attempt))
}

pub fn branch_name(name: &str) -> String {
    format!("claude/{name}")
}

/// Where the worktree called `name` of `repo` goes. A custom folder that is the repository or
/// inside it is ignored, as the desktop app does.
pub fn worktree_dir(repo: &Path, name: &str, location: &WorktreeLocation) -> PathBuf {
    let in_project = || repo.join(".claude").join("worktrees").join(name);
    match location {
        WorktreeLocation::Project => in_project(),
        WorktreeLocation::Folder(folder) if folder.starts_with(repo) => in_project(),
        WorktreeLocation::Folder(folder) => folder.join(repo.file_name().unwrap_or_default()).join(name),
    }
}

/// The repository a path of the form `<repo>/.claude/worktrees/<name>` belongs to. A rule on the
/// path only; nothing is read from disk.
pub fn main_repo_of(path: &Path) -> Option<PathBuf> {
    let parts: Vec<_> = path.components().collect();
    let count = parts.len();
    if count < 4 || parts[count - 2].as_os_str() != "worktrees" || parts[count - 3].as_os_str() != ".claude" {
        return None;
    }
    Some(parts[..count - 3].iter().collect())
}

/// The folder a session's project belongs to in the sidebar: the repository of a recorded worktree,
/// else the repository the path names, else the path itself.
pub fn origin_of(records: &[WorktreeRecord], path: &Path) -> PathBuf {
    if let Some(record) = records.iter().find(|record| record.path == path) {
        return record.repo.clone();
    }
    main_repo_of(path).unwrap_or_else(|| path.to_path_buf())
}
```

In `crates/forge-domain/src/lib.rs` add `mod worktree;` next to the other `mod` lines and this export:

```rust
pub use worktree::{branch_name, first_line, main_repo_of, origin_of, slug, suffix, worktree_dir, worktree_name, WorktreeLocation, WorktreeRecord};
```

In `crates/forge-domain/src/config.rs` add to the `AppConfig` struct, after `trusted_folders`:

```rust
    /// The worktrees Forge made for sessions.
    #[serde(default, deserialize_with = "lenient")]
    pub worktrees: Vec<crate::WorktreeRecord>,
    /// Where new worktrees are kept.
    #[serde(default, deserialize_with = "lenient")]
    pub worktree_location: crate::WorktreeLocation,
    /// Sessions with no activity for this many days are archived; `None` is off.
    #[serde(default, deserialize_with = "lenient")]
    pub auto_archive_days: Option<u32>,
```

and to `impl Default for AppConfig`, after `trusted_folders: Vec::new(),`:

```rust
            worktrees: Vec::new(),
            worktree_location: crate::WorktreeLocation::Project,
            auto_archive_days: None,
```

`AppConfig` derives `PartialEq` already; `WorktreeRecord` and `WorktreeLocation` derive it, so the round-trip test compiles.

Create `scripts/i18n_add.py`:

```python
#!/usr/bin/env python3
"""Adds sentences to crates/forge-shared/src/ko.rs at their sorted place.

usage: python3 scripts/i18n_add.py 'English' 'Korean' ['English2' 'Korean2' ...]
Run it from the repository root. A sentence that is already there stops the script.
"""
import json
import re
import sys

PATH = 'crates/forge-shared/src/ko.rs'
LINE = re.compile(r'^    \("((?:[^"\\]|\\.)*)", "((?:[^"\\]|\\.)*)"\),$')


def main(arguments):
    if len(arguments) % 2 != 0 or not arguments:
        sys.exit(__doc__)
    lines = open(PATH, encoding='utf-8').read().split('\n')
    for english, korean in zip(arguments[0::2], arguments[1::2]):
        key = english.encode('utf-8')
        at = None
        for index, line in enumerate(lines):
            match = LINE.match(line)
            if not match:
                continue
            existing = json.loads('"' + match.group(1) + '"').encode('utf-8')
            if existing == key:
                sys.exit('already there: ' + english)
            if existing > key:
                at = index
                break
        if at is None:
            at = lines.index('];')
        lines.insert(at, '    ("%s", "%s"),' % (english, korean))
    open(PATH, 'w', encoding='utf-8').write('\n'.join(lines))


main(sys.argv[1:])
```

Update the spec: in `docs/superpowers/specs/2026-10-17-worktree-sessions-design.md` append, before `## Out of scope`, a section:

```markdown
### Changes made while planning (2026-10-17)

- The branch row and the worktree switch of the desktop folder menu are a **Branch chip** beside the folder chip (a normal picker list, shown only for a repository with the switch on) and a checkable **`Worktree` row** in the folder menu; the existing picker has no submenu or switch.
- `WorktreeRecord.created_at` is epoch milliseconds.
- Removal on archive only applies to worktrees Forge recorded; one the desktop app made still groups under its repository (path rule) and gets the `Worktree` tag, without a branch. `main_repo_of` reads the path only.
- A worktree is kept while another session (open, or not archived) uses the same folder (a fork keeps its folder).
- When creation fails the typed text comes back into the box; attachments and extra folders do not.
- The search palette and the Date grouping label a worktree session's row with the worktree's folder name.
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-domain 2>&1 | tail -15`
Expected: PASS (the new worktree tests and the three config tests; everything else unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain scripts/i18n_add.py docs/superpowers/specs/2026-10-17-worktree-sessions-design.md
git commit -m "feat: worktree records, names, paths and the config fields for worktrees (SET-07, SET-08)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `WorktreePort`

**Files:**
- Create: `crates/forge-ports/src/worktree_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::WorktreeRecord`.
- Produces:
  - `struct BranchList { current: Option<String>, local: Vec<String> }` (`Default`)
  - `struct WorktreeState { dirty: bool, own_commits: bool }`
  - `enum WorktreeError { NotARepository, NameTaken, Failed(String) }` (`thiserror`)
  - `trait WorktreePort: Send + Sync` with `fn branches(&self, repo: &Path) -> Result<BranchList, WorktreeError>`, `fn create(&self, repo: &Path, dir: &Path, branch: &str, base: &str) -> Result<(), WorktreeError>`, `fn inspect(&self, record: &WorktreeRecord) -> Result<WorktreeState, WorktreeError>`, `fn remove(&self, record: &WorktreeRecord) -> Result<(), WorktreeError>`
  - `MockWorktreePort` (under `test` or feature `mocks`)

- [ ] **Step 1: Write the port and its test**

Create `crates/forge-ports/src/worktree_port.rs`:

```rust
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
```

In `crates/forge-ports/src/lib.rs` add `mod worktree_port;` after `mod webview_port;`, then

```rust
pub use worktree_port::{BranchList, WorktreeError, WorktreePort, WorktreeState};
```

after `pub use webview_port::{...};`, and at the end of the mock exports:

```rust
#[cfg(any(test, feature = "mocks"))]
pub use worktree_port::MockWorktreePort;
```

- [ ] **Step 2: Run the tests**

Run: `cargo test -p forge-ports worktree 2>&1 | tail -10`
Expected: PASS (2 tests).

- [ ] **Step 3: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: WorktreePort, the port that makes and removes git worktrees" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `GitCli` implements `WorktreePort`

**Files:**
- Modify: `crates/forge-adapter-git/Cargo.toml`, `crates/forge-adapter-git/src/lib.rs`

**Interfaces:**
- Consumes: `WorktreePort`, `BranchList`, `WorktreeState`, `WorktreeError` (Task 2), `WorktreeRecord` (Task 1).
- Produces: `impl WorktreePort for GitCli`; `GitCli::run_for(cwd, args, timeout)` (private).

- [ ] **Step 1: Write the failing tests**

Add to `crates/forge-adapter-git/Cargo.toml` under `[dependencies]`: `forge-domain = { path = "../forge-domain" }`.

Append inside the existing `mod tests` of `crates/forge-adapter-git/src/lib.rs` (after the last test; it already has `git`, `repo`, `fs`, `Path`, `TempDir`), and add `use forge_domain::WorktreeRecord; use forge_ports::{WorktreeError, WorktreePort};` to the test module imports:

```rust
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
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-adapter-git 2>&1 | tail -15`
Expected: FAIL to compile (`GitCli` does not implement `WorktreePort`).

- [ ] **Step 3: Write the implementation**

In `crates/forge-adapter-git/src/lib.rs`:

1. Change the first `use` of forge_ports: `use forge_domain::WorktreeRecord;` and
   `use forge_ports::{BranchList, ChangeStatus, ChangedFile, GitError, GitPort, WorktreeError, WorktreePort, WorktreeState};`
2. Add a creation timeout constant under `DEFAULT_TIMEOUT`: `const CREATE_TIMEOUT: Duration = Duration::from_secs(120);`
3. Turn `run` into a thin wrapper. Rename the existing function `fn run(&self, cwd: &Path, args: &[&str]) -> Result<Output, GitError>` to `fn run_for(&self, cwd: &Path, args: &[&str], timeout: Duration) -> Result<Output, GitError>`, change `let deadline = Instant::now() + self.timeout;` to `let deadline = Instant::now() + timeout;`, and add above it:

```rust
    fn run(&self, cwd: &Path, args: &[&str]) -> Result<Output, GitError> {
        self.run_for(cwd, args, self.timeout)
    }
```

4. Add after `fn failure(...)`:

```rust
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
```

5. Add this impl before `#[cfg(test)]`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-adapter-git 2>&1 | tail -15`
Expected: PASS (existing tests and the 11 new ones). If `a_worktree_with_changes_is_not_removed` fails because `git worktree remove` removed the folder, run `git --version`; a git older than 2.17 is not supported and the plan stops here.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-git
git commit -m "feat: GitCli makes, inspects and removes worktrees" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `WorktreesUseCase`

**Files:**
- Create: `crates/forge-application/src/use_cases/worktrees.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`, `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `WorktreePort`, `BranchList`, `WorktreeError` (Task 2); `SettingsUseCase::{load, save_code_settings}`; domain `worktree_name`, `branch_name`, `worktree_dir`, `WorktreeRecord` (Task 1).
- Produces:
  - `struct WorktreesUseCase`, `WorktreesUseCase::new(git: Arc<dyn WorktreePort>, settings: Arc<SettingsUseCase>) -> Self`
  - `async fn branches(&self, repo: PathBuf) -> Result<BranchList, WorktreeUseCaseError>`
  - `async fn create(&self, repo: &Path, base: Option<String>, message: &str, seed: u64, created_at: i64) -> Result<WorktreeRecord, WorktreeUseCaseError>`: picks the name, makes the worktree, saves the record.
  - `async fn release(&self, path: &Path) -> Result<Release, WorktreeUseCaseError>`
  - `enum Release { Removed, Kept(Kept), NotTracked }`, `enum Kept { Changes, Commits }`
  - `enum WorktreeUseCaseError { Git(WorktreeError), Config(ConfigError) }` with `Display`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-application/src/use_cases/worktrees.rs` with the tests below at the bottom (the implementation comes in Step 3; write the file in two parts: tests now, implementation above them in Step 3).

```rust
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
                    && dir.starts_with("/work/app/.claude/worktrees/fix-the-bug-")
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
```

`forge-application`'s dev-dependencies need `tempfile`: add `tempfile = "3"` under `[dev-dependencies]` of `crates/forge-application/Cargo.toml` if it is not there already.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-application worktrees 2>&1 | tail -15`
Expected: FAIL to compile (`WorktreesUseCase` not found).

- [ ] **Step 3: Write the implementation**

Put this at the top of `crates/forge-application/src/use_cases/worktrees.rs`:

```rust
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
```

In `crates/forge-application/src/use_cases/mod.rs` add `mod worktrees;` and `pub use worktrees::{Kept, Release, WorktreeUseCaseError, WorktreesUseCase};`. In `crates/forge-application/src/lib.rs` add `pub use use_cases::{Kept, Release, WorktreeUseCaseError, WorktreesUseCase};` below the other `pub use use_cases::...` lines.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-application worktrees 2>&1 | tail -15`
Expected: PASS (10 tests). Then `cargo test -p forge-application 2>&1 | tail -5` still passes.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application
git commit -m "feat: WorktreesUseCase makes a worktree for a session and releases it on archive" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Sidebar grouping by origin, folder choices, wiring and state

**Files:**
- Create: `crates/forge-features/src/worktree_view.rs`
- Modify: `crates/forge-features/src/arrange.rs`, `crates/forge-features/src/lib.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: `origin_of`, `WorktreeRecord` (Task 1); `WorktreesUseCase`, `BranchList` (Tasks 2, 4).
- Produces:
  - `arrange::arrange_with_origins(groups, prefs, now, active, origin: &dyn Fn(&Path) -> PathBuf) -> Vec<ArrangedGroup>` (`arrange` keeps its signature)
  - `worktree_view::folder_choices(projects: Vec<ProjectSummary>, origin: &dyn Fn(&Path) -> PathBuf) -> Vec<ProjectSummary>`
  - `worktree_view::{RepoInfo { folder: PathBuf, branches: BranchList }, NewWorktree { base: Option<String> }, WorktreeTag { branch: Option<String> }}`
  - `WorkspaceServices.worktrees: Arc<WorktreesUseCase>`
  - `Workspace::origin_of(&self, &Path) -> PathBuf`, `Workspace::worktree_tag(&self, &Path) -> Option<WorktreeTag>`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/worktree_view.rs` with these tests at the bottom (implementation in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn project(path: &str, exists: bool) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: "2026-10-17T00:00:00Z".to_string(), session_count: 2, exists }
    }

    fn same(path: &Path) -> PathBuf {
        path.to_path_buf()
    }

    #[test]
    fn folders_that_are_not_worktrees_are_offered_as_they_are() {
        let choices = folder_choices(vec![project("/a", true), project("/b", true)], &same);
        assert_eq!(choices.iter().map(|p| p.path.clone()).collect::<Vec<_>>(), vec![PathBuf::from("/a"), PathBuf::from("/b")]);
    }

    #[test]
    fn a_worktree_is_replaced_by_its_repository_when_that_folder_exists() {
        let repo = std::env::temp_dir();
        let worktree = repo.join(".claude").join("worktrees").join("x-1");
        let home = repo.clone();
        let origin = move |path: &Path| if path.starts_with(&home) && path != home { home.clone() } else { path.to_path_buf() };
        let choices = folder_choices(vec![ProjectSummary { path: worktree, ..project("/ignored", true) }], &origin);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].path, repo);
        assert!(choices[0].exists);
    }

    #[test]
    fn a_worktree_of_a_repository_that_is_gone_is_not_offered() {
        let origin = |_: &Path| PathBuf::from("/definitely/not/here");
        assert!(folder_choices(vec![project("/w/x", true)], &origin).is_empty());
    }

    #[test]
    fn the_real_repository_wins_over_the_one_made_up_for_its_worktree() {
        let repo = std::env::temp_dir();
        let worktree = repo.join(".claude").join("worktrees").join("x-1");
        let home = repo.clone();
        let origin = move |path: &Path| if path == worktree { home.clone() } else { path.to_path_buf() };
        let projects = vec![ProjectSummary { path: repo.join(".claude").join("worktrees").join("x-1"), ..project("/", true) }, ProjectSummary { path: repo.clone(), session_count: 9, ..project("/", true) }];
        let choices = folder_choices(projects, &origin);
        assert_eq!(choices.len(), 1);
        assert_eq!(choices[0].session_count, 9);
    }
}
```

Add to the `tests` module of `crates/forge-features/src/arrange.rs`. It already has a helper `entry(title, created, updated)` (around line 292); add a project helper next to the tests you write:

```rust
    fn worktree_project(path: &str) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: String::new(), session_count: 1, exists: true }
    }

    #[test]
    fn a_worktrees_sessions_join_the_group_of_their_repository_and_keep_their_own_folder() {
        let groups = vec![
            SidebarGroup { project: worktree_project("/app"), entries: vec![entry("main", None, Some("2026-09-29T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/app/.claude/worktrees/x-1"), entries: vec![entry("tree", None, Some("2026-09-30T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/other"), entries: vec![entry("o", None, Some("2026-09-28T00:00:00Z"))] },
        ];
        let origin = |path: &std::path::Path| match path.to_str().unwrap() {
            "/app/.claude/worktrees/x-1" => PathBuf::from("/app"),
            other => PathBuf::from(other),
        };
        let arranged = arrange_with_origins(&groups, &SidebarPrefs::default(), now(), None, &origin);
        assert_eq!(arranged.len(), 2);
        let app = &arranged[0];
        assert!(matches!(&app.heading, Heading::Project(project) if project.path == PathBuf::from("/app")));
        assert_eq!(app.entries.len(), 2);
        // The newest first (Activity): the worktree's row keeps its own folder for opening and archiving.
        assert_eq!(app.entries[0].entry.title, "tree");
        assert_eq!(app.entries[0].project.path, PathBuf::from("/app/.claude/worktrees/x-1"));
        assert_eq!(app.entries[1].project.path, PathBuf::from("/app"));
    }

    #[test]
    fn a_group_made_only_of_a_worktree_is_headed_by_its_repository() {
        let groups = vec![SidebarGroup { project: worktree_project("/app/.claude/worktrees/x-1"), entries: vec![entry("tree", None, Some("2026-09-30T00:00:00Z"))] }];
        let origin = |_: &std::path::Path| PathBuf::from("/app");
        let arranged = arrange_with_origins(&groups, &SidebarPrefs::default(), now(), None, &origin);
        assert!(matches!(&arranged[0].heading, Heading::Project(project) if project.path == PathBuf::from("/app")));
        assert_eq!(arranged[0].id, folder_group_id(std::path::Path::new("/app")));
    }
```

(`now()` is whatever helper the module's tests already use to build an `OffsetDateTime`; run `grep -n "fn now\|OffsetDateTime::from\|datetime!" crates/forge-features/src/arrange.rs | head` and use the same expression the existing `arrange(` tests pass. `PathBuf` and `SidebarPrefs` are already imported by the tests.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features worktree 2>&1 | tail -10; cargo test -p forge-features arrange 2>&1 | tail -10`
Expected: FAIL to compile (`folder_choices`, `arrange_with_origins` not found).

- [ ] **Step 3: Write the implementation**

`crates/forge-features/src/worktree_view.rs` (above the tests):

```rust
use std::path::{Path, PathBuf};

use forge_domain::ProjectSummary;
use forge_ports::BranchList;

/// What the new-session panel knows about the chosen folder when it is a git repository.
#[derive(Debug, Clone, PartialEq)]
pub struct RepoInfo {
    pub folder: PathBuf,
    pub branches: BranchList,
}

/// The worktree the first message asks for: from `base`, or from the checked-out branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorktree {
    pub base: Option<String>,
}

/// What the header of a worktree session shows after "Worktree".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeTag {
    /// `None` for a worktree Forge has no record of.
    pub branch: Option<String>,
}

/// The folders a new session can start in. A worktree is not offered; its repository is, when
/// that folder still exists and is not listed already.
pub fn folder_choices(projects: Vec<ProjectSummary>, origin: &dyn Fn(&Path) -> PathBuf) -> Vec<ProjectSummary> {
    let mut out: Vec<ProjectSummary> = Vec::new();
    for project in projects {
        let home = origin(&project.path);
        let is_home = home == project.path;
        if let Some(existing) = out.iter_mut().find(|listed| listed.path == home) {
            // The real repository replaces the one that was made up for its worktree.
            if is_home {
                *existing = project;
            }
            continue;
        }
        if is_home {
            out.push(project);
        } else if home.is_dir() {
            out.push(ProjectSummary { path: home, last_active: project.last_active, session_count: 0, exists: true });
        }
    }
    out
}
```

Register the module in `crates/forge-features/src/lib.rs`: `mod worktree_view;` with the other `mod` lines and `pub use worktree_view::{folder_choices, NewWorktree, RepoInfo, WorktreeTag};` with the other exports.

`arrange.rs`: change `arrange` and `arrange_filtered` as follows. Add `use std::path::{Path, PathBuf};` if the file lacks it. Replace the `arrange` function with:

```rust
pub fn arrange(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
) -> Vec<ArrangedGroup> {
    arrange_with_origins(groups, prefs, now, active, &|path: &Path| path.to_path_buf())
}

/// `arrange`, where the Folder grouping puts the projects that share an `origin` in one group (a
/// worktree's sessions join their repository's). Every row keeps its own project, which is what
/// opening, archiving and forking use.
pub fn arrange_with_origins(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
    origin: &dyn Fn(&Path) -> PathBuf,
) -> Vec<ArrangedGroup> {
    let limited: Vec<SidebarGroup> = groups
        .iter()
        .map(|group| SidebarGroup {
            project: group.project.clone(),
            entries: group.entries.iter().filter(|entry| shown(entry, prefs.status)).cloned().collect(),
        })
        .collect();
    arrange_filtered(&limited, prefs, now, active, origin)
}
```

Add `origin: &dyn Fn(&Path) -> PathBuf,` as the last parameter of `arrange_filtered` and replace its `GroupBy::Folder => groups.iter()....collect(),` arm with:

```rust
        GroupBy::Folder => {
            let mut merged: Vec<(PathBuf, ProjectSummary, Vec<ArrangedEntry>)> = Vec::new();
            for group in groups {
                let home = origin(&group.project.path);
                let items: Vec<ArrangedEntry> = group
                    .entries
                    .iter()
                    .map(|entry| ArrangedEntry { project: group.project.clone(), entry: entry.clone() })
                    .collect();
                match merged.iter_mut().find(|(path, _, _)| *path == home) {
                    Some((_, heading, entries)) => {
                        heading.exists |= group.project.exists;
                        heading.session_count += group.project.session_count;
                        entries.extend(items);
                    }
                    None => {
                        let mut heading = group.project.clone();
                        heading.path = home.clone();
                        merged.push((home, heading, items));
                    }
                }
            }
            merged
                .into_iter()
                .filter(|(_, _, entries)| prefs.show_empty || !entries.is_empty())
                .map(|(home, heading, mut entries)| {
                    sort_entries(&mut entries, prefs.sort_by);
                    finish(folder_group_id(&home), Heading::Project(heading), entries, prefs, active)
                })
                .collect()
        }
```

The other arms of `arrange_filtered` do not use `origin`; call sites of `arrange_filtered` other than `arrange_with_origins` (grep: `grep -n "arrange_filtered" crates/forge-features/src/*.rs`) pass `&|path: &Path| path.to_path_buf()`. `folder_group_id` takes `&Path` (it is called with `&group.project.path` today).

Export: in `crates/forge-features/src/lib.rs` add `arrange_with_origins` to the `pub use arrange::{...}` list.

`workspace.rs`:

1. Imports: add `WorktreeRecord` to the `forge_domain` import and `WorktreesUseCase` to the `forge_application` import; add `use crate::worktree_view::{folder_choices, RepoInfo, WorktreeTag};` and `use crate::arrange::arrange_with_origins;` (extend the existing `arrange` import).
2. `WorkspaceServices`: add `pub worktrees: Arc<WorktreesUseCase>,` after `connector_catalog`.
3. `Workspace`: add fields `worktrees: Signal<Vec<WorktreeRecord>>,`, `new_repo: Signal<Option<RepoInfo>>,`, `new_unsent: Signal<Option<String>>,` and initialise them in `use_workspace_provider` with `Signal::new(Vec::new())`, `Signal::new(None)`, `Signal::new(None)`.
4. `reload_prefs`: add `let mut worktrees = self.worktrees;` and inside `if let Ok(config)`: `worktrees.set(config.worktrees);` (after `archived.set(config.archived);`).
5. `arranged_sidebar`: replace `arrange(&self.sidebar(), &self.prefs.read(), now, active)` with:

```rust
        let records = self.worktrees.read().clone();
        arrange_with_origins(&self.sidebar(), &self.prefs.read(), now, active, &|path| forge_domain::origin_of(&records, path))
```

6. `new_session_projects`: build the list as today into `projects`, then return `folder_choices(projects, &|path| forge_domain::origin_of(&records, path))` with `let records = self.worktrees.read().clone();` taken at the top. The function currently filters `project.exists`; keep that filter before `folder_choices`, and keep the `extra_folders` loop after it (extra folders are real folders).
7. Add methods:

```rust
    /// The folder a session's project belongs to in the sidebar and the header: the repository of
    /// a worktree, else the project itself.
    pub fn origin_of(&self, path: &Path) -> PathBuf {
        forge_domain::origin_of(&self.worktrees.read(), path)
    }

    /// `Some` for a session in a worktree (one Forge made, or one at `<repo>/.claude/worktrees/<name>`).
    pub fn worktree_tag(&self, path: &Path) -> Option<WorktreeTag> {
        if let Some(record) = self.worktrees.read().iter().find(|record| record.path == path) {
            return Some(WorktreeTag { branch: Some(record.branch.clone()) });
        }
        forge_domain::main_repo_of(path).map(|_| WorktreeTag { branch: None })
    }
```

`crates/forge-app/src/main.rs`: in `build_services`, create one `GitCli` and share it:

```rust
    let git = Arc::new(GitCli::new());
    let worktrees = Arc::new(WorktreesUseCase::new(git.clone(), settings.clone()));
```

(place after `settings` is built), replace `git: Arc::new(GitCli::new()),` with `git: git.clone(),`, and add `worktrees,` to `WorkspaceServices { ... }`. Add `WorktreesUseCase` to the `forge_application` import. `Arc<GitCli>` coerces to `Arc<dyn GitPort>` and `Arc<dyn WorktreePort>`.

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -15`
Expected: PASS and no errors. Fix any other `WorkspaceServices { .. }` construction the compiler finds (there is only `main.rs` today).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features crates/forge-app
git commit -m "feat: a worktree's sessions group under their repository in the sidebar" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The new-session screen starts a worktree session

**Files:**
- Modify: `crates/forge-features/src/new_session.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/new_session_controls.rs`, `crates/forge-widgets/src/new_session_panel.rs`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `WorktreesUseCase::{branches, create}` (Task 4), `RepoInfo`, `NewWorktree`, `Workspace::{origin_of, worktree_tag}` (Task 5).
- Produces:
  - `NewSessionDraft { mode, model, effort, worktree: Signal<bool>, branch: Signal<Option<String>> }`
  - `Workspace::new_repo(&self) -> Option<RepoInfo>` (only for the chosen folder), `Workspace::probe_new_repo(&self, Option<PathBuf>)`, `Workspace::new_unsent(&self) -> Option<String>`, `Workspace::clear_new_unsent(&self)`
  - `Workspace::start_new(.., folders: Vec<PathBuf>, worktree: Option<NewWorktree>)`
  - widgets `BranchChip`

- [ ] **Step 1: Write the failing test**

The pure part is the choice of whether a send creates a worktree. Add to `crates/forge-features/src/new_session.rs` (below `send_ready`) a test and, in Step 3, its function:

```rust
#[cfg(test)]
mod worktree_request_tests {
    use super::*;
    use crate::worktree_view::RepoInfo;
    use forge_ports::BranchList;

    fn repo(folder: &str) -> Option<RepoInfo> {
        Some(RepoInfo { folder: PathBuf::from(folder), branches: BranchList::default() })
    }

    #[test]
    fn a_repository_with_the_switch_on_asks_for_a_worktree_from_the_chosen_branch() {
        let asked = worktree_request(true, Some("dev".to_string()), repo("/a"), std::path::Path::new("/a"));
        assert_eq!(asked, Some(NewWorktree { base: Some("dev".to_string()) }));
    }

    #[test]
    fn no_worktree_when_the_switch_is_off_or_the_folder_is_not_the_probed_repository() {
        assert_eq!(worktree_request(false, None, repo("/a"), std::path::Path::new("/a")), None);
        assert_eq!(worktree_request(true, None, repo("/a"), std::path::Path::new("/b")), None);
        assert_eq!(worktree_request(true, None, None, std::path::Path::new("/a")), None);
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p forge-features worktree_request 2>&1 | tail -8`
Expected: FAIL to compile (`worktree_request` not found).

- [ ] **Step 3: Write the implementation**

**`new_session.rs`**:

```rust
use std::path::{Path, PathBuf};
use crate::worktree_view::{NewWorktree, RepoInfo};

/// The worktree a send asks for: only when the switch is on and the folder being started in is the
/// repository the panel probed.
pub fn worktree_request(on: bool, branch: Option<String>, repo: Option<RepoInfo>, folder: &Path) -> Option<NewWorktree> {
    (on && repo.is_some_and(|info| info.folder == folder)).then_some(NewWorktree { base: branch })
}
```

Add `worktree: Signal<bool>, branch: Signal<Option<String>>` to `NewSessionDraft` and to its construction in `NewSessionComposer`:
`NewSessionDraft { mode: Signal::new(PermissionMode::Default), model: Signal::new(None), effort: Signal::new(None), worktree: Signal::new(true), branch: Signal::new(None) }`.

In `NewSessionComposer`, change the folder `use_effect`:

```rust
    use_effect(move || {
        let folder = workspace.chosen_new_folder();
        // Another folder starts from its own current branch.
        let mut branch = draft.branch;
        branch.set(None);
        spawn(async move {
            workspace.probe_new_folder(folder.clone());
            workspace.probe_new_repo(folder);
        });
    });
    use_drop(move || workspace.probe_new_folder(None));
```

and add, below it, the effect that puts back a message whose worktree could not be made:

```rust
    use_effect(move || {
        if let Some(back) = workspace.new_unsent() {
            workspace.clear_new_unsent();
            text.set(back);
        }
    });
```

(`text` is the `Signal<String>` that the component already declares as `let text = use_signal(String::new);` and re-binds with `let mut text = text;`. Make the `use_effect` closures capture a `let mut text = text;` copy if the compiler asks.)

In `submit`, after `project` is found and before `workspace.start_new(`, add:

```rust
        let worktree = worktree_request(*draft.worktree.peek(), draft.branch.peek().clone(), workspace.new_repo(), &project.path);
```

and pass `worktree,` as the new last argument of `start_new`.

**`workspace.rs`**:

1. `use crate::worktree_view::NewWorktree;` and `use forge_shared::tr;` is already there.
2. Methods:

```rust
    /// The branches of the chosen folder when it is a git repository (and the probe has answered).
    pub fn new_repo(&self) -> Option<RepoInfo> {
        let chosen = self.chosen_new_folder()?;
        self.new_repo.read().clone().filter(|info| info.folder == chosen)
    }

    /// Reads the branches of `folder` for the new-session panel; `None` forgets them. The same
    /// folder keeps its answer.
    pub fn probe_new_repo(&self, folder: Option<PathBuf>) {
        let mut repo = self.new_repo;
        let Some(folder) = folder else {
            repo.set(None);
            return;
        };
        if repo.peek().as_ref().is_some_and(|info| info.folder == folder) {
            return;
        }
        repo.set(None);
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            let found = services.worktrees.branches(folder.clone()).await.ok();
            // The person may have chosen another folder while git ran.
            if this.chosen_new_folder().as_deref() != Some(folder.as_path()) {
                return;
            }
            repo.set(found.map(|branches| RepoInfo { folder, branches }));
        });
    }

    /// The message of a send whose worktree could not be made, until the panel takes it back.
    pub fn new_unsent(&self) -> Option<String> {
        self.new_unsent.read().clone()
    }

    pub fn clear_new_unsent(&self) {
        let mut unsent = self.new_unsent;
        unsent.set(None);
    }
```

3. `start_new`: add the parameter `worktree: Option<NewWorktree>,` last, and make the start of the `spawn_forever` body (before `let model = match model {`) read:

```rust
            let project = match worktree {
                None => project,
                Some(request) => {
                    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
                    let created_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
                    match services.worktrees.create(&project.path, request.base, &text, seed, created_at).await {
                        Ok(record) => {
                            let mut records = this.worktrees;
                            records.write().push(record.clone());
                            ProjectSummary { path: record.path, last_active: String::new(), session_count: 0, exists: true }
                        }
                        Err(error) => {
                            this.report(tr!("Could not create the worktree: {error}", error = error));
                            let mut unsent = this.new_unsent;
                            unsent.set(Some(text));
                            return;
                        }
                    }
                }
            };
```

The existing body already clones `services` and `this`; `text` is moved into the closure as today. Everything after it (`model`, `slot.end()`, `open_new_with_effort(&project, ...)`) is unchanged and now uses the worktree's project.

**`new_session_controls.rs`**: in `FolderChip` add the worktree row and the branch chip.

```rust
use forge_features::{effort_levels, folder_menu_rows, refocus_composer, use_new_session_draft, use_settings, use_workspace};
```

stays; replace the body of `FolderChip` after `let mut items ...` is built by:

```rust
    let mut draft = use_new_session_draft();
    let repo = workspace.new_repo();
    items.push(PickerItem { label: tr!("Open folder…").to_string(), description: None, key: None, checked: false, divider_before: true });
    if repo.is_some() {
        items.push(PickerItem { label: tr!("Worktree").to_string(), description: None, key: None, checked: *draft.worktree.read(), divider_before: true });
    }
```

(the existing `items.push` for "Open folder…" is kept; do not push it twice) and extend `onpick`:

```rust
            onpick: move |index: usize| {
                match paths.get(index) {
                    Some(path) => workspace.set_new_folder(Some(path.clone())),
                    None if index == paths.len() => workspace.open_folder(),
                    None => {
                        let on = *draft.worktree.peek();
                        draft.worktree.set(!on);
                    }
                }
                refocus_composer();
            },
```

Add the branch chip below `FolderChip`:

```rust
/// The branch a worktree starts from; shown only for a git folder with the worktree row on.
#[component]
pub fn BranchChip() -> Element {
    let workspace = use_workspace();
    let mut draft = use_new_session_draft();
    let Some(info) = workspace.new_repo() else { return rsx! {} };
    if !*draft.worktree.read() {
        return rsx! {};
    }
    let chosen = draft.branch.read().clone().or_else(|| info.branches.current.clone());
    let items: Vec<PickerItem> = info
        .branches
        .local
        .iter()
        .map(|name| PickerItem { label: name.clone(), description: None, key: None, checked: chosen.as_deref() == Some(name.as_str()), divider_before: false })
        .collect();
    let names = info.branches.local.clone();
    let label = chosen.unwrap_or_else(|| tr!("Detached HEAD").to_string());
    rsx! {
        Picker {
            title: tr!("Branch"),
            heading: tr!("Branches"),
            current: label,
            items,
            onpick: move |index: usize| {
                if let Some(name) = names.get(index) {
                    draft.branch.set(Some(name.clone()));
                }
                refocus_composer();
            },
        }
    }
}
```

In `new_session_panel.rs` add `BranchChip` to the import list and render `BranchChip {}` on the line after `FolderChip {}`.

**`chat_window.rs`**: the heading uses the repository's name, and a tag follows the title:

```rust
    let heading = format!(
        "{} · {}",
        project_label(&workspace.origin_of(&view.project_path)),
        view.session.title.as_deref().unwrap_or(tr!("Session"))
    );
    let tag = workspace.worktree_tag(&view.project_path).map(|tag| match tag.branch {
        Some(branch) => format!("{} · {}", tr!("Worktree"), branch),
        None => tr!("Worktree").to_string(),
    });
```

and in the header `rsx!`, right after the `span { class: "pane-title", "{heading}" }` line:

```rust
                if let Some(tag) = tag {
                    span { class: "pane-tag", "{tag}" }
                }
```

`crates/forge-shared/src/theme.css`: first run `grep -n "pane-title" crates/forge-shared/src/theme.css` and add next to that rule:

```css
.pane-tag {
  margin-left: 8px;
  padding: 1px 8px;
  border-radius: 999px;
  background: var(--surface-2, rgba(0, 0, 0, 0.06));
  color: var(--text-muted, inherit);
  font-size: 12px;
  white-space: nowrap;
}
```

(use the variable names that the `.pane-title` or `.picker-description` rules next to it use; the fallbacks above keep it readable if a name differs.)

**Korean text**: run

```bash
python3 scripts/i18n_add.py \
  'Worktree' '워크트리' \
  'Branch' '브랜치' \
  'Branches' '브랜치' \
  'Detached HEAD' '분리된 HEAD' \
  'Could not create the worktree: {error}' '워크트리를 만들 수 없습니다: {error}'
```

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features -p forge-shared 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -15`
Expected: PASS and no errors. If the i18n test names an entry as unused or unsorted, fix that entry only.

- [ ] **Step 5: Commit**

```bash
git add crates scripts
git commit -m "feat: the new-session screen can start a session in a git worktree" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Check stage A in the real window

**Files:** none (a check; record the result in the spec's "As built" in Task 14).

- [ ] **Step 1: Build a fixture repository and start the harness**

Read `scripts/harness/README.md` first (the memory file `driving-the-forge-app-visually.md` summarises it: `launch.sh`, `capture.py`, `click.py --image`, `stop.sh`). Make a throwaway repo and start the app:

```bash
rm -rf /private/tmp/forge-wt-fixture && mkdir -p /private/tmp/forge-wt-fixture && cd /private/tmp/forge-wt-fixture \
  && git init -q && echo hi > a.txt && git add a.txt && git -c user.name=t -c user.email=t@t commit -qm init && git branch dev
cd /Users/james/orca/workspaces/forge/features && scripts/harness/launch.sh
```

- [ ] **Step 2: Walk the checklist** (capture after every click; read the PNG)

1. New session screen → folder `Open folder…` → pick `/private/tmp/forge-wt-fixture` (or start a first session there through the harness fixture). The folder chip menu ends with `Worktree` (checked); a `master` chip appears beside the folder chip; its list shows `dev` and `master`.
2. Pick `dev`, send "Reply with just ok" with mode Manual. The session opens; `git -C /private/tmp/forge-wt-fixture worktree list` shows `.claude/worktrees/reply-with-just-ok-xxxxxx` on `claude/reply-with-just-ok-xxxxxx`; `git log` there shows the `dev` tip.
3. The sidebar shows one group `forge-wt-fixture` holding the session; the header reads `forge-wt-fixture · <title>` with a `Worktree · claude/...` tag.
4. Turn the `Worktree` row off in the folder menu, send another message: the session starts in the folder itself (no new worktree, no tag).
5. A folder that is not a git repository shows neither the row nor the chip.
6. A repository with a detached HEAD (`git -C ... checkout --detach`): the chip reads `Detached HEAD`; sending shows the notice "HEAD is detached: choose a branch first" (inside "Could not create the worktree: ...") and the typed text is back in the box.
7. The folder menu does not list the worktree folder as a recent folder.

- [ ] **Step 3: Stop the app and record**

Run `scripts/harness/stop.sh`. Note anything that differs from the checklist; fix it in a follow-up commit (`fix: ...`) before going on. A long branch list that runs off the screen needs `max-height: 320px; overflow-y: auto;` on `.picker-menu`; add it if you see it.

---

## Stage B: archive removes a clean worktree, and SET-08

### Task 8: Release the worktree when its last session is archived

**Files:**
- Modify: `crates/forge-features/src/worktree_view.rs`, `crates/forge-features/src/workspace.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `WorktreesUseCase::release`, `Release`, `Kept` (Task 4).
- Produces: `worktree_view::worktree_in_use(path, except, past, archived, live) -> bool`; `Workspace::archive_session` now releases the worktree.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `crates/forge-features/src/worktree_view.rs` (extend its imports with `use forge_domain::{ArchivedRef, SessionId, SessionSummary};`):

```rust
    fn session(id: &str) -> SessionSummary {
        SessionSummary { id: SessionId(id.to_string()), title: None, updated_at: String::new(), created_at: String::new() }
    }

    #[test]
    fn a_worktree_with_no_other_session_is_not_in_use() {
        let past = vec![(project("/w/x", true), vec![session("a")])];
        assert!(!worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &past, &[], &[]));
    }

    #[test]
    fn another_session_that_is_not_archived_keeps_the_worktree_in_use() {
        let past = vec![(project("/w/x", true), vec![session("a"), session("b")])];
        assert!(worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &past, &[], &[]));
    }

    #[test]
    fn an_archived_sibling_does_not_keep_it_in_use() {
        let past = vec![(project("/w/x", true), vec![session("a"), session("b")])];
        let archived = vec![ArchivedRef { project: PathBuf::from("/w/x"), session_id: "b".to_string() }];
        assert!(!worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &past, &archived, &[]));
    }

    #[test]
    fn a_sibling_in_another_folder_does_not_count() {
        let past = vec![(project("/w/x", true), vec![session("a")]), (project("/w/y", true), vec![session("b")])];
        assert!(!worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &past, &[], &[]));
    }

    #[test]
    fn an_open_session_of_the_folder_keeps_it_in_use_unless_it_is_the_archived_one() {
        let live = vec![(PathBuf::from("/w/x"), Some(SessionId("a".to_string()))), (PathBuf::from("/w/x"), None)];
        assert!(worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &[], &[], &live), "a new session with no id yet counts");
        let only_itself = vec![(PathBuf::from("/w/x"), Some(SessionId("a".to_string())))];
        assert!(!worktree_in_use(Path::new("/w/x"), &SessionId("a".to_string()), &[], &[], &only_itself));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features worktree_view 2>&1 | tail -10`
Expected: FAIL to compile (`worktree_in_use` not found).

- [ ] **Step 3: Write the implementation**

In `worktree_view.rs` (add `use forge_domain::{ArchivedRef, SessionId, SessionSummary};` to the imports):

```rust
/// Whether a session other than `except` still uses the folder `path`: one that is listed and not
/// archived, or one that is open (`live` holds each open session's folder and Claude session id;
/// a new session has no id yet and counts).
pub fn worktree_in_use(
    path: &Path,
    except: &SessionId,
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    archived: &[ArchivedRef],
    live: &[(PathBuf, Option<SessionId>)],
) -> bool {
    let listed = past
        .iter()
        .filter(|(project, _)| project.path == path)
        .flat_map(|(_, sessions)| sessions)
        .any(|session| session.id != *except && !archived.iter().any(|item| item.project == path && item.session_id == session.id.0));
    let open = live.iter().any(|(folder, id)| folder == path && id.as_ref() != Some(except));
    listed || open
}
```

In `workspace.rs` change `archive_session` and add `release_worktree`:

```rust
    /// Hides a session from the default list. Nothing of the transcript changes; when it was the last
    /// session of a worktree Forge made, the worktree is removed if it holds no work.
    pub fn archive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project.clone(), session.clone(), true);
        self.release_worktree(project, session);
    }

    fn release_worktree(&self, path: PathBuf, session: SessionId) {
        if !self.worktrees.peek().iter().any(|record| record.path == path) {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: the removal must finish even if the row that asked for it unmounts. It starts
        // after the caller returns, so a session the caller closes right after archiving is closed by then.
        dioxus::core::spawn_forever(async move {
            let live: Vec<(PathBuf, Option<SessionId>)> = this
                .live
                .peek()
                .iter()
                .map(|entry| {
                    let view = entry.view.peek();
                    (view.project_path.clone(), view.claude_session_id.clone())
                })
                .collect();
            if crate::worktree_view::worktree_in_use(&path, &session, &this.past.peek(), &this.archived.peek(), &live) {
                return;
            }
            let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
            match services.worktrees.release(&path).await {
                Ok(forge_application::Release::Removed) => {
                    let mut records = this.worktrees;
                    records.write().retain(|record| record.path != path);
                }
                Ok(_) => {}
                Err(error) => this.report(tr!("Could not remove the worktree {name}: {error}", name = name, error = error)),
            }
        });
    }
```

(`archive_session` replaces the existing one-line version; `archive_active` and the row menus call it unchanged. Export `Release` from `forge_application` is done in Task 4.)

Korean text:

```bash
python3 scripts/i18n_add.py \
  'Could not remove the worktree {name}: {error}' '워크트리 {name}을(를) 제거할 수 없습니다: {error}'
```

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features -p forge-shared 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -10`
Expected: PASS and no errors.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: archiving the last session of a worktree removes it when it holds no work" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The `Worktree location` setting (SET-08)

**Files:**
- Modify: `crates/forge-features/src/settings.rs`, `crates/forge-features/src/settings_pages.rs`, `crates/forge-widgets/src/settings_overlay.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `AppConfig.worktree_location`, `WorktreeLocation` (Task 1); `FolderPickerPort`.
- Produces: `Settings::worktree_location() -> WorktreeLocation`, `Settings::set_worktree_location(WorktreeLocation)`, `Settings::choose_worktree_folder()`, `Settings::location_nonce() -> u32`; `CodeChange::WorktreeLocation(WorktreeLocation)`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `crates/forge-features/src/settings.rs` (next to the existing `apply_code_change` tests around line 680):

```rust
    #[test]
    fn the_worktree_location_change_sets_only_the_location() {
        let mut config = AppConfig { auto_archive_days: Some(7), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::WorktreeLocation(forge_domain::WorktreeLocation::Folder("/trees".into())));
        assert_eq!(config.worktree_location, forge_domain::WorktreeLocation::Folder("/trees".into()));
        assert_eq!(config.auto_archive_days, Some(7));
        apply_code_change(&mut config, CodeChange::WorktreeLocation(forge_domain::WorktreeLocation::Project));
        assert_eq!(config.worktree_location, forge_domain::WorktreeLocation::Project);
    }

    #[test]
    fn the_search_finds_the_worktree_location_row_on_the_code_page() {
        let pages = crate::settings_pages::filter_pages("worktree");
        assert!(pages.contains(&SettingsSection::Code), "{pages:?}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features worktree_location 2>&1 | tail -10`
Expected: FAIL to compile (`CodeChange::WorktreeLocation` not found).

- [ ] **Step 3: Write the implementation**

`settings.rs`:

1. Import `WorktreeLocation` in the `use forge_domain::{...}` line.
2. `CodeChange`: add `WorktreeLocation(WorktreeLocation),`; `apply_code_change`: add `CodeChange::WorktreeLocation(value) => config.worktree_location = value,`.
3. `Settings` struct: add `location_nonce: Signal<u32>,`; initialise `location_nonce: Signal::new(0),` in `use_settings_provider`.
4. `after_save_code`: add `current.worktree_location = config.worktree_location;` and `current.auto_archive_days = config.auto_archive_days;` next to the other copies.
5. Methods (next to `set_dynamic_workflows`):

```rust
    pub fn worktree_location(&self) -> WorktreeLocation {
        self.saved.read().worktree_location.clone()
    }

    pub fn set_worktree_location(&self, location: WorktreeLocation) {
        self.change_code(CodeChange::WorktreeLocation(location));
    }

    /// Changes whenever the folder dialog closes without a choice, so the menu that opened it is drawn again.
    pub fn location_nonce(&self) -> u32 {
        *self.location_nonce.read()
    }

    /// Asks for a folder with the system dialog and keeps worktrees there from now on. A cancelled
    /// dialog changes nothing.
    pub fn choose_worktree_folder(&self) {
        if *self.trusted_picking.peek() {
            return;
        }
        let picker = (*self.folder_picker.peek()).clone();
        let this = *self;
        let mut picking = self.trusted_picking;
        picking.set(true);
        // Root scope, like the other saves: the dialog may outlive the page.
        dioxus::core::spawn_forever(async move {
            let picked = picker.pick_folder().await;
            picking.set(false);
            match picked {
                Some(folder) => this.set_worktree_location(WorktreeLocation::Folder(folder)),
                None => {
                    let mut nonce = this.location_nonce;
                    nonce.set(*nonce.peek() + 1);
                }
            }
        });
    }
```

(`trusted_picking` doubles as "a folder dialog is open"; it is the guard the trusted-folders dialog already uses, so two dialogs never open together.)

`settings_pages.rs`: add `tr!("Worktree location")` to the `SettingsSection::Code` row list.

`settings_overlay.rs`, in `CodeSection` before `let style_value`:

```rust
    let location = settings.worktree_location();
    let (location_value, mut location_options) = match &location {
        forge_domain::WorktreeLocation::Project => (PROJECT_LOCATION.to_string(), Vec::new()),
        forge_domain::WorktreeLocation::Folder(folder) => (folder.display().to_string(), vec![(folder.display().to_string(), folder.display().to_string())]),
    };
    location_options.insert(0, (PROJECT_LOCATION.to_string(), tr!("Inside the project (.claude/worktrees)").to_string()));
    location_options.push((CUSTOM_LOCATION.to_string(), tr!("Custom…").to_string()));
```

with constants next to `DEFAULT_STYLE_VALUE`:

```rust
const PROJECT_LOCATION: &str = "\u{0}project";
const CUSTOM_LOCATION: &str = "\u{0}custom";
```

and the row, placed before the `Output style` row:

```rust
        SettingRow {
            title: tr!("Worktree location"),
            description: tr!("Where the Git worktrees of isolated sessions on this computer are kept. A folder inside the project is not used."),
            Select {
                key: "{settings.location_nonce()}",
                value: location_value,
                options: location_options,
                onchange: move |value: String| match value.as_str() {
                    PROJECT_LOCATION => settings.set_worktree_location(forge_domain::WorktreeLocation::Project),
                    CUSTOM_LOCATION => settings.choose_worktree_folder(),
                    folder => settings.set_worktree_location(forge_domain::WorktreeLocation::Folder(folder.into())),
                },
            }
        }
```

Korean text:

```bash
python3 scripts/i18n_add.py \
  'Worktree location' '워크트리 위치' \
  'Inside the project (.claude/worktrees)' '프로젝트 내부 (.claude/worktrees)' \
  'Custom…' '사용자 설정…' \
  'Where the Git worktrees of isolated sessions on this computer are kept. A folder inside the project is not used.' '이 컴퓨터에서 격리된 세션에 사용할 Git 워크트리를 저장할 위치입니다. 프로젝트 안쪽 폴더는 사용되지 않습니다.'
```

(`forge-widgets` already depends on `forge-domain`; if the compiler says otherwise, add it to `crates/forge-widgets/Cargo.toml`.)

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features -p forge-shared 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -10`
Expected: PASS and no errors.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: the Worktree location setting (SET-08)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Check stage B in the real window

**Files:** none.

- [ ] **Step 1: Start the harness with a fresh fixture repository** (as in Task 7 Step 1).

- [ ] **Step 2: Walk the checklist**

1. Settings (⌘,) → Code: the `Worktree location` row shows `Inside the project (.claude/worktrees)`. Pick `Custom…`, choose `/private/tmp/forge-wt-trees` in the folder dialog (a made-up folder; `osascript` `⇧⌘G` types the path). The row shows the path. Cancel a second `Custom…`: the row goes back to the saved value (no stale `Custom…`).
2. Start a worktree session in the fixture repo: the worktree is `/private/tmp/forge-wt-trees/forge-wt-fixture/<name>`. Set the location back to the project and make another: it is under `.claude/worktrees`.
3. Archive a session whose worktree is clean (row menu → Archive, and again with `A`): the folder and the branch are gone (`git worktree list`, `git branch`), the row moves to Archived.
4. Make a session, `echo x > <worktree>/new.txt`, archive it: the row is archived, the folder stays, no notice.
5. Make a session, commit in its worktree, archive it: the folder and branch stay.
6. Fork a worktree session (key `F`), archive the original: the worktree stays (the fork uses it); archive the fork too: it goes.
7. Delete a worktree folder by hand (`rm -rf`), archive its session: no notice, `config.worktrees` has no record of it (`FORGE_CONFIG_PATH` file under `/private/tmp/forge-harness-home`).
8. A worktree session of the desktop app (made in the desktop app earlier) is grouped under its repository and shows `Worktree` without a branch; archiving it leaves the folder alone.

- [ ] **Step 3: Stop the app, fix what differs, commit fixes as `fix: ...`.**

---

## Stage C: archive inactive sessions (SET-07)

### Task 11: Which sessions are due

**Files:**
- Create: `crates/forge-domain/src/auto_archive.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Produces:
  - `struct ArchiveCandidate { project: PathBuf, session_id: String, updated_at: String, busy: bool, archived: bool }`
  - `fn sessions_to_archive(now_ms: i64, days: u32, rows: &[ArchiveCandidate]) -> Vec<&ArchiveCandidate>`
  - `const AUTO_ARCHIVE_CHOICES: [Option<u32>; 6]` = `[None, Some(1), Some(2), Some(7), Some(14), Some(30)]`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/auto_archive.rs` with the tests (implementation in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;
    // 2026-10-17T00:00:00Z
    const NOW: i64 = 1_792_195_200_000;

    fn row(updated_at: &str) -> ArchiveCandidate {
        ArchiveCandidate { project: "/p".into(), session_id: "s".to_string(), updated_at: updated_at.to_string(), busy: false, archived: false }
    }

    #[test]
    fn a_session_idle_for_exactly_the_days_is_due() {
        let rows = vec![row("2026-10-16T00:00:00Z")];
        assert_eq!(sessions_to_archive(NOW, 1, &rows).len(), 1);
        assert_eq!(NOW - DAY, 1_792_108_800_000);
    }

    #[test]
    fn a_session_idle_for_less_than_the_days_is_not_due() {
        let rows = vec![row("2026-10-16T00:00:01Z")];
        assert!(sessions_to_archive(NOW, 1, &rows).is_empty());
    }

    #[test]
    fn a_busy_or_archived_session_is_never_due() {
        let busy = ArchiveCandidate { busy: true, ..row("2020-01-01T00:00:00Z") };
        let archived = ArchiveCandidate { archived: true, ..row("2020-01-01T00:00:00Z") };
        assert!(sessions_to_archive(NOW, 30, &[busy, archived]).is_empty());
    }

    #[test]
    fn a_time_that_cannot_be_read_is_not_due() {
        assert!(sessions_to_archive(NOW, 1, &[row(""), row("yesterday")]).is_empty());
    }

    #[test]
    fn a_time_with_an_offset_is_read_correctly() {
        // 2026-10-16T09:00:00+09:00 is 2026-10-16T00:00:00Z.
        assert_eq!(sessions_to_archive(NOW, 1, &[row("2026-10-16T09:00:00+09:00")]).len(), 1);
    }

    #[test]
    fn the_menu_choices_are_none_and_the_five_periods() {
        assert_eq!(AUTO_ARCHIVE_CHOICES, [None, Some(1), Some(2), Some(7), Some(14), Some(30)]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-domain auto_archive 2>&1 | tail -10`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

Top of `crates/forge-domain/src/auto_archive.rs`:

```rust
use std::path::PathBuf;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

/// What the Archive inactive sessions menu offers, in order (`None` is off).
pub const AUTO_ARCHIVE_CHOICES: [Option<u32>; 6] = [None, Some(1), Some(2), Some(7), Some(14), Some(30)];

/// A session as the auto archive sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveCandidate {
    pub project: PathBuf,
    pub session_id: String,
    /// RFC 3339, the time of the session's last activity.
    pub updated_at: String,
    /// Open and running, waiting for an approval, or with background work.
    pub busy: bool,
    pub archived: bool,
}

/// The sessions to archive now: not archived, not busy, and idle for at least `days` days.
pub fn sessions_to_archive(now_ms: i64, days: u32, rows: &[ArchiveCandidate]) -> Vec<&ArchiveCandidate> {
    let limit = i64::from(days) * DAY_MS;
    rows.iter()
        .filter(|row| !row.archived && !row.busy)
        .filter(|row| {
            chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|at| now_ms - at.timestamp_millis() >= limit)
                .unwrap_or(false)
        })
        .collect()
}
```

In `lib.rs`: `mod auto_archive;` and `pub use auto_archive::{sessions_to_archive, ArchiveCandidate, AUTO_ARCHIVE_CHOICES};`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-domain auto_archive 2>&1 | tail -10`
Expected: PASS (6 tests). If `a_session_idle_for_exactly_the_days_is_due` fails on the constant check, recompute `NOW` with `date -u -j -f %Y-%m-%dT%H:%M:%SZ 2026-10-17T00:00:00Z +%s` and fix the two constants (the logic is what matters).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: which sessions are due for auto archive" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 12: The `Archive inactive sessions` setting row

**Files:**
- Modify: `crates/forge-features/src/settings.rs`, `crates/forge-features/src/settings_pages.rs`, `crates/forge-widgets/src/settings_overlay.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `AppConfig.auto_archive_days`, `AUTO_ARCHIVE_CHOICES` (Tasks 1, 11).
- Produces: `Settings::auto_archive_days() -> Option<u32>`, `Settings::set_auto_archive_days(Option<u32>)`; `CodeChange::AutoArchiveDays(Option<u32>)`; `fn auto_archive_label(Option<u32>) -> &'static str` in `forge-features` (English text for `tr!`).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module of `settings.rs`:

```rust
    #[test]
    fn the_auto_archive_change_sets_only_the_days() {
        let mut config = AppConfig { worktree_location: forge_domain::WorktreeLocation::Folder("/t".into()), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::AutoArchiveDays(Some(14)));
        assert_eq!(config.auto_archive_days, Some(14));
        assert_eq!(config.worktree_location, forge_domain::WorktreeLocation::Folder("/t".into()));
        apply_code_change(&mut config, CodeChange::AutoArchiveDays(None));
        assert_eq!(config.auto_archive_days, None);
    }

    #[test]
    fn the_search_finds_the_archive_inactive_sessions_row() {
        assert!(crate::settings_pages::filter_pages("inactive").contains(&SettingsSection::Code));
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-features auto_archive 2>&1 | tail -10`
Expected: FAIL to compile (`CodeChange::AutoArchiveDays` not found).

- [ ] **Step 3: Write the implementation**

`settings.rs`: add `AutoArchiveDays(Option<u32>),` to `CodeChange`, `CodeChange::AutoArchiveDays(days) => config.auto_archive_days = days,` to `apply_code_change`, and the accessors:

```rust
    pub fn auto_archive_days(&self) -> Option<u32> {
        self.saved.read().auto_archive_days
    }

    pub fn set_auto_archive_days(&self, days: Option<u32>) {
        self.change_code(CodeChange::AutoArchiveDays(days));
    }
```

(`after_save_code` already copies the field since Task 9.)

`settings_pages.rs`: add `tr!("Archive inactive sessions")` to the Code row list.

`settings_overlay.rs`: add the row before `Worktree location`:

```rust
        SettingRow {
            title: tr!("Archive inactive sessions"),
            description: tr!("Archives local sessions with no activity for a while. A session that is running or has background work is not archived, and a worktree with uncommitted changes stays on disk."),
            Select {
                value: forge_features::auto_archive_value(settings.auto_archive_days()),
                options: forge_domain::AUTO_ARCHIVE_CHOICES.iter().map(|days| (forge_features::auto_archive_value(*days), forge_shared::i18n::text(forge_features::auto_archive_label(*days)).to_string())).collect::<Vec<_>>(),
                onchange: move |value: String| settings.set_auto_archive_days(value.parse::<u32>().ok()),
            }
        }
```

and in `crates/forge-features/src/settings.rs` (public, exported from `lib.rs`):

```rust
/// The value of the menu entry for `days` (an empty text is off).
pub fn auto_archive_value(days: Option<u32>) -> String {
    days.map(|days| days.to_string()).unwrap_or_default()
}

/// The English text of the menu entry for `days`; the widget translates it.
pub fn auto_archive_label(days: Option<u32>) -> &'static str {
    match days {
        None => "None",
        Some(1) => "1 day",
        Some(2) => "2 days",
        Some(7) => "7 days",
        Some(14) => "14 days",
        Some(30) => "30 days",
        Some(_) => "Other",
    }
}
```

Because the labels reach `tr!` only through `forge_shared::i18n::text(label)`, the i18n source scan does not see them. Mark each as used in the sources with a function that spells them out through `tr!`:

```rust
/// The text of the menu entry for `days`, translated.
pub fn auto_archive_text(days: Option<u32>) -> String {
    match days {
        None => tr!("None"),
        Some(1) => tr!("1 day"),
        Some(2) => tr!("2 days"),
        Some(7) => tr!("7 days"),
        Some(14) => tr!("14 days"),
        Some(30) => tr!("30 days"),
        Some(other) => format!("{other}"),
    }
    .to_string()
}
```

and use `forge_features::auto_archive_text(*days)` in the widget instead of `i18n::text(auto_archive_label(...))`; drop `auto_archive_label`. Export `auto_archive_value` and `auto_archive_text` from `crates/forge-features/src/lib.rs`. (`tr!` returns a `&str`-like value; if the compiler wants `.to_string()` on each arm instead of one at the end, do that.)

Add a test:

```rust
    #[test]
    fn every_auto_archive_choice_has_a_value_and_a_text() {
        for days in forge_domain::AUTO_ARCHIVE_CHOICES {
            let value = auto_archive_value(days);
            assert_eq!(value.parse::<u32>().ok(), days);
            assert!(!auto_archive_text(days).is_empty());
        }
    }
```

Korean text:

```bash
python3 scripts/i18n_add.py \
  'Archive inactive sessions' '비활성 세션 보관' \
  'Archives local sessions with no activity for a while. A session that is running or has background work is not archived, and a worktree with uncommitted changes stays on disk.' '일정 기간 활동이 없으면 로컬 세션을 자동으로 보관합니다. 실행 중이거나 백그라운드 작업이 있는 세션은 보관되지 않으며, 커밋되지 않은 변경사항이 있는 워크트리는 디스크에 유지됩니다.' \
  'None' '없음' \
  '1 day' '1일' \
  '2 days' '2일' \
  '14 days' '14일' \
  '30 days' '30일' \
  '7 days' '7일'
```

(`'7d'` and `'30d'` exist in `ko.rs` already for other places; the new keys are different strings.)

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features -p forge-shared 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -10`
Expected: PASS and no errors.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: the Archive inactive sessions setting row (SET-07)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 13: The auto archive timer

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `sessions_to_archive`, `ArchiveCandidate` (Task 11); `Workspace::archive_session` (Task 8); `SettingsUseCase::load`.
- Produces: `Workspace::auto_archive_now(&self, now_ms: i64)`; the hourly timer started by `use_workspace_provider`; pure `fn candidates(past, archived, busy) -> Vec<ArchiveCandidate>` in `worktree_view.rs`.

- [ ] **Step 1: Write the failing test**

Add to the `tests` module of `crates/forge-features/src/worktree_view.rs`:

```rust
    #[test]
    fn candidates_list_every_past_session_with_its_state() {
        let past = vec![(project("/w/x", true), vec![SessionSummary { updated_at: "2026-01-01T00:00:00Z".to_string(), ..session("a") }, session("b")])];
        let archived = vec![ArchivedRef { project: PathBuf::from("/w/x"), session_id: "b".to_string() }];
        let busy = vec![SessionId("a".to_string())];
        let rows = candidates(&past, &archived, &busy);
        assert_eq!(rows.len(), 2);
        assert_eq!((rows[0].session_id.as_str(), rows[0].busy, rows[0].archived), ("a", true, false));
        assert_eq!((rows[1].session_id.as_str(), rows[1].busy, rows[1].archived), ("b", false, true));
        assert_eq!(rows[0].updated_at, "2026-01-01T00:00:00Z");
        assert_eq!(rows[0].project, PathBuf::from("/w/x"));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p forge-features candidates 2>&1 | tail -8`
Expected: FAIL to compile (`candidates` not found).

- [ ] **Step 3: Write the implementation**

`worktree_view.rs` (add `use forge_domain::ArchiveCandidate;`):

```rust
/// Every listed session as the auto archive sees it. `busy` holds the Claude session ids of the open
/// sessions that are running, waiting for an approval or have background work.
pub fn candidates(past: &[(ProjectSummary, Vec<SessionSummary>)], archived: &[ArchivedRef], busy: &[SessionId]) -> Vec<ArchiveCandidate> {
    past.iter()
        .flat_map(|(project, sessions)| {
            sessions.iter().map(|session| ArchiveCandidate {
                project: project.path.clone(),
                session_id: session.id.0.clone(),
                updated_at: session.updated_at.clone(),
                busy: busy.contains(&session.id),
                archived: archived.iter().any(|item| item.project == project.path && item.session_id == session.id.0),
            })
        })
        .collect()
}
```

`workspace.rs`:

```rust
const AUTO_ARCHIVE_EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);
```

Methods:

```rust
    /// Archives the sessions that have been idle longer than the setting says; nothing when it is off.
    /// Each one goes through `archive_session`, so a worktree follows the same rule as a hand archive.
    pub fn auto_archive_now(&self, now_ms: i64) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            let Ok(config) = services.settings.load().await else { return };
            let Some(days) = config.auto_archive_days else { return };
            let busy: Vec<SessionId> = this
                .live
                .peek()
                .iter()
                .filter_map(|entry| {
                    let view = entry.view.peek();
                    let working = view.phase == Phase::Running || !view.pending_permissions.is_empty() || !view.background_tasks.is_empty();
                    working.then(|| view.claude_session_id.clone()).flatten()
                })
                .collect();
            let rows = crate::worktree_view::candidates(&this.past.peek(), &this.archived.peek(), &busy);
            let due: Vec<(PathBuf, SessionId)> = forge_domain::sessions_to_archive(now_ms, days, &rows)
                .into_iter()
                .map(|row| (row.project.clone(), SessionId(row.session_id.clone())))
                .collect();
            if due.is_empty() {
                return;
            }
            let count = due.len();
            for (project, session) in due {
                this.archive_session(project, session);
            }
            this.report(tr!("Inactive sessions archived: {count}", count = count));
        });
    }

    // Once the lists have loaded, and then every hour while Forge is open.
    fn start_auto_archive(&self) {
        let this = *self;
        dioxus::core::spawn_forever(async move {
            // The sidebar lists load first (`reload_past`); a first pass over an empty list does nothing.
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            loop {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
                this.auto_archive_now(now);
                tokio::time::sleep(AUTO_ARCHIVE_EVERY).await;
            }
        });
    }
```

In `use_workspace_provider`, after `use_hook(move || workspace.reload_prefs());` add `use_hook(move || workspace.start_auto_archive());`.

Because `archive_session` writes the archived list through `set_archived` (a signal change plus a save), many archives in one pass are saved one after another, each with the whole list (the same as hand archives).

Korean text:

```bash
python3 scripts/i18n_add.py \
  'Inactive sessions archived: {count}' '비활성 세션을 보관했습니다: {count}'
```

- [ ] **Step 4: Run the tests and the build**

Run: `cargo test -p forge-features -p forge-shared -p forge-domain 2>&1 | tail -15; cargo check --workspace 2>&1 | tail -10`
Expected: PASS and no errors.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: archive inactive sessions on a timer (SET-07)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 14: Check stage C in the real window, then docs

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md`, `docs/superpowers/specs/2026-10-17-worktree-sessions-design.md` ("As built"), `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/` (a new file and one line in `MEMORY.md`)

- [ ] **Step 1: Walk the checklist** (harness; the fixture builder makes transcripts, so set one session's transcript file time old with `touch -t 202601010000 <transcript>.jsonl` before launching, or edit `updated_at` through the file time)

1. Settings → Code: `Archive inactive sessions` shows `None`; pick `1 day`, close Settings.
2. Restart the harness app (the first pass runs 20 seconds after start): the old session is archived (Archived filter), the notice `Inactive sessions archived: 1` shows, a recent session is not archived.
3. A session that is open and running when the pass runs is not archived (start one, leave it running, set the transcript time old, wait for a pass or restart).
4. A worktree session that is old and clean is archived and its folder removed; one with an untracked file is archived and the folder stays.
5. With `None` nothing is archived at start.
6. The `Worktree location` and `Archive inactive sessions` rows are found by the Settings search ("worktree", "inactive").

- [ ] **Step 2: Update the docs**

Backlog (`docs/superpowers/roadmap/backlog.md`): in the SET-07 and SET-08 rows set the Plan column to `Plan 47 (built)` and rewrite their Notes as what was built and what was not (the PR switch left out; `사용자 설정…` folder layout `<folder>/<repo>/<name>` is a guess; archive's removal rule; the Branch chip and the Worktree row). Add a row `PNL-08` "Worktree sessions (folder menu: branch and worktree switch; session in `.claude/worktrees/<name>`; group under the repository)" `seen | Plan 47 (built)`, naming what was not built (`.worktreeinclude`, the `환경` panel, routine worktree checkbox). Add the same to the `As built` section of the spec (what was checked in the real window in Tasks 7, 10 and 14, and what was not).

Memory: write `plan-47-worktree-sessions.md` (type `project`: what was built, the desktop app's facts that are not in the code: the worktree switch lives in the folder menu and is on by default for a git folder; `git-worktrees.json` and the session file fields; the name guess; the deferred minors), add a one-line pointer to `MEMORY.md`.

- [ ] **Step 3: Commit**

```bash
git add docs
git commit -m "docs: Plan 47 built: worktree sessions, worktree location and auto archive" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Then finish with `superpowers:finishing-a-development-branch` (the base branch is `master`).

---

## Self-review (done while writing)

- **Spec coverage:** worktree menu and creation (Tasks 5, 6), naming (1), record and grouping (5), header tag (6), archive removal and the keep rules (3, 4, 8), `Worktree location` incl. the inside-the-project rule (1, 9), `Archive inactive sessions` and the timer (11, 12, 13), tests and real-window checks (7, 10, 14), out-of-scope items stay out. The spec's "removal for `main_repo_of` worktrees" and "branch read from HEAD" are the deviations 3 in "Spec changes".
- **Types:** `WorktreeRecord.created_at: i64` is used by Tasks 1, 3 (`created_at: 0`), 4 (`created_at: i64`), 6 (`as_millis() as i64`). `WorktreeError::NameTaken` is returned by the adapter (3) and matched by the use case (4). `Release`/`Kept` (4) are matched in Task 8. `RepoInfo`, `NewWorktree`, `WorktreeTag` (5) are used in 6. `AUTO_ARCHIVE_CHOICES`, `ArchiveCandidate`, `sessions_to_archive` (11) are used in 12 and 13.
- **Known soft spots to watch while building:** the `Picker`'s checkable `Worktree` row closes the menu on click (the person reopens it to see the new state); the `now()` helper name in the `arrange.rs` tests; the exact CSS variable names for `.pane-tag`; `use_effect` capturing `text` in `NewSessionComposer`. Each is a compile or look-and-fix item, not a design question.
