use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::PermissionDecision;
use forge_entities::{plan_card_takes_focus, plan_key, plan_revision_reason, PlanChoice, PlanKey, DEFAULT_PLAN_REFUSAL};
use forge_features::{use_workspace, Panel};
use forge_shared::tr;

use crate::permission_panel::FOCUSED_TEXT_LEN;

/// Focuses the card only when no text field has focus, not even an empty one.
async fn take_focus_when_no_field_has_it(mounted: std::rc::Rc<MountedData>) {
    let mut probe = document::eval(FOCUSED_TEXT_LEN);
    let typed_len = probe.recv::<i64>().await.unwrap_or(-1);
    if plan_card_takes_focus(typed_len) {
        let _ = mounted.set_focus(true).await;
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PlanStep {
    Choose,
    Revise,
}

/// The card for Claude's plan: accept it (and pick the mode to run in), turn it down, or ask for
/// changes. It opens the Plan panel by itself. No key, timer or focus rule accepts a plan: only a
/// press on a button or the accept keys in the Choose step.
#[component]
pub fn PlanCard(id: LiveSessionId, request_id: String) -> Element {
    let workspace = use_workspace();
    let mut step = use_signal(|| PlanStep::Choose);
    let mut menu_open = use_signal(|| false);
    let mut text = use_signal(String::new);

    let accept = {
        let request_id = request_id.clone();
        move |choice: PlanChoice| {
            menu_open.set(false);
            workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
            workspace.set_permission_mode(id, choice.mode());
        }
    };
    let refuse = {
        let request_id = request_id.clone();
        move |reason: Option<String>| workspace.refuse_plan(id, request_id.clone(), reason)
    };
    let (mut accept_key, mut accept_default, mut accept_edits, mut accept_auto, mut accept_auto_menu) =
        (accept.clone(), accept.clone(), accept.clone(), accept.clone(), accept);
    let (refuse_deny, refuse_revise) = (refuse.clone(), refuse);

    rsx! {
        div {
            class: "permission-panel plan-card",
            tabindex: "0",
            onmounted: move |mounted| {
                workspace.open_panel_over_auto_details(id, Panel::Plan);
                take_focus_when_no_field_has_it(mounted.data())
            },
            onkeydown: move |event: KeyboardEvent| {
                if step() == PlanStep::Revise {
                    // Typing must work: only Esc leaves the text field, back to the choice.
                    if event.code().to_string() == "Escape" {
                        event.stop_propagation();
                        step.set(PlanStep::Choose);
                    }
                    return;
                }
                let modifiers = event.modifiers();
                let command = forge_features::command_held(modifiers);
                let other = modifiers.alt() || ((modifiers.ctrl() || modifiers.meta()) && !command);
                let code = event.code().to_string();
                if let Some(key) = plan_key(&code, modifiers.shift(), command, other) {
                    event.stop_propagation();
                    event.prevent_default();
                    match key {
                        PlanKey::Revise => step.set(PlanStep::Revise),
                        PlanKey::Accept => accept_key(PlanChoice::Accept),
                        PlanKey::AcceptAuto => accept_key(PlanChoice::AcceptAuto),
                    }
                }
            },
            div { class: "perm-title plan-card-head",
                span { {tr!("Claude proposes a plan")} }
                button {
                    class: "plan-card-link",
                    r#type: "button",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.open(Panel::Plan)),
                    {tr!("Open plan")}
                }
            }
            if step() == PlanStep::Choose {
                div { class: "perm-buttons",
                    button {
                        class: "perm-button",
                        r#type: "button",
                        onclick: move |_| refuse_deny(Some(DEFAULT_PLAN_REFUSAL.to_string())),
                        {tr!("Deny")}
                    }
                    button {
                        class: "perm-button",
                        r#type: "button",
                        onclick: move |_| step.set(PlanStep::Revise),
                        {tr!("Revise…")}
                        span { class: "perm-key", "Esc" }
                    }
                    span { class: "spacer" }
                    button {
                        class: "perm-button",
                        r#type: "button",
                        onclick: move |_| accept_default(PlanChoice::Accept),
                        {tr!("Accept")}
                        span { class: "perm-key", "{forge_domain::KEY_ALLOW_ALWAYS}" }
                    }
                    div { class: "perm-primary",
                        button {
                            class: "perm-button primary",
                            r#type: "button",
                            onclick: move |_| accept_auto(PlanChoice::AcceptAuto),
                            {tr!("Accept and use Auto mode")}
                            span { class: "perm-key", "{forge_domain::KEY_ANSWER}" }
                        }
                        button {
                            class: "perm-button primary perm-more",
                            r#type: "button",
                            "aria-label": tr!("More choices"),
                            "aria-expanded": "{menu_open()}",
                            onclick: move |_| menu_open.set(!menu_open()),
                            "⌄"
                        }
                        if menu_open() {
                            div { class: "perm-menu",
                                button {
                                    class: "perm-menu-item",
                                    r#type: "button",
                                    onclick: move |_| accept_edits(PlanChoice::AcceptEdits),
                                    div { class: "perm-menu-title", {tr!("Accept and allow edits")} }
                                }
                                button {
                                    class: "perm-menu-item",
                                    r#type: "button",
                                    onclick: move |_| accept_auto_menu(PlanChoice::AcceptAuto),
                                    div { class: "perm-menu-title", {tr!("Accept and use Auto mode")} }
                                }
                            }
                        }
                    }
                }
            } else {
                textarea {
                    class: "plan-card-text",
                    rows: "3",
                    placeholder: tr!("What should change? (optional)"),
                    value: "{text}",
                    oninput: move |event| text.set(event.value()),
                    onmounted: move |mounted| async move {
                        let _ = mounted.data().set_focus(true).await;
                    },
                }
                if !workspace.plan_comments(id).is_empty() {
                    div { class: "dock-note", {tr!("{count} comments on the plan are sent too", count = workspace.plan_comments(id).len())} }
                }
                div { class: "perm-buttons",
                    button {
                        class: "perm-button",
                        r#type: "button",
                        onclick: move |_| step.set(PlanStep::Choose),
                        {tr!("Back to choices")}
                    }
                    span { class: "spacer" }
                    button {
                        class: "perm-button primary",
                        r#type: "button",
                        onclick: move |_| refuse_revise(Some(plan_revision_reason(&text(), &workspace.plan_comments(id)))),
                        {tr!("Revise")}
                    }
                }
            }
        }
    }
}
