use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

/// The icons at the right of the chat header: the terminal toggle (hidden when `terminal` is
/// false, as for an archived session) and the `⋮` menu with the Prevent sleep switch.
#[component]
pub fn SessionTools(id: LiveSessionId, terminal: bool) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let terminal_open = workspace.dock(id).is_open(Panel::Terminal);
    let sleeping = workspace.prevents_sleep(id);
    rsx! {
        div { class: "pane-tools",
            if terminal {
                button {
                    class: if terminal_open { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: "Terminal",
                    "aria-label": "Terminal",
                    "aria-pressed": "{terminal_open}",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.toggle(Panel::Terminal)),
                    ">_"
                }
            }
            div { class: "picker",
                button {
                    class: if open() { "picker-button open" } else { "picker-button" },
                    r#type: "button",
                    title: "More",
                    "aria-label": "More",
                    "aria-haspopup": "menu",
                    "aria-expanded": "{open()}",
                    onclick: move |_| open.set(!open()),
                    "⋮"
                }
                if open() {
                    div { class: "picker-backdrop", onclick: move |_| open.set(false) }
                    div {
                        class: "picker-menu down",
                        role: "menu",
                        tabindex: "0",
                        onmounted: move |mounted| async move {
                            let _ = mounted.set_focus(true).await;
                        },
                        onkeydown: move |event: KeyboardEvent| {
                            if event.key() == Key::Escape {
                                event.stop_propagation();
                                open.set(false);
                            }
                        },
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitemcheckbox",
                            "aria-checked": "{sleeping}",
                            onclick: move |_| workspace.set_prevent_sleep(id, !sleeping),
                            span { "Prevent sleep" }
                            span { class: "menu-key", if sleeping { "On · This session only" } else { "Off · This session only" } }
                        }
                    }
                }
            }
        }
    }
}
