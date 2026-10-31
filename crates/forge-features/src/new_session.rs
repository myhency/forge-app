use std::path::Path;

use dioxus::prelude::*;
use forge_domain::{EffortLevel, PermissionMode, ProjectSummary};

use crate::command_box::CommandTextArea;
use crate::environment::{folder_missing, start_plan, StartError};
use crate::composer_shell::{composer_action, ComposerShell};
use crate::composer_state::{use_composer_state_provider, ComposerChips, TrustCard};
use crate::workspace::use_workspace;
use crate::worktree_view::{NewWorktree, RepoInfo};
use forge_shared::tr;

/// What was chosen for the new session before it exists: `None` follows the saved default model,
/// `Some(None)` is the SDK's default, `Some(Some(id))` is that model.
pub type ModelChoice = Option<Option<String>>;

/// The mode, model and effort picked on the new-session screen. Nothing is saved to settings.
/// An effort of `None` is the SDK's default.
#[derive(Clone, Copy)]
pub struct NewSessionDraft {
    pub mode: Signal<PermissionMode>,
    pub model: Signal<ModelChoice>,
    pub effort: Signal<Option<EffortLevel>>,
    pub worktree: Signal<bool>,
    pub branch: Signal<Option<String>>,
    /// The saved environment picked, or `None` for this computer.
    pub environment: Signal<Option<String>>,
}

pub fn use_new_session_draft() -> NewSessionDraft {
    use_context::<NewSessionDraft>()
}

/// Whether the send button is ready: there is text or something attached.
pub fn send_ready(text: &str, attachments: usize) -> bool {
    !text.trim().is_empty() || attachments > 0
}

/// Whether the composer can send: an environment is picked, `폴더 없음` is chosen, or a past project exists.
pub fn can_send_here(picked_environment: bool, no_folder: bool, projects: usize) -> bool {
    picked_environment || no_folder || projects > 0
}

/// The worktree a send asks for: only when the switch is on and the folder being started in is the
/// repository the panel probed.
pub fn worktree_request(on: bool, branch: Option<String>, repo: Option<RepoInfo>, folder: &Path) -> Option<NewWorktree> {
    (on && repo.is_some_and(|info| info.folder == folder)).then_some(NewWorktree { base: branch })
}

