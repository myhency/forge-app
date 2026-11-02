use dioxus::prelude::*;

use crate::use_workspace;

/// Which pane a component is drawn in. A signal, because closing the left pane makes the right one
/// pane 0 without remounting it.
#[derive(Clone, Copy)]
pub struct PaneIndex(pub Signal<usize>);

/// The width of the pane this component is in, in pixels. A signal, because the window and the splits change it.
#[derive(Clone, Copy)]
pub struct PaneWidth(pub Signal<u32>);

/// The width of the pane this component is in; 1200 outside any pane.
pub fn use_pane_width() -> u32 {
    try_use_context::<PaneWidth>().map(|width| *width.0.read()).unwrap_or(1200)
}

/// The index of the pane this component is in; 0 outside any pane.
pub fn use_pane_index() -> usize {
    try_use_context::<PaneIndex>().map(|index| *index.0.read()).unwrap_or(0)
}

/// Whether the pane this component is in has the focus. True outside any pane.
pub fn use_pane_focused() -> bool {
    let workspace = use_workspace();
    match try_use_context::<PaneIndex>() {
        Some(index) => workspace.focused_pane() == *index.0.read(),
        None => true,
    }
}
