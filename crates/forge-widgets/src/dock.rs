use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_pane_width, use_workspace, Panel};

use crate::changes_panel::ChangesPanel;
use crate::files_panel::FilesPanel;
use crate::preview_panel::PreviewPanel;
use crate::details_panel::DetailsPanel;
use crate::plan_panel::PlanPanel;
use crate::side_panel::SidePanel;
use crate::tasks_panel::TasksPanel;
use crate::terminal_panel::TerminalPanel;

// Where a column's left edge was grabbed: the pointer's x and the column's width at that moment.
#[derive(Clone, Copy)]
struct Grab {
    column: usize,
    x: f64,
    width: f32,
    /// The width of the column left of this one, for a handle between two columns.
    before: f32,
}

/// The columns right of the chat. Each column's left edge is a handle that resizes it; while one
/// is held a transparent shield covers the window, so the drag follows the pointer even over the
/// terminal.
#[component]
pub fn Dock(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let state = workspace.dock(id);
    let mut grab = use_signal(|| None::<Grab>);
    let pane_width = use_pane_width() as f32;
    let widths: Vec<f32> = state.columns.iter().map(|column| column.width).collect();
    let rails: Vec<bool> = (0..state.columns.len()).map(|column| state.is_rail(column)).collect();
    if state.columns.is_empty() {
        return rsx! {};
    }
    let expanded = state.expanded;
    rsx! {
        div { class: "dock",
            if grab().is_some() {
                div {
                    class: "dock-drag-shield",
                    onmousemove: move |event| {
                        if let Some(start) = grab() {
                            let left = (start.x - event.client_coordinates().x) as f32;
                            workspace.change_dock(id, |dock| {
                                if dock.is_chat_edge(start.column) {
                                    dock.resize_chat_edge(start.column, start.width + left, pane_width);
                                } else if start.column > 0 {
                                    // Between two columns: the one on the left gives what the one on the right takes.
                                    dock.move_boundary(start.column - 1, start.before, start.width, -left);
                                }
                            });
                        }
                    },
                    onmouseup: move |_| grab.set(None),
                }
            }
            for (index, column) in state.columns.iter().enumerate() {
                if expanded.is_none_or(|panel| column.panels.contains(&panel)) {
                    {
                        let width = column.width;
                        let before = widths.get(index.wrapping_sub(1)).copied().unwrap_or(0.0);
                        let style = if expanded.is_some() { String::new() } else { format!("width: {width}px") };
                        rsx! {
                            div {
                                key: "{index}",
                                class: match (expanded.is_some(), column.panels.contains(&Panel::Details)) {
                                    (true, _) => "dock-column expanded",
                                    (false, true) => "dock-column rail",
                                    (false, false) => "dock-column",
                                },
                                style: "{style}",
                                if expanded.is_none() && !rails[index] {
                                    div {
                                        class: "dock-resize",
                                        onmousedown: move |event| {
                                            event.prevent_default();
                                            grab.set(Some(Grab { column: index, x: event.client_coordinates().x, width, before }));
                                        },
                                    }
                                }
                                for panel in column.panels.iter().copied() {
                                    match panel {
                                        // Keyed by panel only: `폴더 변경…` keeps open shells in the old folder; a new tab starts in `cwd`.
                                        Panel::Terminal => rsx! { TerminalPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                        // Keyed by folder too, so a moved session builds them again for its new folder.
                                        Panel::Changes => rsx! { ChangesPanel { key: "{panel:?}-{cwd.display()}", id, cwd: cwd.clone() } },
                                        Panel::Files => rsx! { FilesPanel { key: "{panel:?}-{cwd.display()}", id, cwd: cwd.clone() } },
                                        Panel::Preview => rsx! { PreviewPanel { key: "{panel:?}", id } },
                                        Panel::Tasks => rsx! { TasksPanel { key: "{panel:?}", id } },
                                        Panel::Plan => rsx! { PlanPanel { key: "{panel:?}", id } },
                                        Panel::Side => rsx! { SidePanel { key: "{panel:?}", id } },
                                        Panel::Details => rsx! { DetailsPanel { key: "{panel:?}", id } },
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
