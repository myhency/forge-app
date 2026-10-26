use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{preview_text, rewind_rows};
use forge_shared::tr;

use crate::workspace::use_workspace;

// What the copy is called before " (fork)": the session's title, else its first message.
fn fork_name(session: &forge_domain::Session) -> String {
    session.title.clone().filter(|title| !title.trim().is_empty()).or_else(|| rewind_rows(&session.messages).pop().map(|row| row.label)).unwrap_or_default()
}

/// `/rewind`: pick an earlier message, see what putting the files back would change, then put the
/// files back, open the conversation as it was before it (a new session; this one stays), or both.
#[component]
pub fn RewindDialog(id: LiveSessionId, open: Signal<bool>) -> Element {
    let workspace = use_workspace();
    let mut open = open;
    let mut picked = use_signal(|| None::<String>);
    // "Both" asked: the conversation opens once the files are back.
    let mut then_fork = use_signal(|| false);
    let Some(view) = workspace.live_view(id) else { return rsx! {} };

    let close = move |_| {
        if let Some(handle) = workspace.handle(id) {
            handle.close_rewind();
        }
        picked.set(None);
        then_fork.set(false);
        open.set(false);
    };

    // The files are back and "both" was asked: now the conversation.
    use_effect(move || {
        let snapshot = view.read();
        let done = snapshot.rewind.as_ref().is_some_and(|state| !state.dry_run && state.outcome.as_ref().is_some_and(|o| o.can_rewind));
        if *then_fork.peek() && done {
            let (project, session, title) = (snapshot.project_path.clone(), snapshot.claude_session_id.clone(), fork_name(&snapshot.session));
            let uuid = snapshot.rewind.as_ref().map(|state| state.message_uuid.clone());
            drop(snapshot);
            then_fork.set(false);
            if let (Some(session), Some(uuid)) = (session, uuid) {
                workspace.fork_session_before(project, session, uuid, title);
            }
            open.set(false);
        }
    });

    if !open() {
        return rsx! {};
    }
    let snapshot = view.read().clone();
    let running = snapshot.phase == forge_application::Phase::Running;
    let rows = rewind_rows(&snapshot.session.messages);
    let state = snapshot.rewind.clone().filter(|state| Some(&state.message_uuid) == picked.read().as_ref());
    // Before the oldest message there is no conversation to open.
    let has_earlier = picked.read().as_ref().and_then(|uuid| rows.iter().position(|row| &row.uuid == uuid)).is_some_and(|at| at + 1 < rows.len());
    let label = picked.read().as_ref().and_then(|uuid| rows.iter().find(|row| &row.uuid == uuid)).map(|row| row.label.clone());

    rsx! {
        div { class: "picker-backdrop", onclick: close }
        div { class: "card-modal rewind-dialog", role: "dialog",
            div { class: "card-modal-head",
                span { class: "card-modal-title", {tr!("Go back to an earlier point")} }
                button { class: "card-remove always", r#type: "button", "aria-label": tr!("Close"), onclick: close, "×" }
            }
            if let Some(label) = label {
                div { class: "card-modal-note", {tr!("Before this message:")} }
                div { class: "rewind-picked", "{label}" }
                div { class: "rewind-preview",
                    match state.as_ref().and_then(|state| state.outcome.as_ref().map(|outcome| (state.dry_run, outcome))) {
                        None => rsx! { {tr!("Checking the files…")} },
                        Some((true, outcome)) => rsx! { "{preview_text(outcome)}" },
                        Some((false, outcome)) if outcome.can_rewind => rsx! { {tr!("The files are back as they were.")} },
                        Some((false, outcome)) => rsx! { "{preview_text(outcome)}" },
                    }
                }
                if has_earlier {
                    div { class: "card-modal-note", {tr!("The conversation opens as a new session. This session stays as it is.")} }
                } else {
                    div { class: "card-modal-note", {tr!("Nothing came before this message, so there is no conversation to open.")} }
                }
                if running {
                    div { class: "card-modal-note", {tr!("Wait for Claude to finish, or stop it first.")} }
                }
                div { class: "rewind-actions",
                    button {
                        class: "rewind-button",
                        r#type: "button",
                        disabled: running || !state.as_ref().is_some_and(|s| s.dry_run && s.outcome.as_ref().is_some_and(|o| o.can_rewind && !o.files.is_empty())),
                        onclick: move |_| {
                            if let (Some(handle), Some(uuid)) = (workspace.handle(id), picked.peek().clone()) {
                                handle.rewind_files(uuid, false);
                            }
                        },
                        {tr!("Restore files")}
                    }
                    button {
                        class: "rewind-button",
                        r#type: "button",
                        disabled: running || !has_earlier || snapshot.claude_session_id.is_none(),
                        onclick: {
                            let (project, session, title) = (snapshot.project_path.clone(), snapshot.claude_session_id.clone(), fork_name(&snapshot.session));
                            move |_| {
                                if let (Some(session), Some(uuid)) = (session.clone(), picked.peek().clone()) {
                                    workspace.fork_session_before(project.clone(), session, uuid, title.clone());
                                    open.set(false);
                                }
                            }
                        },
                        {tr!("Open the conversation")}
                    }
                    button {
                        class: "rewind-button",
                        r#type: "button",
                        disabled: running || !has_earlier || snapshot.claude_session_id.is_none() || !state.as_ref().is_some_and(|s| s.dry_run && s.outcome.as_ref().is_some_and(|o| o.can_rewind && !o.files.is_empty())),
                        onclick: move |_| {
                            if let (Some(handle), Some(uuid)) = (workspace.handle(id), picked.peek().clone()) {
                                then_fork.set(true);
                                handle.rewind_files(uuid, false);
                            }
                        },
                        {tr!("Both")}
                    }
                    button { class: "rewind-button", r#type: "button", onclick: move |_| picked.set(None), {tr!("Back")} }
                }
            } else {
                div { class: "card-modal-note", {tr!("Pick the message to go back to.")} }
                if rows.is_empty() {
                    div { class: "card-modal-note", {tr!("There is no earlier message yet.")} }
                }
                for row in rows {
                    button {
                        key: "{row.uuid}",
                        class: "rewind-row",
                        r#type: "button",
                        onclick: {
                            let uuid = row.uuid.clone();
                            move |_| {
                                picked.set(Some(uuid.clone()));
                                if let Some(handle) = workspace.handle(id) {
                                    handle.rewind_files(uuid.clone(), true);
                                }
                            }
                        },
                        "{row.label}"
                    }
                }
            }
        }
    }
}
