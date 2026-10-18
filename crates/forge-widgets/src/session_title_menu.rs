use dioxus::prelude::*;
use forge_features::{menu_key_action, refocus_workspace, use_workspace, ArchiveAction, MenuKeyAction, RowActions};
use forge_shared::tr;

/// The chevron after the title of the open session and its menu: rename, fork, archive. It runs the
/// same actions as the shortcuts (⌥⌘R, F, ⌥⌘A) on the active session. There is no delete: an open
/// session has to be closed before it can be deleted.
#[component]
pub fn SessionTitleMenu(actions: RowActions) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    if actions.archive == ArchiveAction::Unavailable && !actions.can_rename && !actions.can_fork {
        return rsx! {};
    }
    let archive_label = if actions.archive == ArchiveAction::Archive { tr!("Archive") } else { tr!("Unarchive") };
    let mut close = move || {
        open.set(false);
        refocus_workspace();
    };
    rsx! {
        div { class: "title-menu",
            button {
                class: if open() { "title-chevron open" } else { "title-chevron" },
                title: tr!("Rename, fork or archive"),
                "aria-haspopup": "menu",
                "aria-expanded": "{open()}",
                onclick: move |_| open.set(!open()),
                "⌄"
            }
            if open() {
                div { class: "menu-backdrop", onclick: move |_| open.set(false) }
                div {
                    class: "row-menu title-menu-list",
                    tabindex: "0",
                    role: "menu",
                    onmounted: move |mounted| async move {
                        let _ = mounted.set_focus(true).await;
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Escape {
                            event.stop_propagation();
                            close();
                            return;
                        }
                        let modified = event.modifiers().intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                        let Some(action) = menu_key_action(&event.code().to_string(), modified, actions) else {
                            return;
                        };
                        event.stop_propagation();
                        event.prevent_default();
                        open.set(false);
                        match action {
                            MenuKeyAction::Rename => workspace.rename_active(),
                            MenuKeyAction::Fork => {
                                workspace.fork_active();
                                refocus_workspace();
                            }
                            MenuKeyAction::ToggleArchive => {
                                workspace.archive_active();
                                refocus_workspace();
                            }
                            MenuKeyAction::Delete => {}
                        }
                    },
                    if actions.can_rename {
                        button {
                            class: "row-menu-item",
                            role: "menuitem",
                            onclick: move |_| {
                                open.set(false);
                                workspace.rename_active();
                            },
                            span { {tr!("Rename")} }
                            span { class: "menu-key", "R" }
                        }
                    }
                    if actions.can_fork {
                        button {
                            class: "row-menu-item",
                            role: "menuitem",
                            onclick: move |_| {
                                open.set(false);
                                workspace.fork_active();
                                refocus_workspace();
                            },
                            span { {tr!("Fork")} }
                            span { class: "menu-key", "F" }
                        }
                    }
                    if actions.archive != ArchiveAction::Unavailable {
                        button {
                            class: "row-menu-item",
                            role: "menuitem",
                            onclick: move |_| {
                                open.set(false);
                                workspace.archive_active();
                                refocus_workspace();
                            },
                            span { {archive_label} }
                            span { class: "menu-key", "A" }
                        }
                    }
                }
            }
        }
    }
}
