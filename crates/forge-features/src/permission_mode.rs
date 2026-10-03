use forge_domain::PermissionMode;

const MODES: [(PermissionMode, &str, &str); 5] = [
    (PermissionMode::Default, "default", "Ask before edits"),
    (PermissionMode::AcceptEdits, "accept_edits", "Accept edits"),
    (PermissionMode::Plan, "plan", "Plan mode"),
    (PermissionMode::BypassPermissions, "bypass_permissions", "Bypass permissions"),
    (PermissionMode::Auto, "auto", "Auto"),
];

/// `(value, label)` pairs for the mode selector.
pub fn mode_options() -> Vec<(String, String)> {
    MODES
        .iter()
        .map(|(_, value, label)| (value.to_string(), label.to_string()))
        .collect()
}

pub fn mode_value(mode: PermissionMode) -> &'static str {
    MODES
        .iter()
        .find(|(candidate, _, _)| *candidate == mode)
        .map(|(_, value, _)| *value)
        .unwrap_or("default")
}

/// An unknown value falls back to `Default`, the safest mode.
pub fn parse_mode(value: &str) -> PermissionMode {
    MODES
        .iter()
        .find(|(_, candidate, _)| *candidate == value)
        .map(|(mode, _, _)| *mode)
        .unwrap_or(PermissionMode::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_value() {
        for mode in [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::Plan,
            PermissionMode::BypassPermissions,
            PermissionMode::Auto,
        ] {
            assert_eq!(parse_mode(mode_value(mode)), mode);
        }
    }

    #[test]
    fn an_unknown_value_falls_back_to_default() {
        assert_eq!(parse_mode("nonsense"), PermissionMode::Default);
        assert_eq!(parse_mode(""), PermissionMode::Default);
    }

    #[test]
    fn the_selector_offers_all_five_modes_with_distinct_values() {
        let options = mode_options();
        assert_eq!(options.len(), 5);
        let mut values: Vec<&String> = options.iter().map(|(value, _)| value).collect();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), 5);
        assert_eq!(options[0].0, "default", "the safest mode is listed first");
    }
}
