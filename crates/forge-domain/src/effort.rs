use serde::{Deserialize, Serialize};

/// How much thinking the model does for a turn (the SDK's `EffortLevel`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    #[serde(rename = "xhigh")]
    XHigh,
    Max,
}

impl EffortLevel {
    pub const ALL: [EffortLevel; 5] = [EffortLevel::Low, EffortLevel::Medium, EffortLevel::High, EffortLevel::XHigh, EffortLevel::Max];

    pub fn label(self) -> &'static str {
        match self {
            EffortLevel::Low => "Low",
            EffortLevel::Medium => "Medium",
            EffortLevel::High => "High",
            EffortLevel::XHigh => "Extra high",
            EffortLevel::Max => "Max",
        }
    }
}

/// A model the SDK lists for the running session, and the effort levels it takes (empty: none).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AvailableModel {
    pub value: String,
    pub display_name: String,
    #[serde(default)]
    pub effort_levels: Vec<EffortLevel>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_use_the_sdk_names_on_the_wire() {
        let json: Vec<String> = EffortLevel::ALL.iter().map(|l| serde_json::to_string(l).unwrap()).collect();
        assert_eq!(json, vec!["\"low\"", "\"medium\"", "\"high\"", "\"xhigh\"", "\"max\""]);
        assert_eq!(serde_json::from_str::<EffortLevel>("\"xhigh\"").unwrap(), EffortLevel::XHigh);
    }

    #[test]
    fn levels_are_ordered_from_low_to_max() {
        let mut sorted = EffortLevel::ALL.to_vec();
        sorted.reverse();
        sorted.sort();
        assert_eq!(sorted, EffortLevel::ALL.to_vec());
    }

    #[test]
    fn an_available_model_without_levels_reads_as_taking_none() {
        let model: AvailableModel = serde_json::from_str(r#"{"value":"m","display_name":"M"}"#).unwrap();
        assert!(model.effort_levels.is_empty());
    }
}
