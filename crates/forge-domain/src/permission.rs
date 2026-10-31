use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    /// A model classifier approves or denies the permission prompts.
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_mode_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&PermissionMode::Default).unwrap(), "\"default\"");
        assert_eq!(serde_json::to_string(&PermissionMode::AcceptEdits).unwrap(), "\"accept_edits\"");
        assert_eq!(serde_json::to_string(&PermissionMode::BypassPermissions).unwrap(), "\"bypass_permissions\"");
        assert_eq!(serde_json::to_string(&PermissionMode::Plan).unwrap(), "\"plan\"");
        assert_eq!(serde_json::to_string(&PermissionMode::Auto).unwrap(), "\"auto\"");
    }

    #[test]
    fn permission_decision_serializes_to_snake_case() {
        assert_eq!(serde_json::to_string(&PermissionDecision::AllowOnce).unwrap(), "\"allow_once\"");
        assert_eq!(serde_json::to_string(&PermissionDecision::AllowAlways).unwrap(), "\"allow_always\"");
        assert_eq!(serde_json::to_string(&PermissionDecision::Deny).unwrap(), "\"deny\"");
    }

    #[test]
    fn permission_mode_round_trips_through_json() {
        for mode in [PermissionMode::Default, PermissionMode::AcceptEdits, PermissionMode::BypassPermissions, PermissionMode::Plan, PermissionMode::Auto] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: PermissionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }
}
