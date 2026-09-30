/// The value of the "Default" option in a model list (an empty model is stored as `None`).
pub const DEFAULT_MODEL_VALUE: &str = "__default";

/// The models offered in Settings: `(id, label)`. Nothing lists models at runtime, so this is the
/// one place to edit when models change.
pub const KNOWN_MODELS: [(&str, &str); 4] = [
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-haiku-4-5-20251001", "Haiku 4.5"),
];

const DEFAULT_LABEL: &str = "Default (chosen by the SDK)";

fn is_known(id: &str) -> bool {
    KNOWN_MODELS.iter().any(|(known, _)| *known == id)
}

/// `(value, label)` for a model dropdown: the default, the known models, and — when the saved
/// model is something else (typed by hand) — that model, so it is shown rather than hidden.
pub fn model_options(saved: Option<&str>) -> Vec<(String, String)> {
    let mut options = vec![(DEFAULT_MODEL_VALUE.to_string(), DEFAULT_LABEL.to_string())];
    options.extend(KNOWN_MODELS.iter().map(|(id, label)| (id.to_string(), label.to_string())));
    if let Some(id) = saved.filter(|id| !is_known(id)) {
        options.push((id.to_string(), format!("Custom: {id}")));
    }
    options
}

/// The option value that represents the saved model.
pub fn model_value(saved: Option<&str>) -> String {
    saved.map(str::to_string).unwrap_or_else(|| DEFAULT_MODEL_VALUE.to_string())
}

/// A short name for the saved model, for read-only display.
pub fn model_label(saved: Option<&str>) -> String {
    match saved {
        None => "Default model".to_string(),
        Some(id) => KNOWN_MODELS
            .iter()
            .find(|(known, _)| *known == id)
            .map(|(_, label)| label.to_string())
            .unwrap_or_else(|| id.to_string()),
    }
}

/// What to save for a chosen option value: empty for the default.
pub fn model_from_value(value: &str) -> String {
    if value == DEFAULT_MODEL_VALUE {
        String::new()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_comes_first_then_the_known_models() {
        let options = model_options(None);
        assert_eq!(options[0].0, DEFAULT_MODEL_VALUE);
        assert_eq!(options.len(), 1 + KNOWN_MODELS.len());
        assert!(options.iter().any(|(value, label)| value == "claude-opus-5-5" && label == "Opus 5.5"));
    }

    #[test]
    fn a_saved_model_that_is_not_in_the_table_is_still_offered_and_selected() {
        let options = model_options(Some("my-private-model"));
        assert!(options.contains(&("my-private-model".to_string(), "Custom: my-private-model".to_string())));
        assert_eq!(model_value(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn a_known_saved_model_is_not_listed_twice() {
        let options = model_options(Some("claude-sonnet-5-5"));
        assert_eq!(options.iter().filter(|(value, _)| value == "claude-sonnet-5-5").count(), 1);
    }

    #[test]
    fn no_saved_model_selects_the_default_option() {
        assert_eq!(model_value(None), DEFAULT_MODEL_VALUE);
    }

    #[test]
    fn labels_use_the_friendly_name_for_known_models_and_the_id_otherwise() {
        assert_eq!(model_label(None), "Default model");
        assert_eq!(model_label(Some("claude-haiku-4-5-20251001")), "Haiku 4.5");
        assert_eq!(model_label(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn choosing_the_default_option_saves_an_empty_model() {
        assert_eq!(model_from_value(DEFAULT_MODEL_VALUE), "");
        assert_eq!(model_from_value("claude-opus-5-5"), "claude-opus-5-5");
    }
}
