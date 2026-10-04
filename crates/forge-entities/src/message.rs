use dioxus::prelude::*;
use forge_domain::ToolCall;

use crate::tool_summary::{tool_detail_text, tool_input_lines};

/// The text of a chip on a sent message: an icon and the file name.
pub fn attachment_label(attachment: &forge_domain::Attachment) -> String {
    let icon = match attachment.kind {
        forge_domain::AttachmentKind::Image => "🖼",
        forge_domain::AttachmentKind::File => "📄",
    };
    format!("{icon} {}", attachment.file_name())
}

/// The input of a tool call as `key: value` lines and, once it has one, its result. A failed call
/// shows its error text first, in red (a denied call's text tells what was rejected).
#[component]
pub(crate) fn ToolDetail(tool_call: ToolCall) -> Element {
    let lines = tool_input_lines(&tool_call.input);
    let result = tool_call.result.as_ref().map(|result| (result.is_error, tool_detail_text(&result.output)));
    rsx! {
        div { class: "tool-detail",
            if let Some((true, error)) = result.clone() {
                div { class: "tool-error", "{error}" }
            }
            div { class: "input-lines",
                for line in lines.iter() {
                    div { key: "{line.key}", class: "input-line",
                        span { class: "input-key", "{line.key}:" }
                        span { class: if line.is_path { "input-value input-path" } else { "input-value" }, "{line.value}" }
                    }
                }
            }
            if let Some((false, output)) = result {
                if !output.is_empty() {
                    pre { class: "tool-output", "{output}" }
                }
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
