use std::path::Path;

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

/// Sessions of every project, grouped by project, with a "+ New session" button on top.
#[component]
pub fn Sidebar() -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
    let groups = workspace.sidebar();
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
            for group in groups {
                div { key: "{group.project.path.display()}",
                    div { class: "sidebar-group-header",
                        div {
                            class: if group.project.exists { "sidebar-group-title" } else { "sidebar-group-title missing" },
                            title: "{group.project.path.display()}",
                            "{project_label(&group.project.path)}"
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
}
