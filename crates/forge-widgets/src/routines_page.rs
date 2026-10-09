use dioxus::prelude::*;
use forge_features::{cards, use_routines, RoutineDialog, RoutineFilter, RoutineScreen, RoutineSort};
use forge_shared::{Button, ButtonKind};

use crate::routines_detail::RoutineDetail;
use crate::routines_form::RoutineForm;

const NOTICE: &str = "Routines run only while Forge is open and the computer is awake.";

/// The Routines page: the list, one routine's detail, and the form (spec 2026-10-09-routines-design).
#[component]
pub fn RoutinesPage() -> Element {
    let routines = use_routines();
    use_hook(move || routines.open_page());
    let screen = routines.screen();
    rsx! {
        div { class: "customize routines",
            if let Some(message) = routines.error() {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| routines.clear_error() }
                }
            }
            match screen {
                RoutineScreen::List => rsx! { RoutineList {} },
                RoutineScreen::Detail(id) => rsx! { RoutineDetail { id } },
                RoutineScreen::Form(id) => rsx! { RoutineForm { id } },
            }
            RoutineDialogs {}
        }
    }
}

#[component]
fn RoutineList() -> Element {
    let routines = use_routines();
    let all = routines.routines();
    let empty = all.is_empty();
    let list = cards(&all, &routines.runs(), &routines.waiting(), &routines.query(), routines.filter(), routines.sort());
    let mut status_menu = use_signal(|| false);
    let mut sort_menu = use_signal(|| false);
    rsx! {
        h1 { class: "customize-title", "Routines" }
        div { class: "customize-bar",
            input {
                class: "customize-search",
                r#type: "text",
                placeholder: "Search",
                value: "{routines.query()}",
                oninput: move |event| routines.set_query(event.value()),
            }
            div { class: "customize-add",
                Button { label: "Status", onclick: move |_| status_menu.set(!status_menu()) }
                if status_menu() {
                    div { class: "menu-backdrop", onclick: move |_| status_menu.set(false) }
                    div { class: "customize-menu",
                        for (value, label) in [(RoutineFilter::All, "All"), (RoutineFilter::On, "On"), (RoutineFilter::Paused, "Paused")] {
                            button {
                                key: "{label}",
                                onclick: move |_| {
                                    routines.set_filter(value);
                                    status_menu.set(false);
                                },
                                if routines.filter() == value { "✓ {label}" } else { "{label}" }
                            }
                        }
                    }
                }
            }
            div { class: "customize-add",
                Button { label: "Sort", onclick: move |_| sort_menu.set(!sort_menu()) }
                if sort_menu() {
                    div { class: "menu-backdrop", onclick: move |_| sort_menu.set(false) }
                    div { class: "customize-menu",
                        for (value, label) in [(RoutineSort::Name, "Name"), (RoutineSort::LastRun, "Last run"), (RoutineSort::Created, "Created")] {
                            button {
                                key: "{label}",
                                onclick: move |_| {
                                    routines.set_sort(value);
                                    sort_menu.set(false);
                                },
                                if routines.sort() == value { "✓ {label}" } else { "{label}" }
                            }
                        }
                    }
                }
            }
            div { class: "customize-add",
                Button { label: "+ New routine", kind: ButtonKind::Primary, onclick: move |_| routines.open_form(None) }
            }
        }
        p { class: "routines-notice", "{NOTICE}" }
        if empty {
            p { class: "hint", "No routines yet. A routine runs a prompt in a folder on a schedule." }
        } else if list.is_empty() {
            p { class: "hint", "No routine matches." }
        }
        div { class: "routine-cards",
            for card in list {
                button {
                    key: "{card.id}",
                    class: "routine-card",
                    onclick: {
                        let id = card.id.clone();
                        move |_| routines.show_detail(id.clone())
                    },
                    div { class: "routine-card-title",
                        span { "{card.name}" }
                        if card.waiting {
                            span { class: "routine-tag", "Waiting for approval" }
                        }
                    }
                    div { class: "routine-card-desc", "{card.description}" }
                    div { class: "routine-card-meta", "{card.meta()}" }
                }
            }
        }
    }
}

#[component]
fn RoutineDialogs() -> Element {
    let routines = use_routines();
    let Some(dialog) = routines.dialog() else {
        return rsx! {};
    };
    rsx! {
        div { class: "settings-backdrop", onclick: move |_| routines.close_dialog(),
            div { class: "customize-dialog", onclick: move |event| event.stop_propagation(),
                match dialog {
                    RoutineDialog::ConfirmDelete(id) => rsx! {
                        h2 { "Delete this routine?" }
                        p { "Its history is deleted too. The sessions it made stay." }
                        div { class: "customize-dialog-buttons",
                            Button { label: "Cancel", onclick: move |_| routines.close_dialog() }
                            Button { label: "Delete", kind: ButtonKind::Danger, onclick: move |_| routines.delete(id.clone()) }
                        }
                    },
                    RoutineDialog::ConfirmDiscard => rsx! {
                        h2 { "Discard unsaved changes?" }
                        p { "The changes are lost if they are not saved." }
                        div { class: "customize-dialog-buttons",
                            Button { label: "Keep editing", onclick: move |_| routines.close_dialog() }
                            Button { label: "Discard", kind: ButtonKind::Danger, onclick: move |_| routines.discard_form() }
                        }
                    },
                }
            }
        }
    }
}
