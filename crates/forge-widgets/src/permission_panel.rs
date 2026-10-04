use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionMode, PermissionRequest};
use forge_entities::{diff_for_tool, permission_key, permission_view, DiffView, PermissionKey};
use forge_features::use_workspace;

/// A permission request above the message box: what Claude wants to do, the change it would make,
/// and the choice. `1` / Esc deny, `2` / ⇧⌘↩ allow always, `3` / ⌘↩ allow once.
#[component]
pub fn PermissionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let mut menu_open = use_signal(|| false);
    let view = permission_view(&request.tool_name, &request.input);
    let diff = diff_for_tool(&request.tool_name, &request.input);
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
            // The card takes focus when it appears so its keys work at once.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event: KeyboardEvent| {
                let modifiers = event.modifiers();
                let other = modifiers.intersects(Modifiers::CONTROL | Modifiers::ALT);
                if let Some(key) = permission_key(&event.code().to_string(), modifiers.shift(), modifiers.meta(), other) {
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
                div { class: "perm-box", "{command}" }
            }
            if let Some(diff) = diff {
                DiffView { diff }
            }
            div { class: "perm-buttons",
                button {
                    class: "perm-button",
                    r#type: "button",
                    onclick: move |_| deny(PermissionDecision::Deny),
                    "Deny"
                    span { class: "perm-key", "1" }
                    span { class: "perm-key", "Esc" }
                }
                span { class: "spacer" }
                button {
                    class: "perm-button",
                    r#type: "button",
                    onclick: move |_| always(PermissionDecision::AllowAlways),
                    "Always allow"
                    span { class: "perm-key", "2" }
                    span { class: "perm-key", "⇧⌘↩" }
                }
                div { class: "perm-primary",
                    button {
                        class: "perm-button primary",
                        r#type: "button",
                        onclick: move |_| once(PermissionDecision::AllowOnce),
                        "Allow once"
                        span { class: "perm-key", "3" }
                        span { class: "perm-key", "⌘↩" }
                    }
                    button {
                        class: "perm-button primary perm-more",
                        r#type: "button",
                        "aria-label": "More choices",
                        "aria-expanded": "{menu_open()}",
                        onclick: move |_| menu_open.set(!menu_open()),
                        "⌄"
                    }
                    if menu_open() {
                        div { class: "perm-menu",
                            button { class: "perm-menu-item", r#type: "button", onclick: switch_to_auto,
                                div { class: "perm-menu-title", "Allow and switch to Auto mode" }
                                div { class: "perm-menu-note", "The rest of this session is checked automatically." }
                            }
                        }
                    }
                }
            }
        }
    }
}
