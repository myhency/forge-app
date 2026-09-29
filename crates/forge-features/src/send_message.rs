use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_shared::{Button, ButtonKind, TextArea};

use crate::workspace::use_workspace;

/// The message box of an open session: Enter sends, Shift+Enter adds a line, and while a turn is
/// running a Stop button interrupts it.
#[component]
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool) -> Element {
    let workspace = use_workspace();
    let mut draft = use_signal(String::new);

    let mut submit = move || {
        let text = draft.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(handle) = workspace.handle(id) {
            handle.send(text);
            draft.set(String::new());
        }
    };
    let stop = move |_| {
        if let Some(handle) = workspace.handle(id) {
            handle.interrupt();
        }
    };

    rsx! {
        div { class: "composer",
            TextArea {
                value: draft(),
                placeholder: "Message Claude…",
                disabled: !can_send,
                oninput: move |text| draft.set(text),
                onkeydown: move |event: KeyboardEvent| {
                    // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                    if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                        event.prevent_default();
                        submit();
                    }
                },
            }
            div { class: "composer-row",
                span { class: "spacer" }
                if running {
                    Button { label: "Stop", kind: ButtonKind::Danger, onclick: stop }
                }
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
