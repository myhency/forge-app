use std::collections::HashSet;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::ChangesViewPrefs;
use forge_entities::icon_of;
use forge_features::{tree_rows, use_workspace, ChangeTreeRow, ROW_LIMIT};
use forge_ports::{ChangeStatus, ChangedFile};
use forge_shared::tr;

use crate::files_icons::FileTypeIcon;

/// The right column: the changed files as a folder tree (or flat), then whatever `commits` draws.
#[component]
pub fn ChangesTree(
    id: LiveSessionId,
    files: Vec<ChangedFile>,
    prefs: ChangesViewPrefs,
    closed: HashSet<String>,
    on_pick: EventHandler<String>,
    on_menu: EventHandler<(String, bool, f64, f64)>,
    commits: Element,
) -> Element {
    let workspace = use_workspace();
    let rows = tree_rows(&files, prefs.group_by_folder, &closed);
    let hidden = rows.len().saturating_sub(ROW_LIMIT);
    rsx! {
        div { class: "cd-side",
            ul { class: "cd-tree",
                for row in rows.into_iter().take(ROW_LIMIT) {
                    match row {
                        ChangeTreeRow::Folder { path, name, depth, open } => rsx! {
                            li { key: "dir:{path}",
                                button {
                                    class: "cd-row",
                                    r#type: "button",
                                    style: "padding-left: {depth * 14 + 6}px;",
                                    onclick: {
                                        let path = path.clone();
                                        move |_| {
                                            workspace.update_changes_session(id, |session| {
                                                if !session.closed_folders.remove(&path) {
                                                    session.closed_folders.insert(path.clone());
                                                }
                                            })
                                        }
                                    },
                                    span { class: "cd-chevron", if open { "▾" } else { "▸" } }
                                    span { class: "cd-name", "{name}" }
                                }
                            }
                        },
                        ChangeTreeRow::File { path, name, depth, added, removed, status, .. } => {
                            let deleted = status == ChangeStatus::Deleted;
                            let counts = match (added, removed) {
                                (Some(added), Some(removed)) => rsx! {
                                    span { class: "cd-counts",
                                        span { class: "cd-add", "+{added}" }
                                        " "
                                        span { class: "cd-del", "−{removed}" }
                                    }
                                },
                                _ => rsx! {},
                            };
                            rsx! {
                                li { key: "file:{path}",
                                    button {
                                        class: "cd-row",
                                        r#type: "button",
                                        title: "{path}",
                                        style: "padding-left: {depth * 14 + 6}px;",
                                        onclick: {
                                            let path = path.clone();
                                            move |_| on_pick.call(path.clone())
                                        },
                                        oncontextmenu: {
                                            let path = path.clone();
                                            move |event: MouseEvent| {
                                                event.prevent_default();
                                                let point = event.client_coordinates();
                                                on_menu.call((path.clone(), deleted, point.x, point.y));
                                            }
                                        },
                                        FileTypeIcon { icon: icon_of(&path) }
                                        span { class: "cd-name", "{name}" }
                                        {counts}
                                    }
                                }
                            }
                        }
                    }
                }
                if hidden > 0 {
                    li { class: "cd-note", {tr!("… {count} more files", count = hidden)} }
                }
            }
            {commits}
        }
    }
}
