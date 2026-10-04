use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{code_html, is_markdown, language_of, render_markdown};
use forge_features::{parse_files_query, run_blocking, use_workspace, FileTree, FilesQuery, Panel};
use forge_ports::{ContentHit, FileContent, FilesError, FilesPort};

use crate::panel_frame::PanelFrame;

// Typing waits this long before the files are searched.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 100;
// Highlighting a file larger than this would keep the UI busy, so it is shown as plain text.
const HIGHLIGHT_LIMIT: usize = 200_000;

async fn load_dir(files: Arc<dyn FilesPort>, root: PathBuf, dir: String, mut tree: Signal<FileTree>, mut problem: Signal<Option<String>>) {
    let key = dir.clone();
    match run_blocking(move || files.list(&root, &dir)).await {
        Some(Ok(entries)) => tree.write().set_children(&key, entries),
        // Only the root failing leaves nothing to show. A folder that cannot be read (a link out
        // of the session folder, no permission) just shows as empty.
        Some(Err(error)) if key.is_empty() => problem.set(Some(error.to_string())),
        Some(Err(_)) => tree.write().set_children(&key, Vec::new()),
        None => {}
    }
}

/// A file tree with a filter field, and a read only viewer for the picked file. A filter that
/// starts with `?` searches inside the files instead.
#[component]
pub fn FilesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut tree = use_signal(FileTree::default);
    let mut problem = use_signal(|| None::<String>);
    let mut filter = use_signal(String::new);
    let mut tree_open = use_signal(|| true);
    let mut opened = use_signal(|| None::<String>);
    let mut content = use_signal(|| None::<Result<FileContent, FilesError>>);
    let mut hits = use_signal(|| None::<Vec<ContentHit>>);
    let mut source = use_signal(|| false);

    {
        let cwd = cwd.clone();
        use_future(move || {
            let cwd = cwd.clone();
            async move { load_dir(workspace.files(), cwd, String::new(), tree, problem).await }
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let typed = filter();
            match parse_files_query(&typed) {
                FilesQuery::Content(query) if !query.is_empty() => {
                    let query = query.to_string();
                    let (files, root) = (workspace.files(), cwd.clone());
                    spawn(async move {
                        tokio::time::sleep(SEARCH_DEBOUNCE).await;
                        if filter.peek().as_str() != typed {
                            return;
                        }
                        if let Some(Ok(found)) = run_blocking(move || files.search(&root, &query, SEARCH_LIMIT)).await {
                            if filter.peek().as_str() == typed {
                                hits.set(Some(found));
                            }
                        }
                    });
                }
                _ => hits.set(None),
            }
        });
    }

    let open_file = {
        let cwd = cwd.clone();
        move |path: String| {
            opened.set(Some(path.clone()));
            source.set(false);
            content.set(None);
            let (files, root) = (workspace.files(), cwd.clone());
            spawn(async move {
                let wanted = path.clone();
                let Some(result) = run_blocking(move || files.read(&root, &path)).await else { return };
                if opened.peek().as_deref() == Some(wanted.as_str()) {
                    content.set(Some(result));
                }
            });
        }
    };

    // A click in the chat asked for a file: show it (the request is read when the panel mounts, too).
    {
        let mut open_file = open_file.clone();
        use_effect(move || {
            if let Some(path) = workspace.file_request(id) {
                workspace.clear_file_request(id);
                open_file(path);
            }
        });
    }

    let tools = rsx! {
        button {
            class: if tree_open() { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: "File tree",
            "aria-label": "File tree",
            "aria-pressed": "{tree_open()}",
            onclick: move |_| tree_open.set(!tree_open()),
            "☰"
        }
        if opened().as_deref().is_some_and(is_markdown) {
            button {
                class: "icon-button files-source-toggle",
                r#type: "button",
                title: if source() { "Show rendered" } else { "Show source" },
                "aria-label": if source() { "Show rendered" } else { "Show source" },
                onclick: move |_| source.set(!source()),
                if source() { "👁" } else { "</>" }
            }
        }
    };

    let typed = filter();
    let query = parse_files_query(&typed);
    let list = if let FilesQuery::Content(text) = query {
        match hits() {
            _ if text.is_empty() => rsx! { div { class: "dock-note", "Type the text to search for after ?" } },
            None => rsx! { div { class: "dock-note", "Searching…" } },
            Some(found) if found.is_empty() => rsx! { div { class: "dock-note", "No matches" } },
            Some(found) => rsx! {
                ul { class: "files-list",
                    for (index, hit) in found.iter().enumerate() {
                        li { key: "{index}",
                            button {
                                class: "files-hit",
                                r#type: "button",
                                onclick: {
                                    let path = hit.path.clone();
                                    let mut open_file = open_file.clone();
                                    move |_| open_file(path.clone())
                                },
                                span { class: "files-hit-path", "{hit.path}:{hit.line}" }
                                span { class: "files-hit-text", "{hit.text}" }
                            }
                        }
                    }
                }
            },
        }
    } else if let Some(message) = problem() {
        rsx! { div { class: "dock-note error", "{message}" } }
    } else if !tree.read().is_loaded("") {
        rsx! { div { class: "dock-note", "Loading…" } }
    } else {
        let rows = tree.read().rows(&typed);
        if rows.is_empty() {
            let note = if typed.trim().is_empty() { "Folder is empty" } else { "No files match" };
            rsx! { div { class: "dock-note", "{note}" } }
        } else {
            let current = opened();
            rsx! {
                ul { class: "files-list",
                    for row in rows {
                        {
                            let indent = format!("padding-left: {}px", 6 + row.depth * 14);
                            let mark = if !row.is_dir { " " } else if row.expanded { "▾" } else { "▸" };
                            let selected = current.as_deref() == Some(row.path.as_str());
                            let path = row.path.clone();
                            let (is_dir, cwd) = (row.is_dir, cwd.clone());
                            let mut open_file = open_file.clone();
                            rsx! {
                                li { key: "{row.path}",
                                    button {
                                        class: if selected { "files-row selected" } else { "files-row" },
                                        r#type: "button",
                                        style: "{indent}",
                                        onclick: move |_| {
                                            if is_dir {
                                                if tree.write().toggle(&path) {
                                                    spawn(load_dir(workspace.files(), cwd.clone(), path.clone(), tree, problem));
                                                }
                                            } else {
                                                open_file(path.clone());
                                            }
                                        },
                                        span { class: "files-mark", "{mark}" }
                                        span { "{row.name}" }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    // Highlighting runs when a file is loaded, not on every render (typing in the filter renders).
    let shown = use_memo(move || match (opened(), content()) {
        (Some(path), Some(Ok(FileContent::Text(text)))) => {
            let language = if text.len() > HIGHLIGHT_LIMIT { "" } else { language_of(&path) };
            let numbers = (1..=text.lines().count().max(1)).map(|n| n.to_string()).collect::<Vec<_>>().join("\n");
            let html = code_html(language, &text);
            let rendered = is_markdown(&path).then(|| render_markdown(&text));
            Some((path, html, numbers, rendered))
        }
        _ => None,
    });
    let viewer = match (opened(), content()) {
        (None, _) => rsx! { div { class: "dock-note", "Pick a file" } },
        (Some(_), None) => rsx! { div { class: "dock-note", "Loading…" } },
        (Some(_), Some(Err(error))) => rsx! { div { class: "dock-note error", "{error}" } },
        (Some(_), Some(Ok(FileContent::Binary | FileContent::TooLarge))) => rsx! { div { class: "dock-note", "Cannot show this file" } },
        (Some(_), Some(Ok(FileContent::Text(_)))) => match shown() {
            Some((path, _, _, Some(markdown))) if !source() => rsx! {
                div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{markdown}" }
            },
            Some((path, html, numbers, _)) => rsx! {
                div { class: "file-viewer", "aria-label": "{path}",
                    pre { class: "file-gutter", "{numbers}" }
                    pre { class: "file-code", dangerous_inner_html: "{html}" }
                }
            },
            None => rsx! { div { class: "dock-note", "Loading…" } },
        },
    };

    rsx! {
        PanelFrame { id, panel: Panel::Files, tools,
            div { class: "files-filter",
                input {
                    class: "files-filter-input",
                    r#type: "text",
                    placeholder: "Filter, or start with ? to search inside files",
                    value: "{filter}",
                    oninput: move |event| filter.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                }
            }
            div { class: "changes-body",
                if tree_open() {
                    div { class: "files-side", {list} }
                }
                div { class: "changes-diff", {viewer} }
            }
        }
    }
}
