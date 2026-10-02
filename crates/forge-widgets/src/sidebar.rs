use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_features::{use_settings, use_workspace, Active, EntryKey};
use forge_shared::{Button, ButtonKind};

use crate::session_row::SessionRow;

/// The folder's own name for a sidebar heading; the full path stays available as a tooltip. A
/// path with no final component (the filesystem root) is shown whole.
pub fn project_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

// The name of the `depth`-th ancestor of `path` (depth 1 is the parent). The root is written as
// `/`, which keeps `/app` apart from `/b/app`.
fn ancestor_name(path: &Path, depth: usize) -> Option<String> {
    path.ancestors().nth(depth).map(|ancestor| {
        ancestor
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| ancestor.to_string_lossy().to_string())
    })
}

/// One heading per project, in the order of `paths`. Folders that share a name are told apart by
/// the nearest parent name that differs among them: `app (a)` and `app (b)` for `/a/app` and
/// `/b/app`. A name nobody else uses stays plain.
pub fn project_labels(paths: &[PathBuf]) -> Vec<String> {
    let labels: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let base = project_label(path);
            let others: Vec<&PathBuf> = paths
                .iter()
                .enumerate()
                .filter(|(other, candidate)| *other != index && project_label(candidate) == base)
                .map(|(_, candidate)| candidate)
                .collect();
            if others.is_empty() {
                return base;
            }
            let depth_limit = path.ancestors().count();
            for depth in 1..depth_limit {
                let mine = ancestor_name(path, depth);
                if others.iter().all(|other| ancestor_name(other, depth) != mine) {
                    if let Some(parent) = mine {
                        return format!("{base} ({parent})");
                    }
                }
            }
            format!("{base} ({})", path.display())
        })
        .collect();
    // Paths can stop at different depths and land on the same text; two different folders must
    // never share a heading, so any label that is still not unique shows the full path.
    labels
        .iter()
        .zip(paths)
        .map(|(label, path)| {
            if labels.iter().filter(|other| *other == label).count() > 1 {
                format!("{} ({})", project_label(path), path.display())
            } else {
                label.clone()
            }
        })
        .collect()
}

/// Sessions of every project, grouped by project, with a "+ New session" button on top.
#[component]
pub fn Sidebar() -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
    let groups = workspace.sidebar();
    let labels = project_labels(&groups.iter().map(|group| group.project.path.clone()).collect::<Vec<_>>());
    let active = workspace.active();
    let notice = workspace.notice();

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-new",
                Button {
                    label: "+ New session",
                    kind: ButtonKind::Primary,
                    onclick: move |_| workspace.show_new_session(),
                }
            }
            if let Some(message) = notice {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| workspace.dismiss_notice() }
                }
            }
            for (group, label) in groups.into_iter().zip(labels) {
                div { key: "{group.project.path.display()}",
                    div { class: "sidebar-group-header",
                        div {
                            class: if group.project.exists { "sidebar-group-title" } else { "sidebar-group-title missing" },
                            title: "{group.project.path.display()}",
                            "{label}"
                        }
                        if group.project.exists {
                            button {
                                class: "group-add",
                                title: "New session in this folder",
                                onclick: {
                                    let path = group.project.path.clone();
                                    move |_| workspace.show_new_session_in(path.clone())
                                },
                                "+"
                            }
                        }
                    }
                    for entry in group.entries.iter() {
                        SessionRow {
                            key: "{entry.key.dom_key()}",
                            project: group.project.path.clone(),
                            entry: entry.clone(),
                            active: match &entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            onopen: {
                                let project = group.project.clone();
                                let key = entry.key.clone();
                                move |_| match &key {
                                    EntryKey::Live(id) => workspace.select(*id),
                                    EntryKey::Past { session, .. } => {
                                        workspace.open_past(project.clone(), session.clone())
                                    }
                                }
                            },
                        }
                    }
                }
            }
            div { class: "sidebar-footer",
                button { class: "sidebar-footer-button", onclick: move |_| settings.open(),
                    span { "⚙" }
                    span { "Settings" }
                    span { class: "keys", "⌘," }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_label_is_the_folders_own_name() {
        assert_eq!(project_label(Path::new("/Users/me/Developer/forge")), "forge");
        assert_eq!(project_label(Path::new("/Users/me/my project")), "my project");
        assert_eq!(project_label(Path::new("/work/한글 폴더")), "한글 폴더");
    }

    #[test]
    fn a_trailing_slash_does_not_hide_the_name() {
        assert_eq!(project_label(Path::new("/work/forge/")), "forge");
    }

    #[test]
    fn the_root_is_shown_whole() {
        assert_eq!(project_label(Path::new("/")), "/");
    }

    fn labels(paths: &[&str]) -> Vec<String> {
        let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
        project_labels(&paths)
    }

    #[test]
    fn unique_names_stay_plain() {
        assert_eq!(labels(&["/work/forge", "/work/other"]), ["forge", "other"]);
    }

    #[test]
    fn same_named_folders_show_their_parent() {
        assert_eq!(labels(&["/a/app", "/b/app"]), ["app (a)", "app (b)"]);
    }

    #[test]
    fn the_nearest_parent_that_tells_them_apart_is_used() {
        assert_eq!(labels(&["/x/same/app", "/y/same/app"]), ["app (x)", "app (y)"]);
    }

    #[test]
    fn only_the_shared_names_get_a_parent() {
        assert_eq!(
            labels(&["/a/app", "/work/other", "/b/app"]),
            ["app (a)", "other", "app (b)"]
        );
    }

    #[test]
    fn a_folder_at_the_root_is_told_apart_by_the_root() {
        assert_eq!(labels(&["/app", "/b/app"]), ["app (/)", "app (b)"]);
    }

    #[test]
    fn no_two_different_folders_share_a_heading() {
        let result = labels(&["/a/app", "/a/w/app", "/b/w/app"]);

        let mut sorted = result.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "headings must be unique, got {result:?}");
        assert_eq!(result[2], "app (b)", "a heading that was already unique is kept");
    }

    #[test]
    fn three_folders_with_one_name_each_get_their_own_parent() {
        assert_eq!(
            labels(&["/a/app", "/b/app", "/c/app"]),
            ["app (a)", "app (b)", "app (c)"]
        );
    }
}
