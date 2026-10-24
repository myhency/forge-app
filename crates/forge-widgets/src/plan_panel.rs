use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{copy_text, render_markdown};
use forge_features::{use_workspace, Panel};
use forge_shared::tr;

use crate::panel_frame::PanelFrame;

/// The plan Claude proposed, rendered as markdown. Read-only: the answer is given in the card.
#[component]
pub fn PlanPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let plan = workspace.plan_text(id);
    let copied = plan.clone();
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Plan,
            tools: rsx! {
                button {
                    class: "icon-button",
                    r#type: "button",
                    disabled: plan.is_none(),
                    title: tr!("Copy"),
                    "aria-label": tr!("Copy"),
                    onclick: move |_| {
                        if let Some(text) = copied.as_deref() {
                            copy_text(text);
                        }
                    },
                    "⧉"
                }
            },
            match plan {
                Some(text) => rsx! {
                    div { class: "plan-body",
                        div { class: "dock-note", {tr!("Read the plan, then answer in the card below.")} }
                        div { class: "markdown", dangerous_inner_html: render_markdown(&text) }
                    }
                },
                None => rsx! { div { class: "dock-note", {tr!("No plan yet")} } },
            }
        }
    }
}
