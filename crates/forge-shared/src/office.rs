const JSZIP_JS: &str = include_str!("../assets/jszip.min.js");
const DOCX_PREVIEW_JS: &str = include_str!("../assets/docx-preview.min.js");
const PPTX_PREVIEW_JS: &str = include_str!("../assets/pptx-preview.umd.js");
const BRIDGE_JS: &str = include_str!("../assets/office-bridge.js");

/// The HTML that goes into the window's `<head>`: the library that draws a docx as pages and
/// `window.forgeOffice` that uses it. Inline, so no network is needed (licences:
/// `assets/OFFICE-LICENSES`). The pptx library is 1.4 MB: in the head it kept the window from opening,
/// so `pptx_library` runs it the first time a deck is drawn.
pub fn office_head() -> String {
    format!("<script>{JSZIP_JS}</script><script>{DOCX_PREVIEW_JS}</script><script>{BRIDGE_JS}</script>")
}

/// A script that loads the pptx library once (a no-op after the first time).
pub fn pptx_library() -> String {
    format!("if (!window.pptxPreview) {{ {PPTX_PREVIEW_JS}\n}}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_three_inline_scripts_and_leaves_the_big_library_out() {
        let head = office_head();
        assert_eq!(head.matches("<script>").count(), 3);
        // A `</script>` inside the vendored code would end a script early.
        assert_eq!(head.matches("</script>").count(), 3);
        assert!(head.len() < 400_000, "a big head keeps the window from opening");
        assert!(!head.contains("echarts"), "the big library is not in the head");
    }

    #[test]
    fn the_scripts_define_the_globals_the_bridge_uses() {
        let head = office_head();
        assert!(head.contains("renderAsync"));
        assert!(head.contains("window.forgeOffice"));
        assert!(pptx_library().contains("pptxPreview"));
    }

    #[test]
    fn the_bridge_removes_links_and_active_elements_from_what_it_draws() {
        assert!(BRIDGE_JS.contains("removeAttribute(\"href\")"));
        assert!(BRIDGE_JS.contains("script, iframe, object, embed, link"));
    }
}
