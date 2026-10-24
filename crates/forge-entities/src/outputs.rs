//! The files a session produced: what its file tools wrote and what the last message of each turn
//! names. The Workspace card lists them; programs are left out of the list.

use forge_domain::{is_absolute_path, Message, MessageRole};

use crate::links::absolute_paths_in;

/// The extensions of a file named in a message that count as an output.
const OUTPUT_EXTENSIONS: [&str; 15] = ["xlsx", "xls", "csv", "docx", "doc", "pptx", "ppt", "pdf", "md", "txt", "html", "png", "jpg", "jpeg", "svg"];
const PROGRAM_EXTENSIONS: [&str; 9] = ["app", "exe", "command", "sh", "bat", "cmd", "msi", "scpt", "jar"];

// The extension in lower case, if the last part of the path has one.
fn extension(path: &str) -> Option<String> {
    let (stem, extension) = forge_domain::file_name(path).rsplit_once('.')?;
    (!stem.is_empty()).then(|| extension.to_ascii_lowercase())
}

/// Whether the path is a program or script, which opens in its default program and is never an output.
pub fn is_program(path: &str) -> bool {
    extension(path).is_some_and(|extension| PROGRAM_EXTENSIONS.contains(&extension.as_str()))
}

fn is_output_extension(path: &str) -> bool {
    extension(path).is_some_and(|extension| OUTPUT_EXTENSIONS.contains(&extension.as_str()))
}

// The index of each turn's last assistant message with text: the last one before the next user message.
fn last_texts(messages: &[Message]) -> Vec<usize> {
    let mut found = Vec::new();
    let mut last: Option<usize> = None;
    for (index, message) in messages.iter().enumerate() {
        if message.role == MessageRole::User {
            found.extend(last.take());
        } else if !message.text().trim().is_empty() {
            last = Some(index);
        }
    }
    found.extend(last);
    found
}

/// The absolute paths the session produced, newest first, each once: files its Write, Edit,
/// MultiEdit and NotebookEdit calls named (Read does not count), and files of a document type
/// named in the last assistant text of each turn.
pub fn output_paths(messages: &[Message]) -> Vec<String> {
    let closing = last_texts(messages);
    let mut mentions: Vec<String> = Vec::new();
    for (index, message) in messages.iter().enumerate() {
        for call in message.tool_calls().filter(|call| matches!(call.name.as_str(), "Write" | "Edit" | "MultiEdit" | "NotebookEdit")) {
            let path = ["file_path", "notebook_path"].iter().find_map(|key| call.input.get(*key).and_then(|value| value.as_str()));
            mentions.extend(path.filter(|path| is_absolute_path(path) && !path.contains(['"', '\'', '<', '>', '&']) && !path.chars().any(char::is_control)).map(str::to_string));
        }
        if closing.contains(&index) {
            mentions.extend(absolute_paths_in(&message.text()).into_iter().filter(|path| is_output_extension(path)));
        }
    }
    let mut paths: Vec<String> = Vec::new();
    for path in mentions.into_iter().rev() {
        if !paths.contains(&path) {
            paths.push(path);
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use forge_domain::{Block, ToolCall};
    use serde_json::json;

    use super::*;

    fn user() -> Message {
        let mut message = Message::new(MessageRole::User);
        message.blocks = vec![Block::Text("go".to_string())];
        message
    }
    fn said(text: &str) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = vec![Block::Text(text.to_string())];
        message
    }
    fn tool(name: &str, key: &str, path: &str) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = vec![Block::Tool(ToolCall { id: "t".to_string(), name: name.to_string(), input: json!({ key: path }), result: None })];
        message
    }

    #[test]
    fn write_tools_count_and_read_does_not() {
        let messages = vec![
            user(),
            tool("Write", "file_path", "/p/a.txt"),
            tool("Edit", "file_path", "/p/b.rs"),
            tool("MultiEdit", "file_path", "/p/c.rs"),
            tool("NotebookEdit", "notebook_path", "/p/d.ipynb"),
            tool("Read", "file_path", "/p/e.rs"),
            tool("Write", "file_path", "relative.txt"),
        ];
        assert_eq!(output_paths(&messages), vec!["/p/d.ipynb", "/p/c.rs", "/p/b.rs", "/p/a.txt"]);
    }

    #[test]
    fn a_path_in_the_last_message_of_a_turn_counts_if_its_extension_is_a_document_type() {
        let messages = vec![user(), said("done"), said("Saved `/p/report.xlsx` and /p/notes.MD."), user(), said("see /p/x.PDF")];
        assert_eq!(output_paths(&messages), vec!["/p/x.PDF", "/p/notes.MD", "/p/report.xlsx"]);
    }

    #[test]
    fn a_path_in_an_earlier_message_of_the_turn_does_not_count() {
        let messages = vec![user(), said("first /p/early.xlsx"), tool("Read", "file_path", "/p/r.rs"), said("finished")];
        assert!(output_paths(&messages).is_empty());
    }

    #[test]
    fn etc_hosts_and_a_path_without_a_document_extension_do_not_count() {
        let messages = vec![user(), said("see /etc/hosts and /p/main.rs and /p/run.sh and relative/a.xlsx")];
        assert!(output_paths(&messages).is_empty());
    }

    #[test]
    fn the_newest_mention_comes_first_and_duplicates_collapse() {
        let messages = vec![user(), tool("Write", "file_path", "/p/a.csv"), tool("Write", "file_path", "/p/b.csv"), said("/p/a.csv is ready")];
        assert_eq!(output_paths(&messages), vec!["/p/a.csv", "/p/b.csv"]);
    }

    #[test]
    fn a_write_path_with_an_attribute_breaking_character_is_dropped() {
        let messages = vec![
            user(),
            tool("Write", "file_path", "/p/a\"onmouseover=x.txt"),
            tool("Write", "file_path", "/p/<b>&.md"),
            tool("Write", "file_path", "/p/it's.md"),
            tool("Write", "file_path", "/p/bell\u{7}.md"),
            tool("Write", "file_path", "/p/ok.md"),
        ];
        assert_eq!(output_paths(&messages), vec!["/p/ok.md"]);
    }

    #[test]
    fn korean_and_windows_paths_named_in_text_count() {
        let messages = vec![user(), said("저장: C:\\Users\\나\\보고서.xlsx 그리고 /p/보고서.md를 확인")];
        assert_eq!(output_paths(&messages), vec!["/p/보고서.md", "C:\\Users\\나\\보고서.xlsx"]);
    }

    #[test]
    fn is_program_knows_scripts_in_any_case() {
        for path in ["/p/run.sh", "/p/Setup.EXE", "/Applications/X.app", "C:\\a\\b.Bat", "/p/a.jar", "/p/a.scpt"] {
            assert!(is_program(path), "{path}");
        }
        assert!(!is_program("/p/a.xlsx"));
        assert!(!is_program("/p/sh"));
    }
}
