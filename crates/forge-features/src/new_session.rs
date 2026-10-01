use dioxus::prelude::*;
use forge_entities::model_label;
use forge_shared::{Button, ButtonKind, Select, TextArea};

use crate::permission_mode::{mode_options, parse_mode};
use crate::settings::use_settings;
use crate::workspace::use_workspace;

/// The new-session panel: pick a folder and permission mode, type the first message. The session
/// is only created when that message is sent.
#[component]
pub fn NewSessionComposer() -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
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
    // The chosen folder if it is still offered, otherwise the first one.
    let selected = workspace
        .new_folder()
        .map(|path| path.to_string_lossy().to_string())
        .filter(|path| options.iter().any(|(value, _)| value == path))
        .or_else(|| options.first().map(|(value, _)| value.clone()))
        .unwrap_or_default();
    let model = model_label(settings.model().as_deref());

    // `submit` is used by two handlers, so it may capture only `Copy` values (signals): it looks the
    // folder up again instead of capturing the `selected` string.
    let mut submit = move || {
        let body = text.read().trim().to_string();
        if body.is_empty() {
            return;
        }
        let projects = workspace.new_session_projects();
        let project = match workspace.new_folder() {
            Some(path) => projects
                .iter()
                .find(|project| project.path == path)
                .cloned()
                .or_else(|| projects.first().cloned()),
            None => projects.first().cloned(),
        };
        let Some(project) = project else {
            return;
        };
        workspace.start_new(project, parse_mode(&mode.read()), body);
        text.set(String::new());
    };
    let has_folder = !options.is_empty();

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", "No existing project folder was found in your Claude Code history. Use “Open folder…” to choose one." }
            }
            div { class: "field",
                label { "Folder" }
                Select {
                    value: selected,
                    options,
                    disabled: !has_folder,
                    onchange: move |path: String| workspace.set_new_folder(Some(path.into())),
                }
                Button {
                    label: "Open folder…",
                    disabled: workspace.picking_folder(),
                    onclick: move |_| workspace.open_folder(),
                }
            }
            div { class: "field",
                label { "Model" }
                span { class: "field-value", "{model}" }
                span { class: "hint", "Change it in Settings (⌘,)." }
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
