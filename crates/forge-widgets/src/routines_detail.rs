use dioxus::prelude::*;
use forge_features::{history_rows, local_offset, use_routines, use_workspace, when_label, RoutineDialog};
use forge_shared::{Button, ButtonKind, Switch};

/// One routine: its settings at the left, its instructions and history at the right.
#[component]
pub fn RoutineDetail(id: String) -> Element {
    let routines = use_routines();
    let workspace = use_workspace();
    let mut expanded = use_signal(|| false);
    let Some(routine) = routines.routines().into_iter().find(|routine| routine.id == id) else {
        return rsx! {
            p { "This routine was deleted." }
            Button { label: "Back to routines", onclick: move |_| routines.show_list() }
        };
    };
    let offset = local_offset();
    let state_of = |session_id: &str| workspace.run_state(session_id);
    let rows = history_rows(&routines.runs(), &routine.id, &state_of);
    let waiting = routines.waiting().contains(&routine.id);
    let long = routine.instructions.lines().count() > 3 || routine.instructions.len() > 240;
    rsx! {
        div { class: "routine-head",
            button { class: "link", onclick: move |_| routines.show_list(), "Routines" }
            span { " / {routine.name}" }
            div { class: "routine-actions",
                Button {
                    label: "Edit",
                    onclick: {
                        let id = id.clone();
                        move |_| routines.open_form(Some(id.clone()))
                    },
                }
                Button {
                    label: "Delete",
                    kind: ButtonKind::Danger,
                    onclick: {
                        let id = id.clone();
                        move |_| routines.open_dialog(RoutineDialog::ConfirmDelete(id.clone()))
                    },
                }
                Button {
                    label: "Run now",
                    kind: ButtonKind::Primary,
                    disabled: routines.busy(),
                    onclick: {
                        let id = id.clone();
                        move |_| routines.run_now(id.clone(), workspace)
                    },
                }
            }
        }
        if waiting {
            div { class: "banner error", "A session of this routine is waiting for your approval." }
        }
        div { class: "routine-columns",
            div { class: "routine-left",
                h3 { "Description" }
                p { "{routine.description}" }
                h3 { "Status" }
                div { class: "routine-switch",
                    Switch {
                        on: routine.enabled,
                        disabled: routines.busy(),
                        onchange: {
                            let id = id.clone();
                            move |on: bool| routines.set_enabled(id.clone(), on)
                        },
                    }
                    span { if routine.enabled { "On" } else { "Paused" } }
                }
                h3 { "Folder" }
                p { class: "routine-folder", "{routine.folder.display()}" }
                h3 { "Repeats" }
                p { "{routine.schedule.label()}" }
                h3 { "Always allowed" }
                if routine.always_allowed.is_empty() {
                    p { class: "hint", "Permissions you allow while a run of this routine is working show up here." }
                }
                for rule in routine.always_allowed.clone() {
                    div { key: "{rule}", class: "routine-rule",
                        code { "{rule}" }
                        button {
                            class: "link",
                            onclick: {
                                let id = id.clone();
                                let rule = rule.clone();
                                move |_| routines.remove_allowed(id.clone(), rule.clone())
                            },
                            "Remove"
                        }
                    }
                }
            }
            div { class: "routine-right",
                h3 { "Instructions" }
                div { class: if expanded() { "routine-instructions open" } else { "routine-instructions" }, "{routine.instructions}" }
                if long {
                    button { class: "link", onclick: move |_| expanded.set(!expanded()), if expanded() { "Show less" } else { "Show all" } }
                }
                h3 { "History" }
                if rows.is_empty() {
                    p { class: "hint", "Nothing has run yet." }
                }
                for row in rows {
                    {
                        let when = when_label(row.due_ms, offset);
                        let session = row.session_id.clone();
                        let text = row.text.clone();
                        rsx! {
                            button {
                                key: "{row.due_ms}",
                                class: if session.is_some() { "routine-run clickable" } else { "routine-run" },
                                onclick: move |_| {
                                    if let Some(session) = session.clone() {
                                        workspace.open_session_by_claude_id(&session);
                                    }
                                },
                                span { class: "routine-run-when", "{when}" }
                                span { class: "routine-run-text", "{text}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
