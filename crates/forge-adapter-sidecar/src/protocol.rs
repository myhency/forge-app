use forge_domain::{PermissionDecision, PermissionMode};
use forge_ports::StartSessionParams;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarCommand {
    StartSession {
        id: String,
        cwd: String,
        resume_session_id: Option<String>,
        permission_mode: WireMode,
        model: Option<String>,
    },
    SendMessage { id: String, text: String },
    PermissionResponse { id: String, permission_request_id: String, decision: WireDecision },
    Interrupt { id: String },
    Shutdown { id: String },
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WireDecision {
    AllowOnce,
    AllowAlways,
    Deny,
}

impl From<PermissionMode> for WireMode {
    fn from(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::Default => WireMode::Default,
            PermissionMode::AcceptEdits => WireMode::AcceptEdits,
            PermissionMode::BypassPermissions => WireMode::BypassPermissions,
            PermissionMode::Plan => WireMode::Plan,
        }
    }
}

impl From<PermissionDecision> for WireDecision {
    fn from(decision: PermissionDecision) -> Self {
        match decision {
            PermissionDecision::AllowOnce => WireDecision::AllowOnce,
            PermissionDecision::AllowAlways => WireDecision::AllowAlways,
            PermissionDecision::Deny => WireDecision::Deny,
        }
    }
}

impl SidecarCommand {
    pub fn start_session(id: String, params: StartSessionParams) -> Self {
        SidecarCommand::StartSession {
            id,
            cwd: params.cwd,
            resume_session_id: params.resume_session_id,
            permission_mode: params.permission_mode.into(),
            model: params.model,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_session_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
            },
        );

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "start_session",
                "id": "1",
                "cwd": "/tmp/project",
                "resume_session_id": null,
                "permission_mode": "default",
                "model": null
            })
        );
    }

    #[test]
    fn permission_response_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::PermissionResponse {
            id: "2".to_string(),
            permission_request_id: "perm-1".to_string(),
            decision: PermissionDecision::AllowAlways.into(),
        };

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "permission_response",
                "id": "2",
                "permission_request_id": "perm-1",
                "decision": "allow_always"
            })
        );
    }
}
