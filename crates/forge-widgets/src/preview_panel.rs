use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{preview_key_from_event, use_workspace, Panel, PreviewKey, Workspace};

use crate::panel_frame::PanelFrame;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// Puts the keyboard in the address field, once the field is there (a new tab draws it a moment later).
fn focus_address() {
    let _ = document::eval("setTimeout(function(){var i=document.querySelector('.pane.focused .preview-url');if(i){i.focus();i.select();}},30);");
}

/// Does a Preview key for session `id`. A new tab and the address key leave the cursor in the address field.
pub(crate) fn run_preview_key(workspace: Workspace, id: LiveSessionId, key: PreviewKey) {
    workspace.preview_key(id, key);
    if matches!(key, PreviewKey::NewTab | PreviewKey::FocusAddress) {
        focus_address();
    }
}

/// Tabs, back, forward, reload and an address bar over a surface that the page's own web view
/// covers (`WebViewHost` keeps the two in step). A tab that could not be reached shows a notice
/// in the surface's place.
#[component]
pub fn PreviewPanel(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let focused = forge_features::use_pane_focused();
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
    // While the person is typing in the address field a page that moves on (a redirect, a reload on
    // change) must not rewrite what they typed.
    let mut editing = use_signal(|| false);

    let tabs = workspace.previews(id);
    let active = tabs.active();
    let ids = tabs.ids();
    let current = active.and_then(|tab| tabs.current(tab));
    let failure = active.and_then(|tab| tabs.failure(tab));
    let (can_back, can_forward) = match active {
        Some(tab) => (tabs.can_back(tab), tabs.can_forward(tab)),
        None => (false, false),
    };
    // The address field follows the page, and is only rewritten when the address itself changes and
    // nobody is typing in it.
    let address = use_memo(move || {
        let tabs = workspace.previews(id);
        tabs.active().and_then(|tab| tabs.current(tab)).unwrap_or_default()
    });
    use_effect(move || {
        let shown = address();
        if !*editing.peek() {
            input.set(shown);
        }
        notice.set(None);
    });

    let mut submit = move || {
        let Some(tab) = workspace.previews(id).active() else { return };
        editing.set(false);
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
    // The copy button shows a check for a moment after it copied.
    let mut copied = use_signal(|| false);

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
                    Icon { kind: IconKind::Close, size: IconSize::Small }
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
        PanelFrame {
            id,
            panel: Panel::Preview,
            tools,
            // Browser keys pressed in the panel: they come before the session keys (⌃Tab, ⌘1…9).
            onkeydown: move |event: KeyboardEvent| {
                if let Some(key) = preview_key_from_event(&event) {
                    event.prevent_default();
                    event.stop_propagation();
                    run_preview_key(workspace, id, key);
                }
            },
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
                    Icon { kind: IconKind::ChevronLeft }
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
                    Icon { kind: IconKind::Chevron }
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Reload"),
                    "aria-label": "Reload",
                    disabled: current.is_none(),
                    onclick: move |_| retry(),
                    Icon { kind: IconKind::Reload }
                }
                input {
                    class: "preview-url",
                    r#type: "text",
                    placeholder: tr!("Enter a URL"),
                    value: "{input}",
                    oninput: move |event| {
                        editing.set(true);
                        input.set(event.value());
                    },
                    onblur: move |_| {
                        editing.set(false);
                        input.set(address());
                    },
                    onkeydown: move |event: KeyboardEvent| {
                        if event.key() == Key::Enter {
                            submit();
                        }
                    },
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Copy address"),
                    "aria-label": tr!("Copy address"),
                    disabled: current.is_none(),
                    onclick: {
                        let current = current.clone();
                        move |_| {
                            if let Some(url) = current.as_deref() {
                                forge_entities::copy_text(url);
                                copied.set(true);
                                spawn(async move {
                                    tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
                                    copied.set(false);
                                });
                            }
                        }
                    },
                    Icon { kind: if copied() { IconKind::Check } else { IconKind::Copy }, size: IconSize::Small }
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
                } else if !focused {
                    // Only the focused pane has the native view; this one comes back on a click.
                    div { class: "preview-idle", div { class: "dock-note", {tr!("Click to show the page")} } }
                } else {
                    // The page's web view lies over this; the hint shows until it has an address.
                    div { class: "preview-surface",
                        if current.is_none() {
                            div { class: "dock-note", {tr!("Enter an address above.")} }
                            if let Some(tab) = active {
                                DevServerFinder { id, tab }
                            }
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

/// The desktop app's `앱을 미리 볼까요? 개발 서버 감지` in an empty tab (observed in round 36): the
/// servers the folder offers, each with its command and `이것 사용`, which saves it in
/// `.claude/launch.json`, starts it and shows it here.
#[component]
fn DevServerFinder(id: LiveSessionId, tab: forge_features::PreviewTabId) -> Element {
    let workspace = use_workspace();
    let mut found = use_signal(|| None::<Vec<forge_domain::ServerCandidate>>);
    let mut starting = use_signal(|| None::<String>);
    let mut problem = use_signal(|| None::<String>);
    match found() {
        None => rsx! {
            div { class: "dev-servers",
                p { class: "dev-servers-ask", {tr!("Preview your app?")} }
                button {
                    class: "preview-open",
                    r#type: "button",
                    onclick: move |_| found.set(Some(workspace.detect_dev_servers(id))),
                    {tr!("Detect dev server")}
                }
            }
        },
        Some(servers) => rsx! {
            div { class: "dev-servers",
                if servers.is_empty() {
                    p { class: "dev-servers-ask", {tr!("No dev server was found in this folder. Enter its address above.")} }
                }
                for server in servers {
                    div { key: "{server.name}-{server.command_line()}", class: "dev-server",
                        div { class: "dev-server-text",
                            div { class: "dev-server-name",
                                {match server.framework {
                                    Some(framework) => tr!("{framework} · port {port}", framework = framework, port = server.port),
                                    None => tr!("{name} · port {port}", name = server.name.clone(), port = server.port),
                                }}
                            }
                            code { class: "dev-server-command", "{server.command_line()}" }
                        }
                        button {
                            class: "preview-open",
                            r#type: "button",
                            disabled: starting().is_some(),
                            onclick: {
                                let server = server.clone();
                                move |_| {
                                    problem.set(None);
                                    match workspace.use_dev_server(id, tab, server.clone()) {
                                        Ok(()) => starting.set(Some(server.command_line())),
                                        Err(message) => problem.set(Some(message)),
                                    }
                                }
                            },
                            {tr!("Use this")}
                        }
                    }
                }
                if let Some(command) = starting() {
                    p { class: "dock-note", {tr!("Starting {command}. The page opens here once it answers.", command = command)} }
                }
                if let Some(message) = problem() {
                    p { class: "dock-note error", "{message}" }
                }
                button {
                    class: "dev-servers-back",
                    r#type: "button",
                    onclick: move |_| {
                        found.set(None);
                        starting.set(None);
                        problem.set(None);
                    },
                    {tr!("Back to browsing")}
                }
            }
        },
    }
}
