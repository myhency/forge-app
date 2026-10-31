use dioxus::prelude::*;
use forge_application::{LiveSessionId, QueuedMessage};
use forge_shared::{tr, Icon, IconKind};

use crate::composer_state::use_composer_state;
use crate::workspace::use_workspace;

/// The queued messages the person sees, each with its place in the whole queue (the buttons act on that place).
/// What a run of a code block tells the model is queued like a message but is not one of theirs.
fn visible_queued(queued: &[QueuedMessage]) -> Vec<(usize, &QueuedMessage)> {
    queued.iter().enumerate().filter(|(_, message)| !forge_entities::code_run::is_run_message(&message.text)).collect()
}

/// Messages waiting for the running turn to end: one row each, with send now, edit and remove.
/// The session sends the front one itself when the turn ends.
#[component]
pub fn QueuedCard(id: LiveSessionId, draft: Signal<String>) -> Element {
    let workspace = use_workspace();
    let composer = use_composer_state();
    let mut draft = draft;
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let all = view.read().queued.clone();
    let queued = visible_queued(&all);
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
            for (index, message) in queued.iter().copied() {
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
                        Icon { kind: IconKind::ArrowUp }
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
                        Icon { kind: IconKind::Pencil }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn queued(text: &str) -> QueuedMessage {
        QueuedMessage { text: text.to_string(), attachments: Vec::new() }
    }

    #[test]
    fn a_queued_run_message_has_no_card_row_and_the_others_keep_their_place() {
        let run = forge_entities::code_run::model_message("ls", Some(0), "a\n", false);
        let all = vec![queued("first"), queued(&run), queued("third")];
        let rows: Vec<(usize, &str)> = visible_queued(&all).into_iter().map(|(index, message)| (index, message.text.as_str())).collect();
        assert_eq!(rows, vec![(0, "first"), (2, "third")]);
        assert!(visible_queued(&[queued(&run)]).is_empty());
    }
}
