use std::path::{Path, PathBuf};

use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{clear_key, task_lists, Clock, connector_rows, context_card, icon_of, row_link, ContextItemView, ContextRowView, RowLink};
use forge_features::{card_folders, use_workspace, DetailsCard, EnvironmentRequest, Panel};
use forge_shared::{tr, Icon, IconKind};

use crate::mcp_servers_dialog::{connector_state_text, McpServersDialog};
use crate::files_icons::FileTypeIcon;

/// The session rail: the cards of the session one under another, each its own box with a gap
/// between them, and no frame or title around them (the desktop app's rail). The header's button
/// shows and hides it.
#[component]
pub fn DetailsPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    rsx! {
        section { class: "session-rail", "aria-label": Panel::Details.title(),
            // Only in a narrow window, where the dock covers the header's button for the rail.
            button {
                class: "icon-button rail-close",
                r#type: "button",
                title: tr!("Close"),
                "aria-label": tr!("Close"),
                onclick: move |_| workspace.change_dock(id, |dock| dock.close(Panel::Details)),
                Icon { kind: IconKind::Close }
            }
            EnvironmentCard { id }
            ContextCard { id }
            ConnectorsCard { id }
            OutputsCard { id }
            ChangesCard { id }
            TasksCard { id }
        }
    }
}

#[component]
fn CardIcon(kind: &'static str) -> Element {
    rsx! {
        svg {
            class: "env-card-icon",
            view_box: "0 0 16 16",
            width: "14",
            height: "14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match kind {
                "laptop" => rsx! {
                    rect { x: "3", y: "3", width: "10", height: "7", rx: "1" }
                    path { d: "M1.5 12.5h13" }
                },
                "folder" => rsx! { path { d: "M2 4.5A1 1 0 0 1 3 3.5h3l1.5 1.5H13a1 1 0 0 1 1 1v6a1 1 0 0 1-1 1H3a1 1 0 0 1-1-1z" } },
                "diff" => rsx! {
                    path { d: "M8 2.5v6M5 5.5h6M5 12.5h6" }
                },
                "terminal" => rsx! {
                    rect { x: "1.5", y: "2.5", width: "13", height: "11", rx: "2" }
                    path { d: "M4.5 6l2.5 2-2.5 2M8.5 10.5h3" }
                },
                "done" => rsx! {
                    path { d: "M3 8l3.5-4.5L10 8l-3.5 4.5zM8 8l3-3.8L14 8l-3 3.8" }
                },
                "branch" => rsx! {
                    circle { cx: "4.5", cy: "3.5", r: "1.5" }
                    circle { cx: "4.5", cy: "12.5", r: "1.5" }
                    circle { cx: "11.5", cy: "6", r: "1.5" }
                    path { d: "M4.5 5v6M11.5 7.5c0 2-2 2.5-7 3.5" }
                },
                "gear" => rsx! {
                    circle { cx: "8", cy: "8", r: "2" }
                    path { d: "M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4" }
                },
                _ => rsx! { path { d: "M4 6l4 4 4-4" } },
            }
        }
    }
}

fn folder_parts(path: &Path) -> (String, String) {
    let name = path.file_name().map_or_else(|| path.to_string_lossy().to_string(), |name| name.to_string_lossy().to_string());
    let parent = path.parent().map(|parent| parent.to_string_lossy().to_string()).unwrap_or_default();
    (name, parent)
}

