use std::path::PathBuf;

use dioxus::prelude::*;
use forge_entities::{relative_time, SidebarItem};
use forge_features::{menu_key_action, refocus_workspace, use_workspace, DeleteAction, MenuKeyAction, SidebarEntry};
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
pub fn SessionRow(
    project: PathBuf,
    entry: SidebarEntry,
    active: bool,
    onopen: EventHandler<()>,
    /// Set in date and no-grouping lists, where the heading does not say which project a row is in.
    #[props(default)]
    project_name: Option<String>,
) -> Element {
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
                        onblur: move |_| {
                            mode.set(RowMode::Normal);
                            refocus_workspace();
                        },
                        onkeydown: {
                            let draft = draft.clone();
                            move |event: KeyboardEvent| {
                                // Only the keys the input consumes stop here; everything else,
                                // the app shortcuts included, bubbles to the workspace root.
                                match event.key() {
                                    // Enter that ends an IME composition (Korean, Japanese, Chinese)
                                    // confirms the text being composed, not the rename.
                                    Key::Enter if !event.is_composing() => {
                                        event.stop_propagation();
                                        event.prevent_default();
                                        if let (Some(title), Some(id)) = (rename_submit(&draft, &original), session_id.clone()) {
                                            workspace.rename_session(project.clone(), id, title);
                                        }
                                        mode.set(RowMode::Normal);
                                        refocus_workspace();
                                    }
                                    Key::Escape => {
                                        event.stop_propagation();
                                        mode.set(RowMode::Normal);
                                        refocus_workspace();
                                    }
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
                    Button {
                        label: "Cancel",
                        onclick: move |_| {
                            mode.set(RowMode::Normal);
                            refocus_workspace();
                        },
                    }
                    Button {
                        label: "Delete",
                        kind: ButtonKind::Danger,
                        onclick: move |_| {
                            if let Some(id) = session_id.clone() {
                                workspace.delete_session(project.clone(), id);
                            }
                            mode.set(RowMode::Normal);
                            refocus_workspace();
                        },
                    }
                }
            }
        }
        current => {
            let menu_open = current == RowMode::MenuOpen;
            let has_menu = actions.can_rename || actions.can_fork || actions.delete != DeleteAction::Unavailable;
            rsx! {
                div { class: "sidebar-row",
                    SidebarItem {
                        title: entry.title.clone(),
                        subtitle: row_subtitle(
                            project_name.as_deref(),
                            entry.updated_at.as_deref().map(|updated_at| relative_time(updated_at, now)),
                        ),
                        marker: entry.marker,
                        active,
                        onclick: move |_| {
                            mode.set(RowMode::Normal);
                            onopen.call(());
                        },
                        if has_menu {
                            button {
                                class: if menu_open { "row-menu-button open" } else { "row-menu-button" },
                                title: "Rename, fork or delete",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    mode.set(if menu_open { RowMode::Normal } else { RowMode::MenuOpen });
                                },
                                "⋯"
                            }
                        }
                    }
                    if menu_open {
                        div {
                            class: "row-menu",
                            tabindex: "0",
                            onclick: move |event| event.stop_propagation(),
                            // The menu takes focus when it opens so its letters work at once.
                            onmounted: move |mounted| async move {
                                let _ = mounted.set_focus(true).await;
                            },
                            onkeydown: {
                                let title = entry.title.clone();
                                move |event: KeyboardEvent| {
                                    if event.key() == Key::Escape {
                                        event.stop_propagation();
                                        mode.set(RowMode::Normal);
                                        refocus_workspace();
                                        return;
                                    }
                                    let modified = event
                                        .modifiers()
                                        .intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT);
                                    match menu_key_action(&event.code().to_string(), modified, actions) {
                                        Some(MenuKeyAction::Rename) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            mode.set(RowMode::Renaming(title.clone()));
                                        }
                                        Some(MenuKeyAction::Fork) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            if let Some(id) = session_id.clone() {
                                                workspace.fork_session(project.clone(), id, title.clone());
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
                                        Some(MenuKeyAction::Delete) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            mode.set(RowMode::ConfirmingDelete);
                                        }
                                        None => {}
                                    }
                                }
                            },
                            if actions.can_rename {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let title = entry.title.clone();
                                        move |_| mode.set(RowMode::Renaming(title.clone()))
                                    },
                                    span { "Rename" }
                                    span { class: "menu-key", "R" }
                                }
                            }
                            if actions.can_fork {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let project = project.clone();
                                        let session_id = session_id.clone();
                                        let title = entry.title.clone();
                                        move |_| {
                                            if let Some(id) = session_id.clone() {
                                                workspace.fork_session(project.clone(), id, title.clone());
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
                                    },
                                    span { "Fork" }
                                    span { class: "menu-key", "F" }
                                }
                            }
                            button {
                                class: "row-menu-item danger",
                                disabled: actions.delete != DeleteAction::Allowed,
                                onclick: move |_| mode.set(RowMode::ConfirmingDelete),
                                span { "Delete" }
                                span { class: "menu-key", "D" }
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

/// The grey text at the right of a row: how long ago, or in a date or no-grouping list the project
/// (the date heading already says when, and a longer text squeezes the title to a few letters).
pub fn row_subtitle(project_name: Option<&str>, ago: Option<String>) -> String {
    match project_name {
        Some(name) => name.to_string(),
        None => ago.unwrap_or_default(),
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

    #[test]
    fn a_row_subtitle_is_the_project_in_a_date_list_and_the_time_otherwise() {
        assert_eq!(row_subtitle(None, Some("3d ago".to_string())), "3d ago");
        // The time is left out beside a project name: the long subtitle squeezed titles to a few letters.
        assert_eq!(row_subtitle(Some("forge"), Some("3d ago".to_string())), "forge");
        assert_eq!(row_subtitle(Some("forge"), None), "forge");
        assert_eq!(row_subtitle(None, None), "");
    }
}
