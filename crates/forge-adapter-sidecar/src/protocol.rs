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
        effort: Option<forge_domain::EffortLevel>,
        skills: BTreeMap<String, String>,
        plugins: Vec<String>,
        mcp_servers: BTreeMap<String, serde_json::Value>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        additional_directories: Vec<String>,
    },
    SendMessage {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        attachments: Vec<WireAttachment>,
    },
    PermissionResponse { id: String, permission_request_id: String, decision: WireDecision },
    Interrupt { id: String },
    SetPermissionMode { id: String, mode: WireMode },
    /// `None` is the SDK's default model.
    SetModel { id: String, model: Option<String> },
    /// `None` is the SDK's default effort.
    SetEffort { id: String, effort: Option<forge_domain::EffortLevel> },
    Shutdown { id: String },
}

#[derive(Debug, Clone, Serialize)]
pub struct WireAttachment {
    kind: WireAttachmentKind,
    path: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum WireAttachmentKind {
    Image,
    File,
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
            effort: params.effort,
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
            additional_directories: params.additional_directories,
        }
    }

    pub fn send_message(id: String, text: String, attachments: Vec<forge_domain::Attachment>) -> Self {
        SidecarCommand::SendMessage {
            id,
            text,
            attachments: attachments
                .into_iter()
                .map(|attachment| WireAttachment {
                    kind: match attachment.kind {
                        forge_domain::AttachmentKind::Image => WireAttachmentKind::Image,
                        forge_domain::AttachmentKind::File => WireAttachmentKind::File,
                    },
                    path: attachment.path.to_string_lossy().to_string(),
                })
                .collect(),
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
    fn send_message_with_attachments_serialises_kind_and_path() {
        use forge_domain::{Attachment, AttachmentKind};
        let command = SidecarCommand::send_message(
            "7".to_string(),
            "look".to_string(),
            vec![
                Attachment { path: "/내 문서/사진 1.png".into(), kind: AttachmentKind::Image },
                Attachment { path: "/x/b.txt".into(), kind: AttachmentKind::File },
            ],
        );
        let json: serde_json::Value = serde_json::to_value(&command).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "send_message", "id": "7", "text": "look",
                "attachments": [
                    { "kind": "image", "path": "/내 문서/사진 1.png" },
                    { "kind": "file", "path": "/x/b.txt" }
                ]
            })
        );
    }

    #[test]
    fn send_message_without_attachments_leaves_the_field_out() {
        let command = SidecarCommand::send_message("7".to_string(), "hi".to_string(), Vec::new());
        let json: serde_json::Value = serde_json::to_value(&command).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "send_message", "id": "7", "text": "hi" }));
    }

    fn params_with(additional_directories: Vec<String>) -> StartSessionParams {
        StartSessionParams {
            cwd: "/tmp/project".to_string(),
            resume_session_id: None,
            permission_mode: PermissionMode::Default,
            model: None,
            effort: None,
            extensions: ActiveExtensions::default(),
            additional_directories,
        }
    }

    #[test]
    fn set_effort_serializes_a_level_and_the_default_as_null() {
        let named = SidecarCommand::SetEffort { id: "6".to_string(), effort: Some(forge_domain::EffortLevel::XHigh) };
        let default = SidecarCommand::SetEffort { id: "7".to_string(), effort: None };

        assert_eq!(serde_json::to_value(&named).unwrap(), serde_json::json!({"type": "set_effort", "id": "6", "effort": "xhigh"}));
        assert_eq!(serde_json::to_value(&default).unwrap(), serde_json::json!({"type": "set_effort", "id": "7", "effort": null}));
    }

    #[test]
    fn start_session_carries_the_effort() {
        let mut params = params_with(Vec::new());
        params.effort = Some(forge_domain::EffortLevel::High);
        let started = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params)).unwrap();
        assert_eq!(started["effort"], "high");

        let default = serde_json::to_value(SidecarCommand::start_session("2".to_string(), params_with(Vec::new()))).unwrap();
        assert!(default["effort"].is_null());
    }

    #[test]
    fn start_session_leaves_additional_directories_out_when_there_are_none() {
        let json = serde_json::to_value(SidecarCommand::start_session("1".to_string(), params_with(Vec::new()))).unwrap();
        assert!(json.get("additional_directories").is_none());
    }

    #[test]
    fn start_session_carries_additional_directories_when_there_are_some() {
        let json = serde_json::to_value(SidecarCommand::start_session(
            "1".to_string(),
            params_with(vec!["/extra dir".to_string()]),
        ))
        .unwrap();
        assert_eq!(json["additional_directories"], serde_json::json!(["/extra dir"]));
    }

    #[test]
    fn start_session_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::start_session(
            "1".to_string(),
            StartSessionParams {
                cwd: "/tmp/project".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                effort: None,
                extensions: ActiveExtensions::default(),
                additional_directories: Vec::new(),
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
                "effort": null,
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
                effort: None,
                extensions: ActiveExtensions {
                    skills: BTreeMap::from([("pdf".to_string(), PathBuf::from("/skills/pdf"))]),
                    plugin_paths: vec![PathBuf::from("/plugins/hud")],
                    mcp_servers,
                },
                additional_directories: Vec::new(),
            },
        );

        let json = serde_json::to_value(&command).unwrap();
        assert_eq!(json["skills"], serde_json::json!({"pdf": "/skills/pdf"}));
        assert_eq!(json["plugins"], serde_json::json!(["/plugins/hud"]));
        assert_eq!(json["mcp_servers"], serde_json::json!({"files": {"command": "npx", "args": ["files-mcp"]}}));
    }

    #[test]
    fn set_permission_mode_command_serializes_with_the_expected_shape() {
        let command = SidecarCommand::SetPermissionMode { id: "3".to_string(), mode: PermissionMode::Auto.into() };

        assert_eq!(
            serde_json::to_value(&command).unwrap(),
            serde_json::json!({"type": "set_permission_mode", "id": "3", "mode": "auto"})
        );
    }

    #[test]
    fn set_model_command_serializes_a_model_and_the_default_as_null() {
        let named = SidecarCommand::SetModel { id: "4".to_string(), model: Some("claude-opus-5-5".to_string()) };
        let default = SidecarCommand::SetModel { id: "5".to_string(), model: None };

        assert_eq!(
            serde_json::to_value(&named).unwrap(),
            serde_json::json!({"type": "set_model", "id": "4", "model": "claude-opus-5-5"})
        );
        assert_eq!(
            serde_json::to_value(&default).unwrap(),
            serde_json::json!({"type": "set_model", "id": "5", "model": null})
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
