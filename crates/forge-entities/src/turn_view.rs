use std::time::SystemTime;

use forge_domain::{Block, Message, MessageRole, PermissionRequest, ToolCall};
use serde_json::Value;

use crate::background::background_ref;
use crate::code_run::is_run_message;
use crate::diff::{diff_for_tool, diff_for_tool_call};
use crate::preview::{preview_kind, PreviewKind};
use crate::question::{answer_pairs, AnswerPair, QUESTION_TOOL};
use crate::generated_ui::{generated_ui_of, is_ui_call, GeneratedUi};
use crate::turn_cards::{chosen_action, end_card_of, plan_of, turn_tool, EndCard, Step, TurnTool};
use crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};
use crate::workflow_progress::workflow_ref;
use crate::workflow_script::workflow_meta;
use forge_domain::TranscriptView;
use forge_shared::{i18n, tr};

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
    /// The call was refused or the turn was stopped while it ran (shown as stopped, not failed).
    pub stopped: bool,
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
    /// The one image the turn read, shown as a small thumbnail at the end of the folded line.
    pub image: Option<String>,
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
    /// A workflow the turn launched: a card while it runs, one line after.
    Workflow(WorkflowItem),
    /// The plan Claude proposed with ExitPlanMode.
    Plan(PlanItem),
    /// The task list Claude keeps with `update_plan`, shown only while the turn runs.
    PlanList(Vec<Step>),
    /// The next-step card Claude fills with `wrap_up`.
    EndCard(EndCardItem),
    /// An interactive page Claude drew with `generate_sandboxed_ui`.
    GeneratedUi(GeneratedUi),
    /// The conversation was compacted here (token counts before and after, when known).
    Compacted { before: Option<u64>, after: Option<u64> },
    /// The person stopped the turn here.
    Stopped,
}

/// The end-of-turn card Claude filled with `wrap_up`, and what became of it.
#[derive(Debug, Clone, PartialEq)]
pub struct EndCardItem {
    pub card: EndCard,
    pub state: EndCardState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndCardState {
    /// The last turn, not answered yet: the buttons work.
    Live,
    /// The next message was one of its buttons: kept in grey with the button's label.
    Spent { chosen: String },
}

/// A plan Claude proposed, as the transcript shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanItem {
    pub text: String,
}

/// The plan an ExitPlanMode call proposed, when it carries a non-empty text.
fn plan_proposal(call: &ToolCall) -> Option<PlanItem> {
    if call.name != "ExitPlanMode" {
        return None;
    }
    let text = call.input.get("plan")?.as_str()?.to_string();
    (!text.is_empty()).then_some(PlanItem { text })
}

/// The calls that have a card of their own and so are neither counted nor listed in the folded line.
fn is_turn_tool(call: &ToolCall) -> bool {
    turn_tool(&call.name).is_some() || is_ui_call(call)
}

/// What follows a turn: nothing (it is the last one) or the next user message.
#[derive(Clone, Copy)]
enum Next<'a> {
    End,
    Message(&'a str),
}

/// The turn's last `wrap_up` call, unless the person stopped the turn after it.
fn last_wrap_up(blocks: &[Block]) -> Option<&ToolCall> {
    let (index, call) = blocks.iter().enumerate().rev().find_map(|(index, block)| match block {
        Block::Tool(call) if turn_tool(&call.name) == Some(TurnTool::WrapUp) => Some((index, call)),
        _ => None,
    })?;
    (!blocks[index..].iter().any(|block| matches!(block, Block::Stopped))).then_some(call)
}

fn end_card_item(blocks: &[Block], next: Next<'_>) -> Option<Item> {
    let card = end_card_of(last_wrap_up(blocks)?)?;
    let state = match next {
        Next::End => EndCardState::Live,
        Next::Message(text) => EndCardState::Spent { chosen: chosen_action(&card, text)?.label.clone() },
    };
    Some(Item::EndCard(EndCardItem { card, state }))
}

/// The card of the last turn when nothing has answered it yet; the composer uses it to give up its
/// grey suggestion and to take `⌘⏎` as the first button.
pub fn live_end_card(messages: &[Message]) -> Option<EndCard> {
    let start = messages.iter().rposition(|message| message.role == MessageRole::User).map_or(0, |index| index + 1);
    let blocks: Vec<Block> = messages[start..].iter().flat_map(|message| message.blocks.iter().cloned()).collect();
    end_card_of(last_wrap_up(&blocks)?)
}

/// The text of the last plan in the transcript.
pub fn latest_plan(items: &[Item]) -> Option<String> {
    items.iter().rev().find_map(|item| if let Item::Plan(plan) = item { Some(plan.text.clone()) } else { None })
}

/// What the chat knows of a workflow from the call that started it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowItem {
    pub task_id: String,
    pub name: String,
}

