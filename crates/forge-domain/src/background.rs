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

/// A task that is running now, and when the live view first saw it (the clock counts from here).
#[derive(Debug, Clone, PartialEq)]
pub struct RunningTask {
    pub info: BackgroundTaskInfo,
    pub seen_at: SystemTime,
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
}
