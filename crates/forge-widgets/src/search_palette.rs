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
            div { class: "palette", onclick: move |event| event.stop_propagation(),
                div { class: "palette-input-row",
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
                                // The desktop app's `유형 변경 ←→`.
                                Key::ArrowLeft if !event.is_composing() => {
                                    event.prevent_default();
                                    switch_tab(-1);
                                }
                                Key::ArrowRight if !event.is_composing() => {
                                    event.prevent_default();
                                    switch_tab(1);
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
                    button {
                        class: "palette-close",
                        title: tr!("Close"),
                        onclick: move |_| workspace.close_search(),
                        Icon { kind: IconKind::Close, size: IconSize::Small }
                    }
                }
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
                                        onmouseenter: move |_| highlight.set(index),
                                        onclick: move |_| open(&clicked),
                                        {row_body(&entry, &query, now, scratch.as_deref())}
                                        if index == selected {
                                            span { class: "palette-enter", "↵" } // icon-ok: a key cap, as in the desktop app
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
                div { class: "palette-footer",
                    span { {tr!("Close")} span { class: "palette-key", "Esc" } }
                    span { {tr!("Change type")} span { class: "palette-key", "←" } span { class: "palette-key", "→" } } // icon-ok: key caps
                    span { {tr!("Open")} span { class: "palette-key", "↵" } } // icon-ok: a key cap
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
