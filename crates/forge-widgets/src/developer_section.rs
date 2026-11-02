use dioxus::prelude::*;
use forge_features::{use_settings, use_workspace};
use forge_shared::{tr, Button, SettingRow, Switch};

/// The Developer page: the MCP servers that run on this computer, a detail for the chosen one, and buttons that
/// open the file defining them and the server's log.
#[component]
pub fn DeveloperSection() -> Element {
    let settings = use_settings();
    let workspace = use_workspace();
    let servers = settings.local_servers();
    let chosen = settings.selected_server();
    let mut message = use_signal(|| None::<String>);
    let mut report = use_signal(|| ReportStatus::Idle);
    let mut with_conversations = use_signal(|| false);
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
        div { class: "developer-report",
            h3 { class: "settings-group", {tr!("Report a problem")} }
            div { class: "hint",
                {tr!("Save Forge's logs, this computer's system logs and its details (processor, memory, disk) as one zip file to attach to a report. Your home folder, user name, API keys, tokens and e-mail addresses are hidden first.")}
            }
            SettingRow {
                title: tr!("Include conversations"),
                description: tr!("Adds the newest three conversations (their last part). They hold what you and Claude wrote, so check them before you send the file."),
                Switch { on: with_conversations(), onchange: move |on| with_conversations.set(on) }
            }
            div { class: "developer-report-buttons",
                Button {
                    label: if report() == ReportStatus::Saving { tr!("Saving…") } else { tr!("Save logs…") },
                    disabled: report() == ReportStatus::Saving,
                    onclick: move |_| {
                        if report() == ReportStatus::Saving {
                            return;
                        }
                        // The task belongs to this page: closing Settings ends it instead of writing to a gone signal.
                        spawn(async move {
                            let Some(dest) = workspace.pick_report_path().await else { return };
                            report.set(ReportStatus::Saving);
                            match workspace.write_log_report(dest, with_conversations()).await {
                                Ok((path, summary)) => report.set(ReportStatus::Saved { path, files: summary.entries.len(), hidden: summary.hidden }),
                                Err(text) => report.set(ReportStatus::Failed(text)),
                            }
                        });
                    },
                }
                Button {
                    label: tr!("Open log folder"),
                    onclick: move |_| {
                        if let Err(text) = workspace.reveal_log_dir() {
                            report.set(ReportStatus::Failed(text));
                        }
                    },
                }
            }
            match report() {
                ReportStatus::Saved { path, files, hidden } => rsx! {
                    div { class: "status-line",
                        {tr!("Saved {files} files ({hidden} personal details hidden) to {path}", files = files, hidden = hidden, path = path.display().to_string())}
                    }
                    Button { label: tr!("Show in Finder"), onclick: { let path = path.clone(); move |_| workspace.reveal(&path) } }
                },
                ReportStatus::Failed(text) => rsx! { div { class: "status-line error", "{text}" } },
                _ => rsx! {},
            }
        }
    }
}

#[derive(Clone, PartialEq)]
enum ReportStatus {
    Idle,
    Saving,
    Saved { path: std::path::PathBuf, files: usize, hidden: usize },
    Failed(String),
}
