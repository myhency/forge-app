const XTERM_CSS: &str = include_str!("../assets/xterm.css");
const XTERM_JS: &str = include_str!("../assets/xterm.js");
const XTERM_FIT_JS: &str = include_str!("../assets/addon-fit.js");

/// The HTML that goes into the window's `<head>`: xterm.js, its fit addon and its stylesheet,
/// inline, so the terminal needs no network (MIT licence: `assets/XTERM-LICENSE`).
pub fn xterm_head() -> String {
    format!("<style>{XTERM_CSS}</style><script>{XTERM_JS}</script><script>{XTERM_FIT_JS}</script>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_carries_the_stylesheet_and_exactly_two_scripts() {
        let head = xterm_head();
        assert!(head.starts_with("<style>"));
        assert_eq!(head.matches("<script>").count(), 2, "xterm and its fit addon");
        // A `</script>` inside the vendored code would end a script early.
        assert_eq!(head.matches("</script>").count(), 2);
    }

    #[test]
    fn the_scripts_define_the_two_globals_the_bridge_uses() {
        let head = xterm_head();
        assert!(head.contains("Terminal"));
        assert!(head.contains("FitAddon"));
    }
}
