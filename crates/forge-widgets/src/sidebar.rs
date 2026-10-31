use std::collections::HashMap;
use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_features::{use_workspace, Active, EntryKey, Heading};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};

use crate::account_menu::AccountMenu;
use crate::filter_menu::SidebarFilterMenu;
use crate::session_row::{fork_indent, SessionRow};

/// The folder's own name for a sidebar heading; the full path stays available as a tooltip. A
/// path with no final component (the filesystem root) is shown whole.
pub fn project_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// The label of a project with the scratch root known: a scratch folder reads `This computer`.
pub fn project_label_in(scratch_root: Option<&Path>, path: &Path) -> String {
    if scratch_root.is_some_and(|root| forge_domain::is_scratch_group(root, path)) {
        tr!("This computer").to_string()
    } else {
        project_label(path)
    }
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
pub fn project_labels(paths: &[PathBuf], scratch_root: Option<&Path>) -> Vec<String> {
    let is_scratch = |path: &Path| scratch_root.is_some_and(|root| forge_domain::is_scratch_group(root, path));
    let labels: Vec<String> = paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            if is_scratch(path) {
                return tr!("This computer").to_string();
            }
            let base = project_label(path);
            let others: Vec<&PathBuf> = paths
                .iter()
                .enumerate()
                .filter(|(other, candidate)| *other != index && !is_scratch(candidate) && project_label(candidate) == base)
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
            if is_scratch(path) {
                label.clone()
            } else if labels.iter().filter(|other| *other == label).count() > 1 {
                format!("{} ({})", project_label(path), path.display())
            } else {
                label.clone()
            }
        })
        .collect()
}

/// The back and forward buttons. They are dim and do nothing when there is no screen to go to.
#[component]
pub fn NavButtons() -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "icon-button nav-button",
            r#type: "button",
            title: tr!("Go back"),
            "aria-label": "Back",
            disabled: !workspace.can_go_back(),
            onclick: move |_| workspace.go_back(),
            Icon { kind: IconKind::ArrowLeft }
        }
        button {
            class: "icon-button nav-button",
            r#type: "button",
            title: tr!("Go forward"),
            "aria-label": "Forward",
            disabled: !workspace.can_go_forward(),
            onclick: move |_| workspace.go_forward(),
            Icon { kind: IconKind::ArrowRight }
        }
    }
}

