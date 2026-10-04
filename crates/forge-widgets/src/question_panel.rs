use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionRequest};
use forge_entities::{answers_map, questions_of, Picks};
use forge_features::{menu_digit_row, use_workspace};

/// A question from Claude above the message box: one question at a time, the options as rows
/// (number keys pick on a single-select question), an "Other" field, and Back / Skip / Next.
#[component]
pub fn QuestionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let questions = questions_of(&request.input);
    let total = questions.len();
    let mut index = use_signal(|| 0usize);
    let mut picks = use_signal(|| vec![Picks::default(); total]);
    let request_id = request.id.clone();

    let skip = {
        let request_id = request_id.clone();
        move || workspace.answer_permission(id, request_id.clone(), PermissionDecision::Deny)
    };

    let Some(question) = questions.get(index().min(total.saturating_sub(1))).cloned() else {
        // Nothing to ask: all the person can do is skip, which answers the sidecar.
        let skip = skip.clone();
        return rsx! {
            div { class: "question-panel",
                div { class: "perm-title", "Claude asked a question that Forge cannot show." }
                div { class: "question-footer",
                    span { class: "spacer" }
                    button { class: "perm-button", r#type: "button", onclick: move |_| skip(), "Skip" }
                }
            }
        };
    };
    let position = index().min(total - 1);
    let last = position + 1 == total;
    let current = picks.read().get(position).cloned().unwrap_or_default();
    let ready = current.ready();

    let advance = {
        let questions = questions.clone();
        let request_id = request_id.clone();
        move || {
            if !picks.read().get(position).map_or(false, Picks::ready) {
                return;
            }
            if last {
                workspace.answer_question(id, request_id.clone(), answers_map(&questions, &picks.read()));
            } else {
                index.set(position + 1);
            }
        }
    };
    let pick_option = {
        let multi = question.multi;
        move |option: usize, advance_now: bool| {
            if let Some(slot) = picks.write().get_mut(position) {
                slot.toggle(multi, option);
            }
            if advance_now && !multi && !last {
                index.set(position + 1);
            }
        }
    };
    let (skip_key, skip_button) = (skip.clone(), skip.clone());
    let mut advance_key = advance.clone();
    let mut advance_button = advance.clone();
    let mut pick_key = pick_option.clone();
    let multi = question.multi;
    let option_count = question.options.len();

    rsx! {
        div {
            class: "question-panel",
            tabindex: "0",
            onmounted: move |mounted| crate::permission_panel::take_focus_unless_typing(mounted.data()),
            onkeydown: move |event: KeyboardEvent| {
                let modifiers = event.modifiers();
                let code = event.code().to_string();
                if code == "Escape" {
                    event.stop_propagation();
                    skip_key();
                } else if matches!(code.as_str(), "Enter" | "NumpadEnter") && modifiers.meta() {
                    event.stop_propagation();
                    event.prevent_default();
                    advance_key();
                } else if !modifiers.intersects(Modifiers::CONTROL | Modifiers::META | Modifiers::ALT) {
                    if let Some(row) = menu_digit_row(&code) {
                        if row < option_count {
                            event.stop_propagation();
                            event.prevent_default();
                            pick_key(row, true);
                        }
                    }
                }
            },
            div { class: "question-head",
                span { class: "question-chip", "{position + 1}/{total}" }
                span { class: "perm-title", "{question.text}" }
                span { class: "spacer" }
                button { class: "icon-button", r#type: "button", title: "Skip", onclick: move |_| skip_button(), "×" }
            }
            div { class: "question-options",
                for (option_index, option) in question.options.iter().enumerate() {
                    button {
                        key: "{option_index}",
                        class: if current.chosen.contains(&option_index) { "question-option chosen" } else { "question-option" },
                        r#type: "button",
                        onclick: {
                            let mut pick_option = pick_option.clone();
                            move |_| pick_option(option_index, false)
                        },
                        span { class: "option-text",
                            span { class: "option-label", "{option.label}" }
                            if !option.description.is_empty() {
                                span { class: "option-description", "{option.description}" }
                            }
                        }
                        if multi {
                            span { class: if current.chosen.contains(&option_index) { "option-box checked" } else { "option-box" } }
                        } else if option_index < 4 {
                            span { class: "perm-key", "{option_index + 1}" }
                        }
                    }
                }
                div { class: "question-option other",
                    span { class: "option-label", "Other" }
                    input {
                        class: "question-other",
                        r#type: "text",
                        placeholder: "Type your answer here",
                        value: "{current.other}",
                        // The panel's number keys must not fire while the person types.
                        onkeydown: move |event: KeyboardEvent| {
                            if event.code().to_string() != "Escape" && !event.modifiers().meta() {
                                event.stop_propagation();
                            }
                        },
                        oninput: move |event| {
                            if let Some(slot) = picks.write().get_mut(position) {
                                slot.set_other(multi, event.value());
                            }
                        },
                    }
                }
            }
            div { class: "question-footer",
                if position > 0 {
                    button { class: "perm-button", r#type: "button", onclick: move |_| index.set(position - 1), "Back" }
                }
                span { class: "spacer" }
                button { class: "perm-button", r#type: "button", onclick: move |_| skip(), "Skip" }
                button {
                    class: if ready { "perm-button primary" } else { "perm-button primary disabled" },
                    r#type: "button",
                    disabled: !ready,
                    onclick: move |_| advance_button(),
                    if last { "Submit" } else { "Next" }
                    span { class: "perm-key", "⌘↩" }
                }
            }
        }
    }
}
