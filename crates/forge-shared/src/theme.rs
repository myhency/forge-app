/// The app's stylesheet. Neutral on purpose: visual parity with Claude Code desktop is out of
/// scope for the walking skeleton (spec §15.1).
pub const THEME_CSS: &str = include_str!("theme.css");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stylesheet_defines_the_layout_classes_the_components_use() {
        for class in [
            ".workspace",
            ".sidebar",
            ".sidebar-item",
            ".main-pane",
            ".messages",
            ".message",
            ".tool-call",
            ".permission-card",
            ".composer",
            ".status-dot",
            ".banner",
        ] {
            assert!(THEME_CSS.contains(class), "theme.css is missing {class}");
        }
    }

    #[test]
    fn the_stylesheet_has_a_dark_mode() {
        assert!(THEME_CSS.contains("prefers-color-scheme: dark"));
    }
}
