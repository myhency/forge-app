use dioxus::prelude::*;
use forge_features::{use_settings, use_workspace};
use forge_shared::{tr, Button};

/// The Developer page: the MCP servers that run on this computer, a detail for the chosen one, and buttons that
/// open the file defining them and the server's log.
#[component]
pub fn DeveloperSection() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    let servers = settings.local_servers();
    let chosen = settings.selected_server();
    let mut message = use_signal(|| None::<String>);
    rsx! {
        h2 { {tr!("Developer")} }
        div { class: "developer-head",
            div {
                h3 { class: "settings-group", {tr!("Local MCP servers")} }
                div { class: "hint", {tr!("Add and manage the MCP servers you work with.")} }
            }
            Button { label: tr!("Edit config"), onclick: move |_| workspace.reveal_mcp_config() }
        }
        match servers {
            Err(message) => rsx! { div { class: "status-line error", "{message}" } },
            Ok(servers) if servers.is_empty() => rsx! { div { class: "hint", {tr!("No local MCP servers.")} } },
            Ok(servers) => rsx! {
                div { class: "developer-body",
                    div { class: "developer-list",
                        for server in servers {
                            button {
                                key: "{server.name}",
                                class: if chosen.as_ref().is_some_and(|c| c.name == server.name) { "settings-nav-item active" } else { "settings-nav-item" },
                                onclick: {
                                    let name = server.name.clone();
                                    move |_| {
                                        message.set(None);
                                        settings.select_server(name.clone());
                                    }
                                },
                                "{server.name}"
                            }
                        }
                    }
                    if let Some(server) = chosen {
                        div { class: "developer-detail",
                            h3 { class: "customize-wrap", "{server.name}" }
                            div { class: "label", {tr!("Command")} }
                            p { class: "mono customize-wrap", "{server.command}" }
                            if !server.args.is_empty() {
                                div { class: "label", {tr!("Arguments")} }
                                p { class: "mono customize-wrap", "{server.args.join(\" \")}" }
                            }
                            Button {
                                label: tr!("View logs"),
                                onclick: {
                                    let name = server.name.clone();
                                    move |_| message.set(workspace.open_mcp_log(&name).err())
                                },
                            }
                            if let Some(text) = message() {
                                div { class: "status-line error", "{text}" }
                            }
                        }
                    }
                }
            },
        }
    }
}
