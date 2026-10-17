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
