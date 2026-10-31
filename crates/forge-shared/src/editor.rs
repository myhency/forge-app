const EDITOR_JS: &str = include_str!("../assets/editor.js");

/// The HTML that goes into the window's `<head>` for the Files panel's editor: CodeMirror 6 and
/// Forge's glue (`window.ForgeEditor`), inline, so the editor needs no network (MIT licences:
/// `assets/EDITOR-LICENSE`). Built from `tools/editor-bundle`.
pub fn editor_head() -> String {
    format!("<script>{EDITOR_JS}</script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_one_inline_script() {
        let head = editor_head();
        assert!(head.starts_with("<script>"));
        assert!(head.ends_with("</script>"));
        // A `</script>` inside the vendored code would end the script early.
        assert_eq!(head.matches("<script>").count(), 1);
        assert_eq!(head.matches("</script>").count(), 1);
    }

    #[test]
    fn the_script_defines_the_global_the_bridge_uses() {
        let head = editor_head();
        assert!(head.contains("ForgeEditor"));
    }

    #[test]
    fn the_licence_file_names_the_authors() {
        assert!(include_str!("../assets/EDITOR-LICENSE").contains("Marijn Haverbeke"));
    }
}
