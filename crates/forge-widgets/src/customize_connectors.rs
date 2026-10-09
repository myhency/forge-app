use dioxus::prelude::*;
use forge_domain::ConnectorKind;
use forge_features::{
    connection_text, connector_groups, explore_connectors, find_connector, shows_sign_in, source_label, use_customize, Busy, ConnectionState, ConnectorRow,
    CustomizeMode, Dialog, Job, Loaded,
};
use forge_shared::{tr, Button, ButtonKind};

#[component]
pub fn ConnectorsTab() -> Element {
    let customize = use_customize();
    let Some(view) = customize.connectors() else {
        return match customize.view() {
            Loaded::Failed(message) => rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: tr!("Try again"), onclick: move |_| customize.reload() }
            },
            Loaded::Ready(_) => rsx! {
                div { class: "status-line error", {tr!("The connectors could not be read.")} }
                Button { label: tr!("Try again"), onclick: move |_| customize.reload() }
            },
            Loaded::Loading => rsx! { div { class: "hint", {tr!("Loading…")} } },
        };
    };
    if let Some(id) = customize.detail() {
        return match find_connector(&view, &id) {
            Some(row) => rsx! { ConnectorDetail { row } },
            None => rsx! {
                div { class: "hint",
                    {tr!("That connector is no longer there. ")}
                    Button { label: tr!("Back"), onclick: move |_| customize.close_detail() }
                }
            },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = connector_groups(&view, &query);
            if groups.mine.is_empty() && groups.from_plugins.is_empty() {
                let text = if query.trim().is_empty() { tr!("No connectors yet. Use + Add to add one.") } else { tr!("Nothing matches.") };
                return rsx! { div { class: "hint", "{text}" } };
            }
            rsx! {
                if !groups.mine.is_empty() { ConnectorGroup { title: tr!("Mine"), rows: groups.mine } }
                if !groups.from_plugins.is_empty() { ConnectorGroup { title: tr!("From plugins"), rows: groups.from_plugins } }
            }
        }
        CustomizeMode::Explore => {
            if !view.has_marketplaces {
                return rsx! {
                    div { class: "customize-note",
                        p { {tr!("Add a marketplace in the Plugins tab to find connectors.")} }
                        Button { label: tr!("Add marketplace"), kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            let cards = explore_connectors(&view, &query);
            if cards.is_empty() {
                return rsx! {
                    div { class: "hint",
                        {tr!("No connectors to find in the marketplaces you added. A plugin that lives in another Git repository shows its connectors after it is installed.")}
                    }
                };
            }
            rsx! {
                div { class: "customize-grid",
                    for row in cards {
                        ConnectorCard { key: "{row.entry.id}", row }
                    }
                }
            }
        }
    }
}

/// What went wrong on the way to the list but did not stop it: a project file that could not be read, and a
/// plugin read that failed (the connectors of plugins are then missing). Shown above the list, not in a detail.
#[component]
pub fn ConnectorNotes() -> Element {
    let customize = use_customize();
    if customize.detail().is_some() {
        return rsx! {};
    }
    let notes = customize.connectors().map(|view| view.snapshot.notes).unwrap_or_default();
    let plugins_failed = match customize.view() {
        Loaded::Failed(message) => Some(message),
        _ => None,
    };
    rsx! {
        for note in notes {
            div { key: "{note}", class: "status-line error customize-wrap", "{note}" }
        }
        if let Some(message) = plugins_failed {
            div { class: "status-line error customize-wrap", {tr!("The connectors of plugins are not shown: {message}", message = message)} }
        }
    }
}

fn kind_tag(kind: ConnectorKind) -> &'static str {
    match kind {
        ConnectorKind::Stdio => "local",
        ConnectorKind::Http => "http",
        ConnectorKind::Sse => "sse",
        ConnectorKind::Unknown => "unreadable",
    }
}

#[component]
fn ConnectorGroup(title: String, rows: Vec<ConnectorRow>) -> Element {
    let customize = use_customize();
    rsx! {
        div { class: "customize-group",
            h2 { "{title} " span { class: "count", "{rows.len()}" } }
            for row in rows {
                {
                    let id = row.entry.id.clone();
                    let (name, on) = (row.entry.name.clone(), row.enabled);
                    rsx! {
                        div {
                            key: "{id}",
                            class: "customize-row",
                            onclick: move |_| customize.open_connector(id.clone()),
                            div { class: "customize-icon", "⌁" }
                            div { class: "customize-main",
                                div { class: "customize-name",
                                    "{row.entry.name}"
                                    span { class: "tag", "{kind_tag(row.entry.kind)}" }
                                    span { class: "tag", "{source_label(&row.entry.source)}" }
                                    if let Some(tag) = customize.checked_tag(&row.entry.name) {
                                        span { class: "tag warn", "{tag}" }
                                    }
                                }
                                div { class: "customize-desc", "{row.entry.target}" }
                            }
                            button {
                                class: if on { "switch on" } else { "switch" },
                                disabled: !row.switchable(),
                                title: if row.switchable() { if on { tr!("Turn off") } else { tr!("Turn on") } } else { tr!("Turned on and off with its plugin") },
                                onclick: move |event| {
                                    event.stop_propagation();
                                    customize.set_connector_enabled(name.clone(), !on);
                                },
                                span { class: "knob" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ConnectorCard(row: ConnectorRow) -> Element {
    let customize = use_customize();
    let plugin_id = row.entry.source.plugin_id().unwrap_or_default().to_string();
    let busy = customize.busy(&plugin_id);
    let id = row.entry.id.clone();
    let install = plugin_id.clone();
    rsx! {
        div { class: "customize-card", onclick: move |_| customize.open_connector(id.clone()),
            div { class: "customize-icon", "⌁" }
            div { class: "customize-main",
                div { class: "customize-name", "{row.entry.name}" }
                div { class: "customize-card-desc", "{row.entry.target}" }
                div { class: "customize-source", "{plugin_id}" }
            }
            if busy != Busy::No {
                span { class: "spinner", title: if busy == Busy::Waiting { tr!("Waiting…") } else { tr!("Working…") } }
            } else {
                button {
                    class: "customize-plus",
                    title: tr!("Install the plugin {plugin}", plugin = plugin_id),
                    onclick: move |event| {
                        event.stop_propagation();
                        customize.run(Job::Install(install.clone()));
                    },
                    "+"
                }
            }
        }
    }
}

#[component]
fn ConnectorDetail(row: ConnectorRow) -> Element {
    let customize = use_customize();
    let entry = row.entry.clone();
    let name = entry.name.clone();
    let on = row.enabled;
    let state = customize.connection();
    let checking = state == ConnectionState::Checking;
    let plugin_id = entry.source.plugin_id().map(str::to_string);
    let busy = plugin_id.as_deref().map_or(Busy::No, |id| customize.busy(id));
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), {tr!("← Connectors")} }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "⌁" }
                div {
                    h2 { class: "customize-wrap", "{entry.name}" }
                    div { class: "hint", "{forge_shared::i18n::text(entry.kind.label())} · {source_label(&entry.source)}" }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: tr!("Working…") }
                    } else if !entry.installed {
                        if let Some(id) = plugin_id.clone() {
                            Button { label: tr!("Install plugin"), kind: ButtonKind::Primary, onclick: move |_| customize.run(Job::Install(id.clone())) }
                        }
                    } else if row.switchable() {
                        button {
                            class: if on { "switch on" } else { "switch" },
                            title: if on { tr!("Turn off") } else { tr!("Turn on") },
                            onclick: {
                                let name = name.clone();
                                move |_| customize.set_connector_enabled(name.clone(), !on)
                            },
                            span { class: "knob" }
                        }
                    }
                }
            }
            div { class: "customize-section",
                div { class: "label", if entry.kind == ConnectorKind::Stdio { {tr!("Command")} } else { {tr!("Address")} } }
                p { class: "mono customize-wrap", "{entry.target}" }
            }
            if !entry.header_names.is_empty() {
                div { class: "customize-section",
                    div { class: "label", {tr!("Headers")} }
                    for header in entry.header_names.clone() {
                        p { key: "{header}", class: "mono customize-wrap", "{header}: ••••••" }
                    }
                }
            }
            if !entry.env_keys.is_empty() {
                div { class: "customize-section",
                    div { class: "label", {tr!("Environment")} }
                    for key in entry.env_keys.clone() {
                        p { key: "{key}", class: "mono customize-wrap", "{key}=••••••" }
                    }
                }
            }
            if entry.installed && entry.source.checkable() {
                div { class: "customize-section",
                    div { class: "label", {tr!("Connection")} }
                    div { class: "customize-check",
                        Button {
                            label: tr!("Check connection"),
                            disabled: checking,
                            onclick: {
                                let name = name.clone();
                                move |_| customize.check_connector(name.clone())
                            },
                        }
                        if shows_sign_in(&state) {
                            Button {
                                label: tr!("Sign in"),
                                onclick: {
                                    let name = name.clone();
                                    move |_| customize.sign_in_connector(name.clone())
                                },
                            }
                        }
                        span { class: "connection-line customize-wrap", "{connection_text(&state)}" }
                    }
                }
            }
            div { class: "customize-section",
                if entry.source.removable() {
                    Button {
                        label: tr!("Remove"),
                        onclick: {
                            let name = name.clone();
                            move |_| customize.open_dialog(Dialog::ConfirmRemoveConnector(name.clone()))
                        },
                    }
                } else if entry.source.is_plugin() {
                    p { class: "hint", {tr!("This connector comes with its plugin. Uninstall the plugin in the Plugins tab to remove it.")} }
                } else {
                    p { class: "hint", {tr!("This connector is defined in the project's files. Change it there.")} }
                }
            }
        }
    }
}
