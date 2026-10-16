use dioxus::prelude::*;
use forge_domain::{EffortLevel, PermissionMode};

use crate::command_box::CommandTextArea;
use crate::composer_state::{use_composer_state_provider, ComposerChips, TrustCard};
use crate::workspace::use_workspace;
use forge_shared::tr;

/// What was chosen for the new session before it exists: `None` follows the saved default model,
/// `Some(None)` is the SDK's default, `Some(Some(id))` is that model.
pub type ModelChoice = Option<Option<String>>;

/// The mode, model and effort picked on the new-session screen. Nothing is saved to settings.
/// An effort of `None` is the SDK's default.
#[derive(Clone, Copy)]
pub struct NewSessionDraft {
    pub mode: Signal<PermissionMode>,
    pub model: Signal<ModelChoice>,
    pub effort: Signal<Option<EffortLevel>>,
}

pub fn use_new_session_draft() -> NewSessionDraft {
    use_context::<NewSessionDraft>()
}

/// Whether the send button is ready: there is text or something attached.
pub fn send_ready(text: &str, attachments: usize) -> bool {
    !text.trim().is_empty() || attachments > 0
}

/// The new-session panel: the folder and other chips (`chips`), the first message, and a row of controls
/// (`children`). The session is only created when that message is sent.
#[component]
pub fn NewSessionComposer(chips: Element, children: Element) -> Element {
    let workspace = use_workspace();
    let draft = use_context_provider(|| NewSessionDraft { mode: Signal::new(PermissionMode::Default), model: Signal::new(None), effort: Signal::new(None) });
    let text = use_signal(String::new);
    let composer = use_composer_state_provider(text);
    let mut text = text;

    // The probe follows the chosen folder: it is replaced when the folder changes, kept when it does
    // not, and ended when this panel goes away (send, another session, the settings).
    use_effect(move || {
        let folder = workspace.chosen_new_folder();
        spawn(async move { workspace.probe_new_folder(folder) });
    });
    use_drop(move || workspace.probe_new_folder(None));

    let has_folder = !workspace.new_session_projects().is_empty();

    // `submit` is used by two handlers, so it may capture only `Copy` values (signals): it looks the
    // folder up again instead of capturing a string.
    let mut submit = move || {
        if !send_ready(&text.read(), composer.attachments().len()) {
            return;
        }
        let projects = workspace.new_session_projects();
        let project = match workspace.new_folder() {
            Some(path) => projects.iter().find(|project| project.path == path).cloned().or_else(|| projects.first().cloned()),
            None => projects.first().cloned(),
        };
        let Some(project) = project else {
            return;
        };
        let Some(outgoing) = composer.take_for_new_session(&text.read()) else {
            return;
        };
        workspace.start_new(
            project,
            *draft.mode.peek(),
            draft.model.peek().clone(),
            *draft.effort.peek(),
            outgoing.text,
            outgoing.attachments,
            composer.take_folders(),
        );
        text.set(String::new());
    };
    let ready = send_ready(&text.read(), composer.attachments().len());

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", {tr!("No existing project folder was found in your Claude Code history. Use “Open folder…” to choose one.")} }
            }
            div { class: "env-chips", {chips} }
            if let Some(message) = composer.error() {
                div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
            }
            ComposerChips {}
            TrustCard {}
            div { class: "composer-box",
                CommandTextArea {
                    value: text(),
                    open_token: composer.popup_token(),
                    commands: workspace.new_commands(),
                    placeholder: tr!("Describe a task or ask a question"),
                    disabled: !has_folder,
                    oninput: move |value| text.set(value),
                    onsubmit: move |_| submit(),
                }
                button {
                    class: if ready { "send-icon ready" } else { "send-icon" },
                    r#type: "button",
                    title: tr!("Send"),
                    disabled: !has_folder || !ready,
                    onclick: move |_| submit(),
                    "↵"
                }
            }
            div { class: "composer-row", {children} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_button_is_ready_with_text_or_an_attachment() {
        assert!(!send_ready("", 0));
        assert!(!send_ready("  \n ", 0));
        assert!(send_ready("hi", 0));
        assert!(send_ready("", 1));
    }
}
