use dioxus::prelude::*;
use forge_features::{folder_menu_rows, refocus_composer, use_new_session_draft, use_settings, use_workspace};

use crate::composer_controls::{ModePickerView, ModelPickerView, Picker, PickerItem};

/// The folder chip: the folder's name, and a menu of the recent folders with "Open folder…" below.
#[component]
pub fn FolderChip() -> Element {
    let workspace = use_workspace();
    let projects = workspace.new_session_projects();
    let current = workspace.chosen_new_folder();
    let rows = folder_menu_rows(&projects, current.as_deref());
    let mut items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem { label: row.label.clone(), description: None, key: None, checked: row.checked, divider_before: false })
        .collect();
    items.push(PickerItem { label: "Open folder…".to_string(), description: None, key: None, checked: false, divider_before: true });
    let current_label = rows.iter().find(|row| row.checked).map_or_else(|| "Choose a folder".to_string(), |row| row.label.clone());
    let paths: Vec<_> = rows.iter().map(|row| row.path.clone()).collect();
    rsx! {
        Picker {
            title: "Folder",
            heading: "Recent",
            current: current_label,
            items,
            onpick: move |index: usize| {
                match paths.get(index) {
                    Some(path) => workspace.set_new_folder(Some(path.clone())),
                    None => workspace.open_folder(),
                }
                refocus_composer();
            },
        }
    }
}

/// A button that opens the native folder dialog, like "Open folder…" in the menu.
#[component]
pub fn AddFolderButton() -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "chip-button",
            r#type: "button",
            title: "Open folder",
            disabled: workspace.picking_folder(),
            onclick: move |_| workspace.open_folder(),
            "📁+"
        }
    }
}

/// The mode for the session about to start.
#[component]
pub fn NewModePicker() -> Element {
    let mut draft = use_new_session_draft();
    let current = *draft.mode.read();
    rsx! { ModePickerView { current, onchange: move |mode| draft.mode.set(mode) } }
}

/// The model for the session about to start; until one is picked it is the saved default.
#[component]
pub fn NewModelPicker() -> Element {
    let mut draft = use_new_session_draft();
    let settings = use_settings();
    let current = match draft.model.read().clone() {
        Some(chosen) => chosen,
        None => settings.model(),
    };
    rsx! {
        div { class: "picker-end",
            ModelPickerView { current, onchange: move |model| draft.model.set(Some(model)) }
        }
    }
}
