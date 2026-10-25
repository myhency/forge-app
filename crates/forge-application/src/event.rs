use forge_domain::{Attachment, AvailableModel, ContextBreakdown, ContextItem, ContextKind, ContextRow, ContextUsage, McpServerState, McpStatus, EffortLevel, Message, MessageRole, PermissionMode, PermissionRequest, Session, SessionId, SlashCommand, ToolCall, ToolResult};
use forge_ports::SidecarEvent;

#[derive(Debug, Clone, PartialEq)]
pub enum SessionUpdate {
    SessionStarted { session_id: SessionId },
    MessageDelta { text: String },
    ThinkingDelta { text: String },
    /// A compaction line was added to the transcript.
    Compacted,
    ToolCallStarted { tool_call: ToolCall },
    ToolCallCompleted { tool_call_id: String },
    PermissionRequested(PermissionRequest),
    /// The SDK withdrew a request (a stop); its card goes.
    PermissionCancelled { id: String },
    TurnCompleted,
    ContextUsage(ContextUsage),
    /// The breakdown behind the context ring; rows of an unknown kind are already dropped.
    ContextDetail(ContextBreakdown),
    /// The state of every MCP server; an empty list means there are none.
    McpStatus(Vec<McpServerState>),
    CommandsAvailable(Vec<SlashCommand>),
    /// The SDK accepted a mode change; the session already holds the new mode.
    PermissionModeChanged { mode: PermissionMode },
    /// The SDK accepted a model change (`None`: its default).
    ModelChanged { model: Option<String> },
    /// The SDK accepted an effort change (`None`: its default).
    EffortChanged { effort: Option<EffortLevel> },
    /// The models the SDK listed for this session.
    ModelsAvailable(Vec<AvailableModel>),
    /// The SDK refused a mode, model or effort change. The session goes on.
    SettingFailed { message: String },
    BackgroundTasks(Vec<forge_domain::BackgroundTaskInfo>),
    TaskFinished { task_id: String },
    TaskProgress { task_id: String, total_tokens: u64 },
    PromptSuggestion(String),
    Errored { message: String },
    SessionEnded,
}

pub fn push_user_message(session: &mut Session, text: String, attachments: Vec<Attachment>) {
    let (cards, rest) = forge_domain::split_cards(&text);
    let mut message = Message::user(rest);
    message.attachments = attachments;
    message.cards = cards;
    session.messages.push(message);
}

/// The assistant message the running turn is still writing, if there is one.
fn open_assistant_message(session: &mut Session) -> Option<&mut Message> {
    session.messages.last_mut().filter(|message| message.role == MessageRole::Assistant && !message.turn_closed)
}

