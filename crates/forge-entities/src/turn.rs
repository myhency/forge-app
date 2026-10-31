use std::time::{Duration, SystemTime};

use dioxus::prelude::*;
use forge_domain::{FinishedTask, Message, TaskStatus};
use time::OffsetDateTime;

use crate::card_view::CardView;
use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
use crate::links::{known_paths, render_markdown_linked};
use crate::attachment_card::{ReadThumbnail, SentAttachment};
use crate::message::ToolDetail;
use crate::question::AnswerPair;
use crate::relative_time::relative_time;
use crate::tool_summary::elapsed_label;
use forge_domain::TranscriptView;
use crate::generated_ui_view::GeneratedUiView;
use crate::turn_card_views::{EndCardView, PlanListView};
use crate::turn_view::{build_turns, Activity, FileCard, Footer, Item, LiveKind, LiveLine, LiveState, Row, ToolRow, WorkflowItem};
use crate::workflow_progress::{dot_class, JournalAgent};
use forge_shared::{tr, Icon, IconKind, IconSize};

fn counts(added: usize, removed: usize) -> Option<(String, String)> {
    (added > 0 || removed > 0).then(|| (format!("+{added}"), format!("−{removed}")))
}

#[component]
fn CountChips(added: usize, removed: usize) -> Element {
    let Some((plus, minus)) = counts(added, removed) else { return rsx! {} };
    rsx! {
        span { class: "count-chips",
            span { class: "count-added", "{plus}" }
            span { class: "count-removed", "{minus}" }
        }
    }
}

/// A workflow that is running now, with the agents its journal lists so far.
#[derive(Debug, Clone, PartialEq)]
pub struct RunningWorkflow {
    pub task_id: String,
    pub seen_at: SystemTime,
    pub agents: Vec<JournalAgent>,
}

/// The conversation: user bubbles, answer text, one folded activity line per turn, a card of the
/// files the turn changed, and a footer under a finished turn.
#[component]
pub fn Turns(
    messages: Vec<Message>,
    view: TranscriptView,
    live: Option<LiveState>,
    on_open_file: EventHandler<String>,
    #[props(default)] workflows: Vec<RunningWorkflow>,
    #[props(default)] finished: Vec<FinishedTask>,
    #[props(default)] on_open_tasks: EventHandler<()>,
    #[props(default)] on_open_plan: EventHandler<()>,
    /// Asked to go back to the user message with this id (its action row's ↺); no ↺ without it.
    #[props(default)] on_rewind: Option<EventHandler<String>>,
    /// Asked to fork the session before the user message with this id (its action row's ⑂); no ⑂ without it.
    #[props(default)] on_fork: Option<EventHandler<String>>,
    /// Asked to send this text as the next message (an end card's button); its buttons do nothing without it.
    #[props(default)] on_send: Option<EventHandler<String>>,
    /// What became of each end card of the session, by the `wrap_up` call's id.
    #[props(default)] card_marks: std::collections::HashMap<String, crate::turn_cards::CardMark>,
    /// Asked to record a mark (closed, or sent) for the end card with this id.
    #[props(default)] on_card_mark: Option<EventHandler<(String, crate::turn_cards::CardMark)>>,
) -> Element {
    let known = known_paths(&messages);
    let forkable: Vec<String> = messages.iter().filter_map(|message| message.uuid.clone()).filter(|uuid| forge_domain::can_fork_before(&messages, uuid)).collect();
    let items = build_turns(&messages, view, live.as_ref());
    let detailed = view.expands_tool_calls();
    rsx! {
        div { class: "turns",
            for (index, item) in items.into_iter().enumerate() {
                TurnItem { key: "{index}", item, detailed, known: known.clone(), on_open_file, workflows: workflows.clone(), finished: finished.clone(), on_open_tasks, on_open_plan, on_rewind, on_fork, on_send, card_marks: card_marks.clone(), on_card_mark, forkable: forkable.clone() }
            }
        }
    }
}

