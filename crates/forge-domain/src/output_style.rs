use serde::{Deserialize, Serialize};

/// Claude Code's own output styles that the desktop app offers besides the default. The SDK reads
/// the name as written here (`settings.outputStyle`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputStyle {
    Concise,
    Explanatory,
    Learning,
    Proactive,
}

impl OutputStyle {
    pub const ALL: [OutputStyle; 4] = [OutputStyle::Concise, OutputStyle::Explanatory, OutputStyle::Learning, OutputStyle::Proactive];

    /// The name the SDK and the menu use.
    pub fn name(self) -> &'static str {
        match self {
            OutputStyle::Concise => "Concise",
            OutputStyle::Explanatory => "Explanatory",
            OutputStyle::Learning => "Learning",
            OutputStyle::Proactive => "Proactive",
        }
    }

    pub fn from_name(name: &str) -> Option<OutputStyle> {
        OutputStyle::ALL.into_iter().find(|style| style.name() == name)
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
    fn the_default_style_and_unknown_names_are_not_styles() {
        assert_eq!(OutputStyle::from_name("default"), None);
        assert_eq!(OutputStyle::from_name(""), None);
        assert_eq!(OutputStyle::from_name("concise"), None, "names are case-sensitive, as the SDK reads them");
    }
}
