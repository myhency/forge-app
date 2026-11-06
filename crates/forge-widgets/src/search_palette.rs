use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind, IconSize};
use forge_domain::split_at_match;
use forge_entities::relative_time;
use forge_features::{use_routines, use_workspace, SearchRow};
use std::path::PathBuf;
use time::OffsetDateTime;

use crate::sidebar::project_label_in;

/// The palette's tabs, as the desktop app has them (observed 2026-11-06: 전체, 아티팩트, 세션, Code,
/// 프로젝트, 예약됨). Forge has no artifacts and only Code sessions, so 아티팩트 and Code are left out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    All,
    Sessions,
    Projects,
    Scheduled,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::All, Tab::Sessions, Tab::Projects, Tab::Scheduled];

    fn label(self) -> String {
        match self {
            Tab::All => tr!("All").to_string(),
            Tab::Sessions => tr!("Sessions").to_string(),
            Tab::Projects => tr!("Projects").to_string(),
            Tab::Scheduled => tr!("Scheduled").to_string(),
        }
    }
}

/// One row the palette can open.
#[derive(Debug, Clone, PartialEq)]
enum Entry {
    Session(SearchRow),
    Project { path: PathBuf, label: String },
    Routine { id: String, name: String },
    NewSession,
}

/// The search palette: a box over the dimmed window. Its tabs list sessions (their title or message
/// text matching what is typed), project folders and scheduled tasks; with nothing typed, the 전체
/// tab shows the recent sessions and a `작업` row for a new session, as the desktop app does. Up and
/// Down move, Left and Right change the tab, Enter opens, Escape or a click outside closes. It draws
/// nothing while closed; the body is a separate component so its highlight starts at the first row
/// every time it opens.
#[component]
pub fn SearchPalette() -> Element {
    let workspace = use_workspace();
    if !workspace.search_open() {
        return rsx! {};
    }
    rsx! { PaletteBody {} }
}

