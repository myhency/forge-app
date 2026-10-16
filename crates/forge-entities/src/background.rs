use std::time::SystemTime;

use forge_domain::{BackgroundTaskInfo, FinishedTask, Message, RunningTask, TaskStatus};
use forge_shared::tr;

use crate::workflow_progress::{journal_path, workflow_ref};
use crate::workflow_script::workflow_meta;

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
    if !forge_domain::is_absolute_path(path) || path.contains('\0') || forge_domain::has_parent_part(path) {
        return false;
    }
    path.replace('\\', "/").ends_with(&format!("/tasks/{task_id}.output"))
}

/// What a finished task's card knows about its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskEnd {
    pub status: TaskStatus,
    pub summary: String,
    pub tokens: Option<u64>,
    pub duration_ms: Option<u64>,
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
    /// Set for a workflow, when the Workflow call that started it is in the session.
    pub workflow: Option<WorkflowCard>,
    /// The tokens the SDK says a running task has used.
    pub tokens: Option<u64>,
    /// Set once the task has ended.
    pub end: Option<TaskEnd>,
}

/// The running cards and the finished ones of a session.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskLists {
    pub running: Vec<TaskCard>,
    /// Newest first.
    pub finished: Vec<TaskCard>,
}

/// What a workflow's card knows besides the task itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowCard {
    pub name: String,
    /// The phases the script lists, in order.
    pub phases: Vec<String>,
    /// The journal the card reads its progress from, only when its folder is a workflow run's.
    pub journal_path: Option<String>,
}

struct Calls<'a> {
    started: Vec<(BackgroundRef, &'a forge_domain::ToolCall)>,
    workflows: Vec<(String, WorkflowCard, Option<String>)>,
}

fn calls(messages: &[Message]) -> Calls<'_> {
    let started = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter_map(|call| {
            let reference = background_ref(&call.result.as_ref()?.output)?;
            Some((reference, call))
        })
        .collect();
    let workflows = messages
        .iter()
        .flat_map(|message| message.tool_calls())
        .filter(|call| call.name == "Workflow")
        .filter_map(|call| {
            let reference = workflow_ref(&call.result.as_ref()?.output)?;
            let meta = workflow_meta(call.input.get("script")?.as_str()?)?;
            let card = WorkflowCard { name: meta.name, phases: meta.phases, journal_path: journal_path(&reference.transcript_dir) };
            Some((reference.task_id, card, meta.description))
        })
        .collect();
    Calls { started, workflows }
}

fn build_card(calls: &Calls<'_>, info: &BackgroundTaskInfo, seen_at: SystemTime, tokens: Option<u64>, end: Option<TaskEnd>) -> TaskCard {
    let workflow = calls.workflows.iter().find(|(task_id, _, _)| task_id == &info.id);
    let call = calls.started.iter().find(|(reference, _)| reference.task_id == info.id);
    let command = call.and_then(|(_, call)| call.input.get("command")).and_then(|value| value.as_str()).map(str::to_string);
    let description = call
        .and_then(|(_, call)| call.input.get("description"))
        .and_then(|value| value.as_str())
        .filter(|text| !text.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| info.description.clone());
    let output_path = call.map(|(reference, _)| reference.output_path.clone()).filter(|path| valid_output_path(path, &info.id));
    let description = workflow.and_then(|(_, _, text)| text.clone()).unwrap_or(description);
    let workflow = workflow.map(|(_, card, _)| card.clone());
    TaskCard { id: info.id.clone(), description, task_type: info.task_type.clone(), command, output_path, seen_at, workflow, tokens, end }
}

/// A card for each running task, in the SDK's order. The command, a better description and the
/// output file come from the Bash call whose result names the task; a task without such a call
/// (a sub-agent) keeps the SDK's description and has no command and no output.
pub fn task_cards(messages: &[Message], running: &[RunningTask]) -> Vec<TaskCard> {
    let calls = calls(messages);
    running.iter().map(|task| build_card(&calls, &task.info, task.seen_at, task.tokens, None)).collect()
}

/// The running cards and the finished ones. A task that is in both (its notification came before the
/// set lost it) is a finished card only. A finished task with no call in the session has the
/// notification's summary for its description.
pub fn task_lists(messages: &[Message], running: &[RunningTask], finished: &[FinishedTask]) -> TaskLists {
    let calls = calls(messages);
    let ended = |id: &str| finished.iter().any(|item| item.task_id == id);
    let running = running.iter().filter(|task| !ended(&task.info.id)).map(|task| build_card(&calls, &task.info, task.seen_at, task.tokens, None)).collect();
    let finished = finished
        .iter()
        .rev()
        .map(|item| {
            let info = BackgroundTaskInfo { id: item.task_id.clone(), description: item.summary.clone(), task_type: String::new() };
            let end = TaskEnd { status: item.status, summary: item.summary.clone(), tokens: item.tokens, duration_ms: item.duration_ms };
            build_card(&calls, &info, SystemTime::UNIX_EPOCH, item.tokens, Some(end))
        })
        .collect();
    TaskLists { running, finished }
}

