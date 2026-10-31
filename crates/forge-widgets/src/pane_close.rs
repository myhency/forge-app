use dioxus::prelude::*;
use forge_features::use_workspace;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// The × on a pane's header while the screen is split.
#[component]
pub fn PaneClose(index: usize) -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "icon-button pane-close",
            r#type: "button",
            title: tr!("Close split view"),
            "aria-label": tr!("Close split view"),
            onclick: move |_| workspace.close_pane(index),
            Icon { kind: IconKind::Close, size: IconSize::Small }
        }
    }
}
