use forge_domain::{Attachment, ContextUsage, Message, MessageRole, PermissionMode, PermissionRequest, Session, SessionId, SlashCommand, ToolCall, ToolResult};
use forge_ports::SidecarEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionUpdate {
    SessionStarted { session_id: SessionId },
    MessageDelta { text: String },
    ThinkingDelta { text: String },
    ToolCallStarted { tool_call: ToolCall },
    ToolCallCompleted { tool_call_id: String },
    PermissionRequested(PermissionRequest),
    TurnCompleted,
    ContextUsage(ContextUsage),
    CommandsAvailable(Vec<SlashCommand>),
    /// The SDK accepted a mode change; the session already holds the new mode.
    PermissionModeChanged { mode: PermissionMode },
    /// The SDK accepted a model change (`None`: its default).
    ModelChanged { model: Option<String> },
    /// The SDK refused a mode or model change. The session goes on.
    SettingFailed { message: String },
    Errored { message: String },
    SessionEnded,
}

pub fn push_user_message(session: &mut Session, text: String, attachments: Vec<Attachment>) {
    session.messages.push(Message {
        role: MessageRole::User,
        content: text,
        thinking: String::new(),
        tool_calls: Vec::new(),
        attachments,
    });
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
        SidecarEvent::ThinkingDelta { text } => {
            match session.messages.last_mut() {
                Some(message) if message.role == MessageRole::Assistant => {
                    message.thinking.push_str(text);
                }
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.thinking.push_str(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::ThinkingDelta { text: text.clone() }
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
        SidecarEvent::TurnComplete { usage } => {
            for (model, tokens) in usage {
                session.usage.add(model, *tokens);
            }
            SessionUpdate::TurnCompleted
        }
        SidecarEvent::ContextUsage { total_tokens, max_tokens } => SessionUpdate::ContextUsage(ContextUsage {
            total_tokens: *total_tokens,
            max_tokens: *max_tokens,
        }),
        SidecarEvent::CommandsAvailable { commands } => SessionUpdate::CommandsAvailable(commands.clone()),
        SidecarEvent::PermissionModeSet { mode } => {
            session.permission_mode = *mode;
            SessionUpdate::PermissionModeChanged { mode: *mode }
        }
        SidecarEvent::ModelSet { model } => SessionUpdate::ModelChanged { model: model.clone() },
        SidecarEvent::SettingFailed { message } => SessionUpdate::SettingFailed { message: message.clone() },
        SidecarEvent::Error { message } => SessionUpdate::Errored { message: message.clone() },
        SidecarEvent::SessionEnded => SessionUpdate::SessionEnded,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{ContextUsage, PermissionMode, TokenUsage};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn empty_session() -> Session {
        Session {
            id: SessionId("s-1".to_string()),
            project_path: PathBuf::from("/tmp/project"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: forge_domain::SessionUsage::default(),
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
    fn thinking_deltas_grow_the_thinking_of_the_current_assistant_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "Let me ".to_string() });
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "think".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Answer".to_string() });

        assert_eq!(update, SessionUpdate::ThinkingDelta { text: "think".to_string() });
        assert_eq!(session.messages.len(), 1, "the thought and the answer share one message");
        assert_eq!(session.messages[0].thinking, "Let me think");
        assert_eq!(session.messages[0].content, "Answer");
    }

    #[test]
    fn thinking_after_a_user_message_starts_an_assistant_message() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "hm".to_string() });

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].thinking, "hm");
    }

    #[test]
    fn an_accepted_mode_change_updates_the_session_mode() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::PermissionModeSet { mode: PermissionMode::Auto });

        assert_eq!(session.permission_mode, PermissionMode::Auto);
        assert_eq!(update, SessionUpdate::PermissionModeChanged { mode: PermissionMode::Auto });
    }

    #[test]
    fn an_accepted_model_change_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let before = session.clone();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ModelSet { model: Some("m".to_string()) });

        assert_eq!(session, before);
        assert_eq!(update, SessionUpdate::ModelChanged { model: Some("m".to_string()) });
    }

    #[test]
    fn a_refused_setting_is_reported_and_changes_nothing() {
        let mut session = empty_session();
        let before = session.clone();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::SettingFailed { message: "no".to_string() });

        assert_eq!(session, before);
        assert_eq!(update, SessionUpdate::SettingFailed { message: "no".to_string() });
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
            &SidecarEvent::ToolUse { tool_call_id: "t-1".to_string(), name: "Bash".to_string(), input: serde_json::json!({}) },
        );
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "does-not-exist".to_string(), output: "ignored".to_string(), is_error: false },
        );
        assert!(session.messages[0].tool_calls[0].result.is_none());
    }

    #[test]
    fn push_user_message_appends_a_user_message() {
        let mut session = empty_session();
        push_user_message(&mut session, "hello".to_string(), Vec::new());

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[0].content, "hello");
    }

    #[test]
    fn push_user_message_keeps_the_attachments() {
        let mut session = empty_session();
        let attachment = forge_domain::Attachment { path: "/x/b.txt".into(), kind: forge_domain::AttachmentKind::File };
        push_user_message(&mut session, "hi".to_string(), vec![attachment.clone()]);
        assert_eq!(session.messages[0].attachments, vec![attachment]);
    }

    #[test]
    fn push_user_message_ends_the_previous_assistant_message_so_the_next_delta_starts_fresh() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "first answer".to_string() });
        push_user_message(&mut session, "second question".to_string(), Vec::new());
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "second answer".to_string() });

        assert_eq!(session.messages.len(), 3);
        assert_eq!(session.messages[0].content, "first answer");
        assert_eq!(session.messages[1].role, MessageRole::User);
        assert_eq!(session.messages[2].content, "second answer");
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
    fn turn_complete_adds_the_turns_usage_to_the_session() {
        let mut session = empty_session();
        session.usage.add("claude-test", TokenUsage { input: 100, output: 1, ..Default::default() });
        let event = SidecarEvent::TurnComplete {
            usage: BTreeMap::from([
                ("claude-test".to_string(), TokenUsage { input: 10, output: 20, ..Default::default() }),
                ("claude-other".to_string(), TokenUsage { output: 5, ..Default::default() }),
            ]),
        };

        let update = apply_sidecar_event(&mut session, &event);

        assert_eq!(update, SessionUpdate::TurnCompleted);
        assert_eq!(session.usage.total(), TokenUsage { input: 110, output: 26, ..Default::default() });
        assert_eq!(session.usage.models().count(), 2);
    }

    #[test]
    fn context_usage_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ContextUsage { total_tokens: 5, max_tokens: 10 });
        assert_eq!(update, SessionUpdate::ContextUsage(ContextUsage { total_tokens: 5, max_tokens: 10 }));
        assert!(session.usage.is_empty());
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

    #[test]
    fn commands_available_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let commands = vec![forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None }];
        let update = apply_sidecar_event(&mut session, &SidecarEvent::CommandsAvailable { commands: commands.clone() });
        assert_eq!(update, SessionUpdate::CommandsAvailable(commands));
        assert!(session.messages.is_empty());
    }
}
