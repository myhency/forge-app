/// The app's stylesheet: a flat single-tone look after Claude Code desktop (spec §16.6).
pub const THEME_CSS: &str = include_str!("theme.css");

#[cfg(test)]
mod tests {
    use super::*;

    const VARIABLE_BLOCK: &str = "@media (prefers-color-scheme: dark) {\n  :root:not([data-theme=\"light\"]) {";

    #[test]
    fn every_dark_block_has_a_forced_dark_twin_and_light_stays_light_under_a_dark_system() {
        // The variable blocks; the `.office-page [data-lum` overrides are dark media blocks of another kind.
        let media = THEME_CSS.matches(VARIABLE_BLOCK).count();
        assert_eq!(THEME_CSS.matches("@media (prefers-color-scheme: dark)").count(), media + 1, "one override block besides the variable blocks");
        assert!(media >= 3);
        assert_eq!(THEME_CSS.matches(":root[data-theme=\"dark\"] {").count(), media, "one forced-dark rule per media block");
        assert_eq!(THEME_CSS.matches(":root:not([data-theme=\"light\"]) {").count(), media, "the media version must not apply when the person chose light");
    }

    // The declarations of every variable block of one kind, in file order: the `@media` ones are indented one level deeper.
    fn dark_blocks(forced: bool) -> Vec<Vec<String>> {
        let (open, close) = if forced { (":root[data-theme=\"dark\"] {\n", "\n}") } else { ("@media (prefers-color-scheme: dark) {\n  :root:not([data-theme=\"light\"]) {\n", "\n  }\n}") };
        THEME_CSS
            .match_indices(open)
            .map(|(at, _)| {
                let body = &THEME_CSS[at + open.len()..];
                let body = &body[..body.find(close).unwrap()];
                body.lines().map(|line| line.trim().to_string()).collect()
            })
            .collect()
    }

    #[test]
    fn the_system_dark_block_and_the_forced_dark_block_hold_the_same_declarations() {
        let system = dark_blocks(false);
        let forced = dark_blocks(true);
        assert_eq!(system.len(), forced.len());
        assert!(system.len() >= 3);
        for (system, forced) in system.iter().zip(&forced) {
            assert_eq!(system, forced, "the two dark palettes have drifted apart");
        }
    }

    #[test]
    fn the_scale_tokens_are_defined_on_the_root() {
        let root = &THEME_CSS[..THEME_CSS.find("\n}").unwrap()];
        for token in [
            "--sp-4:", "--sp-8:", "--sp-16:", "--text-sm:", "--text-base:", "--text-md:", "--leading-normal:", "--weight-semibold:", "--r-6:", "--r-pill:", "--dur-fast:", "--dur-base:", "--ease-out:", "--surface-0:", "--surface-1:",
            "--surface-2:", "--surface-3:", "--shadow-1:", "--shadow-2:", "--shadow-3:",
        ] {
            assert!(root.contains(token), "the root block does not define {token}");
        }
        for old_name in ["--bg:", "--bg-sidebar:", "--bg-card:", "--radius:", "--radius-lg:", "--shadow:"] {
            assert!(root.contains(&format!("{old_name} var(--")), "{old_name} must stay as an alias of a scale token");
        }
    }

    // The palette as the browser sees it: every `--name: value;` of the three light blocks, and the forced-dark twins laid over them.
    mod palette {
        use super::{dark_blocks, THEME_CSS};
        use std::collections::HashMap;

        pub type Rgba = (f64, f64, f64, f64);

        fn declarations(lines: impl Iterator<Item = String>, into: &mut HashMap<String, String>) {
            for line in lines {
                for declaration in line.split(';') {
                    if let Some((name, value)) = declaration.trim().split_once(':') {
                        if name.starts_with("--") {
                            into.insert(name.trim().to_string(), value.trim().to_string());
                        }
                    }
                }
            }
        }

        pub fn theme(dark: bool) -> HashMap<String, String> {
            let mut map = HashMap::new();
            for (at, _) in THEME_CSS.match_indices(":root {\n") {
                let body = &THEME_CSS[at + 8..];
                declarations(body[..body.find("\n}").unwrap()].lines().map(str::to_string), &mut map);
            }
            if dark {
                for block in dark_blocks(true) {
                    declarations(block.into_iter(), &mut map);
                }
            }
            map
        }

        fn resolve(map: &HashMap<String, String>, name: &str) -> String {
            let mut value = map.get(&format!("--{name}")).unwrap_or_else(|| panic!("no --{name}")).clone();
            while let Some(inner) = value.strip_prefix("var(--").and_then(|rest| rest.strip_suffix(')')) {
                value = map.get(&format!("--{inner}")).unwrap_or_else(|| panic!("no --{inner}")).clone();
            }
            value
        }

