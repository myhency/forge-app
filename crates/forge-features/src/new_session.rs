use dioxus::prelude::*;
use forge_shared::{Button, ButtonKind, Select, TextArea};

use crate::permission_mode::{mode_options, parse_mode};
use crate::workspace::use_workspace;

/// The new-session panel: pick a folder and permission mode, type the first message. The session
/// is only created when that message is sent.
#[component]
pub fn NewSessionComposer() -> Element {
    let workspace = use_workspace();
    let mut folder = use_signal(|| None::<String>);
    let mut mode = use_signal(|| "default".to_string());
    let mut text = use_signal(String::new);

    let projects = workspace.new_session_projects();
    let options: Vec<(String, String)> = projects
        .iter()
        .map(|project| {
            let path = project.path.to_string_lossy().to_string();
            (path.clone(), path)
        })
        .collect();
    let selected = folder
        .read()
        .clone()
        .or_else(|| options.first().map(|(value, _)| value.clone()))
        .unwrap_or_default();

    let mut submit = move || {
        let body = text.read().trim().to_string();
        if body.is_empty() {
            return;
        }
        let projects = workspace.new_session_projects();
        let chosen = match folder.read().clone() {
            Some(path) => projects.into_iter().find(|project| project.path.to_string_lossy() == path),
            None => projects.into_iter().next(),
        };
        let Some(project) = chosen else {
            return;
        };
        workspace.start_new(project, parse_mode(&mode.read()), None, body);
        text.set(String::new());
    };
    let has_folder = !options.is_empty();

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", "No existing project folder was found in your Claude Code history." }
            }
            div { class: "field",
                label { "Folder" }
                Select {
                    value: selected,
                    options,
                    disabled: !has_folder,
                    onchange: move |path| folder.set(Some(path)),
                }
            }
            div { class: "field",
                label { "Model" }
                // Only the default model is offered: nothing lists models yet (spec §15.1).
                Select {
                    value: "default",
                    options: vec![("default".to_string(), "Default model".to_string())],
                    onchange: |_| {},
                }
            }
            div { class: "field",
                label { "Mode" }
                Select {
                    value: mode(),
                    options: mode_options(),
                    onchange: move |value| mode.set(value),
                }
            }
            TextArea {
                value: text(),
                placeholder: "What should Claude do?",
                disabled: !has_folder,
                oninput: move |value| text.set(value),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                        event.prevent_default();
                        submit();
                    }
                },
            }
            div { class: "composer-row",
                span { class: "spacer" }
                Button {
                    label: "Start",
                    kind: ButtonKind::Primary,
                    disabled: !has_folder,
                    onclick: move |_| submit(),
                }
            }
        }
    }
}
