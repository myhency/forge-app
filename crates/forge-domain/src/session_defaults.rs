use crate::{AppConfig, OutputStyle};

/// What the Code tab of Settings decides for a session at the moment its sidecar starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionDefaults {
    pub output_style: Option<OutputStyle>,
    pub prompt_suggestions: bool,
    /// Whether the SDK may enter bypass-permissions mode in this session.
    pub allow_bypass: bool,
}

impl Default for SessionDefaults {
    fn default() -> Self {
        Self { output_style: None, prompt_suggestions: true, allow_bypass: false }
    }
}

impl SessionDefaults {
    pub fn from_config(config: &AppConfig) -> Self {
        Self { output_style: config.output_style, prompt_suggestions: config.prompt_suggestions, allow_bypass: config.allow_bypass_mode }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_default_config_gives_the_default_defaults() {
        assert_eq!(SessionDefaults::from_config(&AppConfig::default()), SessionDefaults::default());
    }

    #[test]
    fn the_config_values_are_carried() {
        let config = AppConfig { output_style: Some(OutputStyle::Learning), prompt_suggestions: false, allow_bypass_mode: true, ..AppConfig::default() };
        assert_eq!(
            SessionDefaults::from_config(&config),
            SessionDefaults { output_style: Some(OutputStyle::Learning), prompt_suggestions: false, allow_bypass: true }
        );
    }
}
