use std::path::PathBuf;

use crate::{PermissionMode, SessionUsage};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);

#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub id: SessionId,
    pub project_path: PathBuf,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub permission_mode: PermissionMode,
    /// What the session has used so far: restored from the session file, then grown by each turn.
    pub usage: SessionUsage,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    pub id: SessionId,
    pub title: Option<String>,
    pub updated_at: String,
    /// RFC 3339 time the session began: its first transcript timestamp, else the file's creation
    /// time, else `updated_at`.
    pub created_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// What the model thought before it answered; empty when it did not think or the file has none.
    pub thinking: String,
    pub tool_calls: Vec<ToolCall>,
    /// Files and images the person attached to this message (user messages only).
    pub attachments: Vec<crate::Attachment>,
}

impl Message {
    pub fn new(role: MessageRole) -> Self {
        Self {
            role,
            content: String::new(),
            thinking: String::new(),
            tool_calls: Vec::new(),
            attachments: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: serde_json::Value,
    pub result: Option<ToolResult>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolResult {
    pub output: String,
    pub is_error: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PermissionRequest {
    pub id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_new_starts_empty() {
        let message = Message::new(MessageRole::Assistant);
        assert_eq!(message.role, MessageRole::Assistant);
        assert_eq!(message.content, "");
        assert!(message.tool_calls.is_empty());
        assert_eq!(message.thinking, "");
    }

    #[test]
    fn session_id_can_key_a_hash_set() {
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(SessionId("s-1".to_string())));
        assert!(!seen.insert(SessionId("s-1".to_string())));
        assert!(seen.insert(SessionId("s-2".to_string())));
    }

    #[test]
    fn session_id_equality_is_value_based() {
        assert_eq!(SessionId("s-1".to_string()), SessionId("s-1".to_string()));
        assert_ne!(SessionId("s-1".to_string()), SessionId("s-2".to_string()));
    }

    #[test]
    fn a_new_session_has_used_nothing() {
        let session = Session {
            id: SessionId("s".to_string()),
            project_path: PathBuf::from("/p"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: SessionUsage::default(),
        };
        assert!(session.usage.is_empty());
    }
}
