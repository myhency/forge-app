use dioxus::prelude::*;
use forge_domain::SlashCommand;
use forge_entities::{insert_text, menu_action, menu_rows, move_selection, typed_after_change, MenuAction, MenuKey};
use forge_shared::TextArea;

use crate::composer_state::ComposerState;
use crate::mention_popup::{key_decision, KeyDecision};

// The text area's text and cursor; no cursor while text is selected.
const READ_CURSOR: &str = "const a = document.activeElement; \
    return (a && a.tagName === 'TEXTAREA') ? [a.value, a.selectionStart === a.selectionEnd ? a.selectionStart : null] : null;";

// Fits the box to its text, one line up to the height the stylesheet allows.
const GROW_TEXTAREA: &str = "document.querySelectorAll('.composer .textarea').forEach((t) => { t.style.height = '0'; t.style.height = t.scrollHeight + 'px'; });";

// Scrolls the highlighted command into view.
const SCROLL_SELECTED: &str = "const row = document.querySelector('.command-item.selected'); if (row) row.scrollIntoView({ block: 'nearest' });";

fn menu_key(key: &Key) -> MenuKey {
    match key {
        Key::ArrowUp => MenuKey::Up,
        Key::ArrowDown => MenuKey::Down,
        Key::Enter => MenuKey::Enter,
        Key::Tab => MenuKey::Tab,
        Key::Escape => MenuKey::Escape,
        _ => MenuKey::Other,
    }
}

// Keys after which the cursor may stand somewhere else without the text changing.
fn moves_cursor(key: &Key) -> bool {
    matches!(key, Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown | Key::Home | Key::End | Key::PageUp | Key::PageDown)
}

// Reads where the cursor is, for the `@` mention of the composer around this text box (the page
// owns the cursor; Dioxus only sees the text).
fn read_cursor(composer: Option<ComposerState>) {
    let Some(composer) = composer else { return };
    let Some(read) = composer.begin_cursor_read() else { return };
    spawn(async move {
        if let Ok(Some((text, cursor))) = document::eval(READ_CURSOR).join::<Option<(String, Option<usize>)>>().await {
            composer.note_cursor(read, text, cursor);
        }
    });
}

/// A text box that lists slash commands while its text is just `/name`. The parent owns the text:
/// picking a command calls `oninput` with `/name `. Enter (without Shift) calls `onsubmit` unless the
/// popup is open, in which case it picks. Inside an open session's composer an `@` at the cursor
/// opens the composer's file mention list, which then takes the keys (and Enter never sends).
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
    /// ⌘Enter: send later. Without a handler it sends like Enter.
    onqueue: Option<EventHandler<()>>,
) -> Element {
    let composer = try_use_context::<ComposerState>();
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

    // The box grows with the text, and shrinks again when the text is sent or cleared.
    use_effect(use_reactive!(|value| {
        let _ = &value;
        spawn(async move {
            let _ = document::eval(GROW_TEXTAREA).await;
        });
    }));

    // Keeps the selected row in view when the list scrolls.
    use_effect(use_reactive!(|current, count| {
        if count > 0 {
            let _ = current;
            spawn(async move {
                let _ = document::eval(SCROLL_SELECTED).await;
            });
        }
    }));

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
        div {
            class: "command-box",
            onkeyup: move |event: KeyboardEvent| {
                if moves_cursor(&event.key()) {
                    read_cursor(composer);
                }
            },
            onmouseup: move |_| read_cursor(composer),
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
                    if let Some(composer) = composer {
                        composer.note_typed(&text);
                    }
                    oninput.call(text);
                    read_cursor(composer);
                },
                onkeydown: {
                    let mut pick = pick.clone();
                    move |event: KeyboardEvent| {
                        let key = menu_key(&event.key());
                        let (composing, shift) = (event.is_composing(), event.modifiers().shift());
                        if let Some(composer) = composer.filter(ComposerState::mentions_on) {
                            if let KeyDecision::Mention(action) = composer.key_decision(key, composing, shift) {
                                event.prevent_default();
                                event.stop_propagation();
                                composer.mention_act(action);
                                return;
                            }
                        }
                        if count > 0 {
                            if let Some(action) = menu_action(key, composing, shift) {
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
                        if key_decision(false, key, composing, shift) == KeyDecision::Send {
                            event.prevent_default();
                            match onqueue {
                                Some(onqueue) if crate::command_held(event.modifiers()) => onqueue.call(()),
                                _ => onsubmit.call(()),
                            }
                        }
                    }
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cursor_read_gives_no_cursor_while_text_is_selected() {
        assert!(READ_CURSOR.contains("a.selectionStart === a.selectionEnd ? a.selectionStart : null"), "{READ_CURSOR}");
        assert!(READ_CURSOR.contains("a.tagName === 'TEXTAREA'"));
    }

    #[test]
    fn the_selected_row_is_scrolled_into_view() {
        assert!(SCROLL_SELECTED.contains(".command-item.selected") && SCROLL_SELECTED.contains("scrollIntoView"));
    }

    #[test]
    fn only_moving_keys_read_the_cursor_again() {
        assert!(moves_cursor(&Key::ArrowLeft) && moves_cursor(&Key::End));
        assert!(!moves_cursor(&Key::Enter) && !moves_cursor(&Key::Character("a".into())));
    }

    #[test]
    fn the_menu_keys_are_the_arrows_enter_tab_and_escape() {
        assert_eq!(menu_key(&Key::ArrowDown), MenuKey::Down);
        assert_eq!(menu_key(&Key::Tab), MenuKey::Tab);
        assert_eq!(menu_key(&Key::ArrowLeft), MenuKey::Other);
    }
}
