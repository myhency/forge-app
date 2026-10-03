use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::model_menu_rows;
use forge_features::{menu_digit_row, mode_choice, mode_rows, refocus_composer, use_workspace, ModeChoice};
use forge_shared::{Button, ButtonKind};

/// One row of a picker's list, whatever it picks.
#[derive(Debug, Clone, PartialEq)]
struct PickerItem {
    label: String,
    description: Option<String>,
    key: Option<usize>,
    checked: bool,
    divider_before: bool,
}

/// A button with the current choice that opens a list above it. Number keys pick the rows that
/// have one, Escape or a click outside closes it. `onpick` gets the index of the row.
#[component]
fn Picker(title: String, current: String, items: Vec<PickerItem>, onpick: EventHandler<usize>) -> Element {
    let mut open = use_signal(|| false);
    let keyed = items.clone();
    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: "{title}",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                span { "{current}" }
                span { class: "picker-chevron", "▾" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        refocus_composer();
                    },
                }
                div {
                    class: "picker-menu",
                    role: "menu",
                    tabindex: "0",
                    // The list takes focus when it opens so its number keys work at once.
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            refocus_composer();
                            return;
                        }
                        // The physical key, because the character depends on the input method.
                        let Some(row) = menu_digit_row(&event.code().to_string()) else { return };
                        let modified = event
                            .modifiers()
                            .intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                        if modified {
                            return;
                        }
                        if let Some(index) = keyed.iter().position(|item| item.key == Some(row + 1)) {
                            event.stop_propagation();
                            event.prevent_default();
                            open.set(false);
                            onpick.call(index);
                        }
                    },
                    div { class: "picker-title", "{title}" }
                    for (index, item) in items.iter().enumerate() {
                        div { key: "{index}",
                            if item.divider_before {
                                div { class: "picker-divider" }
                            }
                            button {
                                class: if item.checked { "picker-item checked" } else { "picker-item" },
                                role: "menuitemradio",
                                "aria-checked": "{item.checked}",
                                onclick: move |_| {
                                    open.set(false);
                                    onpick.call(index);
                                },
                                span { class: "picker-check", if item.checked { "✓" } }
                                span { class: "picker-text",
                                    span { class: "picker-label", "{item.label}" }
                                    if let Some(description) = item.description.as_ref() {
                                        span { class: "picker-description", "{description}" }
                                    }
                                }
                                if let Some(key) = item.key {
                                    span { class: "menu-key", "{key}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The permission mode of the open session, changed from the composer. Bypass asks first.
#[component]
pub fn ModePicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    // The row waiting for its confirmation.
    let mut confirming = use_signal(|| None::<usize>);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().session.permission_mode;
    let rows = mode_rows();
    let items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem {
            label: row.label.to_string(),
            description: Some(row.description.to_string()),
            key: row.key,
            checked: row.mode == current,
            divider_before: row.divider_before,
        })
        .collect();
    let label = rows.iter().find(|row| row.mode == current).map_or("Mode", |row| row.label).to_string();
    let rows_for_pick = rows.clone();
    let rows_for_enable = rows.clone();
    rsx! {
        Picker {
            title: "Mode",
            current: label,
            items,
            onpick: move |index: usize| {
                let mode = rows_for_pick[index].mode;
                match mode_choice(mode) {
                    ModeChoice::Apply => {
                        workspace.set_permission_mode(id, mode);
                        refocus_composer();
                    }
                    ModeChoice::AskFirst => confirming.set(Some(index)),
                }
            },
        }
        if let Some(index) = confirming() {
            div { class: "picker-confirm", role: "alertdialog",
                span { "Bypass permissions skips every permission check in this session." }
                Button {
                    label: "Cancel",
                    onclick: move |_| {
                        confirming.set(None);
                        refocus_composer();
                    },
                }
                Button {
                    label: "Enable",
                    kind: ButtonKind::Danger,
                    onclick: move |_| {
                        confirming.set(None);
                        workspace.set_permission_mode(id, rows_for_enable[index].mode);
                        refocus_composer();
                    },
                }
            }
        }
    }
}

/// The model of the open session, changed from the composer.
#[component]
pub fn ModelPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().model.clone();
    let rows = model_menu_rows(current.as_deref());
    let items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem {
            label: row.label.clone(),
            description: None,
            key: row.key,
            checked: row.value == current,
            divider_before: row.divider_before,
        })
        .collect();
    let label = rows
        .iter()
        .find(|row| row.value == current)
        .map_or_else(|| "Model".to_string(), |row| row.label.clone());
    let values: Vec<Option<String>> = rows.iter().map(|row| row.value.clone()).collect();
    rsx! {
        Picker {
            title: "Model",
            current: label,
            items,
            onpick: move |index: usize| {
                workspace.set_model(id, values[index].clone());
                refocus_composer();
            },
        }
    }
}
