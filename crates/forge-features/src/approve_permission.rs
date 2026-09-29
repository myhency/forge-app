use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::PermissionDecision;
use forge_shared::{Button, ButtonKind};

use crate::workspace::use_workspace;

/// Allow once / allow always / deny for one permission request.
#[component]
pub fn PermissionActions(id: LiveSessionId, request_id: String) -> Element {
    let workspace = use_workspace();
    let (once, always, deny) = (request_id.clone(), request_id.clone(), request_id);

    rsx! {
        div { class: "permission-actions",
            Button {
                label: "Allow",
                kind: ButtonKind::Primary,
                onclick: move |_| workspace.answer_permission(id, once.clone(), PermissionDecision::AllowOnce),
            }
            Button {
                label: "Always allow",
                onclick: move |_| workspace.answer_permission(id, always.clone(), PermissionDecision::AllowAlways),
            }
            Button {
                label: "Deny",
                kind: ButtonKind::Danger,
                onclick: move |_| workspace.answer_permission(id, deny.clone(), PermissionDecision::Deny),
            }
        }
    }
}