/// Search and filter on top, then the sessions grouped as the preferences say, then Settings.
#[component]
pub fn Sidebar() -> Element {
    let workspace = use_workspace();
    let prefs = workspace.prefs();
    let groups = workspace.arranged_sidebar(forge_features::now_local());
    // Headings of folder groups are told apart when two folders share a name; rows in a date
    // list show their folder's plain name.
    let paths: Vec<PathBuf> = groups
        .iter()
        .filter_map(|group| match &group.heading {
            Heading::Project(project) => Some(project.path.clone()),
            _ => None,
        })
        .collect();
    let scratch_root = workspace.scratch_root();
    let labels: HashMap<PathBuf, String> = paths.iter().cloned().zip(project_labels(&paths, scratch_root.as_deref())).collect();
    let active = workspace.active();
    let shown_targets = workspace.pane_targets();
    let notice = workspace.notice();

    rsx! {
        div { class: "sidebar", style: "width: {prefs.clamped_width()}px",
            div { class: "sidebar-top",
                NavButtons {}
                button {
                    class: "sidebar-search",
                    title: tr!("Search sessions ({key})", key = forge_domain::KEY_SEARCH),
                    onclick: move |_| workspace.open_search(),
                    span { Icon { kind: IconKind::Search } }
                    span { {tr!("Search")} }
                    span { class: "keys", "{forge_domain::KEY_SEARCH}" }
                }
                button {
                    class: "icon-button sidebar-hide",
                    title: tr!("Hide the sidebar"),
                    onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = true),
                    Icon { kind: IconKind::ChevronLeft }
                }
            }
            div { class: "sidebar-new",
                Button {
                    label: tr!("+ New session"),
                    kind: ButtonKind::Primary,
                    onclick: move |_| workspace.show_new_session(),
                }
                SidebarFilterMenu {}
            }
            button {
                class: if active == Active::Customize { "sidebar-link active" } else { "sidebar-link" },
                onclick: move |_| workspace.show_customize(),
                span { Icon { kind: IconKind::Sparkle } }
                span { {tr!("Customize")} }
            }
            button {
                class: if active == Active::Routines { "sidebar-link active" } else { "sidebar-link" },
                onclick: move |_| workspace.show_routines(),
                span { Icon { kind: IconKind::Clock } }
                span { {tr!("Routines")} }
            }
            div { class: "sidebar-scroll",
            if let Some(message) = notice {
                div { class: if workspace.notice_is_info() { "banner info" } else { "banner error" },
                    "{message} "
                    Button { label: tr!("Dismiss"), onclick: move |_| workspace.dismiss_notice() }
                }
            }
            for group in groups {
                div { key: "{group.id}",
                    match &group.heading {
                        Heading::None => rsx! {},
                        Heading::Bucket(bucket) => rsx! {
                            div { class: "sidebar-group-header",
                                {group_toggle(workspace, &group.id, group.collapsed, bucket.label().to_string(), String::new(), false)}
                            }
                        },
                        Heading::Project(project) => {
                            let is_scratch_group = workspace.scratch_available() && workspace.is_scratch_group(&project.path);
                            rsx! {
                            div { class: "sidebar-group-header",
                                {group_toggle(
                                    workspace,
                                    &group.id,
                                    group.collapsed,
                                    labels.get(&project.path).cloned().unwrap_or_default(),
                                    project.path.display().to_string(),
                                    !project.exists && !is_scratch_group,
                                )}
                                if project.exists || is_scratch_group {
                                    button {
                                        class: "group-add",
                                        title: if is_scratch_group { tr!("New session") } else { tr!("New session in this folder") },
                                        onclick: {
                                            let path = project.path.clone();
                                            move |_| {
                                                if workspace.is_scratch_group(&path) {
                                                    workspace.show_new_no_folder_session()
                                                } else {
                                                    workspace.show_new_session_in(path.clone())
                                                }
                                            }
                                        },
                                        "+"
                                    }
                                }
                            }
                            }
                        },
                    }
                    for item in group.entries.iter() {
                        SessionRow {
                            key: "{item.entry.key.dom_key()}",
                            project: item.project.path.clone(),
                            entry: item.entry.clone(),
                            project_name: match group.heading {
                                Heading::Project(_) => None,
                                _ => Some(project_label_in(scratch_root.as_deref(), &item.project.path)),
                            },
                            depth: item.depth,
                            fork_count: item.fork_count,
                            forks_folded: item.forks_folded,
                            active: match &item.entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            shown: match &item.entry.key {
                                EntryKey::Live(id) => shown_targets.contains(&Active::Live(*id)),
                                EntryKey::Past { .. } => false,
                            },
                            onopen: {
                                let project = item.project.clone();
                                let key = item.entry.key.clone();
                                move |beside: bool| match (&key, beside) {
                                    (EntryKey::Live(id), false) => workspace.open_session(*id),
                                    (EntryKey::Live(id), true) => workspace.add_session(*id),
                                    (EntryKey::Past { session, .. }, false) => workspace.open_past_clicked(project.clone(), session.clone()),
                                    (EntryKey::Past { session, .. }, true) => workspace.add_past(project.clone(), session.clone()),
                                }
                            },
                        }
                        if item.hidden_forks > 0 {
                            div { class: "fork-hidden", style: "margin-left: {fork_indent(item.depth.saturating_add(1))}px", {nested_label(item.hidden_forks)} }
                        }
                    }
                }
            }
            }
            div { class: "sidebar-footer",
                AccountMenu {}
            }
        }
    }
}

/// The grey line under a folded session: how many forks are folded away.
fn nested_label(count: usize) -> String {
    let text = if count == 1 { tr!("{count} nested session", count = count) } else { tr!("{count} nested sessions", count = count) };
    format!("└ {text}")
}

// A heading that collapses and expands its group when clicked.
fn group_toggle(
    workspace: forge_features::Workspace,
    id: &str,
    collapsed: bool,
    title: String,
    tooltip: String,
    missing: bool,
) -> Element {
    let id = id.to_string();
    rsx! {
        button {
            class: "group-toggle",
            title: if collapsed { tr!("Expand") } else { tr!("Collapse") },
            onclick: move |_| workspace.update_prefs(|prefs| prefs.toggle_collapsed(&id)),
            span { class: "chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: !collapsed } }
            span {
                class: if missing { "sidebar-group-title missing" } else { "sidebar-group-title" },
                title: "{tooltip}",
                "{title}"
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
    fn the_line_under_a_folded_session_counts_its_forks() {
        assert_eq!(nested_label(1), "└ 1 nested session");
        assert_eq!(nested_label(3), "└ 3 nested sessions");
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
        project_labels(&paths, None)
    }

    #[test]
    fn a_scratch_folder_is_labelled_this_computer_without_shifting_others() {
        let root = Path::new("/cfg/scratch-workspaces");
        let paths: Vec<PathBuf> = ["/a/app", "/cfg/scratch-workspaces/scratch-2026-10-20-aaaaaa", "/b/app"]
            .iter()
            .map(PathBuf::from)
            .collect();
        assert_eq!(project_labels(&paths, Some(root)), ["app (a)", "This computer", "app (b)"]);
        assert_eq!(project_label_in(Some(root), &paths[1]), "This computer");
        assert_eq!(project_label_in(Some(root), &paths[0]), "app");
        // No root: nothing is scratch.
        assert_eq!(project_label_in(None, &paths[1]), "scratch-2026-10-20-aaaaaa");
        // A folder merely named like a scratch folder elsewhere is not one.
        assert_eq!(project_label_in(Some(root), Path::new("/x/scratch-2026-10-20-aaaaaa")), "scratch-2026-10-20-aaaaaa");
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