/// The new-session panel: the first message in the same box an open session uses. `leading` is the `+`, `start` the
/// environment and folder pickers at the left of the line under the box, `end` the pickers at its right. The session is
/// only created when that message is sent.
#[component]
pub fn NewSessionComposer(above: Element, leading: Element, start: Element, end: Element) -> Element {
    let workspace = use_workspace();
    let draft = use_context_provider(|| NewSessionDraft { mode: Signal::new(PermissionMode::Default), model: Signal::new(None), effort: Signal::new(None), worktree: Signal::new(true), branch: Signal::new(None), environment: Signal::new(None) });
    let text = use_signal(String::new);
    let composer = use_composer_state_provider(text);
    let mut text = text;

    let previous_folder = use_signal(|| workspace.chosen_new_folder());

    // The probe follows the chosen folder: it is replaced when the folder changes, kept when it does
    // not, and ended when this panel goes away (send, another session, the settings).
    use_effect(move || {
        let folder = workspace.chosen_new_folder();
        // Another folder starts from its own current branch; the same folder (a reloaded project
        // list) keeps the branch that was chosen.
        if *previous_folder.peek() != folder {
            let mut branch = draft.branch;
            branch.set(None);
            let mut previous = previous_folder;
            previous.set(folder.clone());
        }
        let no_folder = workspace.effective_no_folder();
        spawn(async move {
            workspace.probe_new_commands(no_folder).await;
            workspace.probe_new_repo(folder);
        });
    });
    use_drop(move || workspace.probe_new_folder(None));

    // A message whose worktree could not be made comes back into the text box.
    use_effect(move || {
        if let Some(back) = workspace.new_unsent() {
            workspace.clear_new_unsent();
            text.set(back);
        }
    });

    // An environment deleted while it was picked: back to this computer.
    use_effect(move || {
        let picked = draft.environment.read().clone();
        if let Some(id) = picked {
            if !workspace.environments().iter().any(|environment| environment.id == id) {
                let mut picked_signal = draft.environment;
                picked_signal.set(None);
            }
        }
    });

    let picked_environment = draft.environment.read().clone().and_then(|id| workspace.environments().into_iter().find(|environment| environment.id == id));
    let has_folder = can_send_here(picked_environment.is_some(), workspace.effective_no_folder(), workspace.new_session_projects().len());

    // `submit` is used by two handlers, so it may capture only `Copy` values (signals): it looks the
    // folder up again instead of capturing a string.
    let mut submit = move || {
        if !send_ready(&text.read(), composer.attachments().len()) {
            return;
        }
        let environment = draft.environment.peek().clone().and_then(|id| workspace.environments().into_iter().find(|environment| environment.id == id));
        let (project, plan) = match &environment {
            Some(environment) => match start_plan(environment) {
                Ok(plan) => {
                    if folder_missing(&plan) {
                        workspace.report(tr!("The folder of this environment no longer exists. Edit the environment.").to_string());
                        return;
                    }
                    (ProjectSummary { path: plan.folder.clone(), last_active: String::new(), session_count: 0, exists: true }, Some(plan))
                }
                Err(StartError::NoFolder) => {
                    workspace.report(tr!("Add a folder to this environment first").to_string());
                    return;
                }
                Err(StartError::Variables(error)) => {
                    workspace.report(tr!("The variables of this environment are not valid (line {line}). Edit the environment.", line = error.line));
                    return;
                }
            },
            None if workspace.effective_no_folder() => {
                // Only a placeholder: `start_new` makes the folder once the text is taken.
                (ProjectSummary { path: std::path::PathBuf::new(), last_active: String::new(), session_count: 0, exists: true }, None)
            }
            None => {
                let projects = workspace.new_session_projects();
                let project = match workspace.new_folder() {
                    Some(path) => projects.iter().find(|project| project.path == path).cloned().or_else(|| projects.first().cloned()),
                    None => projects.first().cloned(),
                };
                let Some(project) = project else {
                    return;
                };
                (project, None)
            }
        };
        let Some(outgoing) = composer.take_for_new_session(&text.read()) else {
            return;
        };
        let no_folder = environment.is_none() && workspace.effective_no_folder();
        let worktree = match &plan {
            None if no_folder => None,
            Some(plan) => plan.worktree.clone(),
            None => worktree_request(*draft.worktree.peek(), draft.branch.peek().clone(), workspace.new_repo(), &project.path),
        };
        workspace.start_new(
            project,
            *draft.mode.peek(),
            draft.model.peek().clone(),
            *draft.effort.peek(),
            outgoing.text,
            outgoing.attachments,
            composer.take_folders(),
            worktree,
            plan,
            no_folder,
        );
        if let Some(environment) = environment {
            workspace.touch_environment(environment.id);
        }
        text.set(String::new());
    };
    let ready = send_ready(&text.read(), composer.attachments().len());

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", {tr!("No existing project folder was found in your Claude Code history. Use “Open folder…” to choose one.")} }
            }
            ComposerShell {
                above,
                top: rsx! {
                    if let Some(message) = composer.error() {
                        div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
                    }
                    ComposerChips {}
                    TrustCard {}
                },
                overlay: rsx! {},
                leading,
                input: rsx! {
                    CommandTextArea {
                        value: text(),
                        open_token: composer.popup_token(),
                        commands: workspace.new_commands(),
                        placeholder: tr!("Describe a task or ask a question"),
                        disabled: !has_folder,
                        oninput: move |value| text.set(value),
                        onsubmit: move |_| submit(),
                    }
                },
                action: composer_action(false, ready, !has_folder || !ready),
                onsend: move |_| submit(),
                onstop: move |_| {},
                start,
                end,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_composer_can_send_with_an_environment_no_folder_or_a_project() {
        assert!(can_send_here(true, false, 0));
        assert!(can_send_here(false, true, 0));
        assert!(can_send_here(false, false, 1));
        assert!(!can_send_here(false, false, 0));
    }

    #[test]
    fn the_send_button_is_ready_with_text_or_an_attachment() {
        assert!(!send_ready("", 0));
        assert!(!send_ready("  \n ", 0));
        assert!(send_ready("hi", 0));
        assert!(send_ready("", 1));
    }
}

#[cfg(test)]
mod worktree_request_tests {
    use super::*;
    use std::path::PathBuf;

    use crate::worktree_view::RepoInfo;
    use forge_ports::BranchList;

    fn repo(folder: &str) -> Option<RepoInfo> {
        Some(RepoInfo { folder: PathBuf::from(folder), branches: BranchList::default() })
    }

    #[test]
    fn a_repository_with_the_switch_on_asks_for_a_worktree_from_the_chosen_branch() {
        let asked = worktree_request(true, Some("dev".to_string()), repo("/a"), std::path::Path::new("/a"));
        assert_eq!(asked, Some(NewWorktree { base: Some("dev".to_string()) }));
    }

    #[test]
    fn no_worktree_when_the_switch_is_off_or_the_folder_is_not_the_probed_repository() {
        assert_eq!(worktree_request(false, None, repo("/a"), std::path::Path::new("/a")), None);
        assert_eq!(worktree_request(true, None, repo("/a"), std::path::Path::new("/b")), None);
        assert_eq!(worktree_request(true, None, None, std::path::Path::new("/a")), None);
    }
}
