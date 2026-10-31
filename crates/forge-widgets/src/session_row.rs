use std::path::PathBuf;

use dioxus::prelude::*;
use forge_entities::{relative_time, SidebarItem};
use forge_features::{menu_key_action, refocus_workspace, use_workspace, ArchiveAction, DeleteAction, EntryKey, MenuKeyAction, SidebarEntry};
use forge_shared::{tr, Button, ButtonKind, Icon, IconKind, IconSize};
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
    /// Shown in the other pane of a split screen.
    #[props(default)]
    shown: bool,
    /// The row was clicked; `true` when Option was held (open it beside).
    onopen: EventHandler<bool>,
    /// Set in date and no-grouping lists, where the heading does not say which project a row is in.
    #[props(default)]
    project_name: Option<String>,
    /// How many forks deep the row is drawn (0 = not under another session).
    #[props(default)]
    depth: u8,
    /// Forks under this session, shown or folded; a fold button is drawn when there are any.
    #[props(default)]
    fork_count: usize,
    #[props(default)]
    forks_folded: bool,
) -> Element {
    let workspace = use_workspace();
    let mut mode = use_signal(|| RowMode::Normal);
    let actions = entry.actions();
    let now = OffsetDateTime::now_utc();
    let session_id = entry.session_id.clone();

    // ⌥⌘R asks the active session's row to start renaming.
    let own_key = entry.key.clone();
    let own_title = entry.title.clone();
    // Follows the props: the title changes while the row lives (the first message titles a new
    // session), and the draft must start from the current one.
    use_effect(use_reactive!(|own_key, own_title| {
        if workspace.rename_request().as_ref() == Some(&own_key) {
            workspace.clear_rename_request();
            mode.set(RowMode::Renaming(own_title));
        }
    }));

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
                        // Focus and select at once; the value is the draft the row opened with and
                        // never changes while typing, so an IME composition is left alone.
                        onmounted: move |_| {
                            document::eval("const input = document.querySelector('.rename-input'); if (input) { input.focus(); input.select(); }");
                        },
                        // A click in the field must not open the session behind it.
                        onclick: move |event| event.stop_propagation(),
                        onblur: move |_| {
                            mode.set(RowMode::Normal);
                            refocus_workspace();
                        },
                        onkeydown: {
                            move |event: KeyboardEvent| {
                                // Only the keys the input consumes stop here; everything else,
                                // the app shortcuts included, bubbles to the workspace root.
                                match event.key() {
                                    // Enter that ends an IME composition (Korean, Japanese, Chinese)
                                    // confirms the text being composed, not the rename.
                                    Key::Enter if !event.is_composing() => {
                                        event.stop_propagation();
                                        event.prevent_default();
                                        // The text is read from the element only now.
                                        let (original, project, session_id) = (original.clone(), project.clone(), session_id.clone());
                                        spawn(async move {
                                            let text = document::eval("return document.querySelector('.rename-input')?.value ?? null;")
                                                .join::<Option<String>>()
                                                .await
                                                .ok()
                                                .flatten();
                                            if let (Some(text), Some(id)) = (text, session_id) {
                                                if let Some(title) = rename_submit(&text, &original) {
                                                    workspace.rename_session(project, id, title);
                                                }
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        });
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
            let delete_question = tr!("Delete “{title}”? Moves the session to the trash.", title = entry.title);
            rsx! {
                div { class: "confirm-row",
                    span { class: "confirm-text", "{delete_question}" }
                    Button {
                        label: tr!("Cancel"),
                        onclick: move |_| {
                            mode.set(RowMode::Normal);
                            refocus_workspace();
                        },
                    }
                    Button {
                        label: tr!("Delete"),
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
            let has_menu = actions.can_rename
                || actions.can_fork
                || actions.archive != ArchiveAction::Unavailable
                || actions.delete != DeleteAction::Unavailable;
            let plan_hint = match entry.key {
                EntryKey::Live(id) => workspace.plan_ready_hint(id).unwrap_or_default(),
                _ => String::new(),
            };
            let shown_title = entry.title.clone();
            let shown_title = if depth > 0 { format!("└ {shown_title}") } else { shown_title };
            let fold_id = session_id.as_ref().map(forge_features::fork_fold_id);
            rsx! {
                div {
                    class: "sidebar-row",
                    style: "margin-left: {fork_indent(depth)}px",
                    title: "{plan_hint}",
                    SidebarItem {
                        title: shown_title,
                        archived: entry.archived,
                        subtitle: row_subtitle(
                            project_name.as_deref(),
                            entry.updated_at.as_deref().map(|updated_at| relative_time(updated_at, now)),
                        ),
                        marker: entry.marker,
                        active,
                        shown,
                        onclick: move |event: MouseEvent| {
                            mode.set(RowMode::Normal);
                            onopen.call(event.modifiers().alt());
                        },
                        if let (true, Some(fold_id)) = (fork_count > 0, fold_id.clone()) {
                            button {
                                class: "fork-fold",
                                title: if forks_folded { tr!("Show the forks of this session") } else { tr!("Fold the forks of this session") },
                                "aria-label": if forks_folded { "Show forks" } else { "Fold forks" },
                                onclick: move |event| {
                                    event.stop_propagation();
                                    workspace.update_prefs(|prefs| prefs.toggle_collapsed(&fold_id));
                                },
                                Icon { kind: IconKind::Chevron, size: IconSize::Small, open: !forks_folded }
                            }
                        }
                        if has_menu {
                            button {
                                class: if menu_open { "row-menu-button open" } else { "row-menu-button" },
                                title: tr!("Rename, fork, archive or delete"),
                                onclick: move |event| {
                                    event.stop_propagation();
                                    mode.set(if menu_open { RowMode::Normal } else { RowMode::MenuOpen });
                                },
                                Icon { kind: IconKind::MoreHorizontal }
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
                                        Some(MenuKeyAction::ToggleArchive) => {
                                            event.stop_propagation();
                                            event.prevent_default();
                                            if let Some(id) = session_id.clone() {
                                                match actions.archive {
                                                    ArchiveAction::Archive => workspace.archive_session(project.clone(), id),
                                                    _ => workspace.unarchive_session(project.clone(), id),
                                                }
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
                                    span { {tr!("Rename")} }
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
                                    span { {tr!("Fork")} }
                                    span { class: "menu-key", "F" }
                                }
                            }
                            if actions.archive != ArchiveAction::Unavailable {
                                button {
                                    class: "row-menu-item",
                                    onclick: {
                                        let project = project.clone();
                                        let session_id = session_id.clone();
                                        move |_| {
                                            if let Some(id) = session_id.clone() {
                                                match actions.archive {
                                                    ArchiveAction::Archive => workspace.archive_session(project.clone(), id),
                                                    _ => workspace.unarchive_session(project.clone(), id),
                                                }
                                            }
                                            mode.set(RowMode::Normal);
                                            refocus_workspace();
                                        }
                                    },
                                    span { if actions.archive == ArchiveAction::Archive { {tr!("Archive")} } else { {tr!("Unarchive")} } }
                                    span { class: "menu-key", "A" }
                                }
                            }
                            button {
                                class: "row-menu-item danger",
                                disabled: actions.delete != DeleteAction::Allowed,
                                onclick: move |_| mode.set(RowMode::ConfirmingDelete),
                                span { {tr!("Delete")} }
                                span { class: "menu-key", "D" }
                            }
                            if actions.delete == DeleteAction::WhileOpen {
                                div { class: "row-menu-note", {tr!("Close the session first ({key}) to delete it.", key = forge_domain::KEY_CLOSE_SESSION)} }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// How far a fork row is moved in (pixels); a deep chain stops moving at the third step.
pub fn fork_indent(depth: u8) -> u32 {
    u32::from(depth.min(3)) * 14
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

    #[test]
    fn a_fork_row_moves_in_one_step_per_level_and_stops_at_the_third() {
        assert_eq!(fork_indent(0), 0);
        assert_eq!(fork_indent(1), 14);
        assert_eq!(fork_indent(3), 42);
        assert_eq!(fork_indent(9), 42);
    }
}
