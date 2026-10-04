use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

use crate::changes_panel::ChangesPanel;
use crate::files_panel::FilesPanel;
use crate::preview_panel::PreviewPanel;
use crate::tasks_panel::TasksPanel;
use crate::terminal_panel::TerminalPanel;

// Where a column's left edge was grabbed: the pointer's x and the column's width at that moment.
#[derive(Clone, Copy)]
struct Grab {
    column: usize,
    x: f64,
    width: f32,
}

/// The columns right of the chat. Each column's left edge is a handle that resizes it; while one
/// is held a transparent shield covers the window, so the drag follows the pointer even over the
/// terminal.
#[component]
pub fn Dock(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let state = workspace.dock(id);
    let mut grab = use_signal(|| None::<Grab>);
    if state.columns.is_empty() {
        return rsx! {};
    }
    let expanded = state.expanded;
    rsx! {
        if grab().is_some() {
            div {
                class: "dock-drag-shield",
                onmousemove: move |event| {
                    if let Some(start) = grab() {
                        let width = start.width + (start.x - event.client_coordinates().x) as f32;
                        workspace.change_dock(id, |dock| dock.resize(start.column, width));
                    }
                },
                onmouseup: move |_| grab.set(None),
            }
        }
        for (index, column) in state.columns.iter().enumerate() {
            if expanded.is_none_or(|panel| column.panels.contains(&panel)) {
                {
                    let width = column.width;
                    let style = if expanded.is_some() { String::new() } else { format!("width: {width}px") };
                    rsx! {
                        div {
                            key: "{index}",
                            class: if expanded.is_some() { "dock-column expanded" } else { "dock-column" },
                            style: "{style}",
                            if expanded.is_none() {
                                div {
                                    class: "dock-resize",
                                    onmousedown: move |event| {
                                        event.prevent_default();
                                        grab.set(Some(Grab { column: index, x: event.client_coordinates().x, width }));
                                    },
                                }
                            }
                            for panel in column.panels.iter().copied() {
                                match panel {
                                    Panel::Terminal => rsx! { TerminalPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                    Panel::Changes => rsx! { ChangesPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                    Panel::Files => rsx! { FilesPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                    Panel::Preview => rsx! { PreviewPanel { key: "{panel:?}", id } },
                                    Panel::Tasks => rsx! { TasksPanel { key: "{panel:?}", id } },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