#[component]
fn EnvironmentCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut menu = use_signal(|| None::<usize>);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let view = view.read().clone();
    let origin = workspace.origin_of(&view.project_path);
    let scratch = workspace.is_scratch_project(&view.project_path);
    let environment = workspace.session_environment(&view.project_path, view.claude_session_id.as_ref());
    let folded = workspace.details_card_folded(id, DetailsCard::Environment);
    let folders = if scratch { Vec::new() } else { card_folders(&origin, &workspace.session_added_folders(id)) };
    let busy = workspace.session_is_busy(id);
    let worktree_session = workspace.worktree_tag(&view.project_path).is_some();
    // The branch the session's folder is on; read again when a turn starts or ends (it may switch).
    let branch = {
        let project = view.project_path.clone();
        let running = view.phase == Phase::Running;
        use_resource(use_reactive!(|project, running| async move {
            let _ = running;
            workspace.folder_branches(project).await.and_then(|branches| branches.current)
        }))
    };
    let branch_name = branch.read().clone().flatten();

    rsx! {
        div { class: "env-card",
            button {
                class: "env-card-head",
                r#type: "button",
                "aria-expanded": "{!folded}",
                onclick: move |_| workspace.set_details_card_folded(id, DetailsCard::Environment, !folded),
                span { {tr!("Environment")} }
                span { class: if folded { "env-card-chevron folded" } else { "env-card-chevron" }, CardIcon { kind: "chevron" } }
            }
            if !folded {
                div { class: "env-card-row",
                    CardIcon { kind: "laptop" }
                    if let Some(environment) = environment.clone() {
                        {
                            let environment_id = environment.id.clone();
                            rsx! {
                        span { class: "env-card-name", "{environment.name}" }
                        button {
                            class: "icon-button",
                            r#type: "button",
                            title: tr!("Edit"),
                            "aria-label": tr!("Edit"),
                            onclick: move |_| workspace.open_environment_dialog(EnvironmentRequest::EditInSession(environment_id.clone())),
                            CardIcon { kind: "gear" }
                        }
                            }
                        }
                    } else {
                        span { class: "env-card-name", {tr!("This computer")} }
                        if !scratch {
                            button {
                                class: "env-card-save",
                                r#type: "button",
                                title: tr!("Local session"),
                                onclick: {
                                    let origin = origin.clone();
                                    let view = view.clone();
                                    move |_| workspace.open_environment_dialog(EnvironmentRequest::NewFromSession { folder: origin.clone(), model: view.model.clone(), effort: view.effort })
                                },
                                {tr!("Save")}
                            }
                        }
                    }
                }
                if let Some(branch_name) = branch_name {
                    div { class: "env-card-row",
                        CardIcon { kind: "branch" }
                        span { class: "env-card-name", "{branch_name}" }
                        if worktree_session {
                            span { class: "env-card-tag", {tr!("Worktree")} }
                        }
                    }
                }
                for (index, folder) in folders.iter().cloned().enumerate() {
                    FolderRow { key: "{folder.display()}", id, folder, first: index == 0, busy, worktree_session, open: menu() == Some(index), on_open: move |open: bool| menu.set(if open { Some(index) } else { None }) }
                }
            }
        }
    }
}

#[component]
fn FolderRow(id: LiveSessionId, folder: PathBuf, first: bool, busy: bool, worktree_session: bool, open: bool, on_open: EventHandler<bool>) -> Element {
    let workspace = use_workspace();
    let (name, parent) = folder_parts(&folder);
    let items = forge_features::folder_menu(first, busy, worktree_session);
    rsx! {
        div { class: "env-card-folder",
            button { class: "env-card-row env-card-folder-button", r#type: "button", onclick: move |_| on_open.call(!open),
                CardIcon { kind: "folder" }
                span { class: "env-card-name", "{name}" }
                // The desktop app names the parent on the session's own folder only.
                if first {
                    span { class: "env-card-parent", " · {parent}" }
                }
            }
            if open {
                div { class: "picker-backdrop", onclick: move |_| on_open.call(false) }
                div {
                    class: "picker-menu env-card-menu",
                    role: "menu",
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            on_open.call(false);
                        }
                    },
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: {
                            let folder = folder.clone();
                            move |_| {
                                on_open.call(false);
                                workspace.reveal(&folder);
                            }
                        },
                        span { class: "picker-text", {forge_features::show_in_file_manager()} }
                    }
                    div { class: "picker-divider" }
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        onclick: {
                            let folder = folder.clone();
                            move |_| {
                                on_open.call(false);
                                forge_entities::copy_text(&folder.to_string_lossy());
                            }
                        },
                        span { class: "picker-text", {tr!("Copy path")} }
                    }
                    if items.change {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.change_session_folder(id);
                            },
                            span { class: "picker-text", {tr!("Change folder…")} }
                        }
                    }
                    if items.add {
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.add_session_folder(id);
                            },
                            span { class: "picker-text", {tr!("Add folder…")} }
                        }
                    }
                    if items.terminal {
                        div { class: "picker-divider" }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitem",
                            onclick: move |_| {
                                on_open.call(false);
                                workspace.change_dock(id, |dock| {
                                    if !dock.is_open(Panel::Terminal) {
                                        dock.toggle(Panel::Terminal);
                                    }
                                });
                            },
                            span { class: "picker-text", {tr!("Open in terminal")} }
                        }
                    }
                }
            }
        }
    }
}

