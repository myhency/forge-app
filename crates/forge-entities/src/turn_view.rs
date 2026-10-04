use forge_domain::{Block, Message, MessageRole, ToolCall};
use serde_json::Value;

use crate::diff::diff_for_tool_call;
use crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};
use crate::transcript_view::TranscriptView;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolKind {
    Created,
    Edited,
    Read,
    Command,
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolRow {
    pub call: ToolCall,
    pub kind: ToolKind,
    /// "Created", "Edited", "Read", empty for a command, the tool name otherwise.
    pub verb: String,
    /// The file name, the command's description (else the command), or the input summary.
    pub subject: String,
    /// The full path for a file tool; empty for the others.
    pub path: String,
    pub added: usize,
    pub removed: usize,
    /// Whether the call has a diff to show (a Write or Edit with a path and content).
    pub has_diff: bool,
    pub failed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Row {
    Tool(ToolRow),
    Narration(String),
    Thinking(String),
}

/// The folded part of a turn: from the first tool call to the last.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub rows: Vec<Row>,
    pub summary: String,
    pub added: usize,
    pub removed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CardFile {
    pub name: String,
    pub path: String,
    pub added: usize,
    pub removed: usize,
}

/// The files a finished turn wrote or edited, each once.
#[derive(Debug, Clone, PartialEq)]
pub struct FileCard {
    pub files: Vec<CardFile>,
    pub added: usize,
    pub removed: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Footer {
    /// What the copy button puts on the clipboard: the turn's closing text.
    pub text: String,
    pub at: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Item {
    User(Message),
    Text(String),
    Thinking(String),
    Activity(Activity),
    FileCard(FileCard),
    Footer(Footer),
}

pub fn tool_kind(name: &str) -> ToolKind {
    match name {
        "Write" => ToolKind::Created,
        "Edit" | "MultiEdit" | "NotebookEdit" => ToolKind::Edited,
        "Read" => ToolKind::Read,
        "Bash" => ToolKind::Command,
        _ => ToolKind::Other,
    }
}

pub(crate) fn file_name(path: &str) -> String {
    path.rsplit('/').next().filter(|name| !name.is_empty()).unwrap_or(path).to_string()
}

fn tool_row(call: &ToolCall) -> ToolRow {
    let kind = tool_kind(&call.name);
    let diff = diff_for_tool_call(call);
    let (added, removed) = diff.as_ref().map_or((0, 0), |diff| (diff.added, diff.removed));
    let has_diff = diff.is_some();
    let summary = tool_input_summary(&call.input);
    let (verb, subject, path) = match kind {
        ToolKind::Created | ToolKind::Edited | ToolKind::Read => {
            let path = ["file_path", "notebook_path"]
                .iter()
                .find_map(|key| call.input.get(*key).and_then(Value::as_str))
                .map(str::to_string)
                .unwrap_or_else(|| summary.clone());
            let verb = match kind {
                ToolKind::Created => "Created",
                ToolKind::Edited => "Edited",
                _ => "Read",
            };
            (verb.to_string(), file_name(&path), path)
        }
        ToolKind::Command => {
            let label = call
                .input
                .get("description")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| summary.clone());
            (String::new(), label, String::new())
        }
        ToolKind::Other => (call.name.clone(), summary.clone(), String::new()),
    };
    ToolRow { call: call.clone(), kind, verb, subject, path, added, removed, has_diff, failed: tool_call_status(call) == ToolCallStatus::Failed }
}

fn summarize(tools: &[&ToolRow], failed: usize) -> String {
    let mut text = if tools.len() == 1 {
        let row = tools[0];
        if row.verb.is_empty() { row.subject.clone() } else { format!("{} {}", row.verb, row.subject) }
    } else {
        let distinct = |kind: ToolKind| {
            let mut seen: Vec<&str> = Vec::new();
            for row in tools.iter().filter(|row| row.kind == kind) {
                if !seen.contains(&row.path.as_str()) {
                    seen.push(&row.path);
                }
            }
            seen.len()
        };
        let commands = tools.iter().filter(|row| row.kind == ToolKind::Command).count();
        let others = tools.iter().filter(|row| row.kind == ToolKind::Other).count();
        let parts = [
            ("Created", distinct(ToolKind::Created), "file"),
            ("Edited", distinct(ToolKind::Edited), "file"),
            ("Ran", commands, "command"),
            ("Read", distinct(ToolKind::Read), "file"),
            ("Used", others, "other tool"),
        ];
        parts
            .iter()
            .filter(|(_, count, _)| *count > 0)
            .map(|(verb, count, noun)| format!("{verb} {count} {noun}{}", if *count == 1 { "" } else { "s" }))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if failed > 0 {
        text.push_str(&format!(" ({failed} failed)"));
    }
    text
}

fn build_activity(blocks: &[Block], tool_rows: &[Option<ToolRow>], view: TranscriptView) -> Activity {
    let mut rows = Vec::new();
    for (block, tool) in blocks.iter().zip(tool_rows) {
        match block {
            Block::Tool(_) => rows.extend(tool.clone().map(Row::Tool)),
            Block::Text(text) if !text.trim().is_empty() => rows.push(Row::Narration(text.clone())),
            Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => rows.push(Row::Thinking(text.clone())),
            _ => {}
        }
    }
    let tools: Vec<&ToolRow> = rows.iter().filter_map(|row| if let Row::Tool(tool) = row { Some(tool) } else { None }).collect();
    let failed = tools.iter().filter(|row| row.failed).count();
    let (added, removed) = tools.iter().fold((0, 0), |(a, r), row| (a + row.added, r + row.removed));
    let summary = summarize(&tools, failed);
    Activity { rows, summary, added, removed, failed }
}

fn build_card(tool_rows: &[Option<ToolRow>]) -> Option<FileCard> {
    let mut files: Vec<CardFile> = Vec::new();
    for row in tool_rows.iter().flatten() {
        if !matches!(row.kind, ToolKind::Created | ToolKind::Edited) || !row.has_diff {
            continue;
        }
        match files.iter_mut().find(|file| file.path == row.path) {
            Some(file) => {
                file.added += row.added;
                file.removed += row.removed;
            }
            None => files.push(CardFile { name: row.subject.clone(), path: row.path.clone(), added: row.added, removed: row.removed }),
        }
    }
    if files.is_empty() {
        return None;
    }
    let (added, removed) = files.iter().fold((0, 0), |(a, r), file| (a + file.added, r + file.removed));
    Some(FileCard { files, added, removed })
}

fn finish_turn(items: &mut Vec<Item>, blocks: Vec<Block>, at: Option<String>, view: TranscriptView, ended: bool) {
    if blocks.is_empty() {
        return;
    }
    let start = items.len();
    // Each tool call's row, and so its diff, is worked out once per build.
    let tool_rows: Vec<Option<ToolRow>> = blocks
        .iter()
        .map(|block| if let Block::Tool(call) = block { Some(tool_row(call)) } else { None })
        .collect();
    let first = blocks.iter().position(|block| matches!(block, Block::Tool(_)));
    let last = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let mut closing: Vec<&str> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if let (Some(first), Some(last)) = (first, last) {
            if index >= first && index <= last {
                if index == first {
                    items.push(Item::Activity(build_activity(&blocks[first..=last], &tool_rows[first..=last], view)));
                }
                continue;
            }
        }
        match block {
            Block::Text(text) if !text.trim().is_empty() => {
                items.push(Item::Text(text.clone()));
                if last.map_or(true, |last| index > last) {
                    closing.push(text);
                }
            }
            Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => items.push(Item::Thinking(text.clone())),
            _ => {}
        }
    }
    if items.len() == start {
        return;
    }
    if ended {
        if let Some(card) = build_card(&tool_rows) {
            items.push(Item::FileCard(card));
        }
        // A turn that ends in a tool call has no text after it: copy the last text it has.
        let text = if closing.is_empty() {
            blocks.iter().rev().find_map(|block| match block {
                Block::Text(text) if !text.trim().is_empty() => Some(text.clone()),
                _ => None,
            }).unwrap_or_default()
        } else {
            closing.join("\n\n")
        };
        items.push(Item::Footer(Footer { text, at }));
    }
}

/// The conversation as things to draw. A turn runs from a user message to the next one; the
/// assistant messages in between are read as one run of blocks. `running` is whether a turn is in
/// progress now: the last turn then has no file card and no footer.
pub fn build_turns(messages: &[Message], view: TranscriptView, running: bool) -> Vec<Item> {
    let mut items = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut at: Option<String> = None;
    for message in messages {
        match message.role {
            MessageRole::User => {
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, true);
                items.push(Item::User(message.clone()));
            }
            MessageRole::Assistant => {
                blocks.extend(message.blocks.iter().cloned());
                if message.at.is_some() {
                    at = message.at.clone();
                }
            }
        }
    }
    finish_turn(&mut items, blocks, at, view, !running);
    items
}

#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;
    use crate::diff::diff_for_tool;

    fn tool(id: &str, name: &str, input: serde_json::Value, failed: Option<bool>) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            input,
            result: failed.map(|is_error| ToolResult { output: "out".to_string(), is_error }),
        })
    }
    fn write(id: &str, path: &str) -> Block {
        tool(id, "Write", json!({"file_path": path, "content": "a\nb\nc\n"}), Some(false))
    }
    fn edit(id: &str, path: &str) -> Block {
        tool(id, "Edit", json!({"file_path": path, "old_string": "x", "new_string": "y"}), Some(false))
    }
    fn bash(id: &str, description: Option<&str>) -> Block {
        let input = match description {
            Some(d) => json!({"command": "ls -la", "description": d}),
            None => json!({"command": "ls -la"}),
        };
        tool(id, "Bash", input, Some(false))
    }
    fn assistant(blocks: Vec<Block>) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = blocks;
        message
    }
    fn text(t: &str) -> Block {
        Block::Text(t.to_string())
    }
    fn activity(items: &[Item]) -> &Activity {
        items.iter().find_map(|i| if let Item::Activity(a) = i { Some(a) } else { None }).expect("an activity")
    }
    fn kinds(items: &[Item]) -> Vec<&'static str> {
        items
            .iter()
            .map(|item| match item {
                Item::User(_) => "user",
                Item::Text(_) => "text",
                Item::Thinking(_) => "thinking",
                Item::Activity(_) => "activity",
                Item::FileCard(_) => "card",
                Item::Footer(_) => "footer",
            })
            .collect()
    }

    #[test]
    fn a_turn_without_tools_is_user_text_footer() {
        let messages = vec![Message::user("hi"), assistant(vec![text("hello")])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "text", "footer"]);
        assert_eq!(items[1], Item::Text("hello".to_string()));
        assert_eq!(items[2], Item::Footer(Footer { text: "hello".to_string(), at: None }));
    }

    #[test]
    fn one_tool_gives_a_one_step_summary_a_card_and_a_footer() {
        let input = json!({"file_path": "/p/hello.txt", "content": "a\nb\nc\n"});
        let messages = vec![
            Message::user("make it"),
            assistant(vec![write("t-1", "/p/hello.txt"), text("Done.")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "activity", "text", "card", "footer"]);
        let a = activity(&items);
        assert_eq!(a.summary, "Created hello.txt");
        let expected = diff_for_tool("Write", &input).unwrap();
        assert_eq!((a.added, a.removed), (expected.added, 0));
        assert!(a.added > 0);
        let Item::FileCard(card) = &items[3] else { panic!("card") };
        assert_eq!(card.files.len(), 1);
        assert_eq!(card.files[0].name, "hello.txt");
        assert_eq!(card.files[0].path, "/p/hello.txt");
    }

    #[test]
    fn text_before_and_after_the_tools_stays_plain_and_text_between_is_narration() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                text("plan"),
                bash("t-1", Some("Run it")),
                text("step done"),
                bash("t-2", Some("Run it again")),
                text("all done"),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "text", "activity", "text", "footer"]);
        assert_eq!(items[1], Item::Text("plan".to_string()));
        assert_eq!(items[3], Item::Text("all done".to_string()));
        let rows = &activity(&items).rows;
        assert_eq!(rows.len(), 3);
        assert!(matches!(&rows[1], Row::Narration(t) if t == "step done"));
        assert_eq!(items[4], Item::Footer(Footer { text: "all done".to_string(), at: None }));
    }

    #[test]
    fn a_reply_that_starts_with_a_tool_has_the_activity_first_and_no_empty_text() {
        let messages = vec![Message::user("go"), assistant(vec![bash("t-1", None), text("")])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user", "activity", "footer"]);
        assert_eq!(activity(&items).summary, "ls -la");
    }

    #[test]
    fn several_tools_are_counted_by_kind() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                write("t-1", "/p/fizz.py"),
                bash("t-2", Some("Run fizz.py")),
                edit("t-3", "/p/fizz.py"),
                write("t-4", "/p/README.md"),
                bash("t-5", Some("Re-run fizz.py")),
                tool("t-6", "Read", json!({"file_path": "/p/fizz.py"}), Some(false)),
                tool("t-7", "Grep", json!({"pattern": "x"}), Some(false)),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.summary, "Created 2 files, Edited 1 file, Ran 2 commands, Read 1 file, Used 1 other tool");
        assert_eq!(a.failed, 0);
        let Item::FileCard(card) = items.iter().find(|i| matches!(i, Item::FileCard(_))).unwrap() else { panic!() };
        let names: Vec<_> = card.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["fizz.py", "README.md"]);
    }

    #[test]
    fn a_file_edited_twice_counts_once_but_both_edits_are_rows_and_the_card_sums_them() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![edit("t-1", "/p/a.py"), edit("t-2", "/p/a.py")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.summary, "Edited 1 file");
        assert_eq!(a.rows.len(), 2);
        assert_eq!((a.added, a.removed), (2, 2));
        let Item::FileCard(card) = items.iter().find(|i| matches!(i, Item::FileCard(_))).unwrap() else { panic!() };
        assert_eq!(card.files.len(), 1);
        assert_eq!((card.files[0].added, card.files[0].removed), (2, 2));
        assert_eq!((card.added, card.removed), (2, 2));
    }

    #[test]
    fn a_failed_tool_is_marked_and_counted() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![bash("t-1", Some("Run it")), tool("t-2", "Bash", json!({"command": "false"}), Some(true))]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let a = activity(&items);
        assert_eq!(a.failed, 1);
        assert_eq!(a.summary, "Ran 2 commands (1 failed)");
        let Row::Tool(row) = &a.rows[1] else { panic!() };
        assert!(row.failed);
    }

    #[test]
    fn thinking_shows_only_in_thinking_and_detailed_views() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                Block::Thinking("before".to_string()),
                text("lead"),
                bash("t-1", None),
                Block::Thinking("inside".to_string()),
                bash("t-2", None),
                Block::Thinking("after".to_string()),
                text("end"),
            ]),
        ];
        let normal = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&normal), vec!["user", "text", "activity", "text", "footer"]);
        assert_eq!(activity(&normal).rows.len(), 2);
        let thinking = build_turns(&messages, TranscriptView::Thinking, false);
        assert_eq!(kinds(&thinking), vec!["user", "thinking", "text", "activity", "thinking", "text", "footer"]);
        assert_eq!(activity(&thinking).rows.len(), 3);
        assert!(matches!(&activity(&thinking).rows[1], Row::Thinking(t) if t == "inside"));
    }

    #[test]
    fn a_message_of_only_thinking_draws_nothing_in_normal_view() {
        let messages = vec![Message::user("go"), assistant(vec![Block::Thinking("hmm".to_string())])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["user"]);
        let shown = build_turns(&messages, TranscriptView::Thinking, false);
        assert_eq!(kinds(&shown), vec!["user", "thinking", "footer"]);
    }

    #[test]
    fn a_running_turn_has_no_card_and_no_footer_and_a_pending_row_is_not_failed() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![write("t-1", "/p/a.py"), tool("t-2", "Bash", json!({"command": "ls"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, true);
        assert_eq!(kinds(&items), vec!["user", "activity"]);
        let Row::Tool(pending) = &activity(&items).rows[1] else { panic!() };
        assert!(!pending.failed);
        let done = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&done), vec!["user", "activity", "card", "footer"]);
    }

    #[test]
    fn an_earlier_turn_has_its_footer_even_while_a_later_turn_runs() {
        let messages = vec![
            Message::user("one"),
            assistant(vec![text("a")]),
            Message::user("two"),
            assistant(vec![text("b")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, true);
        assert_eq!(kinds(&items), vec!["user", "text", "footer", "user", "text"]);
    }

    #[test]
    fn a_session_that_starts_with_an_assistant_message_builds() {
        let messages = vec![assistant(vec![text("welcome back")]), Message::user("hi")];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        assert_eq!(kinds(&items), vec!["text", "footer", "user"]);
    }

    #[test]
    fn rows_without_a_path_a_diff_or_a_description_still_get_a_label() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![
                tool("t-1", "Write", json!({"content": "x"}), Some(false)),
                tool("t-2", "NotebookEdit", json!({"notebook_path": "/p/n.ipynb", "new_source": "x"}), Some(false)),
                bash("t-3", None),
            ]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let rows = &activity(&items).rows;
        let Row::Tool(write_row) = &rows[0] else { panic!() };
        assert_eq!(write_row.verb, "Created");
        let Row::Tool(notebook) = &rows[1] else { panic!() };
        assert_eq!((notebook.verb.as_str(), notebook.subject.as_str()), ("Edited", "n.ipynb"));
        let Row::Tool(command) = &rows[2] else { panic!() };
        assert_eq!((command.verb.as_str(), command.subject.as_str()), ("", "ls -la"));
        assert!(!items.iter().any(|i| matches!(i, Item::FileCard(_))), "no diff, no card entry");
    }

    #[test]
    fn the_footer_time_is_the_time_of_the_last_assistant_message() {
        let mut first = assistant(vec![text("a")]);
        first.at = Some("2026-10-04T01:00:00Z".to_string());
        let mut second = assistant(vec![text("b")]);
        second.at = Some("2026-10-04T01:00:09Z".to_string());
        let items = build_turns(&[Message::user("go"), first, second], TranscriptView::Normal, false);
        let Item::Footer(footer) = items.last().unwrap() else { panic!() };
        assert_eq!(footer.at.as_deref(), Some("2026-10-04T01:00:09Z"));
        assert_eq!(footer.text, "a\n\nb");
    }

    #[test]
    fn the_footer_copies_the_last_text_even_when_a_tool_call_comes_after_it() {
        let messages = vec![Message::user("go"), assistant(vec![text("what I did"), bash("t-1", Some("Run it"))])];
        let items = build_turns(&messages, TranscriptView::Normal, false);
        let Item::Footer(footer) = items.last().unwrap() else { panic!("footer") };
        assert_eq!(footer.text, "what I did");
    }

    #[test]
    fn a_tool_row_knows_whether_it_has_a_diff_so_nobody_computes_it_again() {
        let rows_of = |block: Block| {
            let items = build_turns(&[Message::user("go"), assistant(vec![block])], TranscriptView::Normal, false);
            let Row::Tool(row) = activity(&items).rows[0].clone() else { panic!() };
            row
        };
        assert!(rows_of(write("t-1", "/p/a.py")).has_diff);
        assert!(rows_of(edit("t-2", "/p/a.py")).has_diff);
        assert!(!rows_of(bash("t-3", None)).has_diff);
        assert!(!rows_of(tool("t-4", "Write", json!({"content": "x"}), Some(false))).has_diff);
    }

    #[test]
    fn three_hundred_commands_still_summarize() {
        let blocks: Vec<Block> = (0..300).map(|i| bash(&format!("t-{i}"), Some("Run"))).collect();
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, false);
        assert_eq!(activity(&items).summary, "Ran 300 commands");
        assert_eq!(activity(&items).rows.len(), 300);
    }
}
