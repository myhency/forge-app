use dioxus::prelude::*;
use forge_domain::PermissionRequest;

use crate::tool_summary::tool_input_summary;

/// Shows what Claude wants to do. The buttons are the caller's `children`, so this entity stays
/// presentational and the approve-permission feature supplies the behaviour.
#[component]
pub fn PermissionCard(request: PermissionRequest, children: Element) -> Element {
    let summary = tool_input_summary(&request.input);
    rsx! {
        div { class: "permission-card",
            div { class: "tool", "{request.tool_name}" }
            div { class: "input", "{summary}" }
            {children}
        }
    }
}
