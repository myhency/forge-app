use dioxus::prelude::*;
use forge_domain::Marketplace;
use forge_features::{connector_dialog_busy, use_customize, Busy, Dialog, Job, Loaded};
use forge_shared::{Button, ButtonKind};

use crate::customize_connector_dialogs::{AddConnector, ConfirmRemoveConnector, SignInConnector};

/// Whichever dialog is open, over the page. A click on the backdrop closes it.
#[component]
pub fn CustomizeDialogs() -> Element {
    let customize = use_customize();
    let Some(dialog) = customize.dialog() else {
        return rsx! {};
    };
    let dialog_for_click = dialog.clone();
    let wide = matches!(dialog, Dialog::SignInConnector(_));
    rsx! {
        div {
            class: "settings-backdrop",
            // While a marketplace is being added the dialog stays: its result (or its error) shows here.
            onclick: move |_| {
                if !customize.adding_marketplace() && !connector_dialog_busy(&dialog_for_click, customize.connectors_busy()) {
                    customize.close_dialog();
                }
            },
            div { class: if wide { "customize-dialog sign-in" } else { "customize-dialog" }, onclick: move |event| event.stop_propagation(),
                match dialog {
                    Dialog::AddMarketplace => rsx! { AddMarketplace {} },
                    Dialog::ManageMarketplaces => rsx! { ManageMarketplaces {} },
                    Dialog::ConfirmUninstall(id) => rsx! { ConfirmUninstall { id } },
                    Dialog::ConfirmRemoveMarketplace(name) => rsx! { ConfirmRemove { name } },
                    Dialog::CreateSkill => rsx! { CreateSkill {} },
                    Dialog::AddConnector => rsx! { AddConnector {} },
                    Dialog::ConfirmRemoveConnector(name) => rsx! { ConfirmRemoveConnector { name } },
                    Dialog::SignInConnector(name) => rsx! { SignInConnector { name } },
                }
            }
        }
    }
}

#[component]
fn AddMarketplace() -> Element {
    let customize = use_customize();
    let mut text = use_signal(String::new);
    let adding = customize.adding_marketplace();
    rsx! {
        h2 { "Add marketplace" }
        p { class: "hint", "A GitHub repository (owner/repo), a Git URL or a folder on this computer." }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: "owner/repo, Git URL or folder path",
            value: "{text}",
            disabled: adding,
            oninput: move |event| text.set(event.value()),
            onkeydown: move |event| {
                if event.key() == Key::Enter && !adding {
                    customize.add_marketplace(&text());
                }
            },
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if adding { span { class: "spinner", title: "Adding…" } }
            Button { label: "Cancel", disabled: adding, onclick: move |_| customize.close_dialog() }
            Button { label: "Add", kind: ButtonKind::Primary, disabled: adding, onclick: move |_| customize.add_marketplace(&text()) }
        }
    }
}

#[component]
fn ManageMarketplaces() -> Element {
    let customize = use_customize();
    let mut menu: Signal<Option<String>> = use_signal(|| None);
    let markets: Vec<Marketplace> = match customize.view() {
        Loaded::Ready(view) => view.snapshot.marketplaces,
        _ => Vec::new(),
    };
    rsx! {
        h2 { "Manage marketplaces" }
        p { class: "hint", "These are the marketplaces added with Claude Code on this computer. Removing one also removes its plugins." }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error", "{message}" }
        }
        if markets.is_empty() {
            div { class: "hint", "No marketplaces yet." }
        }
        for market in markets {
            {
            let busy = customize.busy_marketplace(&market.name);
            rsx! {
            div { key: "{market.name}", class: "customize-market",
                div { class: "customize-main",
                    div { class: "customize-name", "{market.name}" }
                    div { class: "customize-desc", "{market.location}" }
                    if let Some(when) = market.last_updated.clone() { div { class: "customize-source", "Last updated {forge_features::short_time(&when)}" } }
                }
                if busy != Busy::No { span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Working…" } } }
                button {
                    class: "icon-button",
                    title: "More",
                    disabled: busy != Busy::No,
                    onclick: {
                        let name = market.name.clone();
                        move |_| menu.set(if menu() == Some(name.clone()) { None } else { Some(name.clone()) })
                    },
                    "⋮"
                }
                if menu() == Some(market.name.clone()) {
                    div { class: "menu-backdrop", onclick: move |_| menu.set(None) }
                    div { class: "customize-menu row-menu",
                        button {
                            onclick: {
                                let name = market.name.clone();
                                move |_| {
                                    menu.set(None);
                                    customize.run(Job::UpdateMarketplace(name.clone()));
                                }
                            },
                            "Update"
                        }
                        button {
                            class: "danger",
                            onclick: {
                                let name = market.name.clone();
                                move |_| {
                                    menu.set(None);
                                    customize.open_dialog(Dialog::ConfirmRemoveMarketplace(name.clone()));
                                }
                            },
                            "Remove"
                        }
                    }
                }
            }
            }
            }
        }
        div { class: "customize-dialog-buttons",
            Button { label: "Done", kind: ButtonKind::Primary, onclick: move |_| customize.close_dialog() }
        }
    }
}

#[component]
fn ConfirmUninstall(id: String) -> Element {
    let customize = use_customize();
    let name = forge_domain::split_plugin_id(&id).0.to_string();
    rsx! {
        h2 { "Uninstall {name}?" }
        p { "Other Claude Code installs on this computer lose it too." }
        div { class: "customize-dialog-buttons",
            Button { label: "Cancel", onclick: move |_| customize.close_dialog() }
            Button {
                label: "Uninstall",
                kind: ButtonKind::Danger,
                onclick: move |_| {
                    customize.close_dialog();
                    customize.run(Job::Uninstall(id.clone()));
                },
            }
        }
    }
}

#[component]
fn ConfirmRemove(name: String) -> Element {
    let customize = use_customize();
    rsx! {
        h2 { "Remove {name}?" }
        p { "The plugins installed from it are removed too, here and in other Claude Code installs on this computer." }
        div { class: "customize-dialog-buttons",
            Button { label: "Cancel", onclick: move |_| customize.open_dialog(Dialog::ManageMarketplaces) }
            Button {
                label: "Remove",
                kind: ButtonKind::Danger,
                onclick: move |_| {
                    customize.open_dialog(Dialog::ManageMarketplaces);
                    customize.run(Job::RemoveMarketplace(name.clone()));
                },
            }
        }
    }
}

#[component]
fn CreateSkill() -> Element {
    let customize = use_customize();
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let busy = customize.skills_busy();
    rsx! {
        h2 { "Create skill" }
        p { class: "hint", "Makes a SKILL.md in your skills folder. Letters, numbers, - and _ in the name." }
        input {
            class: "customize-field",
            r#type: "text",
            autocapitalize: "off",
            spellcheck: false,
            placeholder: "name, for example my-skill",
            value: "{name}",
            disabled: busy,
            oninput: move |event| name.set(event.value()),
        }
        input {
            class: "customize-field",
            r#type: "text",
            autocapitalize: "off",
            spellcheck: false,
            placeholder: "what it does and when to use it",
            value: "{description}",
            disabled: busy,
            oninput: move |event| description.set(event.value()),
            onkeydown: move |event| {
                if event.key() == Key::Enter && !busy {
                    customize.create_skill(&name(), &description());
                }
            },
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if busy { span { class: "spinner", title: "Creating…" } }
            Button { label: "Cancel", disabled: busy, onclick: move |_| customize.close_dialog() }
            Button { label: "Create", kind: ButtonKind::Primary, disabled: busy, onclick: move |_| customize.create_skill(&name(), &description()) }
        }
    }
}
