use dioxus::prelude::*;
use forge_domain::ConnectorKind;
use forge_features::{
    connection_text, connector_groups, explore_connectors, find_connector, source_label, use_customize, Busy, ConnectionState, ConnectorRow,
    CustomizeMode, Dialog, Job, Loaded,
};
use forge_shared::{Button, ButtonKind};

#[component]
pub fn ConnectorsTab() -> Element {
    let customize = use_customize();
    let Some(view) = customize.connectors() else {
        return match customize.view() {
            Loaded::Failed(message) => rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            },
            Loaded::Ready(_) => rsx! {
                div { class: "status-line error", "The connectors could not be read." }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            },
            Loaded::Loading => rsx! { div { class: "hint", "Loading…" } },
        };
    };
    if let Some(id) = customize.detail() {
        return match find_connector(&view, &id) {
            Some(row) => rsx! { ConnectorDetail { row } },
            None => rsx! {
                div { class: "hint",
                    "That connector is no longer there. "
                    Button { label: "Back", onclick: move |_| customize.close_detail() }
                }
            },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = connector_groups(&view, &query);
            if groups.mine.is_empty() && groups.from_plugins.is_empty() {
                let text = if query.trim().is_empty() { "No connectors yet. Use + Add to add one." } else { "Nothing matches." };
                return rsx! { div { class: "hint", "{text}" } };
            }
            rsx! {
                if !groups.mine.is_empty() { ConnectorGroup { title: "Mine", rows: groups.mine } }
                if !groups.from_plugins.is_empty() { ConnectorGroup { title: "From plugins", rows: groups.from_plugins } }
            }
        }
        CustomizeMode::Explore => {
            if !view.has_marketplaces {
                return rsx! {
                    div { class: "customize-note",
                        p { "Add a marketplace in the Plugins tab to find connectors." }
                        Button { label: "Add marketplace", kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            let cards = explore_connectors(&view, &query);
            if cards.is_empty() {
                return rsx! {
                    div { class: "hint",
                        "No connectors to find in the marketplaces you added. A plugin that lives in another Git repository shows its connectors after it is installed."
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
                                }
                                div { class: "customize-desc", "{row.entry.target}" }
                            }
                            button {
                                class: if on { "switch on" } else { "switch" },
                                disabled: !row.switchable(),
                                title: if row.switchable() { if on { "Turn off" } else { "Turn on" } } else { "Turned on and off with its plugin" },
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
                span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Working…" } }
            } else {
                button {
                    class: "customize-plus",
                    title: "Install the plugin {plugin_id}",
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
            button { class: "customize-back", onclick: move |_| customize.close_detail(), "← Connectors" }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "⌁" }
                div {
                    h2 { class: "customize-wrap", "{entry.name}" }
                    div { class: "hint", "{entry.kind.label()} · {source_label(&entry.source)}" }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: "Working…" }
                    } else if !entry.installed {
                        if let Some(id) = plugin_id.clone() {
                            Button { label: "Install plugin", kind: ButtonKind::Primary, onclick: move |_| customize.run(Job::Install(id.clone())) }
                        }
                    } else if row.switchable() {
                        button {
                            class: if on { "switch on" } else { "switch" },
                            title: if on { "Turn off" } else { "Turn on" },
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
                div { class: "label", if entry.kind == ConnectorKind::Stdio { "Command" } else { "Address" } }
                p { class: "mono customize-wrap", "{entry.target}" }
            }
            if !entry.header_names.is_empty() {
                div { class: "customize-section",
                    div { class: "label", "Headers" }
                    for header in entry.header_names.clone() {
                        p { key: "{header}", class: "mono customize-wrap", "{header}: ••••••" }
                    }
                }
            }
            if !entry.env_keys.is_empty() {
                div { class: "customize-section",
                    div { class: "label", "Environment" }
                    for key in entry.env_keys.clone() {
                        p { key: "{key}", class: "mono customize-wrap", "{key}=••••••" }
                    }
                }
            }
            if entry.installed && entry.source.checkable() {
                div { class: "customize-section",
                    div { class: "label", "Connection" }
                    div { class: "customize-check",
                        Button {
                            label: "Check connection",
                            disabled: checking,
                            onclick: {
                                let name = name.clone();
                                move |_| customize.check_connector(name.clone())
                            },
                        }
                        span { class: "connection-line customize-wrap", "{connection_text(&state)}" }
                    }
                }
            }
            div { class: "customize-section",
                if entry.source.removable() {
                    Button {
                        label: "Remove",
                        onclick: {
                            let name = name.clone();
                            move |_| customize.open_dialog(Dialog::ConfirmRemoveConnector(name.clone()))
                        },
                    }
                } else if entry.source.is_plugin() {
                    p { class: "hint", "This connector comes with its plugin. Uninstall the plugin in the Plugins tab to remove it." }
                } else {
                    p { class: "hint", "This connector is defined in the project's files. Change it there." }
                }
            }
        }
    }
}
