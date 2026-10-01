use dioxus::prelude::*;
use forge_domain::PermissionRequest;

use crate::diff::diff_for_tool;
use crate::diff_view::DiffView;
use crate::tool_summary::tool_input_summary;

/// Shows what Claude wants to do — with the diff, for an Edit or Write, so the change can be
/// read before it is allowed. The buttons are the caller's `children`, so this entity stays
/// presentational and the approve-permission feature supplies the behaviour.
#[component]
pub fn PermissionCard(request: PermissionRequest, children: Element) -> Element {
    let summary = tool_input_summary(&request.input);
    let diff = diff_for_tool(&request.tool_name, &request.input);
    rsx! {
        div { class: "permission-card",
            div { class: "tool", "{request.tool_name}" }
            div { class: "input", "{summary}" }
            if let Some(diff) = diff {
                DiffView { diff }
            }
            {children}
        }
    }
}
