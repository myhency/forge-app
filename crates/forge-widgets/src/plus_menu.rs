use std::path::PathBuf;

use dioxus::prelude::*;
use forge_entities::ExtensionRow;
use forge_features::{menu_digit_row, refocus_composer, use_composer_state, use_settings, ExtensionKind};
use forge_shared::tr;

/// What a row of the `+` menu does when picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlusAction {
    AddFiles,
    AddFolder,
    SlashCommands,
    Connectors,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlusRow {
    pub label: &'static str,
    pub action: PlusAction,
    pub shortcut: Option<&'static str>,
    pub submenu: bool,
}

pub fn plus_rows() -> [PlusRow; 5] {
    [
        PlusRow { label: tr!("Add file or photo"), action: PlusAction::AddFiles, shortcut: Some(forge_domain::KEY_ADD_FILES), submenu: false },
        PlusRow { label: tr!("Add folder"), action: PlusAction::AddFolder, shortcut: None, submenu: false },
        PlusRow { label: tr!("Slash commands"), action: PlusAction::SlashCommands, shortcut: None, submenu: false },
        PlusRow { label: tr!("Connectors"), action: PlusAction::Connectors, shortcut: None, submenu: true },
        PlusRow { label: tr!("Plugins"), action: PlusAction::Plugins, shortcut: None, submenu: true },
    ]
}

/// The `+` button at the left of the composer and the menu it opens. `project` is the folder whose
/// connectors the submenu lists (`None` before a folder is chosen). `add_folder_enabled` greys the
/// "Add folder" row.
#[component]
pub fn PlusMenu(project: Option<PathBuf>, add_folder_enabled: bool) -> Element {
    let composer = use_composer_state();
    let settings = use_settings();
    let mut open = use_signal(|| false);
    let mut submenu = use_signal(|| None::<PlusAction>);
    let rows = plus_rows();

    let mut close = move || {
        open.set(false);
        submenu.set(None);
        refocus_composer();
    };
    let pick = {
        let project = project.clone();
        move |action: PlusAction| match action {
            PlusAction::AddFiles => {
                open.set(false);
                composer.add_files();
            }
            PlusAction::AddFolder => {
                open.set(false);
                composer.add_folder();
            }
            PlusAction::SlashCommands => {
                open.set(false);
                composer.insert_slash();
                refocus_composer();
            }
            PlusAction::Connectors | PlusAction::Plugins => {
                if let Some(project) = project.clone() {
                    settings.prepare_menu(project);
                }
                submenu.set(Some(action));
            }
        }
    };

    rsx! {
        div { class: "picker",
            button {
                class: if open() { "picker-button open" } else { "picker-button" },
                r#type: "button",
                title: tr!("Add"),
                "aria-label": "Add",
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| {
                    if open() {
                        open.set(false);
                        submenu.set(None);
                    } else {
                        open.set(true);
                    }
                },
                "+"
            }
            if open() {
                div { class: "picker-backdrop", onclick: move |_| close() }
                div {
                    class: "picker-menu",
                    role: "menu",
                    tabindex: "0",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: {
                        let mut pick = pick.clone();
                        move |event: KeyboardEvent| {
                            if event.key() == Key::Escape {
                                event.stop_propagation();
                                if submenu().is_some() {
                                    submenu.set(None);
                                } else {
                                    close();
                                }
                                return;
                            }
                            // The physical key, because the character depends on the input method.
                            let Some(row) = menu_digit_row(&event.code().to_string()) else { return };
                            let modified =
                                event.modifiers().intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                            if modified {
                                return;
                            }
                            let Some(chosen) = plus_rows().get(row).copied() else { return };
                            if chosen.action == PlusAction::AddFolder && !add_folder_enabled {
                                return;
                            }
                            event.stop_propagation();
                            event.prevent_default();
                            pick(chosen.action);
                        }
                    },
                    for (index, row) in rows.iter().enumerate() {
                        {
                            let disabled = row.action == PlusAction::AddFolder && !add_folder_enabled;
                            let action = row.action;
                            let mut pick = pick.clone();
                            rsx! {
                                button {
                                    key: "{index}",
                                    class: if disabled { "picker-item disabled" } else { "picker-item" },
                                    role: "menuitem",
                                    disabled,
                                    title: if disabled { tr!("Choose folders before the first message") } else { "" },
                                    onclick: move |_| pick(action),
                                    span { class: "picker-text",
                                        span { class: "picker-label", "{row.label}" }
                                    }
                                    if let Some(shortcut) = row.shortcut {
                                        span { class: "menu-key", "{shortcut}" }
                                    }
                                    if row.submenu {
                                        span { class: "menu-key", "▸" }
                                    }
                                }
                            }
                        }
                    }
                    if let Some(kind) = submenu() {
                        PlusSubmenu { kind }
                    }
                }
            }
        }
    }
}

/// The connectors or plugins that Settings knows, with a check on the enabled ones. A click
/// switches one in Settings; a running session is not changed.
#[component]
fn PlusSubmenu(kind: PlusAction) -> Element {
    let settings = use_settings();
    let (title, extension, listing): (&str, ExtensionKind, Result<Vec<ExtensionRow>, String>) = match kind {
        PlusAction::Plugins => (tr!("Plugins"), ExtensionKind::Plugin, settings.plugin_tab()),
        _ => (tr!("Connectors"), ExtensionKind::McpServer, settings.connector_rows()),
    };
    rsx! {
        div { class: "picker-menu picker-submenu", role: "menu",
            div { class: "picker-title", "{title}" }
            match listing {
                Err(message) => rsx! { div { class: "picker-item disabled", "{message}" } },
                Ok(rows) if rows.is_empty() => rsx! { div { class: "picker-item disabled", {tr!("None installed")} } },
                Ok(rows) => rsx! {
                    for row in rows {
                        button {
                            key: "{row.key}",
                            class: if row.enabled { "picker-item checked" } else { "picker-item" },
                            role: "menuitemcheckbox",
                            "aria-checked": "{row.enabled}",
                            onclick: {
                                let key = row.key.clone();
                                move |_| settings.toggle(extension, key.clone())
                            },
                            span { class: "picker-check", if row.enabled { "✓" } }
                            span { class: "picker-text",
                                span { class: "picker-label", "{row.title}" }
                                if !row.detail.is_empty() {
                                    span { class: "picker-description", "{row.detail}" }
                                }
                            }
                        }
                    }
                },
            }
            div { class: "picker-title", {tr!("Applies to the next new session.")} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rows_are_the_five_the_desktop_app_shows_in_its_order() {
        let labels: Vec<&str> = plus_rows().iter().map(|row| row.label).collect();
        assert_eq!(labels, ["Add file or photo", "Add folder", "Slash commands", "Connectors", "Plugins"]);
    }

    #[test]
    fn only_connectors_and_plugins_open_a_submenu() {
        let with_submenu: Vec<PlusAction> = plus_rows().iter().filter(|r| r.submenu).map(|r| r.action).collect();
        assert_eq!(with_submenu, [PlusAction::Connectors, PlusAction::Plugins]);
    }

    #[test]
    fn the_first_row_shows_the_command_u_shortcut() {
        assert_eq!(plus_rows()[0].shortcut, Some(forge_domain::KEY_ADD_FILES));
    }
}