/// The workflow a Workflow call launched, once its result says so and its script has a name.
fn workflow_launch(call: &ToolCall) -> Option<WorkflowItem> {
    if call.name != "Workflow" {
        return None;
    }
    let reference = workflow_ref(&call.result.as_ref()?.output)?;
    let meta = workflow_meta(call.input.get("script")?.as_str()?)?;
    Some(WorkflowItem { task_id: reference.task_id, name: meta.name })
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
                ToolKind::Created => tr!("Created|tool"),
                ToolKind::Edited => tr!("Edited|tool"),
                _ => tr!("Read|tool"),
            };
            (verb.to_string(), file_name(&path), path)
        }
        ToolKind::Command if is_background_start(call) => (tr!("Background task").to_string(), summary.clone(), String::new()),
        ToolKind::Command => {
            let label = call
                .input
                .get("description")
                .and_then(Value::as_str)
                .filter(|text| !text.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| summary.clone());
            let verb = if call.result.is_some() { tr!("Ran|tool") } else { tr!("Running|tool") };
            (verb.to_string(), label, String::new())
        }
        ToolKind::Other if call.name == SKILL_TOOL => {
            let name = call.input.get("skill").and_then(Value::as_str).unwrap_or_default();
            (tr!("Skill ran|tool").to_string(), format!("/{name}"), String::new())
        }
        ToolKind::Other => (call.name.clone(), summary.clone(), String::new()),
    };
    let error = call.result.as_ref().filter(|result| result.is_error).map(|result| result.output.clone());
    let stopped = error.as_deref().is_some_and(forge_domain::is_denial_text);
    ToolRow { call: call.clone(), kind, verb, subject, path, added, removed, has_diff, failed: tool_call_status(call) == ToolCallStatus::Failed && !stopped, stopped, error }
}

const SKILL_TOOL: &str = "Skill";
const TOOL_SEARCH: &str = "ToolSearch";

/// A Bash call whose result says the command was started in the background.
fn is_background_start(call: &ToolCall) -> bool {
    call.result.as_ref().is_some_and(|result| background_ref(&result.output).is_some())
}

/// Whether a path is one of Claude's memory files (`~/.claude/projects/<project>/memory/…`).
pub(crate) fn is_memory_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path.contains("/.claude/projects/") && path.contains("/memory/")
}

/// How the folded line counts a call: a memory read and a started background command are not
/// counted as a file read and a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Counted {
    Created,
    Edited,
    Command,
    Read,
    Memory,
    Background,
    Skill,
    LoadedTools,
    Other,
}

fn counted(row: &ToolRow) -> Counted {
    match row.kind {
        ToolKind::Created => Counted::Created,
        ToolKind::Edited => Counted::Edited,
        ToolKind::Read if is_memory_path(&row.path) => Counted::Memory,
        ToolKind::Read => Counted::Read,
        ToolKind::Command if is_background_start(&row.call) => Counted::Background,
        ToolKind::Command => Counted::Command,
        ToolKind::Other if row.call.name == SKILL_TOOL => Counted::Skill,
        ToolKind::Other if row.call.name == TOOL_SEARCH => Counted::LoadedTools,
        ToolKind::Other => Counted::Other,
    }
}

fn summarize(tools: &[&ToolRow], failed: usize) -> String {
    let single = match tools {
        [row] if !matches!(counted(row), Counted::Memory | Counted::Background | Counted::LoadedTools) => Some(*row),
        _ => None,
    };
    let mut text = if let Some(row) = single {
        if row.verb.is_empty() { row.subject.clone() } else { format!("{} {}", row.verb, row.subject) }
    } else {
        let distinct = |kind: Counted| {
            let mut seen: Vec<&str> = Vec::new();
            for row in tools.iter().filter(|row| counted(row) == kind) {
                if !seen.contains(&row.path.as_str()) {
                    seen.push(&row.path);
                }
            }
            seen.len()
        };
        let count = |kind: Counted| tools.iter().filter(|row| counted(row) == kind).count();
        let (commands, background, skills, others) = (count(Counted::Command), count(Counted::Background), count(Counted::Skill), count(Counted::Other));
        let (created, edited, read, memory) = (distinct(Counted::Created), distinct(Counted::Edited), distinct(Counted::Read), distinct(Counted::Memory));
        let loaded_tools = count(Counted::LoadedTools);
        // One sentence per count, so a language with other plural rules can word each one.
        let parts = [
            (created, if created == 1 { tr!("Created {count} file") } else { tr!("Created {count} files") }),
            (edited, if edited == 1 { tr!("Edited {count} file") } else { tr!("Edited {count} files") }),
            (commands, if commands == 1 { tr!("Ran {count} command") } else { tr!("Ran {count} commands") }),
            (background, if background == 1 { tr!("Started {count} background task") } else { tr!("Started {count} background tasks") }),
            (read, if read == 1 { tr!("Read {count} file") } else { tr!("Read {count} files") }),
            (memory, if memory == 1 { tr!("Loaded {count} memory file") } else { tr!("Loaded {count} memory files") }),
            (skills, if skills == 1 { tr!("Ran {count} skill") } else { tr!("Ran {count} skills") }),
            (loaded_tools, tr!("Loaded tools")),
            (others, if others == 1 { tr!("Used {count} other tool") } else { tr!("Used {count} other tools") }),
        ];
        parts
            .iter()
            .filter(|(count, _)| *count > 0)
            .map(|(count, template)| i18n::fill(template, &[("count", count.to_string())]))
            .collect::<Vec<_>>()
            .join(", ")
    };
    if failed > 0 {
        text = tr!("{text} ({failed} failed)", text = text, failed = failed);
    }
    text
}

