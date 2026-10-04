use dioxus::prelude::*;
use forge_domain::ToolCall;

use crate::tool_summary::tool_detail_text;

/// The text of a chip on a sent message: an icon and the file name.
pub fn attachment_label(attachment: &forge_domain::Attachment) -> String {
    let icon = match attachment.kind {
        forge_domain::AttachmentKind::Image => "🖼",
        forge_domain::AttachmentKind::File => "📄",
    };
    format!("{icon} {}", attachment.file_name())
}

/// The input of a tool call and, once it has one, its result.
#[component]
pub(crate) fn ToolDetail(tool_call: ToolCall) -> Element {
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
