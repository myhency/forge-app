use dioxus::prelude::*;
use forge_features::{cards, use_routines, RoutineDialog, RoutineFilter, RoutineScreen, RoutineSort};
use forge_shared::{tr, Button, ButtonKind};

use crate::routines_detail::RoutineDetail;
use crate::routines_form::RoutineForm;


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
                    Button { label: tr!("Dismiss"), onclick: move |_| routines.clear_error() }
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
        h1 { class: "customize-title", {tr!("Routines")} }
        div { class: "customize-bar",
            input {
                class: "customize-search",
                r#type: "text",
                placeholder: tr!("Search"),
                value: "{routines.query()}",
                oninput: move |event| routines.set_query(event.value()),
            }
            div { class: "customize-add",
                Button { label: tr!("Status"), onclick: move |_| status_menu.set(!status_menu()) }
                if status_menu() {
                    div { class: "menu-backdrop", onclick: move |_| status_menu.set(false) }
                    div { class: "customize-menu",
                        for (value, label) in [(RoutineFilter::All, tr!("All")), (RoutineFilter::On, tr!("On")), (RoutineFilter::Paused, tr!("Paused"))] {
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
                Button { label: tr!("Sort"), onclick: move |_| sort_menu.set(!sort_menu()) }
                if sort_menu() {
                    div { class: "menu-backdrop", onclick: move |_| sort_menu.set(false) }
                    div { class: "customize-menu",
                        for (value, label) in [(RoutineSort::Name, tr!("Name")), (RoutineSort::LastRun, tr!("Last run")), (RoutineSort::Created, tr!("Created"))] {
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
                Button { label: tr!("+ New routine"), kind: ButtonKind::Primary, onclick: move |_| routines.open_form(None) }
            }
        }
        p { class: "routines-notice", {tr!("Routines run only while Forge is open and the computer is awake.")} }
        if empty {
            p { class: "hint", {tr!("No routines yet. A routine runs a prompt in a folder on a schedule.")} }
        } else if list.is_empty() {
            p { class: "hint", {tr!("No routine matches.")} }
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
                            span { class: "routine-tag", {tr!("Waiting for approval")} }
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
                        h2 { {tr!("Delete this routine?")} }
                        p { {tr!("Its history is deleted too. The sessions it made stay.")} }
                        div { class: "customize-dialog-buttons",
                            Button { label: tr!("Cancel"), onclick: move |_| routines.close_dialog() }
                            Button { label: tr!("Delete"), kind: ButtonKind::Danger, onclick: move |_| routines.delete(id.clone()) }
                        }
                    },
                    RoutineDialog::ConfirmDiscard => rsx! {
                        h2 { {tr!("Discard unsaved changes?")} }
                        p { {tr!("The changes are lost if they are not saved.")} }
                        div { class: "customize-dialog-buttons",
                            Button { label: tr!("Keep editing"), onclick: move |_| routines.close_dialog() }
                            Button { label: tr!("Discard"), kind: ButtonKind::Danger, onclick: move |_| routines.discard_form() }
                        }
                    },
                }
            }
        }
    }
}
