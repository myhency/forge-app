use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_shared::{Button, ButtonKind};

use crate::composer_state::{use_composer_state_provider, ComposerChips};

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
            // The chips come back whatever is in the box: they are not text the person could be typing over.
            composer.restore_attachments(unsent_attachments);
            if let Some(text) = unsent {
                if draft.peek().is_empty() {
                    draft.set(text);
                }
            }
        }
    });

    let mut submit = move || {
        let Some(outgoing) = composer.take_for_send(&draft.read()) else {
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
                if event.code() == Code::KeyU && event.modifiers().meta() {
                    event.prevent_default();
                    composer.add_files();
                }
            },
            if let Some(message) = composer.error() {
                div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
            }
            ComposerChips {}
            CommandTextArea {
                value: draft(),
                commands: view.map(|view| view.read().commands.clone()).unwrap_or_default(),
                placeholder: "Message Claude…",
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
                        title: "Stop",
                        "aria-label": "Stop",
                        onclick: stop,
                        span { class: "stop-dot" }
                    }
                } else {
                    Button {
                        label: "Send",
                        kind: ButtonKind::Primary,
                        disabled: !can_send,
                        onclick: move |_| submit(),
                    }
                }
            }
        }
    }
}
