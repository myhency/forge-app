use forge_domain::{normalize_setting, AppConfig};

/// What the Connection tab's two text fields currently hold (not yet saved).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ConnectionDraft {
    pub base_url: String,
    pub api_key: String,
}

impl ConnectionDraft {
    pub fn from_config(config: &AppConfig) -> Self {
        Self {
            base_url: config.base_url.clone().unwrap_or_default(),
            api_key: config.api_key.clone().unwrap_or_default(),
        }
    }

    /// Whether saving would change the stored config. Compared the way saving stores the text
    /// (trimmed, blank = none), so a stray trailing newline is not a difference.
    pub fn is_dirty(&self, saved: &AppConfig) -> bool {
        normalize_setting(&self.base_url) != saved.base_url || normalize_setting(&self.api_key) != saved.api_key
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved() -> AppConfig {
        AppConfig {
            base_url: Some("https://a.test".to_string()),
            api_key: Some("k".to_string()),
            ..AppConfig::default()
        }
    }

    #[test]
    fn a_draft_loaded_from_the_config_is_clean() {
        let config = saved();
        assert!(!ConnectionDraft::from_config(&config).is_dirty(&config));
        let empty = AppConfig::default();
        assert!(!ConnectionDraft::from_config(&empty).is_dirty(&empty));
    }

    #[test]
    fn editing_a_field_makes_it_dirty() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key = "other".to_string();
        assert!(draft.is_dirty(&config));
    }

    #[test]
    fn whitespace_around_the_same_value_is_not_a_change() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key = "k\n".to_string();
        draft.base_url = "  https://a.test ".to_string();
        assert!(!draft.is_dirty(&config));
    }

    #[test]
    fn clearing_a_saved_value_is_a_change_and_blank_against_none_is_not() {
        let config = saved();
        let mut draft = ConnectionDraft::from_config(&config);
        draft.api_key.clear();
        assert!(draft.is_dirty(&config));
        assert!(!ConnectionDraft { base_url: "  ".to_string(), api_key: String::new() }.is_dirty(&AppConfig::default()));
    }

    #[test]
    fn it_stays_dirty_until_the_saved_config_matches() {
        // A failed save leaves `saved` unchanged, so the Save button must stay enabled.
        let config = saved();
        let draft = ConnectionDraft { base_url: "https://b.test".to_string(), api_key: "k".to_string() };
        assert!(draft.is_dirty(&config));
        let after_success = AppConfig { base_url: Some("https://b.test".to_string()), ..config };
        assert!(!draft.is_dirty(&after_success));
    }
}
