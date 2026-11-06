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

/// A filter the palette's `/` puts on the session list (the desktop app's, observed 2026-11-06:
/// `date:` with 오늘 / 지난 일주일 / 지난 한 달, `project:`, shown as a chip with ×).
#[derive(Debug, Clone, PartialEq)]
enum Filter {
    Days(u8),
    Project(PathBuf, String),
}

impl Filter {
    fn label(&self) -> String {
        match self {
            Filter::Days(1) => tr!("Today").to_string(),
            Filter::Days(7) => tr!("Past week").to_string(),
            Filter::Days(_) => tr!("Past month").to_string(),
            Filter::Project(_, name) => name.clone(),
        }
    }

    /// Whether a session row passes: updated within the days (today: since local midnight), or in the folder.
    fn keeps(&self, row: &SearchRow, now: OffsetDateTime) -> bool {
        match self {
            Filter::Project(path, _) => row.project.path == *path,
            Filter::Days(days) => {
                let Some(at) = row.updated_at.as_deref().and_then(|at| OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()) else { return false };
                let at = at.to_offset(now.offset());
                if *days == 1 {
                    at.date() == now.date()
                } else {
                    now - at <= time::Duration::days(i64::from(*days))
                }
            }
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
    /// After `/`: a kind of filter to pick (`date`, `project`).
    FilterKind(&'static str),
    /// A filter to put on.
    FilterValue(Filter),
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
    let mut filters: Signal<Vec<Filter>> = use_signal(Vec::new);
    let query = workspace.search_query();
    let needle = query.trim().to_lowercase();
    let matches = |text: &str| needle.is_empty() || text.to_lowercase().contains(&needle);
    let scratch = workspace.scratch_root();

    let now_local = forge_features::now_local();
    let active_filters = filters();
    let sessions: Vec<Entry> = workspace
        .search_rows()
        .into_iter()
        .filter(|row| active_filters.iter().all(|filter| filter.keeps(row, now_local)))
        .map(Entry::Session)
        .collect();
    // Two folders with the same name are told apart as in the sidebar (`app (a)`, `app (b)`).
    let project_paths: Vec<PathBuf> = workspace.new_session_projects().into_iter().map(|project| project.path).collect();
    let labels = crate::sidebar::project_labels(&project_paths, scratch.as_deref());
    // Every folder, for the `project:` filter (the list below is narrowed by what is typed).
    let all_projects: Vec<(PathBuf, String)> = project_paths.iter().cloned().zip(labels.iter().cloned()).collect();
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
    let picking = query.trim_start();
    let sections: Vec<(String, Vec<Entry>)> = if picking == "/" {
        vec![(String::new(), vec![Entry::FilterKind("date"), Entry::FilterKind("project")])]
    } else if let Some(rest) = picking.strip_prefix("date:") {
        let rest = rest.trim().to_lowercase();
        let days: Vec<Entry> = [1u8, 7, 30].into_iter().map(Filter::Days).filter(|filter| filter.label().to_lowercase().contains(&rest)).map(Entry::FilterValue).collect();
        vec![(tr!("Date").to_string(), days)]
    } else if let Some(rest) = picking.strip_prefix("project:") {
        let rest = rest.trim().to_lowercase();
        let folders: Vec<Entry> = all_projects
            .iter()
            .filter(|(_, label)| label.to_lowercase().contains(&rest))
            .map(|(path, label)| Entry::FilterValue(Filter::Project(path.clone(), label.clone())))
            .collect();
        vec![(tr!("Project").to_string(), folders)]
    } else if !active_filters.is_empty() {
        // A filter narrows the sessions: only they are listed, as in the desktop app.
        vec![(String::new(), sessions)]
    } else {
        match tab() {
        Tab::All if needle.is_empty() => vec![
            (tr!("Recent items").to_string(), sessions.into_iter().take(8).collect()),
            (tr!("Actions").to_string(), vec![Entry::NewSession]),
        ],
        Tab::All => vec![(tr!("Sessions").to_string(), sessions), (tr!("Projects").to_string(), projects), (tr!("Scheduled").to_string(), scheduled)],
        Tab::Sessions => vec![(String::new(), sessions)],
        Tab::Projects => vec![(String::new(), projects)],
        Tab::Scheduled => vec![(String::new(), scheduled)],
        }
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
        Entry::FilterKind(kind) => workspace.set_search_query(format!("{kind}:")),
        Entry::FilterValue(filter) => {
            let mut filters = filters;
            filters.write().retain(|old| std::mem::discriminant(old) != std::mem::discriminant(filter));
            filters.write().push(filter.clone());
            workspace.set_search_query(String::new());
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
                    for (index, filter) in filters().into_iter().enumerate() {
                        span { key: "{index}", class: "palette-chip",
                            "{filter.label()}"
                            button {
                                class: "palette-chip-close",
                                r#type: "button",
                                title: tr!("Remove"),
                                onclick: move |event| {
                                    event.stop_propagation();
                                    filters.write().remove(index);
                                },
                                Icon { kind: IconKind::Close, size: IconSize::Small }
                            }
                        }
                    }
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
                                Key::Backspace if workspace.search_query().is_empty() && !filters.peek().is_empty() => {
                                    event.prevent_default();
                                    filters.write().pop();
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
                                    Entry::FilterKind(kind) => format!("fk-{kind}"),
                                    Entry::FilterValue(filter) => format!("fv-{}", filter.label()),
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
                    span { {tr!("Filter")} span { class: "palette-key", "/" } }
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
        Entry::FilterKind(kind) => {
            let (icon, name) = match *kind {
                "date" => (IconKind::Clock, "Date"),
                _ => (IconKind::Folder, "Project"),
            };
            rsx! {
                span { class: "palette-icon", Icon { kind: icon, size: IconSize::Small } }
                div { class: "palette-title", span { class: "palette-chip static", "{name}" } " " {tr!("filter by")} }
            }
        }
        Entry::FilterValue(filter) => rsx! {
            span { class: "palette-icon" }
            div { class: "palette-title", "{filter.label()}" }
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

#[cfg(test)]
mod filter_tests {
    use super::*;
    use forge_domain::SessionId;

    fn row(at: &str, path: &str) -> SearchRow {
        SearchRow {
            key: forge_features::EntryKey::Past { project: PathBuf::from(path), session: SessionId("s".into()) },
            project: forge_domain::ProjectSummary { path: PathBuf::from(path), last_active: String::new(), session_count: 0, exists: true },
            title: "t".into(),
            snippet: None,
            updated_at: Some(at.into()),
        }
    }

    #[test]
    fn date_filters_keep_sessions_from_today_the_week_and_the_month() {
        let now = OffsetDateTime::parse("2026-11-06T15:00:00Z", &time::format_description::well_known::Rfc3339).unwrap();
        assert!(Filter::Days(1).keeps(&row("2026-11-06T01:00:00Z", "/p"), now));
        assert!(!Filter::Days(1).keeps(&row("2026-11-05T23:00:00Z", "/p"), now));
        assert!(Filter::Days(7).keeps(&row("2026-11-01T00:00:00Z", "/p"), now));
        assert!(!Filter::Days(7).keeps(&row("2026-10-20T00:00:00Z", "/p"), now));
        assert!(Filter::Days(30).keeps(&row("2026-10-20T00:00:00Z", "/p"), now));
        assert!(Filter::Project(PathBuf::from("/p"), "p".into()).keeps(&row("2026-01-01T00:00:00Z", "/p"), now));
        assert!(!Filter::Project(PathBuf::from("/q"), "q".into()).keeps(&row("2026-01-01T00:00:00Z", "/p"), now));
    }
}
