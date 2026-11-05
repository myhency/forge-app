use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{live_state, CardMark, Turns};
use forge_features::{use_settings, SettingsSection, use_workspace, Panel, SendMessageBox};

use crate::composer_controls::{ModePicker, ModelPicker};
use crate::dock::Dock;
use crate::mention_list::MentionList;
use crate::permission_panel::PermissionPanel;
use crate::plus_menu::PlusMenu;
use crate::question_panel::QuestionPanel;
use crate::session_title_menu::SessionTitleMenu;
use crate::session_tools::SessionTools;

use crate::fork_line::ForkLine;
use crate::local_card::LocalCards;
use crate::start_line::StartLine;
use crate::tasks_panel::use_running_workflows;
use crate::usage_ring::UsageRing;
use forge_shared::{tr, Icon, IconKind, IconSize};

// Watches the message list: `data-away` on the pane tells the CSS whether the button shows.
const WATCH_SCROLL: &str = "if (!window.__forgeScrollWatch) { window.__forgeScrollWatch = setInterval(() => { for (const list of document.querySelectorAll('.messages')) { if (list.dataset.watching) continue; list.dataset.watching = '1'; const pane = list.closest('.main-pane'); const update = () => { const away = list.scrollHeight - list.scrollTop - list.clientHeight > 80; pane?.setAttribute('data-away', away ? 'true' : 'false'); }; list.addEventListener('scroll', update); const resizes = new ResizeObserver(update); resizes.observe(list); const content = list.firstElementChild; if (content) resizes.observe(content); update(); } }, 400); }";

#[component]
fn ScrollToEnd() -> Element {
    let pane = forge_features::use_pane_index();
    use_hook(|| {
        document::eval(WATCH_SCROLL);
    });
    rsx! {
        button {
            class: "scroll-end",
            r#type: "button",
            title: tr!("Scroll to end"),
            "aria-label": tr!("Scroll to end"),
            onclick: move |_| {
                document::eval(&format!("const list = document.querySelector('[data-pane=\"{pane}\"] .messages'); list?.scrollTo({{ top: list.scrollHeight, behavior: 'smooth' }});"));
            },
            Icon { kind: IconKind::ArrowDown }
        }
    }
}

