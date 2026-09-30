use dioxus::prelude::*;
use forge_entities::{model_from_value, model_options, model_value};
use forge_features::{use_settings, ExtensionKind, SettingsSection, SettingsStatus};
use forge_shared::{Button, ButtonKind, Select, SettingRow, Switch, TextInput};

/// The settings overlay (spec §16): a left navigation and rows on the right. Renders nothing while
/// closed. Esc and ⌘, are handled by the page's key handler.
#[component]
pub fn SettingsOverlay() -> Element {
    let settings = use_settings();
    if !settings.is_open() {
        return rsx! {};
    }
    let section = settings.section();

    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| settings.close(),
            div {
                class: "settings-panel",
                // A click inside the panel must not reach the backdrop and close it.
                onclick: move |event| event.stop_propagation(),
                button { class: "settings-close", title: "Close (Esc)", onclick: move |_| settings.close(), "✕" }
                div { class: "settings-nav",
                    div { class: "settings-nav-heading", "Settings" }
                    button {
                        class: if section == SettingsSection::Connection { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Connection),
                        "Connection"
                    }
                    button {
                        class: if section == SettingsSection::Model { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Model),
                        "Model"
                    }
                    button {
                        class: if section == SettingsSection::Skills { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Skills),
                        "Skills"
                    }
                    button {
                        class: if section == SettingsSection::Plugins { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::Plugins),
                        "Plugins"
                    }
                    button {
                        class: if section == SettingsSection::McpServers { "settings-nav-item active" } else { "settings-nav-item" },
                        onclick: move |_| settings.set_section(SettingsSection::McpServers),
                        "MCP servers"
                    }
                }
                div { class: "settings-content",
                    match section {
                        SettingsSection::Connection => rsx! { ConnectionSection {} },
                        SettingsSection::Model => rsx! { ModelSection {} },
                        // Temporary: Task 8 renders these tabs.
                        SettingsSection::Skills => rsx! { ExtensionSection { kind: ExtensionKind::Skill } },
                        SettingsSection::Plugins => rsx! { ExtensionSection { kind: ExtensionKind::Plugin } },
                        SettingsSection::McpServers => rsx! { ExtensionSection { kind: ExtensionKind::McpServer } },
                    }
                    StatusLine {}
                }
            }
        }
    }
}

#[component]
fn ConnectionSection() -> Element {
    let settings = use_settings();
    let key_label = if settings.key_visible() { "Hide" } else { "Show" };
    rsx! {
        h2 { "Connection" }
        SettingRow { title: "Base URL", description: "Leave empty for https://api.anthropic.com.",
            TextInput {
                value: settings.base_url(),
                placeholder: "https://api.anthropic.com",
                oninput: move |value| settings.set_base_url(value),
            }
        }
        SettingRow { title: "API key", description: "Stored in Forge's own config file, never in ~/.claude.",
            TextInput {
                value: settings.api_key(),
                placeholder: "sk-ant-…",
                secret: !settings.key_visible(),
                oninput: move |value| settings.set_api_key(value),
            }
            Button { label: key_label, onclick: move |_| settings.toggle_key_visible() }
        }
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep the old values until they are started again."
        }
        div { style: "margin-top: 16px;",
            Button {
                label: "Save",
                kind: ButtonKind::Primary,
                disabled: !settings.connection_dirty(),
                onclick: move |_| settings.save_connection(),
            }
        }
    }
}

#[component]
fn ModelSection() -> Element {
    let settings = use_settings();
    let mut custom = use_signal(String::new);
    let saved = settings.model();
    let options = model_options(saved.as_deref());
    let selected = model_value(saved.as_deref());
    let custom_empty = custom.read().trim().is_empty();
    rsx! {
        h2 { "Model" }
        SettingRow { title: "Default model", description: "New sessions start with this model. A choice is saved at once.",
            Select {
                value: selected,
                options,
                onchange: move |value: String| settings.choose_model(model_from_value(&value)),
            }
        }
        SettingRow { title: "Other model ID", description: "Use an ID that is not in the list.",
            TextInput {
                value: custom(),
                placeholder: "model-id",
                oninput: move |value| custom.set(value),
            }
            Button {
                label: "Use",
                disabled: custom_empty,
                onclick: move |_| {
                    settings.choose_model(custom.read().clone());
                    custom.set(String::new());
                },
            }
        }
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep their model; a change applies to the next new session."
        }
    }
}

#[component]
fn StatusLine() -> Element {
    let settings = use_settings();
    match settings.status() {
        Some(SettingsStatus::Saved) => rsx! { div { class: "status-line", "Saved." } },
        Some(SettingsStatus::Error(message)) => rsx! { div { class: "status-line error", "{message}" } },
        None => rsx! { div { class: "status-line" } },
    }
}

/// One of the Skills / Plugins / MCP servers tabs: a row per definition with a switch that saves at once.
#[component]
fn ExtensionSection(kind: ExtensionKind) -> Element {
    let settings = use_settings();
    let (heading, tab, scoped) = match kind {
        ExtensionKind::Skill => ("Skills", settings.skill_tab(), true),
        ExtensionKind::Plugin => ("Plugins", settings.plugin_tab(), false),
        ExtensionKind::McpServer => ("MCP servers", settings.mcp_tab(), true),
    };
    let body = match tab {
        Err(message) => rsx! { div { class: "status-line error", "{message}" } },
        Ok(rows) if rows.is_empty() => rsx! { div { class: "hint", "Nothing found." } },
        Ok(rows) => rsx! {
            for row in rows {
                SettingRow { key: "{row.key}", title: row.title.clone(), description: row.detail.clone(),
                    Switch {
                        on: row.enabled,
                        onchange: {
                            let key = row.key.clone();
                            move |_| settings.toggle(kind, key.clone())
                        },
                    }
                }
            }
        },
    };
    rsx! {
        h2 { "{heading}" }
        if scoped {
            div { class: "hint", "Showing what is available for {settings.project_label()}." }
        }
        {body}
        div { class: "hint", style: "margin-top: 12px;",
            "Sessions that are already open keep what they started with; a change applies to the next new session."
        }
        div { class: "hint",
            "Forge sessions do not use the hooks or permission rules from ~/.claude/settings.json; project settings still apply."
        }
    }
}
