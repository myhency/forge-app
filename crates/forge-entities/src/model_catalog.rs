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

/// `(value, label)` for a model dropdown: the default, the models offered in the composer's menus, and — when the
/// saved model is something else (typed by hand, or one that is no longer offered) — that model, so it is shown
/// rather than hidden.
pub fn model_options(saved: Option<&str>) -> Vec<(String, String)> {
    let mut options = vec![(DEFAULT_MODEL_VALUE.to_string(), tr!("Default (chosen by the SDK)").to_string())];
    options.extend(MENU_ORDER.iter().chain(MORE_ORDER.iter()).map(|id| (id.to_string(), model_label(Some(id)))));
    if let Some(id) = saved.filter(|id| !is_offered(id)) {
        let label = if is_known(id) { model_label(Some(id)) } else { tr!("Custom: {id}", id = id) };
        options.push((id.to_string(), label));
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
    /// The model's id.
    pub value: Option<String>,
    pub label: String,
    /// The number key that picks the row (1 or 2); only the models of the main list have one.
    pub key: Option<usize>,
    /// A divider is drawn above the row.
    pub divider_before: bool,
    /// A small tag after the name: `Default` on the model the SDK starts on, `Credits needed` where the account must pay for it.
    pub badge: Option<&'static str>,
    /// The model the SDK starts on: checked while no model was picked.
    pub is_default: bool,
}

// The models Forge offers: the two main ones first, the older ones under "More models". Opus and Fable are
// not offered (the user's decision of 2026-11-02); they stay in `KNOWN_MODELS` so a session already on one still
// shows its name.
const MENU_ORDER: [&str; 2] = ["claude-sonnet-5-5", "claude-haiku-5-5"];
const MORE_ORDER: [&str; 3] = ["claude-haiku-4-5-20251001", "claude-sonnet-5", "claude-sonnet-4-6"];

/// The model the SDK starts on.
const DEFAULT_MODEL_ID: &str = "claude-sonnet-5-5";

fn is_offered(id: &str) -> bool {
    MENU_ORDER.iter().chain(MORE_ORDER.iter()).any(|offered| *offered == id)
}

/// The tag of a model in the menu.
fn badge_of(id: &str) -> Option<&'static str> {
    match id {
        "claude-sonnet-5-5" => Some(tr!("Default")),
        "claude-fable-5-1" | "claude-fable-5" => Some(tr!("Credits needed")),
        _ => None,
    }
}

fn row_of(id: &str, key: Option<usize>) -> ModelRow {
    ModelRow { value: Some(id.to_string()), label: model_label(Some(id)), key, divider_before: false, badge: badge_of(id), is_default: id == DEFAULT_MODEL_ID }
}

/// The rows of the composer's model picker: the main models with keys 1 and 2, then — when the session runs on a
/// model that no list offers — that model below a divider. There is no "default" row: the SDK's default model is
/// the one tagged `Default`, and it is checked while no model was picked.
pub fn model_menu_rows(current: Option<&str>) -> Vec<ModelRow> {
    let mut rows: Vec<ModelRow> = MENU_ORDER.iter().enumerate().map(|(index, id)| row_of(id, Some(index + 1))).collect();
    if let Some(id) = current.filter(|id| !is_offered(id)) {
        let label = if is_known(id) { model_label(Some(id)) } else { tr!("Custom: {id}", id = id) };
        rows.push(ModelRow { value: Some(id.to_string()), label, key: None, divider_before: true, badge: badge_of(id), is_default: false });
    }
    rows
}

/// The older models, for the "More models" submenu.
pub fn more_model_rows() -> Vec<ModelRow> {
    MORE_ORDER.iter().map(|id| row_of(id, None)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_rows_are_sonnet_and_haiku_with_keys_one_and_two() {
        let rows = model_menu_rows(None);
        let labels: Vec<_> = rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, vec!["Sonnet 5.5", "Haiku 5.5"]);
        let keys: Vec<_> = rows.iter().map(|row| row.key).collect();
        assert_eq!(keys, vec![Some(1), Some(2)]);
    }

    #[test]
    fn only_haiku_4_5_sonnet_5_and_sonnet_4_6_are_under_more_models() {
        let labels: Vec<String> = more_model_rows().into_iter().map(|row| row.label).collect();
        assert_eq!(labels, ["Haiku 4.5", "Sonnet 5", "Sonnet 4.6"]);
        assert!(more_model_rows().iter().all(|row| row.key.is_none()));
    }

    #[test]
    fn opus_and_fable_are_offered_nowhere() {
        let offered: Vec<String> = model_menu_rows(None).into_iter().chain(more_model_rows()).map(|row| row.label).collect();
        assert!(offered.iter().all(|label| !label.starts_with("Opus") && !label.starts_with("Fable")), "{offered:?}");
        let options = model_options(None);
        assert!(options.iter().all(|(value, _)| !value.contains("opus") && !value.contains("fable")), "{options:?}");
    }

    #[test]
    fn sonnet_5_5_is_the_default() {
        let badges: Vec<(String, Option<&str>)> = model_menu_rows(None).into_iter().map(|row| (row.label, row.badge)).collect();
        assert!(badges.contains(&("Sonnet 5.5".to_string(), Some("Default"))));
        assert_eq!(badges.iter().filter(|(_, badge)| badge.is_some()).count(), 1);
    }

    #[test]
    fn every_offered_model_is_a_known_model_and_none_is_listed_twice() {
        let listed: Vec<&str> = MENU_ORDER.iter().chain(MORE_ORDER.iter()).copied().collect();
        for id in &listed {
            assert!(is_known(id), "{id}");
        }
        let mut sorted = listed.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), listed.len(), "a model in both lists");
    }

    #[test]
    fn a_model_outside_the_table_is_offered_so_that_it_can_be_shown_as_current() {
        let rows = model_menu_rows(Some("my-private-model"));
        let custom = rows.iter().find(|row| row.value.as_deref() == Some("my-private-model")).expect("a custom row");
        assert_eq!(custom.label, "Custom: my-private-model");
        assert_eq!(custom.key, None);
        assert!(custom.divider_before);
    }

    #[test]
    fn a_session_on_a_hidden_known_model_still_shows_its_name() {
        let rows = model_menu_rows(Some("claude-opus-5-5"));
        assert_eq!(rows.last().unwrap().label, "Opus 5.5");
        assert_eq!(model_label(Some("claude-opus-5-5")), "Opus 5.5");
    }

    #[test]
    fn a_current_model_that_is_only_under_more_models_gets_no_extra_row() {
        let rows = model_menu_rows(Some("claude-sonnet-5"));
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn the_default_comes_first_then_the_offered_models() {
        let options = model_options(None);
        assert_eq!(options[0].0, DEFAULT_MODEL_VALUE);
        assert_eq!(options.len(), 1 + MENU_ORDER.len() + MORE_ORDER.len());
        assert!(options.iter().any(|(value, label)| value == "claude-sonnet-5-5" && label == "Sonnet 5.5"));
    }

    #[test]
    fn a_saved_model_that_is_not_in_the_table_is_still_offered_and_selected() {
        let options = model_options(Some("my-private-model"));
        assert!(options.contains(&("my-private-model".to_string(), "Custom: my-private-model".to_string())));
        assert_eq!(model_value(Some("my-private-model")), "my-private-model");
    }

    #[test]
    fn a_saved_model_that_is_no_longer_offered_is_still_shown_by_its_name() {
        let options = model_options(Some("claude-opus-5-5"));
        assert!(options.contains(&("claude-opus-5-5".to_string(), "Opus 5.5".to_string())));
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
