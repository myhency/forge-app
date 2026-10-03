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

/// One row of the model picker in the composer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRow {
    /// `None` is the SDK's default model.
    pub value: Option<String>,
    pub label: String,
    /// The number key that picks the row (1 to 4); only the four known models have one.
    pub key: Option<usize>,
    /// A divider is drawn above the row.
    pub divider_before: bool,
}

// Claude Code desktop's order. Settings lists the same models in `KNOWN_MODELS` order.
const MENU_ORDER: [&str; 4] = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-1", "claude-haiku-4-5-20251001"];

/// The rows of the composer's model picker: the four known models with keys 1 to 4, then — when the
/// session runs on something else — that model, then the default below a divider.
pub fn model_menu_rows(current: Option<&str>) -> Vec<ModelRow> {
    let mut rows: Vec<ModelRow> = MENU_ORDER
        .iter()
        .enumerate()
        .map(|(index, id)| ModelRow {
            value: Some(id.to_string()),
            label: model_label(Some(id)),
            key: Some(index + 1),
            divider_before: false,
        })
        .collect();
    if let Some(id) = current.filter(|id| !is_known(id)) {
        rows.push(ModelRow { value: Some(id.to_string()), label: format!("Custom: {id}"), key: None, divider_before: true });
    }
    let divider_before = !rows.last().is_some_and(|row| row.divider_before);
    rows.push(ModelRow { value: None, label: "Default model".to_string(), key: None, divider_before });
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_rows_follow_the_desktop_order_with_keys_one_to_four() {
        let rows = model_menu_rows(None);
        let labels: Vec<_> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, vec!["Opus 5.5", "Sonnet 5.5", "Fable 5.1", "Haiku 4.5", "Default model"]);
        let keys: Vec<_> = rows.iter().map(|row| row.key).collect();
        assert_eq!(keys, vec![Some(1), Some(2), Some(3), Some(4), None]);
        assert_eq!(rows[4].value, None);
        assert!(rows[4].divider_before);
    }

    #[test]
    fn every_menu_model_is_a_known_model() {
        for id in MENU_ORDER {
            assert!(is_known(id), "{id}");
        }
        assert_eq!(MENU_ORDER.len(), KNOWN_MODELS.len());
    }

    #[test]
    fn a_model_outside_the_table_is_offered_so_that_it_can_be_shown_as_current() {
        let rows = model_menu_rows(Some("my-private-model"));
        let custom = rows.iter().find(|row| row.value.as_deref() == Some("my-private-model")).expect("a custom row");
        assert_eq!(custom.label, "Custom: my-private-model");
        assert_eq!(custom.key, None);
        assert_eq!(rows.iter().filter(|row| row.divider_before).count(), 1, "one divider, above the custom row");
        assert_eq!(rows.last().unwrap().value, None);
    }

    #[test]
    fn a_known_current_model_is_not_listed_twice() {
        let rows = model_menu_rows(Some("claude-sonnet-5-5"));
        assert_eq!(rows.iter().filter(|row| row.value.as_deref() == Some("claude-sonnet-5-5")).count(), 1);
        assert_eq!(rows.len(), 5);
    }

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
