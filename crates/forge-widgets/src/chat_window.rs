use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{PermissionCard, TranscriptView, Turns};
use forge_features::{use_workspace, PermissionActions, SendMessageBox};

use crate::composer_controls::{EffortPicker, ModePicker, ModelPicker};
use crate::dock::Dock;
use crate::plus_menu::PlusMenu;
use crate::session_tools::SessionTools;
use crate::sidebar::project_label;
use crate::usage_ring::UsageRing;

/// One open session: its messages, any permission request waiting for an answer, and the message box.
#[component]
pub fn ChatWindow(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! { div { class: "empty-state", "This session is no longer open." } };
    };
    let view = view_signal.read();
    let mode = workspace.transcript_view(id);
    let archived = view
        .claude_session_id
        .as_ref()
        .is_some_and(|session| workspace.is_archived(&view.project_path, session));
    let dock_expanded = workspace.dock(id).expanded.is_some() && !archived;
    let heading = format!(
        "{} · {}",
        project_label(&view.project_path),
        view.session.title.as_deref().unwrap_or("Session")
    );

    rsx! {
        div { class: if dock_expanded { "main-pane dock-expanded" } else { "main-pane" },
            div { class: "pane-header",
                span { class: "pane-title", "{heading}" }
                div { class: "view-switch", role: "radiogroup", "aria-label": "Transcript view",
                    for option in TranscriptView::ALL {
                        button {
                            key: "{option.label()}",
                            class: if option == mode { "view-option selected" } else { "view-option" },
                            role: "radio",
                            "aria-checked": "{option == mode}",
                            onclick: move |_| workspace.set_transcript_view(id, option),
                            "{option.label()}"
                        }
                    }
                }
                SessionTools { id, terminal: !archived }
            }
            div { class: "messages",
                Turns { messages: view.session.messages.clone(), view: mode, running: view.phase == Phase::Running }
            }
            if let Some(error) = view.last_error.as_ref() {
                div { class: "banner error", "{error}" }
            }
            if let Some(request) = view.pending_permissions.front() {
                PermissionCard { request: request.clone(),
                    PermissionActions { id, request_id: request.id.clone() }
                }
            }
            if archived {
                div { class: "banner info archived-bar",
                    span { "This session is archived." }
                    button {
                        class: "archived-bar-button",
                        onclick: {
                            let project = view.project_path.clone();
                            let session = view.claude_session_id.clone();
                            move |_| {
                                if let Some(session) = session.clone() {
                                    workspace.unarchive_session(project.clone(), session);
                                }
                            }
                        },
                        "Unarchive"
                    }
                }
            } else if view.can_send {
                SendMessageBox { id, running: view.phase == Phase::Running, can_send: true,
                    PlusMenu { project: Some(view.project_path.clone()), add_folder_enabled: true }
                    ModePicker { id }
                    ModelPicker { id }
                    EffortPicker { id }
                    UsageRing { id }
                }
            } else {
                div { class: "banner info",
                    "The project folder no longer exists, so this session can only be viewed."
                }
            }
        }
        if !archived {
            Dock { id, cwd: view.project_path.clone() }
        }
    }
}
