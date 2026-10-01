use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::SlashCommand;
use forge_entities::{command_query, filter_commands, insert_text, menu_action, move_selection, MenuAction, MenuKey};
use forge_shared::{Button, ButtonKind, TextArea};

use crate::workspace::use_workspace;

/// The message box of an open session: Enter sends, Shift+Enter adds a line, and while a turn is
/// running a Stop button interrupts it.
#[component]
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool, children: Element) -> Element {
    let workspace = use_workspace();
    let mut draft = use_signal(String::new);
    let mut seen_failures = use_signal(|| 0u32);
    let mut selected = use_signal(|| 0usize);
    let mut dismissed = use_signal(|| false);
    let view = workspace.live_view(id);

    // A send that never reached the sidecar hands its text back through the view; put it in the box
    // again (unless the person has already started typing something else) instead of losing it.
    use_effect(move || {
        let Some(view) = view else { return };
        let (failures, unsent) = {
            let view = view.read();
            (view.failed_sends, view.unsent.clone())
        };
        if failures != *seen_failures.peek() {
            seen_failures.set(failures);
            if let Some(text) = unsent {
                if draft.peek().is_empty() {
                    draft.set(text);
                }
            }
        }
    });

    // The popup rows: only while the input is just a `/name` and Esc has not closed it.
    let matches: Vec<SlashCommand> = match view {
        Some(view) if !dismissed() && can_send => {
            let text = draft.read();
            match command_query(&text) {
                Some(query) => filter_commands(&view.read().commands, query).into_iter().cloned().collect(),
                None => Vec::new(),
            }
        }
        _ => Vec::new(),
    };
    let current = move_selection(selected(), matches.len(), 0);
    let pick = {
        let matches = matches.clone();
        move |index: usize| {
            if let Some(command) = matches.get(index) {
                draft.set(insert_text(command));
                selected.set(0);
            }
        }
    };

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
            if !matches.is_empty() {
                div { class: "command-menu", role: "listbox",
                    for (index, command) in matches.iter().enumerate() {
                        div {
                            key: "{command.name}",
                            role: "option",
                            class: if index == current { "command-item selected" } else { "command-item" },
                            // mousedown, not click: a click would first take the focus from the text box.
                            onmousedown: {
                                let mut pick = pick.clone();
                                move |event: MouseEvent| {
                                    event.prevent_default();
                                    pick(index);
                                }
                            },
                            span { class: "command-name", "/{command.name}" }
                            if let Some(hint) = command.argument_hint.as_ref() {
                                span { class: "command-hint", "{hint}" }
                            }
                            if let Some(description) = command.description.as_ref() {
                                span { class: "command-desc", "{description}" }
                            }
                        }
                    }
                }
            }
            TextArea {
                value: draft(),
                placeholder: "Message Claude…",
                disabled: !can_send,
                oninput: move |text| {
                    draft.set(text);
                    selected.set(0);
                    dismissed.set(false);
                },
                onkeydown: {
                    let mut pick = pick.clone();
                    let count = matches.len();
                    move |event: KeyboardEvent| {
                        if count > 0 {
                            let key = match event.key() {
                                Key::ArrowUp => MenuKey::Up,
                                Key::ArrowDown => MenuKey::Down,
                                Key::Enter => MenuKey::Enter,
                                Key::Tab => MenuKey::Tab,
                                Key::Escape => MenuKey::Escape,
                                _ => MenuKey::Other,
                            };
                            if let Some(action) = menu_action(key, event.is_composing()) {
                                event.prevent_default();
                                match action {
                                    MenuAction::Move(delta) => selected.set(move_selection(selected(), count, delta)),
                                    MenuAction::Pick => pick(selected()),
                                    MenuAction::Close => dismissed.set(true),
                                }
                                return;
                            }
                        }
                        // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                        if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                            event.prevent_default();
                            submit();
                        }
                    }
                },
            }
            div { class: "composer-row",
                {children}
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
