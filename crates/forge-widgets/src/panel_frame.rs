use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};
use forge_shared::tr;

/// The box every dock panel sits in: a header with its title, the panel's own tools, expand and
/// close, and the body below.
#[component]
pub fn PanelFrame(id: LiveSessionId, panel: Panel, tools: Element, children: Element) -> Element {
    let workspace = use_workspace();
    let expanded = workspace.dock(id).expanded == Some(panel);
    rsx! {
        section { class: "dock-panel", "aria-label": "{panel.title()}",
            header { class: "dock-header",
                span { class: "dock-title", "{panel.title()}" }
                div { class: "dock-tools", {tools} }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: if expanded { tr!("Collapse") } else { tr!("Expand") },
                    "aria-label": if expanded { tr!("Collapse") } else { tr!("Expand") },
                    onclick: move |_| {
                        workspace.change_dock(id, |dock| if expanded { dock.collapse() } else { dock.expand(panel) })
                    },
                    if expanded { "⤡" } else { "⤢" }
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Close"),
                    "aria-label": tr!("Close"),
                    onclick: move |_| workspace.change_dock(id, |dock| dock.close(panel)),
                    "✕"
                }
            }
            div { class: "dock-body", {children} }
        }
    }
}
