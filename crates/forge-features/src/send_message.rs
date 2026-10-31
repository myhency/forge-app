use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{split_cards, SlashCommand};
use forge_entities::{arrange_commands, btw_question, is_rewind, local_command};
use forge_shared::tr;

use crate::cards::line_count;
use crate::composer_cards::ComposerCards;
use crate::composer_shell::{composer_action, ComposerShell};
use crate::composer_state::{use_composer_state_provider, ComposerChips, TrustCard};
use crate::suggestion::{composer_placeholder, end_card_reply, suggestion_to_accept};

use crate::command_box::CommandTextArea;
use crate::new_session::send_ready;
use crate::queued_card::QueuedCard;
use crate::rewind_dialog::RewindDialog;
use crate::workspace::use_workspace;

// Commands Forge answers itself, listed with the SDK's.
fn local_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand {
            name: "btw".to_string(),
            description: Some(tr!("Ask a quick question without adding it to the session").to_string()),
            argument_hint: Some(tr!("[question]").to_string()),
        },
        SlashCommand { name: "rewind".to_string(), description: Some(tr!("Go back to an earlier point").to_string()), argument_hint: None },
        SlashCommand { name: "tasks".to_string(), description: Some(tr!("Show the background tasks").to_string()), argument_hint: None },
        SlashCommand { name: "diff".to_string(), description: Some(tr!("Show or hide the changes").to_string()), argument_hint: None },
        SlashCommand { name: "status".to_string(), description: Some(tr!("Show this session's model, folder and connectors").to_string()), argument_hint: None },
        SlashCommand { name: "stats".to_string(), description: Some(tr!("Show your use of Claude across all sessions").to_string()), argument_hint: None },
        SlashCommand { name: "usage".to_string(), description: Some(tr!("Show what this session used and the usage limit").to_string()), argument_hint: None },
    ]
}

/// The message box of an open session: Enter sends, Shift+Enter adds a line, and while a turn is
/// running a Stop button interrupts it.
#[component]
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool, overlay: Element, leading: Element, end: Element) -> Element {
    let workspace = use_workspace();
    let draft = use_signal(String::new);
    let composer = use_composer_state_provider(draft);
    // The `@` mention lists this session's folder.
    use_effect(use_reactive!(|id| composer.set_mention_session(id)));
    let mut draft = draft;
    let mut seen_failures = use_signal(|| 0u32);
    let mut rewinding = use_signal(|| false);
    let view = workspace.live_view(id);
    // A live next-step card is the suggestion: the grey text gives way to it.
    let end_card = view
        .and_then(|view| forge_entities::offered(forge_entities::live_end_card(&view.read().session.messages), &workspace.end_card_marks(id)))
        .filter(|_| !running);
    let suggestion = if end_card.is_some() { None } else { view.and_then(|view| view.read().suggestion.clone()) };
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
            workspace.clear_sent_marks(id);
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

    let mut submit_or_queue = move |queue: bool| {
        if workspace.session_is_moving(id) {
            return;
        }
        if is_rewind(&draft.read()) {
            rewinding.set(true);
            draft.set(String::new());
            return;
        }
        // `/tasks`, `/diff`, `/status` and `/usage` are answered here: they are not sent as messages.
        let local = local_command(&draft.read());
        if let Some(command) = local {
            workspace.run_local_command(id, command);
            draft.set(String::new());
            return;
        }
        // `/btw` with nothing after it asks nothing: it is neither sent nor cleared.
        if forge_entities::is_bare_btw(&draft.read()) {
            return;
        }
        // `/btw question` is asked on the side: it is not sent as a message.
        let side = btw_question(&draft.read()).map(str::to_string);
        if let Some(question) = side {
            workspace.ask_side(id, question);
            draft.set(String::new());
            return;
        }
        let cards = workspace.take_cards(id, line_count);
        let Some(outgoing) = composer.take_for_send(&draft.read(), &cards) else {
            // Refused (a chip could not be read): the cards stay where they were.
            workspace.restore_cards(id, cards);
            return;
        };
        if let Some(handle) = workspace.handle(id) {
            // The message goes out: the cards of `/status` and `/usage` no longer belong to the end of the chat.
            workspace.clear_local_cards(id);
            if queue {
                handle.queue(outgoing.text, outgoing.attachments);
            } else {
                handle.send_with(outgoing.text, outgoing.attachments);
            }
            draft.set(String::new());
        }
    };
    let mut submit = move || submit_or_queue(false);
    let mut queue = move || submit_or_queue(true);
    let stop = move |_| {
        if let Some(handle) = workspace.handle(id) {
            handle.interrupt();
        }
    };

    let ready = send_ready(&draft.read(), composer.attachments().len());
    rsx! {
        ComposerShell {
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
            top: rsx! {
                if let Some(message) = composer.error() {
                    div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
                }
                ComposerCards { id }
                RewindDialog { id, open: rewinding }
                QueuedCard { id, draft }
                ComposerChips {}
                TrustCard {}
            },
            overlay,
            leading,
            input: rsx! {
                CommandTextArea {
                    value: draft(),
                    commands: arrange_commands(&view.map(|view| view.read().commands.clone()).unwrap_or_default(), local_commands()),
                    placeholder: composer_placeholder(suggestion.as_deref(), draft.read().is_empty(), running, if running { tr!("⏎ to send, ⌘⏎ to queue and send later") } else { tr!("Message Claude…") }),
                    disabled: !can_send,
                    open_token: composer.popup_token(),
                    oninput: move |text| draft.set(text),
                    onsubmit: move |_| submit(),
                    onqueue: {
                        let end_card = end_card.clone();
                        move |_| {
                            // ⌘⏎ on an empty box with a live card presses the card's first button.
                            if let Some(message) = end_card_reply(end_card.as_ref(), draft.peek().is_empty(), running) {
                                if !workspace.session_is_moving(id) {
                                    if let Some(handle) = workspace.handle(id) {
                                        if let Some(card) = end_card.as_ref() {
                                            workspace.mark_end_card(id, card.id.clone(), forge_entities::CardMark::Sent);
                                        }
                                        handle.send_with(message, Vec::new());
                                    }
                                }
                                return;
                            }
                            queue()
                        }
                    },
                }
            },
            action: composer_action(running, ready, !can_send),
            onsend: move |_| submit(),
            onstop: stop,
            start: rsx! {},
            end,
        }
    }
}
