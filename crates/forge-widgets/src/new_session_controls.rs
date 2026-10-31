use dioxus::prelude::*;
use forge_features::{effort_levels, folder_menu_rows, refocus_composer, use_new_session_draft, use_settings, use_workspace};

use crate::composer_controls::{EffortPickerView, ModePickerView, ModelPickerView, Picker, PickerItem};
use forge_shared::{tr, Icon, IconKind};

/// The folder chip: the folder's name, and a menu of the recent folders with "Open folder…" below.
#[component]
pub fn FolderChip() -> Element {
    let workspace = use_workspace();
    let projects = workspace.new_session_projects();
    let current = workspace.chosen_new_folder();
    let rows = folder_menu_rows(&projects, current.as_deref());
    // With `폴더 없음` first, every index into `paths` is shifted by one.
    let scratch = workspace.scratch_available();
    let no_folder = workspace.effective_no_folder();
    let mut items: Vec<PickerItem> = Vec::new();
    if scratch {
        items.push(PickerItem { label: tr!("No folder").to_string(), description: None, key: None, checked: no_folder, divider_before: false });
    }
    items.extend(rows.iter().map(|row| PickerItem { label: row.label.clone(), description: None, key: None, checked: row.checked, divider_before: false }));
    let mut draft = use_new_session_draft();
    if draft.environment.read().is_some() {
        return rsx! {};
    }
    let repo = workspace.new_repo();
    items.push(PickerItem { label: tr!("Open folder…").to_string(), description: None, key: None, checked: false, divider_before: true });
    if repo.is_some() {
        items.push(PickerItem { label: tr!("Worktree").to_string(), description: None, key: None, checked: *draft.worktree.read(), divider_before: true });
    }
    let current_label = if scratch && no_folder {
        tr!("No folder").to_string()
    } else {
        rows.iter().find(|row| row.checked).map_or_else(|| tr!("Choose a folder").to_string(), |row| row.label.clone())
    };
    let paths: Vec<_> = rows.iter().map(|row| row.path.clone()).collect();
    rsx! {
        Picker {
            title: tr!("Folder"),
            heading: tr!("Recent"),
            icon: IconKind::Folder,
            current: current_label,
            items,
            onpick: move |index: usize| {
                if scratch && index == 0 {
                    workspace.set_new_no_folder(true);
                    workspace.set_new_folder(None);
                    refocus_composer();
                    return;
                }
                let index = if scratch { index - 1 } else { index };
                match paths.get(index) {
                    Some(path) => workspace.set_new_folder(Some(path.clone())),
                    None if index == paths.len() => workspace.open_folder(),
                    None => {
                        let on = *draft.worktree.peek();
                        draft.worktree.set(!on);
                    }
                }
                refocus_composer();
            },
        }
    }
}

/// The branch a worktree starts from; shown only for a git folder with the worktree row on.
#[component]
pub fn BranchChip() -> Element {
    let workspace = use_workspace();
    let mut draft = use_new_session_draft();
    if draft.environment.read().is_some() {
        return rsx! {};
    }
    let Some(info) = workspace.new_repo() else { return rsx! {} };
    if !*draft.worktree.read() {
        return rsx! {};
    }
    let chosen = draft.branch.read().clone().or_else(|| info.branches.current.clone());
    let items: Vec<PickerItem> = info
        .branches
        .local
        .iter()
        .map(|name| PickerItem { label: name.clone(), description: None, key: None, checked: chosen.as_deref() == Some(name.as_str()), divider_before: false })
        .collect();
    let names = info.branches.local.clone();
    let label = chosen.unwrap_or_else(|| tr!("Detached HEAD").to_string());
    rsx! {
        Picker {
            title: tr!("Branch"),
            heading: tr!("Branches"),
            current: label,
            items,
            onpick: move |index: usize| {
                if let Some(name) = names.get(index) {
                    draft.branch.set(Some(name.clone()));
                }
                refocus_composer();
            },
        }
    }
}

/// A button that opens the native folder dialog, like "Open folder…" in the menu.
#[component]
pub fn AddFolderButton() -> Element {
    let workspace = use_workspace();
    let draft = use_new_session_draft();
    if draft.environment.read().is_some() {
        return rsx! {};
    }
    rsx! {
        button {
            class: "chip-button",
            r#type: "button",
            title: tr!("Open folder"),
            disabled: workspace.picking_folder(),
            onclick: move |_| workspace.open_folder(),
            Icon { kind: IconKind::FolderPlus }
        }
    }
}

/// The mode for the session about to start.
#[component]
pub fn NewModePicker() -> Element {
    let mut draft = use_new_session_draft();
    let current = *draft.mode.read();
    rsx! { ModePickerView { current, onchange: move |mode| draft.mode.set(mode) } }
}

/// The model for the session about to start; until one is picked it is the saved default.
#[component]
pub fn NewModelPicker() -> Element {
    let mut draft = use_new_session_draft();
    let settings = use_settings();
    let current = match draft.model.read().clone() {
        Some(chosen) => chosen,
        None => settings.model(),
    };
    rsx! {
        div { class: "picker-end",
            ModelPickerView {
                current,
                onchange: move |model| {
                    // The new model may not take the level chosen for the old one.
                    draft.effort.set(None);
                    draft.model.set(Some(model));
                },
            }
        }
    }
}

/// The effort for the session about to start. The stops are the levels the probed SDK lists for the
/// chosen model, so nothing shows until the probe has answered.
#[component]
pub fn NewEffortPicker() -> Element {
    let mut draft = use_new_session_draft();
    let workspace = use_workspace();
    let settings = use_settings();
    let model = match draft.model.read().clone() {
        Some(chosen) => chosen,
        None => settings.model(),
    };
    let levels = effort_levels(&workspace.new_models(), model.as_deref());
    let current = *draft.effort.read();
    rsx! {
        div { class: "picker-end",
            EffortPickerView { levels, current, onchange: move |level| draft.effort.set(level) }
        }
    }
}
