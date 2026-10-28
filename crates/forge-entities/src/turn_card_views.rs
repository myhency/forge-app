//! The two cards of the task-list tools: the live task list and the end-of-turn card.

use dioxus::prelude::*;
use forge_shared::tr;

use crate::turn_cards::{Step, StepState};
use crate::turn_view::{EndCardItem, EndCardState};

fn step_class(state: StepState) -> &'static str {
    match state {
        StepState::Done => "plan-icon done",
        StepState::InProgress => "plan-icon doing",
        StepState::Pending => "plan-icon waiting",
    }
}

/// The task list of a running turn: a ring or a check and the step's words, one row each.
#[component]
pub fn PlanListView(steps: Vec<Step>) -> Element {
    rsx! {
        div { class: "plan-list",
            for (index, step) in steps.iter().enumerate() {
                div { key: "{index}", class: "plan-step",
                    span { class: step_class(step.state) }
                    span { class: "plan-step-text", "{step.content}" }
                }
            }
        }
    }
}

/// The next-step card. Live: a close button, the recap, the question and the buttons (the first is
/// the primary one and carries `⌘⏎`). Spent: narrower and grey, the chosen button's label under the
/// question, nothing to press.
#[component]
pub fn EndCardView(item: EndCardItem, on_send: Option<EventHandler<String>>) -> Element {
    let mut hidden = use_signal(|| false);
    if hidden() {
        return rsx! {};
    }
    let EndCardItem { card, state } = item;
    match state {
        EndCardState::Spent { chosen } => rsx! {
            div { class: "end-card spent",
                div { class: "end-card-recap", "{card.recap}" }
                div { class: "end-card-ask", "{card.ask}" }
                div { class: "end-card-chosen", "{chosen}" }
            }
        },
        EndCardState::Live => rsx! {
            div { class: "end-card",
                button {
                    class: "end-card-close",
                    r#type: "button",
                    title: tr!("Close"),
                    "aria-label": tr!("Close"),
                    onclick: move |_| hidden.set(true),
                    "×"
                }
                div { class: "end-card-recap", "{card.recap}" }
                div { class: "end-card-ask", "{card.ask}" }
                if !card.actions.is_empty() {
                    div { class: "end-card-actions",
                        for (index, action) in card.actions.iter().enumerate() {
                            button {
                                key: "{index}",
                                class: if index == 0 { "end-card-button primary" } else { "end-card-button" },
                                r#type: "button",
                                onclick: {
                                    let message = action.message.clone();
                                    move |_| {
                                        if let Some(send) = on_send {
                                            send.call(message.clone());
                                        }
                                    }
                                },
                                "{action.label}"
                                if index == 0 {
                                    span { class: "end-card-key", "⌘⏎" }
                                }
                            }
                        }
                    }
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_state_has_its_own_icon_class() {
        assert_eq!(step_class(StepState::Done), "plan-icon done");
        assert_eq!(step_class(StepState::InProgress), "plan-icon doing");
        assert_eq!(step_class(StepState::Pending), "plan-icon waiting");
    }
}
