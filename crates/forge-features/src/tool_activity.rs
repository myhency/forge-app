use forge_domain::Session;

/// How many tool calls in the session have a result. It grows each time a tool call finishes, so
/// a panel that shows the files on disk reloads when it changes.
pub fn finished_tool_calls(session: &Session) -> usize {
    session.messages.iter().flat_map(|message| &message.tool_calls).filter(|call| call.result.is_some()).count()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_domain::{Message, MessageRole, PermissionMode, SessionId, SessionUsage, ToolCall, ToolResult};

    use super::*;

    fn empty_session() -> Session {
        Session {
            id: SessionId(String::new()),
            project_path: PathBuf::from("/work/a"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: SessionUsage::default(),
        }
    }

    fn call(result: bool) -> ToolCall {
        ToolCall {
            id: "t".to_string(),
            name: "Edit".to_string(),
            input: serde_json::json!({}),
            result: result.then(|| ToolResult { output: "ok".to_string(), is_error: false }),
        }
    }

    #[test]
    fn only_tool_calls_with_a_result_count() {
        let mut session = empty_session();
        assert_eq!(finished_tool_calls(&session), 0);
        let mut message = Message::new(MessageRole::Assistant);
        message.tool_calls = vec![call(true), call(false), call(true)];
        session.messages.push(message);
        assert_eq!(finished_tool_calls(&session), 2);
    }
}