/// The colours of the context bar and its row squares; an index past the end wraps around.
const PALETTE_LEN: usize = 8;

fn palette_class(index: usize) -> String {
    format!("ctx-c{}", index % PALETTE_LEN)
}

#[component]
fn CardHead(id: LiveSessionId, card: DetailsCard, title: String, folded: bool, trailing: Element) -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "env-card-head",
            r#type: "button",
            "aria-expanded": "{!folded}",
            onclick: move |_| workspace.set_details_card_folded(id, card, !folded),
            span { "{title}" }
            span { class: if folded { "env-card-chevron folded" } else { "env-card-chevron" }, CardIcon { kind: "chevron" } }
            {trailing}
        }
    }
}

#[component]
fn ContextBar(segments: Vec<(usize, u16)>, mini: bool) -> Element {
    rsx! {
        div { class: if mini { "ctx-bar mini" } else { "ctx-bar" },
            for (color, per_mille) in segments {
                span { class: "ctx-seg {palette_class(color)}", style: "width: {f32::from(per_mille) / 10.0}%" }
            }
        }
    }
}

#[component]
fn ContextRow(row: ContextRowView) -> Element {
    let workspace = use_workspace();
    let square = match row.color {
        Some(color) => palette_class(color),
        None => "ctx-none".to_string(),
    };
    // The desktop app links these two rows to the page that manages them.
    let link = row_link(&row.name);
    rsx! {
        div { class: "ctx-row",
            span { class: "ctx-square {square}" }
            if let Some(link) = link {
                button {
                    class: "ctx-name ctx-link",
                    r#type: "button",
                    title: match link {
                        RowLink::Connectors => tr!("Manage connectors"),
                        RowLink::Skills => tr!("Manage Skills"),
                    },
                    onclick: move |_| match link {
                        RowLink::Connectors => workspace.open_connectors_page(),
                        RowLink::Skills => workspace.open_skills_page(),
                    },
                    "{row.name}"
                }
            } else {
                span { class: "ctx-name", "{row.name}" }
            }
            span { class: "ctx-tokens", "{row.tokens}" }
            span { class: "ctx-percent", {row.percent.clone().unwrap_or_else(|| "\u{2014}".to_string())} }
        }
    }
}

#[component]
fn ContextFold(title: String, items: Vec<ContextItemView>) -> Element {
    let mut open = use_signal(|| false);
    if items.is_empty() {
        return rsx! {};
    }
    rsx! {
        button { class: "ctx-fold", r#type: "button", "aria-expanded": "{open()}", onclick: move |_| open.set(!open()),
            span { class: if open() { "env-card-chevron" } else { "env-card-chevron folded" }, CardIcon { kind: "chevron" } }
            span { class: "ctx-name", "{title}" }
            span { class: "ctx-tokens", "{items.len()}" }
        }
        if open() {
            for (index, item) in items.iter().enumerate() {
                div { class: "ctx-row ctx-item", key: "{index}",
                    span { class: "ctx-name", title: "{item.label}", "{item.label}" }
                    span { class: "ctx-tokens", "{item.tokens}" }
                }
            }
        }
    }
}

