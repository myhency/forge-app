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
use crate::turn_view::{build_turns, Activity, FileCard, Footer, Item, LiveKind, LiveLine, LiveState, Row, ToolKind, ToolRow, WorkflowItem};
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
    /// Asked to fork the session right after an answer (its footer's ⑂, the desktop app's `여기서
    /// 포크하기`): the id of the next user message to cut before, or `None` for the whole session.
    #[props(default)] on_fork_here: Option<EventHandler<Option<String>>>,
    /// The turns the person pinned as chapters, by the id of the message that began them.
    #[props(default)] pinned_chapters: Vec<String>,
    /// Asked to pin or unpin an answer as a chapter (its footer's pin): the id of the message that
    /// began the turn, and the heading. No pin without it.
    #[props(default)] on_pin_chapter: Option<EventHandler<(String, String)>>,
    /// Asked to send this text as the next message (an end card's button); its buttons do nothing without it.
    #[props(default)] on_send: Option<EventHandler<String>>,
    /// What became of each end card of the session, by the `wrap_up` call's id.
    #[props(default)] card_marks: std::collections::HashMap<String, crate::turn_cards::CardMark>,
    /// Asked to record a mark (closed, or sent) for the end card with this id.
    #[props(default)] on_card_mark: Option<EventHandler<(String, crate::turn_cards::CardMark)>>,
    /// Asked to open the Changes panel (a fold line's `변경 사항 보기`); no link without it.
    #[props(default)] on_open_changes: Option<EventHandler<()>>,
    /// What became of each proposed session's card, by the `start_session` call's id.
    #[props(default)] offer_marks: std::collections::HashMap<String, crate::turn_view::OfferMark>,
    /// The person acted on a proposed session's card; the cards have no buttons without it.
    #[props(default)] on_offer: Option<EventHandler<(crate::turn_view::SessionOffer, crate::turn_view::OfferAction)>>,
    /// What a proposed session's `옵션` offers; no `옵션` without it.
    #[props(default)] offer_choices: Option<crate::turn_view::OfferChoices>,
) -> Element {
    let known = known_paths(&messages);
    let forkable: Vec<String> = messages.iter().filter_map(|message| message.uuid.clone()).filter(|uuid| forge_domain::can_fork_before(&messages, uuid)).collect();
    let items = build_turns(&messages, view, live.as_ref());
    let detailed = view.expands_tool_calls();
    // For each item, the id of the next person's message after it: where a fork from an answer cuts.
    let mut next_user: Vec<Option<String>> = vec![None; items.len()];
    let mut upcoming: Option<String> = None;
    for (index, item) in items.iter().enumerate().rev() {
        next_user[index] = upcoming.clone();
        if let Item::User(message) = item {
            upcoming = message.uuid.clone();
        }
    }
    // And the id of the person's message that began the turn: what a pinned chapter hangs on.
    let mut turn_user: Vec<Option<String>> = vec![None; items.len()];
    let mut current: Option<String> = None;
    for (index, item) in items.iter().enumerate() {
        if let Item::User(message) = item {
            current = message.uuid.clone();
        }
        turn_user[index] = current.clone();
    }
    rsx! {
        div { class: "turns",
            for (index, item) in items.into_iter().enumerate() {
                TurnItem { key: "{index}", next_user: next_user[index].clone(), turn_user: turn_user[index].clone(), pinned_chapters: pinned_chapters.clone(), on_pin_chapter, on_fork_here, item, detailed, known: known.clone(), on_open_file, workflows: workflows.clone(), finished: finished.clone(), on_open_tasks, on_open_plan, on_rewind, on_fork, on_send, card_marks: card_marks.clone(), on_card_mark, forkable: forkable.clone(), on_open_changes, offer_marks: offer_marks.clone(), on_offer, offer_choices: offer_choices.clone() }
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
    next_user: Option<String>,
    turn_user: Option<String>,
    pinned_chapters: Vec<String>,
    on_pin_chapter: Option<EventHandler<(String, String)>>,
    on_fork_here: Option<EventHandler<Option<String>>>,
    on_send: Option<EventHandler<String>>,
    card_marks: std::collections::HashMap<String, crate::turn_cards::CardMark>,
    on_card_mark: Option<EventHandler<(String, crate::turn_cards::CardMark)>>,
    forkable: Vec<String>,
    on_open_changes: Option<EventHandler<()>>,
    offer_marks: std::collections::HashMap<String, crate::turn_view::OfferMark>,
    on_offer: Option<EventHandler<(crate::turn_view::SessionOffer, crate::turn_view::OfferAction)>>,
    offer_choices: Option<crate::turn_view::OfferChoices>,
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
                        UserBubble { text: text.clone() }
                        div { class: "message-actions",
                            // The desktop app's read mark (observed 2026-11-06): two ticks, `Claude님이 읽음`,
                            // once the SDK has taken the message in (it then has an id).
                            if message.uuid.is_some() {
                                span { class: "read-mark", title: tr!("Read by Claude"), Icon { kind: IconKind::DoubleCheck, size: IconSize::Small } }
                            } else {
                                span { class: "read-mark", title: tr!("Sent"), Icon { kind: IconKind::Check, size: IconSize::Small } }
                            }
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
        Item::Activity(activity) => rsx! { ActivityBlock { activity, detailed, on_open_file, on_open_changes } },
        Item::FileCard(card) => rsx! { FileCardView { card, on_open_file } },
        Item::Footer(footer) => {
            // Only a turn whose message has an id can carry a pinned chapter.
            let pin = turn_user.map(|anchor| (pinned_chapters.contains(&anchor), anchor));
            rsx! { FooterView { footer, next_user, on_fork_here, pin, on_pin_chapter } }
        }
        Item::Answers(pairs) => rsx! { AnswerCard { pairs } },
        Item::Received { title, text } => rsx! { ReceivedMessage { title, text } },
        Item::SessionOffer(offer) => {
            let mark = offer_marks.get(&offer.id).copied();
            rsx! { SessionOfferCard { offer, mark, on_offer, choices: offer_choices } }
        }
        // A chapter's heading, as the desktop app draws it: the title in bold, the summary on hover.
        // `data-chapter` lets the table of contents scroll to it.
        Item::Chapter(chapter) => rsx! {
            div {
                class: "chapter-heading",
                "data-chapter": "{chapter.index}",
                title: chapter.summary.clone().unwrap_or_default(),
                "{chapter.title}"
            }
        },
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
fn ActivityBlock(activity: Activity, detailed: bool, on_open_file: EventHandler<String>, on_open_changes: Option<EventHandler<()>>) -> Element {
    let mut open = use_signal(|| false);
    let shown = open() || detailed;
    let changed = activity.added + activity.removed > 0;
    rsx! {
        div { class: "activity",
            button {
                class: "activity-head",
                r#type: "button",
                "aria-expanded": "{shown}",
                onclick: move |_| open.set(!shown),
                span { class: "activity-summary", "{activity.summary}" }
                CountChips { added: activity.added, removed: activity.removed }
                // The desktop app's fold line ends with `· 변경 사항 보기` when the turn changed files (round 37).
                if let (true, Some(open_changes)) = (changed, on_open_changes) {
                    span { class: "activity-sep", "·" }
                    span {
                        class: "activity-changes-link",
                        role: "link",
                        onclick: move |event: MouseEvent| {
                            event.stop_propagation();
                            open_changes.call(());
                        },
                        {tr!("View changes")}
                    }
                }
                span { class: "chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: shown } }
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
    let class = if tool.failed { "activity-row-head failed" } else { "activity-row-head" };
    // A file that was read opens in the Files panel from anywhere on its row, as in the desktop app
    // (observed 2026-11-06); other rows open in place.
    let opens_file = tool.kind == ToolKind::Read && !tool.path.is_empty() && !tool.failed && !detailed;
    let read_path = tool.path.clone();
    rsx! {
        div { class: "activity-row",
            button {
                class: "{class}",
                r#type: "button",
                "aria-expanded": "{open}",
                onclick: move |_| {
                    if opens_file {
                        on_open_file.call(read_path.clone());
                    } else {
                        expanded.set(!open);
                    }
                },
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
                    span { class: "row-failed", {tr!("failed")} }
                }
                if tool.stopped {
                    span { class: "row-stopped", {tr!("Stopped")} }
                }
                CountChips { added: tool.added, removed: tool.removed }
                span { class: "chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open } }
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
fn FooterView(footer: Footer, next_user: Option<String>, on_fork_here: Option<EventHandler<Option<String>>>, pin: Option<(bool, String)>, on_pin_chapter: Option<EventHandler<(String, String)>>) -> Element {
    let time = footer.at.as_deref().map(|at| relative_time(at, OffsetDateTime::now_utc())).unwrap_or_default();
    let text = footer.text.clone();
    let spoken = footer.text.clone();
    rsx! {
        div { class: "turn-footer",
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("Copy"),
                onclick: move |_| crate::clipboard::copy_text(&text),
                Icon { kind: IconKind::Copy, size: IconSize::Small }
            }
            // The desktop app's footer (observed 2026-11-06): 여기서 포크하기, 소리 내어 읽기.
            if let Some(on_fork_here) = on_fork_here {
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Fork from here: starts a new session from this point; this session stays as it is."),
                    "aria-label": tr!("Fork from here"),
                    onclick: move |_| on_fork_here.call(next_user.clone()),
                    Icon { kind: IconKind::Fork, size: IconSize::Small }
                }
            }
            if let (Some(on_pin_chapter), Some((pinned, anchor))) = (on_pin_chapter, pin) {
                button {
                    class: if pinned { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: if pinned { tr!("Unpin chapter") } else { tr!("Pin as chapter") },
                    "aria-label": if pinned { tr!("Unpin chapter") } else { tr!("Pin as chapter") },
                    "aria-pressed": "{pinned}",
                    onclick: {
                        let title = crate::turn_view::pinned_chapter_title(&footer.text);
                        move |_| on_pin_chapter.call((anchor.clone(), title.clone()))
                    },
                    Icon { kind: if pinned { IconKind::PinFilled } else { IconKind::Pin }, size: IconSize::Small }
                }
            }
            button {
                class: "icon-button",
                r#type: "button",
                title: tr!("Read aloud"),
                "aria-label": tr!("Read aloud"),
                onclick: move |_| read_aloud(&spoken),
                Icon { kind: IconKind::Speaker, size: IconSize::Small }
            }
            if !time.is_empty() {
                span { class: "footer-time", "{time}" }
            }
        }
    }
}

/// The words of an answer without its markdown marks, for speech.
pub fn speech_text(markdown: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for line in markdown.lines() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let line = line.trim_start_matches(|c: char| c == '#' || c == '>' || c == ' ');
        let line = line.strip_prefix("- ").or_else(|| line.strip_prefix("* ")).unwrap_or(line);
        let cleaned: String = line.chars().filter(|c| !matches!(c, '*' | '`' | '_' | '|')).collect();
        if !cleaned.trim().is_empty() {
            out.push_str(cleaned.trim());
            out.push('\n');
        }
    }
    out.trim().to_string()
}

/// Reads an answer out loud with the system voice through the web view's speech API, or stops
/// the reading when one is going (the desktop app's `소리 내어 읽기`).
fn read_aloud(markdown: &str) {
    let text = serde_json::to_string(&speech_text(markdown)).unwrap_or_else(|_| "\"\"".to_string());
    let script = format!(
        "if (window.speechSynthesis) {{ if (speechSynthesis.speaking) {{ speechSynthesis.cancel(); }} else {{ const u = new SpeechSynthesisUtterance({text}); u.lang = document.documentElement.lang || navigator.language; speechSynthesis.speak(u); }} }}"
    );
    document::eval(&script);
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

/// A message another session sent here, folded as the desktop app does: `수신 메시지 1개 · <title> ›`,
/// the text inside when opened.
#[component]
fn ReceivedMessage(title: String, text: String) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "activity",
            button { class: "activity-head", r#type: "button", "aria-expanded": "{open()}", onclick: move |_| open.set(!open()),
                span { class: "activity-summary", {tr!("Received {count} message", count = 1)} " · {title}" }
                span { class: "chevron", Icon { kind: IconKind::Chevron, size: IconSize::Small, open: open() } }
            }
            if open() {
                div { class: "activity-box received-text", "{text}" }
            }
        }
    }
}

/// A session Claude proposed, as the desktop app's card (observed 2026-11-06): a grey strip `새 세션`,
/// the title, the brief's first line, `×`, and `여기서 하기` / a black `시작`. Started, it becomes one
/// line with the title and `›` that opens the new session; closed or taken here, it goes.
#[component]
fn SessionOfferCard(offer: crate::turn_view::SessionOffer, mark: Option<crate::turn_view::OfferMark>, on_offer: Option<EventHandler<(crate::turn_view::SessionOffer, crate::turn_view::OfferAction)>>, choices: Option<crate::turn_view::OfferChoices>) -> Element {
    use crate::turn_view::{OfferAction, OfferMark, OfferStart};
    let mut options_open = use_signal(|| false);
    // What `옵션` picked; `None` until the person changes something, so Start is like this session.
    let picked: Signal<Option<OfferStart>> = use_signal(|| None);
    let source = offer.clone();
    let act = move |action: OfferAction| {
        let offer = source.clone();
        move |_| {
            if let Some(on_offer) = on_offer {
                let mut offer = offer.clone();
                offer.start_with = picked.peek().clone();
                on_offer.call((offer, action));
            }
        }
    };
    // A hand-off starts by itself, so it is always the line, never a card with buttons (a fork's
    // copy of the conversation holds the call with no mark of its own).
    let mark = if offer.fork { Some(OfferMark::Started) } else { mark };
    match mark {
        Some(OfferMark::Here) => rsx! {},
        // The desktop app keeps a closed card as a quiet line (observed 2026-11-06).
        Some(OfferMark::Dismissed) => rsx! {
            div { class: "offer-started offer-dismissed",
                span { class: "offer-started-title", "{offer.title}" }
                span { class: "offer-dismissed-note", {tr!("Dismissed")} }
            }
        },
        Some(OfferMark::Drafted) => rsx! {
            div { class: "offer-started offer-dismissed",
                span { class: "offer-started-title", "{offer.title}" }
                span { class: "offer-dismissed-note", {tr!("Moved to a new session")} }
            }
        },
        Some(OfferMark::Started) => rsx! {
            button { class: "offer-started", r#type: "button", onclick: act(OfferAction::Open),
                span { class: "offer-started-title", "{offer.title}" }
                Icon { kind: IconKind::Chevron, size: IconSize::Small }
            }
        },
        None => rsx! {
            div { class: "offer-card",
                div { class: "offer-head", {tr!("New session")} }
                div { class: "offer-body",
                    div { class: "offer-text",
                        div { class: "offer-title", "{offer.title}" }
                        if !offer.lead.is_empty() { div { class: "offer-lead", "{offer.lead}" } }
                    }
                    if on_offer.is_some() {
                        button { class: "icon-button offer-close", r#type: "button", title: tr!("Close"), "aria-label": tr!("Close"), onclick: act(OfferAction::Dismiss),
                            Icon { kind: IconKind::Close, size: IconSize::Small }
                        }
                    }
                }
                if on_offer.is_some() {
                    div { class: "offer-actions",
                        if let Some(choices) = choices.clone() {
                            div { class: "offer-options-wrap",
                                button {
                                    class: if options_open() { "offer-options-button open" } else { "offer-options-button" },
                                    r#type: "button",
                                    "aria-expanded": "{options_open()}",
                                    onclick: move |_| options_open.set(!options_open()),
                                    {tr!("Options")}
                                }
                                if options_open() {
                                    div { class: "menu-backdrop", onclick: move |_| options_open.set(false) }
                                    OfferOptions { choices, picked }
                                }
                            }
                        }
                        span { class: "offer-actions-gap" }
                        button { class: "offer-here", r#type: "button", onclick: act(OfferAction::Here), {tr!("Do it here")} }
                        button { class: "offer-start", r#type: "button", onclick: act(OfferAction::Start), {tr!("Start")} }
                    }
                }
            }
        },
    }
}

/// The popover of a proposed session's `옵션`: the folder it starts in, then 모델, 노력 and 권한, each
/// starting from this session's own. A change is kept in `picked` for the card's Start.
#[component]
fn OfferOptions(choices: crate::turn_view::OfferChoices, picked: Signal<Option<crate::turn_view::OfferStart>>) -> Element {
    let mut picked = picked;
    let now = picked().unwrap_or_else(|| choices.current.clone());
    let set = move |change: Box<dyn Fn(&mut crate::turn_view::OfferStart)>| {
        let mut next = picked.peek().clone().unwrap_or_else(|| choices.current.clone());
        change(&mut next);
        picked.set(Some(next));
    };
    let rows: [(String, Vec<(String, String)>, String, u8); 3] = [
        (tr!("Model").to_string(), choices.models.clone(), now.model.clone(), 0),
        (tr!("Effort").to_string(), choices.efforts.clone(), now.effort.clone(), 1),
        (tr!("Permission").to_string(), choices.modes.clone(), now.mode.clone(), 2),
    ];
    let folder = choices.environments.iter().find(|(id, _, _)| *id == now.environment).map(|(_, _, folder)| folder.clone()).unwrap_or_else(|| choices.folder.clone());
    let mut env_options: Vec<(String, String)> = vec![(String::new(), tr!("This computer").to_string())];
    env_options.extend(choices.environments.iter().map(|(id, name, _)| (id.clone(), name.clone())));
    let mut set_env = set.clone();
    rsx! {
        div { class: "offer-options", role: "dialog",
            label { class: "offer-options-row",
                span { {tr!("Environment")} }
                select {
                    class: "select",
                    value: "{now.environment}",
                    onchange: move |event: Event<FormData>| {
                        let chosen = event.value();
                        set_env(Box::new(move |start: &mut crate::turn_view::OfferStart| start.environment = chosen.clone()));
                    },
                    for (option, text) in env_options {
                        option { key: "{option}", value: "{option}", selected: option == now.environment, "{text}" }
                    }
                }
            }
            div { class: "offer-options-folder",
                Icon { kind: IconKind::Folder, size: IconSize::Small }
                span { "{folder}" }
            }
            for (label, options, value, which) in rows {
                label { key: "{which}", class: "offer-options-row",
                    span { "{label}" }
                    select {
                        class: "select",
                        value: "{value}",
                        onchange: {
                            let mut set = set.clone();
                            move |event: Event<FormData>| {
                                let chosen = event.value();
                                set(Box::new(move |start: &mut crate::turn_view::OfferStart| match which {
                                    0 => start.model = chosen.clone(),
                                    1 => start.effort = chosen.clone(),
                                    _ => start.mode = chosen.clone(),
                                }));
                            }
                        },
                        for (option, text) in options {
                            option { key: "{option}", value: "{option}", selected: option == value, "{text}" }
                        }
                    }
                }
            }
        }
    }
}

/// The running turn's output tokens after the clock: `· 474 토큰`. Nothing before the first count.
#[component]
fn LiveTokens(tokens: Option<u64>) -> Element {
    match tokens.filter(|count| *count > 0) {
        Some(count) => {
            let amount = crate::details_view::format_tokens_short(count);
            rsx! { span { class: "live-tokens", "· " {tr!("{amount} tokens", amount = amount)} } }
        }
        None => rsx! {},
    }
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
                LiveTokens { tokens: line.tokens }
            }
        },
        LiveKind::Thinking if line.starting => rsx! {
            div { class: "live-line",
                span { class: "live-starting" }
                span { class: "live-label", "{line.label}" }
            }
        },
        LiveKind::Thinking => rsx! {
            div { class: "live-line",
                span { class: "live-dots", "···" }
                if !line.label.is_empty() {
                    span { class: "live-label", "{line.label}" }
                }
                Clock { started: line.started }
                LiveTokens { tokens: line.tokens }
            }
        },
        LiveKind::NeedsInput => rsx! {
            // The desktop app's waiting mark: an amber pill with an animated ring, nothing else.
            div { class: "live-line",
                span { class: "live-ring waiting" }
                span { class: "needs-input", "{line.label}" }
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

/// Whether a sent message is long enough to fold: more than about nine lines at the bubble's width.
pub fn is_long_message(text: &str) -> bool {
    let lines: usize = text.lines().map(|line| 1 + line.chars().count() / 70).sum();
    lines > 9
}

/// The person's message. A long one is folded to its first lines under a fade with `더 보기`, as
/// in the desktop app (observed 2026-11-06); `간단히 보기` folds it again.
#[component]
fn UserBubble(text: String) -> Element {
    let mut open = use_signal(|| false);
    let long = is_long_message(&text);
    let class = if long && !open() { "user-bubble folded" } else { "user-bubble" };
    rsx! {
        div { class: "{class}",
            div { class: "user-bubble-text", "{text}" }
            if long {
                button {
                    class: "user-bubble-more",
                    r#type: "button",
                    onclick: move |_| open.set(!open()),
                    if open() { {tr!("Show less")} } else { {tr!("Show more")} }
                }
            }
        }
    }
}

#[cfg(test)]
mod speech_tests {
    #[test]
    fn speech_leaves_out_code_blocks_and_markdown_marks() {
        let text = "## Done\n\nThe **fix** is in `stats.py`:\n\n```python\nx = 1\n```\n- one\n| a | b |";
        assert_eq!(super::speech_text(text), "Done\nThe fix is in stats.py:\none\na  b");
    }
}
