use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{preview_text, rewind_rows};
use forge_shared::{tr, Select};

use crate::workspace::use_workspace;

/// What `되감기` puts back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restore {
    CodeAndConversation,
    ConversationOnly,
    CodeOnly,
}

impl Restore {
    fn value(self) -> &'static str {
        match self {
            Restore::CodeAndConversation => "both",
            Restore::ConversationOnly => "conversation",
            Restore::CodeOnly => "code",
        }
    }

    fn from_value(value: &str) -> Self {
        match value {
            "conversation" => Restore::ConversationOnly,
            "code" => Restore::CodeOnly,
            _ => Restore::CodeAndConversation,
        }
    }

    fn files(self) -> bool {
        self != Restore::ConversationOnly
    }

    fn conversation(self) -> bool {
        self != Restore::CodeOnly
    }
}

/// `/rewind`: pick an earlier message, then put back its files, the conversation before it, or both.
/// The conversation goes back in the same session (its transcript is cut; the original is kept
/// beside it) and the message returns to the message box.
#[component]
pub fn RewindDialog(id: LiveSessionId, open: Signal<bool>) -> Element {
    let workspace = use_workspace();
    let mut open = open;
    let mut picked = use_signal(|| None::<String>);
    let mut restore = use_signal(|| Restore::CodeAndConversation);
    // What was asked, while its files are being put back: the conversation follows.
    let mut working = use_signal(|| None::<Restore>);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };

    let mut close = move || {
        if let Some(handle) = workspace.handle(id) {
            handle.close_rewind();
        }
        picked.set(None);
        working.set(None);
        restore.set(Restore::CodeAndConversation);
        open.set(false);
    };

    // A message row's ↺ opens the dialog on that message.
    use_effect(move || {
        if let Some(uuid) = workspace.take_rewind_request(id) {
            picked.set(Some(uuid.clone()));
            open.set(true);
            if let Some(handle) = workspace.handle(id) {
                handle.rewind_files(uuid, true);
            }
        }
    });

    // The files are back: now the conversation, if it was asked for. A refusal stays on screen.
    use_effect(move || {
        let snapshot = view.read();
        let answer = snapshot.rewind.as_ref().filter(|state| !state.dry_run).and_then(|state| state.outcome.clone());
        drop(snapshot);
        let Some(outcome) = answer else { return };
        let asked = *working.peek();
        let Some(asked) = asked else { return };
        if !outcome.can_rewind {
            working.set(None);
            return;
        }
        if asked.conversation() {
            if let Some(uuid) = picked.peek().clone() {
                working.set(None);
                workspace.rewind_conversation(id, uuid, move |ok| {
                    if ok {
                        close();
                    }
                });
            }
        } else {
            close();
        }
    });

    if !open() {
        return rsx! {};
    }
    let snapshot = view.read().clone();
    let running = snapshot.phase == Phase::Running;
    let rows = rewind_rows(&snapshot.session.messages);
    let state = snapshot.rewind.clone().filter(|state| Some(&state.message_uuid) == picked.read().as_ref());
    let label = picked.read().as_ref().and_then(|uuid| rows.iter().find(|row| &row.uuid == uuid)).map(|row| row.label.clone());
    let preview = state.as_ref().filter(|s| s.dry_run).and_then(|s| s.outcome.clone());
    let files_to_restore = preview.as_ref().is_some_and(|outcome| outcome.can_rewind && !outcome.files.is_empty());
    let chosen = restore();
    // Code alone needs files to put back; the conversation can go back whatever the files say.
    let can_apply = !running && working().is_none() && snapshot.claude_session_id.is_some() && (chosen != Restore::CodeOnly || files_to_restore);
    let failed_apply = state.as_ref().filter(|s| !s.dry_run).and_then(|s| s.outcome.clone()).filter(|o| !o.can_rewind);

    rsx! {
        div { class: "picker-backdrop", onclick: move |_| close() }
        div { class: "card-modal rewind-dialog", role: "dialog",
            if let Some(label) = label {
                div { class: "card-modal-head",
                    span { class: "card-modal-title", {tr!("Go back to this message?")} }
                    button { class: "card-remove always", r#type: "button", "aria-label": tr!("Close"), onclick: move |_| close(), "×" }
                }
                div { class: "rewind-picked", "{label}" }
                div { class: "card-modal-note", {tr!("Restore")} }
                Select {
                    value: chosen.value().to_string(),
                    options: vec![
                        (Restore::CodeAndConversation.value().to_string(), tr!("Files and conversation").to_string()),
                        (Restore::ConversationOnly.value().to_string(), tr!("Conversation only").to_string()),
                        (Restore::CodeOnly.value().to_string(), tr!("Files only").to_string()),
                    ],
                    disabled: working().is_some(),
                    onchange: move |value: String| restore.set(Restore::from_value(&value)),
                }
                if chosen.files() {
                    div { class: "rewind-preview",
                        match preview.as_ref() {
                            None => rsx! { {tr!("Checking the files…")} },
                            Some(outcome) => rsx! { "{preview_text(outcome)}" },
                        }
                    }
                    div { class: "card-modal-note", {tr!("Changes made by shell commands are not undone. Commits stay as they are.")} }
                }
                if chosen.conversation() {
                    div { class: "card-modal-note", {tr!("The conversation goes back to before this message, in this session. The message returns to the message box. The earlier transcript is kept beside it as a backup.")} }
                }
                if running {
                    div { class: "card-modal-note", {tr!("Wait for Claude to finish, or stop it first.")} }
                }
                if let Some(outcome) = failed_apply.as_ref() {
                    div { class: "rewind-preview", "{preview_text(outcome)}" }
                }
                div { class: "rewind-actions",
                    button { class: "rewind-button", r#type: "button", onclick: move |_| picked.set(None), {tr!("Back")} }
                    span { class: "spacer" }
                    button { class: "rewind-button", r#type: "button", onclick: move |_| close(), {tr!("Cancel")} }
                    button {
                        class: "rewind-button primary",
                        r#type: "button",
                        disabled: !can_apply,
                        onclick: move |_| {
                            let Some(uuid) = picked.peek().clone() else { return };
                            let chosen = restore();
                            if chosen.files() && files_to_restore {
                                working.set(Some(chosen));
                                if let Some(handle) = workspace.handle(id) {
                                    handle.rewind_files(uuid, false);
                                }
                            } else if chosen.conversation() {
                                workspace.rewind_conversation(id, uuid, move |ok| {
                                    if ok {
                                        close();
                                    }
                                });
                            }
                        },
                        {tr!("Rewind")}
                    }
                }
            } else {
                div { class: "card-modal-head",
                    span { class: "card-modal-title", {tr!("Go back to an earlier point")} }
                    button { class: "card-remove always", r#type: "button", "aria-label": tr!("Close"), onclick: move |_| close(), "×" }
                }
                div { class: "card-modal-note", {tr!("Pick the message to go back to.")} }
                if rows.is_empty() {
                    div { class: "card-modal-note", {tr!("There is no earlier message yet.")} }
                }
                for row in rows.iter().rev() {
                    button {
                        key: "{row.uuid}",
                        class: "rewind-row",
                        r#type: "button",
                        onclick: {
                            let uuid = row.uuid.clone();
                            move |_| {
                                picked.set(Some(uuid.clone()));
                                if let Some(handle) = workspace.handle(id) {
                                    handle.rewind_files(uuid.clone(), true);
                                }
                            }
                        },
                        "{row.label}"
                    }
                }
            }
        }
    }
}
