use std::collections::HashSet;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{PermissionDecision, PermissionMode, PermissionRequest};
use forge_entities::{
    card_takes_focus, code_html, diff_for_tool, numbered_diff_for_tool, permission_key, permission_view, tool_kind,
    workflow_permission_key, DiffView, NumberedDiffView, PermissionKey, ToolKind,
};
use forge_features::use_workspace;
use forge_shared::{tr, Icon, IconKind, IconSize};

use crate::plan_card::PlanCard;

// Asks the page whether a text field holds typed text; the card takes focus only when it does not.
pub(crate) const FOCUSED_TEXT_LEN: &str = "const a = document.activeElement; dioxus.send(a && (a.tagName === 'TEXTAREA' || a.tagName === 'INPUT') ? a.value.length : -1);";

/// Focuses a card that just appeared so its keys work at once, unless the person is typing.
pub(crate) async fn take_focus_unless_typing(mounted: std::rc::Rc<MountedData>) {
    let mut probe = document::eval(FOCUSED_TEXT_LEN);
    let typed_len = probe.recv::<i64>().await.unwrap_or(-1);
    if card_takes_focus(typed_len) {
        let _ = mounted.set_focus(true).await;
    }
}

/// The choice the black button of a session's permission cards offers: the desktop app moves the
/// last choice there (round 37: after "Always allow", the next card's button read "Always allow" and
/// "Allow once" moved into the menu). Kept per session for the app's life; whether the desktop app
/// keeps it per session or for good was not seen.
static LAST_ALWAYS: GlobalSignal<HashSet<LiveSessionId>> = Signal::global(HashSet::new);

/// What "Always allow" does for a request: a file edit switches the session to accept-edits mode
/// (the desktop app's edit card: "이 세션을 편집 수락 모드로 전환합니다"); anything else is saved as a rule.
fn always_switches_to_accept_edits(tool_name: &str) -> bool {
    matches!(tool_kind(tool_name), ToolKind::Created | ToolKind::Edited)
}

/// A file tool's text now, for placing an edit in it. Large files are not read.
fn current_text(path: &str) -> Option<String> {
    const MAX_BYTES: u64 = 2 * 1024 * 1024;
    std::fs::metadata(path).ok().filter(|meta| meta.is_file() && meta.len() <= MAX_BYTES)?;
    std::fs::read_to_string(path).ok()
}

