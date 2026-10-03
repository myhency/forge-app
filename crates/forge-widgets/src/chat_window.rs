use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{MessageBubble, PermissionCard, TranscriptView};
use forge_features::{use_workspace, PermissionActions, SendMessageBox};

use crate::composer_controls::{ModePicker, ModelPicker};
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
    let heading = format!(
        "{} · {}",
        project_label(&view.project_path),
        view.session.title.as_deref().unwrap_or("Session")
    );

    rsx! {
        div { class: "main-pane",
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
            }
            div { class: "messages",
                for (index, message) in view.session.messages.iter().enumerate() {
                    MessageBubble { key: "{index}", message: message.clone(), view: mode }
                }
            }
            if let Some(error) = view.last_error.as_ref() {
                div { class: "banner error", "{error}" }
            }
            if let Some(request) = view.pending_permissions.front() {
                PermissionCard { request: request.clone(),
                    PermissionActions { id, request_id: request.id.clone() }
                }
            }
            if view.can_send {
                SendMessageBox { id, running: view.phase == Phase::Running, can_send: true,
                    ModePicker { id }
                    ModelPicker { id }
                    UsageRing { id }
                }
            } else {
                div { class: "banner info",
                    "The project folder no longer exists, so this session can only be viewed."
                }
            }
        }
    }
}
