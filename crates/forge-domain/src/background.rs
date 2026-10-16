use std::time::SystemTime;

use serde::{Deserialize, Serialize};

/// A task the SDK runs in the background (a command started with `run_in_background`, a
/// sub-agent). `id` is `task_id` on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTaskInfo {
    #[serde(rename = "task_id")]
    pub id: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub task_type: String,
}

/// A task that is running now, when the live view first saw it (the clock counts from here) and
/// how many tokens the SDK says it has used so far.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningTask {
    pub info: BackgroundTaskInfo,
    pub seen_at: SystemTime,
    pub tokens: Option<u64>,
}

/// How a background task ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Completed,
    Failed,
    Stopped,
}

impl TaskStatus {
    pub fn parse(word: &str) -> Option<Self> {
        match word.trim() {
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "stopped" => Some(Self::Stopped),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}

/// A task that has ended, with what the SDK said about it. `tokens` and `duration_ms` are what its
/// notification reported; a Bash command reports neither.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinishedTask {
    pub task_id: String,
    pub status: TaskStatus,
    pub summary: String,
    pub tokens: Option<u64>,
    pub duration_ms: Option<u64>,
}

/// Adds an ended task. One already listed under the same id is replaced and the new one goes last,
/// so the list stays in the order the tasks ended (a resumed workflow ends again).
pub fn record_finished(list: &mut Vec<FinishedTask>, task: FinishedTask) {
    list.retain(|item| item.task_id != task.task_id);
    list.push(task);
}

const NOTIFICATION_OPEN: &str = "<task-notification>";
/// What follows the head of a notification (`task-id` … `summary`); the head is read up to the first.
const AFTER_HEAD: [&str; 5] = ["<result>", "<diagnostics>", "<recovery>", "<failures>", "<usage>"];

fn tag<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = text.find(&open)? + open.len();
    let end = start + text[start..].find(&close)?;
    Some(&text[start..end])
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

/// Reads the `<task-notification>` block the SDK writes into a session record when a task ends. The
/// format is the SDK's own, not a documented one, so it is read loosely: tags by name, the id, the
/// status and the summary from the head (before the result, which may hold any text), the usage from
/// the last `<usage>` block. `None` when it is not a notification or has no id or a status we do not know.
pub fn parse_task_notification(text: &str) -> Option<FinishedTask> {
    let text = text.trim();
    if !text.starts_with(NOTIFICATION_OPEN) {
        return None;
    }
    let head_end = AFTER_HEAD.iter().filter_map(|mark| text.find(mark)).min().unwrap_or(text.len());
    let head = &text[..head_end];
    let task_id = tag(head, "task-id")?.trim();
    let status = TaskStatus::parse(tag(head, "status")?)?;
    if task_id.is_empty() {
        return None;
    }
    let summary = unescape(tag(head, "summary").unwrap_or("").trim());
    let usage = text.rfind("<usage>").and_then(|at| tag(&text[at..], "usage"));
    let number = |name: &str| usage.and_then(|block| tag(block, name)).and_then(|value| value.trim().parse::<u64>().ok());
    Some(FinishedTask {
        task_id: task_id.to_string(),
        status,
        summary,
        tokens: number("subagent_tokens").or_else(|| number("total_tokens")),
        duration_ms: number("duration_ms"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_reads_from_the_wire_shape_with_defaults() {
        let task: BackgroundTaskInfo = serde_json::from_str(r#"{"task_id":"b1","description":"Sleep","task_type":"local_bash"}"#).unwrap();
        assert_eq!(task, BackgroundTaskInfo { id: "b1".to_string(), description: "Sleep".to_string(), task_type: "local_bash".to_string() });
        let bare: BackgroundTaskInfo = serde_json::from_str(r#"{"task_id":"b2"}"#).unwrap();
        assert_eq!(bare.description, "");
        assert_eq!(bare.task_type, "");
    }

    const COMPLETED: &str = "<task-notification>\n<task-id>wm0ok3aqh</task-id>\n<tool-use-id>toolu_01A</tool-use-id>\n<output-file>/tmp/c/tasks/wm0ok3aqh.output</output-file>\n<status>completed</status>\n<summary>Dynamic workflow \"Find largest file\" completed</summary>\n<result>\"Combined summary\"</result>\n<diagnostics>Per-agent results: /x/journal.jsonl</diagnostics>\n<usage><agent_count>3</agent_count><agents_done>3</agents_done><subagent_tokens>157385</subagent_tokens><tool_uses>4</tool_uses><duration_ms>77189</duration_ms></usage>\n</task-notification>";
    const FAILED: &str = "<task-notification>\n<task-id>w5r3hmkem</task-id>\n<tool-use-id>toolu_01M</tool-use-id>\n<output-file>/tmp/c/tasks/w5r3hmkem.output</output-file>\n<status>failed</status>\n<summary>Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at &lt;anonymous&gt; (workflow.js:11:7)\n    at &lt;anonymous&gt; (evalmachine.&lt;anonymous&gt;:1:42)</summary>\n<recovery>To resume, call Workflow</recovery>\n<failures>parallel[1] failed: </failures>\n<usage><agent_count>3</agent_count><subagent_tokens>157469</subagent_tokens><tool_uses>3</tool_uses><duration_ms>70914</duration_ms></usage>\n</task-notification>";
    const BASH: &str = "<task-notification>\n<task-id>bg9x3ylba</task-id>\n<tool-use-id>toolu_01B</tool-use-id>\n<output-file>/tmp/c/tasks/bg9x3ylba.output</output-file>\n<status>completed</status>\n<summary>Background command \"Sleep 12 seconds then print done\" completed (exit code 0)</summary>\n</task-notification>";

    #[test]
    fn a_completed_workflow_notification_gives_status_tokens_and_time() {
        let task = parse_task_notification(COMPLETED).unwrap();
        assert_eq!(task.task_id, "wm0ok3aqh");
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.summary, "Dynamic workflow \"Find largest file\" completed");
        assert_eq!((task.tokens, task.duration_ms), (Some(157_385), Some(77_189)));
    }

    #[test]
    fn a_failed_notification_keeps_its_unescaped_multi_line_summary() {
        let task = parse_task_notification(FAILED).unwrap();
        assert_eq!(task.status, TaskStatus::Failed);
        assert_eq!(
            task.summary,
            "Dynamic workflow \"Run three\" failed: Error: obs-c boom\n    at <anonymous> (workflow.js:11:7)\n    at <anonymous> (evalmachine.<anonymous>:1:42)"
        );
        assert_eq!((task.tokens, task.duration_ms), (Some(157_469), Some(70_914)));
    }

    #[test]
    fn a_bash_notification_has_no_tokens_and_no_time() {
        let task = parse_task_notification(BASH).unwrap();
        assert_eq!((task.task_id.as_str(), task.status, task.tokens, task.duration_ms), ("bg9x3ylba", TaskStatus::Completed, None, None));
    }

    #[test]
    fn tag_lookalikes_in_the_result_do_not_change_the_task() {
        let tricky = COMPLETED.replace("\"Combined summary\"", "<task-id>evil</task-id><status>failed</status><usage><subagent_tokens>1</subagent_tokens></usage>");
        let task = parse_task_notification(&tricky).unwrap();
        assert_eq!((task.task_id.as_str(), task.status), ("wm0ok3aqh", TaskStatus::Completed));
        assert_eq!(task.tokens, Some(157_385), "the real usage block is the last one");
    }

    #[test]
    fn other_text_is_not_a_notification() {
        for text in ["", "hello", "<task-notification></task-notification>", "<task-notification>\n<task-id>x</task-id>\n<status>weird</status>\n</task-notification>", "<task-notification>\n<task-id></task-id>\n<status>completed</status>\n</task-notification>", "run <task-notification> please"] {
            assert_eq!(parse_task_notification(text), None, "{text}");
        }
    }

    #[test]
    fn a_task_recorded_twice_keeps_the_newer_entry_last() {
        let mut list = Vec::new();
        let task = |id: &str, summary: &str| FinishedTask { task_id: id.to_string(), status: TaskStatus::Completed, summary: summary.to_string(), tokens: None, duration_ms: None };
        record_finished(&mut list, task("a", "first"));
        record_finished(&mut list, task("b", "other"));
        record_finished(&mut list, task("a", "second"));
        let seen: Vec<(&str, &str)> = list.iter().map(|item| (item.task_id.as_str(), item.summary.as_str())).collect();
        assert_eq!(seen, vec![("b", "other"), ("a", "second")]);
    }

    #[test]
    fn status_words_round_trip() {
        for status in [TaskStatus::Completed, TaskStatus::Failed, TaskStatus::Stopped] {
            assert_eq!(TaskStatus::parse(status.as_str()), Some(status));
        }
        assert_eq!(TaskStatus::parse(" stopped\n"), Some(TaskStatus::Stopped));
        assert_eq!(TaskStatus::parse("done"), None);
    }
}
