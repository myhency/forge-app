use dioxus::prelude::*;

/// Puts text on the clipboard (the page's `navigator.clipboard`).
pub fn copy_text(text: &str) {
    let literal = serde_json::to_string(text).unwrap_or_default();
    document::eval(&format!("navigator.clipboard.writeText({literal});"));
}
