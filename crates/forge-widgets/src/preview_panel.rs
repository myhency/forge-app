use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

use crate::panel_frame::PanelFrame;
use forge_shared::tr;

/// Tabs, back, forward, reload and an address bar over a surface that the page's own web view
/// covers (`WebViewHost` keeps the two in step). A tab that could not be reached shows a notice
/// in the surface's place.
#[component]
pub fn PreviewPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    // The panel never opens empty: a tab is there to type an address into.
    use_hook(|| {
        if workspace.previews(id).is_empty() {
            workspace.change_previews(id, |tabs| {
                tabs.open();
            });
        }
    });
    let mut input = use_signal(String::new);
    let mut notice = use_signal(|| None::<String>);

    let tabs = workspace.previews(id);
    let active = tabs.active();
    let ids = tabs.ids();
    let current = active.and_then(|tab| tabs.current(tab));
    let failure = active.and_then(|tab| tabs.failure(tab));
    let (can_back, can_forward) = match active {
        Some(tab) => (tabs.can_back(tab), tabs.can_forward(tab)),
        None => (false, false),
    };
    // The address field follows the page, and is only rewritten when the address itself changes.
    let address = use_memo(move || {
        let tabs = workspace.previews(id);
        tabs.active().and_then(|tab| tabs.current(tab)).unwrap_or_default()
    });
    use_effect(move || {
        input.set(address());
        notice.set(None);
    });

    let mut submit = move || {
        let Some(tab) = workspace.previews(id).active() else { return };
        match workspace.preview_go(id, tab, &input()) {
            Ok(()) => notice.set(None),
            Err(message) => notice.set(Some(message)),
        }
    };
    let mut retry = move || {
        let Some(tab) = workspace.previews(id).active() else { return };
        match workspace.preview_reload(id, tab) {
            Ok(()) => notice.set(None),
            Err(message) => notice.set(Some(message)),
        }
    };
    let web_address = current.as_deref().is_some_and(|url| url.starts_with("http://") || url.starts_with("https://"));

    let tools = rsx! {
        for tab in ids.iter().copied() {
            span { key: "{tab.0}", class: if Some(tab) == active { "terminal-tab active" } else { "terminal-tab" },
                button {
                    class: "terminal-tab-label",
                    r#type: "button",
                    onclick: move |_| workspace.change_previews(id, |tabs| tabs.select(tab)),
                    "{tabs.label(tab)}"
                }
                button {
                    class: "terminal-tab-close",
                    r#type: "button",
                    title: tr!("Close tab"),
                    "aria-label": "Close tab",
                    onclick: move |_| workspace.close_preview_tab(id, tab),
                    "×"
                }
            }
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: tr!("New tab"),
            "aria-label": "New tab",
            onclick: move |_| workspace.change_previews(id, |tabs| {
                tabs.open();
            }),
            "+"
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Preview, tools,
            div { class: "preview-bar",
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Back|browser"),
                    "aria-label": tr!("Back|browser"),
                    disabled: !can_back,
                    onclick: move |_| {
                        if let Some(tab) = active {
                            workspace.preview_back(tab);
                        }
                    },
                    "‹"
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Forward"),
                    "aria-label": tr!("Forward"),
                    disabled: !can_forward,
                    onclick: move |_| {
                        if let Some(tab) = active {
                            workspace.preview_forward(tab);
                        }
                    },
                    "›"
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Reload"),
                    "aria-label": "Reload",
                    disabled: current.is_none(),
                    onclick: move |_| retry(),
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
                button {
                    class: "preview-open",
                    r#type: "button",
                    disabled: !web_address,
                    onclick: {
                        let current = current.clone();
                        move |_| {
                            if let Some(url) = current.as_deref() {
                                if let Err(message) = workspace.browser().open(url) {
                                    notice.set(Some(message));
                                }
                            }
                        }
                    },
                    {tr!("Open in browser")}
                }
            }
            div { class: "preview-body",
                if let Some((url, _reason)) = failure {
                    div { class: "preview-failed",
                        p { {tr!("Cannot connect to this address.")} }
                        p { class: "preview-failed-url", "{url}" }
                        button { class: "preview-open", r#type: "button", onclick: move |_| retry(), {tr!("Try again")} }
                    }
                } else {
                    // The page's web view lies over this; the hint shows until it has an address.
                    div { class: "preview-surface",
                        if current.is_none() {
                            div { class: "dock-note", {tr!("Enter an address above.")} }
                        }
                    }
                }
                if let Some(message) = notice() {
                    div { class: "dock-note error", "{message}" }
                }
            }
        }
    }
}
