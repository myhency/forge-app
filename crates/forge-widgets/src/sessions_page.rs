use dioxus::prelude::*;
use forge_features::{
    now_local, session_when, sessions_list, sessions_sort_label, use_workspace, EntryKey, SessionsSort, SessionsTab, SESSIONS_SORTS,
};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};

use crate::session_row::{PageRow, SessionRow};
use crate::sidebar::project_label_in;

/// The Sessions page (the sidebar menu's `모두 보기`): every session in one flat list, with the active and archived
/// tabs and a search over the titles. The bar stays at the top while the list scrolls.
#[component]
pub fn SessionsPage() -> Element {
    let workspace = use_workspace();
    let mut tab = use_signal(SessionsTab::default);
    let mut searching = use_signal(|| false);
    let mut query = use_signal(String::new);
    let mut sort = use_signal(SessionsSort::default);
    let mut sort_menu = use_signal(|| false);

    let groups = workspace.sidebar();
    let now = now_local();
    let rows = sessions_list(&groups, tab(), &query(), sort());
    let scratch_root = workspace.scratch_root();
    let has_query = !query().trim().is_empty();

    rsx! {
        div { class: "customize sessions-page",
            h1 { class: "customize-title", {tr!("Sessions")} }
            div { class: "sessions-bar",
                div { class: "customize-switch", role: "tablist",
                    for (value, label) in [(SessionsTab::Active, tr!("Active")), (SessionsTab::Archived, tr!("Archived"))] {
                        button {
                            key: "{label}",
                            role: "tab",
                            "aria-selected": "{tab() == value}",
                            class: if tab() == value { "active" } else { "" },
                            onclick: move |_| tab.set(value),
                            "{label}"
                        }
                    }
                }
                div { class: "sessions-tools",
                    if searching() {
                        div { class: "sessions-search",
                            Icon { kind: IconKind::Search, size: IconSize::Small }
                            input {
                                r#type: "text",
                                placeholder: tr!("Search sessions"),
                                value: "{query()}",
                                autofocus: true,
                                spellcheck: "false",
                                onmounted: move |mounted| async move {
                                    let _ = mounted.set_focus(true).await;
                                },
                                oninput: move |event| query.set(event.value()),
                                onkeydown: move |event: KeyboardEvent| {
                                    if event.key() == Key::Escape {
                                        event.stop_propagation();
                                        query.set(String::new());
                                        searching.set(false);
                                    }
                                },
                            }
                            button {
                                class: "icon-button",
                                title: tr!("Clear search"),
                                onclick: move |_| {
                                    query.set(String::new());
                                    searching.set(false);
                                    sort_menu.set(false);
                                },
                                Icon { kind: IconKind::Close, size: IconSize::Small }
                            }
                        }
                        div { class: "sessions-sort",
                            button {
                                class: if sort_menu() { "icon-button open" } else { "icon-button" },
                                title: tr!("Sorting applies to search results only"),
                                disabled: !has_query,
                                onclick: move |_| sort_menu.set(!sort_menu()),
                                Icon { kind: IconKind::Sort }
                            }
                            if sort_menu() && has_query {
                                div { class: "menu-backdrop", onclick: move |_| sort_menu.set(false) }
                                div { class: "customize-menu sessions-sort-menu",
                                    for value in SESSIONS_SORTS {
                                        button {
                                            key: "{sessions_sort_label(value)}",
                                            onclick: move |_| {
                                                sort.set(value);
                                                sort_menu.set(false);
                                            },
                                            span { "{sessions_sort_label(value)}" }
                                            if sort() == value {
                                                Icon { kind: IconKind::Check, size: IconSize::Small }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        button {
                            class: "icon-button",
                            title: tr!("Search sessions"),
                            onclick: move |_| searching.set(true),
                            Icon { kind: IconKind::Search }
                        }
                    }
                    Button { label: tr!("New session"), kind: ButtonKind::Primary, onclick: move |_| workspace.show_new_session() }
                }
            }
            if has_query {
                p { class: "sessions-hint", {tr!("The search covers the titles shown.")} }
            }
            if rows.is_empty() && !has_query {
                p { class: "hint", {tr!("No sessions here.")} }
            }
            div { class: "sessions-list",
                for row in rows {
                    {
                        let folder = if workspace.is_scratch_group(&row.project.path) {
                            tr!("No folder").to_string()
                        } else {
                            project_label_in(scratch_root.as_deref(), &workspace.origin_of(&row.project.path))
                        };
                        let when = row.entry.updated_at.as_deref().map(|at| session_when(at, now)).unwrap_or_default();
                        let key = row.entry.key.clone();
                        let project = row.project.clone();
                        rsx! {
                            SessionRow {
                                key: "{row.entry.key.dom_key()}",
                                project: row.project.path.clone(),
                                entry: row.entry.clone(),
                                active: false,
                                page: PageRow { folder, when, query: query() },
                                onopen: move |beside: bool| match (&key, beside) {
                                    (EntryKey::Live(id), false) => workspace.open_session(*id),
                                    (EntryKey::Live(id), true) => workspace.add_session(*id),
                                    (EntryKey::Past { session, .. }, false) => workspace.open_past_clicked(project.clone(), session.clone()),
                                    (EntryKey::Past { session, .. }, true) => workspace.add_past(project.clone(), session.clone()),
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}
