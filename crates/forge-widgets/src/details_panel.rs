use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{card_folders, use_workspace, EnvironmentRequest, Panel};
use forge_shared::tr;

use crate::panel_frame::PanelFrame;

#[component]
pub fn DetailsPanel(id: LiveSessionId) -> Element {
    rsx! {
        PanelFrame { id, panel: Panel::Details, tools: rsx! {}, div { class: "details-body", EnvironmentCard { id } } }
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
    let environment = workspace.session_environment(&view.project_path);
    let folded = workspace.environment_card_folded(id);
    let folders = if scratch { Vec::new() } else { card_folders(&origin, &workspace.session_added_folders(id)) };
    let mut menu = use_signal(|| None::<usize>);

    rsx! {
        div { class: "env-card",
            button {
                class: "env-card-head",
                r#type: "button",
                "aria-expanded": "{!folded}",
                onclick: move |_| workspace.set_environment_card_folded(id, !folded),
                span { {tr!("Environment")} }
                span { class: if folded { "env-card-chevron folded" } else { "env-card-chevron" }, "⌄" }
            }
            if !folded {
                div { class: "env-card-row",
                    span { class: "env-card-icon", "💻" }
                    if let Some(environment) = environment.clone() {
                        span { class: "env-card-name", "{environment.name}" }
                        button {
                            class: "icon-button",
                            r#type: "button",
                            title: tr!("Edit"),
                            "aria-label": tr!("Edit"),
                            onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::EditInSession(environment.id.clone())),
                            "⚙"
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
                    FolderRow { key: "{folder.display()}", id, folder, first: index == 0, open: menu() == Some(index), on_open: move |open: bool| menu.set(if open { Some(index) } else { None }) }
                }
            }
        }
    }
}

#[component]
fn FolderRow(id: LiveSessionId, folder: PathBuf, first: bool, open: bool, on_open: EventHandler<bool>) -> Element {
    let workspace = use_workspace();
    let (name, parent) = folder_parts(&folder);
    rsx! {
        div { class: "env-card-folder",
            button { class: "env-card-row env-card-folder-button", r#type: "button", onclick: move |_| on_open.call(!open),
                span { class: "env-card-icon", "📁" }
                span { class: "env-card-name", "{name}" }
                span { class: "env-card-parent", " · {parent}" }
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
                        span { class: "picker-text", {tr!("Show in Finder")} }
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
                    // The Terminal panel opens in the session's own folder, so only the first row can use it.
                    if first {
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
