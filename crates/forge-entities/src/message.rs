use dioxus::prelude::*;
use forge_domain::ToolCall;

use crate::tool_summary::{tool_detail_text, tool_input_lines};

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
