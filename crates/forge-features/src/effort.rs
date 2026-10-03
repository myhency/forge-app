use forge_domain::{AvailableModel, EffortLevel};

pub const EFFORT_HELP: &str = "Higher effort means a more thorough answer, but it takes longer and uses your limit faster.";
pub const SLIDER_FASTER: &str = "Faster";
pub const SLIDER_SMARTER: &str = "Smarter";

/// The effort levels the session's model takes, lowest first; empty when the model is not listed or
/// takes none. A session on the SDK's default model (`None`) uses the first model the SDK listed.
pub fn effort_levels(models: &[AvailableModel], model: Option<&str>) -> Vec<EffortLevel> {
    let entry = match model {
        Some(id) => models.iter().find(|candidate| candidate.value == id),
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
    current.map_or("Default", EffortLevel::label).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use EffortLevel::*;

    fn model(value: &str, levels: &[EffortLevel]) -> AvailableModel {
        AvailableModel { value: value.to_string(), display_name: value.to_string(), effort_levels: levels.to_vec() }
    }

    #[test]
    fn the_levels_are_those_of_the_sessions_model_lowest_first() {
        let models = vec![model("a", &[High, Low]), model("b", &[Medium])];
        assert_eq!(effort_levels(&models, Some("a")), vec![Low, High]);
        assert_eq!(effort_levels(&models, Some("b")), vec![Medium]);
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