pub fn apply_sidecar_event(session: &mut Session, event: &SidecarEvent) -> SessionUpdate {
    match event {
        SidecarEvent::SessionStarted { session_id } => SessionUpdate::SessionStarted {
            session_id: SessionId(session_id.clone()),
        },
        SidecarEvent::AssistantDelta { text } => {
            match open_assistant_message(session) {
                Some(message) => message.push_text(text),
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_text(text);
                    session.messages.push(message);
                }
            }
            SessionUpdate::MessageDelta { text: text.clone() }
        }
        SidecarEvent::Compacted { pre_tokens, post_tokens } => {
            match open_assistant_message(session) {
                Some(message) => message.push_compacted(*pre_tokens, *post_tokens),
                None => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_compacted(*pre_tokens, *post_tokens);
                    session.messages.push(message);
                }
            }
            SessionUpdate::Compacted
        }
        SidecarEvent::ThinkingDelta { text } => {
            match open_assistant_message(session) {
                Some(message) => message.push_thinking(text),
                _ => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_thinking(text);
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
            match open_assistant_message(session) {
                Some(message) => message.push_tool(tool_call.clone()),
                None => {
                    let mut message = Message::new(MessageRole::Assistant);
                    message.push_tool(tool_call.clone());
                    session.messages.push(message);
                }
            }
            SessionUpdate::ToolCallStarted { tool_call }
        }
        SidecarEvent::ToolResult { tool_call_id, output, is_error } => {
            'search: for message in session.messages.iter_mut().rev() {
                for tool_call in message.tool_calls_mut() {
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
        SidecarEvent::PermissionRequest { permission_request_id, tool_name, input, rules } => {
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: permission_request_id.clone(),
                tool_name: tool_name.clone(),
                input: input.clone(),
                rules: rules.clone(),
            })
        }
        SidecarEvent::PermissionCancelled { permission_request_id } => SessionUpdate::PermissionCancelled { id: permission_request_id.clone() },
        SidecarEvent::TurnComplete { usage } => {
            for (model, tokens) in usage {
                session.usage.add(model, *tokens);
            }
            if let Some(message) = session.messages.last_mut().filter(|message| message.role == MessageRole::Assistant) {
                message.turn_closed = true;
            }
            SessionUpdate::TurnCompleted
        }
        SidecarEvent::ContextUsage { total_tokens, max_tokens } => SessionUpdate::ContextUsage(ContextUsage {
            total_tokens: *total_tokens,
            max_tokens: *max_tokens,
        }),
        SidecarEvent::ContextDetail { categories, mcp_tools, agents } => SessionUpdate::ContextDetail(ContextBreakdown {
            rows: categories
                .iter()
                .filter_map(|c| Some(ContextRow { name: c.name.clone(), tokens: c.tokens, kind: ContextKind::from_sdk(&c.kind)? }))
                .collect(),
            mcp_tools: mcp_tools.iter().map(context_item).collect(),
            agents: agents.iter().map(context_item).collect(),
        }),
        SidecarEvent::McpStatus { servers } => SessionUpdate::McpStatus(
            servers
                .iter()
                .map(|s| McpServerState {
                    name: s.name.clone(),
                    status: McpStatus::from_sdk(&s.status),
                    scope: s.scope.clone(),
                    source: s.source.clone(),
                    remote: McpServerState::remote_from_transport(s.transport.as_deref()),
                    tool_count: s.tool_count,
                    error: s.error.clone(),
                })
                .collect(),
        ),
        SidecarEvent::CommandsAvailable { commands } => SessionUpdate::CommandsAvailable(commands.clone()),
        SidecarEvent::PermissionModeSet { mode } => {
            session.permission_mode = *mode;
            SessionUpdate::PermissionModeChanged { mode: *mode }
        }
        SidecarEvent::ModelSet { model } => SessionUpdate::ModelChanged { model: model.clone() },
        SidecarEvent::EffortSet { effort } => SessionUpdate::EffortChanged { effort: *effort },
        SidecarEvent::ModelsAvailable { models } => SessionUpdate::ModelsAvailable(models.clone()),
        SidecarEvent::SettingFailed { message } => SessionUpdate::SettingFailed { message: message.clone() },
        SidecarEvent::BackgroundTasks { tasks } => SessionUpdate::BackgroundTasks(tasks.clone()),
        SidecarEvent::TaskFinished { task_id, status, summary, tokens, duration_ms } => {
            if let Some(status) = forge_domain::TaskStatus::parse(status) {
                forge_domain::record_finished(
                    &mut session.finished_tasks,
                    forge_domain::FinishedTask { task_id: task_id.clone(), status, summary: summary.clone(), tokens: *tokens, duration_ms: *duration_ms },
                );
            }
            SessionUpdate::TaskFinished { task_id: task_id.clone() }
        }
        SidecarEvent::TaskProgress { task_id, total_tokens } => SessionUpdate::TaskProgress { task_id: task_id.clone(), total_tokens: *total_tokens },
        SidecarEvent::PromptSuggestion { text } => SessionUpdate::PromptSuggestion(text.clone()),
        SidecarEvent::Error { message } => SessionUpdate::Errored { message: message.clone() },
        SidecarEvent::SessionEnded => SessionUpdate::SessionEnded,
    }
}

