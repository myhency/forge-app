use dioxus::prelude::*;
use forge_shared::tr;
use forge_domain::split_at_match;
use forge_entities::relative_time;
use forge_features::use_workspace;
use time::OffsetDateTime;

use crate::sidebar::project_label_in;

/// The search palette: a box over the dimmed window that lists the sessions whose title or message
/// text contains what is typed. Up and Down move, Enter opens, Escape or a click outside closes.
/// It draws nothing while closed; the body is a separate component so its highlight starts at the
/// first row every time it opens.
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
    let mut highlight = use_signal(|| 0usize);
    let query = workspace.search_query();
    let rows = workspace.search_rows();
    let count = rows.len();
    let selected = highlight().min(count.saturating_sub(1));
    let now = OffsetDateTime::now_utc();

    rsx! {
        div { class: "palette-backdrop", onclick: move |_| workspace.close_search(),
            div { class: "palette", onclick: move |event| event.stop_propagation(),
                div { class: "palette-input-row",
                    input {
                        class: "palette-input",
                        r#type: "text",
                        value: "{query}",
                        placeholder: tr!("Search sessions"),
                        spellcheck: "false",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        oninput: move |event| {
                            highlight.set(0);
                            workspace.set_search_query(event.value());
                        },
                        onkeydown: {
                            let rows = rows.clone();
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
                                // Enter that ends an IME composition confirms the text, not the row.
                                Key::Enter if !event.is_composing() => {
                                    event.prevent_default();
                                    event.stop_propagation();
                                    if let Some(row) = rows.get(selected) {
                                        workspace.open_search_row(row);
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
                        "×"
                    }
                }
                div { class: "palette-results",
                    for (index, row) in rows.iter().enumerate() {
                        div {
                            key: "{row.key.dom_key()}",
                            class: if index == selected { "palette-row selected" } else { "palette-row" },
                            onmouseenter: move |_| highlight.set(index),
                            onclick: {
                                let row = row.clone();
                                move |_| workspace.open_search_row(&row)
                            },
                            div { class: "palette-title",
                                match split_at_match(&row.title, &query) {
                                    Some((before, matched, after)) => rsx! {
                                        "{before}"
                                        b { "{matched}" }
                                        "{after}"
                                    },
                                    None => rsx! { "{row.title}" },
                                }
                                span { class: "palette-project", "{project_label_in(workspace.scratch_root().as_deref(), &row.project.path)}" }
                            }
                            span { class: "palette-date",
                                {row.updated_at.as_deref().map(|at| relative_time(at, now)).unwrap_or_default()}
                            }
                            if let Some(snippet) = &row.snippet {
                                div { class: "palette-snippet",
                                    "{snippet.before}"
                                    b { "{snippet.matched}" }
                                    "{snippet.after}"
                                }
                            }
                        }
                    }
                    if rows.is_empty() {
                        div { class: "palette-empty", {tr!("No sessions match")} }
                    }
                }
            }
        }
    }
}