/// The text of a failed task's error box: what follows `failed: ` in the summary, else the summary.
pub fn error_text(summary: &str) -> String {
    match summary.find(" failed: ") {
        Some(at) => summary[at + " failed: ".len()..].to_string(),
        None => summary.to_string(),
    }
}

pub fn end_label(status: TaskStatus) -> String {
    match status {
        TaskStatus::Completed => tr!("Completed").to_string(),
        TaskStatus::Stopped => tr!("Stopped").to_string(),
        TaskStatus::Failed => tr!("Failed").to_string(),
    }
}

/// The key of a card in the panel. It changes when the card's output file becomes known: the panel
/// starts reading the output when a card is created, so a card made before the Bash result named
/// the file must be replaced by a new one. The same for a workflow's card and its journal, and for a
/// running card that has ended.
pub fn card_key(card: &TaskCard) -> String {
    let end = card.end.as_ref().map_or("running", |end| end.status.as_str());
    format!("{}:{}:{}:{}", card.id, card.output_path.is_some(), card.workflow.is_some(), end)
}

/// The key of a finished card in the cleared set: the task id plus how this run ended, so a task
/// that is resumed and ends again is not hidden by an earlier clear.
pub fn clear_key(card: &TaskCard) -> String {
    match &card.end {
        Some(end) => format!("{}:{}:{:?}:{:?}", card.id, end.status.as_str(), end.tokens, end.duration_ms),
        None => card.id.clone(),
    }
}

/// The pill under the chat: `1 background task`, `2 background tasks`.
pub fn pill_label(count: usize) -> String {
    if count == 1 { tr!("{count} background task", count = count) } else { tr!("{count} background tasks", count = count) }
}

