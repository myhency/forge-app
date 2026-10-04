use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{live_state, TranscriptView, Turns};
use forge_features::{use_workspace, SendMessageBox};

use crate::composer_controls::{EffortPicker, ModePicker, ModelPicker};
use crate::dock::Dock;
use crate::permission_panel::PermissionPanel;
use crate::plus_menu::PlusMenu;
use crate::question_panel::QuestionPanel;
use crate::session_tools::SessionTools;
use crate::sidebar::project_label;
use crate::usage_ring::UsageRing;

// A click on a file name in an answer: the page sends the path to Rust. The handler is replaced each
// time (`dioxus.send` belongs to the eval that created it).
const WATCH_FILE_LINKS: &str = "if (window.__forgeFileLinkHandler) { document.removeEventListener('click', window.__forgeFileLinkHandler); } window.__forgeFileLinkHandler = (event) => { const link = event.target && event.target.closest ? event.target.closest('.file-link') : null; if (link && link.dataset.path) { dioxus.send(link.dataset.path); } }; document.addEventListener('click', window.__forgeFileLinkHandler);";

// Watches the message list: `data-away` on the pane tells the CSS whether the button shows.
const WATCH_SCROLL: &str = "if (!window.__forgeScrollWatch) { window.__forgeScrollWatch = setInterval(() => { const list = document.querySelector('.messages'); if (!list || list.dataset.watching) return; list.dataset.watching = '1'; const pane = list.closest('.main-pane'); const update = () => { const away = list.scrollHeight - list.scrollTop - list.clientHeight > 80; pane?.setAttribute('data-away', away ? 'true' : 'false'); }; list.addEventListener('scroll', update); const resizes = new ResizeObserver(update); resizes.observe(list); const content = list.firstElementChild; if (content) resizes.observe(content); update(); }, 400); }";

#[component]
fn ScrollToEnd() -> Element {
    use_hook(|| {
        document::eval(WATCH_SCROLL);
    });
    rsx! {
        button {
            class: "scroll-end",
            r#type: "button",
            title: "Scroll to end",
            "aria-label": "Scroll to end",
            onclick: |_| {
                document::eval("const list = document.querySelector('.messages'); list?.scrollTo({ top: list.scrollHeight, behavior: 'smooth' });");
            },
            "↓"
        }
    }
}

/// One open session: its messages, any permission request waiting for an answer, and the message box.
#[component]
pub fn ChatWindow(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        let mut links = document::eval(WATCH_FILE_LINKS);
        while let Ok(path) = links.recv::<String>().await {
            workspace.open_file_in_active(path);
        }
    });
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! { div { class: "empty-state", "This session is no longer open." } };
    };
    let view = view_signal.read();
    let mode = workspace.transcript_view(id);
    let archived = view
        .claude_session_id
        .as_ref()
        .is_some_and(|session| workspace.is_archived(&view.project_path, session));
    let dock_expanded = workspace.dock(id).expanded.is_some() && !archived;
    let heading = format!(
        "{} · {}",
        project_label(&view.project_path),
        view.session.title.as_deref().unwrap_or("Session")
    );

    rsx! {
        div { class: if dock_expanded { "main-pane dock-expanded" } else { "main-pane" },
            div { class: "pane-header",
                span { class: "pane-title", "{heading}" }
                div { class: "view-switch", role: "radiogroup", "aria-label": "Transcript view",
                    for option in TranscriptView::ALL {
                        button {
                            key: "{option.label()}",
                            class: if option == mode { "view-option selected" } else { "view-option" },
                            role: "radio",
                            "aria-checked": "{option == mode}",
                            onclick: move |_| workspace.set_transcript_view(id, option),
                            "{option.label()}"
                        }
                    }
                }
                SessionTools { id, terminal: !archived }
            }
            div { class: "messages",
                Turns {
                    messages: view.session.messages.clone(),
                    view: mode,
                    live: (view.phase == Phase::Running).then(|| live_state(view.turn_started, view.pending_permissions.front())),
                    on_open_file: move |path: String| workspace.open_file(id, path),
                }
            }
            ScrollToEnd {}
            if let Some(error) = view.last_error.as_ref() {
                div { class: "banner error", "{error}" }
            }
            if let Some(request) = view.pending_permissions.front() {
                if request.tool_name == forge_entities::QUESTION_TOOL {
                    QuestionPanel { key: "{request.id}", id, request: request.clone() }
                } else {
                    PermissionPanel { key: "{request.id}", id, request: request.clone() }
                }
            }
            if archived {
                div { class: "banner info archived-bar",
                    span { "This session is archived." }
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
                        "Unarchive"
                    }
                }
            } else if view.can_send {
                SendMessageBox { id, running: view.phase == Phase::Running, can_send: true,
                    PlusMenu { project: Some(view.project_path.clone()), add_folder_enabled: true }
                    ModePicker { id }
                    ModelPicker { id }
                    EffortPicker { id }
                    UsageRing { id }
                }
            } else {
                div { class: "banner info",
                    "The project folder no longer exists, so this session can only be viewed."
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
    fn the_file_link_listener_replaces_its_old_self_so_a_new_page_scope_is_the_one_that_hears_clicks() {
        use super::WATCH_FILE_LINKS;
        // `dioxus.send` belongs to the eval that installed the listener; when the chat is shown
        // again a new eval must take over, so the old handler is removed first.
        assert!(WATCH_FILE_LINKS.contains("removeEventListener"), "{WATCH_FILE_LINKS}");
        assert!(WATCH_FILE_LINKS.contains(".file-link"));
        assert!(WATCH_FILE_LINKS.contains("dioxus.send"));
        assert!(WATCH_FILE_LINKS.contains("dataset.path"));
    }

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