/// The path of the image the calls read, when they read exactly one image (even several times).
fn read_image(tools: &[&ToolRow]) -> Option<String> {
    let mut images = tools.iter().filter(|row| row.kind == ToolKind::Read && preview_kind(&row.path) == PreviewKind::Image).map(|row| row.path.as_str());
    let first = images.next()?;
    images.all(|path| path == first).then(|| first.to_string())
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
    let image = read_image(&tools);
    Activity { rows, summary, added, removed, failed, image }
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
        ToolKind::Created => (tr!("Creating").to_string(), row.subject.clone()),
        ToolKind::Edited => (tr!("Editing").to_string(), row.subject.clone()),
        ToolKind::Read => (tr!("Reading").to_string(), row.subject.clone()),
        ToolKind::Command => (row.subject.clone(), String::new()),
        ToolKind::Other if matches!(row.call.name.as_str(), "Grep" | "Glob") => (tr!("Searching").to_string(), row.subject.clone()),
        ToolKind::Other => (tr!("Using {name}", name = row.call.name), row.subject.clone()),
    }
}

fn live_line(blocks: &[Block], tool_rows: &[Option<ToolRow>], live: &LiveState) -> LiveLine {
    let started = live.started;
    if let Some(waiting) = &live.waiting {
        let (added, removed) = match waiting {
            Waiting::Permission { tool_name, input } => diff_for_tool(tool_name, input).map_or((0, 0), |diff| (diff.added, diff.removed)),
            Waiting::Question => (0, 0),
        };
        return LiveLine { kind: LiveKind::NeedsInput, label: tr!("Input needed").to_string(), subject: String::new(), added, removed, started };
    }
    // The turn's own tools (task list, next-step card) never name what is running.
    let last = blocks.iter().rposition(|block| !matches!(block, Block::Tool(call) if is_turn_tool(call)));
    if let (Some(Block::Tool(call)), Some(Some(row))) = (last.map(|index| &blocks[index]), last.and_then(|index| tool_rows.get(index))) {
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
fn push_segment(items: &mut Vec<Item>, blocks: &[Block], tool_rows: &[Option<ToolRow>], view: TranscriptView, live: Option<&LiveState>, steps: Option<Vec<Step>>) {
    let first = blocks.iter().position(|block| matches!(block, Block::Tool(_)));
    let last = blocks.iter().rposition(|block| matches!(block, Block::Tool(_)));
    let plain = |items: &mut Vec<Item>, block: &Block| match block {
        Block::Text(text) if !text.trim().is_empty() => items.push(Item::Text(text.clone())),
        Block::Thinking(text) if view.shows_thinking() && !text.trim().is_empty() => items.push(Item::Thinking(text.clone())),
        Block::Compacted { before, after } => items.push(Item::Compacted { before: *before, after: *after }),
        Block::Stopped => items.push(Item::Stopped),
        _ => {}
    };
    if let Some(live) = live {
        for block in &blocks[..first.unwrap_or(blocks.len())] {
            plain(items, block);
        }
        for block in blocks {
            if let Block::Tool(call) = block {
                items.extend(generated_ui_of(call).map(Item::GeneratedUi));
            }
        }
        if let Some(steps) = steps {
            items.push(Item::PlanList(steps));
        }
        items.push(Item::Live(live_line(blocks, tool_rows, live)));
        return;
    }
    for (index, block) in blocks.iter().enumerate() {
        if let (Some(first), Some(last)) = (first, last) {
            if index >= first && index <= last {
                if index == first {
                    // A launched workflow has its own card; the rest of the stretch folds as before.
                    let (kept, rows): (Vec<Block>, Vec<Option<ToolRow>>) = blocks[first..=last]
                        .iter()
                        .zip(&tool_rows[first..=last])
                        .filter(|(block, _)| !matches!(block, Block::Tool(call) if workflow_launch(call).is_some() || plan_proposal(call).is_some() || is_turn_tool(call)))
                        .map(|(block, row)| (block.clone(), row.clone()))
                        .unzip();
                    if kept.iter().any(|block| matches!(block, Block::Tool(_))) {
                        items.push(Item::Activity(build_activity(&kept, &rows, view)));
                    } else {
                        for block in &kept {
                            plain(items, block);
                        }
                    }
                    for block in &blocks[first..=last] {
                        if let Block::Tool(call) = block {
                            items.extend(workflow_launch(call).map(Item::Workflow));
                            items.extend(plan_proposal(call).map(Item::Plan));
                            items.extend(generated_ui_of(call).map(Item::GeneratedUi));
                        }
                    }
                }
                continue;
            }
        }
        plain(items, block);
    }
}

fn finish_turn(items: &mut Vec<Item>, blocks: Vec<Block>, at: Option<String>, view: TranscriptView, live: Option<&LiveState>, next: Next<'_>) {
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
        push_segment(items, &blocks[segment_start..index], &tool_rows[segment_start..index], view, None, None);
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
    // The task list is the whole turn's, even when a question split it into segments.
    let steps = live.and_then(|_| plan_of(blocks.iter().filter_map(|block| if let Block::Tool(call) = block { Some(call) } else { None })));
    push_segment(items, &blocks[segment_start..], &tool_rows[segment_start..], view, live, steps);
    // A turn that is only a compaction or a stop has nothing to copy.
    let has_content = blocks.iter().any(|block| matches!(block, Block::Text(_) | Block::Tool(_) | Block::Thinking(_)));
    if live.is_none() && items.len() > start && has_content {
        if let Some(card) = build_card(&tool_rows) {
            items.push(Item::FileCard(card));
        }
        items.extend(end_card_item(&blocks, next));
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
                let text = message.text();
                finish_turn(&mut items, std::mem::take(&mut blocks), at.take(), view, None, Next::Message(&text));
                // What a run of a code block told the model is not something the person said.
                if !is_run_message(&text) {
                    items.push(Item::User(message.clone()));
                }
            }
            MessageRole::Assistant => {
                blocks.extend(message.blocks.iter().cloned());
                if message.at.is_some() {
                    at = message.at.clone();
                }
            }
        }
    }
    finish_turn(&mut items, blocks, at, view, live, Next::End);
    // A running `/compact` has no tool call to name: the line says what it is doing.
    let compacting = messages.iter().rev().find(|message| message.role == MessageRole::User).is_some_and(|message| is_compact_command(&message.text()));
    if live.is_some() && compacting {
        if let Some(Item::Live(line)) = items.last_mut() {
            if line.kind == LiveKind::Thinking && line.label.is_empty() {
                line.label = tr!("Compacting the session").to_string();
            }
        }
    }
    items
}

