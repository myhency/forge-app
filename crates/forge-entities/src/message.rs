use dioxus::prelude::*;
use forge_domain::{Message, MessageRole, ToolCall};

use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
use crate::markdown::render_markdown;
use crate::tool_summary::{tool_call_status, tool_detail_text, tool_input_summary, ToolCallStatus};
use crate::transcript_view::TranscriptView;

/// The text of a chip on a sent message: an icon and the file name.
pub fn attachment_label(attachment: &forge_domain::Attachment) -> String {
    let icon = match attachment.kind {
        forge_domain::AttachmentKind::Image => "🖼",
        forge_domain::AttachmentKind::File => "📄",
    };
    format!("{icon} {}", attachment.file_name())
}

#[component]
pub fn MessageBubble(message: Message, #[props(default)] view: TranscriptView) -> Element {
    if !view.shows(&message) {
        return rsx! {};
    }
    let (role_class, role_label) = match message.role {
        MessageRole::User => ("user", "You"),
        MessageRole::Assistant => ("assistant", "Claude"),
    };
    // Only Claude's text is markdown; what a person typed is shown exactly as typed.
    let markdown = match message.role {
        MessageRole::Assistant if !message.content.is_empty() => Some(render_markdown(&message.content)),
        _ => None,
    };
    rsx! {
        div { class: "message {role_class}",
            div { class: "role", "{role_label}" }
            if view.shows_thinking() && !message.thinking.is_empty() {
                div { class: "thinking", "{message.thinking}" }
            }
            if let Some(html) = markdown {
                div { class: "content markdown", dangerous_inner_html: "{html}" }
            } else if !message.content.is_empty() {
                div { class: "content", "{message.content}" }
            }
            if !message.attachments.is_empty() {
                div { class: "chips",
                    for attachment in message.attachments.iter() {
                        span { class: "chip", title: "{attachment.path.display()}", "{attachment_label(attachment)}" }
                    }
                }
            }
            for tool_call in message.tool_calls.iter() {
                ToolCallLine { key: "{tool_call.id}", tool_call: tool_call.clone(), detailed: view.expands_tool_calls() }
            }
        }
    }
}

/// One collapsed line per tool call: `▸ Read  src/lib.rs`. An Edit, MultiEdit or Write line is a
/// button that expands the diff underneath; every other call is plain text. `detailed` opens every
/// call: the diff of an edit, and for the others the input and the result underneath the line.
#[component]
pub fn ToolCallLine(tool_call: ToolCall, #[props(default)] detailed: bool) -> Element {
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
            if detailed {
                ToolDetail { tool_call: tool_call.clone() }
            }
        };
    };
    let open = expanded() || detailed;
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

/// The input of a tool call and, once it has one, its result.
#[component]
fn ToolDetail(tool_call: ToolCall) -> Element {
    let input = tool_detail_text(&serde_json::to_string_pretty(&tool_call.input).unwrap_or_default());
    let result = tool_call.result.as_ref().map(|result| {
        (
            if result.is_error { "tool-output failed" } else { "tool-output" },
            tool_detail_text(&result.output),
        )
    });
    rsx! {
        div { class: "tool-detail",
            pre { class: "tool-input", "{input}" }
            if let Some((class, output)) = result {
                pre { class: "{class}", "{output}" }
            }
        }
    }
}

#[cfg(test)]
mod attachment_tests {
    use super::*;
    use forge_domain::{Attachment, AttachmentKind};

    #[test]
    fn a_chip_shows_an_icon_and_the_file_name_only() {
        let image = Attachment { path: "/내 문서/사진 1.png".into(), kind: AttachmentKind::Image };
        let file = Attachment { path: "/x/notes.txt".into(), kind: AttachmentKind::File };
        assert_eq!(attachment_label(&image), "🖼 사진 1.png");
        assert_eq!(attachment_label(&file), "📄 notes.txt");
    }
}
