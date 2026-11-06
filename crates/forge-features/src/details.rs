use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_ports::ChangedFile;

/// The cards of a session's Details panel; each folds on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DetailsCard {
    Environment,
    Context,
    Connectors,
    Outputs,
    Changes,
    Tasks,
}

/// What the Changes card shows: the files with uncommitted changes and the lines they add and remove.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChangeCounts {
    pub files: usize,
    pub added: usize,
    pub removed: usize,
}

/// A request that is applied once: `take` hands it out and forgets it, a new `set` replaces the old.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OneShot<T>(Option<T>);

impl<T: Copy> OneShot<T> {
    pub fn set(&mut self, value: T) {
        self.0 = Some(value);
    }

    pub fn take(&mut self) -> Option<T> {
        self.0.take()
    }

    pub fn pending(&self) -> bool {
        self.0.is_some()
    }
}

impl<T> Default for OneShot<T> {
    fn default() -> Self {
        Self(None)
    }
}

/// `None` when nothing changed. A file git gave no line count for (binary, untracked) is still a
/// file and adds nothing to the sums.
pub fn change_counts(files: &[ChangedFile]) -> Option<ChangeCounts> {
    if files.is_empty() {
        return None;
    }
    Some(ChangeCounts {
        files: files.len(),
        added: files.iter().filter_map(|file| file.added).sum(),
        removed: files.iter().filter_map(|file| file.removed).sum(),
    })
}

/// What the session's own file tools changed, for a folder git cannot read (not a repository): one
/// file per path, the lines each finished Edit, MultiEdit or Write added and removed. The desktop
/// app's Changes card showed `+2 −2` in such a folder after one edit (round 37). `None` when the
/// session changed nothing.
pub fn session_edit_counts(messages: &[forge_domain::Message]) -> Option<ChangeCounts> {
    let mut files: Vec<String> = Vec::new();
    let (mut added, mut removed) = (0, 0);
    for call in messages.iter().flat_map(|message| message.tool_calls()) {
        if !call.result.as_ref().is_some_and(|result| !result.is_error) {
            continue;
        }
        let Some(diff) = forge_entities::diff_for_tool_call(call) else { continue };
        if !files.contains(&diff.path) {
            files.push(diff.path.clone());
        }
        added += diff.added;
        removed += diff.removed;
    }
    (!files.is_empty()).then_some(ChangeCounts { files: files.len(), added, removed })
}

/// Whether a card is folded; cards start open.
pub fn card_folded(flags: &HashMap<(LiveSessionId, DetailsCard), bool>, id: LiveSessionId, card: DetailsCard) -> bool {
    flags.get(&(id, card)).copied().unwrap_or(false)
}

/// Drops the fold flags of every card of a session that closed.
pub fn forget_card_flags(flags: &mut HashMap<(LiveSessionId, DetailsCard), bool>, id: LiveSessionId) {
    flags.retain(|(session, _), _| *session != id);
}

#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall, ToolResult};
    use forge_ports::ChangeStatus;
    use serde_json::json;

    fn finished(name: &str, input: serde_json::Value, is_error: bool) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = vec![Block::Tool(ToolCall {
            id: "t".to_string(),
            name: name.to_string(),
            input,
            result: Some(ToolResult { output: String::new(), is_error }),
        })];
        message
    }

    #[test]
    fn the_sessions_own_edits_count_when_git_cannot() {
        let messages = vec![
            finished("Edit", json!({"file_path": "/p/a.py", "old_string": "a\nb", "new_string": "c\nd"}), false),
            finished("Write", json!({"file_path": "/p/s.md", "content": "1\n2\n3\n"}), false),
            finished("Edit", json!({"file_path": "/p/a.py", "old_string": "x", "new_string": "y"}), false),
            finished("Edit", json!({"file_path": "/p/b.py", "old_string": "x", "new_string": "y"}), true),
            finished("Bash", json!({"command": "ls"}), false),
        ];
        assert_eq!(session_edit_counts(&messages), Some(ChangeCounts { files: 2, added: 6, removed: 3 }));
        assert_eq!(session_edit_counts(&[]), None);
    }

    use super::*;

    fn file(added: Option<usize>, removed: Option<usize>) -> ChangedFile {
        ChangedFile { path: "a.rs".into(), status: ChangeStatus::Modified, added, removed }
    }

    #[test]
    fn change_counts_sums_lines_and_counts_files() {
        let files = [file(Some(3), Some(1)), file(Some(10), Some(0))];
        assert_eq!(change_counts(&files), Some(ChangeCounts { files: 2, added: 13, removed: 1 }));
    }

    #[test]
    fn change_counts_is_none_for_no_files() {
        assert_eq!(change_counts(&[]), None);
    }

    #[test]
    fn a_missing_line_count_counts_as_zero() {
        let files = [file(None, None), file(Some(2), None)];
        assert_eq!(change_counts(&files), Some(ChangeCounts { files: 2, added: 2, removed: 0 }));
    }

    #[test]
    fn a_one_shot_request_is_taken_once_and_a_new_one_replaces_the_old() {
        let mut request = OneShot::default();
        assert!(!request.pending());
        request.set(1);
        request.set(2);
        assert!(request.pending());
        assert_eq!(request.take(), Some(2));
        assert_eq!(request.take(), None);
        assert!(!request.pending());
    }

    #[test]
    fn folding_one_card_leaves_the_others() {
        let mut flags = HashMap::new();
        let id = LiveSessionId(1);
        flags.insert((id, DetailsCard::Context), true);
        assert!(card_folded(&flags, id, DetailsCard::Context));
        assert!(!card_folded(&flags, id, DetailsCard::Environment));
        assert!(!card_folded(&flags, LiveSessionId(2), DetailsCard::Context));
    }

    #[test]
    fn a_closed_sessions_card_flags_are_dropped() {
        let mut flags = HashMap::new();
        let (a, b) = (LiveSessionId(1), LiveSessionId(2));
        for card in [DetailsCard::Environment, DetailsCard::Changes] {
            flags.insert((a, card), true);
        }
        flags.insert((b, DetailsCard::Changes), true);
        forget_card_flags(&mut flags, a);
        assert_eq!(flags.len(), 1);
        assert!(card_folded(&flags, b, DetailsCard::Changes));
    }
}