#[component]
fn ContextCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (usage, detail) = {
        let view = view.read();
        (view.context, view.context_detail.clone())
    };
    let card = context_card(usage, detail.as_ref());
    let folded = workspace.details_card_folded(id, DetailsCard::Context);
    let title = tr!("Context");
    rsx! {
        div { class: "env-card",
            CardHead {
                id,
                card: DetailsCard::Context,
                title,
                folded,
                trailing: match (&card, folded) {
                    (Some(card), true) => rsx! {
                        span { class: "ctx-folded-percent", "{card.percent}%" }
                        div { class: "ctx-bar mini", span { class: "ctx-seg ctx-c0", style: "width: {f32::from(card.folded_bar) / 10.0}%" } }
                    },
                    _ => rsx! {},
                },
            }
            if !folded {
                if let Some(card) = card {
                    div { class: "ctx-summary",
                        span { class: "ctx-summary-percent", {tr!("{percent}% used", percent = card.percent)} }
                        span { class: "ctx-summary-label", "{card.used_label}" }
                    }
                    ContextBar { segments: card.bar.iter().map(|segment| (segment.color, segment.per_mille)).collect::<Vec<_>>(), mini: false }
                    div { class: "ctx-rows",
                        for row in card.rows.iter().cloned() {
                            ContextRow { key: "{row.name}", row }
                        }
                    }
                    if !card.deferred.is_empty() {
                        div { class: "ctx-rows ctx-deferred",
                            for row in card.deferred.iter().cloned() {
                                ContextRow { key: "{row.name}", row }
                            }
                        }
                    }
                    ContextFold { title: tr!("MCP tools"), items: card.mcp_tools.clone() }
                    ContextFold { title: tr!("Custom agents"), items: card.agents.clone() }
                } else {
                    div { class: "env-card-note", {tr!("Send a message to see the context usage.")} }
                }
            }
        }
    }
}

/// The rows shown before `더 보기`.
const CONNECTORS_SHOWN: usize = 3;

#[component]
fn ConnectorsCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut all = use_signal(|| false);
    let mut dialog = use_signal(|| false);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let servers = view.read().mcp_servers.clone();
    let folded = workspace.details_card_folded(id, DetailsCard::Connectors);
    let rows = servers.as_deref().map(connector_rows);
    rsx! {
        div { class: "env-card",
            CardHead { id, card: DetailsCard::Connectors, title: tr!("Connectors"), folded, trailing: rsx! {} }
            if !folded {
                match rows {
                    None => rsx! { div { class: "env-card-note", {tr!("Send a message to see the connectors.")} } },
                    Some(rows) if rows.is_empty() => rsx! { div { class: "env-card-note", {tr!("No connectors")} } },
                    Some(rows) => {
                        let hidden = rows.len() > CONNECTORS_SHOWN;
                        let shown = if all() { rows.len() } else { CONNECTORS_SHOWN };
                        rsx! {
                            for row in rows.into_iter().take(shown) {
                                button { class: "env-card-row conn-row", r#type: "button", key: "{row.name}", onclick: move |_| dialog.set(true),
                                    span { class: "conn-icon", "{row.initial}" }
                                    span { class: "env-card-name", "{row.name}" }
                                    if let Some(state) = row.state {
                                        span { class: "conn-state", {connector_state_text(state)} }
                                    }
                                }
                            }
                            if hidden {
                                button { class: "conn-more", r#type: "button", onclick: move |_| all.set(!all()),
                                    if all() { {tr!("Show less|connectors")} } else { {tr!("Show more")} }
                                }
                            }
                        }
                    }
                }
            }
            if dialog() {
                McpServersDialog { id, on_close: move |_| dialog.set(false) }
            }
        }
    }
}

/// The files the session wrote or named that still exist; nothing is drawn while there are none.
#[component]
fn OutputsCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    // A memo, so streaming chunks do not rerun the effect: it follows a finished tool call and a
    // turn that starts or ends.
    let progress = use_memo(move || (workspace.finished_tool_calls(id), workspace.live_view(id).is_some_and(|view| view.read().phase == Phase::Running)));
    use_effect(move || {
        let _ = progress();
        workspace.refresh_outputs(id);
    });
    let paths = workspace.session_outputs(id);
    if paths.is_empty() {
        return rsx! {};
    }
    let folded = workspace.details_card_folded(id, DetailsCard::Outputs);
    rsx! {
        div { class: "env-card",
            CardHead { id, card: DetailsCard::Outputs, title: tr!("Outputs"), folded, trailing: rsx! {} }
            if !folded {
                for path in paths {
                    OutputRow { key: "{path}", id, path }
                }
            }
        }
    }
}

