use std::time::SystemTime;

use forge_domain::{Message, RunningTask};

/// How much of a task's output the panel shows: the last 20000 bytes.
pub const TASK_OUTPUT_TAIL: usize = 20_000;

/// What a Bash call that started a background command says about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackgroundRef {
    pub task_id: String,
    pub output_path: String,
}

const ID_MARK: &str = "Command running in background with ID: ";
const PATH_MARK: &str = "Output is being written to: ";

/// Reads `Command running in background with ID: <id>. Output is being written to: <path>. You will
/// be notified ...` (the Bash tool's result for `run_in_background`). Anything else is `None`.
pub fn background_ref(output: &str) -> Option<BackgroundRef> {
    let after_id = &output[output.find(ID_MARK)? + ID_MARK.len()..];
    let id_end = after_id.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after_id.len());
    let task_id = &after_id[..id_end];
    let after_path = &output[output.find(PATH_MARK)? + PATH_MARK.len()..];
    let path_end = after_path.find(". You will be notified").or_else(|| after_path.find('\n')).unwrap_or(after_path.len());
    let output_path = after_path[..path_end].trim().trim_end_matches('.');
    (!task_id.is_empty() && !output_path.is_empty()).then(|| BackgroundRef { task_id: task_id.to_string(), output_path: output_path.to_string() })
}

/// Whether Forge may read this file as a task's output: absolute, no `..`, no NUL, and exactly
/// `…/tasks/<task_id>.output`.
pub fn valid_output_path(path: &str, task_id: &str) -> bool {
    if task_id.is_empty() || !task_id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return false;
    }
    if !path.starts_with('/') || path.contains('\0') || path.split('/').any(|part| part == "..") {
        return false;
    }
    path.ends_with(&format!("/tasks/{task_id}.output"))
}

/// One card of the Tasks panel.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskCard {
    pub id: String,
    pub description: String,
    pub task_type: String,
    /// The command of the Bash call that started it, when that call is in the session.
    pub command: Option<String>,
    /// The file the task writes its output to, only when `valid_output_path` holds.
    pub output_path: Option<String>,
    pub seen_at: SystemTime,
}

/// A card for each running task, in the SDK's order. The command, a better description and the
/// output file come from the Bash call whose result names the task; a task without such a call
/// (a sub-agent) keeps the SDK's description and has no command and no output.
pub fn task_cards(messages: &[Message], running: &[RunningTask]) -> Vec<TaskCard> {
    let started: Vec<(BackgroundRef, &forge_domain::ToolCall)> = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter_map(|call| {
            let reference = background_ref(&call.result.as_ref()?.output)?;
            Some((reference, call))
        })
        .collect();
    running
        .iter()
        .map(|task| {
            let call = started.iter().find(|(reference, _)| reference.task_id == task.info.id);
            let command = call.and_then(|(_, call)| call.input.get("command")).and_then(|value| value.as_str()).map(str::to_string);
            let description = call
                .and_then(|(_, call)| call.input.get("description"))
                .and_then(|value| value.as_str())
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| task.info.description.clone());
            let output_path = call
                .map(|(reference, _)| reference.output_path.clone())
                .filter(|path| valid_output_path(path, &task.info.id));
            TaskCard { id: task.info.id.clone(), description, task_type: task.info.task_type.clone(), command, output_path, seen_at: task.seen_at }
        })
        .collect()
}