#[component]
fn TurnItem(
    item: Item,
    detailed: bool,
    known: Vec<String>,
    on_open_file: EventHandler<String>,
    workflows: Vec<RunningWorkflow>,
    finished: Vec<FinishedTask>,
    on_open_tasks: EventHandler<()>,
    on_open_plan: EventHandler<()>,
    on_rewind: Option<EventHandler<String>>,
    on_fork: Option<EventHandler<String>>,
    on_send: Option<EventHandler<String>>,
    card_marks: std::collections::HashMap<String, crate::turn_cards::CardMark>,
    on_card_mark: Option<EventHandler<(String, crate::turn_cards::CardMark)>>,
    forkable: Vec<String>,
) -> Element {
    match item {
        Item::User(message) => {
            let text = message.text();
            rsx! {
                div { class: "turn-user",
                    if !message.attachments.is_empty() {
                        div { class: "attach-row",
                            for (index, attachment) in message.attachments.iter().enumerate() {
                                SentAttachment { key: "{index}{attachment.path.display()}", attachment: attachment.clone() }
                            }
                        }
                    }
                    if !message.cards.is_empty() {
                        div { class: "cards-row",
                            for (index, card) in message.cards.iter().enumerate() {
                                CardView { key: "{index}", card: card.clone(), on_click: None, on_remove: None }
                            }
                        }
                    }
                    if !text.is_empty() {
                        div { class: "user-bubble", "{text}" }
                        div { class: "message-actions",
                            if let Some(at) = message.at.as_deref() {
                                {
                                    let time = relative_time(at, OffsetDateTime::now_utc());
                                    rsx! { if !time.is_empty() { span { class: "footer-time", "{time}" } } }
                                }
                            }
                            button {
                                class: "icon-button",
                                r#type: "button",
                                title: tr!("Copy"),
                                "aria-label": tr!("Copy"),
                                onclick: {
                                    let text = text.clone();
                                    move |_| crate::clipboard::copy_text(&text)
                                },
                                Icon { kind: IconKind::Copy, size: IconSize::Small }
                            }
                            if let (Some(on_rewind), Some(uuid)) = (on_rewind, message.uuid.clone()) {
                                button {
                                    class: "icon-button",
                                    r#type: "button",
                                    title: tr!("Go back to an earlier point"),
                                    "aria-label": tr!("Go back to an earlier point"),
                                    onclick: move |_| on_rewind.call(uuid.clone()),
                                    Icon { kind: IconKind::Rewind, size: IconSize::Small }
                                }
                            }
                            if let (Some(on_fork), Some(uuid)) = (on_fork, message.uuid.clone().filter(|uuid| forkable.contains(uuid))) {
                                button {
                                    class: "icon-button",
                                    r#type: "button",
                                    title: tr!("Fork from this message"),
                                    "aria-label": tr!("Fork from this message"),
                                    onclick: move |_| on_fork.call(uuid.clone()),
                                    Icon { kind: IconKind::Fork, size: IconSize::Small }
                                }
                            }
                        }
                    }
                }
            }
        }
        Item::Text(text) => {
            let html = render_markdown_linked(&text, &known);
            rsx! { div { class: "turn-text markdown", dangerous_inner_html: "{html}" } }
        }
        Item::Thinking(text) => rsx! { div { class: "thinking", "{text}" } },
        Item::Activity(activity) => rsx! { ActivityBlock { activity, detailed, on_open_file } },
        Item::FileCard(card) => rsx! { FileCardView { card, on_open_file } },
        Item::Footer(footer) => rsx! { FooterView { footer } },
        Item::Answers(pairs) => rsx! { AnswerCard { pairs } },
        Item::Live(line) => rsx! { RunningLine { line } },
        Item::Workflow(item) => {
            let running = workflows.iter().find(|workflow| workflow.task_id == item.task_id).cloned();
            let ended = finished.iter().find(|task| task.task_id == item.task_id).cloned();
            rsx! { WorkflowLine { item, running, ended, on_open_tasks } }
        }
        // Task 6 draws these two.
        Item::PlanList(steps) => rsx! { PlanListView { steps } },
        Item::EndCard(item) => rsx! { EndCardView { item, on_send, marks: card_marks, on_mark: on_card_mark } },
        Item::GeneratedUi(ui) => rsx! { GeneratedUiView { key: "{ui.id}", ui, on_send } },
        Item::Stopped => rsx! { div { class: "stopped-line", {tr!("Stopped")} } },
        Item::Compacted { before, after } => {
            let label = compacted_label(before, after);
            rsx! { div { class: "compacted-line", span { "{label}" } } }
        }
        Item::Plan(_) => rsx! {
            button { class: "plan-row", r#type: "button", onclick: move |_| on_open_plan.call(()), {tr!("Plan proposed")} " " Icon { kind: IconKind::Chevron, size: IconSize::Small } }
        },
    }
}

/// `Session compacted · saved 63.6k tokens`, or just the first part when the counts are unknown.
fn compacted_label(before: Option<u64>, after: Option<u64>) -> String {
    use crate::details_view::format_tokens_short;
    match (before, after) {
        (Some(before), Some(after)) if before > after => tr!("Session compacted · saved {saved} tokens", saved = format_tokens_short(before - after)),
        _ => tr!("Session compacted").to_string(),
    }
}

/// A launched workflow: a card while it runs (its name, how many agents it has started, a clock and
/// a dot for each agent), one grey line after it ends. The card opens the Background tasks panel.
#[component]
fn WorkflowLine(item: WorkflowItem, running: Option<RunningWorkflow>, ended: Option<FinishedTask>, on_open_tasks: EventHandler<()>) -> Element {
    let Some(running) = running else {
        let suffix = ended.as_ref().and_then(|task| match task.status {
            TaskStatus::Completed => None,
            TaskStatus::Stopped => Some(("workflow-end", tr!("stopped"))),
            TaskStatus::Failed => Some(("workflow-end failed", tr!("failed"))),
        });
        return rsx! {
            div { class: "workflow-done",
                {tr!("Workflow ran {name}", name = item.name)}
                if let Some((class, label)) = suffix {
                    " "
                    span { class: "{class}", "{label}" }
                }
            }
        };
    };
    let started = running.seen_at;
    rsx! {
        button { class: "workflow-chat-card", r#type: "button", onclick: move |_| on_open_tasks.call(()),
            span { class: "workflow-chat-main",
                span { class: "workflow-chat-name", "{item.name}" }
                span { class: "workflow-chat-meta",
                    span { {tr!("Workflow")} }
                    if running.agents.is_empty() {
                        span { {tr!("Starting workflow")} }
                    } else {
                        span { {if running.agents.len() == 1 { tr!("{count} agent", count = 1) } else { tr!("{count} agents", count = running.agents.len()) }} }
                    }
                    Clock { started: Some(started) }
                }
                span { class: "wf-dots",
                    for agent in running.agents.iter() {
                        span { class: dot_class(agent.state, false) }
                    }
                }
            }
            span { class: "chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small } }
        }
    }
}

#[component]
fn ActivityBlock(activity: Activity, detailed: bool, on_open_file: EventHandler<String>) -> Element {
    let mut open = use_signal(|| false);
    let shown = open() || detailed;
    let chevron = rsx! { Icon { kind: IconKind::Chevron, size: IconSize::Small, open: shown } };
    rsx! {
        div { class: "activity",
            button {
                class: "activity-head",
                r#type: "button",
                "aria-expanded": "{shown}",
                onclick: move |_| open.set(!shown),
                span { class: "activity-summary", "{activity.summary}" }
                CountChips { added: activity.added, removed: activity.removed }
                span { class: "chevron", {chevron.clone()} }
                if let Some(path) = activity.image.clone() {
                    ReadThumbnail { path }
                }
            }
            if shown {
                div { class: "activity-box",
                    for (index, row) in activity.rows.iter().enumerate() {
                        ActivityRow { key: "{index}", row: row.clone(), detailed, on_open_file }
                    }
                }
            }
        }
    }
}

#[component]
fn ActivityRow(row: Row, detailed: bool, on_open_file: EventHandler<String>) -> Element {
    match row {
        Row::Narration(text) => rsx! { div { class: "activity-row narration", "{text}" } },
        Row::Thinking(text) => rsx! { div { class: "activity-row narration thinking", "{text}" } },
        Row::Tool(tool) => rsx! { ToolRowView { tool, detailed, on_open_file } },
    }
}

#[component]
fn ToolRowView(tool: ToolRow, detailed: bool, on_open_file: EventHandler<String>) -> Element {
    let mut expanded = use_signal(|| false);
    let open = expanded() || detailed;
    let chevron = rsx! { Icon { kind: IconKind::Chevron, size: IconSize::Small, open: open } };
    let class = if tool.failed { "activity-row-head failed" } else { "activity-row-head" };
    rsx! {
        div { class: "activity-row",
            button {
                class: "{class}",
                r#type: "button",
                "aria-expanded": "{open}",
                onclick: move |_| expanded.set(!open),
                if !tool.verb.is_empty() {
                    span { class: "row-verb", "{tool.verb}" }
                }
                if !tool.path.is_empty() {
                    span {
                        class: "row-subject file-name-link",
                        onclick: {
                            let path = tool.path.clone();
                            move |event: MouseEvent| {
                                event.stop_propagation();
                                on_open_file.call(path.clone());
                            }
                        },
                        "{tool.subject}"
                    }
                } else {
                    span { class: "row-subject", "{tool.subject}" }
                }
                if tool.failed {
                    span { class: "row-failed", "failed" }
                }
                if tool.stopped {
                    span { class: "row-stopped", {tr!("Stopped")} }
                }
                CountChips { added: tool.added, removed: tool.removed }
                span { class: "chevron", {chevron.clone()} }
            }
            if open {
                if let Some(diff) = tool.has_diff.then(|| diff_for_tool_call(&tool.call)).flatten() {
                    if let Some(error) = tool.error.clone() {
                        div { class: "state-error", "{error}" }
                    }
                    DiffView { diff }
                } else {
                    ToolDetail { tool_call: tool.call.clone() }
                }
            }
        }
    }
}

#[component]
fn FileCardView(card: FileCard, on_open_file: EventHandler<String>) -> Element {
    let heading = if card.files.len() == 1 { None } else { Some(tr!("{count} files edited", count = card.files.len())) };
    rsx! {
        div { class: "file-card",
            if let Some(heading) = heading {
                div { class: "file-card-row file-card-head",
                    span { class: "file-name", "{heading}" }
                    CountChips { added: card.added, removed: card.removed }
                }
            }
            for file in card.files.iter() {
                div { key: "{file.path}", class: "file-card-row", title: "{file.path}",
                    button {
                        class: "file-name file-name-link",
                        r#type: "button",
                        title: tr!("Open {path}", path = file.path),
                        onclick: {
                            let path = file.path.clone();
                            move |_| on_open_file.call(path.clone())
                        },
                        "{file.name}"
                    }
                    CountChips { added: file.added, removed: file.removed }
                }
            }
        }
    }
}

#[component]
fn FooterView(footer: Footer) -> Element {
    let time = footer.at.as_deref().map(|at| relative_time(at, OffsetDateTime::now_utc())).unwrap_or_default();
    let text = footer.text.clone();
    rsx! {
        div { class: "turn-footer",
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("Copy"),
                onclick: move |_| crate::clipboard::copy_text(&text),
                Icon { kind: IconKind::Copy, size: IconSize::Small }
            }
            if !time.is_empty() {
                span { class: "footer-time", "{time}" }
            }
        }
    }
}

/// The seconds of a running turn. Only this component re-renders each second; the transcript above
/// it is not rebuilt.
#[component]
pub fn Clock(started: Option<SystemTime>) -> Element {
    let mut now = use_signal(SystemTime::now);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            now.set(SystemTime::now());
        }
    });
    let Some(started) = started else { return rsx! {} };
    let seconds = now().duration_since(started).map(|elapsed| elapsed.as_secs()).unwrap_or(0);
    let label = elapsed_label(seconds);
    rsx! { span { class: "live-clock", "{label}" } }
}

