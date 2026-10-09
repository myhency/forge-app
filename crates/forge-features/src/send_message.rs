use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::split_cards;
use forge_shared::{tr, Button, ButtonKind};

use crate::cards::line_count;
use crate::composer_cards::ComposerCards;
use crate::composer_state::{use_composer_state_provider, ComposerChips};
use crate::suggestion::{composer_placeholder, suggestion_to_accept};

use crate::command_box::CommandTextArea;
use crate::workspace::use_workspace;

/// The message box of an open session: Enter sends, Shift+Enter adds a line, and while a turn is
/// running a Stop button interrupts it.
#[component]
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool, children: Element) -> Element {
    let workspace = use_workspace();
    let draft = use_signal(String::new);
    let composer = use_composer_state_provider(draft);
    let mut draft = draft;
    let mut seen_failures = use_signal(|| 0u32);
    let view = workspace.live_view(id);
    let suggestion = view.and_then(|view| view.read().suggestion.clone());
    let accept = suggestion.clone();

    // A send that never reached the sidecar hands its text back through the view; put it in the box
    // again (unless the person has already started typing something else) instead of losing it.
    use_effect(move || {
        let Some(view) = view else { return };
        let (failures, unsent, unsent_attachments) = {
            let view = view.read();
            (view.failed_sends, view.unsent.clone(), view.unsent_attachments.clone())
        };
        if failures != *seen_failures.peek() {
            seen_failures.set(failures);
            // The cards were written into the text that came back: take them out again.
            let unsent = unsent.map(|text| {
                let (cards, rest) = split_cards(&text);
                workspace.restore_cards(id, cards);
                rest
            });
            // The chips come back whatever is in the box: they are not text the person could be typing over.
            composer.restore_attachments(unsent_attachments);
            if let Some(text) = unsent {
                if draft.peek().is_empty() {
                    draft.set(text);
                }
            }
        }
    });

    // Files the Files panel wants attached (a tree row or a tab menu) become chips.
    use_effect(move || {
        let queued = workspace.queued_files(id);
        if !queued.is_empty() {
            workspace.clear_queued_files(id);
            composer.add_paths(queued);
        }
    });

    let mut submit = move || {
        let cards = workspace.take_cards(id, line_count);
        let Some(outgoing) = composer.take_for_send(&draft.read(), &cards) else {
            // Refused (a chip could not be read): the cards stay where they were.
            workspace.restore_cards(id, cards);
            return;
        };
        if let Some(handle) = workspace.handle(id) {
            handle.send_with(outgoing.text, outgoing.attachments);
            draft.set(String::new());
        }
    };
    let stop = move |_| {
        if let Some(handle) = workspace.handle(id) {
            handle.interrupt();
        }
    };

    rsx! {
        div {
            class: "composer",
            // ⌘U adds a file or photo. The physical key: the character depends on the input method.
            onkeydown: move |event: KeyboardEvent| {
                let draft_empty = draft.peek().is_empty();
                if let Some(text) = suggestion_to_accept(&event.code().to_string(), accept.as_deref(), draft_empty, running) {
                    event.prevent_default();
                    draft.set(text);
                    return;
                }
                if event.code() == Code::KeyU && crate::command_held(event.modifiers()) {
                    event.prevent_default();
                    composer.add_files();
                }
            },
            if let Some(message) = composer.error() {
                div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
            }
            ComposerCards { id }
            ComposerChips {}
            CommandTextArea {
                value: draft(),
                commands: view.map(|view| view.read().commands.clone()).unwrap_or_default(),
                placeholder: composer_placeholder(suggestion.as_deref(), draft.read().is_empty(), running, tr!("Message Claude…")),
                disabled: !can_send,
                open_token: composer.popup_token(),
                oninput: move |text| draft.set(text),
                onsubmit: move |_| submit(),
            }
            div { class: "composer-row",
                {children}
                span { class: "spacer" }
                if running {
                    button {
                        class: "stop-button",
                        r#type: "button",
                        title: tr!("Stop"),
                        "aria-label": "Stop",
                        onclick: stop,
                        span { class: "stop-dot" }
                    }
                } else {
                    Button {
                        label: tr!("Send"),
                        kind: ButtonKind::Primary,
                        disabled: !can_send,
                        onclick: move |_| submit(),
                    }
                }
            }
        }
    }
}
