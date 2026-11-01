use forge_shared::tr;
/// The value of the "Default" option in a model list (an empty model is stored as `None`).
pub const DEFAULT_MODEL_VALUE: &str = "__default";

/// The models offered in Settings: `(id, label)`, the composer's four first and the older ones after them.
///
/// The list is the one the bundled Claude Code reports (`supportedModels()`), checked against the real SDK on 2026-10-31
/// with `sidecar/scripts/list_models.mjs`, and every id was accepted by `setModel` (the SDK refused `claude-sonnet-5-5`
/// on 2026-10-03; it accepts it now). Edit it here when the list changes.
pub const KNOWN_MODELS: [(&str, &str); 12] = [
    ("claude-opus-5-5", "Opus 5.5"),
    ("claude-sonnet-5-5", "Sonnet 5.5"),
    ("claude-fable-5-1", "Fable 5.1"),
    ("claude-haiku-5-5", "Haiku 5.5"),
    ("claude-haiku-4-5-20251001", "Haiku 4.5"),
    ("claude-sonnet-5", "Sonnet 5"),
    ("claude-opus-5", "Opus 5"),
    ("claude-fable-5", "Fable 5"),
    ("claude-opus-4-8", "Opus 4.8"),
    ("claude-opus-4-7", "Opus 4.7"),
    ("claude-opus-4-6", "Opus 4.6"),
    ("claude-sonnet-4-6", "Sonnet 4.6"),
];

fn is_known(id: &str) -> bool {
    KNOWN_MODELS.iter().any(|(known, _)| *known == id)
}

/// `(value, label)` for a model dropdown: the default, the known models, and — when the saved
/// model is something else (typed by hand) — that model, so it is shown rather than hidden.
pub fn model_options(saved: Option<&str>) -> Vec<(String, String)> {
    let mut options = vec![(DEFAULT_MODEL_VALUE.to_string(), tr!("Default (chosen by the SDK)").to_string())];
    options.extend(KNOWN_MODELS.iter().map(|(id, label)| (id.to_string(), label.to_string())));
    if let Some(id) = saved.filter(|id| !is_known(id)) {
        options.push((id.to_string(), tr!("Custom: {id}", id = id)));
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
        None => tr!("Default model").to_string(),
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
    /// The number key that picks the row (1 to 4); only the first four known models have one.
    pub key: Option<usize>,
    /// A divider is drawn above the row.
    pub divider_before: bool,
    /// A small tag after the name: `Default` on the model the SDK starts on, `Credits needed` where the account must pay for it.
    pub badge: Option<&'static str>,
}

// Claude Code desktop's order: the four first, then the older ones under "More models".
const MENU_ORDER: [&str; 4] = ["claude-opus-5-5", "claude-sonnet-5-5", "claude-fable-5-1", "claude-haiku-5-5"];
const MORE_ORDER: [&str; 8] =
    ["claude-haiku-4-5-20251001", "claude-sonnet-5", "claude-opus-5", "claude-fable-5", "claude-opus-4-8", "claude-opus-4-7", "claude-opus-4-6", "claude-sonnet-4-6"];

/// The tag of a model in the menu.
fn badge_of(id: &str) -> Option<&'static str> {
    match id {
        "claude-sonnet-5-5" => Some(tr!("Default")),
        "claude-fable-5-1" | "claude-fable-5" => Some(tr!("Credits needed")),
        _ => None,
    }
}

fn row_of(id: &str, key: Option<usize>) -> ModelRow {
    ModelRow { value: Some(id.to_string()), label: model_label(Some(id)), key, divider_before: false, badge: badge_of(id) }
}

/// The rows of the composer's model picker: the four main models with keys 1 to 4, then — when the session runs on
/// something that is in no list — that model, then the default below a divider.
pub fn model_menu_rows(current: Option<&str>) -> Vec<ModelRow> {
    let mut rows: Vec<ModelRow> = MENU_ORDER.iter().enumerate().map(|(index, id)| row_of(id, Some(index + 1))).collect();
    if let Some(id) = current.filter(|id| !is_known(id)) {
        rows.push(ModelRow { value: Some(id.to_string()), label: tr!("Custom: {id}", id = id), key: None, divider_before: true, badge: None });
    }
    let divider_before = !rows.last().is_some_and(|row| row.divider_before);
    rows.push(ModelRow { value: None, label: tr!("Default model").to_string(), key: None, divider_before, badge: None });
    rows
}

