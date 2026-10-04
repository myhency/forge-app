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
            ".row-menu-button",
            ".row-menu",
            ".rename-input",
            ".confirm-row",
            ".sidebar-footer",
            ".main-pane",
            ".messages",
            ".turns",
            ".live-line",
            ".live-dots",
            ".live-clock",
            ".needs-input",
            ".answer-card",
            ".input-lines",
            ".input-path",
            ".tool-error",
            ".permission-panel",
            ".perm-title",
            ".perm-buttons",
            ".perm-key",
            ".perm-menu",
            ".question-panel",
            ".question-chip",
            ".question-option",
            ".question-footer",
            ".turn-user",
            ".user-bubble",
            ".activity-head",
            ".activity-box",
            ".activity-row-head",
            ".count-added",
            ".count-removed",
            ".file-card",
            ".turn-footer",
            ".permission-card",
            ".composer",
            ".status-dot",
            ".banner",
            ".settings-backdrop",
            ".usage-card",
            ".heatmap",
            ".legend",
            ".settings-panel",
            ".settings-nav-item",
            ".setting-row",
            ".text-input",
            ".sidebar-top",
            ".sidebar-search",
            ".icon-button",
            ".filter-wrap",
            ".filter-menu",
            ".filter-submenu",
            ".menu-backdrop",
            ".group-toggle",
            ".sidebar-resize",
            ".sidebar-rail",
            ".palette-backdrop",
            ".palette",
            ".palette-input",
            ".palette-row",
            ".palette-snippet",
            ".palette-empty",
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
            ".thinking",
            ".tool-detail",
            ".view-switch",
            ".picker-menu",
            ".picker-item",
            ".picker-confirm",
            ".view-option.selected",
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
        for definition in ["--hl-keyword:", "--hl-string:", "--diff-added-bg:", "--diff-removed-bg:", "--link:"] {
            assert!(dark.contains(definition), "the dark palette does not define {definition}");
        }
    }

    #[test]
    fn a_diff_inside_a_permission_card_scrolls_instead_of_pushing_the_buttons_away() {
        let rule = THEME_CSS.lines().find(|line| line.starts_with(".permission-card .diff {")).unwrap();
        assert!(rule.contains("max-height"), "{rule}");
        assert!(rule.contains("overflow-y: auto"), "{rule}");
    }

    #[test]
    fn the_effort_styles_exist_and_use_theme_colors() {
        for selector in [".effort-menu", ".effort-head", ".effort-ends", ".effort-slider", ".effort-help"] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
        for rule in THEME_CSS.lines().filter(|line| line.starts_with(".effort")) {
            assert!(!rule.contains('#'), "a hard-coded color in: {rule}");
            assert!(!rule.contains("rgb"), "a hard-coded color in: {rule}");
        }
    }

    #[test]
    fn the_usage_ring_styles_exist() {
        for selector in [".usage-ring", ".usage-ring.warn", ".usage-ring.danger", ".usage-popover", ".usage-backdrop"] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
    }

    #[test]
    fn the_usage_styles_take_every_color_from_the_theme_variables() {
        let rules: Vec<&str> = THEME_CSS.lines().filter(|line| line.starts_with(".usage")).collect();
        assert!(rules.len() >= 5, "found only {} usage rules", rules.len());
        for rule in rules {
            assert!(!rule.contains('#'), "a hard-coded color in: {rule}");
            assert!(!rule.contains("rgb"), "a hard-coded color in: {rule}");
        }
    }
}
