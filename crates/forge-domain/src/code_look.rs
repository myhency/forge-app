//! How code looks: the syntax colours for light and dark mode, a code font and a code size. The
//! desktop app has these on its Claude Code settings page under `코드 모양` (observed 2026-11-06:
//! a theme menu for light and one for dark — Claude Light, GitHub Light, One Light, Solarized
//! Light, Catppuccin Latte, VS Code Light and more — a font field `예: JetBrains Mono` and a size
//! menu `기본값 · 13px`, 11 to 20 px).

use serde::{Deserialize, Serialize};

/// A set of syntax colours. `Claude` keeps Forge's own colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodeTheme {
    #[default]
    Claude,
    Github,
    One,
    Solarized,
    Catppuccin,
    VsCode,
    /// Dark only.
    Dracula,
}

impl CodeTheme {
    /// The themes the light menu offers.
    pub const LIGHT: [CodeTheme; 6] = [CodeTheme::Claude, CodeTheme::Github, CodeTheme::One, CodeTheme::Solarized, CodeTheme::Catppuccin, CodeTheme::VsCode];
    /// The themes the dark menu offers.
    pub const DARK: [CodeTheme; 7] = [CodeTheme::Claude, CodeTheme::Github, CodeTheme::One, CodeTheme::Solarized, CodeTheme::Catppuccin, CodeTheme::VsCode, CodeTheme::Dracula];

    pub fn value(self) -> &'static str {
        match self {
            CodeTheme::Claude => "claude",
            CodeTheme::Github => "github",
            CodeTheme::One => "one",
            CodeTheme::Solarized => "solarized",
            CodeTheme::Catppuccin => "catppuccin",
            CodeTheme::VsCode => "vs-code",
            CodeTheme::Dracula => "dracula",
        }
    }

    pub fn from_value(value: &str) -> Option<CodeTheme> {
        CodeTheme::DARK.into_iter().find(|theme| theme.value() == value)
    }

    /// The name in the menu: the same words in every language, as in the desktop app.
    pub fn label(self, dark: bool) -> &'static str {
        match (self, dark) {
            (CodeTheme::Claude, false) => "Claude Light",
            (CodeTheme::Claude, true) => "Claude Dark",
            (CodeTheme::Github, false) => "GitHub Light",
            (CodeTheme::Github, true) => "GitHub Dark",
            (CodeTheme::One, false) => "One Light",
            (CodeTheme::One, true) => "One Dark",
            (CodeTheme::Solarized, false) => "Solarized Light",
            (CodeTheme::Solarized, true) => "Solarized Dark",
            (CodeTheme::Catppuccin, false) => "Catppuccin Latte",
            (CodeTheme::Catppuccin, true) => "Catppuccin Mocha",
            (CodeTheme::VsCode, false) => "VS Code Light",
            (CodeTheme::VsCode, true) => "VS Code Dark",
            (CodeTheme::Dracula, _) => "Dracula",
        }
    }

    /// The six syntax colours (comment, keyword, string, number, function, type), or `None` to keep
    /// Forge's own. The values are each theme's published colours.
    fn palette(self, dark: bool) -> Option<[&'static str; 6]> {
        Some(match (self, dark) {
            (CodeTheme::Claude, _) => return None,
            (CodeTheme::Github, false) => ["#6e7781", "#cf222e", "#0a3069", "#0550ae", "#8250df", "#953800"],
            (CodeTheme::Github, true) => ["#8b949e", "#ff7b72", "#a5d6ff", "#79c0ff", "#d2a8ff", "#ffa657"],
            (CodeTheme::One, false) => ["#a0a1a7", "#a626a4", "#50a14f", "#986801", "#4078f2", "#c18401"],
            (CodeTheme::One, true) => ["#7f848e", "#c678dd", "#98c379", "#d19a66", "#61afef", "#e5c07b"],
            (CodeTheme::Solarized, false) => ["#93a1a1", "#859900", "#2aa198", "#d33682", "#268bd2", "#b58900"],
            (CodeTheme::Solarized, true) => ["#586e75", "#859900", "#2aa198", "#d33682", "#268bd2", "#b58900"],
            (CodeTheme::Catppuccin, false) => ["#9ca0b0", "#8839ef", "#40a02b", "#fe640b", "#1e66f5", "#df8e1d"],
            (CodeTheme::Catppuccin, true) => ["#6c7086", "#cba6f7", "#a6e3a1", "#fab387", "#89b4fa", "#f9e2af"],
            (CodeTheme::VsCode, false) => ["#008000", "#0000ff", "#a31515", "#098658", "#795e26", "#267f99"],
            (CodeTheme::VsCode, true) => ["#6a9955", "#569cd6", "#ce9178", "#b5cea8", "#dcdcaa", "#4ec9b0"],
            (CodeTheme::Dracula, false) => return None,
            (CodeTheme::Dracula, true) => ["#6272a4", "#ff79c6", "#f1fa8c", "#bd93f9", "#50fa7b", "#8be9fd"],
        })
    }
}

/// Forge's own syntax colours (theme.css `--hl-*`), for the Settings preview of `Claude Light` and
/// `Claude Dark`; keep them in step with the stylesheet.
const CLAUDE_LIGHT: [&str; 6] = ["oklch(0.500 0.015 71.2)", "oklch(0.518 0.226 323.9)", "oklch(0.510 0.137 150.1)", "oklch(0.535 0.146 49.0)", "oklch(0.488 0.217 264.4)", "oklch(0.511 0.086 186.4)"];
const CLAUDE_DARK: [&str; 6] = ["oklch(0.700 0.015 71.2)", "oklch(0.748 0.207 322.2)", "oklch(0.871 0.136 154.4)", "oklch(0.837 0.117 66.3)", "oklch(0.809 0.096 251.8)", "oklch(0.855 0.125 181.1)"];

