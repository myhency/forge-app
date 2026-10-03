use forge_domain::{PermissionDecision, PermissionMode};
use forge_ports::StartSessionParams;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SidecarCommand {
    StartSession {
        id: String,
        cwd: String,
        resume_session_id: Option<String>,
        permission_mode: WireMode,
        model: Option<String>,
        skills: BTreeMap<String, String>,
        plugins: Vec<String>,
        mcp_servers: BTreeMap<String, serde_json::Value>,
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
    Auto,
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
            PermissionMode::Auto => WireMode::Auto,
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
            skills: params
                .extensions
                .skills
                .into_iter()
                .map(|(name, dir)| (name, dir.to_string_lossy().to_string()))
                .collect(),
            plugins: params
                .extensions
                .plugin_paths
                .iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
            mcp_servers: params.extensions.mcp_servers,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::ActiveExtensions;
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    #[test]
    fn start_session_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                extensions: ActiveExtensions::default(),
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
                "model": null,
                "skills": {},
                "plugins": [],
                "mcp_servers": {}
            })
        );
    }

    #[test]
    fn start_session_command_carries_the_active_extensions() {
        let mut mcp_servers = BTreeMap::new();
        mcp_servers.insert("files".to_string(), serde_json::json!({"command": "npx", "args": ["files-mcp"]}));
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                extensions: ActiveExtensions {
                    skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
                    plugin_paths: vec![PathBuf::from("/plugins/hud")],
                    mcp_servers,
                },
            },
        );

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(json["skills"], serde_json::json!({"pdf": "/skills/pdf"}));
        assert_eq!(json["plugins"], serde_json::json!(["/plugins/hud"]));
        assert_eq!(json["mcp_servers"], serde_json::json!({"files": {"command": "npx", "args": ["files-mcp"]}}));
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
