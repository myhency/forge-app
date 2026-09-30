/// The app's stylesheet: a flat single-tone look after Claude Code desktop (spec §16.6).
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
            ".sidebar-group-header",
            ".group-add",
            ".sidebar-footer",
            ".main-pane",
            ".messages",
            ".message",
            ".tool-call",
            ".permission-card",
            ".composer",
            ".status-dot",
            ".banner",
            ".settings-backdrop",
            ".settings-panel",
            ".settings-nav-item",
            ".setting-row",
            ".text-input",
        ] {
            assert!(THEME_CSS.contains(class), "theme.css is missing {class}");
        }
    }

    #[test]
    fn the_stylesheet_has_a_dark_mode() {
        assert!(THEME_CSS.contains("prefers-color-scheme: dark"));
    }

    #[test]
    fn the_running_marker_animates_but_not_for_people_who_asked_for_less_motion() {
        assert!(THEME_CSS.contains("@keyframes"));
        assert!(THEME_CSS.contains("prefers-reduced-motion"));
    }
}