#[component]
fn PaletteBody() -> Element {
    let workspace = use_workspace();
    let routines = use_routines();
    let mut highlight = use_signal(|| 0usize);
    let mut tab = use_signal(|| Tab::All);
    // `작성` mode (Tab): the box takes a message that starts a new session.
    let mut compose = use_signal(|| false);
    let mut draft = use_signal(String::new);
    // The highlighted session row's `⋯` menu (the desktop app's, also `작업 ⌥↵`).
    let mut row_menu = use_signal(|| false);
    let query = workspace.search_query();
    let needle = query.trim().to_lowercase();
    let matches = |text: &str| needle.is_empty() || text.to_lowercase().contains(&needle);
    let scratch = workspace.scratch_root();

    let sessions: Vec<Entry> = workspace.search_rows().into_iter().map(Entry::Session).collect();
    // Two folders with the same name are told apart as in the sidebar (`app (a)`, `app (b)`).
    let project_paths: Vec<PathBuf> = workspace.new_session_projects().into_iter().map(|project| project.path).collect();
    let labels = crate::sidebar::project_labels(&project_paths, scratch.as_deref());
    let projects: Vec<Entry> = project_paths
        .into_iter()
        .zip(labels)
        .map(|(path, label)| Entry::Project { label, path })
        .filter(|entry| matches!(entry, Entry::Project { label, .. } if matches(label)))
        .collect();
    let scheduled: Vec<Entry> = routines
        .routines()
        .into_iter()
        .filter(|routine| matches(&routine.name))
        .map(|routine| Entry::Routine { id: routine.id, name: routine.name })
        .collect();
    // Sections in order: (heading, entries). The 전체 tab with nothing typed is the desktop's
    // "최근 항목" and "작업"; with a query it lists every kind that matches.
    let sections: Vec<(String, Vec<Entry>)> = match tab() {
        Tab::All if needle.is_empty() => vec![
            (tr!("Recent items").to_string(), sessions.into_iter().take(8).collect()),
            (tr!("Actions").to_string(), vec![Entry::NewSession]),
        ],
        Tab::All => vec![(tr!("Sessions").to_string(), sessions), (tr!("Projects").to_string(), projects), (tr!("Scheduled").to_string(), scheduled)],
        Tab::Sessions => vec![(String::new(), sessions)],
        Tab::Projects => vec![(String::new(), projects)],
        Tab::Scheduled => vec![(String::new(), scheduled)],
    };
    let sections: Vec<(String, Vec<Entry>)> = sections.into_iter().filter(|(_, entries)| !entries.is_empty()).collect();
    let flat: Vec<Entry> = sections.iter().flat_map(|(_, entries)| entries.iter().cloned()).collect();
    let count = flat.len();
    let selected = highlight().min(count.saturating_sub(1));
    let now = OffsetDateTime::now_utc();

    let open = move |entry: &Entry| match entry {
        Entry::Session(row) => workspace.open_search_row(row),
        Entry::Project { path, .. } => {
            workspace.close_search();
            workspace.show_new_session_in(path.clone());
        }
        Entry::Routine { id, .. } => {
            workspace.close_search();
            workspace.show_routines();
            routines.show_detail(id.clone());
        }
        Entry::NewSession => {
            workspace.close_search();
            workspace.show_new_session();
        }
    };
    let mut switch_tab = move |step: isize| {
        let index = Tab::ALL.iter().position(|item| *item == tab()).unwrap_or(0) as isize;
        let next = (index + step).rem_euclid(Tab::ALL.len() as isize) as usize;
        tab.set(Tab::ALL[next]);
        highlight.set(0);
    };

    let mut position = 0usize;
    rsx! {
        div { class: "palette-backdrop", onclick: move |_| workspace.close_search(),
            div {
                class: "palette",
                onclick: move |event| {
                    event.stop_propagation();
                    row_menu.set(false);
                },
                div { class: "palette-input-row",
                    if compose() {
                        input {
                            class: "palette-input",
                            r#type: "text",
                            value: "{draft}",
                            placeholder: tr!("Type a message..."),
                            spellcheck: "false",
                            onmounted: move |mounted| async move {
                                let _ = mounted.set_focus(true).await;
                            },
                            oninput: move |event| draft.set(event.value()),
                            onkeydown: move |event: KeyboardEvent| match event.key() {
                                Key::Escape => {
                                    event.stop_propagation();
                                    workspace.close_search();
                                }
                                Key::Tab => {
                                    event.prevent_default();
                                    compose.set(false);
                                }
                                Key::Enter if !event.is_composing() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    workspace.start_from_palette(draft.peek().clone());
                                }
                                _ => {}
                            },
                        }
                    } else {
                    input {
                        class: "palette-input",
                        r#type: "text",
                        value: "{query}",
                        placeholder: tr!("Search"),
                        spellcheck: "false",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| {
                            highlight.set(0);
                            workspace.set_search_query(event.value());
                        },
                        onkeydown: {
                            let flat = flat.clone();
                            move |event: KeyboardEvent| match event.key() {
                                Key::Escape => {
                                    event.stop_propagation();
                                    workspace.close_search();
                                }
                                Key::ArrowDown => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    highlight.set((selected + 1).min(count.saturating_sub(1)));
                                }
                                Key::ArrowUp => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    highlight.set(selected.saturating_sub(1));
                                }
                                // The desktop app's `작성 Tab`: write a message for a new session instead.
                                Key::Tab => {
                                    event.prevent_default();
                                    compose.set(true);
                                }
                                // The desktop app's `유형 변경 ←→`.
                                Key::ArrowLeft if !event.is_composing() => {
                                    event.prevent_default();
                                    switch_tab(-1);
                                }
                                Key::ArrowRight if !event.is_composing() => {
                                    event.prevent_default();
                                    switch_tab(1);
                                }
                                // The desktop app's `작업 ⌥↵`: the highlighted session's menu.
                                Key::Enter if !event.is_composing() && event.modifiers().alt() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    if matches!(flat.get(selected), Some(Entry::Session(_))) {
                                        row_menu.set(true);
                                    }
                                }
                                // Enter that ends an IME composition confirms the text, not the row.
                                Key::Enter if !event.is_composing() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    if let Some(entry) = flat.get(selected) {
                                        open(entry);
                                    }
                                }
                                _ => {}
                            }
                        },
                    }
                    }
                    button {
                        class: "palette-close",
                        title: tr!("Close"),
                        onclick: move |_| workspace.close_search(),
                        Icon { kind: IconKind::Close, size: IconSize::Small }
                    }
                }
                if !compose() {
                div { class: "palette-tabs", role: "tablist",
                    for item in Tab::ALL {
                        button {
                            key: "{item.label()}",
                            class: if item == tab() { "palette-tab active" } else { "palette-tab" },
                            r#type: "button",
                            role: "tab",
                            "aria-selected": "{item == tab()}",
                            onclick: move |_| {
                                tab.set(item);
                                highlight.set(0);
                            },
                            "{item.label()}"
                        }
                    }
                }
                div { class: "palette-results",
                    for (heading, entries) in sections.iter() {
                        if !heading.is_empty() {
                            div { key: "h-{heading}", class: "palette-heading", "{heading}" }
                        }
                        for entry in entries.iter() {
                            {
                                let index = position;
                                position += 1;
                                let entry = entry.clone();
                                let key = match &entry {
                                    Entry::Session(row) => row.key.dom_key(),
                                    Entry::Project { path, .. } => format!("p-{}", path.display()),
                                    Entry::Routine { id, .. } => format!("r-{id}"),
                                    Entry::NewSession => "new".to_string(),
                                };
                                let clicked = entry.clone();
                                rsx! {
                                    div {
                                        key: "{key}",
                                        class: if index == selected { "palette-row selected" } else { "palette-row" },
                                        // While a row's menu is open the pointer crosses other rows on its way into the menu.
                                        onmouseenter: move |_| {
                                            if !row_menu() {
                                                highlight.set(index);
                                            }
                                        },
                                        onclick: move |_| open(&clicked),
                                        {row_body(&entry, &query, now, scratch.as_deref())}
                                        if index == selected {
                                            span { class: "palette-enter",
                                                if let Entry::Session(row) = &entry {
                                                    button {
                                                        class: "icon-button palette-more",
                                                        r#type: "button",
                                                        title: tr!("Actions"),
                                                        onclick: move |event| {
                                                            event.stop_propagation();
                                                            row_menu.set(!row_menu());
                                                        },
                                                        Icon { kind: IconKind::MoreHorizontal, size: IconSize::Small }
                                                    }
                                                    if row_menu() {
                                                        PaletteRowMenu { row: row.clone(), on_close: move |_| row_menu.set(false) }
                                                    }
                                                }
                                                "↵" // icon-ok: a key cap, as in the desktop app
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    if count == 0 {
                        div { class: "state-empty", {tr!("No results")} }
                    }
                }
                }
                if compose() {
                    div { class: "palette-footer",
                        span { {tr!("Send")} span { class: "palette-key", "↵" } } // icon-ok: a key cap
                        span { {tr!("Search")} span { class: "palette-key", "Tab" } }
                    }
                } else {
                div { class: "palette-footer",
                    span { {tr!("Close")} span { class: "palette-key", "Esc" } }
                    span { {tr!("Change type")} span { class: "palette-key", "←" } span { class: "palette-key", "→" } } // icon-ok: key caps
                    span { {tr!("Open")} span { class: "palette-key", "↵" } } // icon-ok: a key cap
                    span { {tr!("Actions")} span { class: "palette-key", "⌥" } span { class: "palette-key", "↵" } } // icon-ok: key caps
                    span { {tr!("Compose")} span { class: "palette-key", "Tab" } }
                }
                }
            }
        }
    }
}

fn row_body(entry: &Entry, query: &str, now: OffsetDateTime, scratch: Option<&std::path::Path>) -> Element {
    match entry {
        Entry::Session(row) => rsx! {
            span { class: "palette-icon", Icon { kind: IconKind::Code, size: IconSize::Small } }
            div { class: "palette-title",
                match split_at_match(&row.title, query) {
                    Some((before, matched, after)) => rsx! {
                        "{before}"
                        b { "{matched}" }
                        "{after}"
                    },
                    None => rsx! { "{row.title}" },
                }
                span { class: "palette-project", "{project_label_in(scratch, &row.project.path)}" }
            }
            span { class: "palette-date", {row.updated_at.as_deref().map(|at| relative_time(at, now)).unwrap_or_default()} }
            if let Some(snippet) = &row.snippet {
                div { class: "palette-snippet",
                    "{snippet.before}"
                    b { "{snippet.matched}" }
                    "{snippet.after}"
                }
            }
        },
        Entry::Project { path, label } => rsx! {
            span { class: "palette-icon", Icon { kind: IconKind::Folder, size: IconSize::Small } }
            div { class: "palette-title", "{label}" span { class: "palette-project", "{path.display()}" } }
        },
        Entry::Routine { name, .. } => rsx! {
            span { class: "palette-icon", Icon { kind: IconKind::Clock, size: IconSize::Small } }
            div { class: "palette-title", "{name}" }
        },
        Entry::NewSession => rsx! {
            span { class: "palette-icon", Icon { kind: IconKind::Plus, size: IconSize::Small } }
            div { class: "palette-title", {tr!("New session")} }
        },
    }
}

/// What the palette's row menu shows: its items, the rename box, or the delete question.
#[derive(Debug, Clone, PartialEq)]
enum RowMenuPage {
    Main,
    Renaming(String),
    ConfirmDelete,
}

/// A session row's menu in the palette, as the desktop app has it (observed 2026-11-06): the title
/// and `세션 · <when>`, then 열기, 고정, 이름 변경, 보관, 삭제.
#[component]
fn PaletteRowMenu(row: SearchRow, on_close: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut page = use_signal(|| RowMenuPage::Main);
    let project = row.project.path.clone();
    let session = match &row.key {
        forge_features::EntryKey::Past { session, .. } => Some(session.clone()),
        forge_features::EntryKey::Live(id) => workspace.live_view(*id).and_then(|view| view.peek().claude_session_id.clone()),
    };
    let open_now = matches!(row.key, forge_features::EntryKey::Live(_));
    let when = row.updated_at.as_deref().map(|at| relative_time(at, OffsetDateTime::now_utc())).unwrap_or_default();
    let pinned = session.as_ref().is_some_and(|id| workspace.is_pinned(&project, id));
    let archived = session.as_ref().is_some_and(|id| workspace.is_archived(&project, id));
    let opened = row.clone();
    rsx! {
        div { class: "row-menu palette-row-menu", role: "menu", onclick: move |event| event.stop_propagation(),
            div { class: "palette-row-menu-head",
                div { class: "palette-row-menu-title", "{row.title}" }
                div { class: "palette-row-menu-sub", {tr!("Session")} if !when.is_empty() { " · {when}" } }
            }
            match page() {
                RowMenuPage::Main => rsx! {
                    button { class: "row-menu-item", role: "menuitem", onclick: move |_| workspace.open_search_row(&opened), span { {tr!("Open")} } }
                    if let Some(id) = session.clone() {
                        button {
                            class: "row-menu-item",
                            role: "menuitem",
                            onclick: {
                                let project = project.clone();
                                let id = id.clone();
                                move |_| {
                                    workspace.toggle_pin(project.clone(), id.clone());
                                    on_close.call(());
                                }
                            },
                            span { if pinned { {tr!("Unpin")} } else { {tr!("Pin")} } }
                        }
                        button {
                            class: "row-menu-item",
                            role: "menuitem",
                            onclick: {
                                let title = row.title.clone();
                                move |_| page.set(RowMenuPage::Renaming(title.clone()))
                            },
                            span { {tr!("Rename")} }
                        }
                        if !open_now || archived {
                            button {
                                class: "row-menu-item",
                                role: "menuitem",
                                onclick: {
                                    let project = project.clone();
                                    let id = id.clone();
                                    move |_| {
                                        if archived {
                                            workspace.unarchive_session(project.clone(), id.clone());
                                        } else {
                                            workspace.archive_session(project.clone(), id.clone());
                                        }
                                        on_close.call(());
                                    }
                                },
                                span { if archived { {tr!("Unarchive")} } else { {tr!("Archive")} } }
                            }
                        }
                        div { class: "row-menu-separator" }
                        button {
                            class: "row-menu-item danger",
                            role: "menuitem",
                            disabled: open_now,
                            title: if open_now { tr!("Close the session before deleting it.") } else { "" },
                            onclick: move |_| page.set(RowMenuPage::ConfirmDelete),
                            span { {tr!("Delete")} }
                        }
                    }
                },
                RowMenuPage::Renaming(title) => rsx! {
                    input {
                        class: "text-input palette-rename",
                        r#type: "text",
                        value: "{title}",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| page.set(RowMenuPage::Renaming(event.value())),
                        onkeydown: {
                            let project = project.clone();
                            let session = session.clone();
                            move |event: KeyboardEvent| match event.key() {
                                Key::Enter if !event.is_composing() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    if let (RowMenuPage::Renaming(title), Some(id)) = (page.peek().clone(), session.clone()) {
                                        if !title.trim().is_empty() {
                                            workspace.rename_session(project.clone(), id, title.trim().to_string());
                                        }
                                    }
                                    on_close.call(());
                                }
                                Key::Escape => {
                                    event.stop_propagation();
                                    page.set(RowMenuPage::Main);
                                }
                                _ => {}
                            }
                        },
                    }
                },
                RowMenuPage::ConfirmDelete => rsx! {
                    div { class: "row-menu-note", {tr!("Delete this session? It is moved to the trash.")} }
                    div { class: "row-menu-actions",
                        button { class: "row-menu-item", onclick: move |_| page.set(RowMenuPage::Main), span { {tr!("Cancel")} } }
                        button {
                            class: "row-menu-item danger",
                            onclick: {
                                let project = project.clone();
                                let session = session.clone();
                                move |_| {
                                    if let Some(id) = session.clone() {
                                        workspace.delete_session(project.clone(), id);
                                    }
                                    on_close.call(());
                                }
                            },
                            span { {tr!("Delete")} }
                        }
                    }
                },
            }
        }
    }
}