#[component]
fn OutputRow(id: LiveSessionId, path: String) -> Element {
    let workspace = use_workspace();
    let (name, _) = folder_parts(Path::new(&path));
    let target = path.clone();
    rsx! {
        button { class: "env-card-row", r#type: "button", title: "{path}", onclick: move |_| workspace.open_file(id, target.clone()),
            FileTypeIcon { icon: icon_of(&path) }
            span { class: "env-card-name", "{name}" }
        }
    }
}

#[component]
fn ChangesCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    // A memo, so streaming chunks do not rerun the effect.
    let finished = use_memo(move || workspace.finished_tool_calls(id));
    use_effect(move || {
        let _ = finished();
        workspace.refresh_change_counts(id);
    });
    let Some(counts) = workspace.session_change_counts(id) else { return rsx! {} };
    let folded = workspace.details_card_folded(id, DetailsCard::Changes);
    rsx! {
        div { class: "env-card",
            CardHead { id, card: DetailsCard::Changes, title: tr!("Changes"), folded, trailing: rsx! {} }
            if !folded {
                button { class: "env-card-row changes-row", r#type: "button", onclick: move |_| workspace.open_changes_panel(id),
                    CardIcon { kind: "diff" }
                    span { class: "cd-add", "+{counts.added}" }
                    span { class: "cd-del", "\u{2212}{counts.removed}" }
                }
            }
        }
    }
}

/// The background tasks that are running now, with the time each has run and a stop button; a row
/// says how many have ended. A click opens the Background tasks panel. It is there only while a
/// task runs: when the last one ends the card goes.
#[component]
fn TasksCard(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let (running, finished_count) = {
        let view = view.read();
        let lists = task_lists(&view.session.messages, &view.background_tasks, &view.session.finished_tasks);
        let cleared = workspace.task_list_view(id).cleared;
        let finished = lists.finished.iter().filter(|card| !cleared.contains(&clear_key(card))).count();
        (lists.running, finished)
    };
    if running.is_empty() {
        return rsx! {};
    }
    let folded = workspace.details_card_folded(id, DetailsCard::Tasks);
    let open_panel = move |_| workspace.change_dock(id, |dock| dock.open(Panel::Tasks));
    rsx! {
        div { class: "env-card",
            CardHead { id, card: DetailsCard::Tasks, title: tr!("Background tasks"), folded, trailing: rsx! {} }
            if !folded {
                for card in running.iter() {
                    div { key: "{card.id}", class: "env-card-row task-row", role: "button", tabindex: "0", onclick: open_panel,
                        CardIcon { kind: "terminal" }
                        span { class: "task-row-name", title: "{card.description}", "{card.description}" }
                        span { class: "task-row-time", Clock { started: Some(card.seen_at) } }
                        button {
                            class: "task-stop",
                            r#type: "button",
                            title: tr!("Stop"),
                            "aria-label": tr!("Stop"),
                            onclick: {
                                let task_id = card.id.clone();
                                move |event: MouseEvent| {
                                    event.stop_propagation();
                                    workspace.stop_task(id, task_id.clone());
                                }
                            },
                        }
                    }
                }
                if finished_count > 0 {
                    button { class: "env-card-row", r#type: "button", onclick: open_panel,
                        CardIcon { kind: "done" }
                        span { {tr!("{count} done", count = finished_count)} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod output_row_tests {
    use super::*;

    // The row needs a workspace context, which the crate cannot render in a test: the check is on the
    // name the row shows; its `title` is the path itself.
    #[test]
    fn an_output_row_shows_the_file_name() {
        let (name, _) = folder_parts(Path::new("/Users/me/out/report.docx"));
        assert_eq!(name, "report.docx");
    }
}
