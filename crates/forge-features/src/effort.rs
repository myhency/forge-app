use forge_domain::{AvailableModel, EffortLevel};
use forge_shared::tr;

pub fn effort_help() -> &'static str {
    tr!("Higher effort means a more thorough answer, but it takes longer and uses your limit faster.")
}

pub fn slider_faster() -> &'static str {
    tr!("Faster")
}

pub fn slider_smarter() -> &'static str {
    tr!("Smarter")
}

/// The effort levels the session's model takes, lowest first; empty when the model is not listed or
/// takes none. A model is found by its listed name or by the full id that name stands for. A session
/// on the SDK's default model (`None`) uses the first model the SDK listed: that is the `default` row
/// (checked against the real SDK, Plan 19).
pub fn effort_levels(models: &[AvailableModel], model: Option<&str>) -> Vec<EffortLevel> {
    let entry = match model {
        Some(id) => models.iter().find(|candidate| candidate.value == id || candidate.resolved_model.as_deref() == Some(id)),
        None => models.first(),
    };
    let mut levels = entry.map(|entry| entry.effort_levels.clone()).unwrap_or_default();
    levels.sort();
    levels.dedup();
    levels
}

/// Where the slider thumb stands: on the current level, else on High (the SDK's default), else on
/// the first stop.
pub fn slider_position(levels: &[EffortLevel], current: Option<EffortLevel>) -> usize {
    let wanted = current.unwrap_or(EffortLevel::High);
    levels.iter().position(|level| *level == wanted).unwrap_or(0)
}

/// The text of the composer's effort button.
pub fn effort_button_label(current: Option<EffortLevel>) -> String {
    current.map_or(tr!("Default"), |level| forge_shared::i18n::text(level.label())).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use EffortLevel::*;

    fn model(value: &str, levels: &[EffortLevel]) -> AvailableModel {
        AvailableModel { value: value.to_string(), resolved_model: None, display_name: value.to_string(), effort_levels: levels.to_vec() }
    }

    #[test]
    fn the_levels_are_those_of_the_sessions_model_lowest_first() {
        let models = vec![model("a", &[High, Low]), model("b", &[Medium])];
        assert_eq!(effort_levels(&models, Some("a")), vec![Low, High]);
        assert_eq!(effort_levels(&models, Some("b")), vec![Medium]);
    }

    #[test]
    fn a_model_is_found_by_the_full_id_an_alias_stands_for() {
        // The SDK lists `opus` and `default` for claude-opus-5-5; a session's model is the full id.
        let mut opus = model("opus", &[Low, High]);
        opus.resolved_model = Some("claude-opus-5-5".to_string());
        let models = vec![model("haiku", &[]), opus];
        assert_eq!(effort_levels(&models, Some("claude-opus-5-5")), vec![Low, High]);
        assert!(effort_levels(&models, Some("claude-opus-4-6")).is_empty());
    }

    #[test]
    fn a_model_that_is_not_listed_or_takes_no_effort_has_no_levels() {
        let models = vec![model("a", &[]), model("b", &[Low])];
        assert!(effort_levels(&models, Some("a")).is_empty(), "takes no effort");
        assert!(effort_levels(&models, Some("zzz")).is_empty(), "not listed");
        assert!(effort_levels(&[], Some("a")).is_empty(), "nothing listed yet");
    }

    #[test]
    fn the_default_model_uses_the_first_listed_one() {
        let models = vec![model("first", &[Low, Medium]), model("second", &[Max])];
        assert_eq!(effort_levels(&models, None), vec![Low, Medium]);
    }

    #[test]
    fn the_thumb_sits_on_the_current_level_else_on_high_else_on_the_first_stop() {
        assert_eq!(slider_position(&[Low, Medium, High], Some(Medium)), 1);
        assert_eq!(slider_position(&[Low, Medium, High], None), 2, "the SDK default is high");
        assert_eq!(slider_position(&[Low, Medium], None), 0, "no high to stand on");
        assert_eq!(slider_position(&[Low, Medium], Some(Max)), 0, "a level the model lacks");
    }

    #[test]
    fn the_button_names_the_level_or_the_default() {
        assert_eq!(effort_button_label(Some(XHigh)), "Extra high");
        assert_eq!(effort_button_label(None), "Default");
    }
}
