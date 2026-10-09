use dioxus::prelude::*;
use forge_shared::tr;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

/// The icons at the right of the chat header: the terminal toggle (hidden when `terminal` is
/// false, as for an archived session) and the `⋮` menu with the Prevent sleep switch.
#[component]
pub fn SessionTools(id: LiveSessionId, terminal: bool) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let terminal_open = workspace.dock(id).is_open(Panel::Terminal);
    let changes_open = workspace.dock(id).is_open(Panel::Changes);
    let files_open = workspace.dock(id).is_open(Panel::Files);
    let preview_open = workspace.dock(id).is_open(Panel::Preview);
    let sleeping = workspace.prevents_sleep(id);
    rsx! {
        div { class: "pane-tools",
            if terminal {
                button {
                    class: if terminal_open { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: tr!("Terminal"),
                    "aria-label": tr!("Terminal"),
                    "aria-pressed": "{terminal_open}",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.toggle(Panel::Terminal)),
                    ">_"
                }
            }
            if terminal {
                button {
                    class: if changes_open { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: tr!("Changes"),
                    "aria-label": tr!("Changes"),
                    "aria-pressed": "{changes_open}",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.toggle(Panel::Changes)),
                    "±"
                }
            }
            if terminal {
                button {
                    class: if preview_open { "icon-button active" } else { "icon-button" },
                    r#type: "button",
                    title: tr!("Preview"),
                    "aria-label": tr!("Preview"),
                    "aria-pressed": "{preview_open}",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.toggle(Panel::Preview)),
                    "◎"
                }
            }
            div { class: "picker",
                button {
                    class: if open() { "picker-button open" } else { "picker-button" },
                    r#type: "button",
                    title: tr!("More"),
                    "aria-label": tr!("More"),
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
                        if terminal {
                            button {
                                class: "picker-item menu-narrow",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{terminal_open}",
                                onclick: move |_| {
                                    workspace.change_dock(id, |dock| dock.toggle(Panel::Terminal));
                                    open.set(false);
                                },
                                span { {tr!("Terminal")} }
                            }
                            button {
                                class: "picker-item menu-narrow",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{changes_open}",
                                onclick: move |_| {
                                    workspace.change_dock(id, |dock| dock.toggle(Panel::Changes));
                                    open.set(false);
                                },
                                span { {tr!("Changes")} }
                            }
                            button {
                                class: "picker-item menu-narrow",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{preview_open}",
                                onclick: move |_| {
                                    workspace.change_dock(id, |dock| dock.toggle(Panel::Preview));
                                    open.set(false);
                                },
                                span { {tr!("Preview")} }
                            }
                        }
                        if terminal {
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitemcheckbox",
                                "aria-checked": "{files_open}",
                                onclick: move |_| {
                                    workspace.change_dock(id, |dock| dock.toggle(Panel::Files));
                                    open.set(false);
                                },
                                span { {tr!("Files")} }
                            }
                        }
                        button {
                            class: "picker-item",
                            r#type: "button",
                            role: "menuitemcheckbox",
                            "aria-checked": "{sleeping}",
                            onclick: move |_| workspace.set_prevent_sleep(id, !sleeping),
                            span { {tr!("Prevent sleep")} }
                            span { class: "menu-key", if sleeping { {tr!("On · This session only")} } else { {tr!("Off · This session only")} } }
                        }
                    }
                }
            }
        }
    }
}
