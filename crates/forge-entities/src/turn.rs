use dioxus::prelude::*;
use forge_domain::Message;
use time::OffsetDateTime;

use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
use crate::markdown::render_markdown;
use crate::message::{attachment_label, ToolDetail};
use crate::relative_time::relative_time;
use crate::transcript_view::TranscriptView;
use crate::turn_view::{build_turns, Activity, FileCard, Footer, Item, Row, ToolRow};

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

/// The conversation: user bubbles, answer text, one folded activity line per turn, a card of the
/// files the turn changed, and a footer under a finished turn.
#[component]
pub fn Turns(messages: Vec<Message>, view: TranscriptView, running: bool) -> Element {
    let items = build_turns(&messages, view, running);
    let detailed = view.expands_tool_calls();
    rsx! {
        div { class: "turns",
            for (index, item) in items.into_iter().enumerate() {
                TurnItem { key: "{index}", item, detailed }
            }
        }
    }
}

#[component]
fn TurnItem(item: Item, detailed: bool) -> Element {
    match item {
        Item::User(message) => {
            let text = message.text();
            rsx! {
                div { class: "turn-user",
                    if !text.is_empty() {
                        div { class: "user-bubble", "{text}" }
                    }
                    if !message.attachments.is_empty() {
                        div { class: "chips",
                            for attachment in message.attachments.iter() {
                                span { class: "chip", title: "{attachment.path.display()}", "{attachment_label(attachment)}" }
                            }
                        }
                    }
                }
            }
        }
        Item::Text(text) => {
            let html = render_markdown(&text);
            rsx! { div { class: "turn-text markdown", dangerous_inner_html: "{html}" } }
        }
        Item::Thinking(text) => rsx! { div { class: "thinking", "{text}" } },
        Item::Activity(activity) => rsx! { ActivityBlock { activity, detailed } },
        Item::FileCard(card) => rsx! { FileCardView { card } },
        Item::Footer(footer) => rsx! { FooterView { footer } },
    }
}

#[component]
fn ActivityBlock(activity: Activity, detailed: bool) -> Element {
    let mut open = use_signal(|| false);
    let shown = open() || detailed;
    let chevron = if shown { "⌄" } else { "›" };
    rsx! {
        div { class: "activity",
            button {
                class: "activity-head",
                r#type: "button",
                "aria-expanded": "{shown}",
                onclick: move |_| open.set(!shown),
                span { class: "activity-summary", "{activity.summary}" }
                CountChips { added: activity.added, removed: activity.removed }
                span { class: "chevron", "{chevron}" }
            }
            if shown {
                div { class: "activity-box",
                    for (index, row) in activity.rows.iter().enumerate() {
                        ActivityRow { key: "{index}", row: row.clone(), detailed }
                    }
                }
            }
        }
    }
}

#[component]
fn ActivityRow(row: Row, detailed: bool) -> Element {
    match row {
        Row::Narration(text) => rsx! { div { class: "activity-row narration", "{text}" } },
        Row::Thinking(text) => rsx! { div { class: "activity-row narration thinking", "{text}" } },
        Row::Tool(tool) => rsx! { ToolRowView { tool, detailed } },
    }
}

#[component]
fn ToolRowView(tool: ToolRow, detailed: bool) -> Element {
    let mut expanded = use_signal(|| false);
    let open = expanded() || detailed;
    let chevron = if open { "⌄" } else { "›" };
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
                span { class: "row-subject", "{tool.subject}" }
                if tool.failed {
                    span { class: "row-failed", "failed" }
                }
                CountChips { added: tool.added, removed: tool.removed }
                span { class: "chevron", "{chevron}" }
            }
            if open {
                if let Some(diff) = tool.has_diff.then(|| diff_for_tool_call(&tool.call)).flatten() {
                    DiffView { diff }
                } else {
                    ToolDetail { tool_call: tool.call.clone() }
                }
            }
        }
    }
}

#[component]
fn FileCardView(card: FileCard) -> Element {
    let heading = if card.files.len() == 1 { None } else { Some(format!("{} files edited", card.files.len())) };
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
                    span { class: "file-name", "{file.name}" }
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
                title: "Copy",
                onclick: move |_| {
                    let literal = serde_json::to_string(&text).unwrap_or_default();
                    document::eval(&format!("navigator.clipboard.writeText({literal});"));
                },
                "⧉"
            }
            if !time.is_empty() {
                span { class: "footer-time", "{time}" }
            }
        }
    }
}
