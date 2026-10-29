use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use base64::Engine;
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, CodeRuns, Panel, RunTap, TabId, TapHandle, TerminalTap, TerminalTabs};
use forge_ports::{TerminalEvent, TerminalId, TerminalPort, TerminalSize};
use serde::Deserialize;

use crate::panel_frame::PanelFrame;
use forge_shared::tr;

// What the page tells Rust: its size (columns and rows) and what the person typed.
#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum FromPage {
    Size { cols: u16, rows: u16 },
    Data { d: String },
}

// The page side of one tab: an xterm.js in the element `__HOST__`, kept to its size, wired to the
// Rust side with `dioxus.send` / `dioxus.recv`. A hidden tab (`display: none`) has no size and
// reports nothing; a size is sent 150 ms after the last change. It cleans itself up when its element leaves the page.
const BRIDGE_JS: &str = r#"
const host = document.getElementById("__HOST__");
const term = new Terminal({ fontFamily: 'ui-monospace, "SF Mono", Menlo, monospace', fontSize: 12, cursorBlink: true });
const fit = new FitAddon.FitAddon();
term.loadAddon(fit);
term.open(host);
// WebKit's Korean input method rewrites the syllable under construction with an
// insertReplacementText that xterm.js does not turn into data. Send the difference ourselves:
// backspaces for what went, then what came.
let preIme = "";
host.addEventListener("beforeinput", (e) => { if (e.target === term.textarea) preIme = term.textarea.value; }, true);
host.addEventListener("input", (e) => {
    if (e.target !== term.textarea || e.inputType !== "insertReplacementText" || !e.data) return;
    const now = term.textarea.value;
    let i = 0;
    while (i < preIme.length && i < now.length && preIme[i] === now[i]) i++;
    const gone = [...preIme.slice(i)].length, added = now.slice(i);
    if (gone || added) dioxus.send({ t: "data", d: "\x7f".repeat(gone) + added });
}, true);
let last = "";
let timer = null;
function report() {
    if (host.offsetWidth === 0 || host.offsetHeight === 0) return;
    fit.fit();
    const key = term.cols + "x" + term.rows;
    if (key === last) return;
    last = key;
    // The screen follows a drag at once; the shell hears the new size only when it settles, or it
    // would redraw its prompt at every step.
    clearTimeout(timer);
    timer = setTimeout(() => dioxus.send({ t: "size", cols: term.cols, rows: term.rows }), 150);
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
    tap: Option<TapHandle>,
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
            let message = match &error {
                forge_ports::TerminalError::MissingDirectory(path) => tr!("Working directory “{path}” does not exist", path = path.display()),
                other => other.to_string(),
            };
            let _ = page.send(serde_json::json!({ "t": "text", "s": format!("\x1b[31m{message}\x1b[0m\r\n") }));
            phase.set(Phase::Failed);
            return;
        }
    };
    running.set(Some(session.id));
    phase.set(Phase::Running);
    let mut ticker = tokio::time::interval(std::time::Duration::from_millis(250));
    loop {
        tokio::select! {
            from_page = page.recv::<FromPage>() => match from_page {
                Ok(FromPage::Data { d }) => { let _ = terminal.write(session.id, d.as_bytes()); }
                Ok(FromPage::Size { cols, rows }) => { let _ = terminal.resize(session.id, TerminalSize { cols, rows }); }
                Err(_) => return,
            },
            event = session.recv_batch() => match event {
                Some(TerminalEvent::Output(bytes)) => {
                    if let Some(tap) = &tap {
                        let keys = tap.0.output(&bytes);
                        if !keys.is_empty() {
                            let _ = terminal.write(session.id, &keys);
                        }
                    }
                    let b = base64::engine::general_purpose::STANDARD.encode(bytes);
                    let _ = page.send(serde_json::json!({ "t": "out", "b": b }));
                }
                Some(TerminalEvent::Exited(code)) => {
                    if let Some(tap) = &tap {
                        tap.0.shell_ended();
                    }
                    let line = match code {
                        Some(code) => format!("\r\n[{}]\r\n", tr!("Process exited ({code})", code = code)),
                        None => format!("\r\n[{}]\r\n", tr!("Process exited")),
                    };
                    let _ = page.send(serde_json::json!({ "t": "text", "s": line }));
                    running.set(None);
                    phase.set(Phase::Exited(code));
                    return;
                }
                None => return,
            },
            _ = ticker.tick(), if tap.is_some() => {
                if let Some(tap) = &tap {
                    let keys = tap.0.tick();
                    if !keys.is_empty() {
                        let _ = terminal.write(session.id, &keys);
                    }
                }
            },
        }
    }
}

