use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{card_key, parse_journal, steps, task_cards, AgentState, Clock, Step, TaskCard, TASK_OUTPUT_TAIL};
use forge_features::{run_blocking, use_workspace, Panel};

use crate::panel_frame::PanelFrame;
use forge_shared::tr;

/// How much of a workflow's journal is read: the SDK's own file, with each agent's result in it.
const JOURNAL_TAIL: usize = 4_000_000;

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
                div { class: "tasks-heading", {tr!("Running")} }
                if cards.is_empty() {
                    div { class: "dock-note", {tr!("No running tasks")} }
                }
                for card in cards {
                    TaskCardView { key: "{card_key(&card)}", id, card }
                }
                div { class: "dock-note", {tr!("Tasks end when Forge closes the session.")} }
            }
        }
    }
}

#[component]
fn TaskCardView(id: LiveSessionId, card: TaskCard) -> Element {
    if card.workflow.is_some() {
        return rsx! { WorkflowCardView { id, card } };
    }
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
                    title: tr!("Stop task"),
                    "aria-label": tr!("Stop task"),
                    onclick: move |_| workspace.stop_task(id, task_id.clone()),
                    span { class: "task-stop-square" }
                }
            }
            if let Some(command) = card.command.clone() {
                div { class: "task-command", "$ {command}" }
            }
            if card.output_path.is_some() {
                if text.trim().is_empty() {
                    div { class: "task-output empty", {tr!("No output yet")} }
                } else {
                    pre { class: "task-output", "{text}" }
                }
            }
        }
    }
}

/// A workflow: its name, how many agents it has started, the description, and its phases with the
/// agents of each (read from the workflow's journal once a second).
#[component]
fn WorkflowCardView(id: LiveSessionId, card: TaskCard) -> Element {
    let workspace = use_workspace();
    let Some(workflow) = card.workflow.clone() else { return rsx! {} };
    let mut journal = use_signal(String::new);
    let path = workflow.journal_path.clone();
    use_future(move || {
        let path = path.clone();
        async move {
            let Some(path) = path else { return };
            loop {
                let files = workspace.files();
                let target = PathBuf::from(&path);
                if let Some(Ok(text)) = run_blocking(move || files.read_tail(&target, JOURNAL_TAIL)).await {
                    if journal.peek().as_str() != text {
                        journal.set(text);
                    }
                }
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    });
    // The phases the person opened or closed against their default (open while an agent runs).
    let mut flipped = use_signal(Vec::<String>::new);
    let agents = parse_journal(&journal());
    let phases = steps(&workflow.phases, &agents);
    let task_id = card.id.clone();
    rsx! {
        div { class: "task-card workflow-card",
            div { class: "task-head",
                div { class: "task-title",
                    div { class: "task-description", "{workflow.name}" }
                    div { class: "task-meta",
                        span { {tr!("Workflow")} }
                        if !agents.is_empty() {
                            span { {if agents.len() == 1 { tr!("{count} agent", count = 1) } else { tr!("{count} agents", count = agents.len()) }} }
                        }
                        Clock { started: Some(card.seen_at) }
                    }
                }
                button {
                    class: "task-stop",
                    r#type: "button",
                    title: tr!("Stop task"),
                    "aria-label": tr!("Stop task"),
                    onclick: move |_| workspace.stop_task(id, task_id.clone()),
                    span { class: "task-stop-square" }
                }
            }
            if !card.description.is_empty() {
                div { class: "task-note", "{card.description}" }
            }
            if !phases.is_empty() {
                div { class: "tasks-heading", {tr!("Steps")} }
            }
            for step in phases {
                {
                    let name = step.name.clone();
                    let open = step_open(&step) != flipped.read().contains(&name);
                    rsx! {
                        PhaseView {
                            key: "{step.name}",
                            step: step.clone(),
                            open,
                            ontoggle: move |_| {
                                let mut list = flipped.write();
                                match list.iter().position(|item| item == &name) {
                                    Some(at) => {
                                        list.remove(at);
                                    }
                                    None => list.push(name.clone()),
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}

/// A phase is open by default while one of its agents still runs.
fn step_open(step: &Step) -> bool {
    step.agents.iter().any(|agent| agent.state == AgentState::Running)
}

#[component]
fn PhaseView(step: Step, open: bool, ontoggle: EventHandler<()>) -> Element {
    let done = step.done();
    let total = step.agents.len();
    rsx! {
        div { class: "wf-step",
            button { class: "wf-step-head", r#type: "button", "aria-expanded": "{open}", onclick: move |_| ontoggle.call(()),
                span { class: "wf-step-name", "{step.name}" }
                if total > 0 {
                    span { class: "wf-step-count", "{done}/{total}" }
                }
                span { class: "wf-step-chevron", if open { "⌄" } else { "›" } }
            }
            div { class: "wf-dots",
                for agent in step.agents.iter() {
                    span { class: if agent.state == AgentState::Done { "wf-dot done" } else { "wf-dot running" } }
                }
            }
            if open {
                if step.agents.is_empty() {
                    div { class: "dock-note", {tr!("No agents started yet")} }
                }
                for agent in step.agents.iter() {
                    div { class: "wf-agent", key: "{agent.id}",
                        span { class: "wf-agent-mark", if agent.state == AgentState::Done { "✓" } else { "" } }
                        span { class: "wf-agent-label", "{agent.label}" }
                    }
                }
            }
        }
    }
}
