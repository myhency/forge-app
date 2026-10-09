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
pub enum Block {
    Text(String),
    /// What the model thought before it answered.
    Thinking(String),
    Tool(ToolCall),
}

/// One message, its parts in the order they happened: an assistant reply can be text, a tool call,
/// more text, another tool call.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub blocks: Vec<Block>,
    /// Files and images the person attached to this message (user messages only).
    pub attachments: Vec<crate::Attachment>,
    /// Excerpts and comments the person attached (user messages only).
    pub cards: Vec<crate::Card>,
    /// RFC 3339 time of the last transcript line of this message; `None` for a message built live.
    pub at: Option<String>,
}

impl Message {
    pub fn new(role: MessageRole) -> Self {
        Self { role, blocks: Vec::new(), attachments: Vec::new(), cards: Vec::new(), at: None }
    }

    pub fn user(text: impl Into<String>) -> Self {
        let mut message = Self::new(MessageRole::User);
        let text = text.into();
        if !text.is_empty() {
            message.blocks.push(Block::Text(text));
        }
        message
    }

    pub fn assistant_text(text: impl Into<String>) -> Self {
        let mut message = Self::new(MessageRole::Assistant);
        message.blocks.push(Block::Text(text.into()));
        message
    }

    /// All text blocks, joined by a blank line.
    pub fn text(&self) -> String {
        self.joined(|block| match block {
            Block::Text(text) => Some(text.as_str()),
            _ => None,
        })
    }

    /// All thinking blocks, joined by a blank line.
    pub fn thinking(&self) -> String {
        self.joined(|block| match block {
            Block::Thinking(text) => Some(text.as_str()),
            _ => None,
        })
    }

    fn joined<'a>(&'a self, pick: impl Fn(&'a Block) -> Option<&'a str>) -> String {
        self.blocks.iter().filter_map(pick).collect::<Vec<_>>().join("\n\n")
    }

    pub fn tool_calls(&self) -> impl Iterator<Item = &ToolCall> {
        self.blocks.iter().filter_map(|block| match block {
            Block::Tool(call) => Some(call),
            _ => None,
        })
    }

    pub fn tool_calls_mut(&mut self) -> impl Iterator<Item = &mut ToolCall> {
        self.blocks.iter_mut().filter_map(|block| match block {
            Block::Tool(call) => Some(call),
            _ => None,
        })
    }

    /// Streamed text: extends the last block when it is text, otherwise starts a block.
    pub fn push_text(&mut self, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Text(existing)) => existing.push_str(text),
            _ => self.blocks.push(Block::Text(text.to_string())),
        }
    }

    pub fn push_thinking(&mut self, text: &str) {
        match self.blocks.last_mut() {
            Some(Block::Thinking(existing)) => existing.push_str(text),
            _ => self.blocks.push(Block::Thinking(text.to_string())),
        }
    }

    pub fn push_text_block(&mut self, text: String) {
        self.blocks.push(Block::Text(text));
    }

    pub fn push_thinking_block(&mut self, text: String) {
        self.blocks.push(Block::Thinking(text));
    }

    pub fn push_tool(&mut self, call: ToolCall) {
        self.blocks.push(Block::Tool(call));
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
    /// What "always allow" would add, as the SDK's `allowedTools` strings.
    pub rules: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(id: &str) -> ToolCall {
        ToolCall { id: id.to_string(), name: "Bash".to_string(), input: serde_json::json!({}), result: None }
    }

    #[test]
    fn message_new_starts_empty() {
        let message = Message::new(MessageRole::Assistant);
        assert_eq!(message.role, MessageRole::Assistant);
        assert!(message.blocks.is_empty());
        assert_eq!(message.text(), "");
        assert_eq!(message.thinking(), "");
        assert_eq!(message.tool_calls().count(), 0);
        assert_eq!(message.at, None);
    }

    #[test]
    fn a_user_message_has_one_text_block_unless_it_is_empty() {
        assert_eq!(Message::user("hi").blocks, vec![Block::Text("hi".to_string())]);
        assert!(Message::user("").blocks.is_empty());
        assert_eq!(Message::user("hi").role, MessageRole::User);
    }

    #[test]
    fn push_text_extends_the_last_text_block() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text("Hel");
        message.push_text("lo");
        assert_eq!(message.blocks, vec![Block::Text("Hello".to_string())]);
    }

    #[test]
    fn push_text_after_a_tool_call_starts_a_new_block() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text("before");
        message.push_tool(call("t-1"));
        message.push_text("after");
        assert_eq!(
            message.blocks,
            vec![Block::Text("before".to_string()), Block::Tool(call("t-1")), Block::Text("after".to_string())]
        );
    }

    #[test]
    fn push_thinking_extends_thinking_but_not_text() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_thinking("a");
        message.push_thinking("b");
        message.push_text("answer");
        message.push_thinking("c");
        assert_eq!(
            message.blocks,
            vec![Block::Thinking("ab".to_string()), Block::Text("answer".to_string()), Block::Thinking("c".to_string())]
        );
    }

    #[test]
    fn block_pushes_always_start_a_new_block_and_text_joins_with_a_blank_line() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_text_block("one".to_string());
        message.push_text_block("two".to_string());
        message.push_thinking_block("x".to_string());
        message.push_thinking_block("y".to_string());
        assert_eq!(message.blocks.len(), 4);
        assert_eq!(message.text(), "one\n\ntwo");
        assert_eq!(message.thinking(), "x\n\ny");
    }

    #[test]
    fn tool_calls_mut_lets_a_result_be_attached() {
        let mut message = Message::new(MessageRole::Assistant);
        message.push_tool(call("t-1"));
        for tool_call in message.tool_calls_mut() {
            tool_call.result = Some(ToolResult { output: "ok".to_string(), is_error: false });
        }
        assert_eq!(message.tool_calls().next().unwrap().result.as_ref().unwrap().output, "ok");
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
