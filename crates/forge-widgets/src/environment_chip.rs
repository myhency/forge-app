use dioxus::prelude::*;
use forge_domain::Environment;
use forge_features::{environment_menu, numbered_ids, picker_choices, refocus_composer, use_new_session_draft, use_workspace, EnvRow, EnvironmentKey, EnvironmentRequest};
use forge_shared::tr;

/// The chip at the left of the new-session chips: the environment the session starts from, and a menu
/// of this computer and the saved environments. ⌥⌘1…9 pick a row, ⌥⌘0 makes a new one.
#[component]
pub fn EnvironmentChip() -> Element {
    let workspace = use_workspace();
    let mut draft = use_new_session_draft();
    let mut open = use_signal(|| false);
    let mut more = use_signal(|| false);
    let mut query = use_signal(String::new);
    let environments = workspace.environments();
    let this_computer = tr!("This computer").to_string();
    let picked: Option<Environment> = draft.environment.read().clone().and_then(|id| environments.iter().find(|environment| environment.id == id).cloned());
    let picked_id = picked.as_ref().map(|environment| environment.id.clone());
    let label = picked.as_ref().map_or_else(|| this_computer.clone(), |environment| environment.name.clone());

    let mut pick = move |id: Option<String>| {
        // This computer leaves the pickers as they are. Before the SDK lists models, the environment's own model is taken as it is.
        if let Some(environment) = id.as_ref().and_then(|id| workspace.environments().into_iter().find(|environment| &environment.id == id)) {
            let mut known: Vec<String> = workspace.new_models().into_iter().flat_map(|model| std::iter::once(model.value).chain(model.resolved_model)).collect();
            if known.is_empty() {
                known.extend(environment.model.clone());
            }
            let (model, effort) = picker_choices(&environment, &draft.model.peek(), *draft.effort.peek(), &known);
            draft.model.set(model);
            draft.effort.set(effort);
        }
        draft.environment.set(id);
        open.set(false);
        more.set(false);
        query.set(String::new());
        refocus_composer();
    };

    // The gear: close the menu like the other ways into the dialog, then open the edit dialog.
    let mut edit = move |id: String| {
        open.set(false);
        more.set(false);
        query.set(String::new());
        workspace.open_environment_dialog(EnvironmentRequest::Edit(id));
    };

    // The keys ⌥⌘0…9 land here.
    use_effect(move || {
        let Some(key) = workspace.environment_key() else { return };
        workspace.clear_environment_key();
        match key {
            EnvironmentKey::Pick(index) => {
                if let Some(id) = numbered_ids(&workspace.environments()).get(index).cloned() {
                    pick(id);
                }
            }
            EnvironmentKey::New => workspace.open_environment_dialog(EnvironmentRequest::New { folder: workspace.chosen_new_folder() }),
        }
    });

    let menu = environment_menu(&environments, &query.read(), &this_computer);
    let searching = !query.read().trim().is_empty();
    rsx! {
        div { class: "picker env-chip",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: tr!("Environment"),
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                span { "{label}" }
                span { class: "picker-chevron", "▾" }
            }
            if open() {
                div {
                    class: "picker-backdrop",
                    onclick: move |_| {
                        open.set(false);
                        more.set(false);
                        refocus_composer();
                    },
                }
                div {
                    class: "picker-menu env-menu",
                    role: "menu",
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            open.set(false);
                            more.set(false);
                            refocus_composer();
                        }
                    },
                    input {
                        class: "env-search",
                        r#type: "text",
                        placeholder: tr!("Search environments…"),
                        value: "{query}",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| query.set(event.value()),
                    }
                    if !searching {
                        div { class: "picker-title", {tr!("Local")} }
                    }
                    for row in menu.visible.iter().cloned() {
                        EnvironmentRow { key: "{row.id:?}", row: row.clone(), checked: row.id == picked_id, onpick: move |id: Option<String>| pick(id), onedit: move |id: String| edit(id) }
                    }
                    if !menu.more.is_empty() {
                        button {
                            class: "picker-item env-more",
                            r#type: "button",
                            onclick: move |_| more.set(!more()),
                            span { class: "picker-check" }
                            span { class: "picker-text", span { class: "picker-label", {tr!("More")} } }
                            span { class: "menu-key", "{menu.more.len()} ›" }
                        }
                        if more() {
                            for row in menu.more.iter().cloned() {
                                EnvironmentRow { key: "{row.id:?}", row: row.clone(), checked: row.id == picked_id, onpick: move |id: Option<String>| pick(id), onedit: move |id: String| edit(id) }
                            }
                        }
                    }
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        onclick: move |_| {
                            open.set(false);
                            workspace.open_environment_dialog(EnvironmentRequest::New { folder: workspace.chosen_new_folder() });
                        },
                        span { class: "picker-check", "+" }
                        span { class: "picker-text", span { class: "picker-label", {tr!("New environment")} } }
                        span { class: "menu-key", {new_key_label()} }
                    }
                }
            }
        }
    }
}

fn new_key_label() -> &'static str {
    if cfg!(target_os = "macos") { "⌥⌘0" } else { "Ctrl+Alt+0" }
}

fn number_label(number: usize) -> String {
    if cfg!(target_os = "macos") { format!("⌥⌘{number}") } else { format!("Ctrl+Alt+{number}") }
}

#[component]
fn EnvironmentRow(row: EnvRow, checked: bool, onpick: EventHandler<Option<String>>, onedit: EventHandler<String>) -> Element {
    let workspace = use_workspace();
    let id = row.id.clone();
    let edit_id = row.id.clone();
    let star_id = row.id.clone();
    let pinned = row.pinned;
    rsx! {
        div { class: if checked { "env-row checked" } else { "env-row" },
            button {
                class: if checked { "picker-item checked" } else { "picker-item" },
                r#type: "button",
                role: "menuitemradio",
                "aria-checked": "{checked}",
                onclick: move |_| onpick.call(id.clone()),
                span { class: "picker-check", if checked { "✓" } }
                span { class: "picker-text", span { class: "picker-label", "{row.name}" } }
                if let Some(number) = row.number {
                    span { class: "menu-key", {number_label(number)} }
                }
            }
            if let Some(edit_id) = edit_id {
                span { class: "env-row-icons",
                    button {
                        class: "env-icon",
                        r#type: "button",
                        title: tr!("Edit environment"),
                        onclick: move |_| onedit.call(edit_id.clone()),
                        "⚙"
                    }
                    button {
                        class: if pinned { "env-icon pinned" } else { "env-icon" },
                        r#type: "button",
                        title: if pinned { tr!("Unpin") } else { tr!("Pin") },
                        onclick: move |_| {
                            if let Some(star_id) = star_id.clone() {
                                workspace.set_environment_pinned(star_id, !pinned);
                            }
                        },
                        if pinned { "★" } else { "☆" }
                    }
                }
            }
        }
    }
}