/// The older models, for the "More models" submenu, in the desktop app's order.
pub fn more_model_rows() -> Vec<ModelRow> {
    MORE_ORDER.iter().map(|id| row_of(id, None)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_rows_follow_the_desktop_order_with_keys_one_to_four() {
        let rows = model_menu_rows(None);
        let labels: Vec<_> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, vec!["Opus 5.5", "Sonnet 5.5", "Fable 5.1", "Haiku 5.5", "Default model"]);
        let keys: Vec<_> = rows.iter().map(|row| row.key).collect();
        assert_eq!(keys, vec![Some(1), Some(2), Some(3), Some(4), None]);
        assert_eq!(rows[4].value, None);
        assert!(rows[4].divider_before);
    }

    #[test]
    fn the_older_models_are_under_more_models_in_the_desktop_order() {
        let labels: Vec<String> = more_model_rows().into_iter().map(|row| row.label).collect();
        assert_eq!(labels, ["Haiku 4.5", "Sonnet 5", "Opus 5", "Fable 5", "Opus 4.8", "Opus 4.7", "Opus 4.6", "Sonnet 4.6"]);
        assert!(more_model_rows().iter().all(|row| row.key.is_none()));
    }

    #[test]
    fn sonnet_5_5_is_the_default_and_fable_needs_credits() {
        let badges: Vec<(String, Option<&str>)> = model_menu_rows(None).into_iter().map(|row| (row.label, row.badge)).collect();
        assert!(badges.contains(&("Sonnet 5.5".to_string(), Some("Default"))));
        assert!(badges.contains(&("Fable 5.1".to_string(), Some("Credits needed"))));
        assert!(badges.iter().filter(|(_, badge)| badge.is_some()).count() == 2);
        assert!(more_model_rows().iter().any(|row| row.label == "Fable 5" && row.badge == Some("Credits needed")));
    }

    #[test]
    fn every_menu_model_is_a_known_model_and_none_is_listed_twice() {
        let listed: Vec<&str> = MENU_ORDER.iter().chain(MORE_ORDER.iter()).copied().collect();
        for id in &listed {
            assert!(is_known(id), "{id}");
        }
        assert_eq!(listed.len(), KNOWN_MODELS.len());
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), listed.len(), "a model in both lists");
        // Settings lists the same ids in the same order as the menus.
        assert_eq!(KNOWN_MODELS.iter().map(|(id, _)| *id).collect::<Vec<_>>(), listed);
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
    fn a_current_model_that_is_only_under_more_models_is_not_offered_as_custom() {
        let rows = model_menu_rows(Some("claude-sonnet-5"));
        assert!(rows.iter().all(|row| !row.label.starts_with("Custom")), "a known model is not custom");
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
        let options = model_options(Some("claude-sonnet-5"));
        assert_eq!(options.iter().filter(|(value, _)| value == "claude-sonnet-5").count(), 1);
    }

    #[test]
    fn no_saved_model_selects_the_default_option() {
        assert_eq!(model_value(None), DEFAULT_MODEL_VALUE);
    }

    #[test]
    fn labels_use_the_friendly_name_for_known_models_and_the_id_otherwise() {
        assert_eq!(model_label(None), "Default model");
        assert_eq!(model_label(Some("claude-haiku-5-5")), "Haiku 5.5");
        assert_eq!(model_label(Some("claude-sonnet-5-5")), "Sonnet 5.5");
        assert_eq!(model_label(Some("claude-haiku-4-5-20251001")), "Haiku 4.5");
        assert_eq!(model_label(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn choosing_the_default_option_saves_an_empty_model() {
        assert_eq!(model_from_value(DEFAULT_MODEL_VALUE), "");
        assert_eq!(model_from_value("claude-opus-5-5"), "claude-opus-5-5");
    }
}
