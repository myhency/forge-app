use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind};

/// What the round button at the right of the box is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerAction {
    /// `ready` shows the arrow in full colour (something to send); `disabled` makes it inert.
    Send { ready: bool, disabled: bool },
    Stop,
    /// Nothing to send at the right of the box (the microphone where the system has dictation).
    Hidden,
}

/// A turn that is running shows Stop; otherwise the arrow, lit when there is something to send.
pub fn composer_action(running: bool, ready: bool, disabled: bool) -> ComposerAction {
    if running {
        ComposerAction::Stop
    } else {
        ComposerAction::Send { ready, disabled }
    }
}

/// The message box of both the new session and an open session: one rounded box holding `+`, the text and the
/// round action button, and a quiet line under it for the pickers. `top` (cards, chips) and `overlay` (popups)
/// live inside the box; `start` and `end` are the two ends of the line below it.
#[component]
pub fn ComposerShell(
    top: Element,
    overlay: Element,
    leading: Element,
    input: Element,
    action: ComposerAction,
    onsend: EventHandler<()>,
    onstop: EventHandler<()>,
    #[props(default)] onkeydown: EventHandler<KeyboardEvent>,
    start: Element,
    end: Element,
) -> Element {
    let workspace = crate::use_workspace();
    let dictation = workspace.dictation_available();
    // Files dragged from the Finder onto the box are attached, as in the desktop app.
    let composer = try_use_context::<crate::composer_state::ComposerState>();
    let mut drop_over = use_signal(|| false);
    rsx! {
        div { class: "composer-wrap",
            div {
                class: if drop_over() { "composer drop-over" } else { "composer" },
                onkeydown: move |event| onkeydown.call(event),
                ondragover: move |event| {
                    // A sidebar row dragged across (to pin it) is not a file.
                    if composer.is_some() && workspace.pin_drag().is_none() {
                        event.prevent_default();
                        if !drop_over() {
                            drop_over.set(true);
                        }
                    }
                },
                ondragleave: move |_| drop_over.set(false),
                ondrop: move |event| {
                    drop_over.set(false);
                    if let Some(composer) = composer {
                        let paths: Vec<std::path::PathBuf> = event.data_transfer().files().iter().map(|file| file.path()).collect();
                        if !paths.is_empty() {
                            event.prevent_default();
                            composer.add_dropped(paths);
                        }
                    }
                },
                {top}
                div { class: "composer-input-row",
                    div { class: "composer-lead", {leading} }
                    div { class: "composer-field", {input} }
                    div { class: "composer-action",
                        match action {
                            // An empty box offers the system's dictation where the desktop app has its
                            // microphone; the arrow comes back with something to send.
                            ComposerAction::Send { ready: false, .. } | ComposerAction::Hidden if dictation => rsx! {
                                button {
                                    class: "send-icon mic",
                                    r#type: "button",
                                    title: tr!("Dictate"),
                                    "aria-label": tr!("Dictate"),
                                    onclick: move |_| async move {
                                        // The system types into the focused field: the message box.
                                        let _ = document::eval("document.querySelector('.composer-field textarea')?.focus()").await;
                                        workspace.start_dictation();
                                    },
                                    Icon { kind: IconKind::Mic }
                                }
                            },
                            ComposerAction::Send { ready, disabled } => rsx! {
                                button {
                                    class: if ready { "send-icon ready" } else { "send-icon" },
                                    r#type: "button",
                                    title: tr!("Send"),
                                    "aria-label": tr!("Send"),
                                    disabled,
                                    onclick: move |_| onsend.call(()),
                                    Icon { kind: IconKind::Send }
                                }
                            },
                            ComposerAction::Hidden => rsx! {},
                            ComposerAction::Stop => rsx! {
                                button {
                                    class: "stop-button",
                                    r#type: "button",
                                    title: tr!("Stop"),
                                    "aria-label": "Stop",
                                    onclick: move |_| onstop.call(()),
                                    span { class: "stop-dot" }
                                }
                            },
                        }
                    }
                }
                {overlay}
            }
            div { class: "composer-controls",
                div { class: "composer-controls-start", {start} }
                div { class: "composer-controls-end", {end} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_running_turn_shows_stop_whatever_the_box_holds() {
        assert_eq!(composer_action(true, true, false), ComposerAction::Stop);
        assert_eq!(composer_action(true, false, true), ComposerAction::Stop);
    }

    #[test]
    fn otherwise_the_arrow_is_lit_only_when_there_is_something_to_send() {
        assert_eq!(composer_action(false, false, false), ComposerAction::Send { ready: false, disabled: false });
        assert_eq!(composer_action(false, true, false), ComposerAction::Send { ready: true, disabled: false });
        assert_eq!(composer_action(false, true, true), ComposerAction::Send { ready: true, disabled: true });
    }
}
