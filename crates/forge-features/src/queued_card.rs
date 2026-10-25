use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_shared::tr;

use crate::composer_state::use_composer_state;
use crate::workspace::use_workspace;

/// Messages waiting for the running turn to end: one row each, with send now, edit and remove.
/// The session sends the front one itself when the turn ends.
#[component]
pub fn QueuedCard(id: LiveSessionId, draft: Signal<String>) -> Element {
    let workspace = use_workspace();
    let composer = use_composer_state();
    let mut draft = draft;
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let queued = view.read().queued.clone();
    if queued.is_empty() {
        return rsx! {};
    }
    let title = if queued.len() == 1 {
        tr!("{count} message waiting", count = queued.len())
    } else {
        tr!("{count} messages waiting", count = queued.len())
    };
    rsx! {
        div { class: "queue-card",
            div { class: "queue-title", "{title}" }
            for (index, message) in queued.iter().enumerate() {
                div { key: "{index}-{message.text}", class: "queue-row",
                    span { class: "queue-text", title: "{message.text}", "{message.text}" }
                    button {
                        class: "queue-action",
                        r#type: "button",
                        title: tr!("Send now"),
                        "aria-label": tr!("Send now"),
                        onclick: move |_| {
                            if let Some(handle) = workspace.handle(id) {
                                handle.dequeue(index, true);
                            }
                        },
                        "↑"
                    }
                    button {
                        class: "queue-action",
                        r#type: "button",
                        title: tr!("Edit"),
                        "aria-label": tr!("Edit"),
                        onclick: {
                            let message = message.clone();
                            move |_| {
                                if let Some(handle) = workspace.handle(id) {
                                    handle.dequeue(index, false);
                                }
                                composer.restore_attachments(message.attachments.clone());
                                let rest = draft.peek().clone();
                                draft.set(if rest.is_empty() { message.text.clone() } else { format!("{}\n{rest}", message.text) });
                            }
                        },
                        "✎"
                    }
                    button {
                        class: "queue-action",
                        r#type: "button",
                        title: tr!("Remove"),
                        "aria-label": tr!("Remove"),
                        onclick: move |_| {
                            if let Some(handle) = workspace.handle(id) {
                                handle.dequeue(index, false);
                            }
                        },
                        "×"
                    }
                }
            }
        }
    }
}
