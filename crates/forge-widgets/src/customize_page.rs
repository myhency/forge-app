use dioxus::prelude::*;
use forge_features::{use_customize, use_settings, use_workspace, CustomizeMode, CustomizeTab, Dialog, SettingsSection};
use forge_shared::{Button, ButtonKind};

use crate::customize_dialogs::CustomizeDialogs;
use crate::customize_plugins::PluginsTab;
use crate::customize_skills::SkillsTab;

const TABS: [(CustomizeTab, &str); 3] =
    [(CustomizeTab::Skills, "Skills"), (CustomizeTab::Connectors, "Connectors"), (CustomizeTab::Plugins, "Plugins")];

/// The Customize page (spec 2026-10-08): tabs, the Mine / Explore switch, search and the Add menu.
/// Plugins and Skills are built; Connectors points at Settings until its own plan.
#[component]
pub fn CustomizePage() -> Element {
    let customize = use_customize();
    let workspace = use_workspace();
    use_hook(move || customize.open_page(workspace.context_project()));
    let tab = customize.tab();
    let mode = customize.mode();
    let mut add_menu = use_signal(|| false);

    rsx! {
        div { class: "customize",
            h1 { class: "customize-title", "Customize" }
            div { class: "customize-bar",
                div { class: "customize-tabs",
                    for (value, label) in TABS {
                        button {
                            key: "{label}",
                            class: if tab == value { "customize-tab active" } else { "customize-tab" },
                            onclick: move |_| customize.set_tab(value),
                            "{label}"
                        }
                    }
                }
                if tab == CustomizeTab::Plugins || tab == CustomizeTab::Skills {
                    div { class: "customize-switch",
                        button {
                            class: if mode == CustomizeMode::Mine { "active" } else { "" },
                            onclick: move |_| customize.set_mode(CustomizeMode::Mine),
                            "Mine"
                        }
                        button {
                            class: if mode == CustomizeMode::Explore { "active" } else { "" },
                            onclick: move |_| customize.set_mode(CustomizeMode::Explore),
                            "Explore"
                        }
                    }
                    input {
                        class: "customize-search",
                        r#type: "text",
                        placeholder: "Search",
                        value: "{customize.query()}",
                        oninput: move |event| customize.set_query(event.value()),
                    }
                    div { class: "customize-add",
                        Button { label: "+ Add", kind: ButtonKind::Primary, disabled: customize.skills_busy(), onclick: move |_| add_menu.set(!add_menu()) }
                        if add_menu() {
                            div { class: "menu-backdrop", onclick: move |_| add_menu.set(false) }
                            div { class: "customize-menu",
                                if tab == CustomizeTab::Skills {
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.upload_skill_folder();
                                        },
                                        "Upload skill folder"
                                    }
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.upload_skill_zip();
                                        },
                                        "Upload skill zip"
                                    }
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.open_dialog(Dialog::CreateSkill);
                                        },
                                        "Create skill"
                                    }
                                } else {
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.open_dialog(Dialog::AddMarketplace);
                                        },
                                        "Add marketplace"
                                    }
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.open_dialog(Dialog::ManageMarketplaces);
                                        },
                                        "Manage marketplaces"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if let Some(message) = customize.error() {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| customize.dismiss_error() }
                }
            }
            match tab {
                CustomizeTab::Plugins => rsx! { PluginsTab {} },
                CustomizeTab::Skills => rsx! { SkillsTab {} },
                CustomizeTab::Connectors => rsx! { SettingsNote { what: "Connectors", section: SettingsSection::McpServers } },
            }
            CustomizeDialogs {}
        }
    }
}

// Until the Skills and Connectors tabs are built, their switches are in Settings.
#[component]
fn SettingsNote(what: String, section: SettingsSection) -> Element {
    let settings = use_settings();
    rsx! {
        div { class: "customize-note",
            p { "{what} are switched on and off in Settings for now." }
            Button {
                label: "Open in Settings",
                onclick: move |_| {
                    settings.open();
                    settings.set_section(section);
                },
            }
        }
    }
}
