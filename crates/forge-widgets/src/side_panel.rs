use dioxus::prelude::*;
use forge_application::{LiveSessionId, SideAnswer};
use forge_entities::render_markdown;
use forge_features::{use_workspace, Panel};
use forge_shared::{tr, TextArea};

use crate::panel_frame::PanelFrame;

/// The `/btw` side chat: each question on the right, its answer under it, and a box to ask more.
/// Nothing here is part of the session; the main chat stays as it was.
#[component]
pub fn SidePanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut draft = use_signal(String::new);
    let thread = workspace.live_view(id).map(|view| view.read().side_thread.clone()).unwrap_or_default();
    let mut ask = move || {
        let question = draft.peek().trim().to_string();
        if !question.is_empty() {
            workspace.ask_side(id, question);
            draft.set(String::new());
        }
    };
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Side,
            tools: rsx! {},
            div { class: "side-chat",
                div { class: "side-thread",
                    if thread.is_empty() {
                        div { class: "dock-note", {tr!("Ask something without adding it to the session. Start with /btw or type below.")} }
                    }
                    for side in thread.iter() {
                        div { key: "{side.id}", class: "side-turn",
                            div { class: "side-question", "{side.question}" }
                            match &side.answer {
                                SideAnswer::Pending => rsx! { div { class: "side-note", {tr!("Thinking…")} } },
                                SideAnswer::Done(text) => rsx! { div { class: "side-answer markdown", dangerous_inner_html: render_markdown(text) } },
                                SideAnswer::Failed(reason) => rsx! {
                                    div { class: "side-note", if reason.is_empty() { {tr!("There was no answer.")} } else { "{reason}" } }
                                },
                            }
                        }
                    }
                }
                div { class: "side-input",
                    TextArea {
                        value: draft(),
                        placeholder: tr!("Reply in this session…").to_string(),
                        oninput: move |text: String| draft.set(text),
                        onkeydown: move |event: KeyboardEvent| {
                            // Enter asks; Shift+Enter adds a line; Enter that ends an IME composition is the input method's.
                            if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                                event.prevent_default();
                                ask();
                            }
                        },
                    }
                }
            }
        }
    }
}
