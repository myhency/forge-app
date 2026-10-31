use std::time::{Duration, Instant};

use dioxus::prelude::*;
use forge_features::{finished_messages, use_workspace, Active, RunState};
use forge_shared::tr;

// A click on the run button of a code block, or on the terminal button of a result box: the page sends "R\n<pane>\n<block text>" or
// "T\n<pane>". The block text is read from the block's own `pre`, never from an attribute. One handler serves
// every pane; it is replaced each time (`dioxus.send` belongs to the eval that created it).
const WATCH_CODE_RUN: &str = "if (window.__forgeCodeRunHandler) { document.removeEventListener('click', window.__forgeCodeRunHandler); } window.__forgeCodeRunHandler = (event) => { const target = event.target && event.target.closest ? event.target : null; if (!target) return; const paneOf = (el) => { const p = el.closest('[data-pane]'); return p ? p.dataset.pane : '0'; }; const run = target.closest('.code-run'); if (run) { const wrap = run.closest('.code-wrap'); const pre = wrap ? wrap.querySelector('pre') : null; if (pre) dioxus.send('R\\n' + paneOf(run) + '\\n' + pre.innerText); return; } const open = target.closest('.run-open-terminal'); if (open) dioxus.send('T\\n' + paneOf(open)); }; document.addEventListener('click', window.__forgeCodeRunHandler);";

// Draws the result boxes into the chat. It can run any number of times with the same data: a box that is
// already under its block is updated, otherwise one is made. Output and words are set as text, never as html.
const APPLY_RUN_BOXES: &str = r#"window.__forgeApplyRunBoxes = (data) => {
  for (const button of document.querySelectorAll('.code-run')) button.title = data.labels.run;
  for (const pane of data.panes) {
    const root = document.querySelector('[data-pane="' + pane.pane + '"] .messages');
    if (!root) continue;
    for (const run of pane.runs) {
      for (const button of root.querySelectorAll('.code-run[data-hash="' + run.hash + '"]')) {
        const wrap = button.closest('.code-wrap');
        if (!wrap) continue;
        let box = wrap.nextElementSibling && wrap.nextElementSibling.classList.contains('run-box') ? wrap.nextElementSibling : null;
        if (!box) {
          box = document.createElement('div');
          box.className = 'run-box';
          const out = document.createElement('pre');
          out.className = 'run-out';
          const foot = document.createElement('div');
          foot.className = 'run-foot';
          const cwd = document.createElement('span');
          cwd.className = 'run-cwd';
          const meta = document.createElement('span');
          meta.className = 'run-meta';
          const open = document.createElement('button');
          open.className = 'run-open-terminal';
          open.type = 'button';
          open.innerHTML = `__TERMINAL_ICON__`;
          foot.append(cwd, meta, open);
          box.append(out, foot);
          wrap.after(box);
        }
        const out = box.querySelector('.run-out');
        const text = (run.cut ? data.labels.cut + '\n' : '') + run.output;
        if (out.textContent !== text) {
          out.textContent = text;
          out.scrollTop = out.scrollHeight;
        }
        out.hidden = text === '';
        box.querySelector('.run-cwd').textContent = pane.folder;
        const meta = box.querySelector('.run-meta');
        meta.className = 'run-meta';
        const open = box.querySelector('.run-open-terminal');
        open.title = data.labels.terminal;
        if (run.state === 'running') {
          meta.innerHTML = `__PENDING_ICON__`;
          meta.append(' ' + data.labels.running);
          meta.classList.add('running');
        } else if (run.state === 'ended') {
          meta.textContent = data.labels.ended;
        } else if (run.code !== 0) {
          meta.textContent = 'exit ' + run.code;
          meta.classList.add('failed');
        } else {
          meta.textContent = run.ended ? new Date(run.ended).toTimeString().slice(0, 8) : '';
        }
      }
    }
  }
};"#;

#[derive(Debug, PartialEq, Eq)]
enum Click {
    Run { pane: usize, block: String },
    Terminal { pane: usize },
}

// "R\n<pane>\n<block text>" or "T\n<pane>"; anything else is not ours.
fn read_click(text: &str) -> Option<Click> {
    let mut parts = text.splitn(3, '\n');
    let kind = parts.next()?;
    let pane = parts.next()?.parse().ok()?;
    match kind {
        "R" => {
            let block = parts.next()?;
            (!block.trim().is_empty()).then(|| Click::Run { pane, block: block.to_string() })
        }
        "T" => Some(Click::Terminal { pane }),
        _ => None,
    }
}

const PANES: usize = 4;