fn is_compact_command(text: &str) -> bool {
    let text = text.trim();
    text == "/compact" || text.starts_with("/compact ")
}

#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall, ToolResult};
    use serde_json::json;

    use super::*;
    use crate::diff::diff_for_tool;
    use crate::turn_cards::StepState;

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
                Item::Workflow(_) => "workflow",
                Item::Plan(_) => "plan",
                Item::PlanList(_) => "planlist",
                Item::EndCard(_) => "endcard",
                Item::GeneratedUi(_) => "generatedui",
                Item::Compacted { .. } => "compacted",
                Item::Stopped => "stopped",
            })
            .collect()
    }

    fn update_plan(id: &str, input: serde_json::Value, done: bool) -> Block {
        tool(id, "mcp__ccd_turn__update_plan", input, done.then_some(false))
    }
    fn wrap_up(id: &str, ask: Option<&str>, actions: serde_json::Value) -> Block {
        let mut input = json!({"recap": "Built it.", "actions": actions});
        if let Some(ask) = ask {
            input["ask"] = json!(ask);
        }
        tool(id, "mcp__ccd_turn__wrap_up", input, Some(false))
    }
    fn two_steps() -> serde_json::Value {
        json!({"items": [{"content": "Look", "status": "in_progress"}, {"content": "Fix", "status": "pending"}]})
    }
    fn running() -> LiveState {
        LiveState::default()
    }

    #[test]
    fn the_task_list_shows_while_the_turn_runs_above_the_live_line_and_never_after() {
        let messages = vec![Message::user("go"), assistant(vec![text("Looking."), update_plan("p", two_steps(), false)])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert_eq!(kinds(&items), vec!["user", "text", "planlist", "live"]);
        let Item::PlanList(steps) = &items[2] else { panic!() };
        assert_eq!(steps.len(), 2);
        let finished = build_turns(&messages, TranscriptView::Normal, None);
        assert!(!kinds(&finished).contains(&"planlist"));
    }

    #[test]
    fn the_two_tools_are_not_rows_and_not_counted_in_the_folded_line() {
        let messages = vec![
            Message::user("go"),
            assistant(vec![update_plan("p", two_steps(), true), bash("b", None), wrap_up("w", Some("Commit?"), json!([{"label": "Commit"}]))]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        let a = activity(&items);
        assert_eq!(a.rows.len(), 1);
        assert_eq!(a.summary, "Ran ls -la");
    }

    fn ui_page(id: &str, done: bool) -> Block {
        tool(id, "mcp__forge_ui__generate_sandboxed_ui", json!({"html": "<p>hi</p>"}), done.then_some(false))
    }

    #[test]
    fn a_generated_page_gets_its_own_card_and_is_not_counted_in_the_folded_line() {
        let messages = vec![Message::user("go"), assistant(vec![bash("b", None), ui_page("u", true), text("Here it is.")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "generatedui", "text", "footer"]);
        assert_eq!(activity(&items).rows.len(), 1);
        let Item::GeneratedUi(ui) = &items[2] else { panic!() };
        assert_eq!(ui.id, "u");
        let only = vec![Message::user("go"), assistant(vec![ui_page("u", true), text("Done.")])];
        assert!(!kinds(&build_turns(&only, TranscriptView::Normal, None)).contains(&"activity"));
    }

    #[test]
    fn a_page_shows_while_the_turn_runs_and_is_not_the_live_label() {
        let messages = vec![Message::user("go"), assistant(vec![text("Sure."), ui_page("u", false)])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert_eq!(kinds(&items), vec!["user", "text", "generatedui", "live"]);
        let Some(Item::Live(line)) = items.last() else { panic!() };
        assert_eq!(line.kind, LiveKind::Thinking);
    }

    #[test]
    fn a_turn_of_only_the_two_tools_has_no_folded_line() {
        let messages = vec![Message::user("go"), assistant(vec![update_plan("p", two_steps(), true), text("done")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert!(!kinds(&items).contains(&"activity"));
    }

    #[test]
    fn a_running_turn_tool_call_does_not_become_the_live_label() {
        let messages = vec![Message::user("go"), assistant(vec![update_plan("p", two_steps(), false)])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        let Some(Item::Live(line)) = items.last() else { panic!() };
        assert_eq!(line.kind, LiveKind::Thinking, "no raw tool name");
    }

    #[test]
    fn a_finished_last_turn_with_a_wrap_up_question_shows_a_live_end_card_before_the_footer() {
        let messages = vec![Message::user("go"), assistant(vec![bash("b", None), wrap_up("w", Some("Commit?"), json!([{"label": "Commit", "message": "commit it"}])), text("All done.")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "text", "endcard", "footer"]);
        let Item::EndCard(card) = &items[3] else { panic!() };
        assert_eq!(card.state, EndCardState::Live);
        assert_eq!(card.card.ask, "Commit?");
        assert_eq!(live_end_card(&messages).map(|c| c.ask), Some("Commit?".to_string()));
        let running_items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert!(!kinds(&running_items).contains(&"endcard"), "not while the turn runs");
    }

    #[test]
    fn no_question_no_card_and_only_the_last_wrap_up_counts() {
        let none = vec![Message::user("go"), assistant(vec![wrap_up("w", None, json!([])), text("x")])];
        assert!(!kinds(&build_turns(&none, TranscriptView::Normal, None)).contains(&"endcard"));
        assert_eq!(live_end_card(&none), None);
        let twice = vec![Message::user("go"), assistant(vec![wrap_up("a", Some("First?"), json!([])), wrap_up("b", Some("Second?"), json!([])), text("x")])];
        let items = build_turns(&twice, TranscriptView::Normal, None);
        assert_eq!(items.iter().filter(|i| matches!(i, Item::EndCard(_))).count(), 1);
        let Some(Item::EndCard(card)) = items.iter().find(|i| matches!(i, Item::EndCard(_))) else { panic!() };
        assert_eq!(card.card.ask, "Second?");
    }

    #[test]
    fn a_reply_that_equals_a_button_leaves_a_spent_card_and_any_other_message_removes_it() {
        let first = vec![Message::user("go"), assistant(vec![wrap_up("w", Some("Commit?"), json!([{"label": "Commit", "message": "commit it"}])), text("x")])];
        let mut chosen = first.clone();
        chosen.push(Message::user("commit it"));
        chosen.push(assistant(vec![text("Committed.")]));
        let items = build_turns(&chosen, TranscriptView::Normal, None);
        let Some(Item::EndCard(card)) = items.iter().find(|i| matches!(i, Item::EndCard(_))) else { panic!("spent card kept") };
        assert_eq!(card.state, EndCardState::Spent { chosen: "Commit".to_string() });
        assert_eq!(live_end_card(&chosen), None, "no live card once the turn after it exists");
        let mut other = first.clone();
        other.push(Message::user("something else"));
        other.push(assistant(vec![text("ok")]));
        assert!(!kinds(&build_turns(&other, TranscriptView::Normal, None)).contains(&"endcard"));
    }

    #[test]
    fn a_resumed_session_answered_with_other_text_has_no_leftover_card() {
        let mut messages = vec![Message::user("go"), assistant(vec![wrap_up("w", Some("Commit?"), json!([{"label": "Yes"}])), text("x")])];
        messages.push(Message::user("Yes!"));
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        assert!(!kinds(&items).contains(&"endcard"));
    }

    fn plan_call(id: &str, plan: &str) -> Block {
        tool(id, "ExitPlanMode", json!({"plan": plan}), None)
    }

    #[test]
    fn an_exit_plan_mode_call_is_a_plan_item_and_latest_plan_takes_the_last() {
        let messages = vec![Message::user("go"), assistant(vec![plan_call("p", "# P")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert!(items.contains(&Item::Plan(PlanItem { text: "# P".to_string() })));
        assert_eq!(latest_plan(&items), Some("# P".to_string()));
        let messages = vec![Message::user("go"), assistant(vec![plan_call("p", "# P"), text("x"), plan_call("q", "# Q")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(latest_plan(&items), Some("# Q".to_string()));
        assert_eq!(latest_plan(&[]), None);
    }

    #[test]
    fn an_exit_plan_mode_call_without_a_usable_plan_stays_an_activity_row() {
        for input in [json!({}), json!({"plan": ""}), json!({"plan": 5})] {
            let call = tool("p", "ExitPlanMode", input, None);
            let messages = vec![Message::user("go"), assistant(vec![call])];
            let items = build_turns(&messages, TranscriptView::Normal, None);
            assert!(!kinds(&items).contains(&"plan"));
            assert!(kinds(&items).contains(&"activity"));
        }
    }

    const LAUNCHED: &str = "Workflow launched in background. Task ID: wf1\nSummary: s\nTranscript dir: /h/.claude/projects/-p/s/subagents/workflows/wf_abc\n";

    fn workflow(id: &str, result: Option<&str>) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: "Workflow".to_string(),
            input: json!({"script": "export const meta = { name: 'capitals', description: 'd', phases: [{ title: 'Ask' }] }"}),
            result: result.map(|output| ToolResult { output: output.to_string(), is_error: false }),
        })
    }

    #[test]
    fn a_launched_workflow_is_its_own_item_not_a_row_of_the_folded_activity() {
        let messages = vec![Message::user("go"), assistant(vec![workflow("w", Some(LAUNCHED)), text("Running.")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "workflow", "text", "footer"]);
        assert_eq!(items[1], Item::Workflow(WorkflowItem { task_id: "wf1".to_string(), name: "capitals".to_string() }));
    }

    #[test]
    fn other_tools_of_the_turn_still_fold_and_the_workflow_follows_them() {
        let messages = vec![Message::user("go"), assistant(vec![bash("b", Some("List")), workflow("w", Some(LAUNCHED)), text("Running.")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "workflow", "text", "footer"]);
        assert_eq!(activity(&items).rows.len(), 1, "the activity holds the command only");
    }

    #[test]
    fn a_workflow_call_with_no_result_or_no_readable_script_stays_an_ordinary_row() {
        for block in [workflow("w", None), workflow("w", Some("denied"))] {
            let items = build_turns(&[Message::user("go"), assistant(vec![block])], TranscriptView::Normal, None);
            assert_eq!(kinds(&items), vec!["user", "activity", "footer"]);
        }
        let nameless = Block::Tool(ToolCall {
            id: "w".to_string(),
            name: "Workflow".to_string(),
            input: json!({"script": "const x = 1"}),
            result: Some(ToolResult { output: LAUNCHED.to_string(), is_error: false }),
        });
        let items = build_turns(&[Message::user("go"), assistant(vec![nameless])], TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "activity", "footer"]);
    }

    #[test]
    fn a_run_message_is_not_drawn_and_the_answer_after_it_still_shows() {
        let run = crate::code_run::model_message("echo hi", Some(0), "hi\n", false);
        let messages = vec![
            Message::user("hi"),
            assistant(vec![text("a")]),
            Message::user(&run),
            assistant(vec![text("The command printed hi.")]),
        ];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        assert_eq!(kinds(&items), vec!["user", "text", "footer", "text", "footer"]);
        assert!(items.iter().all(|item| !matches!(item, Item::User(message) if message.text().contains("[forge-run]"))));
        assert_eq!(items[3], Item::Text("The command printed hi.".to_string()));
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
        assert_eq!(activity(&items).summary, "Ran ls -la");
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
        assert_eq!((command.verb.as_str(), command.subject.as_str()), ("Ran", "ls -la"));
        assert!(!items.iter().any(|i| matches!(i, Item::FileCard(_))), "no diff, no card entry");
    }

    fn bash_in_background(id: &str) -> Block {
        Block::Tool(ToolCall {
            id: id.to_string(),
            name: "Bash".to_string(),
            input: json!({"command": "sleep 4 && ls", "run_in_background": true}),
            result: Some(ToolResult {
                output: "Command running in background with ID: bk1. Output is being written to: /p/tasks/bk1.output. You will be notified when it completes.".to_string(),
                is_error: false,
            }),
        })
    }
    fn summary_of(blocks: Vec<Block>) -> String {
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
        activity(&items).summary.clone()
    }
    fn read(id: &str, path: &str) -> Block {
        tool(id, "Read", json!({"file_path": path}), Some(false))
    }
    const MEMORY: &str = "/Users/me/.claude/projects/-p/memory/MEMORY.md";

    #[test]
    fn a_memory_read_is_counted_as_memory_not_as_a_file() {
        assert_eq!(summary_of(vec![read("t-1", MEMORY), bash("t-2", None), bash("t-3", None)]), "Ran 2 commands, Loaded 1 memory file");
        assert_eq!(summary_of(vec![read("t-1", MEMORY)]), "Loaded 1 memory file");
        assert_eq!(summary_of(vec![read("t-1", "/p/a.rs"), read("t-2", MEMORY)]), "Read 1 file, Loaded 1 memory file");
    }

    #[test]
    fn a_started_background_command_is_counted_apart_from_commands() {
        assert_eq!(summary_of(vec![bash("t-1", None), bash_in_background("t-2")]), "Ran 1 command, Started 1 background task");
        let items = build_turns(&[Message::user("go"), assistant(vec![bash_in_background("t-1")])], TranscriptView::Normal, None);
        let Row::Tool(row) = &activity(&items).rows[0] else { panic!() };
        assert_eq!((row.verb.as_str(), row.subject.as_str()), ("Background task", "sleep 4 && ls"));
    }

    #[test]
    fn a_skill_shows_its_name_and_tool_search_shows_loaded_tools() {
        assert_eq!(summary_of(vec![tool("t-1", "Skill", json!({"skill": "superpowers:brainstorming"}), Some(false))]), "Skill ran /superpowers:brainstorming");
        let skill = |id| tool(id, "Skill", json!({"skill": "x"}), Some(false));
        assert_eq!(summary_of(vec![skill("t-1"), skill("t-2")]), "Ran 2 skills");
        assert_eq!(summary_of(vec![tool("t-1", "ToolSearch", json!({"query": "select:A"}), Some(false)), bash("t-2", None)]), "Ran 1 command, Loaded tools");
    }

    #[test]
    fn a_command_row_says_whether_it_ran_or_is_running() {
        let messages = vec![Message::user("go"), assistant(vec![bash("t-1", Some("List")), tool("t-2", "Bash", json!({"command": "sleep 9"}), None)])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        let rows = &activity(&items).rows;
        let Row::Tool(done) = &rows[0] else { panic!() };
        let Row::Tool(running) = &rows[1] else { panic!() };
        assert_eq!((done.verb.as_str(), running.verb.as_str()), ("Ran", "Running"));
    }

    #[test]
    fn a_turn_that_read_one_image_names_it_for_a_thumbnail() {
        let image = |blocks| {
            let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
            activity(&items).image.clone()
        };
        assert_eq!(image(vec![read("t-1", "/p/shot.png"), bash("t-2", None)]), Some("/p/shot.png".to_string()));
        assert_eq!(image(vec![read("t-1", "/p/shot.png"), read("t-2", "/p/shot.png")]), Some("/p/shot.png".to_string()));
        assert_eq!(image(vec![read("t-1", "/p/a.png"), read("t-2", "/p/b.png")]), None);
        assert_eq!(image(vec![read("t-1", "/p/a.rs")]), None);
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
    fn a_refused_call_is_stopped_not_failed_and_a_stop_line_has_no_footer() {
        let refused = Block::Tool(ToolCall {
            id: "t-1".to_string(),
            name: "Bash".to_string(),
            input: json!({"command": "sleep 40"}),
            result: Some(ToolResult { output: "The user doesn't want to proceed with this tool use. The tool use was rejected".to_string(), is_error: true }),
        });
        let broke = Block::Tool(ToolCall {
            id: "t-2".to_string(),
            name: "Bash".to_string(),
            input: json!({"command": "false"}),
            result: Some(ToolResult { output: "exit 1".to_string(), is_error: true }),
        });
        let items = build_turns(&[Message::user("go"), assistant(vec![refused, broke, Block::Stopped])], TranscriptView::Normal, None);
        let rows: Vec<_> = activity(&items).rows.iter().filter_map(|row| if let Row::Tool(tool) = row { Some((tool.stopped, tool.failed)) } else { None }).collect();
        assert_eq!(rows, vec![(true, false), (false, true)]);
        assert_eq!(activity(&items).failed, 1, "only the real failure counts");
        assert!(items.iter().any(|item| matches!(item, Item::Stopped)));

        let only_stop = build_turns(&[Message::user("go"), assistant(vec![Block::Stopped])], TranscriptView::Normal, None);
        assert!(!only_stop.iter().any(|item| matches!(item, Item::Footer(_))), "nothing to copy");
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
    fn a_running_compact_says_it_is_compacting() {
        let running = LiveState::default();
        let items = build_turns(&[Message::user("/compact")], TranscriptView::Normal, Some(&running));
        assert_eq!(live_line(&items).label, "Compacting the session");
        let other = build_turns(&[Message::user("hello")], TranscriptView::Normal, Some(&running));
        assert_eq!(live_line(&other).label, "");
        let finished = build_turns(&[Message::user("/compact")], TranscriptView::Normal, None);
        assert!(!finished.iter().any(|item| matches!(item, Item::Live(_))));
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
        let question = forge_domain::PermissionRequest { id: "q".to_string(), tool_name: "AskUserQuestion".to_string(), input: json!({}), rules: Vec::new() };
        assert_eq!(live_state(None, Some(&question)).waiting, Some(Waiting::Question));
        let write = forge_domain::PermissionRequest { id: "p".to_string(), tool_name: "Write".to_string(), input: json!({"file_path": "/p/a"}), rules: Vec::new() };
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

    #[test]
    fn the_task_list_survives_a_question_in_the_same_turn() {
        let advance = update_plan("p2", json!({"advance": true}), true);
        let messages = vec![Message::user("go"), assistant(vec![update_plan("p1", two_steps(), true), ask("q-1", Some(ANSWERED)), advance])];
        let items = build_turns(&messages, TranscriptView::Normal, Some(&running()));
        let steps = items.iter().find_map(|item| if let Item::PlanList(steps) = item { Some(steps.clone()) } else { None }).expect("a task list");
        assert_eq!(steps.iter().map(|s| s.state).collect::<Vec<_>>(), vec![StepState::Done, StepState::InProgress]);
    }

    #[test]
    fn a_question_and_a_wrap_up_in_one_finished_turn_draw_the_card_once_after_the_last_segment() {
        let blocks = vec![bash("b", None), ask("q-1", Some(ANSWERED)), text("Nice."), wrap_up("w", Some("Commit?"), json!([{"label": "Commit"}])), text("Done.")];
        let items = build_turns(&[Message::user("go"), assistant(blocks)], TranscriptView::Normal, None);
        assert_eq!(items.iter().filter(|i| matches!(i, Item::EndCard(_))).count(), 1);
        let kinds = kinds(&items);
        assert_eq!(&kinds[kinds.len() - 2..], ["endcard", "footer"]);
    }

    #[test]
    fn a_turn_tool_after_a_running_tool_leaves_the_tool_as_the_live_label() {
        let running_bash = tool("b", "Bash", json!({"command": "ls", "description": "List files"}), None);
        let items = live_items(vec![running_bash, update_plan("p", two_steps(), false)], running());
        let line = live_line(&items);
        assert_eq!(line.kind, LiveKind::Tool);
        assert_eq!(line.label, "List files");
    }

    #[test]
    fn no_end_card_when_the_turn_was_stopped_after_the_wrap_up() {
        let blocks = vec![wrap_up("w", Some("Commit?"), json!([{"label": "Commit"}])), Block::Stopped];
        let messages = vec![Message::user("go"), assistant(blocks)];
        assert!(!kinds(&build_turns(&messages, TranscriptView::Normal, None)).contains(&"endcard"));
        assert_eq!(live_end_card(&messages), None);
        let before = vec![Message::user("go"), assistant(vec![Block::Stopped, wrap_up("w", Some("Commit?"), json!([{"label": "Commit"}]))])];
        assert!(kinds(&build_turns(&before, TranscriptView::Normal, None)).contains(&"endcard"));
    }

    #[test]
    fn a_wrap_up_without_a_question_is_neither_a_row_nor_counted() {
        let messages = vec![Message::user("go"), assistant(vec![bash("b", None), wrap_up("w", None, json!([])), text("x")])];
        let items = build_turns(&messages, TranscriptView::Normal, None);
        let a = activity(&items);
        assert_eq!(a.rows.len(), 1);
        assert_eq!(a.summary, "Ran ls -la");
    }

    #[test]
    fn a_reply_with_spaces_around_the_message_spends_the_card_but_a_different_case_does_not() {
        let first = vec![Message::user("go"), assistant(vec![wrap_up("w", Some("Commit?"), json!([{"label": "Commit", "message": "commit it"}])), text("x")])];
        let mut spaced = first.clone();
        spaced.push(Message::user("  commit it \n"));
        let items = build_turns(&spaced, TranscriptView::Normal, None);
        let Some(Item::EndCard(card)) = items.iter().find(|i| matches!(i, Item::EndCard(_))) else { panic!("spent card kept") };
        assert_eq!(card.state, EndCardState::Spent { chosen: "Commit".to_string() });
        let mut cased = first.clone();
        cased.push(Message::user("Commit it"));
        assert!(!kinds(&build_turns(&cased, TranscriptView::Normal, None)).contains(&"endcard"));
    }
}
