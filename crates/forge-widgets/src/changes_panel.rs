use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{parse_unified_diff, DiffView};
use forge_features::{run_blocking, use_workspace, Panel};
use forge_ports::{ChangedFile, GitError};

use crate::panel_frame::PanelFrame;

// A repository can have thousands of changed files (an untracked `node_modules`); only this many
// rows are drawn.
const ROW_LIMIT: usize = 500;

/// The files git says changed in the session's repository, with the diff of the one picked. Read
/// only. It reloads when "Refresh" is pressed and whenever a tool call of the session finishes.
#[component]
pub fn ChangesPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut refresh = use_signal(|| 0u32);
    let mut selected = use_signal(|| None::<String>);
    let mut list_open = use_signal(|| true);
    let mut status = use_signal(|| None::<Result<Vec<ChangedFile>, GitError>>);
    let mut diff = use_signal(|| None::<(String, Result<String, GitError>)>);
    // The count changes only when a tool call finishes; reading the session's view directly in an
    // effect would rerun it on every streamed chunk of a reply.
    let finished = use_memo(move || workspace.finished_tool_calls(id));
    // Each run takes a ticket; a result is kept only if no later run started meanwhile.
    let mut status_ticket = use_signal(|| 0u64);
    let mut diff_ticket = use_signal(|| 0u64);

    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = status_ticket.peek().wrapping_add(1);
            status_ticket.set(ticket);
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let Some(result) = run_blocking(move || git.status(&cwd)).await else { return };
                if *status_ticket.peek() != ticket {
                    return;
                }
                if let Ok(files) = &result {
                    // Keep the picked file while it is still changed, else pick the first one.
                    let keep = selected.peek().as_ref().is_some_and(|path| files.iter().any(|file| &file.path == path));
                    if !keep {
                        selected.set(files.first().map(|file| file.path.clone()));
                    }
                }
                status.set(Some(result));
            });
        });
    }
    {
        let cwd = cwd.clone();
        use_effect(move || {
            let _ = refresh();
            let _ = finished();
            let ticket = diff_ticket.peek().wrapping_add(1);
            diff_ticket.set(ticket);
            let Some(path) = selected() else {
                diff.set(None);
                return;
            };
            let (git, cwd) = (workspace.git(), cwd.clone());
            spawn(async move {
                let shown = path.clone();
                let Some(result) = run_blocking(move || git.diff(&cwd, &path)).await else { return };
                if *diff_ticket.peek() != ticket {
                    return;
                }
                diff.set(Some((shown, result)));
            });
        });
    }

    let tools = rsx! {
        button {
            class: if list_open() { "icon-button active" } else { "icon-button" },
            r#type: "button",
            title: "File list",
            "aria-label": "File list",
            "aria-pressed": "{list_open()}",
            onclick: move |_| list_open.set(!list_open()),
            "☰"
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: "Refresh",
            "aria-label": "Refresh",
            onclick: move |_| refresh.set(refresh() + 1),
            "⟳"
        }
    };

    let body = match status() {
        None => rsx! { div { class: "dock-note", "Loading…" } },
        Some(Err(GitError::NotARepository)) => rsx! { div { class: "dock-note", "No changes to show" } },
        Some(Err(error)) => rsx! { div { class: "dock-note error", "{error}" } },
        Some(Ok(files)) if files.is_empty() => rsx! { div { class: "dock-note", "No changes to show" } },
        Some(Ok(files)) => {
            let picked = selected();
            let hidden = files.len().saturating_sub(ROW_LIMIT);
            let shown = match diff() {
                Some((path, Ok(text))) if Some(&path) == picked.as_ref() => {
                    let parsed = parse_unified_diff(&path, &text);
                    if parsed.hunks.is_empty() {
                        rsx! { div { class: "dock-note", "No textual changes to show" } }
                    } else {
                        rsx! { DiffView { diff: parsed } }
                    }
                }
                Some((path, Err(error))) if Some(&path) == picked.as_ref() => rsx! { div { class: "dock-note error", "{error}" } },
                _ => rsx! { div { class: "dock-note", "Loading…" } },
            };
            rsx! {
                div { class: "changes-body",
                    if list_open() {
                        ul { class: "changes-list",
                            for file in files.iter().take(ROW_LIMIT) {
                                {
                                    let path = file.path.clone();
                                    let letter = file.status.letter();
                                    let counts = match (file.added, file.removed) {
                                        (Some(added), Some(removed)) => format!("+{added} −{removed}"),
                                        _ => String::new(),
                                    };
                                    rsx! {
                                        li { key: "{file.path}",
                                            button {
                                                class: if picked.as_deref() == Some(file.path.as_str()) { "changes-row selected" } else { "changes-row" },
                                                r#type: "button",
                                                title: "{file.path}",
                                                onclick: {
                                                    let path = path.clone();
                                                    move |_| selected.set(Some(path.clone()))
                                                },
                                                span { class: "change-letter change-{letter}", "{letter}" }
                                                span { class: "change-path", "{file.path}" }
                                                span { class: "change-counts", "{counts}" }
                                            }
                                        }
                                    }
                                }
                            }
                            if hidden > 0 {
                                li { class: "dock-note", "… {hidden} more files" }
                            }
                        }
                    }
                    div { class: "changes-diff", {shown} }
                }
            }
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Changes, tools, {body} }
    }
}
