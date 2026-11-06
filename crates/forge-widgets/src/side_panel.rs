use dioxus::prelude::*;
use forge_application::{LiveSessionId, SideAnswer, SideQuestion};
use forge_entities::render_markdown;
use forge_features::{use_workspace, Panel};
use forge_shared::{tr, Icon, IconKind, TextArea};

use crate::panel_frame::PanelFrame;

/// The name of a side chat tab: the first question, or `사이드 채팅` (`사이드 채팅 2`…) while it is empty.
pub fn side_tab_title(tab: u32, thread: &[SideQuestion]) -> String {
    match thread.iter().find(|side| side.tab == tab) {
        Some(side) => side.question.lines().next().unwrap_or_default().to_string(),
        None if tab <= 1 => tr!("Side chat").to_string(),
        None => format!("{} {tab}", tr!("Side chat")),
    }
}

/// Keeps the session's answered side questions as sessions of their own (see `Workspace::save_side_chats`). Draws nothing;
/// the sidebar mounts one for every open session, so an answer that comes while another session is shown is saved too.
#[component]
pub fn SideChatSync(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    use_effect(move || {
        // Read, so the effect runs again when an answer arrives or a save finishes.
        if let Some(view) = workspace.live_view(id) {
            let _ = view.read().side_thread.len();
        }
        workspace.save_side_chats(id);
    });
    rsx! {}
}

/// The side chat (⌘; or `/btw`): tabs along the top, each question on the right with its answer under it, and a box
/// to ask more. Nothing here is part of the session; the main chat stays as it was.
#[component]
pub fn SidePanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut draft = use_signal(String::new);
    // Opening the panel gives the session its first (empty) tab, which the sidebar lists.
    use_hook(|| workspace.change_side_tabs(id, |_| {}));
    let tabs = workspace.side_tabs(id);
    // The tab in use scrolls into view, so a new tab past the edge shows its `×`.
    let shown = tabs.active;
    use_effect(use_reactive!(|shown| {
        let _ = shown;
        document::eval("document.querySelector('.side-tab.active')?.scrollIntoView({ inline: 'nearest', block: 'nearest' });");
    }));
    let thread = workspace
        .live_view(id)
        .map(|view| view.read().side_thread.clone())
        .unwrap_or_default();
    let active = tabs.active;
    let turns: Vec<SideQuestion> = thread
        .iter()
        .filter(|side| side.tab == active)
        .cloned()
        .collect();
    let pending = turns.iter().any(|side| side.answer == SideAnswer::Pending);
    let mut ask = move || {
        let question = draft.peek().trim().to_string();
        if !question.is_empty() {
            workspace.ask_side(id, question);
            draft.set(String::new());
        }
    };
    let placeholder = if pending {
        // The side box is narrow: the desktop app shows the short queue hint in a narrow box.
        tr!("Press ⌘⏎ to add it to the queue")
    } else if turns.is_empty() {
        tr!("Ask a quick question")
    } else {
        tr!("Reply in this session…")
    };
    let heading = rsx! {
        div { class: "side-head",
          div { class: "side-tabs",
            for tab in tabs.tabs.iter().copied() {
                div {
                    key: "{tab}",
                    class: if tab == active { "side-tab active" } else { "side-tab" },
                    onclick: move |_| workspace.change_side_tabs(id, |tabs| tabs.select(tab)),
                    span { class: "side-tab-title", {side_tab_title(tab, &thread)} }
                    button {
                        class: "side-tab-close",
                        r#type: "button",
                        title: tr!("Close"),
                        "aria-label": tr!("Close"),
                        onclick: move |event| {
                            event.stop_propagation();
                            let mut stays = true;
                            workspace.change_side_tabs(id, |tabs| stays = tabs.close(tab));
                            if !stays {
                                workspace.change_dock(id, |dock| dock.close(Panel::Side));
                            }
                        },
                        Icon { kind: IconKind::Close }
                    }
                }
            }
          }
            button {
                class: "icon-button side-add",
                r#type: "button",
                title: tr!("New side chat"),
                "aria-label": tr!("New side chat"),
                onclick: move |_| workspace.change_side_tabs(id, |tabs| {
                    tabs.add();
                }),
                "+"
            }
        }
    };
    rsx! {
        PanelFrame {
            id,
            panel: Panel::Side,
            tools: rsx! {},
            heading,
            div { class: "side-chat",
                div { class: "side-thread",
                    if turns.is_empty() {
                        div { class: "dock-note", {tr!("No messages yet")} }
                    }
                    for side in turns.iter() {
                        div { key: "{side.id}", class: "side-turn",
                            div { class: "side-question", "{side.question}" }
                            match &side.answer {
                                SideAnswer::Pending => rsx! { div { class: "side-dots", "aria-label": tr!("Thinking…"), span {} span {} span {} } },
                                SideAnswer::Done(text) => rsx! { div { class: "side-answer markdown", dangerous_inner_html: render_markdown(text) } },
                                SideAnswer::Failed(reason) => rsx! {
                                    div { class: "side-note", if reason.is_empty() { {tr!("There was no answer.")} } else { "{reason}" } }
                                },
                            }
                        }
                    }
                }
                div { class: "side-input",
                    TextArea {
                        value: draft(),
                        placeholder: placeholder.to_string(),
                        oninput: move |text: String| draft.set(text),
                        onkeydown: move |event: KeyboardEvent| {
                            // Enter asks; Shift+Enter adds a line; Enter that ends an IME composition is the input method's.
                            if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                                event.prevent_default();
                                ask();
                            }
                        },
                    }
                    span { class: "side-send", Icon { kind: IconKind::Send } }
                }
            }
        }
    }
}
