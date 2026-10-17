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