#[component]
fn TerminalTab(tab: TabId, cwd: PathBuf, active: bool, on_restart: EventHandler<TabId>, #[props(default)] tap: Option<TapHandle>, #[props(default)] runs: Option<CodeRuns>) -> Element {
    let workspace = use_workspace();
    let host_id = format!("forge-terminal-{}", tab.0);
    let phase = use_signal(|| Phase::Starting);
    // Not a signal: `use_drop` runs while the component's own signals may already be gone.
    let running = use_hook(|| Rc::new(Cell::new(None::<TerminalId>)));
    // The runs of code blocks that the chat asked for are typed into the active tab only.
    let active_cell = use_hook(|| Rc::new(Cell::new(active)));
    active_cell.set(active);
    let run_tap = use_hook(|| runs.clone().map(|runs| Rc::new(RunTap::new(runs, active_cell.clone()))));
    let tap = tap.or_else(|| run_tap.as_ref().map(|run_tap| TapHandle(run_tap.clone() as Rc<dyn TerminalTap>)));

    {
        let host_id = host_id.clone();
        let running = running.clone();
        use_future(move || {
            let (host_id, cwd, running, tap) = (host_id.clone(), cwd.clone(), running.clone(), tap.clone());
            async move { drive(workspace.terminal(), host_id, cwd, phase, running, tap).await }
        });
    }
    {
        let running = running.clone();
        let run_tap = run_tap.clone();
        use_drop(move || {
            if let Some(tap) = &run_tap {
                tap.shell_ended();
            }
            if let Some(id) = running.get() {
                workspace.terminal().close(id);
            }
        });
    }

    let notice = match phase() {
        Phase::Exited(Some(code)) => Some(tr!("Process exited ({code})", code = code)),
        Phase::Exited(None) => Some(tr!("Process exited").to_string()),
        Phase::Failed => Some(tr!("Could not start the shell").to_string()),
        Phase::Starting | Phase::Running => None,
    };
    rsx! {
        div { class: if active { "terminal-view active" } else { "terminal-view" },
            div { id: "{host_id}", class: "terminal-host" }
            if let Some(notice) = notice {
                div { class: "terminal-exited",
                    span { "{notice}" }
                    button { class: "terminal-restart", r#type: "button", onclick: move |_| on_restart.call(tab), {tr!("Restart")} }
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
    let runs = use_hook(|| workspace.code_runs(id));
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
                button { class: "terminal-tab-label", r#type: "button", onclick: move |_| tabs.write().select(tab), {tr!("Shell {n}", n = index + 1)} }
                button {
                    class: "terminal-tab-close",
                    r#type: "button",
                    title: tr!("Close tab"),
                    "aria-label": tr!("Close tab"),
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
            title: tr!("New terminal"),
            "aria-label": tr!("New terminal"),
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
                    runs: Some(runs.clone()),
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

/// A tab id no dock terminal uses, so the page element ids do not meet.
const SIGN_IN_TAB: TabId = TabId(u64::MAX);

/// One terminal for the sign-in dialog: the same view as a dock tab, started in `cwd`, with a tap that types.
/// Taking the view away (closing the dialog) ends the shell, as closing a dock tab does.
#[component]
pub fn SignInTerminal(cwd: PathBuf, tap: TapHandle) -> Element {
    rsx! {
        div { class: "sign-in-terminal",
            TerminalTab { tab: SIGN_IN_TAB, cwd, active: true, on_restart: move |_| {}, tap: Some(tap) }
        }
    }
}

#[cfg(test)]
mod ime_tests {
    #[test]
    fn bridge_repairs_korean_replacement_text() {
        assert!(super::BRIDGE_JS.contains("insertReplacementText"));
        assert!(super::BRIDGE_JS.contains(r"\x7f"));
    }
}
