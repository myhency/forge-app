use serde::{Deserialize, Serialize};

/// The model that approves or denies each tool call in auto mode, when the person picks one. Without a
/// choice Claude Code decides (and may use Opus). The SDK reads the id from `CLAUDE_CODE_AUTO_MODE_MODEL`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModeModel {
    Sonnet,
    Opus,
}

impl AutoModeModel {
    pub const ALL: [AutoModeModel; 2] = [AutoModeModel::Sonnet, AutoModeModel::Opus];

    /// The value the menu stores and the id the SDK is given.
    pub fn id(self) -> &'static str {
        match self {
            AutoModeModel::Sonnet => "claude-sonnet-5-5",
            AutoModeModel::Opus => "claude-opus-5-5",
        }
    }

    /// The name as the menu shows it.
    pub fn label(self) -> &'static str {
        match self {
            AutoModeModel::Sonnet => "Sonnet 5.5",
            AutoModeModel::Opus => "Opus 5.5",
        }
    }

    pub fn from_id(id: &str) -> Option<AutoModeModel> {
        AutoModeModel::ALL.into_iter().find(|model| model.id() == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_model_is_found_again_by_its_id() {
        for model in AutoModeModel::ALL {
            assert_eq!(AutoModeModel::from_id(model.id()), Some(model));
        }
        assert_eq!(AutoModeModel::from_id(""), None);
    }
}
