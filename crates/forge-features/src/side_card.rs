use dioxus::prelude::*;
use forge_application::{LiveSessionId, SideAnswer};
use forge_entities::render_markdown;
use forge_shared::tr;

use crate::workspace::use_workspace;

/// The answer to a `/btw` question, above the message box. It is not part of the session; the cross
/// closes it.
#[component]
pub fn SideCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let Some(side) = view.read().side_question.clone() else { return rsx! {} };
    rsx! {
        div { class: "side-card",
            div { class: "side-head",
                span { class: "side-title", "/btw " "{side.question}" }
                button {
                    class: "card-remove always",
                    r#type: "button",
                    "aria-label": tr!("Close"),
                    onclick: move |_| {
                        if let Some(handle) = workspace.handle(id) {
                            handle.dismiss_side();
                        }
                    },
                    "×"
                }
            }
            match side.answer {
                SideAnswer::Pending => rsx! { div { class: "side-note", {tr!("Thinking…")} } },
                SideAnswer::Done(text) => {
                    let html = render_markdown(&text);
                    rsx! { div { class: "side-answer markdown", dangerous_inner_html: "{html}" } }
                }
                SideAnswer::Failed(reason) => rsx! {
                    div { class: "side-note", if reason.is_empty() { {tr!("There was no answer.")} } else { "{reason}" } }
                },
            }
        }
    }
}
