use std::time::SystemTime;

use forge_domain::{Block, Message, MessageRole, PermissionRequest, ToolCall};
use serde_json::Value;

use crate::diff::{diff_for_tool, diff_for_tool_call};
use crate::question::{answer_pairs, AnswerPair, QUESTION_TOOL};
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
    /// What a failed call said (a denial tells what was rejected); shown in red above the opened row.
    pub error: Option<String>,
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
    Answers(Vec<AnswerPair>),
    Live(LiveLine),
}

/// What a running turn is waiting for, if anything.
#[derive(Debug, Clone, PartialEq)]
pub enum Waiting {
    Permission { tool_name: String, input: Value },
    Question,
}

/// A turn that is running now.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LiveState {
    /// When it was sent; the clock counts from here.
    pub started: Option<SystemTime>,
    pub waiting: Option<Waiting>,
}

/// The live state of a session from what its view holds: the start time and the request on top of
/// the queue.
pub fn live_state(turn_started: Option<SystemTime>, pending: Option<&PermissionRequest>) -> LiveState {
    let waiting = pending.map(|request| {
        if request.tool_name == QUESTION_TOOL {
            Waiting::Question
        } else {
            Waiting::Permission { tool_name: request.tool_name.clone(), input: request.input.clone() }
        }
    });
    LiveState { started: turn_started, waiting }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveKind {
    /// A tool call is running: `label` is the verb or the command's description, `subject` the file.
    Tool,
    /// The model is writing or thinking.
    Thinking,
    /// A permission or a question waits for the person.
    NeedsInput,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveLine {
    pub kind: LiveKind,
    pub label: String,
    pub subject: String,
    /// The pending write or edit's `+a −r` (NeedsInput only).
    pub added: usize,
    pub removed: usize,
    pub started: Option<SystemTime>,
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
    path.rsplit(['/', '\\']).next().filter(|name| !name.is_empty()).unwrap_or(path).to_string()
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
    ToolRow { call: call.clone(), kind, verb, subject, path, added, removed, has_diff, failed: tool_call_status(call) == ToolCallStatus::Failed, error: call.result.as_ref().filter(|result| result.is_error).map(|result| result.output.clone()) }
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

fn live_label(row: &ToolRow) -> (String, String) {
    match row.kind {
        ToolKind::Created => ("Creating".to_string(), row.subject.clone()),
        ToolKind::Edited => ("Editing".to_string(), row.subject.clone()),
        ToolKind::Read => ("Reading".to_string(), row.subject.clone()),
        ToolKind::Command => (row.subject.clone(), String::new()),
        ToolKind::Other if matches!(row.call.name.as_str(), "Grep" | "Glob") => ("Searching".to_string(), row.subject.clone()),
        ToolKind::Other => (format!("Using {}", row.call.name), row.subject.clone()),
    }
}

fn live_line(blocks: &[Block], tool_rows: &[Option<ToolRow>], live: &LiveState) -> LiveLine {
    let started = live.started;
    if let Some(waiting) = &live.waiting {
        let (added, removed) = match waiting {
            Waiting::Permission { tool_name, input } => diff_for_tool(tool_name, input).map_or((0, 0), |diff| (diff.added, diff.removed)),
            Waiting::Question => (0, 0),
        };
        return LiveLine { kind: LiveKind::NeedsInput, label: "Input needed".to_string(), subject: String::new(), added, removed, started };
    }
    if let (Some(Block::Tool(call)), Some(Some(row))) = (blocks.last(), tool_rows.last()) {
        if call.result.is_none() {
            let (label, subject) = live_label(row);
            return LiveLine { kind: LiveKind::Tool, label, subject, added: 0, removed: 0, started };
        }
    }
    LiveLine { kind: LiveKind::Thinking, label: String::new(), subject: String::new(), added: 0, removed: 0, started }
}

fn is_question(block: &Block) -> bool {
    matches!(block, Block::Tool(call) if call.name == QUESTION_TOOL)
}

// The text after the last tool call (a question counts as one); when there is none, the last text
// the turn has, so a turn that ends in a tool call still has something to copy.
fn closing_text(blocks: &[Block]) -> String {
    let last_tool = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let texts = |from: usize| -> Vec<&str> {
        blocks
            .iter()
            .enumerate()
            .filter(|(index, _)| *index >= from)
            .filter_map(|(_, block)| match block {
                Block::Text(text) if !text.trim().is_empty() => Some(text.as_str()),
                _ => None,
            })
            .collect()
    };
    let after = texts(last_tool.map_or(0, |last| last + 1));
    if !after.is_empty() {
        return after.join("\n\n");
    }
    texts(0).last().map(|text| text.to_string()).unwrap_or_default()
}

// One stretch of a turn without a question in it. A finished stretch folds its tool calls into one
// activity; the running one shows its text before the first tool call and one live line.
fn push_segment(items: &mut Vec<Item>, blocks: &[Block], tool_rows: &[Option<ToolRow>], view: TranscriptView, live: Option<&LiveState>) {
    let first = blocks.iter().position(|block| matches!(block, Block::Tool(_)));
    let last = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let plain = |items: &mut Vec<Item>, block: &Block| match block {
        Block::Text(text) if !text.trim().is_empty() => items.push(Item::Text(text.clone())),
        Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => items.push(Item::Thinking(text.clone())),
        _ => {}
    };
    if let Some(live) = live {
        for block in &blocks[..first.unwrap_or(blocks.len())] {
            plain(items, block);
        }
        items.push(Item::Live(live_line(blocks, tool_rows, live)));
        return;
    }
    for (index, block) in blocks.iter().enumerate() {
        if let (Some(first), Some(last)) = (first, last) {
            if index >= first && index <= last {
                if index == first {
                    items.push(Item::Activity(build_activity(&blocks[first..=last], &tool_rows[first..=last], view)));
                }
                continue;
            }
        }
        plain(items, block);
    }
}

fn finish_turn(items: &mut Vec<Item>, blocks: Vec<Block>, at: Option<String>, view: TranscriptView, live: Option<&LiveState>) {
    if blocks.is_empty() && live.is_none() {
        return;
    }
    let start = items.len();
    // Each tool call's row, and so its diff, is worked out once per build.
    let tool_rows: Vec<Option<ToolRow>> = blocks
        .iter()
        .map(|block| if let Block::Tool(call) = block { Some(tool_row(call)) } else { None })
        .collect();
    // A question splits the turn: what came before it folds, the question becomes its answer card.
    let mut segment_start = 0;
    for (index, block) in blocks.iter().enumerate() {
        if !is_question(block) {
            continue;
        }
        push_segment(items, &blocks[segment_start..index], &tool_rows[segment_start..index], view, None);
        if let Block::Tool(call) = block {
            if let Some(result) = &call.result {
                let pairs = answer_pairs(&call.input, &result.output);
                if !pairs.is_empty() {
                    items.push(Item::Answers(pairs));
                }
            }
        }
        segment_start = index + 1;
    }
    push_segment(items, &blocks[segment_start..], &tool_rows[segment_start..], view, live);
    if live.is_none() && items.len() > start {
        if let Some(card) = build_card(&tool_rows) {
            items.push(Item::FileCard(card));
        }
        items.push(Item::Footer(Footer { text: closing_text(&blocks), at }));
    }
}

/// The conversation as things to draw. A turn runs from a user message to the next one; the
/// assistant messages in between are read as one run of blocks. `live` is the state of the turn
/// that is running now, if any: the last turn then shows one live line in place of its folded
/// activity, and has no file card and no footer.
pub fn build_turns(messages: &[Message], view: TranscriptView, live: Option<&LiveState>) -> Vec<Item> {
    let mut items = Vec::new();
    let mut blocks: Vec<Block> = Vec::new();
    let mut at: Option<String> = None;
    for message in messages {
        match message.role {
            MessageRole::User => {
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, None);
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
    finish_turn(&mut items, blocks, at, view, live);
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
                Item::Answers(_) => "answers",
                Item::Live(_) => "live",
            })
            .collect()
    }

    #[test]
    fn a_turn_without_tools_is_user_text_footer() {
        let messages = vec![Message::user("hi"), assistant(vec![text("hello")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let normal = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&normal), vec!["user", "text", "activity", "text", "footer"]);
        assert_eq!(activity(&normal).rows.len(), 2);
        let thinking = build_turns(&messages, TranscriptView::Thinking, None);
        assert_eq!(kinds(&thinking), vec!["user", "thinking", "text", "activity", "thinking", "text", "footer"]);
        assert_eq!(activity(&thinking).rows.len(), 3);
        assert!(matches!(&activity(&thinking).rows[1], Row::Thinking(t) if t == "inside"));
    }

    #[test]
    fn a_message_of_only_thinking_draws_nothing_in_normal_view() {
        let messages = vec![Message::user("go"), assistant(vec![Block::Thinking("hmm".to_string())])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user"]);
        let shown = build_turns(&messages, TranscriptView::Thinking, None);
        assert_eq!(kinds(&shown), vec!["user", "thinking", "footer"]);
    }

    #[test]
    fn an_earlier_turn_has_its_footer_even_while_a_later_turn_runs() {
        let messages = vec![
            Message::user("one"),
            assistant(vec![text("a")]),
            Message::user("two"),
            assistant(vec![text("b")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&LiveState::default()));
        assert_eq!(kinds(&items), vec!["user", "text", "footer", "user", "text", "live"]);
    }

    #[test]
    fn a_session_that_starts_with_an_assistant_message_builds() {
        let messages = vec![assistant(vec![text("welcome back")]), Message::user("hi")];
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&messages, TranscriptView::Normal, None);
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
        let items = build_turns(&[Message::user("go"), first, second], TranscriptView::Normal, None);
        let Item::Footer(footer) = items.last().unwrap() else { panic!() };
        assert_eq!(footer.at.as_deref(), Some("2026-10-04T01:00:09Z"));
        assert_eq!(footer.text, "a\n\nb");
    }

    #[test]
    fn the_footer_copies_the_last_text_even_when_a_tool_call_comes_after_it() {
        let messages = vec![Message::user("go"), assistant(vec![text("what I did"), bash("t-1", Some("Run it"))])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        let Item::Footer(footer) = items.last().unwrap() else { panic!("footer") };
        assert_eq!(footer.text, "what I did");
    }

    #[test]
    fn a_tool_row_knows_whether_it_has_a_diff_so_nobody_computes_it_again() {
        let rows_of = |block: Block| {
            let items = build_turns(&[Message::user("go"), assistant(vec![block])], TranscriptView::Normal, None);
            let Row::Tool(row) = activity(&items).rows[0].clone() else { panic!() };
            row
        };
        assert!(rows_of(write("t-1", "/p/a.py")).has_diff);
        assert!(rows_of(edit("t-2", "/p/a.py")).has_diff);
        assert!(!rows_of(bash("t-3", None)).has_diff);
        assert!(!rows_of(tool("t-4", "Write", json!({"content": "x"}), Some(false))).has_diff);
    }

    #[test]
    fn a_failed_row_carries_its_error_text_so_an_opened_diff_can_show_it() {
        let denied = Block::Tool(ToolCall {
            id: "t-1".to_string(),
            name: "Write".to_string(),
            input: json!({"file_path": "/p/a.py", "content": "x"}),
            result: Some(ToolResult { output: "The user doesn't want to proceed.".to_string(), is_error: true }),
        });
        let items = build_turns(&[Message::user("go"), assistant(vec![denied])], TranscriptView::Normal, None);
        let Row::Tool(row) = &activity(&items).rows[0] else { panic!() };
        assert_eq!(row.error.as_deref(), Some("The user doesn't want to proceed."));
    }

    #[test]
    fn a_row_that_did_not_fail_has_no_error_text() {
        let items = build_turns(&[Message::user("go"), assistant(vec![write("t-1", "/p/a.py"), tool("t-2", "Bash", json!({"command": "ls"}), None)])], TranscriptView::Normal, None);
        for row in &activity(&items).rows {
            let Row::Tool(row) = row else { continue };
            assert_eq!(row.error, None);
        }
    }

    #[test]
    fn three_hundred_commands_still_summarize() {
        let blocks: Vec<Block> = (0..300).map(|i| bash(&format!("t-{i}"), Some("Run"))).collect();
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
        assert_eq!(activity(&items).summary, "Ran 300 commands");
        assert_eq!(activity(&items).rows.len(), 300);
    }

    #[test]
    fn a_running_turn_shows_text_before_the_first_tool_then_one_live_line() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![text("plan"), write("t-1", "/p/a.py"), text("then"), tool("t-2", "Bash", json!({"command": "ls", "description": "List"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&LiveState::default()));
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        let Item::Live(line) = &items[2] else { panic!() };
        assert_eq!(line.kind, LiveKind::Tool);
        assert_eq!(line.label, "List");
        assert_eq!(line.subject, "");
    }

    #[test]
    fn a_turn_that_ended_with_an_unanswered_tool_folds_and_the_row_is_not_failed() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![write("t-1", "/p/a.py"), tool("t-2", "Bash", json!({"command": "ls"}), None)]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "card", "footer"]);
        let Row::Tool(pending) = &activity(&items).rows[1] else { panic!() };
        assert!(!pending.failed);
    }

    fn live_items(blocks: Vec<Block>, live: LiveState) -> Vec<Item> {
        build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, Some(&live))
    }
    fn live_line(items: &[Item]) -> LiveLine {
        let Item::Live(line) = items.last().unwrap() else { panic!("a live line last") };
        line.clone()
    }
    fn unfinished(name: &str, input: serde_json::Value) -> Block {
        tool("t-x", name, input, None)
    }

    #[test]
    fn a_running_turn_with_no_reply_yet_shows_the_user_and_a_thinking_line() {
        let items = build_turns(&[Message::user("go")], TranscriptView::Normal, Some(&LiveState::default()));
        assert_eq!(kinds(&items), vec!["user", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
    }

    #[test]
    fn streamed_text_stays_visible_above_a_thinking_line() {
        let items = live_items(vec![text("partial answ")], LiveState::default());
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
    }

    #[test]
    fn the_live_label_names_the_step_in_the_present_tense() {
        let line = |name: &str, input: serde_json::Value| live_line(&live_items(vec![unfinished(name, input)], LiveState::default()));
        let write = line("Write", json!({"file_path": "/p/hello.txt", "content": "x"}));
        assert_eq!((write.kind, write.label.as_str(), write.subject.as_str()), (LiveKind::Tool, "Creating", "hello.txt"));
        let edit = line("Edit", json!({"file_path": "/p/a.py", "old_string": "a", "new_string": "b"}));
        assert_eq!((edit.label.as_str(), edit.subject.as_str()), ("Editing", "a.py"));
        let read = line("Read", json!({"file_path": "/p/a.py"}));
        assert_eq!((read.label.as_str(), read.subject.as_str()), ("Reading", "a.py"));
        let bash = line("Bash", json!({"command": "ls -la", "description": "List files"}));
        assert_eq!((bash.label.as_str(), bash.subject.as_str()), ("List files", ""));
        let bare = line("Bash", json!({"command": "ls -la"}));
        assert_eq!(bare.label, "ls -la");
        let grep = line("Grep", json!({"pattern": "fn main"}));
        assert_eq!((grep.label.as_str(), grep.subject.as_str()), ("Searching", "fn main"));
        let other = line("WebFetch", json!({"url": "https://a.test"}));
        assert_eq!((other.label.as_str(), other.subject.as_str()), ("Using WebFetch", "https://a.test"));
        let empty = line("mcp__x__go", json!({}));
        assert_eq!((empty.label.as_str(), empty.subject.as_str()), ("Using mcp__x__go", ""));
    }

    #[test]
    fn between_tools_the_line_is_thinking() {
        let items = live_items(vec![write("t-1", "/p/a.py"), text("now the next")], LiveState::default());
        assert_eq!(live_line(&items).kind, LiveKind::Thinking);
        let after_tool = live_items(vec![write("t-1", "/p/a.py")], LiveState::default());
        assert_eq!(live_line(&after_tool).kind, LiveKind::Thinking);
    }

    #[test]
    fn waiting_for_a_permission_shows_needs_input_with_the_pending_changes() {
        let input = json!({"file_path": "/p/a.py", "content": "a\nb\nc\n"});
        let waiting = Waiting::Permission { tool_name: "Write".to_string(), input: input.clone() };
        let items = live_items(vec![unfinished("Write", input)], LiveState { started: None, waiting: Some(waiting) });
        let line = live_line(&items);
        assert_eq!(line.kind, LiveKind::NeedsInput);
        assert_eq!(line.label, "Input needed");
        assert_eq!((line.added, line.removed), (3, 0));
    }

    #[test]
    fn waiting_for_a_command_or_a_question_has_no_changes() {
        let command = Waiting::Permission { tool_name: "Bash".to_string(), input: json!({"command": "ls"}) };
        let items = live_items(vec![], LiveState { started: None, waiting: Some(command) });
        assert_eq!((live_line(&items).added, live_line(&items).removed), (0, 0));
        let question = live_items(vec![], LiveState { started: None, waiting: Some(Waiting::Question) });
        assert_eq!(live_line(&question).kind, LiveKind::NeedsInput);
    }

    #[test]
    fn the_start_time_travels_to_the_live_line() {
        let started = std::time::UNIX_EPOCH + std::time::Duration::from_secs(5);
        let items = live_items(vec![], LiveState { started: Some(started), waiting: None });
        assert_eq!(live_line(&items).started, Some(started));
    }

    #[test]
    fn live_state_follows_the_first_pending_request() {
        let started = std::time::UNIX_EPOCH;
        assert_eq!(live_state(Some(started), None), LiveState { started: Some(started), waiting: None });
        let question = forge_domain::PermissionRequest { id: "q".to_string(), tool_name: "AskUserQuestion".to_string(), input: json!({}) };
        assert_eq!(live_state(None, Some(&question)).waiting, Some(Waiting::Question));
        let write = forge_domain::PermissionRequest { id: "p".to_string(), tool_name: "Write".to_string(), input: json!({"file_path": "/p/a"}) };
        assert_eq!(
            live_state(None, Some(&write)).waiting,
            Some(Waiting::Permission { tool_name: "Write".to_string(), input: json!({"file_path": "/p/a"}) })
        );
    }

    fn ask(id: &str, result: Option<&str>) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: "AskUserQuestion".to_string(),
            input: json!({"questions": [{"question": "Season?", "header": "S", "multiSelect": false, "options": [{"label": "Spring", "description": ""}, {"label": "Summer", "description": ""}]}]}),
            result: result.map(|output| ToolResult { output: output.to_string(), is_error: false }),
        })
    }
    const ANSWERED: &str = "Your questions have been answered: \"Season?\"=\"Summer\". You can now continue with these answers in mind.";

    #[test]
    fn an_answered_question_becomes_an_answer_card_outside_the_fold() {
        let blocks = vec![write("t-1", "/p/a.py"), ask("q-1", Some(ANSWERED)), text("Nice."), bash("t-2", Some("Run"))];
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "answers", "text", "activity", "card", "footer"]);
        let Item::Answers(pairs) = &items[2] else { panic!() };
        assert_eq!(pairs, &vec![AnswerPair { question: "Season?".to_string(), answer: "Summer".to_string() }]);
    }

    #[test]
    fn a_pending_question_draws_nothing_but_the_needs_input_line() {
        let items = live_items(vec![text("Let me ask."), ask("q-1", None)], LiveState { started: None, waiting: Some(Waiting::Question) });
        assert_eq!(kinds(&items), vec!["user", "text", "live"]);
        assert_eq!(live_line(&items).kind, LiveKind::NeedsInput);
    }

    #[test]
    fn a_question_that_was_skipped_leaves_no_card() {
        let skipped = Block::Tool(ToolCall {
            id: "q-2".to_string(),
            name: "AskUserQuestion".to_string(),
            input: json!({"questions": [{"question": "Season?", "options": []}]}),
            result: Some(ToolResult { output: "The user skipped the question.".to_string(), is_error: true }),
        });
        let items = build_turns(&[Message::user("go"), assistant(vec![skipped, text("ok")])], TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "text", "footer"]);
    }

    #[test]
    fn the_footer_copies_the_text_after_the_last_question_or_tool() {
        let items = build_turns(&[Message::user("go"), assistant(vec![text("before"), ask("q-1", Some(ANSWERED)), text("after")])], TranscriptView::Normal, None);
        let Item::Footer(footer) = items.last().unwrap() else { panic!() };
        assert_eq!(footer.text, "after");
    }
}
