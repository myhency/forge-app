use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::CardKind;
use forge_entities::{has_rendered_view, icon_of};
use forge_features::{file_menu, group_hits, parse_files_query, split_hit, run_blocking, use_workspace, Banner, FileAction, FileTree, MenuTarget, FilesQuery, Panel};
use forge_ports::{ContentHit, FilesPort, SearchOptions};

use crate::context_menu::ContextMenu;
use crate::files_editor::{use_editor_channel, use_editor_driver, FromPage};
use crate::files_icons::FileTypeIcon;
use crate::files_kinds::{FileKindView, RenderedView, TabKind};
use crate::files_menu::FilesMenu;
use crate::files_sheet::failed_offers_open;
use crate::files_state::FilesCtx;
use crate::files_tabs::{FilesTabs, TabView};
use crate::panel_frame::PanelFrame;
use forge_shared::tr;

// Typing waits this long before the files are searched.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 100;

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

/// The Files panel: a file tree with a filter field (a filter that starts with `?` searches inside
/// the files), tabs, and an editor that saves by itself. See `files_state.rs` for what the panel does.
#[component]
pub fn FilesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut tree = use_signal(FileTree::default);
    let problem = use_signal(|| None::<String>);
    let mut filter = use_signal(String::new);
    // The query and options a list was found with travel with it: a click uses those, not what is typed now.
    let mut hits = use_signal(|| None::<(String, SearchOptions, Result<Vec<ContentHit>, String>)>);
    let mut options = use_signal(SearchOptions::default);
    // The right click menu: what it is for, the file and where it was asked for.
    let mut menu = use_signal(|| None::<(MenuTarget, String, f64, f64)>);

    // What the panel kept in the workspace while it was away.
    let (stored_tabs, stored_source) = use_hook(|| workspace.files_view(id));
    let handle = use_editor_channel();
    let ctx = FilesCtx {
        workspace,
        id,
        cwd: cwd.clone(),
        handle: handle.clone(),
        tabs: use_signal(|| stored_tabs.clone()),
        kinds: use_signal(HashMap::new),
        docs: use_signal(HashMap::new),
        rendered: use_signal(HashMap::new),
        source: use_signal(|| stored_source.clone()),
        pending: use_signal(HashMap::new),
        confirm: use_signal(|| None),
        queue: use_signal(Vec::new),
        writing: use_signal(HashSet::new),
        fallback: use_signal(HashMap::new),
        ready: use_signal(|| false),
        unavailable: use_signal(|| false),
    };
    // The tabs and the source set go to the store whenever they change, so they outlive the panel.
    {
        let (tabs, source) = (ctx.tabs, ctx.source);
        use_effect(move || {
            workspace.save_files_view(id, tabs.read().clone(), source.read().clone());
        });
    }
    // A panel that comes back shows the selected tab again (the others are read when shown).
    {
        let ctx = ctx.clone();
        use_future(move || {
            let ctx = ctx.clone();
            async move { ctx.show_selected() }
        });
    }
    let host_id = format!("forge-editor-{}", id.0);
    let on_event = {
        let ctx = ctx.clone();
        use_callback(move |event: FromPage| ctx.on_event(event))
    };

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
            let search_options = options();
            match parse_files_query(&typed) {
                FilesQuery::Content(query) if !query.is_empty() => {
                    let query = query.to_string();
                    let query_text = query.clone();
                    let (files, root) = (workspace.files(), cwd.clone());
                    spawn(async move {
                        tokio::time::sleep(SEARCH_DEBOUNCE).await;
                        if filter.peek().as_str() != typed {
                            return;
                        }
                        if let Some(result) = run_blocking(move || files.search(&root, &query, search_options, SEARCH_LIMIT)).await {
                            if filter.peek().as_str() == typed && *options.peek() == search_options {
                                hits.set(Some((query_text, search_options, result.map_err(|error| error.to_string()))));
                            }
                        }
                    });
                }
                _ => hits.set(None),
            }
        });
    }
    // The settings go to the page once it is ready and whenever they change.
    {
        let (handle, ready) = (handle.clone(), ctx.ready);
        use_effect(move || {
            let prefs = workspace.editor_prefs();
            if ready() {
                handle.settings(&prefs);
            }
        });
    }
    // The editor's own sentences go to the page once it is ready and whenever the language changes.
    {
        let (handle, ready) = (handle.clone(), ctx.ready);
        use_effect(move || {
            let _ = forge_shared::i18n::current();
            if ready() {
                handle.strings();
            }
        });
    }
    // A click in the chat asked for a file: show it (the request is read when the panel mounts, too).
    {
        let ctx = ctx.clone();
        use_effect(move || {
            if let Some(path) = workspace.file_request(id) {
                workspace.clear_file_request(id);
                ctx.open(path);
            }
        });
    }
    // The comments of every open file go to the page whenever they or the tabs change.
    {
        let ctx = ctx.clone();
        use_effect(move || {
            let cards = workspace.cards(id);
            let ready = *ctx.ready.read();
            let keys: Vec<String> = ctx.tabs.read().tabs().iter().map(|tab| tab.path.clone()).collect();
            if !ready {
                return;
            }
            for key in keys {
                let path = ctx.cwd.join(&key).to_string_lossy().to_string();
                let list: Vec<_> = cards.iter().filter(|c| c.kind == CardKind::Comment && c.path == path).cloned().collect();
                ctx.handle.comments(&key, &list);
            }
        });
    }
    // A comment card in the message box was clicked: open its file and show its box.
    {
        let ctx = ctx.clone();
        use_effect(move || {
            let Some(card_id) = workspace.comment_focus(id) else { return };
            workspace.clear_comment_focus(id);
            let Some(card) = workspace.cards(id).into_iter().find(|c| c.id == card_id) else { return };
            let Ok(relative) = std::path::Path::new(&card.path).strip_prefix(&ctx.cwd) else { return };
            let key = relative.to_string_lossy().to_string();
            // Keys use `/`; a Windows path comes with `\`, which on other systems is part of a name.
            #[cfg(windows)]
            let key = key.replace('\\', "/");
            ctx.open(key.clone());
            ctx.handle.focus_comment(&key, card_id);
        });
    }
    // A tool call of the session finished: tabs with no edits show the file as it is now. A memo, so
    // a streamed reply (which rewrites the view per chunk) does not rerun it.
    {
        let ctx = ctx.clone();
        let finished = use_memo(move || workspace.finished_tool_calls(id));
        let mut seen = use_signal(|| *finished.peek());
        use_effect(move || {
            let now = finished();
            if now != *seen.peek() {
                seen.set(now);
                ctx.refresh_all();
            }
        });
    }
    use_editor_driver(host_id.clone(), &handle, on_event);

    let prefs = workspace.editor_prefs();
    let selected = ctx.tabs.read().selected().map(str::to_string);
    let kind = selected.as_ref().and_then(|path| ctx.kinds.read().get(path).cloned());
    let banner = selected.as_ref().and_then(|path| ctx.docs.read().get(path).and_then(|doc| doc.banner.clone()));
    let tab_views: Vec<TabView> = ctx.tabs.read().tabs().iter().map(|tab| TabView { path: tab.path.clone(), preview: tab.preview }).collect();
    let unavailable = *ctx.unavailable.read();
    let is_text = kind.as_ref().is_some_and(TabKind::shows_editor);
    let rendered_selected = is_text && selected.as_deref().is_some_and(has_rendered_view);
    let source_on = selected.as_ref().is_some_and(|path| ctx.source.read().contains(path));
    let shows_editor = is_text && !unavailable && (!rendered_selected || source_on);

    let tools = rsx! {
        button {
            class: if prefs.tree { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: tr!("File tree"),
            "aria-label": "File tree",
            "aria-pressed": "{prefs.tree}",
            onclick: move |_| workspace.update_editor_prefs(|prefs| prefs.tree = !prefs.tree),
            "☰"
        }
        if rendered_selected {
            button {
                class: "icon-button files-source-toggle",
                r#type: "button",
                title: if source_on { tr!("Show rendered") } else { tr!("Show source") },
                "aria-label": if source_on { tr!("Show rendered") } else { tr!("Show source") },
                onclick: {
                    let (ctx, path) = (ctx.clone(), selected.clone().unwrap_or_default());
                    move |_| ctx.toggle_source(&path)
                },
                if source_on { "👁" } else { "</>" }
            }
        }
        FilesMenu {}
    };

    let typed = filter();
    let query = parse_files_query(&typed);
    let list = if let FilesQuery::Content(text) = query {
        match hits() {
            _ if text.is_empty() => rsx! { div { class: "dock-note", {tr!("Type the text to search for after ?")} } },
            None => rsx! { div { class: "dock-note", {tr!("Searching…")} } },
            Some((_, _, Err(message))) => rsx! { div { class: "dock-note error", "{message}" } },
            Some((_, _, Ok(found))) if found.is_empty() => rsx! { div { class: "dock-note", {tr!("No matches")} } },
            Some((found_query, found_options, Ok(found))) => rsx! {
                div { class: "files-list",
                    for group in group_hits(&found) {
                        div { key: "{group.path}", class: "files-hit-group",
                            div { class: "files-hit-file", "{group.path}" }
                            for hit in group.hits.iter() {
                                {
                                    let (before, matched, after) = split_hit(hit);
                                    let (ctx, path, line, needle, opts) = (ctx.clone(), hit.path.clone(), hit.line, found_query.clone(), found_options);
                                    rsx! {
                                        button {
                                            key: "{hit.line}",
                                            class: "files-hit",
                                            r#type: "button",
                                            onclick: move |_| ctx.find_in_file(&path, &needle, opts, line),
                                            span { class: "files-hit-line", "{hit.line}" }
                                            span { class: "files-hit-text", "{before}", mark { "{matched}" }, "{after}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            },
        }
    } else if let Some(message) = problem() {
        rsx! { div { class: "dock-note error", "{message}" } }
    } else if !tree.read().is_loaded("") {
        rsx! { div { class: "dock-note", {tr!("Loading…")} } }
    } else {
        let rows = tree.read().rows(&typed);
        if rows.is_empty() {
            let note = if typed.trim().is_empty() { tr!("Folder is empty") } else { tr!("No files match") };
            rsx! { div { class: "dock-note", "{note}" } }
        } else {
            let current = selected.clone();
            rsx! {
                ul { class: "files-list",
                    for row in rows {
                        {
                            let indent = format!("padding-left: {}px", 6 + row.depth * 14);
                            let mark = if !row.is_dir { " " } else if row.expanded { "▾" } else { "▸" };
                            let is_selected = current.as_deref() == Some(row.path.as_str());
                            let path = row.path.clone();
                            let (is_dir, cwd, ctx) = (row.is_dir, cwd.clone(), ctx.clone());
                            rsx! {
                                li { key: "{row.path}",
                                    button {
                                        class: if is_selected { "files-row selected" } else { "files-row" },
                                        r#type: "button",
                                        style: "{indent}",
                                        "data-path": "{row.path}",
                                        oncontextmenu: {
                                            let path = path.clone();
                                            move |event: MouseEvent| {
                                                event.prevent_default();
                                                if !is_dir {
                                                    let point = event.client_coordinates();
                                                    menu.set(Some((MenuTarget::Tree, path.clone(), point.x, point.y)));
                                                }
                                            }
                                        },
                                        onclick: move |_| {
                                            if is_dir {
                                                if tree.write().toggle(&path) {
                                                    spawn(load_dir(workspace.files(), cwd.clone(), path.clone(), tree, problem));
                                                }
                                            } else {
                                                ctx.open(path.clone());
                                            }
                                        },
                                        if is_dir {
                                            span { class: "files-mark", "{mark}" }
                                        } else {
                                            FileTypeIcon { icon: icon_of(&row.path) }
                                        }
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

    let banner_view = match banner {
        Some(Banner::Conflict) => {
            let (cancel_ctx, overwrite_ctx, key) = (ctx.clone(), ctx.clone(), selected.clone().unwrap_or_default());
            let cancel_key = key.clone();
            rsx! {
                div { class: "files-banner", role: "alert",
                    span { "{Banner::Conflict.message()}" }
                    button { r#type: "button", onclick: move |_| cancel_ctx.reload(cancel_key.clone()), {tr!("Cancel")} }
                    button { r#type: "button", onclick: move |_| overwrite_ctx.save_from_editor(&key, true), {tr!("Overwrite")} }
                }
            }
        }
        Some(Banner::SaveFailed(reason)) => {
            let (retry_ctx, dismiss_ctx, key) = (ctx.clone(), ctx.clone(), selected.clone().unwrap_or_default());
            let dismiss_key = key.clone();
            rsx! {
                div { class: "files-banner", role: "alert",
                    span { {Banner::SaveFailed(reason.clone()).message()} }
                    button { r#type: "button", onclick: move |_| retry_ctx.save_from_editor(&key, false), {tr!("Retry")} }
                    button { r#type: "button", onclick: move |_| dismiss_ctx.dismiss_banner(&dismiss_key), {tr!("Dismiss")} }
                }
            }
        }
        None => rsx! {},
    };

    let notice = kind.as_ref().and_then(TabKind::notice);
    let confirm_view = match ctx.confirm.read().clone() {
        Some((path, _)) => {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            let (save, discard, cancel) = (ctx.clone(), ctx.clone(), ctx.clone());
            rsx! {
                div { class: "files-banner", role: "alertdialog",
                    span { {tr!("Save changes to {name}?", name = name)} }
                    button { r#type: "button", onclick: move |_| save.confirm_save(), {tr!("Save")} }
                    button { r#type: "button", onclick: move |_| discard.confirm_discard(), {tr!("Don't save")} }
                    button { r#type: "button", onclick: move |_| cancel.confirm_cancel(), {tr!("Cancel")} }
                }
            }
        }
        None => rsx! {},
    };

    let content = match (&selected, &kind) {
        (None, _) => rsx! { div { class: "dock-note", {tr!("Pick a file")} } },
        (Some(_), None | Some(TabKind::Loading)) => rsx! { div { class: "dock-note", {tr!("Loading…")} } },
        (Some(path), Some(TabKind::Failed(message))) => {
            let open_ctx = ctx.clone();
            let open_path = path.clone();
            rsx! {
                div { class: "dock-note error", "{message}" }
                if failed_offers_open(path) {
                    div { class: "files-kind-buttons",
                        button {
                            class: "files-kind-button",
                            r#type: "button",
                            onclick: move |_| open_ctx.run_action(FileAction::OpenDefault, &open_path),
                            {tr!("Open in the default program")}
                        }
                    }
                }
            }
        }
        (Some(path), Some(TabKind::Text { .. })) if unavailable => match ctx.fallback.read().get(path).cloned() {
            Some((html, numbers)) => rsx! {
                div { class: "dock-note", {tr!("The editor could not be loaded, so this file is shown read only.")} }
                div { class: "changes-diff",
                    div { class: "file-viewer", "aria-label": "{path}",
                        pre { class: "file-gutter", "{numbers}" }
                        pre { class: "file-code", dangerous_inner_html: "{html}" }
                    }
                }
            },
            None => rsx! { div { class: "dock-note", {tr!("Loading…")} } },
        },
        (Some(path), Some(TabKind::Text { .. })) if rendered_selected && !source_on => match ctx.rendered.read().get(path).cloned() {
            Some(rendered) => rsx! { RenderedView { key: "{path}", rendered, path: path.clone() } },
            None => rsx! { div { class: "dock-note", {tr!("Loading…")} } },
        },
        (Some(_), Some(TabKind::Text { .. })) => rsx! {},
        (Some(path), Some(other)) => {
            let (anyway_ctx, reveal_ctx, open_ctx) = (ctx.clone(), ctx.clone(), ctx.clone());
            let (anyway_path, reveal_path, open_path) = (path.clone(), path.clone(), path.clone());
            rsx! {
                FileKindView {
                    kind: other.clone(),
                    path: path.clone(),
                    on_anyway: move |_| anyway_ctx.show_anyway(anyway_path.clone()),
                    on_reveal: move |_| reveal_ctx.reveal(&reveal_path),
                    on_open: move |_| open_ctx.run_action(FileAction::OpenDefault, &open_path),
                }
            }
        }
    };

    // `Show in file tree`: the folders above the file are read and opened one after the other, the
    // filter is cleared, the tree is turned on, and the row is scrolled into view.
    let show_in_tree = {
        let (cwd, ctx) = (cwd.clone(), ctx.clone());
        move |path: String| {
            workspace.update_editor_prefs(|prefs| prefs.tree = true);
            filter.set(String::new());
            let (cwd, ctx) = (cwd.clone(), ctx.clone());
            spawn(async move {
                // The root and every folder above the file are listed again: a file Claude made after
                // a folder was read is not in the tree yet.
                let mut dirs = vec![String::new()];
                dirs.extend(FileTree::reveal_dirs(&path));
                for dir in dirs {
                    if !dir.is_empty() {
                        tree.write().open_dir(&dir);
                    }
                    load_dir(workspace.files(), cwd.clone(), dir, tree, problem).await;
                }
                // The row with the grey band is the selected tab's: select the file's tab.
                if ctx.tabs.peek().paths().contains(&path) {
                    ctx.select(&path);
                }
                // The rows are drawn after this task yields: look for the row a few frames.
                let literal = serde_json::to_string(&path).unwrap_or_default();
                document::eval(&format!(
                    "(function(p){{let n=0;const go=()=>{{const el=[...document.querySelectorAll('.pane.focused [data-path]')].find(e=>e.dataset.path===p);if(el){{el.scrollIntoView({{block:'center'}});}}else if(n++<30){{requestAnimationFrame(go);}}}};go();}})({literal});"
                ));
            });
        }
    };

    let menu_view = match menu() {
        Some((target, path, x, y)) => {
            let (ctx, run_path, mut show_in_tree) = (ctx.clone(), path.clone(), show_in_tree.clone());
            rsx! {
                ContextMenu {
                    entries: file_menu(target),
                    x,
                    y,
                    on_close: move |_| menu.set(None),
                    on_select: move |action: FileAction| {
                        menu.set(None);
                        if action == FileAction::ShowInTree {
                            show_in_tree(run_path.clone());
                        } else {
                            ctx.run_action(action, &run_path);
                        }
                    },
                }
            }
        }
        None => rsx! {},
    };

    rsx! {
        PanelFrame { id, panel: Panel::Files, tools,
            {menu_view}
            div { class: "files-filter",
                input {
                    class: "files-filter-input",
                    r#type: "text",
                    placeholder: tr!("Filter, or start with ? to search inside files"),
                    value: "{filter}",
                    oninput: move |event| filter.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                }
                if matches!(query, FilesQuery::Content(_)) {
                    div { class: "files-search-toggles",
                        for (title, label, on, flip) in [
                            (tr!("Match case"), "Aa", options().match_case, 0u8),
                            (tr!("Whole word"), "ab", options().whole_word, 1),
                            (tr!("Regex"), ".*", options().regex, 2),
                        ] {
                            button {
                                key: "{title}",
                                class: if on { "files-toggle on" } else { "files-toggle" },
                                r#type: "button",
                                title: "{title}",
                                "aria-pressed": "{on}",
                                onclick: move |_| options.with_mut(|o| match flip {
                                    0 => o.match_case = !o.match_case,
                                    1 => o.whole_word = !o.whole_word,
                                    _ => o.regex = !o.regex,
                                }),
                                "{label}"
                            }
                        }
                        button {
                            class: "files-toggle",
                            r#type: "button",
                            title: tr!("Clear"),
                            "aria-label": "Clear",
                            onclick: move |_| filter.set(String::new()),
                            "×"
                        }
                    }
                }
            }
            div { class: "changes-body",
                if prefs.tree {
                    div { class: "files-side", {list} }
                }
                div { class: "files-main",
                    FilesTabs {
                        tabs: tab_views,
                        selected: selected.clone(),
                        on_select: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.select(&path)
                        },
                        on_close: {
                            let ctx = ctx.clone();
                            move |path: String| ctx.close(path)
                        },
                        on_reorder: {
                            let ctx = ctx.clone();
                            move |(from, to): (String, String)| ctx.reorder(&from, &to)
                        },
                        on_menu: move |(path, preview, x, y): (String, bool, f64, f64)| {
                            menu.set(Some((MenuTarget::Tab { preview }, path, x, y)));
                        },
                    }
                    {banner_view}
                    {confirm_view}
                    if let Some(text) = notice {
                        div { class: "dock-note", "{text}" }
                    }
                    div { class: "files-view",
                        div { id: "{host_id}", class: if shows_editor { "files-editor" } else { "files-editor hidden" } }
                        {content}
                    }
                }
            }
        }
    }
}