/// What the running turn is doing right now: a tool, the model writing, or a question for the person.
#[component]
fn RunningLine(line: LiveLine) -> Element {
    match line.kind {
        LiveKind::Tool => rsx! {
            div { class: "live-line",
                span { class: "live-spinner" }
                span { class: "live-label", "{line.label}" }
                if !line.subject.is_empty() {
                    span { class: "live-subject", "{line.subject}" }
                }
                Clock { started: line.started }
            }
        },
        LiveKind::Thinking => rsx! {
            div { class: "live-line",
                span { class: "live-dots", "···" }
                if !line.label.is_empty() {
                    span { class: "live-label", "{line.label}" }
                }
                Clock { started: line.started }
            }
        },
        LiveKind::NeedsInput => rsx! {
            div { class: "live-line",
                span { class: "live-ring" }
                span { class: "needs-input", "{line.label} " Icon { kind: IconKind::Chevron, size: IconSize::Small } }
                CountChips { added: line.added, removed: line.removed }
            }
        },
    }
}

/// A finished question: each question in grey and its answer in black, in a narrow bordered card.
#[component]
fn AnswerCard(pairs: Vec<AnswerPair>) -> Element {
    rsx! {
        div { class: "answer-card",
            for pair in pairs.iter() {
                div { key: "{pair.question}", class: "answer-pair",
                    div { class: "answer-question", "{pair.question}" }
                    div { class: "answer-text", "{pair.answer}" }
                }
            }
        }
    }
}

#[cfg(test)]
mod compacted_tests {
    use super::compacted_label;

    #[test]
    fn the_line_names_the_tokens_saved_when_both_counts_are_known() {
        assert_eq!(compacted_label(Some(21_702), Some(4_290)), "Session compacted · saved 17.4k tokens");
        assert_eq!(compacted_label(Some(5), None), "Session compacted");
        assert_eq!(compacted_label(None, Some(5)), "Session compacted");
        assert_eq!(compacted_label(Some(10), Some(20)), "Session compacted", "nothing saved is not claimed");
    }
}