fn context_item(item: &forge_ports::ContextItemReport) -> ContextItem {
    ContextItem { group: item.server.clone(), name: item.name.clone(), tokens: item.tokens }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{ContextUsage, PermissionMode, TokenUsage};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn empty_session() -> Session {
        Session {
            finished_tasks: Vec::new(),
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
        assert_eq!(session.messages[0].text(), "Hello");
    }

    #[test]
    fn second_assistant_delta_appends_to_the_same_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Hel".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "lo".to_string() });

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].text(), "Hello");
    }

    #[test]
    fn text_after_a_finished_turn_starts_a_new_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "It is running.".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::TurnComplete { usage: Default::default() });
        // A background task ended and Claude answered again; no user message came in between.
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "It finished.".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: " Here it is.".to_string() });

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].text(), "It is running.");
        assert_eq!(session.messages[1].text(), "It finished. Here it is.", "the new turn keeps growing its own message");
    }

    #[test]
    fn a_thought_or_a_tool_call_after_a_finished_turn_starts_a_new_message_too() {
        let mut thought = empty_session();
        apply_sidecar_event(&mut thought, &SidecarEvent::AssistantDelta { text: "One".to_string() });
        apply_sidecar_event(&mut thought, &SidecarEvent::TurnComplete { usage: Default::default() });
        apply_sidecar_event(&mut thought, &SidecarEvent::ThinkingDelta { text: "Hmm".to_string() });
        assert_eq!(thought.messages.len(), 2);

        let mut tool = empty_session();
        apply_sidecar_event(&mut tool, &SidecarEvent::AssistantDelta { text: "One".to_string() });
        apply_sidecar_event(&mut tool, &SidecarEvent::TurnComplete { usage: Default::default() });
        apply_sidecar_event(
            &mut tool,
            &SidecarEvent::ToolUse { tool_call_id: "t".to_string(), name: "Read".to_string(), input: serde_json::json!({}) },
        );
        assert_eq!(tool.messages.len(), 2);
    }

    #[test]
    fn thinking_deltas_grow_the_thinking_of_the_current_assistant_message() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "Let me ".to_string() });
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "think".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "Answer".to_string() });

        assert_eq!(update, SessionUpdate::ThinkingDelta { text: "think".to_string() });
        assert_eq!(session.messages.len(), 1, "the thought and the answer share one message");
        assert_eq!(session.messages[0].thinking(), "Let me think");
        assert_eq!(session.messages[0].text(), "Answer");
    }

    #[test]
    fn a_compaction_adds_a_line_to_the_open_message_or_starts_one() {
        let mut session = empty_session();
        session.messages.push(Message::user("/compact".to_string()));
        let update = apply_sidecar_event(&mut session, &SidecarEvent::Compacted { pre_tokens: Some(21_702), post_tokens: Some(4_290) });

        assert_eq!(update, SessionUpdate::Compacted);
        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].blocks, vec![forge_domain::Block::Compacted { before: Some(21_702), after: Some(4_290) }]);
    }

    #[test]
    fn thinking_after_a_user_message_starts_an_assistant_message() {
        let mut session = empty_session();
        session.messages.push(Message::new(MessageRole::User));
        apply_sidecar_event(&mut session, &SidecarEvent::ThinkingDelta { text: "hm".to_string() });

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[1].role, MessageRole::Assistant);
        assert_eq!(session.messages[1].thinking(), "hm");
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
    fn effort_set_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let before = session.clone();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::EffortSet { effort: Some(forge_domain::EffortLevel::Low) });

        assert_eq!(session, before);
        assert_eq!(update, SessionUpdate::EffortChanged { effort: Some(forge_domain::EffortLevel::Low) });
    }

    #[test]
    fn the_model_list_is_reported() {
        let mut session = empty_session();
        let models = vec![forge_domain::AvailableModel { value: "m".to_string(), resolved_model: None, display_name: "M".to_string(), effort_levels: vec![] }];
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ModelsAvailable { models: models.clone() });

        assert_eq!(update, SessionUpdate::ModelsAvailable(models));
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
        assert_eq!(session.messages[1].text(), "Hi");
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

        assert_eq!(session.messages[0].tool_calls().count(), 1);
        assert_eq!(session.messages[0].tool_calls().nth(0).unwrap().id, "t-1");
        assert_eq!(session.messages[0].tool_calls().nth(0).unwrap().name, "Bash");
        assert!(session.messages[0].tool_calls().nth(0).unwrap().result.is_none());
    }

    fn tool_use(id: &str) -> SidecarEvent {
        SidecarEvent::ToolUse { tool_call_id: id.to_string(), name: "Bash".to_string(), input: serde_json::json!({"command": "ls"}) }
    }

    #[test]
    fn text_after_a_tool_call_starts_a_new_text_block() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "one ".to_string() });
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "two".to_string() });
        apply_sidecar_event(&mut session, &tool_use("t-1"));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "three".to_string() });

        let kinds: Vec<String> = session.messages[0]
            .blocks
            .iter()
            .map(|block| match block {
                forge_domain::Block::Text(text) => format!("text:{text}"),
                forge_domain::Block::Thinking(_) => "thinking".to_string(),
                forge_domain::Block::Tool(call) => format!("tool:{}", call.id),
                forge_domain::Block::Compacted { .. } => "compacted".to_string(),
            })
            .collect();
        assert_eq!(kinds, vec!["text:one two", "tool:t-1", "text:three"]);
    }

    #[test]
    fn a_tool_result_lands_on_its_call_even_with_text_after_it() {
        let mut session = empty_session();
        apply_sidecar_event(&mut session, &tool_use("t-1"));
        apply_sidecar_event(&mut session, &SidecarEvent::AssistantDelta { text: "later".to_string() });
        apply_sidecar_event(
            &mut session,
            &SidecarEvent::ToolResult { tool_call_id: "t-1".to_string(), output: "out".to_string(), is_error: false },
        );
        let call = session.messages[0].tool_calls().next().unwrap();
        assert_eq!(call.result.as_ref().unwrap().output, "out");
        assert_eq!(session.messages[0].text(), "later");
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
        assert_eq!(session.messages[1].tool_calls().count(), 1);
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

        let tool_call = &session.messages[0].tool_calls().nth(0).unwrap();
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
        assert!(session.messages[0].tool_calls().nth(0).unwrap().result.is_none());
    }

    #[test]
    fn push_user_message_appends_a_user_message() {
        let mut session = empty_session();
        push_user_message(&mut session, "hello".to_string(), Vec::new());

        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, MessageRole::User);
        assert_eq!(session.messages[0].text(), "hello");
    }

    #[test]
    fn push_user_message_turns_card_blocks_into_cards() {
        let mut session = empty_session();
        let card = forge_domain::Card {
            id: 0,
            kind: forge_domain::CardKind::Comment,
            path: "/p/a.py".into(),
            range: forge_domain::LineRange::new(3, 3),
            text: "why".into(),
        };
        push_user_message(&mut session, forge_domain::with_cards(&[card.clone()], "hi"), Vec::new());
        assert_eq!(session.messages[0].text(), "hi");
        assert_eq!(session.messages[0].cards, vec![card]);
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
        assert_eq!(session.messages[0].text(), "first answer");
        assert_eq!(session.messages[1].role, MessageRole::User);
        assert_eq!(session.messages[2].text(), "second answer");
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

        assert_eq!(session.messages[0].tool_calls().nth(0).unwrap().result.as_ref().unwrap().output, "first");
        assert_eq!(session.messages[2].tool_calls().nth(0).unwrap().result.as_ref().unwrap().output, "second");
    }

    #[test]
    fn a_cancelled_permission_request_is_surfaced_by_its_id() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::PermissionCancelled { permission_request_id: "perm-1".to_string() });
        assert_eq!(update, SessionUpdate::PermissionCancelled { id: "perm-1".to_string() });
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
                rules: Vec::new(),
            },
        );
        assert_eq!(
            update,
            SessionUpdate::PermissionRequested(PermissionRequest {
                id: "perm-1".to_string(),
                tool_name: "Bash".to_string(),
                input: serde_json::json!({"command": "rm -rf /"}),
                rules: Vec::new(),
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

    #[test]
    fn background_task_task_finished_and_suggestion_events_become_updates() {
        let mut session = empty_session();
        let tasks = vec![forge_domain::BackgroundTaskInfo { id: "b1".to_string(), description: "d".to_string(), task_type: "t".to_string() }];
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::BackgroundTasks { tasks: tasks.clone() }),
            SessionUpdate::BackgroundTasks(tasks)
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::TaskFinished { task_id: "b1".to_string(), status: "completed".to_string(), summary: "s".to_string(), tokens: None, duration_ms: None }),
            SessionUpdate::TaskFinished { task_id: "b1".to_string() }
        );
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::PromptSuggestion { text: "next".to_string() }),
            SessionUpdate::PromptSuggestion("next".to_string())
        );
    }

    #[test]
    fn a_finished_task_is_recorded_in_the_session_and_progress_becomes_an_update() {
        let mut session = empty_session();
        let update = apply_sidecar_event(
            &mut session,
            &SidecarEvent::TaskFinished { task_id: "w1".to_string(), status: "failed".to_string(), summary: "boom".to_string(), tokens: Some(5), duration_ms: Some(9) },
        );
        assert_eq!(update, SessionUpdate::TaskFinished { task_id: "w1".to_string() });
        assert_eq!(
            session.finished_tasks,
            vec![forge_domain::FinishedTask { task_id: "w1".to_string(), status: forge_domain::TaskStatus::Failed, summary: "boom".to_string(), tokens: Some(5), duration_ms: Some(9) }]
        );
        apply_sidecar_event(&mut session, &SidecarEvent::TaskFinished { task_id: "w2".to_string(), status: "weird".to_string(), summary: String::new(), tokens: None, duration_ms: None });
        assert_eq!(session.finished_tasks.len(), 1, "a status we do not know is not recorded");
        assert_eq!(
            apply_sidecar_event(&mut session, &SidecarEvent::TaskProgress { task_id: "w1".to_string(), total_tokens: 7 }),
            SessionUpdate::TaskProgress { task_id: "w1".to_string(), total_tokens: 7 }
        );
    }
}