/// One open session: its messages, any permission request waiting for an answer, and the message box.
#[component]
pub fn ChatWindow(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let settings = use_settings();
    let workflows = use_running_workflows(id);
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! { div { class: "state-empty empty-state", {tr!("This session is no longer open.")} } };
    };
    let view = view_signal.read();
    let mode = workspace.transcript_view(id, settings.default_transcript_view());
    let archived = view
        .claude_session_id
        .as_ref()
        .is_some_and(|session| workspace.is_archived(&view.project_path, session));
    let dock_expanded = workspace.dock(id).expanded.is_some() && !archived;
    let heading = view.session.title.as_deref().unwrap_or(tr!("Session")).to_string();
    let title_menu = workspace.active_title_menu();
    let tag = workspace.worktree_tag(&view.project_path).map(|tag| match tag.branch {
        Some(branch) => format!("{} · {}", tr!("Worktree"), branch),
        None => tr!("Worktree").to_string(),
    });

    rsx! {
        div { class: if dock_expanded { "main-pane dock-expanded" } else { "main-pane" },
            div { class: "pane-header",
                div { class: "pane-titlebox",
                    span { class: "pane-title", "{heading}" }
                    if let Some(actions) = title_menu {
                        SessionTitleMenu { id, actions, mode }
                    }
                    if let Some(tag) = tag {
                        span { class: "pane-tag", "{tag}" }
                    }
                }
                forge_features::DragSpacer {}
                SessionTools { id, terminal: !archived }
            }
            div { class: "messages",
                StartLine { id }
                ForkLine { id }
                Turns {
                    messages: view.session.messages.clone(),
                    view: mode,
                    live: (view.phase == Phase::Running).then(|| live_state(view.turn_started, view.pending_permissions.front(), view.turn_output_tokens)),
                    on_open_file: move |path: String| workspace.open_file(id, path),
                    workflows: workflows(),
                    finished: view.session.finished_tasks.clone(),
                    on_open_tasks: move |_| workspace.change_dock(id, |dock| if !dock.is_open(Panel::Tasks) { dock.toggle(Panel::Tasks) }),
                    on_open_plan: move |_| workspace.change_dock(id, |dock| dock.open(Panel::Plan)),
                    on_open_changes: EventHandler::new(move |_| workspace.change_dock(id, |dock| dock.open(Panel::Changes))),
                    on_rewind: move |uuid: String| workspace.request_rewind(id, uuid),
                    on_fork: move |uuid: String| workspace.fork_from_message(id, uuid),
                    // No buttons on an archived or view-only session: it cannot send.
                    on_send: (!archived && view.can_send).then(|| {
                        EventHandler::new(move |text: String| {
                            if workspace.session_is_moving(id) {
                                return;
                            }
                            if let Some(handle) = workspace.handle(id) {
                                handle.send_with(text, Vec::new());
                            }
                        })
                    }),
                    card_marks: workspace.end_card_marks(id),
                    on_card_mark: move |(call_id, mark): (String, CardMark)| {
                        // A send the session refuses while it moves marks nothing.
                        if mark == CardMark::Sent && workspace.session_is_moving(id) {
                            return;
                        }
                        workspace.mark_end_card(id, call_id, mark);
                    },
                }
                LocalCards { id }
                // A permission card sits in the conversation, under the turn that asks, as in the desktop
                // app (round 37); a question keeps its place above the box.
                if let Some(request) = view.pending_permissions.front().filter(|request| request.tool_name != forge_entities::QUESTION_TOOL) {
                    div { class: "inline-permission",
                        PermissionPanel { key: "{request.id}", id, request: request.clone() }
                    }
                }
            }
            ScrollToEnd {}
            if let Some(error) = view.last_error.as_ref() {
                if error.contains(forge_domain::CONFIG_INCOMPLETE) {
                    div { class: "banner error",
                        span { {tr!("Forge is not set up: config.json is missing or has no API key.")} }
                        button {
                            class: "archived-bar-button",
                            onclick: move |_| {
                                settings.open();
                                settings.set_section(SettingsSection::Connection);
                            },
                            {tr!("Open Settings > Connection")}
                        }
                    }
                } else {
                    div { class: "banner error", "{error}" }
                }
            }
            if let Some(text) = workspace.rate_limit_bar(id) {
                div { class: "usage-bar",
                    span { "{text}" }
                    button {
                        class: "icon-button",
                        r#type: "button",
                        title: tr!("Close"),
                        "aria-label": tr!("Close"),
                        onclick: move |_| workspace.dismiss_rate_limit_bar(id),
                        Icon { kind: IconKind::Close, size: IconSize::Small }
                    }
                }
            }
            if let Some(request) = view.pending_permissions.front() {
                if request.tool_name == forge_entities::QUESTION_TOOL {
                    QuestionPanel { key: "{request.id}", id, request: request.clone() }
                }
            }
            if archived {
                div { class: "banner info archived-bar",
                    span { {tr!("This session is archived.")} }
                    button {
                        class: "archived-bar-button",
                        onclick: {
                            let project = view.project_path.clone();
                            let session = view.claude_session_id.clone();
                            move |_| {
                                if let Some(session) = session.clone() {
                                    workspace.unarchive_session(project.clone(), session);
                                }
                            }
                        },
                        {tr!("Unarchive")}
                    }
                }
            } else if view.can_send {
                SendMessageBox {
                    id,
                    running: view.phase == Phase::Running,
                    can_send: !workspace.session_is_moving(id),
                    overlay: rsx! { MentionList {} },
                    leading: rsx! { PlusMenu { project: Some(view.project_path.clone()), add_folder_enabled: true } },
                    end: rsx! {
                        ModelPicker { id }
                        ModePicker { id }
                        UsageRing { id }
                    },
                }
            } else {
                div { class: "banner info",
                    {tr!("The project folder no longer exists, so this session can only be viewed.")}
                }
            }
        }
        if !archived {
            Dock { id, cwd: view.project_path.clone() }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WATCH_SCROLL;

    #[test]
    fn the_scroll_watch_keeps_looking_for_the_list_so_a_late_or_replaced_list_is_still_watched() {
        // The list may not exist yet when the script runs, and a new one replaces it when another
        // session is opened: a one-shot lookup would leave the button dead.
        assert!(WATCH_SCROLL.contains("setInterval"), "{WATCH_SCROLL}");
        assert!(WATCH_SCROLL.contains("dataset.watching"));
        assert!(WATCH_SCROLL.contains("__forgeScrollWatch"), "installed once per page");
    }

    #[test]
    fn the_scroll_watch_also_follows_the_growth_of_the_content_not_only_its_own_box() {
        // `.messages` keeps its size while its content grows (a reply streaming in), so the
        // observer must be on the content: the list's first child, `.turns`.
        assert!(WATCH_SCROLL.contains("list.firstElementChild"), "{WATCH_SCROLL}");
        assert!(WATCH_SCROLL.contains("ResizeObserver"));
    }
}
