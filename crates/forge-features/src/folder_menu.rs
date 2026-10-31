use std::path::{Path, PathBuf};

use forge_domain::ProjectSummary;

use crate::attachments::folder_label;

/// How many folders the new-session folder menu lists (the desktop app lists eight).
pub const MAX_RECENT_FOLDERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderMenuRow {
    pub label: String,
    pub path: PathBuf,
    pub checked: bool,
}

/// The rows of the folder menu: the projects in the order given, the first `MAX_RECENT_FOLDERS`; the
/// current folder takes the last place when it would not fit, so it is always listed and checked.
pub fn folder_menu_rows(projects: &[ProjectSummary], current: Option<&Path>) -> Vec<FolderMenuRow> {
    let row = |project: &ProjectSummary| FolderMenuRow {
        label: folder_label(&project.path),
        path: project.path.clone(),
        checked: current == Some(project.path.as_path()),
    };
    let mut rows: Vec<FolderMenuRow> = projects.iter().take(MAX_RECENT_FOLDERS).map(row).collect();
    if let Some(current_project) = projects.iter().skip(MAX_RECENT_FOLDERS).find(|project| current == Some(project.path.as_path())) {
        rows.pop();
        rows.push(row(current_project));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(path: &str) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: String::new(), session_count: 1, exists: true }
    }

    #[test]
    fn rows_keep_the_projects_order_name_the_folder_and_check_the_current_one() {
        let projects = vec![project("/work/mywork"), project("/work/forge")];
        let rows = folder_menu_rows(&projects, Some(Path::new("/work/forge")));
        let shown: Vec<_> = rows.iter().map(|r| (r.label.as_str(), r.checked)).collect();
        assert_eq!(shown, vec![("mywork", false), ("forge", true)]);
        assert_eq!(rows[1].path, PathBuf::from("/work/forge"));
    }

    #[test]
    fn at_most_eight_rows_are_offered_but_the_current_folder_is_always_among_them() {
        let projects: Vec<_> = (0..12).map(|i| project(&format!("/work/p{i}"))).collect();
        let rows = folder_menu_rows(&projects, Some(Path::new("/work/p10")));
        assert_eq!(rows.len(), MAX_RECENT_FOLDERS);
        assert!(rows.iter().any(|r| r.checked && r.path == Path::new("/work/p10")));
        let first = folder_menu_rows(&projects, Some(Path::new("/work/p0")));
        assert_eq!(first.len(), MAX_RECENT_FOLDERS);
        assert_eq!(first[0].path, PathBuf::from("/work/p0"));
    }

    #[test]
    fn no_projects_gives_no_rows() {
        assert!(folder_menu_rows(&[], None).is_empty());
    }
}
