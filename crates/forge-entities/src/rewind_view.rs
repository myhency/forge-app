use forge_domain::{Message, MessageRole, RewindOutcome};
use forge_shared::tr;

use crate::code_run::is_run_message;

/// One earlier message the person can go back to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewindRow {
    pub uuid: String,
    /// The first line of what was said, cut to fit a row.
    pub label: String,
}

const LABEL_CHARS: usize = 80;

fn first_line(text: &str) -> String {
    let line = text.lines().map(str::trim).find(|line| !line.is_empty()).unwrap_or_default();
    if line.chars().count() > LABEL_CHARS {
        let cut: String = line.chars().take(LABEL_CHARS).collect();
        format!("{cut}…")
    } else {
        line.to_string()
    }
}

/// The user messages that can be gone back to, newest first: the ones with an id and some text.
pub fn rewind_rows(messages: &[Message]) -> Vec<RewindRow> {
    messages
        .iter()
        .rev()
        .filter(|message| message.role == MessageRole::User)
        .filter(|message| !is_run_message(&message.text()))
        .filter_map(|message| {
            let label = first_line(&message.text());
            let uuid = message.uuid.clone()?;
            (!label.is_empty()).then_some(RewindRow { uuid, label })
        })
        .collect()
}

/// What the preview of putting the files back says. The paths are shown by name only.
pub fn preview_text(outcome: &RewindOutcome) -> String {
    if !outcome.can_rewind {
        return match outcome.error.as_deref().filter(|e| !e.is_empty()) {
            Some(error) => tr!("The files cannot be put back: {error}", error = error),
            None => tr!("The files cannot be put back.").to_string(),
        };
    }
    match outcome.files.len() {
        0 => tr!("No file changed since then.").to_string(),
        1 => tr!("1 file changes (+{added} −{removed})", added = outcome.insertions, removed = outcome.deletions),
        count => tr!("{count} files change (+{added} −{removed})", count = count, added = outcome.insertions, removed = outcome.deletions),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn said(text: &str, uuid: Option<&str>) -> Message {
        let mut message = Message::user(text);
        message.uuid = uuid.map(str::to_string);
        message
    }

    #[test]
    fn rows_are_the_user_messages_with_an_id_newest_first() {
        let messages = vec![
            said("first\nsecond line", Some("u1")),
            Message::assistant_text("answer"),
            said("no id yet", None),
            said("   ", Some("blank")),
            said("last", Some("u3")),
        ];
        let rows = rewind_rows(&messages);
        assert_eq!(rows, vec![RewindRow { uuid: "u3".into(), label: "last".into() }, RewindRow { uuid: "u1".into(), label: "first".into() }]);
    }

    #[test]
    fn a_run_message_is_not_a_rewind_point() {
        let run = crate::code_run::model_message("ls", Some(0), "a\n", false);
        let rows = rewind_rows(&[said("first", Some("u1")), said(&run, Some("u2"))]);
        assert_eq!(rows, vec![RewindRow { uuid: "u1".into(), label: "first".into() }]);
    }

    #[test]
    fn a_long_first_line_is_cut() {
        let rows = rewind_rows(&[said(&"x".repeat(200), Some("u"))]);
        assert_eq!(rows[0].label.chars().count(), LABEL_CHARS + 1);
        assert!(rows[0].label.ends_with('…'));
    }

    #[test]
    fn the_preview_counts_files_or_says_why_not() {
        let ok = |files: Vec<&str>| RewindOutcome { can_rewind: true, files: files.into_iter().map(String::from).collect(), insertions: 3, deletions: 4, ..Default::default() };
        assert_eq!(preview_text(&ok(vec![])), "No file changed since then.");
        assert_eq!(preview_text(&ok(vec!["/a"])), "1 file changes (+3 −4)");
        assert_eq!(preview_text(&ok(vec!["/a", "/b"])), "2 files change (+3 −4)");
        let no = RewindOutcome { can_rewind: false, error: Some("no checkpoint".into()), ..Default::default() };
        assert_eq!(preview_text(&no), "The files cannot be put back: no checkpoint");
        assert_eq!(preview_text(&RewindOutcome::default()), "The files cannot be put back.");
    }
}
