use dioxus::prelude::*;
use forge_features::{effort_levels, folder_menu_rows, refocus_composer, use_new_session_draft, use_settings, use_workspace};

use crate::composer_controls::{ModePickerView, ModelPickerView, Picker, PickerItem};
use forge_domain::EffortLevel;
use forge_shared::{tr, IconKind};

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
        items.push(PickerItem { label: tr!("No folder").to_string(), checked: no_folder, ..Default::default() });
    }
    // The desktop app lists the recent folders under their own heading, below `폴더 없음`.
    items.extend(rows.iter().enumerate().map(|(index, row)| PickerItem {
        label: row.label.clone(),
        checked: row.checked,
        divider_before: index == 0 && scratch,
        section: (index == 0).then(|| tr!("Recent").to_string()),
        ..Default::default()
    }));
    let draft = use_new_session_draft();
    if draft.environment.read().is_some() {
        return rsx! {};
    }
    items.push(PickerItem { label: tr!("Open folder…").to_string(), divider_before: true, ..Default::default() });
    // The desktop app names the button "Choose…" while no folder is chosen.
    let current_label = if scratch && no_folder {
        tr!("Choose…").to_string()
    } else {
        rows.iter().find(|row| row.checked).map_or_else(|| tr!("Choose a folder").to_string(), |row| row.label.clone())
    };
    let paths: Vec<_> = rows.iter().map(|row| row.path.clone()).collect();
    rsx! {
        Picker {
            title: tr!("Folder"),
            icon: IconKind::Folder,
            search: tr!("Search folders…").to_string(),
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
                    None => workspace.open_folder(),
                }
                refocus_composer();
            },
        }
    }
}

/// The worktree checkbox beside the folder, shown for a git folder (checked by default, like the desktop app): the
/// session then works in its own worktree, made from the folder's current branch.
#[component]
pub fn WorktreeCheckbox() -> Element {
    let workspace = use_workspace();
    let mut draft = use_new_session_draft();
    if draft.environment.read().is_some() || workspace.new_repo().is_none() {
        return rsx! {};
    }
    let on = *draft.worktree.read();
    rsx! {
        label { class: "chip-check",
            input {
                r#type: "checkbox",
                checked: on,
                onchange: move |event| draft.worktree.set(event.checked()),
            }
            span { {tr!("Worktree")} }
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

/// The model and effort for the session about to start; until a model is picked it is the saved default. The effort
/// stops are the levels the probed SDK lists for the model, so nothing is offered until the probe has answered. Like the
/// desktop app, a new session starts on Medium when the model takes it.
#[component]
pub fn NewModelPicker() -> Element {
    let mut draft = use_new_session_draft();
    let workspace = use_workspace();
    let settings = use_settings();
    let current = match draft.model.read().clone() {
        Some(chosen) => chosen,
        None => settings.model(),
    };
    let levels = effort_levels(&workspace.new_models(), current.as_deref());
    let effort = *draft.effort.read();
    // No effort chosen (a new session, or a model change that cleared it): Medium, when the model takes it.
    use_effect(move || {
        let model = match draft.model.read().clone() {
            Some(chosen) => chosen,
            None => settings.model(),
        };
        let takes_medium = effort_levels(&workspace.new_models(), model.as_deref()).contains(&EffortLevel::Medium);
        if takes_medium && draft.effort.read().is_none() {
            draft.effort.set(Some(EffortLevel::Medium));
        }
    });
    rsx! {
        ModelPickerView {
            current,
            levels,
            effort,
            onmodel: move |model| {
                // The new model may not take the level chosen for the old one.
                draft.effort.set(None);
                draft.model.set(Some(model));
            },
            oneffort: move |level| draft.effort.set(level),
        }
    }
}