/// The six colours (comment, keyword, string, number, function, type) a theme draws code with in
/// light or dark mode: the Settings preview shows them side by side, as the desktop app does.
pub fn preview_colors(theme: CodeTheme, dark: bool) -> [&'static str; 6] {
    theme.palette(dark).unwrap_or(if dark { CLAUDE_DARK } else { CLAUDE_LIGHT })
}

/// The code sizes the size menu offers, in px; `None` is the default (13 px).
pub const CODE_SIZES: [u8; 8] = [11, 12, 13, 14, 15, 16, 18, 20];
pub const DEFAULT_CODE_SIZE: u8 = 13;

/// The person's choices for code. Every field falls back to its default when missing or unknown.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CodeLook {
    #[serde(default, deserialize_with = "crate::config::lenient_value")]
    pub light: CodeTheme,
    #[serde(default, deserialize_with = "crate::config::lenient_value")]
    pub dark: CodeTheme,
    /// A monospace font by name; `None` keeps the system's.
    #[serde(default)]
    pub font: Option<String>,
    #[serde(default)]
    pub size: Option<u8>,
}

/// A font name as typed, cut down to what a font family name holds, so it cannot break out of
/// the stylesheet.
pub fn clean_font_name(name: &str) -> Option<String> {
    let kept: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).collect();
    let kept = kept.trim().to_string();
    (!kept.is_empty()).then_some(kept)
}

const TOKENS: [&str; 6] = ["comment", "keyword", "string", "number", "function", "type"];

fn declarations(palette: [&str; 6]) -> String {
    TOKENS.iter().zip(palette).map(|(token, colour)| format!("--hl-{token}: {colour};")).collect::<Vec<_>>().join(" ")
}

/// The stylesheet for the choices: the light palette, the dark one (for a dark theme set by hand
/// and for the system's dark mode), the font and the size. Empty when everything is the default.
/// `:root:root` outranks the theme's own colour rules.
pub fn code_look_css(look: &CodeLook) -> String {
    let mut css = String::new();
    if let Some(palette) = look.light.palette(false) {
        css.push_str(&format!(":root:root {{ {} }}\n", declarations(palette)));
    }
    if let Some(palette) = look.dark.palette(true) {
        let rules = declarations(palette);
        css.push_str(&format!(":root:root[data-theme=\"dark\"] {{ {rules} }}\n"));
        css.push_str(&format!("@media (prefers-color-scheme: dark) {{ :root:root:not([data-theme=\"light\"]) {{ {rules} }} }}\n"));
    }
    if let Some(font) = look.font.as_deref().and_then(clean_font_name) {
        css.push_str(&format!(":root:root {{ --mono: \"{font}\", ui-monospace, \"SF Mono\", Menlo, monospace; }}\n"));
    }
    if let Some(size) = look.size.filter(|size| CODE_SIZES.contains(size)) {
        // Code blocks, diffs, the editor and command output; inline code keeps the text's size.
        css.push_str(&format!("pre, pre code, .diff, .ndiff, .perm-command, .cm-editor, .cm-content {{ font-size: {size}px !important; }}\n"));
    }
    css
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_look_adds_nothing() {
        assert_eq!(code_look_css(&CodeLook::default()), "");
    }

    #[test]
    fn a_light_and_a_dark_theme_set_the_six_colours_for_each_mode() {
        let css = code_look_css(&CodeLook { light: CodeTheme::Github, dark: CodeTheme::Dracula, ..CodeLook::default() });
        assert!(css.contains(":root:root { --hl-comment: #6e7781;"), "{css}");
        assert!(css.contains(":root:root[data-theme=\"dark\"] { --hl-comment: #6272a4;"), "{css}");
        assert!(css.contains("prefers-color-scheme: dark"), "{css}");
        assert_eq!(css.matches("--hl-type").count(), 3);
    }

    #[test]
    fn a_font_name_is_cleaned_and_quoted() {
        assert_eq!(clean_font_name(" JetBrains Mono "), Some("JetBrains Mono".to_string()));
        assert_eq!(clean_font_name("x\"; } body { color: red"), Some("x  body  color red".to_string()));
        assert_eq!(clean_font_name("  "), None);
        let css = code_look_css(&CodeLook { font: Some("Fira Code".into()), ..CodeLook::default() });
        assert!(css.contains("--mono: \"Fira Code\", ui-monospace"), "{css}");
    }

    #[test]
    fn only_a_listed_size_applies() {
        assert!(code_look_css(&CodeLook { size: Some(16), ..CodeLook::default() }).contains("font-size: 16px"));
        assert_eq!(code_look_css(&CodeLook { size: Some(99), ..CodeLook::default() }), "");
    }

    #[test]
    fn an_unknown_theme_in_the_file_falls_back_to_claude() {
        let look: CodeLook = serde_json::from_str(r#"{"light":"nope","dark":"one","size":14}"#).unwrap();
        assert_eq!(look.light, CodeTheme::Claude);
        assert_eq!(look.dark, CodeTheme::One);
        assert_eq!(look.size, Some(14));
    }

    #[test]
    fn the_preview_falls_back_to_forges_own_colours() {
        assert_eq!(preview_colors(CodeTheme::Claude, false), CLAUDE_LIGHT);
        assert_eq!(preview_colors(CodeTheme::Github, true)[1], "#ff7b72");
        assert_eq!(preview_colors(CodeTheme::Dracula, false), CLAUDE_LIGHT, "Dracula has no light colours");
    }

    #[test]
    fn every_theme_round_trips_through_its_value() {
        for theme in CodeTheme::DARK {
            assert_eq!(CodeTheme::from_value(theme.value()), Some(theme));
        }
    }
}
