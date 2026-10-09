use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{normalize_url, use_workspace, Panel, PreviewTabs, Workspace};

use crate::panel_frame::PanelFrame;
use forge_shared::tr;

// Hands the address to the system browser; the panel shows the reason when it cannot.
fn open_url(workspace: Workspace, mut notice: Signal<Option<String>>, url: &str) {
    match workspace.browser().open(url) {
        Ok(()) => notice.set(None),
        Err(message) => notice.set(Some(message)),
    }
}

/// Tabs, back, forward, reload and an address bar. The page itself opens in the system browser:
/// the window's web view opens every web address outside, so it cannot show a page inside the
/// panel. Each tab keeps the addresses that were opened from it.
#[component]
pub fn PreviewPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut tabs = use_signal(|| {
        let mut tabs = PreviewTabs::default();
        tabs.open();
        tabs
    });
    let mut input = use_signal(String::new);
    let mut notice = use_signal(|| None::<String>);

    let mut submit = move || {
        let Some(url) = normalize_url(&input()) else {
            notice.set(Some(tr!("Enter a web address, such as localhost:3000").to_string()));
            return;
        };
        let active = tabs.read().active();
        let Some(tab) = active else { return };
        tabs.write().navigate(tab, url.clone());
        input.set(url.clone());
        open_url(workspace, notice, &url);
    };
    let mut step = move |forward: bool| {
        let active = tabs.read().active();
        let Some(tab) = active else { return };
        let url = if forward { tabs.write().forward(tab) } else { tabs.write().back(tab) };
        if let Some(url) = url {
            input.set(url.clone());
            open_url(workspace, notice, &url);
        }
    };

    let active = tabs.read().active();
    let ids = tabs.read().ids();
    let current = active.and_then(|tab| tabs.read().current(tab));
    let (can_back, can_forward) = match active {
        Some(tab) => (tabs.read().can_back(tab), tabs.read().can_forward(tab)),
        None => (false, false),
    };

    let tools = rsx! {
        for tab in ids.iter().copied() {
            span { key: "{tab.0}", class: if Some(tab) == active { "terminal-tab active" } else { "terminal-tab" },
                button {
                    class: "terminal-tab-label",
                    r#type: "button",
                    onclick: move |_| {
                        tabs.write().select(tab);
                        let url = tabs.read().current(tab).unwrap_or_default();
                        input.set(url);
                        notice.set(None);
                    },
                    "{tabs.read().label(tab)}"
                }
                button {
                    class: "terminal-tab-close",
                    r#type: "button",
                    title: tr!("Close tab"),
                    "aria-label": "Close tab",
                    onclick: move |_| {
                        tabs.write().close(tab);
                        if tabs.read().is_empty() {
                            workspace.change_dock(id, |dock| dock.close(Panel::Preview));
                        } else {
                            let url = tabs.read().active().and_then(|active| tabs.read().current(active)).unwrap_or_default();
                            input.set(url);
                        }
                    },
                    "×"
                }
            }
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: tr!("New tab"),
            "aria-label": "New tab",
            onclick: move |_| {
                tabs.write().open();
                input.set(String::new());
                notice.set(None);
            },
            "+"
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Preview, tools,
            div { class: "preview-bar",
                button { class: "icon-button", r#type: "button", title: tr!("Back|browser"), "aria-label": tr!("Back|browser"), disabled: !can_back, onclick: move |_| step(false), "‹" }
                button { class: "icon-button", r#type: "button", title: tr!("Forward"), "aria-label": tr!("Forward"), disabled: !can_forward, onclick: move |_| step(true), "›" }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Reload"),
                    "aria-label": "Reload",
                    disabled: current.is_none(),
                    onclick: {
                        let current = current.clone();
                        move |_| {
                            if let Some(url) = current.as_deref() {
                                open_url(workspace, notice, url);
                            }
                        }
                    },
                    "⟳"
                }
                input {
                    class: "preview-url",
                    r#type: "text",
                    placeholder: tr!("Enter a URL"),
                    value: "{input}",
                    oninput: move |event| input.set(event.value()),
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Enter {
                            submit();
                        }
                    },
                }
                button { class: "preview-open", r#type: "button", onclick: move |_| submit(), {tr!("Open in browser")} }
            }
            div { class: "dock-note",
                if let Some(message) = notice() {
                    span { class: "error", "{message}" }
                } else if let Some(url) = current.clone() {
                    span { "{url}" }
                    br {}
                    span { {tr!("This page is open in your browser.")} }
                } else {
                    span { {tr!("Enter an address above. The page opens in your browser.")} }
                }
            }
        }
    }
}
