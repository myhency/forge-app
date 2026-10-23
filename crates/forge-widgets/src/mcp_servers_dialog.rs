use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{connector_table, ConnectorState, ServerTag, TableStatus, TransportLabel};
use forge_features::{refocus_workspace, use_workspace};
use forge_shared::{tr, Button, ButtonKind};

pub fn connector_state_text(state: ConnectorState) -> &'static str {
    match state {
        ConnectorState::SignIn => tr!("Authenticate"),
        ConnectorState::Reconnect => tr!("Reconnect"),
        ConnectorState::Connecting => tr!("Connecting"),
        ConnectorState::Off => tr!("Off"),
    }
}

/// The session's MCP servers as a table. Escape, `완료` or a click outside closes it.
#[component]
pub fn McpServersDialog(id: LiveSessionId, on_close: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (folder, servers) = {
        let view = view.read();
        (view.project_path.display().to_string(), view.mcp_servers.clone().unwrap_or_default())
    };
    // The plugin name lookup is not available yet, so no subtitle shows.
    // The workspace has no lookup from a server to its plugin's name, so the subtitle is left out.
    let table = connector_table(&servers, &|_| None);
    let close = move || {
        on_close.call(());
        refocus_workspace();
    };
    rsx! {
        div { class: "mcp-backdrop", onclick: move |_| close(),
            div {
                class: "mcp-dialog",
                role: "dialog",
                "aria-modal": "true",
                tabindex: "0",
                "aria-label": tr!("MCP servers"),
                // Key events reach the dialog only while it has focus.
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        close();
                    }
                },
                h3 { {tr!("MCP servers")} }
                p { class: "trust-path", title: "{folder}", "{folder}" }
                div { class: "mcp-table",
                    div { class: "mcp-row mcp-head",
                        span { {tr!("Server")} }
                        span { {tr!("Type")} }
                        span { {tr!("Status")} }
                    }
                    if table.is_empty() {
                        div { class: "mcp-empty", {tr!("No connectors")} }
                    }
                    for row in table.iter() {
                        div { class: "mcp-row", key: "{row.name}",
                            span { class: "mcp-name",
                                span { class: "mcp-name-text", "{row.name}" }
                                if let Some(plugin) = row.subtitle.clone() {
                                    span { class: "mcp-sub", {tr!("{name} plugin provides this", name = plugin)} }
                                }
                                if let Some(tag) = row.tag {
                                    span { class: "pane-tag",
                                        match tag {
                                            ServerTag::Plugin => tr!("Plugin"),
                                            ServerTag::User => tr!("User"),
                                        }
                                    }
                                }
                            }
                            span { class: "mcp-kind",
                                match row.kind {
                                    Some(TransportLabel::Web) => tr!("Web"),
                                    Some(TransportLabel::Desktop) => tr!("Desktop"),
                                    None => "",
                                }
                            }
                            match row.status {
                                TableStatus::Connected => rsx! { span { class: "mcp-status ok", {tr!("Connected")} } },
                                TableStatus::State(state) => rsx! { span { class: "mcp-status action", {connector_state_text(state)} } },
                            }
                        }
                    }
                }
                div { class: "env-buttons",
                    Button {
                        label: tr!("Manage connectors"),
                        onclick: move |_| {
                            workspace.open_connectors_page();
                            close();
                        },
                    }
                    span { class: "spacer" }
                    Button { label: tr!("Done"), kind: ButtonKind::Primary, onclick: move |_| close() }
                }
            }
        }
    }
}