        fn linear_to_srgb(c: f64) -> f64 {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.0031308 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 }
        }

        pub fn oklch_to_srgb(l: f64, c: f64, h: f64) -> (f64, f64, f64) {
            let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
            let (l_, m_, s_) = ((l + 0.3963377774 * a + 0.2158037573 * b).powi(3), (l - 0.1055613458 * a - 0.0638541728 * b).powi(3), (l - 0.0894841775 * a - 1.2914855480 * b).powi(3));
            let r = 4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_;
            let g = -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_;
            let bl = -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_;
            (linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(bl))
        }

        fn parse(value: &str) -> Rgba {
            let number = |text: &str| text.trim().parse::<f64>().unwrap_or_else(|_| panic!("not a number: {text} in {value}"));
            if let Some(hex) = value.strip_prefix('#') {
                let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).unwrap() as f64 / 255.0;
                return (byte(0), byte(2), byte(4), 1.0);
            }
            if let Some(inner) = value.strip_prefix("oklch(").and_then(|rest| rest.strip_suffix(')')) {
                let (color, alpha) = inner.split_once('/').map_or((inner, 1.0), |(color, alpha)| (color, number(alpha)));
                let parts: Vec<f64> = color.split_whitespace().map(number).collect();
                let (r, g, b) = oklch_to_srgb(parts[0], parts[1], parts[2]);
                return (r, g, b, alpha);
            }
            if let Some(inner) = value.strip_prefix("rgba(").and_then(|rest| rest.strip_suffix(')')) {
                let parts: Vec<f64> = inner.split(',').map(number).collect();
                return (parts[0] / 255.0, parts[1] / 255.0, parts[2] / 255.0, parts[3]);
            }
            panic!("a color this test cannot read: {value}");
        }

        pub fn color(map: &HashMap<String, String>, name: &str) -> Rgba {
            parse(&resolve(map, name))
        }

        pub fn over(top: Rgba, bottom: Rgba) -> Rgba {
            let mix = |t: f64, b: f64| t * top.3 + b * (1.0 - top.3);
            (mix(top.0, bottom.0), mix(top.1, bottom.1), mix(top.2, bottom.2), 1.0)
        }

        fn luminance(color: Rgba) -> f64 {
            let linear = |c: f64| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
            0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
        }

        pub fn contrast(a: Rgba, b: Rgba) -> f64 {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        }
    }

    #[test]
    fn the_color_math_matches_known_values() {
        let (r, g, b) = palette::oklch_to_srgb(1.0, 0.0, 0.0);
        assert!(r > 0.999 && g > 0.999 && b > 0.999, "white came out as {r} {g} {b}");
        let (r, g, b) = palette::oklch_to_srgb(0.0, 0.0, 0.0);
        assert!(r < 0.001 && g < 0.001 && b < 0.001);
        // sRGB red is oklch(0.628 0.2577 29.23).
        let (r, g, b) = palette::oklch_to_srgb(0.628, 0.2577, 29.23);
        assert!(r > 0.99 && g < 0.03 && b < 0.03, "red came out as {r} {g} {b}");
        let black = (0.0, 0.0, 0.0, 1.0);
        let white = (1.0, 1.0, 1.0, 1.0);
        assert!((palette::contrast(black, white) - 21.0).abs() < 0.01);
    }

    #[test]
    fn every_text_color_reads_on_every_surface_it_sits_on_in_both_themes() {
        const SURFACES: [&str; 5] = ["surface-0", "surface-1", "surface-2", "surface-3", "bg-hover"];
        let mut failures = Vec::new();
        for dark in [false, true] {
            let map = palette::theme(dark);
            let mut check = |text: &str, surface: &str, behind: Option<&str>| {
                let mut under = palette::color(&map, surface);
                if let Some(behind) = behind {
                    under = palette::over(under, palette::color(&map, behind));
                }
                let ratio = palette::contrast(palette::over(palette::color(&map, text), under), under);
                if ratio < 4.5 {
                    failures.push(format!("{} --{text} on --{surface}: {ratio:.2}", if dark { "dark" } else { "light" }));
                }
            };
            for text in ["fg", "fg-muted", "accent", "danger"] {
                for surface in SURFACES {
                    check(text, surface, None);
                }
            }
            for text in ["link", "hl-comment", "hl-keyword", "hl-string", "hl-number", "hl-function", "hl-type"] {
                check(text, "code-bg", None);
                check(text, "surface-2", None);
            }
            for background in ["diff-added-bg", "diff-removed-bg", "diff-word-bg"] {
                check("fg", background, Some("surface-2"));
            }
            check("accent-fg", "accent", None);
            check("warning-fg", "warning", None);
        }
        assert!(failures.is_empty(), "below 4.5:1:\n{}", failures.join("\n"));
    }

    #[test]
    fn the_focus_ring_stands_out_from_every_surface_in_both_themes() {
        // The ring sits `--focus-ring-offset` away from its control, so it is judged against the surfaces around it, not the control.
        for dark in [false, true] {
            let map = palette::theme(dark);
            assert_eq!(map["--focus-ring-width"], "2px");
            for surface in ["surface-0", "surface-1", "surface-2", "surface-3"] {
                let under = palette::color(&map, surface);
                let ratio = palette::contrast(palette::color(&map, "focus-ring-color"), under);
                assert!(ratio >= 3.0, "{} focus ring on --{surface}: {ratio:.2}", if dark { "dark" } else { "light" });
            }
        }
    }

    #[test]
    fn the_stylesheet_has_a_rule_for_every_attribute_value() {
        for rule in [
            "[data-text-size=\"small\"]",
            "[data-text-size=\"large\"]",
            "[data-width=\"narrow\"]",
            "[data-width=\"wide\"]",
            "[data-font=\"opendyslexic\"]",
            "[data-motion=\"reduced\"]",
        ] {
            assert!(THEME_CSS.contains(rule), "missing {rule}");
        }
    }

    // The one line of the stylesheet that starts with this selector.
    fn rule(selector: &str) -> &'static str {
        let css: &'static str = THEME_CSS;
        css.lines().find(|line| line.starts_with(&format!("{selector} {{"))).unwrap_or_else(|| panic!("no rule for {selector}"))
    }

    fn z_index(selector: &str) -> u32 {
        let line = rule(selector);
        let rest = line.split("z-index: ").nth(1).unwrap_or_else(|| panic!("no z-index in {line}"));
        rest.split(';').next().unwrap().trim().parse().unwrap()
    }

    #[test]
    fn the_floating_dock_sits_under_settings_and_the_search_palette() {
        let dock = z_index(".dock-float .dock");
        assert!(dock < z_index(".settings-backdrop"), "the dock card must not cover Settings");
        assert!(dock < z_index(".palette-backdrop"), "the dock card must not cover the search palette");
    }

    #[test]
    fn the_lightbox_sits_above_settings_and_the_search_palette() {
        let lightbox = z_index(".lightbox-backdrop");
        assert!(lightbox > z_index(".settings-backdrop"));
        assert!(lightbox > z_index(".palette-backdrop"));
    }

    #[test]
    fn the_floating_dock_fills_the_pane_to_its_top_and_the_rail_has_its_own_close_button() {
        // The chat header's tool buttons sit under it: every panel closes itself, the rail through `.rail-close`.
        assert!(rule(".dock-float .dock").contains("top: 8px"));
        assert!(THEME_CSS.contains(".dock-float .rail-close { display: inline-flex; }"));
        assert!(rule(".rail-close").contains("display: none"));
    }

    #[test]
    fn the_overlay_sidebar_hides_the_hide_button_that_would_save_a_hidden_sidebar() {
        assert!(THEME_CSS.contains(".sidebar-overlay .sidebar-hide { display: none; }"));
    }

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
            ".account",
            ".account-button",
            ".account-menu",
            ".picker-check",
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
            ".stop-button",
            ".file-markdown",
            ".files-source-toggle",
            ".files-main",
            ".files-tabs",
            ".files-tab",
            ".files-tab-close",
            ".files-tab-menu",
            ".files-banner",
            ".files-editor",
            ".files-kind",
            ".files-kind-button",
            ".files-pdf",
            ".files-html",
            ".files-csv",
            ".files-csv-note",
            ".files-svg",
            ".files-svg-bar",
            ".files-icon",
            ".lightbox-backdrop",
            ".lightbox-image",
            ".lightbox-close",
            ".lightbox-caption",
            ".files-menu-sub",
            ".files-settings-menu",
            ".file-link",
            ".file-name-link",
            ".task-card",
            ".task-output",
            ".task-stop",
            ".scroll-end",
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
        let start = THEME_CSS.rfind(VARIABLE_BLOCK).unwrap();
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

    #[test]
    fn the_customize_menu_sits_above_the_menu_backdrop_that_closes_it() {
        assert!(z_index(".customize-menu") > z_index(".menu-backdrop"));
    }

    #[test]
    fn the_explore_cards_fall_to_one_column_when_the_chat_is_narrow() {
        assert!(rule(".customize-grid").contains("1fr 1fr"));
        assert!(rule(".chat-narrow .customize-grid").contains("grid-template-columns: 1fr;"));
    }
}
