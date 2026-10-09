use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionMode, PermissionRequest};
use forge_entities::{card_takes_focus, diff_for_tool, permission_key, permission_view, workflow_permission_key, DiffView, PermissionKey};
use forge_features::use_workspace;
use forge_shared::tr;

// Asks the page whether a text field holds typed text; the card takes focus only when it does not.
const FOCUSED_TEXT_LEN: &str = "const a = document.activeElement; dioxus.send(a && (a.tagName === 'TEXTAREA' || a.tagName === 'INPUT') ? a.value.length : -1);";

/// Focuses a card that just appeared so its keys work at once, unless the person is typing.
pub(crate) async fn take_focus_unless_typing(mounted: std::rc::Rc<MountedData>) {
    let mut probe = document::eval(FOCUSED_TEXT_LEN);
    let typed_len = probe.recv::<i64>().await.unwrap_or(-1);
    if card_takes_focus(typed_len) {
        let _ = mounted.set_focus(true).await;
    }
}

/// A permission request above the message box: what Claude wants to do, the change it would make,
/// and the choice. `1` / Esc deny, `2` / ⇧⌘↩ allow always, `3` / ⌘↩ allow once. A workflow's card has
/// two choices only: `1` / Esc deny and `2` / ⌘↩ allow once.
#[component]
pub fn PermissionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let mut menu_open = use_signal(|| false);
    let view = permission_view(&request.tool_name, &request.input);
    let diff = diff_for_tool(&request.tool_name, &request.input);
    let is_workflow = view.workflow.is_some();
    let request_id = request.id.clone();

    let decide = {
        let request_id = request_id.clone();
        move |decision: PermissionDecision| workspace.answer_permission(id, request_id.clone(), decision)
    };
    let decide_key = decide.clone();
    let (deny, always, once) = (decide.clone(), decide.clone(), decide.clone());
    let switch_to_auto = {
        let request_id = request_id.clone();
        move |_| {
            menu_open.set(false);
            workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
            workspace.set_permission_mode(id, PermissionMode::Auto);
        }
    };

    rsx! {
        div {
            class: "permission-panel",
            tabindex: "0",
            // The card takes focus when it appears so its keys work at once, unless the person is typing.
            onmounted: move |mounted| take_focus_unless_typing(mounted.data()),
            onkeydown: move |event: KeyboardEvent| {
                let modifiers = event.modifiers();
                let command = forge_features::command_held(modifiers);
                // The command key is Ctrl off the Mac. Alt rules a key out, and so does the other of Ctrl and Meta (the Windows key).
                let other = modifiers.alt() || ((modifiers.ctrl() || modifiers.meta()) && !command);
                let code = event.code().to_string();
                let key = if is_workflow {
                    workflow_permission_key(&code, modifiers.shift(), command, other)
                } else {
                    permission_key(&code, modifiers.shift(), command, other)
                };
                if let Some(key) = key {
                    event.stop_propagation();
                    event.prevent_default();
                    decide_key(match key {
                        PermissionKey::Deny => PermissionDecision::Deny,
                        PermissionKey::AllowAlways => PermissionDecision::AllowAlways,
                        PermissionKey::AllowOnce => PermissionDecision::AllowOnce,
                    });
                }
            },
            div { class: "perm-title", "{view.title}" }
            if view.path.is_some() || view.command.is_some() {
                div { class: "perm-subject", "{view.subject}" }
            }
            if let Some(path) = view.path.clone() {
                div { class: "perm-box", "{path}" }
                div { class: "perm-path", "{path}" }
            }
            if let Some(command) = view.command.clone() {
                div { class: "perm-box perm-command", "{command}" }
            }
            if let Some(diff) = diff {
                DiffView { diff }
            }
            if let Some(workflow) = view.workflow.clone() {
                if let Some(description) = workflow.description {
                    div { class: "perm-description", "{description}" }
                }
                div { class: "perm-note",
                    {tr!("A dynamic workflow runs several sub-agents in parallel and can use up your usage limit quickly. You can stop it any time in the Background tasks panel.")}
                }
            }
            div { class: "perm-buttons",
                button {
                    class: "perm-button",
                    r#type: "button",
                    onclick: move |_| deny(PermissionDecision::Deny),
                    {tr!("Deny")}
                    span { class: "perm-key", "1" }
                    span { class: "perm-key", "Esc" }
                }
                span { class: "spacer" }
                if !is_workflow {
                    button {
                        class: "perm-button",
                        r#type: "button",
                        onclick: move |_| always(PermissionDecision::AllowAlways),
                        {tr!("Always allow")}
                        span { class: "perm-key", "2" }
                        span { class: "perm-key", "{forge_domain::KEY_ALLOW_ALWAYS}" }
                    }
                }
                div { class: "perm-primary",
                    button {
                        class: "perm-button primary",
                        r#type: "button",
                        onclick: move |_| once(PermissionDecision::AllowOnce),
                        {tr!("Allow once")}
                        span { class: "perm-key", if is_workflow { "2" } else { "3" } }
                        span { class: "perm-key", "{forge_domain::KEY_ANSWER}" }
                    }
                    if !is_workflow {
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
                            button { class: "perm-menu-item", r#type: "button", onclick: switch_to_auto,
                                div { class: "perm-menu-title", {tr!("Allow and switch to Auto mode")} }
                                div { class: "perm-menu-note", {tr!("The rest of this session is checked automatically.")} }
                            }
                        }
                    }
                    }
                }
            }
        }
    }
}
