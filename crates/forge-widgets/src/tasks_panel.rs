use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{task_cards, Clock, TaskCard, TASK_OUTPUT_TAIL};
use forge_features::{run_blocking, use_workspace, Panel};

use crate::panel_frame::PanelFrame;

/// The background tasks of a session: a card each with its command, a clock, the tail of its output
/// and a stop button.
#[component]
pub fn TasksPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let cards = {
        let view = view.read();
        task_cards(&view.session.messages, &view.background_tasks)
    };
    rsx! {
        PanelFrame { id, panel: Panel::Tasks, tools: rsx! {},
            div { class: "tasks-body",
                div { class: "tasks-heading", "Running" }
                if cards.is_empty() {
                    div { class: "dock-note", "No running tasks" }
                }
                for card in cards {
                    TaskCardView { key: "{card.id}", id, card }
                }
                div { class: "dock-note", "Tasks end when Forge closes the session." }
            }
        }
    }
}

#[component]
fn TaskCardView(id: LiveSessionId, card: TaskCard) -> Element {
    let workspace = use_workspace();
    let mut output = use_signal(String::new);
    let path = card.output_path.clone();
    use_future(move || {
        let path = path.clone();
        async move {
            let Some(path) = path else { return };
            loop {
                let files = workspace.files();
                let target = PathBuf::from(&path);
                if let Some(Ok(text)) = run_blocking(move || files.read_tail(&target, TASK_OUTPUT_TAIL)).await {
                    if output.peek().as_str() != text {
                        output.set(text);
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });
    let task_id = card.id.clone();
    let text = output();
    rsx! {
        div { class: "task-card",
            div { class: "task-head",
                div { class: "task-title",
                    div { class: "task-description", "{card.description}" }
                    div { class: "task-meta",
                        span { "Bash" }
                        Clock { started: Some(card.seen_at) }
                    }
                }
                button {
                    class: "task-stop",
                    r#type: "button",
                    title: "Stop task",
                    "aria-label": "Stop task",
                    onclick: move |_| workspace.stop_task(id, task_id.clone()),
                    span { class: "task-stop-square" }
                }
            }
            if let Some(command) = card.command.clone() {
                div { class: "task-command", "$ {command}" }
            }
            if card.output_path.is_some() {
                if text.trim().is_empty() {
                    div { class: "task-output empty", "No output yet" }
                } else {
                    pre { class: "task-output", "{text}" }
                }
            }
        }
    }
}