/// The pill when nothing runs but finished tasks are listed: `1 task finished`.
pub fn finished_pill_label(count: usize) -> String {
    if count == 1 { tr!("{count} task finished", count = count) } else { tr!("{count} tasks finished", count = count) }
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
            tokens: None,
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
    fn a_card_gets_a_new_key_when_its_output_file_becomes_known() {
        // The panel starts reading a card's output when the card is created; a card made before the
        // Bash result arrived (no path yet) must be replaced, not kept, once the path is known.
        let before = task_cards(&[], &[running("b1", 0)]);
        let after = task_cards(&[bash("sleep 60", None, &SENTENCE.replace("bplyt9cwq", "b1"))], &[running("b1", 0)]);
        assert_eq!(before[0].output_path, None);
        assert!(after[0].output_path.is_some());
        assert_ne!(card_key(&before[0]), card_key(&after[0]));
        assert_eq!(card_key(&after[0]), card_key(&after[0].clone()));
        assert!(card_key(&before[0]).starts_with("b1:"));
    }

    const WORKFLOW_RESULT: &str = "Workflow launched in background. Task ID: wf1\nSummary: s\nTranscript dir: /Users/me/.claude/projects/-tmp/6a/subagents/workflows/wf_abc-123\nScript file: /x.js\n";
    const WORKFLOW_SCRIPT: &str = "export const meta = {\n  name: 'risk-review',\n  description: 'Three review, one merges',\n  phases: [{ title: 'Analyze' }, { title: 'Merge' }],\n}\n";

    fn workflow_call(result: &str) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks.push(Block::Tool(ToolCall {
            id: "w".to_string(),
            name: "Workflow".to_string(),
            input: json!({"script": WORKFLOW_SCRIPT}),
            result: Some(ToolResult { output: result.to_string(), is_error: false }),
        }));
        message
    }

    #[test]
    fn a_workflow_card_takes_its_name_phases_and_journal_from_the_workflow_call() {
        let cards = task_cards(&[workflow_call(WORKFLOW_RESULT)], &[running("wf1", 0)]);
        let workflow = cards[0].workflow.as_ref().expect("a workflow card");
        assert_eq!(workflow.name, "risk-review");
        assert_eq!(workflow.phases, vec!["Analyze".to_string(), "Merge".to_string()]);
        assert!(workflow.journal_path.as_deref().is_some_and(|path| path.ends_with("wf_abc-123/journal.jsonl")));
        assert_eq!(cards[0].description, "Three review, one merges");
        assert_eq!(cards[0].output_path, None, "a workflow has no output file");
    }

    #[test]
    fn a_workflow_card_before_its_result_is_an_ordinary_card_and_gets_a_new_key_after() {
        let before = task_cards(&[], &[running("wf1", 0)]);
        let after = task_cards(&[workflow_call(WORKFLOW_RESULT)], &[running("wf1", 0)]);
        assert_eq!(before[0].workflow, None);
        assert_ne!(card_key(&before[0]), card_key(&after[0]));
    }

    #[test]
    fn a_workflow_in_an_unexpected_folder_has_a_card_but_no_journal() {
        let odd = WORKFLOW_RESULT.replace("/subagents/workflows/wf_abc-123", "/elsewhere");
        let cards = task_cards(&[workflow_call(&odd)], &[running("wf1", 0)]);
        let workflow = cards[0].workflow.as_ref().expect("a workflow card");
        assert_eq!(workflow.journal_path, None);
        assert_eq!(workflow.name, "risk-review");
    }

    #[test]
    fn the_pill_counts_the_tasks() {
        assert_eq!(pill_label(1), "1 background task");
        assert_eq!(pill_label(2), "2 background tasks");
        assert_eq!(pill_label(0), "0 background tasks");
    }

    #[test]
    fn the_finished_pill_counts_the_tasks() {
        assert_eq!(finished_pill_label(1), "1 task finished");
        assert_eq!(finished_pill_label(2), "2 tasks finished");
    }

    #[test]
    fn a_windows_output_path_is_valid_and_a_parent_part_is_not() {
        assert!(valid_output_path("C:\\Users\\me\\AppData\\Local\\Temp\\claude\\x\\tasks\\abc1.output", "abc1"));
        assert!(!valid_output_path("C:\\Users\\..\\tasks\\abc1.output", "abc1"));
        assert!(!valid_output_path("C:\\Users\\me\\other\\abc1.output", "abc1"));
    }

    fn finished(id: &str, status: TaskStatus, summary: &str) -> forge_domain::FinishedTask {
        forge_domain::FinishedTask { task_id: id.to_string(), status, summary: summary.to_string(), tokens: Some(157_469), duration_ms: Some(70_914) }
    }

    #[test]
    fn finished_cards_come_newest_first_with_their_end() {
        let lists = task_lists(
            &[workflow_call(WORKFLOW_RESULT)],
            &[],
            &[finished("b1", TaskStatus::Completed, "Background command \"x\" completed"), finished("wf1", TaskStatus::Failed, "Dynamic workflow \"risk-review\" failed: Error: boom")],
        );
        assert!(lists.running.is_empty());
        let ids: Vec<&str> = lists.finished.iter().map(|card| card.id.as_str()).collect();
        assert_eq!(ids, vec!["wf1", "b1"]);
        let end = lists.finished[0].end.as_ref().unwrap();
        assert_eq!((end.status, end.tokens, end.duration_ms), (TaskStatus::Failed, Some(157_469), Some(70_914)));
        assert_eq!(lists.finished[0].workflow.as_ref().unwrap().name, "risk-review", "the Workflow call gives the name");
        assert_eq!(lists.finished[1].description, "Background command \"x\" completed", "no Bash call: the summary stands in");
    }

    #[test]
    fn a_task_in_both_lists_shows_once_as_finished() {
        let lists = task_lists(&[], &[running("b1", 3), running("b2", 3)], &[finished("b1", TaskStatus::Completed, "s")]);
        assert_eq!(lists.running.iter().map(|card| card.id.as_str()).collect::<Vec<_>>(), vec!["b2"]);
        assert_eq!(lists.finished.len(), 1);
    }

    #[test]
    fn a_finished_bash_card_still_names_its_command_and_description() {
        let messages = vec![bash("sleep 12; echo done", Some("Sleep 12 seconds then print done"), &SENTENCE.replace("bplyt9cwq", "bg9"))];
        let lists = task_lists(&messages, &[], &[finished("bg9", TaskStatus::Completed, "Background command completed (exit code 0)")]);
        assert_eq!(lists.finished[0].description, "Sleep 12 seconds then print done");
        assert_eq!(lists.finished[0].command.as_deref(), Some("sleep 12; echo done"));
    }

    #[test]
    fn a_running_card_carries_the_tokens_the_view_saw() {
        let mut task = running("b1", 0);
        task.tokens = Some(99_100);
        assert_eq!(task_cards(&[], &[task])[0].tokens, Some(99_100));
    }

    #[test]
    fn a_running_card_and_its_finished_card_have_different_keys() {
        let before = task_cards(&[], &[running("b1", 0)]);
        let after = task_lists(&[], &[], &[finished("b1", TaskStatus::Stopped, "s")]);
        assert_ne!(card_key(&before[0]), card_key(&after.finished[0]));
    }

    #[test]
    fn two_ends_of_one_task_have_different_clear_keys() {
        let mut first = task_lists(&[], &[], &[finished("b1", TaskStatus::Failed, "s")]).finished.remove(0);
        let mut second = first.clone();
        second.end.as_mut().unwrap().tokens = Some(1);
        second.end.as_mut().unwrap().duration_ms = Some(2);
        assert_ne!(clear_key(&first), clear_key(&second));
        assert_eq!(clear_key(&first), clear_key(&first.clone()));
        first.end = None;
        assert_eq!(clear_key(&first), "b1");
    }

    #[test]
    fn the_error_box_shows_what_follows_the_failed_marker() {
        assert_eq!(
            error_text("Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)"),
            "Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)"
        );
        assert_eq!(error_text("Background command \"x\" failed with exit code 1"), "Background command \"x\" failed with exit code 1");
        assert_eq!(error_text(""), "");
    }

    #[test]
    fn each_end_has_a_label() {
        assert_eq!(end_label(TaskStatus::Completed), "Completed");
        assert_eq!(end_label(TaskStatus::Stopped), "Stopped");
        assert_eq!(end_label(TaskStatus::Failed), "Failed");
    }
}
