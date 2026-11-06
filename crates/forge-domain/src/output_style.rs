use serde::{Deserialize, Serialize};

/// Claude Code's own output styles that the desktop app offers besides the default, and a style the
/// person wrote (a file in their `output-styles` folder, reached through Forge's style plugin). The SDK
/// reads the name as written here (`settings.outputStyle`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyle {
    Concise,
    Explanatory,
    Learning,
    Proactive,
    /// The SDK's name for it, `forge-styles:<name>` for one of the person's files.
    Custom(String),
}

/// The plugin through which the sidecar hands the person's `output-styles` folder to the SDK; the SDK
/// names those styles `forge-styles:<name>`.
pub const STYLE_PLUGIN_PREFIX: &str = "forge-styles:";

impl OutputStyle {
    /// In the order the SDK lists them and the desktop app shows them.
    pub const ALL: [OutputStyle; 4] = [OutputStyle::Proactive, OutputStyle::Concise, OutputStyle::Explanatory, OutputStyle::Learning];

    /// The name the SDK and the menu use.
    pub fn name(&self) -> &str {
        match self {
            OutputStyle::Custom(name) => name,
            OutputStyle::Concise => "Concise",
            OutputStyle::Explanatory => "Explanatory",
            OutputStyle::Learning => "Learning",
            OutputStyle::Proactive => "Proactive",
        }
    }

    /// The name as the menu shows it. The desktop app shows Claude Code's styles by their English
    /// names in every language (observed 2026-11-06), and the person's own as they named them.
    pub fn display(&self) -> String {
        match self {
            OutputStyle::Custom(name) => name.strip_prefix(STYLE_PLUGIN_PREFIX).unwrap_or(name).to_string(),
            builtin => builtin.name().to_string(),
        }
    }

    /// The name of one of Claude Code's styles, for `i18n::text` (a custom style has none).
    pub fn label(&self) -> &'static str {
        match self {
            OutputStyle::Custom(_) => "",
            OutputStyle::Concise => crate::msg!("Concise"),
            OutputStyle::Explanatory => crate::msg!("Explanatory"),
            OutputStyle::Learning => crate::msg!("Learning"),
            OutputStyle::Proactive => crate::msg!("Proactive"),
        }
    }

    /// The style the SDK calls `name`; `default` (and nothing) is the default style.
    pub fn from_name(name: &str) -> Option<OutputStyle> {
        if name.is_empty() || name == "default" {
            return None;
        }
        Some(OutputStyle::ALL.into_iter().find(|style| style.name() == name).unwrap_or_else(|| OutputStyle::Custom(name.to_string())))
    }

    /// The style a person or a tool means by `wanted` (any case; a style of theirs also by its shown
    /// name), among Claude Code's styles and the session's `available` list as the SDK reported it.
    pub fn find(wanted: &str, available: &[String]) -> Option<OutputStyle> {
        let wanted = wanted.trim();
        if let Some(style) = OutputStyle::ALL.into_iter().find(|style| style.name().eq_ignore_ascii_case(wanted)) {
            return Some(style);
        }
        available
            .iter()
            .filter(|name| !name.is_empty() && name.as_str() != "default")
            .map(|name| OutputStyle::from_name(name))
            .flatten()
            .find(|style| style.name().eq_ignore_ascii_case(wanted) || style.display().eq_ignore_ascii_case(wanted))
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, OutputStyle::Custom(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_style_is_found_again_by_its_name() {
        for style in OutputStyle::ALL {
            assert_eq!(OutputStyle::from_name(style.name()), Some(style));
        }
    }

    #[test]
    fn the_default_style_is_none_and_other_names_are_the_persons_styles() {
        assert_eq!(OutputStyle::from_name("default"), None);
        assert_eq!(OutputStyle::from_name(""), None);
        assert_eq!(OutputStyle::from_name("concise"), Some(OutputStyle::Custom("concise".into())), "names are case-sensitive, as the SDK reads them");
        let mine = OutputStyle::from_name("forge-styles:Pirate talk").unwrap();
        assert_eq!(mine.display(), "Pirate talk");
        assert_eq!(mine.name(), "forge-styles:Pirate talk");
    }

    #[test]
    fn a_style_is_found_by_any_case_and_a_persons_style_by_its_shown_name() {
        let available = vec!["default".to_string(), "Concise".to_string(), "forge-styles:Pirate talk".to_string(), "Proj style".to_string()];
        assert_eq!(OutputStyle::find("learning", &available), Some(OutputStyle::Learning));
        assert_eq!(OutputStyle::find("pirate TALK", &available), Some(OutputStyle::Custom("forge-styles:Pirate talk".into())));
        assert_eq!(OutputStyle::find("Proj style", &available), Some(OutputStyle::Custom("Proj style".into())));
        assert_eq!(OutputStyle::find("Poetic", &available), None);
    }

    #[test]
    fn a_saved_custom_style_reads_back_and_an_old_built_in_name_still_reads() {
        let json = serde_json::to_string(&OutputStyle::Custom("forge-styles:Pirate talk".into())).unwrap();
        assert_eq!(serde_json::from_str::<OutputStyle>(&json).unwrap(), OutputStyle::Custom("forge-styles:Pirate talk".into()));
        assert_eq!(serde_json::from_str::<OutputStyle>("\"concise\"").unwrap(), OutputStyle::Concise);
    }
}
