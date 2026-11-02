use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{EffortLevel, PermissionMode};
use forge_entities::{model_menu_rows, more_model_rows, ModelRow};
use forge_features::{
    confirmation_still_valid, effort_after_model_change, effort_button_label, effort_help, effort_levels, menu_digit_row, mode_choice, mode_rows, refocus_composer, use_settings, use_workspace, ModeChoice, PickerKind,
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
    /// Shown greyed and not pressable (the bypass row while the switch in Settings is off).
    pub(crate) disabled: bool,
    /// A heading drawn above the row (after its divider), for a list in sections.
    pub(crate) section: Option<String>,
}

/// A row below the list that opens a flyout beside the menu ("Effort", "More models").
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Flyout {
    pub(crate) label: String,
    /// The current choice, shown at the right of the row.
    pub(crate) value: Option<String>,
    /// A sentence above the rows.
    pub(crate) note: Option<String>,
    pub(crate) items: Vec<PickerItem>,
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
    /// Rows below the list that open a flyout beside it; `onpick_flyout` gets `(flyout index, row index)`. A flyout
    /// without rows is not drawn.
    #[props(default)] flyouts: Vec<Flyout>,
    #[props(default)] onpick_flyout: Option<EventHandler<(usize, usize)>>,
    /// A second shortcut that opens this menu with flyout `index` already open (⇧⌘E opens the effort flyout).
    #[props(default)] flyout_kind: Option<(PickerKind, usize)>,
    /// A fainter word after the current choice (the effort after the model's name).
    #[props(default)] current_dim: Option<String>,
    /// A search field above the list with this placeholder; it filters the rows by their label.
    #[props(default)] search: Option<String>,
) -> Element {
    let mut query = use_signal(String::new);
    let mut open = use_signal(|| false);
    let mut flyout_open = use_signal(|| None::<usize>);
    use_picker_request(kind, open);
    let workspace = use_workspace();
    use_effect(move || {
        if let Some((wanted, index)) = flyout_kind {
            if workspace.picker_request() == Some(wanted) {
                workspace.clear_picker_request();
                open.set(true);
                flyout_open.set(Some(index));
            }
        }
    });
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
                if let Some(dim) = current_dim.as_ref() {
                    span { class: "picker-dim", "{dim}" }
                }
                span { class: "picker-chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true } }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        flyout_open.set(None);
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
                    if let Some(placeholder) = search.as_ref() {
                        input {
                            class: "env-search",
                            r#type: "text",
                            placeholder: "{placeholder}",
                            value: "{query}",
                            // A letter typed here is not a menu key.
                            onkeydown: move |event: KeyboardEvent| {
                                if event.key() != Key::Escape {
                                    event.stop_propagation();
                                }
                            },
                            oninput: move |event| query.set(event.value()),
                        }
                    }
                    if search.is_none() {
                        div { class: "picker-title", "{heading.clone().unwrap_or_else(|| title.clone())}" }
                    }
                    for (index, item) in items.iter().enumerate().filter(|(_, item)| query.read().trim().is_empty() || item.label.to_lowercase().contains(&query.read().trim().to_lowercase())) {
                        div { key: "{index}",
                            if item.divider_before {
                                div { class: "picker-divider" }
                            }
                            if let Some(section) = item.section.as_ref() {
                                div { class: "picker-title", "{section}" }
                            }
                            button {
                                class: if item.checked { "picker-item checked" } else { "picker-item" },
                                role: "menuitemradio",
                                "aria-checked": "{item.checked}",
                                disabled: item.disabled,
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
                    for (flyout_index, flyout) in flyouts.iter().enumerate().filter(|(_, flyout)| !flyout.items.is_empty()) {
                        div { class: "picker-more", key: "flyout-{flyout_index}",
                            div { class: "picker-divider" }
                            button {
                                class: if flyout_open() == Some(flyout_index) { "picker-item picker-more-row open" } else { "picker-item picker-more-row" },
                                r#type: "button",
                                role: "menuitem",
                                "aria-haspopup": "menu",
                                "aria-expanded": "{flyout_open() == Some(flyout_index)}",
                                onclick: move |_| flyout_open.set(if flyout_open() == Some(flyout_index) { None } else { Some(flyout_index) }),
                                span { class: "picker-text", span { class: "picker-label", "{flyout.label}" } }
                                if let Some(value) = flyout.value.as_ref() {
                                    span { class: "picker-more-value", "{value}" }
                                }
                                span { class: "picker-more-chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                            }
                            if flyout_open() == Some(flyout_index) {
                                div { class: "picker-flyout", role: "menu",
                                    if let Some(note) = flyout.note.as_ref() {
                                        div { class: "picker-note", "{note}" }
                                    }
                                    for (index, item) in flyout.items.iter().enumerate() {
                                        button {
                                            key: "{index}",
                                            class: if item.checked { "picker-item checked" } else { "picker-item" },
                                            r#type: "button",
                                            role: "menuitemradio",
                                            "aria-checked": "{item.checked}",
                                            onclick: move |_| {
                                                open.set(false);
                                                flyout_open.set(None);
                                                if let Some(pick) = onpick_flyout {
                                                    pick.call((flyout_index, index));
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
    let bypass_on = settings.allow_bypass_mode() || current == PermissionMode::BypassPermissions;
    let rows = mode_rows(bypass_on);
    let mut items: Vec<PickerItem> = rows
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
    // The desktop app keeps the bypass row in the list, greyed, and says where to turn it on.
    if !bypass_on {
        items.push(PickerItem {
            label: tr!("Bypass permissions").to_string(),
            description: Some(tr!("Turn on in Settings").to_string()),
            disabled: true,
            ..Default::default()
        });
    }
    let label = rows.iter().find(|row| row.mode == current).map_or(tr!("Mode"), |row| row.label).to_string();
    let rows_for_pick = rows.clone();
    rsx! {
        Picker {
            title: tr!("Mode"),
            heading: tr!("Permissions · this session"),
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

/// The model button of the composer, with the effort next to it and a menu like the desktop app's: the models, an
/// "Effort" row with a flyout of the levels, and "More models" with a flyout of the older models. `current` is the
/// model in force (`None` is the SDK's default, which is the row tagged `Default`); `levels` are the efforts the
/// model takes (the Effort row is left out when empty).
#[component]
pub(crate) fn ModelPickerView(
    current: Option<String>,
    levels: Vec<EffortLevel>,
    effort: Option<EffortLevel>,
    onmodel: EventHandler<Option<String>>,
    oneffort: EventHandler<Option<EffortLevel>>,
) -> Element {
    let rows = model_menu_rows(current.as_deref());
    let is_current = |row: &ModelRow| row.value == current || (current.is_none() && row.is_default);
    let item_of = |row: &ModelRow| PickerItem {
        label: row.label.clone(),
        description: None,
        key: row.key,
        checked: is_current(row),
        divider_before: row.divider_before,
        badge: row.badge.map(|badge| forge_shared::i18n::text(badge).to_string()),
        ..Default::default()
    };
    let items: Vec<PickerItem> = rows.iter().map(item_of).collect();
    let more_rows = more_model_rows();
    let more_items: Vec<PickerItem> = more_rows.iter().map(item_of).collect();
    // The button names the model in force, wherever it is listed.
    let label = rows.iter().chain(more_rows.iter()).find(|row| is_current(row)).map_or_else(|| tr!("Model").to_string(), |row| row.label.clone());
    let values: Vec<Option<String>> = rows.iter().map(|row| row.value.clone()).collect();
    let more_values: Vec<Option<String>> = more_rows.iter().map(|row| row.value.clone()).collect();

    let effort_items: Vec<PickerItem> = levels
        .iter()
        .map(|level| PickerItem {
            label: forge_shared::i18n::text(level.label()).to_string(),
            checked: Some(*level) == effort,
            badge: (*level == EffortLevel::Medium).then(|| tr!("Recommended").to_string()),
            ..Default::default()
        })
        .collect();
    let flyouts = vec![
        Flyout { label: tr!("Effort").to_string(), value: effort.map(|_| effort_button_label(effort)), note: Some(effort_help().to_string()), items: effort_items },
        Flyout { label: tr!("More models").to_string(), value: None, note: None, items: more_items },
    ];
    rsx! {
        Picker {
            title: tr!("Model"),
            heading: tr!("Model · this session"),
            kind: PickerKind::Model,
            flyout_kind: (PickerKind::Effort, 0),
            current: label,
            current_dim: effort.map(|_| effort_button_label(effort)),
            items,
            onpick: move |index: usize| {
                onmodel.call(values[index].clone());
                refocus_composer();
            },
            flyouts,
            onpick_flyout: move |(flyout, index): (usize, usize)| {
                if flyout == 0 {
                    oneffort.call(levels.get(index).copied());
                } else {
                    onmodel.call(more_values[index].clone());
                }
                refocus_composer();
            },
        }
    }
}

/// The model and effort of the open session, changed from the composer.
#[component]
pub fn ModelPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (current, levels, effort) = {
        let view = view.read();
        (view.model.clone(), effort_levels(&view.available_models, view.model.as_deref()), view.effort)
    };
    rsx! {
        ModelPickerView {
            current,
            levels,
            effort,
            onmodel: move |model: Option<String>| {
                // The new model may not take the level the session has.
                let (effort, levels) = {
                    let view = view.read();
                    (view.effort, effort_levels(&view.available_models, model.as_deref()))
                };
                let adjusted = effort_after_model_change(effort, &levels);
                workspace.set_model(id, model);
                if adjusted != effort {
                    workspace.set_effort(id, adjusted);
                }
            },
            oneffort: move |level| workspace.set_effort(id, level),
        }
    }
}
