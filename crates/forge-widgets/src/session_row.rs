use std::path::PathBuf;

use dioxus::prelude::*;
use forge_entities::{relative_time, SidebarItem};
use forge_features::{use_workspace, DeleteAction, SidebarEntry};
use forge_shared::{Button, ButtonKind};
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq)]
enum RowMode {
    Normal,
    MenuOpen,
    Renaming(String),
    ConfirmingDelete,
}

/// One session in the sidebar: the item itself, a `⋯` menu on hover, an in-place rename input and
/// an inline delete confirmation. The mode is local to the row.
#[component]
pub fn SessionRow(project: PathBuf, entry: SidebarEntry, active: bool, onopen: EventHandler<()>) -> Element {
    let workspace = use_workspace();
    let mut mode = use_signal(|| RowMode::Normal);
    let actions = entry.actions();
    let now = OffsetDateTime::now_utc();
    let session_id = entry.session_id.clone();

    match mode() {
        RowMode::Renaming(draft) => {
            let original = entry.title.clone();
            let project = project.clone();
            let session_id = session_id.clone();
            rsx! {
                div { class: "sidebar-item sidebar-row",
                    input {
                        class: "rename-input",
                        r#type: "text",
                        value: "{draft}",
                        autofocus: true,
                        spellcheck: "false",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        // A click in the field must not open the session behind it.
                        onclick: move |event| event.stop_propagation(),
                        oninput: move |event| mode.set(RowMode::Renaming(event.value())),
                        onblur: move |_| mode.set(RowMode::Normal),
                        onkeydown: {
                            let draft = draft.clone();
                            move |event: KeyboardEvent| {
                                event.stop_propagation();
                                match event.key() {
                                    // Enter that ends an IME composition (Korean, Japanese, Chinese)
                                    // confirms the text being composed, not the rename.
                                    Key::Enter if !event.is_composing() => {
                                        event.prevent_default();
                                        if let (Some(title), Some(id)) = (rename_submit(&draft, &original), session_id.clone()) {
                                            workspace.rename_session(project.clone(), id, title);
                                        }
                                        mode.set(RowMode::Normal);
                                    }
                                    Key::Escape => mode.set(RowMode::Normal),
                                    _ => {}
                                }
                            }
                        },
                    }
                }
            }
        }
        RowMode::ConfirmingDelete => {
            let project = project.clone();
            let session_id = session_id.clone();
            rsx! {
                div { class: "confirm-row",
                    span { class: "confirm-text", "Delete “{entry.title}”? Moves the session to the trash." }
                    Button { label: "Cancel", onclick: move |_| mode.set(RowMode::Normal) }
                    Button {
                        label: "Delete",
                        kind: ButtonKind::Danger,
                        onclick: move |_| {
                            if let Some(id) = session_id.clone() {
                                workspace.delete_session(project.clone(), id);
                            }
                            mode.set(RowMode::Normal);
                        },
                    }
                }
            }
        }
        current => {
            let menu_open = current == RowMode::MenuOpen;
            let has_menu = actions.can_rename || actions.delete != DeleteAction::Unavailable;
            rsx! {
                div { class: "sidebar-row",
                    SidebarItem {
                        title: entry.title.clone(),
                        subtitle: entry
                            .updated_at
                            .as_deref()
                            .map(|updated_at| relative_time(updated_at, now))
                            .unwrap_or_default(),
                        marker: entry.marker,
                        active,
                        onclick: move |_| {
                            mode.set(RowMode::Normal);
                            onopen.call(());
                        },
                        if has_menu {
                            button {
                                class: if menu_open { "row-menu-button open" } else { "row-menu-button" },
                                title: "Rename or delete",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    mode.set(if menu_open { RowMode::Normal } else { RowMode::MenuOpen });
                                },
                                "⋯"
                            }
                        }
                    }
                    if menu_open {
                        div { class: "row-menu", onclick: move |event| event.stop_propagation(),
                            if actions.can_rename {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let title = entry.title.clone();
                                        move |_| mode.set(RowMode::Renaming(title.clone()))
                                    },
                                    "Rename"
                                }
                            }
                            button {
                                class: "row-menu-item danger",
                                disabled: actions.delete != DeleteAction::Allowed,
                                onclick: move |_| mode.set(RowMode::ConfirmingDelete),
                                "Delete"
                            }
                            if actions.delete == DeleteAction::WhileOpen {
                                div { class: "row-menu-note", "Close the session first (⌘W) to delete it." }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// What a rename input should save: the trimmed draft, unless it is empty or the title the session
/// already has (then closing the input is all that is needed).
pub fn rename_submit(draft: &str, original: &str) -> Option<String> {
    let trimmed = draft.trim();
    if trimmed.is_empty() || trimmed == original.trim() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_changed_title_is_saved_trimmed() {
        assert_eq!(rename_submit("  New name ", "Old name"), Some("New name".to_string()));
    }

    #[test]
    fn an_empty_or_blank_draft_saves_nothing() {
        assert_eq!(rename_submit("", "Old"), None);
        assert_eq!(rename_submit("   ", "Old"), None);
    }

    #[test]
    fn an_unchanged_title_saves_nothing() {
        assert_eq!(rename_submit("Old", "Old"), None);
        assert_eq!(rename_submit(" Old ", "Old"), None);
    }
}
