use std::path::PathBuf;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::TaskStatus;
use forge_entities::{card_key, clear_key, dot_class, elapsed_label, end_label, error_text, format_tokens, parse_journal, steps, task_lists, AgentState, Clock, RunningWorkflow, Step, TaskCard, TASK_OUTPUT_TAIL};
use forge_features::{run_blocking, use_workspace, Panel};

use crate::panel_frame::PanelFrame;
use forge_shared::{tr, Icon, IconKind};

/// How much of a workflow's journal is read: the SDK's own file, with each agent's result in it.
const JOURNAL_TAIL: usize = 4_000_000;

/// The workflows of a session that are running, with what their journals say, refreshed once a
/// second while any runs. The chat draws its workflow cards from this.
pub(crate) fn use_running_workflows(id: LiveSessionId) -> Signal<Vec<RunningWorkflow>> {
    let workspace = use_workspace();
    let mut found = use_signal(Vec::<RunningWorkflow>::new);
    use_future(move || async move {
        loop {
            let cards = match workspace.live_view(id) {
                Some(view) => {
                    let view = view.peek();
                    task_lists(&view.session.messages, &view.background_tasks, &view.session.finished_tasks).running
                }
                None => Vec::new(),
            };
            let mut next = Vec::new();
            for card in cards {
                let Some(workflow) = card.workflow.as_ref() else { continue };
                let mut agents = Vec::new();
                if let Some(path) = workflow.journal_path.clone() {
                    let files = workspace.files();
                    let target = PathBuf::from(path);
                    if let Some(Ok(text)) = run_blocking(move || files.read_tail(&target, JOURNAL_TAIL)).await {
                        agents = parse_journal(&text);
                    }
                }
                next.push(RunningWorkflow { task_id: card.id.clone(), seen_at: card.seen_at, agents });
            }
            if *found.peek() != next {
                found.set(next);
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    });
    found
}

/// The background tasks of a session: the running ones, then a fold of the finished ones.
#[component]
pub fn TasksPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let lists = {
        let view = view.read();
        task_lists(&view.session.messages, &view.background_tasks, &view.session.finished_tasks)
    };
    let state = workspace.task_list_view(id);
    let running_empty = lists.running.is_empty();
    let finished: Vec<TaskCard> = lists.finished.into_iter().filter(|card| !state.cleared.contains(&clear_key(card))).collect();
    let shown_ids: Vec<String> = finished.iter().map(clear_key).collect();
    let nothing_to_clear = finished.is_empty();
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Tasks,
            tools: rsx! {
                button {
                    class: "icon-button",
                    r#type: "button",
                    disabled: nothing_to_clear,
                    title: tr!("Clear finished tasks"),
                    "aria-label": tr!("Clear finished tasks"),
                    onclick: move |_| workspace.clear_finished_tasks(id, shown_ids.clone()),
                    Icon { kind: IconKind::Trash }
                }
            },
            div { class: "tasks-body",
                if !running_empty {
                    div { class: "tasks-heading", {tr!("Running")} }
                    for card in lists.running {
                        TaskCardView { key: "{card_key(&card)}", id, card }
                    }
                }
                if running_empty && finished.is_empty() {
                    div { class: "dock-note", {tr!("No running tasks")} }
                }
                if !finished.is_empty() {
                    button {
                        class: "tasks-fold-head tasks-heading",
                        r#type: "button",
                        "aria-expanded": "{state.finished_open}",
                        onclick: move |_| workspace.toggle_finished_tasks(id),
                        span { {tr!("Finished {count}", count = finished.len())} }
                        span { class: "wf-step-chevron", if state.finished_open { "⌄" } else { "›" } }
                    }
                    if state.finished_open {
                        for card in finished {
                            TaskCardView { key: "{card_key(&card)}", id, card }
                        }
                    }
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
    if let Some(end) = card.end.clone() {
        let class = if end.status == TaskStatus::Failed { "end-failed" } else { "" };
        return rsx! {
            div { class: "task-card task-card-done",
                div { class: "task-title",
                    div { class: "task-description", "{card.description}" }
                    div { class: "task-meta",
                        if card.command.is_some() {
                            span { "Bash" }
                        }
                        span { class: "{class}", {end_label(end.status)} }
                        if let Some(ms) = end.duration_ms {
                            span { {elapsed_label(ms / 1000)} }
                        }
                    }
                }
                if end.status == TaskStatus::Failed && !end.summary.is_empty() {
                    div { class: "state-error", pre { "{error_text(&end.summary)}" } }
                }
            }
        };
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
    let ended = card.end.is_some();
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
                if ended {
                    break;
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
    let end = card.end.clone();
    let stopped = end.as_ref().is_some_and(|end| end.status == TaskStatus::Stopped);
    let tokens = end.as_ref().and_then(|end| end.tokens).or(card.tokens);
    rsx! {
        div { class: if end.is_some() { "task-card workflow-card ended" } else { "task-card workflow-card" },
            div { class: "task-head",
                div { class: "task-title",
                    div { class: "task-description", "{workflow.name}" }
                    div { class: "task-meta",
                        span { {tr!("Workflow")} }
                        if let Some(end) = end.clone() {
                            span { class: if end.status == TaskStatus::Failed { "end-failed" } else { "" }, {end_label(end.status)} }
                        } else {
                            Clock { started: Some(card.seen_at) }
                        }
                    }
                    if !agents.is_empty() || tokens.is_some() {
                        div { class: "task-meta",
                            if !agents.is_empty() {
                                span { {if agents.len() == 1 { tr!("{count} agent", count = 1) } else { tr!("{count} agents", count = agents.len()) }} }
                            }
                            if let Some(tokens) = tokens {
                                span { {tr!("{amount} tokens", amount = format_tokens(tokens))} }
                            }
                        }
                    }
                }
                if let Some(ms) = end.as_ref().and_then(|end| end.duration_ms) {
                    span { class: "task-total", {elapsed_label(ms / 1000)} }
                }
                if end.is_none() {
                    button {
                        class: "task-stop",
                        r#type: "button",
                        title: tr!("Stop task"),
                        "aria-label": tr!("Stop task"),
                        onclick: move |_| workspace.stop_task(id, task_id.clone()),
                        span { class: "task-stop-square" }
                    }
                }
            }
            if !card.description.is_empty() {
                div { class: "task-note", "{card.description}" }
            }
            if let Some(end) = end.clone() {
                if end.status == TaskStatus::Failed && !end.summary.is_empty() {
                    div { class: "state-error",
                        div { class: "state-error-title", {tr!("Error")} }
                        pre { "{error_text(&end.summary)}" }
                    }
                }
            }
            if !phases.is_empty() {
                div { class: "tasks-heading", {tr!("Steps")} }
            }
            for step in phases {
                {
                    let name = step.name.clone();
                    let open = step_open(&step, end.is_some()) != flipped.read().contains(&name);
                    rsx! {
                        PhaseView {
                            key: "{step.name}",
                            step: step.clone(),
                            open,
                            stopped,
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

/// A phase is open by default while one of its agents still runs, unless the run has ended.
fn step_open(step: &Step, ended: bool) -> bool {
    !ended && step.agents.iter().any(|agent| agent.state == AgentState::Running)
}

#[component]
fn PhaseView(step: Step, open: bool, stopped: bool, ontoggle: EventHandler<()>) -> Element {
    let done = step.done();
    let total = step.agents.len();
    let name_class = if step.failed() { "wf-step-name failed" } else { "wf-step-name" };
    rsx! {
        div { class: "wf-step",
            button { class: "wf-step-head", r#type: "button", "aria-expanded": "{open}", onclick: move |_| ontoggle.call(()),
                span { class: "{name_class}", "{step.name}" }
                if total > 0 {
                    span { class: "wf-step-count", "{done}/{total}" }
                }
                span { class: "wf-step-chevron", if open { "⌄" } else { "›" } }
            }
            div { class: "wf-dots",
                for agent in step.agents.iter() {
                    span { class: dot_class(agent.state, stopped) }
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
