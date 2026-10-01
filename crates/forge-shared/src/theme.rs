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

    #[test]
    fn the_stylesheet_styles_markdown_highlighting_and_diffs() {
        for selector in [
            ".markdown",
            ".code-block",
            ".hl-comment",
            ".hl-string",
            ".diff",
            ".diff-line.added",
            ".diff-line.removed",
            ".tool-call.expandable",
        ] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
    }

    #[test]
    fn the_code_and_diff_colors_have_a_dark_palette() {
        // The body of the last dark media block: from its opening to the closing "}\n}".
        let start = THEME_CSS.rfind("@media (prefers-color-scheme: dark)").unwrap();
        let block = &THEME_CSS[start..];
        let dark = &block[..block.find("\n}").unwrap()];
        for definition in ["--hl-keyword:", "--hl-string:", "--diff-added-bg:", "--diff-removed-bg:"] {
            assert!(dark.contains(definition), "the dark palette does not define {definition}");
        }
    }

    #[test]
    fn a_diff_inside_a_permission_card_scrolls_instead_of_pushing_the_buttons_away() {
        let rule = THEME_CSS.lines().find(|line| line.starts_with(".permission-card .diff {")).unwrap();
        assert!(rule.contains("max-height"), "{rule}");
        assert!(rule.contains("overflow-y: auto"), "{rule}");
    }
}
