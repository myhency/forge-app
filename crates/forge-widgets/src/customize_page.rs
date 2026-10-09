use dioxus::prelude::*;
use forge_features::{use_customize, use_workspace, CustomizeMode, CustomizeTab, Dialog};
use forge_shared::{Button, ButtonKind};

use crate::customize_connectors::{ConnectorNotes, ConnectorsTab};
use crate::customize_dialogs::CustomizeDialogs;
use crate::customize_plugins::PluginsTab;
use crate::customize_skills::SkillsTab;

const TABS: [(CustomizeTab, &str); 3] =
    [(CustomizeTab::Skills, "Skills"), (CustomizeTab::Connectors, "Connectors"), (CustomizeTab::Plugins, "Plugins")];

/// The Customize page (spec 2026-10-08): tabs, the Mine / Explore switch, search and the Add menu.
/// Plugins, Skills and Connectors are built.
#[component]
pub fn CustomizePage() -> Element {
    let customize = use_customize();
    let workspace = use_workspace();
    use_hook(move || customize.open_page(workspace.context_project()));
    // Coming back to the window (from the terminal, say) reads the connectors again. One listener at a time: a
    // visit that replaces another takes its place.
    use_future(move || async move {
        let mut focus = document::eval(
            "if (window.__forgeFocus) window.removeEventListener('focus', window.__forgeFocus); \
             window.__forgeFocus = () => dioxus.send(1); \
             window.addEventListener('focus', window.__forgeFocus); \
             await new Promise(() => {});",
        );
        while focus.recv::<i32>().await.is_ok() {
            customize.refresh_connectors();
        }
    });
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
                    Button { label: "+ Add", kind: ButtonKind::Primary, disabled: customize.skills_busy() || customize.connectors_busy(), onclick: move |_| add_menu.set(!add_menu()) }
                    if add_menu() {
                        div { class: "menu-backdrop", onclick: move |_| add_menu.set(false) }
                        div { class: "customize-menu",
                            if tab == CustomizeTab::Connectors {
                                button {
                                    onclick: move |_| {
                                        add_menu.set(false);
                                        customize.open_dialog(Dialog::AddConnector);
                                    },
                                    "Add connector"
                                }
                            } else if tab == CustomizeTab::Skills {
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
            if let Some(message) = customize.error() {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| customize.dismiss_error() }
                }
            }
            if tab == CustomizeTab::Connectors {
                ConnectorNotes {}
            }
            match tab {
                CustomizeTab::Plugins => rsx! { PluginsTab {} },
                CustomizeTab::Skills => rsx! { SkillsTab {} },
                CustomizeTab::Connectors => rsx! { ConnectorsTab {} },
            }
            CustomizeDialogs {}
        }
    }
}
