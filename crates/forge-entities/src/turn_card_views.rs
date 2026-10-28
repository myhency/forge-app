//! The two cards of the task-list tools: the live task list and the end-of-turn card.

use dioxus::prelude::*;
use forge_shared::tr;

use std::collections::HashMap;

use crate::turn_cards::{CardMark, Step, StepState};
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
/// question, nothing to press. `marks` holds what became of the session's cards (by the `wrap_up` call's
/// id): a closed card is not drawn, a card that already sent has its buttons disabled. Without `on_send`
/// (an archived or view-only session) the live card has no buttons.
#[component]
pub fn EndCardView(
    item: EndCardItem,
    on_send: Option<EventHandler<String>>,
    #[props(default)] marks: HashMap<String, CardMark>,
    #[props(default)] on_mark: Option<EventHandler<(String, CardMark)>>,
) -> Element {
    let EndCardItem { card, state } = item;
    let mark = marks.get(&card.id).copied();
    if mark == Some(CardMark::Dismissed) {
        return rsx! {};
    }
    let sent = mark == Some(CardMark::Sent);
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
                    onclick: {
                        let id = card.id.clone();
                        move |_| {
                            if let Some(mark) = on_mark {
                                mark.call((id.clone(), CardMark::Dismissed));
                            }
                        }
                    },
                    "×"
                }
                div { class: "end-card-recap", "{card.recap}" }
                div { class: "end-card-ask", "{card.ask}" }
                if on_send.is_some() && !card.actions.is_empty() {
                    div { class: "end-card-actions",
                        for (index, action) in card.actions.iter().enumerate() {
                            button {
                                key: "{index}",
                                class: if index == 0 { "end-card-button primary" } else { "end-card-button" },
                                r#type: "button",
                                title: "{action.message}",
                                disabled: sent,
                                onclick: {
                                    let message = action.message.clone();
                                    let id = card.id.clone();
                                    move |_| {
                                        if sent {
                                            return;
                                        }
                                        // Marked first: the card turns Spent only once the message is in the transcript.
                                        if let Some(mark) = on_mark {
                                            mark.call((id.clone(), CardMark::Sent));
                                        }
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
