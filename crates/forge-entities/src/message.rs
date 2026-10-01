use dioxus::prelude::*;
use forge_domain::{Message, MessageRole, ToolCall};

use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
use crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let (role_class, role_label) = match message.role {
        MessageRole::User => ("user", "You"),
        MessageRole::Assistant => ("assistant", "Claude"),
    };
    rsx! {
        div { class: "message {role_class}",
            div { class: "role", "{role_label}" }
            if !message.content.is_empty() {
                div { class: "content", "{message.content}" }
            }
            for tool_call in message.tool_calls.iter() {
                ToolCallLine { key: "{tool_call.id}", tool_call: tool_call.clone() }
            }
        }
    }
}

/// One collapsed line per tool call: `▸ Read  src/lib.rs`. An Edit, MultiEdit or Write line is a
/// button that expands the diff underneath; every other call is plain text.
#[component]
pub fn ToolCallLine(tool_call: ToolCall) -> Element {
    let mut expanded = use_signal(|| false);
    let status_class = match tool_call_status(&tool_call) {
        ToolCallStatus::Running => "running",
        ToolCallStatus::Done => "done",
        ToolCallStatus::Failed => "failed",
    };
    let summary = tool_input_summary(&tool_call.input);
    let Some(diff) = diff_for_tool_call(&tool_call) else {
        return rsx! {
            div { class: "tool-call {status_class}", "▸ {tool_call.name}  {summary}" }
        };
    };
    let open = expanded();
    let arrow = if open { "▾" } else { "▸" };
    rsx! {
        button {
            class: "tool-call expandable {status_class}",
            r#type: "button",
            "aria-expanded": "{open}",
            onclick: move |_| expanded.set(!open),
            "{arrow} {tool_call.name}  {summary}"
        }
        if open {
            DiffView { diff: diff.clone() }
        }
    }
}
