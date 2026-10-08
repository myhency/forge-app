use dioxus::prelude::*;
use forge_domain::Marketplace;
use forge_features::{use_customize, Dialog, Job, Loaded};
use forge_shared::{Button, ButtonKind};

/// Whichever dialog is open, over the page. A click on the backdrop closes it.
#[component]
pub fn CustomizeDialogs() -> Element {
    let customize = use_customize();
    let Some(dialog) = customize.dialog() else {
        return rsx! {};
    };
    rsx! {
        div { class: "settings-backdrop", onclick: move |_| customize.close_dialog(),
            div { class: "customize-dialog", onclick: move |event| event.stop_propagation(),
                match dialog {
                    Dialog::AddMarketplace => rsx! { AddMarketplace {} },
                    Dialog::ManageMarketplaces => rsx! { ManageMarketplaces {} },
                    Dialog::ConfirmUninstall(id) => rsx! { ConfirmUninstall { id } },
                    Dialog::ConfirmRemoveMarketplace(name) => rsx! { ConfirmRemove { name } },
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
            placeholder: "owner/repo, https://git.example.com/team/plugins.git or /path/to/folder",
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
        if markets.is_empty() {
            div { class: "hint", "No marketplaces yet." }
        }
        for market in markets {
            div { key: "{market.name}", class: "customize-market",
                div { class: "customize-main",
                    div { class: "customize-name", "{market.name}" }
                    div { class: "customize-desc", "{market.location}" }
                    if let Some(when) = market.last_updated.clone() { div { class: "customize-source", "Last updated {when}" } }
                }
                button {
                    class: "icon-button",
                    title: "More",
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
