use std::path::{Path, PathBuf};

use forge_domain::{ArchiveCandidate, ArchivedRef, ProjectSummary, SessionId, SessionSummary};
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

/// Whether a session other than `except` still uses the folder `path`: one that is listed and not
/// archived, or any one that is open (`live` holds each open session's folder and Claude session id;
/// a new session has no id yet; the archived session itself counts while it is still open).
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
    let open = live.iter().any(|(folder, _)| folder == path);
    listed || open
}

/// Why a worktree could not be made, in the person's language; git's own text for any other failure.
pub fn creation_error_text(error: &forge_application::WorktreeUseCaseError) -> String {
    use forge_application::WorktreeUseCaseError;
    use forge_ports::WorktreeError;
    match error {
        WorktreeUseCaseError::Git(WorktreeError::Detached) => forge_shared::tr!("HEAD is detached: choose a branch first").to_string(),
        WorktreeUseCaseError::Git(WorktreeError::NotARepository) => forge_shared::tr!("Not a git repository").to_string(),
        WorktreeUseCaseError::Git(WorktreeError::NameTaken) => forge_shared::tr!("A worktree with that name already exists").to_string(),
        other => other.to_string(),
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn project(path: &str, exists: bool) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: "2026-10-17T00:00:00Z".to_string(), session_count: 2, exists }
    }

    fn session(id: &str) -> SessionSummary {
        SessionSummary { id: SessionId(id.to_string()), title: None, updated_at: String::new(), created_at: String::new() }
    }

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
    fn any_open_session_of_the_folder_keeps_it_in_use() {
        let a = SessionId("a".to_string());
        assert!(worktree_in_use(Path::new("/w/x"), &a, &[], &[], &[(PathBuf::from("/w/x"), None)]), "a new session with no id yet counts");
        assert!(worktree_in_use(Path::new("/w/x"), &a, &[], &[], &[(PathBuf::from("/w/x"), Some(SessionId("b".to_string())))]));
        assert!(worktree_in_use(Path::new("/w/x"), &a, &[], &[], &[(PathBuf::from("/w/x"), Some(a.clone()))]), "the archived session still open counts");
        assert!(!worktree_in_use(Path::new("/w/x"), &a, &[], &[], &[(PathBuf::from("/w/y"), None)]));
    }

    #[test]
    fn an_open_idle_session_is_never_due() {
        let past = vec![(project("/w/x", true), vec![session("a"), session("b")])];
        let rows = candidates(&past, &[], &[SessionId("a".to_string())]);
        assert!(rows.iter().find(|row| row.session_id == "a").unwrap().busy);
        assert!(!rows.iter().find(|row| row.session_id == "b").unwrap().busy);
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
