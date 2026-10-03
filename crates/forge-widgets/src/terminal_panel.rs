use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use base64::Engine;
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel, TabId, TerminalTabs};
use forge_ports::{TerminalEvent, TerminalId, TerminalPort, TerminalSize};
use serde::Deserialize;

use crate::panel_frame::PanelFrame;

// What the page tells Rust: its size (columns and rows) and what the person typed.
#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum FromPage {
    Size { cols: u16, rows: u16 },
    Data { d: String },
}

// The page side of one tab: an xterm.js in the element `__HOST__`, kept to its size, wired to the
// Rust side with `dioxus.send` / `dioxus.recv`. A hidden tab (`display: none`) has no size and
// reports nothing. It cleans itself up when its element leaves the page.
const BRIDGE_JS: &str = r#"
const host = document.getElementById("__HOST__");
const term = new Terminal({ fontFamily: 'ui-monospace, "SF Mono", Menlo, monospace', fontSize: 12, cursorBlink: true });
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open(host);
let last = "";
function report() {
    if (host.offsetWidth === 0 || host.offsetHeight === 0) return;
    fit.fit();
    const key = term.cols + "x" + term.rows;
    if (key !== last) { last = key; dioxus.send({ t: "size", cols: term.cols, rows: term.rows }); }
}
term.onData((d) => dioxus.send({ t: "data", d }));
const observer = new ResizeObserver(report);
observer.observe(host);
const watch = setInterval(() => {
    if (!host.isConnected) { clearInterval(watch); observer.disconnect(); term.dispose(); }
}, 2000);
report();
term.focus();
while (true) {
    const m = await dioxus.recv();
    if (m.t === "out") {
        const bin = atob(m.b);
        const bytes = new Uint8Array(bin.length);
        for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
        term.write(bytes);
    } else if (m.t === "text") {
        term.write(m.s);
    }
}
"#;

fn bridge_js(host_id: &str) -> String {
    BRIDGE_JS.replace("__HOST__", host_id)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Starting,
    Running,
    Exited(Option<u32>),
    Failed,
}

// Runs one tab from start to end: the page, the shell, and the two streams between them. It ends
// when the shell ends, the page goes away or the shell cannot start; dropping it (the tab was
// closed) stops it anywhere.
async fn drive(
    terminal: Arc<dyn TerminalPort>,
    host_id: String,
    cwd: PathBuf,
    mut phase: Signal<Phase>,
    running: Rc<Cell<Option<TerminalId>>>,
) {
    let mut page = document::eval(&bridge_js(&host_id));
    // The page reports its size first; the shell is started at that size.
    let size = loop {
        match page.recv::<FromPage>().await {
            Ok(FromPage::Size { cols, rows }) => break TerminalSize { cols, rows },
            Ok(FromPage::Data { .. }) => {}
            Err(_) => return,
        }
    };
    let mut session = match terminal.open(&cwd, size) {
        Ok(session) => session,
        Err(error) => {
            let _ = page.send(serde_json::json!({ "t": "text", "s": format!("\x1b[31m{error}\x1b[0m\r\n") }));
            phase.set(Phase::Failed);
            return;
        }
    };
    running.set(Some(session.id));
    phase.set(Phase::Running);
    loop {
        tokio::select! {
            from_page = page.recv::<FromPage>() => match from_page {
                Ok(FromPage::Data { d }) => { let _ = terminal.write(session.id, d.as_bytes()); }
                Ok(FromPage::Size { cols, rows }) => { let _ = terminal.resize(session.id, TerminalSize { cols, rows }); }
                Err(_) => return,
            },
            event = session.events.recv() => match event {
                Some(TerminalEvent::Output(bytes)) => {
                    let b = base64::engine::general_purpose::STANDARD.encode(bytes);
                    let _ = page.send(serde_json::json!({ "t": "out", "b": b }));
                }
                Some(TerminalEvent::Exited(code)) => {
                    let line = match code {
                        Some(code) => format!("\r\n[Process exited ({code})]\r\n"),
                        None => "\r\n[Process exited]\r\n".to_string(),
                    };
                    let _ = page.send(serde_json::json!({ "t": "text", "s": line }));
                    running.set(None);
                    phase.set(Phase::Exited(code));
                    return;
                }
                None => return,
            },
        }
    }
}

#[component]
fn TerminalTab(tab: TabId, cwd: PathBuf, active: bool, on_restart: EventHandler<TabId>) -> Element {
    let workspace = use_workspace();
    let host_id = format!("forge-terminal-{}", tab.0);
    let phase = use_signal(|| Phase::Starting);
    // Not a signal: `use_drop` runs while the component's own signals may already be gone.
    let running = use_hook(|| Rc::new(Cell::new(None::<TerminalId>)));

    {
        let host_id = host_id.clone();
        let running = running.clone();
        use_future(move || {
            let (host_id, cwd, running) = (host_id.clone(), cwd.clone(), running.clone());
            async move { drive(workspace.terminal(), host_id, cwd, phase, running).await }
        });
    }
    {
        let running = running.clone();
        use_drop(move || {
            if let Some(id) = running.get() {
                workspace.terminal().close(id);
            }
        });
    }

    let notice = match phase() {
        Phase::Exited(Some(code)) => Some(format!("Process exited ({code})")),
        Phase::Exited(None) => Some("Process exited".to_string()),
        Phase::Failed => Some("Could not start the shell".to_string()),
        Phase::Starting | Phase::Running => None,
    };
    rsx! {
        div { class: if active { "terminal-view active" } else { "terminal-view" },
            div { id: "{host_id}", class: "terminal-host" }
            if let Some(notice) = notice {
                div { class: "terminal-exited",
                    span { "{notice}" }
                    button { class: "terminal-restart", r#type: "button", onclick: move |_| on_restart.call(tab), "Restart" }
                }
            }
        }
    }
}

/// The terminal panel: tabs, each a shell in the session folder. Closing the last tab closes the
/// panel. Leaving the session ends its shells (the panel is unmounted with the chat).
#[component]
pub fn TerminalPanel(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let mut tabs = use_signal(|| {
        let mut tabs = TerminalTabs::default();
        tabs.open();
        tabs
    });
    let active = tabs.read().active();
    let tab_ids: Vec<TabId> = tabs.read().tabs().to_vec();

    let tools = rsx! {
        for (index, tab) in tab_ids.iter().copied().enumerate() {
            span { key: "{tab.0}", class: if Some(tab) == active { "terminal-tab active" } else { "terminal-tab" },
                button { class: "terminal-tab-label", r#type: "button", onclick: move |_| tabs.write().select(tab), "Shell {index + 1}" }
                button {
                    class: "terminal-tab-close",
                    r#type: "button",
                    title: "Close tab",
                    "aria-label": "Close tab",
                    onclick: move |_| {
                        tabs.write().close(tab);
                        if tabs.read().is_empty() {
                            workspace.change_dock(id, |dock| dock.close(Panel::Terminal));
                        }
                    },
                    "×"
                }
            }
        }
        button {
            class: "icon-button",
            r#type: "button",
            title: "New terminal",
            "aria-label": "New terminal",
            onclick: move |_| {
                tabs.write().open();
            },
            "+"
        }
    };

    rsx! {
        PanelFrame { id, panel: Panel::Terminal, tools,
            for tab in tab_ids.iter().copied() {
                TerminalTab {
                    key: "{tab.0}",
                    tab,
                    cwd: cwd.clone(),
                    active: Some(tab) == active,
                    on_restart: move |tab| {
                        let mut tabs = tabs.write();
                        tabs.close(tab);
                        tabs.open();
                    },
                }
            }
        }
    }
}
