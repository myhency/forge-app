const IME_GUARD_JS: &str = include_str!("../assets/ime-guard.js");

/// The HTML that goes into the window's `<head>`: a document-level capture listener that keeps the
/// Enter confirming an input-method composition away from every handler (see the asset's comment).
pub fn ime_guard_head() -> String {
    format!("<script>{IME_GUARD_JS}</script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_is_one_inline_script() {
        let head = ime_guard_head();
        assert!(head.starts_with("<script>"));
        assert!(head.ends_with("</script>"));
        assert_eq!(head.matches("<script>").count(), 1);
        assert!(!IME_GUARD_JS.contains("</script"), "would end the inline script early");
    }
}
