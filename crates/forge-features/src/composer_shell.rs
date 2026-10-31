use dioxus::prelude::*;
use forge_shared::{tr, Icon, IconKind};

/// What the round button at the right of the box is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerAction {
    /// `ready` shows the arrow in full colour (something to send); `disabled` makes it inert.
    Send { ready: bool, disabled: bool },
    Stop,
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
    rsx! {
        div { class: "composer-wrap",
            div { class: "composer", onkeydown: move |event| onkeydown.call(event),
                {top}
                div { class: "composer-input-row",
                    div { class: "composer-lead", {leading} }
                    div { class: "composer-field", {input} }
                    div { class: "composer-action",
                        match action {
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
