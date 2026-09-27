use forge_domain::{Message, MessageRole, PermissionRequest, Session, SessionId, ToolCall, ToolResult};
use forge_ports::SidecarEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionUpdate {
    SessionStarted { session_id: SessionId },
    MessageDelta { text: String },
    ToolCallStarted { tool_call: ToolCall },
    ToolCallCompleted { tool_call_id: String },
    PermissionRequested(PermissionRequest),
    TurnCompleted { input_tokens: u32, output_tokens: u32 },
    Errored { message: String },
    SessionEnded,
}

pub fn apply_sidecar_event(session: &mut Session, event: &SidecarEvent) -> SessionUpdate {
    match event {
        SidecarEvent::SessionStarted { session_id } => SessionUpdate::SessionStarted {
            session_id: SessionId(session_id.clone()),
        },
        SidecarEvent::AssistantDelta { text } => {
            match session.messages.last_mut() {
                Some(message) if message.role == MessageRole::Assistant => {
                    message.content.push_str(text);
                }
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.content.push_str(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::MessageDelta { text: text.clone() }
        }
        SidecarEvent::ToolUse { tool_call_id, name, input } => {
            let tool_call = ToolCall {
                id: tool_call_id.clone(),
                name: name.clone(),
                input: input.clone(),
                result: None,
            };
            match session
                .messages
                .last_mut()
                .filter(|m| m.role == MessageRole::Assistant)
            {
                Some(message) => message.tool_calls.push(tool_call.clone()),
                None => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.tool_calls.push(tool_call.clone());
                    session.messages.push(message);
                }
            }
            SessionUpdate::ToolCallStarted { tool_call }
        }
        SidecarEvent::ToolResult { tool_call_id, output, is_error } => {
            'search: for message in session.messages.iter_mut().rev() {
                for tool_call in message.tool_calls.iter_mut() {
                    if &tool_call.id == tool_call_id {
                        tool_call.result = Some(ToolResult {
                            output: output.clone(),
                            is_error: *is_error,
                        });
                        break 'search;
                    }
                }
            }
            SessionUpdate::ToolCallCompleted { tool_call_id: tool_call_id.clone() }
        }
        SidecarEvent::PermissionRequest { permission_request_id, tool_name, input } => {
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: permission_request_id.clone(),
                tool_name: tool_name.clone(),
                input: input.clone(),
            })
        }
        SidecarEvent::TurnComplete { input_tokens, output_tokens } => SessionUpdate::TurnCompleted {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
        },
        SidecarEvent::Error { message } => SessionUpdate::Errored { message: message.clone() },
        SidecarEvent::SessionEnded => SessionUpdate::SessionEnded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::PermissionMode;
    use std::path::PathBuf;

    fn empty_session() -> Session {
        Session {
            id: SessionId("s-1".to_string()),
            project_path: PathBuf::from("/tmp/project"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
        }
    }

    #[test]
    fn session_started_returns_the_session_id() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::SessionStarted { session_id: "real-id".to_string() },
        );
        assert_eq!(
            update,
            SessionUpdate::SessionStarted { session_id: SessionId("real-id".to_string()) }
        );
    }

    #[test]
    fn first_assistant_delta_creates_a_new_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hello".to_string() });

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, MessageRole::Assistant);
        assert_eq!(session.messages[0].content, "Hello");
    }

    #[test]
    fn second_assistant_delta_appends_to_the_same_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hel".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "lo".to_string() });

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].content, "Hello");
    }

    #[test]
    fn assistant_delta_after_a_user_message_starts_a_new_message() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hi".to_string() });

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].content, "Hi");
    }

    #[test]
    fn tool_use_appends_a_tool_call_to_the_last_assistant_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Let me check".to_string() });
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse {
                tool_call_id: "t-1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({"command": "ls"}),
            },
        );

        assert_eq!(session.messages[0].tool_calls.len(), 1);
        assert_eq!(session.messages[0].tool_calls[0].id, "t-1");
        assert_eq!(session.messages[0].tool_calls[0].name, "Bash");
        assert!(session.messages[0].tool_calls[0].result.is_none());
    }

    #[test]
    fn tool_use_with_no_preceding_assistant_message_starts_a_new_one() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse {
                tool_call_id: "t-1".to_string(),
                name: "Bash".to_string(),
                input: serde_json::json!({}),
            },
        );

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].tool_calls.len(), 1);
    }

    #[test]
    fn tool_result_attaches_to_the_matching_tool_call_by_id() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "done".to_string(), is_error: false },
        );

        let tool_call = &session.messages[0].tool_calls[0];
        assert_eq!(tool_call.result.as_ref().unwrap().output, "done");
        assert!(!tool_call.result.as_ref().unwrap().is_error);
    }

    #[test]
    fn tool_result_for_an_unknown_id_is_ignored_without_panicking() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "does-not-exist".to_string(), output: "ignored".to_string(), is_error: false },
        );
        assert!(session.messages.is_empty());
    }

    #[test]
    fn tool_result_updates_the_most_recent_matching_tool_call_when_ids_repeat() {
        let mut session = empty_session();
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "first".to_string(), is_error: false },
        );
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "second".to_string(), is_error: false },
        );

        assert_eq!(session.messages[0].tool_calls[0].result.as_ref().unwrap().output, "first");
        assert_eq!(session.messages[2].tool_calls[0].result.as_ref().unwrap().output, "second");
    }

    #[test]
    fn permission_request_is_surfaced_as_a_domain_permission_request() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::PermissionRequest {
                permission_request_id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
                input: serde_json::json!({"command": "rm -rf /"}),
            },
        );
        assert_eq!(
            update,
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
                input: serde_json::json!({"command": "rm -rf /"}),
            })
        );
    }

    #[test]
    fn turn_complete_reports_usage() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::TurnComplete { input_tokens: 10, output_tokens: 20 });
        assert_eq!(update, SessionUpdate::TurnCompleted { input_tokens: 10, output_tokens: 20 });
    }

    #[test]
    fn error_and_session_ended_pass_through() {
        let mut session = empty_session();
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::Error { message: "boom".to_string() }),
            SessionUpdate::Errored { message: "boom".to_string() }
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::SessionEnded),
            SessionUpdate::SessionEnded
        );
    }
}