/// A permission request, laid out like the desktop app's card (round 37): a grey header strip with
/// the question, a grey line with the command's description, the command once (`$`, coloured) or the
/// file's path and its change with line numbers, then a grey "Deny" and a black split button. The
/// split button's face is the session's last choice; its menu holds the other one and "Allow and
/// switch to Auto mode". Keys: Esc / `1` deny, ⌘↩ allow once, ⇧⌘↩ always allow (`2` and `3` still
/// work; the desktop app shows no digits). A workflow's card has only "Deny" and "Allow once".
#[component]
pub fn PermissionPanel(id: LiveSessionId, request: PermissionRequest) -> Element {
    let workspace = use_workspace();
    let mut menu_open = use_signal(|| false);
    let view = permission_view(&request.tool_name, &request.input);
    let plain_diff = diff_for_tool(&request.tool_name, &request.input);
    let numbered = use_hook({
        let (name, input) = (request.tool_name.clone(), request.input.clone());
        let path = view.path.clone();
        move || {
            let current = path.as_deref().and_then(current_text);
            numbered_diff_for_tool(&name, &input, current.as_deref())
        }
    });
    let is_workflow = view.workflow.is_some();
    let edits = always_switches_to_accept_edits(&request.tool_name);
    let request_id = request.id.clone();
    let always_on_top = !is_workflow && LAST_ALWAYS.read().contains(&id);

    // Every way of answering goes through here, so the last choice is remembered once.
    let decide = {
        let request_id = request_id.clone();
        move |choice: PermissionKey| {
            // A copy of the signal handle, so this closure stays `Fn` and can be cloned for each button.
            let mut menu = menu_open;
            menu.set(false);
            match choice {
                PermissionKey::Deny => workspace.answer_permission(id, request_id.clone(), PermissionDecision::Deny),
                PermissionKey::AllowOnce => {
                    LAST_ALWAYS.write().remove(&id);
                    workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
                }
                PermissionKey::AllowAlways => {
                    LAST_ALWAYS.write().insert(id);
                    if edits {
                        workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
                        workspace.set_permission_mode(id, PermissionMode::AcceptEdits);
                    } else {
                        workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowAlways);
                    }
                }
            }
        }
    };
    let (decide_key, deny, face, other) = (decide.clone(), decide.clone(), decide.clone(), decide.clone());
    let switch_to_auto = {
        let request_id = request_id.clone();
        move |_| {
            menu_open.set(false);
            workspace.answer_permission(id, request_id.clone(), PermissionDecision::AllowOnce);
            workspace.set_permission_mode(id, PermissionMode::Auto);
        }
    };

    if view.plan.is_some() {
        return rsx! { PlanCard { id, request_id: request_id.clone() } };
    }

    let (face_choice, other_choice) = if always_on_top {
        (PermissionKey::AllowAlways, PermissionKey::AllowOnce)
    } else {
        (PermissionKey::AllowOnce, PermissionKey::AllowAlways)
    };
    let always_note = if edits {
        tr!("Switches this session to accept-edits mode.")
    } else {
        tr!("Saved to the local project settings.")
    };
    let choice_label = |choice: PermissionKey| match choice {
        PermissionKey::AllowAlways => tr!("Always allow"),
        _ => tr!("Allow once"),
    };
    let choice_keys = |choice: PermissionKey| match choice {
        PermissionKey::AllowAlways => forge_domain::KEY_ALLOW_ALWAYS,
        _ => forge_domain::KEY_ANSWER,
    };
    let command_html = view.command.as_deref().map(|command| code_html("bash", command));

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
                let other_mod = modifiers.alt() || ((modifiers.ctrl() || modifiers.meta()) && !command);
                let code = event.code().to_string();
                // With the menu open, Esc only closes it: Esc is also the deny key.
                if menu_open() {
                    if code == "Escape" {
                        event.stop_propagation();
                        event.prevent_default();
                        menu_open.set(false);
                    }
                    return;
                }
                let key = if is_workflow {
                    workflow_permission_key(&code, modifiers.shift(), command, other_mod)
                } else {
                    permission_key(&code, modifiers.shift(), command, other_mod)
                };
                if let Some(key) = key {
                    event.stop_propagation();
                    event.prevent_default();
                    decide_key(key);
                }
            },
            div { class: "perm-title", "{view.title}" }
            if let Some(description) = view.description.clone() {
                div { class: "perm-description", "{description}" }
            }
            if let (Some(path), true) = (view.path.clone(), view.command.is_none()) {
                div { class: "perm-box perm-path", "{path}" }
            }
            if let Some(html) = command_html {
                div { class: "perm-box perm-command",
                    span { class: "perm-prompt", "$ " }
                    span { dangerous_inner_html: "{html}" }
                }
            }
            if let Some(diff) = numbered.clone() {
                NumberedDiffView { diff }
            } else if let Some(diff) = plain_diff {
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
                    class: "perm-button perm-deny",
                    r#type: "button",
                    onclick: move |_| deny(PermissionKey::Deny),
                    {tr!("Deny")}
                }
                div { class: "perm-primary",
                    button {
                        class: "perm-button primary",
                        r#type: "button",
                        onclick: move |_| face(face_choice),
                        {choice_label(face_choice)}
                        span { class: "perm-key", "{choice_keys(face_choice)}" }
                    }
                    if !is_workflow {
                        button {
                            class: "perm-button primary perm-more",
                            r#type: "button",
                            "aria-label": tr!("More choices"),
                            "aria-expanded": "{menu_open()}",
                            onclick: move |_| menu_open.set(!menu_open()),
                            Icon { kind: IconKind::Chevron, size: IconSize::Small, open: true }
                        }
                    }
                }
                if menu_open() {
                    // A click anywhere outside the menu closes it.
                    div { class: "perm-menu-backdrop", onclick: move |_| menu_open.set(false) }
                    div { class: "perm-menu",
                        button { class: "perm-menu-item", r#type: "button", onclick: move |_| other(other_choice),
                            div { class: "perm-menu-row",
                                span { class: "perm-menu-title", {choice_label(other_choice)} }
                                span { class: "perm-menu-keys", "{choice_keys(other_choice)}" }
                            }
                            if other_choice == PermissionKey::AllowAlways {
                                div { class: "perm-menu-note", "{always_note}" }
                            }
                        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_allow_on_a_file_edit_switches_the_mode_and_on_anything_else_saves_a_rule() {
        for name in ["Edit", "MultiEdit", "Write", "NotebookEdit"] {
            assert!(always_switches_to_accept_edits(name), "{name}");
        }
        for name in ["Bash", "Read", "WebFetch", "mcp__x__y"] {
            assert!(!always_switches_to_accept_edits(name), "{name}");
        }
    }
}
