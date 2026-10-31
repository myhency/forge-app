use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{EffortLevel, PermissionMode};
use forge_entities::{model_menu_rows, more_model_rows};
use forge_features::{
    confirmation_still_valid, effort_button_label, effort_levels, menu_digit_row, mode_choice, mode_rows, refocus_composer, slider_position, use_settings, use_workspace, ModeChoice, PickerKind,
    effort_help, slider_faster, slider_smarter,
};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};

/// One row of a picker's list, whatever it picks.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct PickerItem {
    pub(crate) label: String,
    pub(crate) description: Option<String>,
    pub(crate) key: Option<usize>,
    pub(crate) checked: bool,
    pub(crate) divider_before: bool,
    /// A small tag after the label ("Default", "Credits needed").
    pub(crate) badge: Option<String>,
}

/// Opens the menu when a shortcut asked for the menu of `kind` (⇧⌘M, ⇧⌘I, ⇧⌘E).
pub(crate) fn use_picker_request(kind: Option<PickerKind>, mut open: Signal<bool>) {
    let workspace = use_workspace();
    use_effect(move || {
        if kind.is_some() && workspace.picker_request() == kind {
            workspace.clear_picker_request();
            open.set(true);
        }
    });
}

/// A button with the current choice that opens a list above it. Number keys pick the rows that
/// have one, Escape or a click outside closes it. `onpick` gets the index of the row.
#[component]
pub(crate) fn Picker(
    title: String,
    current: String,
    items: Vec<PickerItem>,
    onpick: EventHandler<usize>,
    #[props(default)] heading: Option<String>,
    #[props(default)] kind: Option<PickerKind>,
    #[props(default)] icon: Option<IconKind>,
    /// Rows for a flyout that opens beside the list under the row `more_label` ("More models"); `onpick_more` gets the index
    /// in this list. The flyout is not drawn when the list is empty.
    #[props(default)] more: Vec<PickerItem>,
    #[props(default)] more_label: Option<String>,
    #[props(default)] onpick_more: Option<EventHandler<usize>>,
) -> Element {
    let mut open = use_signal(|| false);
    let mut more_open = use_signal(|| false);
    use_picker_request(kind, open);
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
                if let Some(icon) = icon {
                    Icon { kind: icon }
                }
                span { "{current}" }
                span { class: "picker-chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true } }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        refocus_composer();
                    },
                }
                div { class: "picker-menu picker-menu-wrap",
                div {
                    class: "picker-list",
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
                    div { class: "picker-title", "{heading.clone().unwrap_or_else(|| title.clone())}" }
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
                                span { class: "picker-check", if item.checked { Icon { kind: IconKind::Check, size: IconSize::Small } } }
                                span { class: "picker-text",
                                    span { class: "picker-label",
                                        "{item.label}"
                                        if let Some(badge) = item.badge.as_ref() {
                                            span { class: "picker-badge", "{badge}" }
                                        }
                                    }
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
                    if let (Some(label), false) = (more_label.clone(), more.is_empty()) {
                        div { class: "picker-more",
                            div { class: "picker-divider" }
                            button {
                                class: if more_open() { "picker-item picker-more-row open" } else { "picker-item picker-more-row" },
                                r#type: "button",
                                role: "menuitem",
                                "aria-haspopup": "menu",
                                "aria-expanded": "{more_open()}",
                                onclick: move |_| more_open.set(!more_open()),
                                span { class: "picker-text", span { class: "picker-label", "{label}" } }
                                span { class: "picker-more-chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                            }
                            if more_open() {
                                div { class: "picker-flyout", role: "menu",
                                    for (index, item) in more.iter().enumerate() {
                                        button {
                                            key: "{index}",
                                            class: if item.checked { "picker-item checked" } else { "picker-item" },
                                            r#type: "button",
                                            role: "menuitemradio",
                                            "aria-checked": "{item.checked}",
                                            onclick: move |_| {
                                                open.set(false);
                                                more_open.set(false);
                                                if let Some(pick) = onpick_more {
                                                    pick.call(index);
                                                }
                                            },
                                            span { class: "picker-check", if item.checked { Icon { kind: IconKind::Check, size: IconSize::Small } } }
                                            span { class: "picker-text",
                                                span { class: "picker-label",
                                                    "{item.label}"
                                                    if let Some(badge) = item.badge.as_ref() {
                                                        span { class: "picker-badge", "{badge}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The permission mode picker. Bypass asks first. `onchange` gets the mode once it is confirmed.
#[component]
pub(crate) fn ModePickerView(current: PermissionMode, onchange: EventHandler<PermissionMode>) -> Element {
    // The mode waiting for its confirmation.
    let mut confirming = use_signal(|| None::<PermissionMode>);
    let settings = use_settings();
    // A session already in bypass mode keeps its row, selected, even when the switch is off.
    let rows = mode_rows(settings.allow_bypass_mode() || current == PermissionMode::BypassPermissions);
    let items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem {
            label: row.label.to_string(),
            description: Some(row.description.to_string()),
            key: row.key,
            checked: row.mode == current,
            divider_before: row.divider_before,
            ..Default::default()
        })
        .collect();
    let label = rows.iter().find(|row| row.mode == current).map_or(tr!("Mode"), |row| row.label).to_string();
    let rows_for_pick = rows.clone();
    rsx! {
        Picker {
            title: tr!("Mode"),
            kind: PickerKind::Mode,
            current: label,
            items,
            onpick: move |index: usize| {
                let mode = rows_for_pick[index].mode;
                match mode_choice(mode) {
                    ModeChoice::Apply => {
                        onchange.call(mode);
                        refocus_composer();
                    }
                    ModeChoice::AskFirst => confirming.set(Some(mode)),
                }
            },
        }
        if let Some(pending) = confirming().filter(|mode| confirmation_still_valid(&rows, *mode)) {
            div { class: "picker-confirm", role: "alertdialog",
                span { {tr!("Bypass permissions skips every permission check in this session.")} }
                Button {
                    label: tr!("Cancel"),
                    onclick: move |_| {
                        confirming.set(None);
                        refocus_composer();
                    },
                }
                Button {
                    label: tr!("Enable"),
                    kind: ButtonKind::Danger,
                    onclick: move |_| {
                        confirming.set(None);
                        onchange.call(pending);
                        refocus_composer();
                    },
                }
            }
        }
    }
}

/// The permission mode of the open session, changed from the composer.
#[component]
pub fn ModePicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().session.permission_mode;
    rsx! { ModePickerView { current, onchange: move |mode| workspace.set_permission_mode(id, mode) } }
}

/// The model picker. `current` is the model in force (`None` is the SDK's default); `onchange` gets
/// the row's value.
#[component]
pub(crate) fn ModelPickerView(current: Option<String>, onchange: EventHandler<Option<String>>) -> Element {
    let rows = model_menu_rows(current.as_deref());
    let item_of = |row: &forge_entities::ModelRow, current: &Option<String>| PickerItem {
        label: row.label.clone(),
        description: None,
        key: row.key,
        checked: row.value == *current,
        divider_before: row.divider_before,
        badge: row.badge.map(|badge| forge_shared::i18n::text(badge).to_string()),
    };
    let items: Vec<PickerItem> = rows.iter().map(|row| item_of(row, &current)).collect();
    let more_rows = more_model_rows();
    let more: Vec<PickerItem> = more_rows.iter().map(|row| item_of(row, &current)).collect();
    // The button names the model in force, wherever it is listed.
    let label = rows
        .iter()
        .chain(more_rows.iter())
        .find(|row| row.value == current)
        .map_or_else(|| tr!("Model").to_string(), |row| row.label.clone());
    let values: Vec<Option<String>> = rows.iter().map(|row| row.value.clone()).collect();
    let more_values: Vec<Option<String>> = more_rows.iter().map(|row| row.value.clone()).collect();
    rsx! {
        Picker {
            title: tr!("Model"),
            kind: PickerKind::Model,
            current: label,
            items,
            onpick: move |index: usize| {
                onchange.call(values[index].clone());
                refocus_composer();
            },
            more,
            more_label: tr!("More models").to_string(),
            onpick_more: move |index: usize| {
                onchange.call(more_values[index].clone());
                refocus_composer();
            },
        }
    }
}

/// The model of the open session, changed from the composer.
#[component]
pub fn ModelPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().model.clone();
    rsx! { ModelPickerView { current, onchange: move |model| workspace.set_model(id, model) } }
}

/// The effort of the open session, changed from the composer with a stepped slider. Only the levels
/// the session's model takes get a stop; a model that takes none, or that the SDK has not listed,
/// shows no control.
#[component]
pub fn EffortPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (levels, current) = {
        let view = view.read();
        (effort_levels(&view.available_models, view.model.as_deref()), view.effort)
    };
    rsx! { EffortPickerView { levels, current, onchange: move |level| workspace.set_effort(id, level) } }
}

/// The effort button and its slider menu, for any owner of an effort: `levels` are the stops (nothing
/// is drawn when empty), `onchange` gets a level, or `None` for "Use the default".
#[component]
pub fn EffortPickerView(levels: Vec<EffortLevel>, current: Option<EffortLevel>, onchange: EventHandler<Option<EffortLevel>>) -> Element {
    let mut open = use_signal(|| false);
    use_picker_request(Some(PickerKind::Effort), open);
    if levels.is_empty() {
        return rsx! {};
    }
    let position = slider_position(&levels, current);
    let last_stop = levels.len() - 1;
    let label = effort_button_label(current);
    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: tr!("Effort"),
                "aria-haspopup": "dialog",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                span { "{label}" }
                span { class: "picker-chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true } }
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
                    class: "picker-menu effort-menu",
                    role: "dialog",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            refocus_composer();
                        }
                    },
                    div { class: "effort-head",
                        span { class: "picker-title", {tr!("Effort")} }
                        span { class: "effort-current", "{label}" }
                        span { class: "effort-help", title: "{effort_help()}", "?" }
                    }
                    div { class: "effort-ends",
                        span { {slider_faster()} }
                        span { {slider_smarter()} }
                    }
                    input {
                        class: "effort-slider",
                        r#type: "range",
                        min: "0",
                        max: "{last_stop}",
                        step: "1",
                        value: "{position}",
                        "aria-label": tr!("Effort"),
                        // `onchange`, not `oninput`: it fires when the thumb is released (and once per arrow
                        // key), so a drag sends one change and the SDK's answers cannot arrive out of order.
                        onchange: {
                            let levels = levels.clone();
                            move |event: FormEvent| {
                                if let Some(level) = event.value().parse::<usize>().ok().and_then(|index| levels.get(index)) {
                                    onchange.call(Some(*level));
                                }
                            }
                        },
                    }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        onclick: move |_| onchange.call(None),
                        span { class: "picker-text", span { class: "picker-label", {tr!("Use the default")} } }
                    }
                }
            }
        }
    }
}