/// Draws nothing. Runs the shell blocks that were clicked in the chat, keeps the result boxes under the blocks up
/// to date, and tells the model when a run is over.
#[component]
pub fn CodeRunWatcher() -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        if workspace.code_run_available() {
            document::eval("document.documentElement.dataset.codeRun = 'on';");
        }
        document::eval(&APPLY_RUN_BOXES.replace("__TERMINAL_ICON__", forge_shared::TERMINAL_ICON_SVG).replace("__PENDING_ICON__", forge_shared::PENDING_ICON_SVG));
        let mut clicks = document::eval(WATCH_CODE_RUN);
        let mut ticker = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                click = clicks.recv::<String>() => {
                    let Ok(text) = click else { break };
                    match read_click(&text) {
                        Some(Click::Run { pane, block }) => {
                            if let Some(Active::Live(id)) = workspace.pane_target(pane) {
                                workspace.run_code(id, &block);
                            }
                        }
                        Some(Click::Terminal { pane }) => {
                            if let Some(Active::Live(id)) = workspace.pane_target(pane) {
                                workspace.open_terminal_panel(id);
                            }
                        }
                        None => {}
                    }
                }
                _ = ticker.tick() => {
                    let sessions = workspace.code_run_sessions();
                    for (id, runs) in &sessions {
                        runs.expire(Instant::now());
                        let finished = runs.take_finished();
                        if finished.is_empty() {
                            continue;
                        }
                        // A session that is gone gets nothing; the run stays in the box.
                        if let Some(handle) = workspace.handle(*id) {
                            for message in finished_messages(finished) {
                                handle.queue(message, Vec::new());
                            }
                        }
                    }
                    let mut panes = Vec::new();
                    for pane in 0..PANES {
                        let Some(Active::Live(id)) = workspace.pane_target(pane) else { continue };
                        let Some((_, runs)) = sessions.iter().find(|(session, _)| *session == id) else { continue };
                        let views = runs.snapshot();
                        if views.is_empty() {
                            continue;
                        }
                        let folder = workspace.live_view(id).map(|view| view.read().project_path.display().to_string()).unwrap_or_default();
                        let runs: Vec<_> = views
                            .iter()
                            .map(|view| {
                                let (state, code) = match view.state {
                                    RunState::Waiting | RunState::Running => ("running", 0),
                                    RunState::Done { code } => ("done", code),
                                    RunState::Ended => ("ended", 0),
                                };
                                serde_json::json!({ "hash": view.hash, "state": state, "code": code, "output": view.output, "cut": view.cut, "ended": view.ended_ms })
                            })
                            .collect();
                        panes.push(serde_json::json!({ "pane": pane, "folder": folder, "runs": runs }));
                    }
                    if !panes.is_empty() {
                        let labels = serde_json::json!({
                            "run": tr!("Run"),
                            "running": tr!("Running"),
                            "ended": tr!("Ended"),
                            "terminal": tr!("Open terminal"),
                            "cut": tr!("(earlier output cut)"),
                        });
                        let data = serde_json::json!({ "labels": labels, "panes": panes });
                        document::eval(&format!("window.__forgeApplyRunBoxes({data});"));
                    }
                }
            }
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_click_message_is_read_as_a_run_or_a_terminal_request() {
        assert_eq!(read_click("R\n1\necho hi\nls"), Some(Click::Run { pane: 1, block: "echo hi\nls".to_string() }));
        assert_eq!(read_click("T\n0"), Some(Click::Terminal { pane: 0 }));
        assert_eq!(read_click("R\nx\necho"), None);
        assert_eq!(read_click("R\n0\n"), None);
        assert_eq!(read_click("Z\n0"), None);
        assert_eq!(read_click(""), None);
    }

    #[test]
    fn the_handler_names_the_pane_and_reads_the_block_text_from_the_pre() {
        assert!(WATCH_CODE_RUN.contains("removeEventListener('click'"), "replaces its old self");
        assert!(WATCH_CODE_RUN.contains(".code-run"));
        assert!(WATCH_CODE_RUN.contains(".run-open-terminal"));
        assert!(WATCH_CODE_RUN.contains("closest('[data-pane]')"));
        assert!(WATCH_CODE_RUN.contains("dioxus.send"));
        assert!(WATCH_CODE_RUN.contains("closest('.code-wrap')") && WATCH_CODE_RUN.contains("querySelector('pre')"), "the text comes from the pre");
        assert!(!WATCH_CODE_RUN.contains("dataset.text"), "no block text in an attribute");
    }

    #[test]
    fn the_box_script_finds_its_box_before_it_makes_one() {
        let find = APPLY_RUN_BOXES.find("querySelector(':scope + .run-box')").or_else(|| APPLY_RUN_BOXES.find("nextElementSibling")).expect("looks for an existing box");
        let make = APPLY_RUN_BOXES.find("createElement").expect("makes one");
        assert!(find < make, "idempotent: look first, then create");
        assert!(APPLY_RUN_BOXES.contains("textContent"), "output is set as text, never as html");
        // Markup is only ever given the two fixed icons (placeholders swapped for our own constants), never output or words.
        assert_eq!(APPLY_RUN_BOXES.matches("innerHTML").count(), 2);
        assert!(APPLY_RUN_BOXES.contains("innerHTML = `__TERMINAL_ICON__`") && APPLY_RUN_BOXES.contains("innerHTML = `__PENDING_ICON__`"), "no html from the output");
    }
}
