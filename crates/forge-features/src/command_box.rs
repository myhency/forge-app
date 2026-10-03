use dioxus::prelude::*;
use forge_domain::SlashCommand;
use forge_entities::{insert_text, menu_action, menu_rows, move_selection, typed_after_change, MenuAction, MenuKey};
use forge_shared::TextArea;

/// A text box that lists slash commands while its text is just `/name`. The parent owns the text:
/// picking a command calls `oninput` with `/name `. Enter (without Shift) calls `onsubmit` unless the
/// popup is open, in which case it picks.
#[component]
pub fn CommandTextArea(
    value: String,
    commands: Vec<SlashCommand>,
    #[props(default)] placeholder: String,
    #[props(default)] disabled: bool,
    /// Changes when something outside asks for the command list (the `+` menu's "Slash commands").
    #[props(default)] open_token: u32,
    oninput: EventHandler<String>,
    onsubmit: EventHandler<()>,
) -> Element {
    let mut selected = use_signal(|| 0usize);
    let mut dismissed = use_signal(|| false);
    // What the person last typed. Text that arrives another way (a failed send put back, a picked
    // command) does not open the popup.
    let mut typed = use_signal(|| None::<String>);
    // Forgotten as soon as the text changes without a keystroke: a pick, a send that clears the box,
    // a failed send put back. Without this the put-back text would equal the old `typed` and reopen
    // the popup.
    use_effect(use_reactive!(|value| {
        let kept = typed_after_change(typed.peek().as_deref(), &value);
        if kept != *typed.peek() {
            typed.set(kept);
        }
    }));

    // The `+` menu's "Slash commands" row put `/` in the text and asks for the list: treat the text
    // as typed, once per token. The first token is the starting value and opens nothing.
    let mut seen_token = use_signal(|| open_token);
    use_effect(use_reactive!(|open_token, value| {
        if open_token != *seen_token.peek() {
            seen_token.set(open_token);
            typed.set(Some(value.clone()));
            dismissed.set(false);
        }
    }));

    let matches: Vec<SlashCommand> = menu_rows(&commands, &value, typed.read().as_deref(), dismissed() || disabled)
        .into_iter()
        .cloned()
        .collect();
    let count = matches.len();
    // The stored selection can be out of range after the list shrinks; the row shown and the row
    // picked are always this one.
    let current = move_selection(selected(), count, 0);

    let pick = {
        let matches = matches.clone();
        move |index: usize| {
            if let Some(command) = matches.get(index) {
                oninput.call(insert_text(command));
                selected.set(0);
            }
        }
    };

    rsx! {
        div { class: "command-box",
            if count > 0 {
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
                value: value.clone(),
                placeholder,
                disabled,
                oninput: move |text: String| {
                    typed.set(Some(text.clone()));
                    selected.set(0);
                    dismissed.set(false);
                    oninput.call(text);
                },
                onkeydown: {
                    let mut pick = pick.clone();
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
                            if let Some(action) = menu_action(key, event.is_composing(), event.modifiers().shift()) {
                                event.prevent_default();
                                match action {
                                    MenuAction::Move(delta) => selected.set(move_selection(current, count, delta)),
                                    MenuAction::Pick => pick(current),
                                    MenuAction::Close => dismissed.set(true),
                                }
                                return;
                            }
                        }
                        // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                        if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                            event.prevent_default();
                            onsubmit.call(());
                        }
                    }
                },
            }
        }
    }
}
