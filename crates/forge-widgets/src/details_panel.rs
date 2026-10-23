use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{card_folders, use_workspace, DetailsCard, EnvironmentRequest, Panel};
use forge_shared::tr;

use crate::panel_frame::PanelFrame;

#[component]
pub fn DetailsPanel(id: LiveSessionId) -> Element {
    rsx! {
        PanelFrame { id, panel: Panel::Details, tools: rsx! {}, div { class: "details-body", EnvironmentCard { id } } }
    }
}

#[component]
fn CardIcon(kind: &'static str) -> Element {
    rsx! {
        svg {
            class: "env-card-icon",
            view_box: "0 0 16 16",
            width: "14",
            height: "14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match kind {
                "laptop" => rsx! {
                    rect { x: "3", y: "3", width: "10", height: "7", rx: "1" }
                    path { d: "M1.5 12.5h13" }
                },
                "folder" => rsx! { path { d: "M2 4.5A1 1 0 0 1 3 3.5h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z" } },
                "gear" => rsx! {
                    circle { cx: "8", cy: "8", r: "2" }
                    path { d: "M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" }
                },
                _ => rsx! { path { d: "M4 6l4 4 4-4" } },
            }
        }
    }
}

fn folder_parts(path: &Path) -> (String, String) {
    let name = path.file_name().map_or_else(|| path.to_string_lossy().to_string(), |name| name.to_string_lossy().to_string());
    let parent = path.parent().map(|parent| parent.to_string_lossy().to_string()).unwrap_or_default();
    (name, parent)
}

#[component]
fn EnvironmentCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let view = view.read().clone();
    let origin = workspace.origin_of(&view.project_path);
    let scratch = workspace.is_scratch_project(&view.project_path);
    let environment = workspace.session_environment(&view.project_path, view.claude_session_id.as_ref());
    let folded = workspace.details_card_folded(id, DetailsCard::Environment);
    let folders = if scratch { Vec::new() } else { card_folders(&origin, &workspace.session_added_folders(id)) };
    let mut menu = use_signal(|| None::<usize>);
    let busy = workspace.session_is_busy(id);
    let worktree_session = workspace.worktree_tag(&view.project_path).is_some();

    rsx! {
        div { class: "env-card",
            button {
                class: "env-card-head",
                r#type: "button",
                "aria-expanded": "{!folded}",
                onclick: move |_| workspace.set_details_card_folded(id, DetailsCard::Environment, !folded),
                span { {tr!("Environment")} }
                span { class: if folded { "env-card-chevron folded" } else { "env-card-chevron" }, CardIcon { kind: "chevron" } }
            }
            if !folded {
                div { class: "env-card-row",
                    CardIcon { kind: "laptop" }
                    if let Some(environment) = environment.clone() {
                        {
                            let environment_id = environment.id.clone();
                            rsx! {
                        span { class: "env-card-name", "{environment.name}" }
                        button {
                            class: "icon-button",
                            r#type: "button",
                            title: tr!("Edit"),
                            "aria-label": tr!("Edit"),
                            onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::EditInSession(environment_id.clone())),
                            CardIcon { kind: "gear" }
                        }
                            }
                        }
                    } else {
                        span { class: "env-card-name", {tr!("This computer")} }
                        if !scratch {
                            button {
                                class: "env-card-save",
                                r#type: "button",
                                title: tr!("Local session"),
                                onclick: {
                                    let origin = origin.clone();
                                    let view = view.clone();
                                    move |_| workspace.open_environment_dialog(EnvironmentRequest::NewFromSession { folder: origin.clone(), model: view.model.clone(), effort: view.effort })
                                },
                                {tr!("Save")}
                            }
                        }
                    }
                }
                for (index, folder) in folders.iter().cloned().enumerate() {
                    FolderRow { key: "{folder.display()}", id, folder, first: index == 0, busy, worktree_session, open: menu() == Some(index), on_open: move |open: bool| menu.set(if open { Some(index) } else { None }) }
                }
            }
        }
    }
}

#[component]
fn FolderRow(id: LiveSessionId, folder: PathBuf, first: bool, busy: bool, worktree_session: bool, open: bool, on_open: EventHandler<bool>) -> Element {
    let workspace = use_workspace();
    let (name, parent) = folder_parts(&folder);
    let items = forge_features::folder_menu(first, busy, worktree_session);
    rsx! {
        div { class: "env-card-folder",
            button { class: "env-card-row env-card-folder-button", r#type: "button", onclick: move |_| on_open.call(!open),
                CardIcon { kind: "folder" }
                span { class: "env-card-name", "{name}" }
                // The desktop app names the parent on the session's own folder only.
                if first {
                    span { class: "env-card-parent", " · {parent}" }
                }
            }
            if open {
                div { class: "picker-backdrop", onclick: move |_| on_open.call(false) }
                div {
                    class: "picker-menu env-card-menu",
                    role: "menu",
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            on_open.call(false);
                        }
                    },
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: {
                            let folder = folder.clone();
                            move |_| {
                                on_open.call(false);
                                workspace.reveal(&folder);
                            }
                        },
                        span { class: "picker-text", {forge_features::show_in_file_manager()} }
                    }
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: {
                            let folder = folder.clone();
                            move |_| {
                                on_open.call(false);
                                forge_entities::copy_text(&folder.to_string_lossy());
                            }
                        },
                        span { class: "picker-text", {tr!("Copy path")} }
                    }
                    if items.change {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.change_session_folder(id);
                            },
                            span { class: "picker-text", {tr!("Change folder…")} }
                        }
                    }
                    if items.add {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.add_session_folder(id);
                            },
                            span { class: "picker-text", {tr!("Add folder…")} }
                        }
                    }
                    if items.terminal {
                        div { class: "picker-divider" }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.change_dock(id, |dock| {
                                    if !dock.is_open(Panel::Terminal) {
                                        dock.toggle(Panel::Terminal);
                                    }
                                });
                            },
                            span { class: "picker-text", {tr!("Open in terminal")} }
                        }
                    }
                }
            }
        }
    }
}