/// The pill under the chat: `1 background task`, `2 background tasks`.
pub fn pill_label(count: usize) -> String {
    format!("{count} background task{}", if count == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, UNIX_EPOCH};

    use forge_domain::{BackgroundTaskInfo, Block, Message, MessageRole, RunningTask, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;

    const SENTENCE: &str = "Command running in background with ID: bplyt9cwq. Output is being written to: /private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output. You will be notified when it completes. To check interim output, use Read on that file path.";

    #[test]
    fn the_real_sentence_gives_the_id_and_the_output_path() {
        assert_eq!(
            background_ref(SENTENCE),
            Some(BackgroundRef {
                task_id: "bplyt9cwq".to_string(),
                output_path: "/private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output".to_string()
            })
        );
    }

    #[test]
    fn other_text_gives_nothing() {
        for text in ["", "ok", "Command running in background with ID: x.", "Output is being written to: /a/tasks/x.output."] {
            assert_eq!(background_ref(text), None, "{text}");
        }
    }

    #[test]
    fn only_a_task_output_path_of_that_task_is_valid() {
        assert!(valid_output_path("/private/tmp/c/tasks/bplyt9cwq.output", "bplyt9cwq"));
        for bad in [
            "/private/tmp/c/tasks/other.output",
            "/private/tmp/c/tasks/../../etc/passwd",
            "/private/tmp/c/tasks/bplyt9cwq.output/../x",
            "tasks/bplyt9cwq.output",
            "/private/tmp/c/bplyt9cwq.output",
            "/private/tmp/c/tasks/bplyt9cwq.txt",
            "/tasks/bplyt9cwq.output\0",
            "",
        ] {
            assert!(!valid_output_path(bad, "bplyt9cwq"), "{bad}");
        }
    }

    fn running(id: &str, secs: u64) -> RunningTask {
        RunningTask {
            info: BackgroundTaskInfo { id: id.to_string(), description: format!("sdk {id}"), task_type: "local_bash".to_string() },
            seen_at: UNIX_EPOCH + Duration::from_secs(secs),
        }
    }
    fn bash(command: &str, description: Option<&str>, result: &str) -> Message {
        let input = match description {
            Some(d) => json!({"command": command, "description": d, "run_in_background": true}),
            None => json!({"command": command, "run_in_background": true}),
        };
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks.push(Block::Tool(ToolCall {
            id: "t".to_string(),
            name: "Bash".to_string(),
            input,
            result: Some(ToolResult { output: result.to_string(), is_error: false }),
        }));
        message
    }

    #[test]
    fn a_card_takes_its_command_and_description_from_the_bash_call_that_started_it() {
        let messages = vec![bash("sleep 60", Some("Sleep for 60 seconds"), SENTENCE)];
        let cards = task_cards(&messages, &[running("bplyt9cwq", 7)]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].command.as_deref(), Some("sleep 60"));
        assert_eq!(cards[0].description, "Sleep for 60 seconds");
        assert_eq!(cards[0].seen_at, UNIX_EPOCH + Duration::from_secs(7));
        assert_eq!(
            cards[0].output_path.as_deref(),
            Some("/private/tmp/claude-501/-private-tmp-forge-observe/f69f/tasks/bplyt9cwq.output")
        );
    }

    #[test]
    fn a_card_without_a_description_falls_back_to_the_sdk_text_then_the_command() {
        let messages = vec![bash("sleep 60", None, SENTENCE)];
        let cards = task_cards(&messages, &[running("bplyt9cwq", 0)]);
        assert_eq!(cards[0].description, "sdk bplyt9cwq");
    }

    #[test]
    fn a_task_with_no_matching_bash_call_still_gets_a_card_without_command_or_output() {
        let cards = task_cards(&[], &[running("agent1", 0)]);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].description, "sdk agent1");
        assert_eq!(cards[0].command, None);
        assert_eq!(cards[0].output_path, None);
    }

    #[test]
    fn an_output_path_that_is_not_a_task_file_is_never_offered() {
        let evil = "Command running in background with ID: b1. Output is being written to: /etc/passwd. You will be notified when it completes.";
        let cards = task_cards(&[bash("x", None, evil)], &[running("b1", 0)]);
        assert_eq!(cards[0].output_path, None);
        assert_eq!(cards[0].command.as_deref(), Some("x"));
    }

    #[test]
    fn cards_keep_the_order_of_the_sdk_set() {
        let cards = task_cards(&[], &[running("b", 0), running("a", 0)]);
        let ids: Vec<&str> = cards.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn the_pill_counts_the_tasks() {
        assert_eq!(pill_label(1), "1 background task");
        assert_eq!(pill_label(2), "2 background tasks");
        assert_eq!(pill_label(0), "0 background tasks");
    }
}
