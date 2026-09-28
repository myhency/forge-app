# Workspace Walking Skeleton Implementation Plan (Plan 5)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A real Dioxus desktop window that shows every Claude Code session in a sidebar grouped by project, opens past sessions read-only, starts and resumes sessions, runs **several sessions at the same time**, and chats with streaming text, one-line tool calls, permission prompts and interrupt — all through the existing adapters and use-cases.

**Architecture:** One tokio task per open session (`LiveSession`, an actor) owns the conversation and its sidecar and publishes its state through a `tokio::sync::watch` channel; a `SessionRegistry` opens, de-duplicates and closes them; a Dioxus hook copies each published state into a signal on the UI thread. Two new ports (`ProjectRepositoryPort`, `SidecarFactoryPort`) keep `forge-application` free of concrete adapters. Six thin frontend crates follow the spec's FSD crate graph, and `forge-app` is the composition root.

**Tech Stack:** Rust 2021; Dioxus **`=0.7.10`** (pinned to match the installed `dx` 0.7.10); `tokio` (`sync`, `rt`, `macros`); `time` 0.3.55; the existing `forge-adapter-*` crates; the Node 22.22.1 sidecar built with `npm ci && npm run build`.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §15 (with §12–§14 for what it builds on). Read §15 first: it records what was verified against the real machine and Claude Code desktop's documentation, and which choices are this design's own.

## Status of this plan — read before starting

- Written 2026-09-29 for execution in a later session. Work happens on branch **`plan-5`** in the worktree `/Users/james/orca/workspaces/forge/plan-6` (the directory name is historical; do not rename it).
- **How it was validated.** Every Rust/CSS/JS block below was assembled from files that were compiled and tested in a scratch copy of this repository. Every "scaffold + tests" state in Tasks 2–9 was run and fails exactly as the plan says, and every implementation was run and passes (workspace total 244). The tests for Tasks 3, 5 and 6 and for the sidebar and shortcut logic of Task 8 were additionally mutation-checked: the code was deliberately broken and the named test failed (the tables in those tasks list them). Tasks 2, 4, 7 and 9 had the red-then-green cycle only. The async tests in Task 5 and 6 ran 100 times in parallel and 30 times single-threaded with no failure or hang. The **Dioxus components, hook, page and app were only compiled — never run** (the authoring environment has no window). Task 1 (spike) and Task 10's first `dx serve` are the first time they meet a real webview: expect small runtime fixes there and record them as amendments.
- **Expected workspace test counts** (run `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'`): baseline **133**; after Task 2 → 140; Task 3 → 155; Task 4 → 166; Task 5 → 187; Task 6 → **198** (checkpoint); Task 7 → 215; Task 8 → 240; Task 9 → 243; Task 10 → **244**.
- **One build notice is expected and is not a failure:** `warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6` comes from the Dioxus desktop dependency tree. Any other warning is a failure to fix.
- **Amendments.** If the Task 1 spike or a first run contradicts a task, do not improvise: add an `## Amendment A (after Task N)` section below the Review Focus (as Plan 4's plan did), say what changed, and edit the affected tasks *before* executing them. Task 1 lists the amendment each spike outcome calls for.

## Global Constraints

- Crate graph (spec §6, §15.5–§15.6): `forge-application` depends on `forge-domain` + `forge-ports` (plus `tokio`, approved in §15.5) and **never** on an adapter crate; `forge-entities` depends on `forge-domain`, `forge-shared`, Dioxus and small utility crates (`serde_json`, `time`) but **never** on `forge-application`; `forge-features` on `forge-application`/`forge-entities`/`forge-shared`; `forge-widgets` on features/entities/shared/application; `forge-pages` on widgets/features; `forge-app` is the only crate that names concrete adapters and the only one enabling `dioxus/desktop`.
- Dependency pins: `dioxus = "=0.7.10"` everywhere (UI library crates use `default-features = false, features = ["lib"]`; only `forge-app` adds `features = ["desktop"]`); `time = "0.3.55"`; `tokio = "1"`. Commit `Cargo.lock` with the task that first changes it (Task 7).
- Tests never touch the real `~/.claude/`, `~/.claude.json` or the real Forge config. The single exception is Task 3's read-only smoke example, which is **never committed**.
- Error strings that come from a path include it: `format!("{}: {}", path.display(), e)` (Plan 3 convention).
- **Never format or log an `AppConfig`** (its `Debug` output contains the API key). `SidecarProcessFactory` builds environment pairs from the fields directly.
- The sidecar needs `npm ci && npm run build` in `sidecar/` before anything runs it (`sidecar/dist/` and `node_modules/` are git-ignored). Node must be **22.22.1** (`sidecar/package.json` `engines`).
- Commands assume `cargo` is on `PATH`; if a fresh shell cannot find it, prefix with `PATH="$HOME/.cargo/bin:$PATH"`.
- Commit messages follow the repo style (`feat: …`, `fix: …`, `docs: …`) and end with the trailer `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>`.
- Own choices, not verified against Claude Code desktop (spec §15.1): the sidebar markers, the folder dropdown, the lazy sidecar (spawned on first send), closing a running session ends its turn, the model selector offering only `default`.

## Review Focus

The failure modes most likely to bite a person using this software. Each is pinned by a named test unless it says otherwise.

1. **A project whose folder was deleted** (a cleaned-up worktree or a deleted scratch directory — 8 of this machine's 12 projects on 2026-09-29) must still appear in the sidebar and open read-only, must not be resumable, and must not be offered as a folder for a new session → Task 3 `list_projects_recovers_the_real_path_from_the_cwd_not_the_lossy_directory_name` (`exists == false`), Task 5 `sending_to_a_session_whose_folder_is_missing_sets_an_error_and_never_spawns`, Task 6 `open_new_rejects_a_missing_folder_and_registers_nothing` and `open_existing_for_a_missing_folder_is_view_only`.
2. **The same Claude session opened twice must never get two sidecars** (two processes writing one transcript), even when both opens race → Task 6 `opening_an_already_open_session_returns_the_same_handle_without_loading_again` and `two_concurrent_opens_of_the_same_session_end_up_with_one_handle`.
3. **A sidecar that dies or reports an error mid-turn must not wedge the session or leave a message that was never delivered in the history** → Task 5 `an_error_event_shuts_the_sidecar_down_clears_permissions_and_fails_the_session`, `sending_after_a_failure_respawns_and_resumes_with_the_known_session_id`, `a_failed_write_removes_the_undelivered_message_from_the_history`, `the_event_stream_ending_fails_the_session_without_asking_the_dead_process_to_shut_down`.
4. **Project directory names are a lossy escape and transcripts are shared with a running Claude Code**: a path with spaces, underscores or Korean characters, a name over 200 characters, a `cwd` that is not on the first line, a half-written final line, invalid UTF-8 → Task 3 (`…_lossy_directory_name`, `…_200_character_cap`, `…_not_on_the_first_line`, `…_tolerates_invalid_utf8_and_a_partial_final_line`, `…_does_not_round_trip…`).
5. **Pressing Enter to confirm a Korean (IME) syllable must not send the message** — a webview event, so **no automated test**; the spike logs `composing=` for exactly this (Task 1 Step 6) and Task 11's checklist repeats it. `SendMessageBox` and `NewSessionComposer` already check `is_composing()`.

**Known limitations, accepted and not tested:** every published change and every render clones the whole conversation (fine for normal sessions; a multi-megabyte tool output will make typing sluggish — watch for it in Task 11); the message list does not auto-scroll while streaming; a fatal and a non-fatal sidecar `Error` cannot be told apart, so every `Error` ends the sidecar (spec §15.5); Windows and Linux are unverified; the sidebar reflects sessions created elsewhere only at start and when a session closes.

---

## File Structure

New files are marked (new). Existing files are edited only where listed.

| Task | Crate | Files |
|---|---|---|
| 1 | `crates/forge-spike/` (throwaway, never committed) | `Cargo.toml`, `src/main.rs` |
| 2 | `forge-domain` | `src/project.rs` (new), `src/lib.rs`, `src/session.rs` (`Hash` on `SessionId`) |
| 2 | `forge-ports` | `src/project_port.rs` (new), `src/sidecar_factory_port.rs` (new), `src/lib.rs` |
| 3 | `forge-adapter-claude-fs` | `src/projects.rs` (new), `src/lib.rs`, `src/sessions.rs` (`pub(crate) fn project_dir`); `examples/smoke_projects.rs` (new, **not committed**) |
| 4 | `forge-adapter-sidecar` | `src/factory.rs` (new), `src/process.rs` (`spawn` takes an environment), `src/lib.rs`, `Cargo.toml`, `tests/factory_test.rs` (new), `tests/contract_test.rs`, `tests/fixtures/fake_sidecar.js`, `examples/spike_drive_sidecar.rs` |
| 5 | `forge-application` | `src/live_session.rs` (new), `src/live_session_actor.rs` (new), `src/test_support.rs` (new, test-only), `src/lib.rs`, `Cargo.toml` |
| 6 | `forge-application` | `src/registry.rs` (new), `src/lib.rs` |
| 7 | `forge-shared` (new crate) | `Cargo.toml`, `src/lib.rs`, `src/theme.rs`, `src/theme.css`, `src/components.rs` |
| 7 | `forge-entities` (new crate) | `Cargo.toml`, `src/lib.rs`, `src/tool_summary.rs`, `src/relative_time.rs`, `src/message.rs`, `src/permission.rs`, `src/sidebar_item.rs` |
| 8 | `forge-features` (new crate) | `Cargo.toml`, `src/lib.rs`, `src/sidebar.rs`, `src/shortcut.rs`, `src/permission_mode.rs`, `src/workspace.rs`, `src/send_message.rs`, `src/approve_permission.rs`, `src/new_session.rs` |
| 9 | `forge-widgets` (new crate) | `Cargo.toml`, `src/lib.rs`, `src/sidebar.rs`, `src/chat_window.rs`, `src/new_session_panel.rs` |
| 9 | `forge-pages` (new crate) | `Cargo.toml`, `src/lib.rs`, `src/workspace_page.rs` |
| 10 | `forge-app` (new crate) | `Cargo.toml`, `src/main.rs` |
| 11 | docs | `docs/superpowers/specs/2026-09-27-forge-design.md` (§15 as built) |

The workspace `Cargo.toml` uses `members = ["crates/*"]`, so new crates need no registration.

---

### Task 1: Spike — does the Dioxus ↔ sidecar path work? (throwaway, never committed)

**Files:**
- Create (uncommitted, deleted at the end): `crates/forge-spike/Cargo.toml`, `crates/forge-spike/src/main.rs`
- Modify (committed): `docs/superpowers/specs/2026-09-27-forge-design.md` (new `### 15.10 Spike findings`), and this plan if a finding needs an amendment

**Interfaces:**
- Consumes: the repository exactly as it stands after Plan 4 (`SidecarProcess::spawn(node, script)` still takes **two** arguments here; Task 4 adds the third).
- Produces: written answers to questions (a)–(g) below. Nothing that later tasks import.

This mirrors the spike that opened Plan 1 (spec §10): the pieces are individually tested, but the risk is where they meet, and only running the real thing shows it. Every question below can change a later task, so do not skip it or start Task 2 first. Nothing in this task is *expected* to cost money except Step 7's real turn, which needs the user's consent. (Step 6 items 2 and 3 would make a real one-word call if the SDK ignored the environment variable it is being tested with — that would itself be the finding, and costs a fraction of a cent.)

| | Question | Why it matters |
|---|---|---|
| (a) | Does the Dioxus desktop UI thread have a tokio runtime, so `SidecarProcess::spawn` (`tokio::spawn`, `tokio::process`) and `SessionRegistry::open_new` work from an event handler? | Tasks 5, 6, 8 call `tokio::spawn` from UI event handlers. |
| (b) | Does real sidecar stream → tokio task → `watch` → Dioxus-local task → `Signal` update the window live while it stays responsive? | The whole state bridge (§15.6) rests on it. |
| (c) | Do ⌘N, ⌘W, Ctrl+Tab, Ctrl+Shift+Tab reach `onkeydown` on macOS, or does the native menu swallow them? And does an Enter that confirms a Korean syllable report `composing=true`? | Decides Task 8's shortcut route and the Enter-to-send guard. |
| (d) | Does the SDK honour `ANTHROPIC_API_KEY` and `ANTHROPIC_BASE_URL` from the spawn environment? What happens with neither? | Task 4's `credentials_env` names. |
| (e) | Does `dx serve -p <workspace member> --platform desktop` work in this workspace? | The run command for Task 10 and the checklist. |
| (f) | Does `session_started` arrive before the first user message is sent, or only after? | Only informs; the design works either way. |
| (g) | Which events follow an `interrupt`? Does a `turn_complete` arrive? | Task 5 assumes the UI returns to `Idle` on `turn_complete`. |

- [ ] **Step 1: Confirm the starting point**

```bash
cd /Users/james/orca/workspaces/forge/plan-6
git status -sb
git log --oneline -3
cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s " tests passed"}'
node --version
dx --version
```

Expected: `## plan-5` with no changes; `133 tests passed`; `v22.22.1`; `dioxus 0.7.10 (…)`. If the working tree is not clean or the count differs, stop and find out why before going on.

- [ ] **Step 2: Build the sidecar** (this worktree has no `node_modules` or `dist`)

```bash
cd sidecar && npm ci && npm run build && npm test && cd ..
ls sidecar/dist
```

Expected: `npm ci` ends with `found 0 vulnerabilities`; `Tests  18 passed (18)`; `ls` lists `index.js io.js protocol.js session.js`. Both directories are git-ignored.

- [ ] **Step 3: Create the throwaway spike crate, and make sure it can never be committed**

```bash
mkdir -p crates/forge-spike/src
echo 'crates/forge-spike/' >> "$(git rev-parse --git-path info/exclude)"
```

`crates/forge-spike/Cargo.toml`:

```toml
[package]
name = "forge-spike"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
forge-adapter-sidecar = { path = "../forge-adapter-sidecar" }
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
dioxus = { version = "=0.7.10", features = ["desktop"] }
tokio = { version = "1", features = ["sync", "rt"] }
```

`crates/forge-spike/src/main.rs`:

```rust
// THROWAWAY spike (spec §15.8, Task 1). Never committed; deleted when the findings are written up.
// One window that drives the real sidecar and logs what happens, to answer the questions in
// Task 1 of the plan.
use std::sync::Arc;
use std::time::Instant;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use forge_adapter_sidecar::SidecarProcess;
use forge_domain::PermissionMode;
use forge_ports::{SidecarPort, StartSessionParams};
use tokio::sync::watch;

const SIDECAR_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecar/dist/index.js");

fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            Config::new().with_window(
                WindowBuilder::new()
                    .with_title("Forge spike")
                    .with_inner_size(LogicalSize::new(980.0, 720.0)),
            ),
        )
        .launch(App);
}

#[component]
fn App() -> Element {
    let mut log = use_signal(Vec::<String>::new);
    let mut last_key = use_signal(|| "(press a key)".to_string());
    let mut session = use_signal(|| None::<Arc<SidecarProcess>>);
    let started = use_hook(Instant::now);

    let stamp = move |line: String| format!("[{:>7.2}s] {line}", started.elapsed().as_secs_f32());
    let mut note = move |line: String| log.write().push(stamp(line));

    // Sets `env` for the child only (question d), spawns a sidecar, starts a session on it and
    // bridges its events to the log (questions a, b, f).
    let mut start = move |label: &str, env: &[(&str, &str)]| {
        note(format!("== {label}"));
        // (a) is there a tokio runtime on the UI thread? SidecarProcess::spawn needs one.
        note(format!(
            "(a) tokio runtime handle available in this handler: {}",
            tokio::runtime::Handle::try_current().is_ok()
        ));
        for (name, value) in env {
            std::env::set_var(name, value);
        }
        let spawned = SidecarProcess::spawn("node", SIDECAR_SCRIPT);
        for (name, _) in env {
            std::env::remove_var(name);
        }
        let process = match spawned {
            Ok(process) => Arc::new(process),
            Err(error) => {
                note(format!("spawn failed: {error}"));
                return;
            }
        };
        session.set(Some(process.clone()));

        // Events are read on tokio and published through a watch channel; a Dioxus-local task
        // copies them into the log (the bridge the real app will use).
        let (tx, mut rx) = watch::channel(Vec::<String>::new());
        let events = process.clone();
        tokio::spawn(async move {
            while let Some(event) = events.next_event().await {
                let line = format!("[{:>7.2}s] event: {event:?}", started.elapsed().as_secs_f32());
                tx.send_modify(|lines| lines.push(line));
            }
            let line = format!("[{:>7.2}s] event stream ended", started.elapsed().as_secs_f32());
            tx.send_modify(|lines| lines.push(line));
        });
        spawn(async move {
            let mut shown = 0;
            while rx.changed().await.is_ok() {
                let lines = rx.borrow_and_update().clone();
                for line in lines.iter().skip(shown) {
                    log.write().push(line.clone());
                }
                shown = lines.len();
            }
        });

        spawn(async move {
            let params = StartSessionParams {
                cwd: std::env::temp_dir().to_string_lossy().to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
            };
            let result = process.start_session(params).await;
            log.write().push(stamp(format!("start_session sent: {result:?}")));
        });
    };

    let mut send = move |text: &'static str| {
        let Some(process) = session() else {
            note("no session: start one first".to_string());
            return;
        };
        note(format!("sending: {text}"));
        spawn(async move {
            let result = process.send_message(text.to_string()).await;
            log.write().push(stamp(format!("send_message returned: {result:?}")));
        });
    };

    rsx! {
        div {
            tabindex: "0",
            style: "font-family: ui-monospace, Menlo, monospace; font-size: 12px; padding: 12px; outline: none;",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            // (c) which key presses reach the page? Nothing is prevented here on purpose.
            onkeydown: move |event| {
                let modifiers = event.modifiers();
                let line = format!(
                    "keydown key={:?} meta={} ctrl={} shift={} alt={} composing={}",
                    event.key(),
                    modifiers.meta(),
                    modifiers.ctrl(),
                    modifiers.shift(),
                    modifiers.alt(),
                    event.is_composing()
                );
                last_key.set(line.clone());
                note(line);
            },
            div { style: "margin-bottom: 8px;",
                button { onclick: move |_| start("start, inheriting this process's environment", &[]), "1. Start (inherit env)" }
                button {
                    onclick: move |_| start(
                        "start with an invalid API key (d: expect an authentication error)",
                        &[("ANTHROPIC_API_KEY", "sk-ant-spike-invalid-key")],
                    ),
                    "2. Start with invalid key"
                }
                button {
                    onclick: move |_| start(
                        "start with an unreachable base URL (d: expect a connection error naming 127.0.0.1:9)",
                        &[("ANTHROPIC_BASE_URL", "http://127.0.0.1:9")],
                    ),
                    "3. Start with unreachable URL"
                }
            }
            div { style: "margin-bottom: 8px;",
                button { onclick: move |_| send("Reply with exactly the single word: pong"), "Send: pong" }
                button { onclick: move |_| send("Count from 1 to 100, one number per line, and write nothing else."), "Send: long" }
                button {
                    onclick: move |_| {
                        let Some(process) = session() else { return };
                        note("interrupt requested".to_string());
                        spawn(async move {
                            let result = process.interrupt().await;
                            log.write().push(stamp(format!("interrupt returned: {result:?}")));
                        });
                    },
                    "Interrupt"
                }
                button {
                    onclick: move |_| {
                        let Some(process) = session() else { return };
                        note("shutdown requested".to_string());
                        spawn(async move {
                            let result = process.shutdown().await;
                            log.write().push(stamp(format!("shutdown returned: {result:?}")));
                        });
                    },
                    "Shutdown"
                }
                button { onclick: move |_| log.write().clear(), "Clear log" }
            }
            div { style: "margin-bottom: 8px; color: #666;",
                "Env in this process: ANTHROPIC_API_KEY set = {std::env::var(\"ANTHROPIC_API_KEY\").is_ok()}, ANTHROPIC_BASE_URL set = {std::env::var(\"ANTHROPIC_BASE_URL\").is_ok()}"
            }
            div { style: "margin-bottom: 8px;", "last key: {last_key}" }
            // (c) IME: type Korean here and press Enter while a syllable is still being composed;
            // the log line for that Enter should say composing=true.
            textarea {
                rows: "2",
                style: "width: 100%; margin-bottom: 8px; font: inherit;",
                placeholder: "type Korean here; press Enter mid-composition",
            }
            div { style: "white-space: pre-wrap;",
                for (index, line) in log.read().iter().enumerate() {
                    div { key: "{index}", "{line}" }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Check that it compiles**

Run: `cargo check -p forge-spike`
Expected: `Finished` with no warnings from this crate (about 30 s the first time: Dioxus desktop is a large dependency tree).

- [ ] **Step 5: Run it as a workspace member — question (e)**

```bash
dx serve -p forge-spike --platform desktop
```

Expected: a window titled "Forge spike" opens. If `dx` rejects the flags or cannot find the package, write down the exact error and use `cargo run -p forge-spike` for the rest of this task. Either way, record whether `dx serve -p <member>` works: Task 10 and Task 11 use that command.

- [ ] **Step 6: Questions that cost nothing — (a), (c), (d), (f)**

In the spike window, in this order, and write down what the log shows each time:

1. Click **1. Start (inherit env)**. Read the `(a) tokio runtime handle available in this handler:` line — **(a)**. Then wait 15 seconds **without sending anything** and note whether a `session_started` event appears and when — **(f)** (expected: starting a session does not call the API, only a message does — that expectation is unverified).
2. Click **3. Start with unreachable URL**, then **Send: pong**. The SDK is pointed at `http://127.0.0.1:9`; if it honours the variable nothing can be reached, so nothing is billed. Expected if `ANTHROPIC_BASE_URL` is honoured: an `Error` event mentioning a refused connection or `127.0.0.1:9` — **(d)**, base URL.
3. Click **2. Start with invalid key**, then **Send: pong**. Expected if `ANTHROPIC_API_KEY` is honoured: an `Error` event about authentication / an invalid key (the API rejects it; nothing is billed) — **(d)**, key. If instead the SDK answers normally, it used other credentials (Claude Code's own login) and ignored the variable — that turn was a real one-word call. Record it; Task 4 must then be amended (see the table below). The same applies to the unreachable-URL step if it answers normally.
4. Keyboard — **(c)**: click into the window's blank area, then press ⌘N, ⌘W, Ctrl+Tab, Ctrl+Shift+Tab and ⌘⇧N, reading the `last key` line and the log after each. **⌘W may close the window; if it does, that is the finding** (the native menu swallowed it) — relaunch and continue. Then click into the text area, switch to the Korean input source, type a syllable but do **not** commit it, and press Enter: the log line for that Enter must say `composing=true`.

- [ ] **Step 7: Ask the user before the last two questions — (b) and (g)**

These make one real API call with the user's own credentials (a one-word answer and a short count that is interrupted; a few cents at most). **Stop and ask for consent first.** With consent:

1. Click **1. Start**, then **Send: pong**. Watch the events arrive in the log while the window stays responsive (try scrolling the log while it runs) — **(b)**.
2. Click **Send: long**; after a few numbers appear click **Interrupt**. Record every event from the click until the log goes quiet, and whether a `TurnComplete` arrives — **(g)**.
3. Click **Shutdown** and confirm `session_ended` (or the stream ending) appears.

- [ ] **Step 8: Write the findings down**

Append `### 15.10 Spike findings (Task 1)` to the end of `docs/superpowers/specs/2026-09-27-forge-design.md`: one line per question (a)–(g) with what was observed, dated, plus the exact `dx serve` command that worked. Then apply the amendments the outcomes call for:

| Outcome | Amendment to make before continuing |
|---|---|
| (a) no tokio runtime in event handlers | Build one multi-thread runtime in `forge-app`'s `main()` and keep it alive; pass its `tokio::runtime::Handle` to `SessionRegistry::new` and `SidecarProcessFactory::new`; use `handle.spawn(…)` in `spawn_live_session` and `let _guard = handle.enter();` around `SidecarProcess::spawn`. Rewrite Tasks 4–6 and 10 accordingly (constructor signatures, tests pass a `Handle::current()`). |
| (b) window does not update, or freezes | **Stop.** Report to the user; the bridge design needs rethinking (alternative: `use_coroutine` fed by an `mpsc` channel). |
| (c) a shortcut never reaches `onkeydown` | Keep `Workspace::on_key` for the ones that do; move the missing ones to native menu accelerators in Task 10 (`dioxus::desktop` re-exports `muda`); record which. |
| (c) Enter mid-composition reports `composing=false` | The `is_composing()` guard is useless: also ignore an Enter whose raw key code is 229 (the value browsers report during composition) — amend `SendMessageBox` and `NewSessionComposer`, checking Dioxus's `KeyboardEvent` for the accessor. |
| (d) key or URL variable not honoured | Change the names in Task 4's `credentials_env` and its tests; if the SDK cannot take a key from the environment at all, stop and report. |
| (f) `session_started` arrives before any message | No change (it only helps: a new session learns its id sooner). |
| (g) no `TurnComplete` after `interrupt` | Apply the code under the table when executing Task 5. |

**If (g) found that no `TurnComplete` follows an interrupt**, make these two changes while executing Task 5 (both were compiled and tested against the finished Task 5 code; without the first, the second fails). In `live_session_actor.rs` replace the whole `interrupt` method with:

```rust
    async fn interrupt(&mut self) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        match InterruptTurnUseCase::new(sidecar).execute().await {
            Ok(()) => {
                // Amendment (Task 1, question g): the sidecar sends no `turn_complete` after an
                // interrupt, so the session must not wait for one.
                self.view.phase = Phase::Idle;
                self.view.pending_permissions.clear();
                self.dirty = true;
            }
            Err(error) => self.fail(error.to_string(), false).await,
        }
    }
```

and in `live_session.rs`, inside `mod tests`, add before `interrupt_reaches_the_sidecar_and_is_a_no_op_without_one`:

```rust
    #[tokio::test]
    async fn interrupt_returns_the_session_to_idle_even_if_no_turn_complete_arrives() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        control.push(permission_request("p-1"));
        wait_for(&mut view, "a queued request", |v| !v.pending_permissions.is_empty()).await;

        handle.interrupt();
        let seen = wait_for(&mut view, "idle after the interrupt", |v| v.phase == Phase::Idle).await;

        assert!(seen.pending_permissions.is_empty());
        assert!(control.calls().contains(&Call::Interrupt));
    }
```

Task 5's counts then rise by one (188 after Task 5, 199 after Task 6, 245 at the end).

- [ ] **Step 9: Delete the spike and commit only the findings**

```bash
rm -rf crates/forge-spike
git checkout -- Cargo.lock
git status --short
```

Expected: only the spec (and, if amended, this plan) show as modified; no `forge-spike` entry, and `Cargo.lock` is unchanged.

```bash
git add docs/superpowers
git commit -m "docs: record the Task 1 spike findings for Plan 5

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 2: `ProjectSummary`, `SessionId: Hash`, and the two new ports

**Files:**
- Create: `crates/forge-domain/src/project.rs`, `crates/forge-ports/src/project_port.rs`, `crates/forge-ports/src/sidecar_factory_port.rs`
- Modify: `crates/forge-domain/src/lib.rs`, `crates/forge-domain/src/session.rs`, `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: the existing `SessionId`, `SidecarPort`, `SidecarError` and the `mocks` feature pattern of `forge-ports` (`#[cfg_attr(any(test, feature = "mocks"), automock)]`).
- Produces: `forge_domain::ProjectSummary { path: PathBuf, last_active: String, session_count: usize, exists: bool }` (`Debug, Clone, PartialEq, Eq`); `SessionId` gains `Hash`; `forge_ports::{ProjectRepositoryPort, ProjectRepositoryError, SidecarFactoryPort}` and their mocks `MockProjectRepositoryPort` / `MockSidecarFactoryPort`. `ProjectRepositoryPort::list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError>`; `SidecarFactoryPort::spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError>` (async).

`SessionId` needs `Hash` because the sidebar de-duplicates sessions by id (Task 8). `ProjectSummary.exists` is computed by the adapter so `forge-application` performs no filesystem I/O (spec §15.3).

- [ ] **Step 1: Write the failing test for `SessionId: Hash`**

In `crates/forge-domain/src/session.rs`, inside `mod tests`, replace

```rust
    #[test]
    fn session_id_equality_is_value_based() {
```

with

    #[test]
    fn session_id_can_key_a_hash_set() {
        let mut seen = std::collections::HashSet::new();
        assert!(seen.insert(SessionId("s-1".to_string())));
        assert!(!seen.insert(SessionId("s-1".to_string())));
        assert!(seen.insert(SessionId("s-2".to_string())));
    }

    #[test]
    fn session_id_equality_is_value_based() {
- [ ] **Step 2: Run it and see it fail to compile**

Run: `cargo test -p forge-domain`
Expected: a compile error in `session.rs` — the method `insert` exists for `HashSet<SessionId>` but its trait bounds were not satisfied (`SessionId: Hash`).

- [ ] **Step 3: Derive `Hash`**

In `crates/forge-domain/src/session.rs` replace

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionId(pub String);
```

with

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionId(pub String);
```

Run: `cargo test -p forge-domain` — Expected: `11 passed`.

- [ ] **Step 4: Add `ProjectSummary`**

`crates/forge-domain/src/project.rs` (a plain data type; its two tests pin equality and `Clone`):

```rust
use std::path::PathBuf;

/// One project directory that has at least one Claude Code transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    /// The real project path, recovered from a transcript's `cwd` (never the escaped directory name).
    pub path: PathBuf,
    /// RFC 3339 modification time of the project's newest transcript.
    pub last_active: String,
    pub session_count: usize,
    /// Whether `path` is a directory on disk right now (a session cannot be started in a missing one).
    pub exists: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(path: &str) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 3,
            exists: true,
        }
    }

    #[test]
    fn project_summary_equality_is_field_based() {
        assert_eq!(summary("/work/a"), summary("/work/a"));
        assert_ne!(summary("/work/a"), summary("/work/b"));
    }

    #[test]
    fn project_summary_clone_keeps_every_field() {
        let original = summary("/work/a");
        assert_eq!(original.clone(), original);
    }
}
```

Replace `crates/forge-domain/src/lib.rs` with:

```rust
mod config;
mod extension;
mod permission;
mod project;
mod session;

pub use config::AppConfig;
pub use extension::{McpServerDefinition, PluginDefinition, SkillDefinition};
pub use permission::{PermissionDecision, PermissionMode};
pub use project::ProjectSummary;
pub use session::{
    Message, MessageRole, PermissionRequest, Session, SessionId, SessionSummary, ToolCall,
    ToolResult,
};
```

Run: `cargo test -p forge-domain` — Expected: `13 passed`.

- [ ] **Step 5: Add the two ports**

`crates/forge-ports/src/project_port.rs`:

```rust
use async_trait::async_trait;
use forge_domain::ProjectSummary;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum ProjectRepositoryError {
    #[error("failed to list projects: {0}")]
    ListFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ProjectRepositoryPort: Send + Sync {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_project_repository_port_can_record_a_list_projects_call() {
        let mut mock = MockProjectRepositoryPort::new();
        mock.expect_list_projects().times(1).returning(|| Ok(vec![]));

        let result = mock.list_projects().await;

        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn list_failed_message_names_the_cause() {
        let error = ProjectRepositoryError::ListFailed("boom".to_string());
        assert_eq!(error.to_string(), "failed to list projects: boom");
    }
}
```

`crates/forge-ports/src/sidecar_factory_port.rs`:

```rust
use std::sync::Arc;

use async_trait::async_trait;

use crate::sidecar_port::{SidecarError, SidecarPort};

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Starts a fresh sidecar process for one session. `SidecarPort` itself stays single-session
/// (spec §3: one sidecar per open session); this port is how the application layer gets a new one.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SidecarFactoryPort: Send + Sync {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MockSidecarPort;

    #[tokio::test]
    async fn mock_sidecar_factory_port_can_hand_out_a_sidecar() {
        let mut factory = MockSidecarFactoryPort::new();
        factory
            .expect_spawn()
            .times(1)
            .returning(|| Ok(Arc::new(MockSidecarPort::new())));

        let sidecar = factory.spawn().await;

        assert!(sidecar.is_ok());
    }

    #[tokio::test]
    async fn mock_sidecar_factory_port_can_fail_to_spawn() {
        let mut factory = MockSidecarFactoryPort::new();
        factory
            .expect_spawn()
            .times(1)
            .returning(|| Err(SidecarError::SpawnFailed("no node".to_string())));

        let result = factory.spawn().await;

        assert!(matches!(result, Err(SidecarError::SpawnFailed(_))));
    }
}
```

Replace `crates/forge-ports/src/lib.rs` with:

```rust
mod config_port;
mod extension_ports;
mod project_port;
mod session_port;
mod sidecar_factory_port;
mod sidecar_port;

pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use extension_ports::{
    McpRegistryError, McpRegistryPort, PluginRepositoryError, PluginRepositoryPort,
    SkillRepositoryError, SkillRepositoryPort,
};
pub use project_port::{ProjectRepositoryError, ProjectRepositoryPort};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_factory_port::SidecarFactoryPort;
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use extension_ports::{MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort};
#[cfg(any(test, feature = "mocks"))]
pub use project_port::MockProjectRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use session_port::MockSessionRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_factory_port::MockSidecarFactoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
```

These files are trait and type definitions; their tests pin the mock wiring and the error message, so there is no separate "fails first" step.

Run: `cargo test -p forge-ports` — Expected: `10 passed`.

- [ ] **Step 6: Whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'`
Expected: `140`, and no warnings other than the `block v0.1.6` notice.

- [ ] **Step 7: Commit**

```bash
git add crates/forge-domain crates/forge-ports
git commit -m "feat: add ProjectSummary, SessionId: Hash, ProjectRepositoryPort and SidecarFactoryPort

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 3: `ClaudeProjectAdapter::list_projects`

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/projects.rs`; `crates/forge-adapter-claude-fs/examples/smoke_projects.rs` (**never committed**)
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs`, `crates/forge-adapter-claude-fs/src/sessions.rs`

**Interfaces:**
- Consumes: `forge_ports::{ProjectRepositoryPort, ProjectRepositoryError}`; `forge_domain::ProjectSummary`; `ClaudeSessionAdapter::project_dir(&self, &Path) -> PathBuf` (made `pub(crate)` here); `SessionRepositoryPort::list_sessions` (in the smoke check).
- Produces: `pub struct ClaudeProjectAdapter` with `pub fn new(claude_home: PathBuf) -> Self`, implementing `ProjectRepositoryPort`, re-exported from `lib.rs`.

Behaviour (spec §15.4): read `<claude_home>/projects/*`; a directory counts if it holds at least one `*.jsonl`; its real path is the first `cwd` found scanning transcripts **newest first**, at most the first 60 lines of each, skipping lines that are not valid JSON; the directory is kept only if `ClaudeSessionAdapter::project_dir(recovered path)` gives that same directory (so `list_sessions(project.path)` lists exactly it); `exists` = the recovered path is a directory now; sorted by newest transcript time descending, ties by path; a missing `projects` directory → empty list; a read error on it → `ListFailed("<path>: <error>")`.

- [ ] **Step 1: Let the project adapter reuse the session adapter's path rule, and wire the module**

In `crates/forge-adapter-claude-fs/src/sessions.rs` replace

```rust
    async fn project_dir(&self, project_path: &Path) -> PathBuf {
```

with

```rust
    pub(crate) async fn project_dir(&self, project_path: &Path) -> PathBuf {
```

In `crates/forge-adapter-claude-fs/src/lib.rs` replace `mod mcp;` with

```rust
mod mcp;
mod projects;
```

and replace `pub use mcp::ClaudeMcpAdapter;` with

```rust
pub use mcp::ClaudeMcpAdapter;
pub use projects::ClaudeProjectAdapter;
```

- [ ] **Step 2: Create `projects.rs` with the scaffold and all tests**

`crates/forge-adapter-claude-fs/src/projects.rs` is **one file**: the scaffold below, immediately followed by the test module.

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::ProjectSummary;
use forge_ports::{ProjectRepositoryError, ProjectRepositoryPort};

// How many lines of one transcript are searched for a `cwd`.
const MAX_CWD_SCAN_LINES: usize = 60;

pub struct ClaudeProjectAdapter {
    claude_home: PathBuf,
}

impl ClaudeProjectAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[async_trait]
impl ProjectRepositoryPort for ClaudeProjectAdapter {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError> {
        let _ = &self.claude_home;
        unimplemented!("implemented in Task 3, Step 4")
    }
}
```

```rust

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    use forge_domain::SessionId;
    use forge_ports::SessionRepositoryPort;

    use super::*;
    use crate::sessions::ClaudeSessionAdapter;

    fn at(seconds: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
    }

    fn cwd_line(cwd: &Path) -> String {
        serde_json::json!({
            "type": "user",
            "cwd": cwd.to_string_lossy(),
            "message": { "role": "user", "content": "hi" }
        })
        .to_string()
    }

    fn set_modified(path: &Path, modified: SystemTime) {
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(modified)
            .unwrap();
    }

    // Writes `<home>/projects/<dir_name>/<id>.jsonl`.
    fn write_transcript_in(home: &Path, dir_name: &str, id: &str, content: &[u8], modified: SystemTime) {
        let dir = home.join("projects").join(dir_name);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        std::fs::write(&path, content).unwrap();
        set_modified(&path, modified);
    }

    // Writes a transcript into the directory Claude Code would use for `project`.
    async fn add_transcript(home: &Path, project: &Path, id: &str, content: &[u8], modified: SystemTime) {
        let dir = ClaudeSessionAdapter::new(home.to_path_buf()).project_dir(project).await;
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        write_transcript_in(home, &name, id, content, modified);
    }

    async fn list(home: &Path) -> Result<Vec<ProjectSummary>, ProjectRepositoryError> {
        ClaudeProjectAdapter::new(home.to_path_buf()).list_projects().await
    }

    #[tokio::test]
    async fn list_projects_returns_empty_when_the_projects_directory_is_missing() {
        let home = tempfile::tempdir().unwrap();

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_recovers_the_real_path_from_the_cwd_not_the_lossy_directory_name() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/my_app one");
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(10)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/my_app one"));
        assert_eq!(projects[0].session_count, 1);
        assert!(!projects[0].exists);
    }

    #[tokio::test]
    async fn list_projects_formats_last_active_as_rfc3339_of_the_newest_transcript() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "old", cwd_line(project).as_bytes(), at(0)).await;
        add_transcript(home.path(), project, "new", cwd_line(project).as_bytes(), at(86_400)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].last_active, "1970-01-02T00:00:00Z");
        assert_eq!(projects[0].session_count, 2);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_with_no_jsonl_files_and_ignores_non_transcript_files() {
        let home = tempfile::tempdir().unwrap();
        let empty = home.path().join("projects").join("-work-empty");
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("notes.txt"), "not a transcript").unwrap();
        std::fs::write(home.path().join("projects").join("stray-file"), "x").unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(1)).await;
        std::fs::write(
            ClaudeSessionAdapter::new(home.path().to_path_buf())
                .project_dir(project)
                .await
                .join("readme.md"),
            "ignored",
        )
        .unwrap();

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].session_count, 1);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_whose_transcripts_have_no_cwd() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(
            home.path(),
            project,
            "s-1",
            br#"{"type":"queue-operation","operation":"enqueue"}"#,
            at(1),
        )
        .await;

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_finds_a_cwd_that_is_not_on_the_first_line() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let content = format!(
            "{}\n{}\n{}\n",
            r#"{"type":"queue-operation"}"#,
            r#"{"type":"attachment"}"#,
            cwd_line(project)
        );
        add_transcript(home.path(), project, "s-1", content.as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_uses_the_newest_transcripts_cwd_when_transcripts_disagree() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let dir_name = ClaudeSessionAdapter::new(home.path().to_path_buf())
            .project_dir(project)
            .await
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        write_transcript_in(home.path(), &dir_name, "older", cwd_line(Path::new("/work/b")).as_bytes(), at(100));
        write_transcript_in(home.path(), &dir_name, "newer", cwd_line(project).as_bytes(), at(200));

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_falls_back_to_an_older_transcript_when_the_newest_has_no_cwd() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        add_transcript(home.path(), project, "older", cwd_line(project).as_bytes(), at(100)).await;
        add_transcript(home.path(), project, "newer", br#"{"type":"attachment"}"#, at(200)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_tolerates_invalid_utf8_and_a_partial_final_line() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let mut content: Vec<u8> = vec![0xff, 0xfe, b'\n'];
        content.extend_from_slice(cwd_line(project).as_bytes());
        content.extend_from_slice(b"\n{\"type\":\"user\",\"cwd\":\"/wor");
        add_transcript(home.path(), project, "s-1", &content, at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from("/work/a"));
    }

    #[tokio::test]
    async fn list_projects_does_not_search_past_the_scan_bound() {
        let home = tempfile::tempdir().unwrap();
        let project = Path::new("/work/a");
        let mut content = String::new();
        for _ in 0..MAX_CWD_SCAN_LINES {
            content.push_str("{\"type\":\"attachment\"}\n");
        }
        content.push_str(&cwd_line(project));
        add_transcript(home.path(), project, "s-1", content.as_bytes(), at(1)).await;

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_sorts_newest_first_and_breaks_ties_by_path() {
        let home = tempfile::tempdir().unwrap();
        for (name, when) in [("/work/b", 100), ("/work/a", 100), ("/work/c", 300)] {
            let project = Path::new(name);
            add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(when)).await;
        }

        let paths: Vec<PathBuf> = list(home.path()).await.unwrap().into_iter().map(|p| p.path).collect();

        assert_eq!(
            paths,
            vec![PathBuf::from("/work/c"), PathBuf::from("/work/a"), PathBuf::from("/work/b")]
        );
    }

    #[tokio::test]
    async fn list_projects_reports_exists_for_a_real_directory() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let cwd = project_dir.path().canonicalize().unwrap();
        add_transcript(home.path(), &cwd, "s-1", cwd_line(&cwd).as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects[0].path, cwd);
        assert!(projects[0].exists);
    }

    #[tokio::test]
    async fn list_projects_skips_a_directory_that_does_not_round_trip_to_its_recovered_path() {
        let home = tempfile::tempdir().unwrap();
        write_transcript_in(
            home.path(),
            "-some-other-name",
            "s-1",
            cwd_line(Path::new("/work/a")).as_bytes(),
            at(1),
        );

        assert!(list(home.path()).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_projects_keeps_a_project_whose_directory_name_hit_the_200_character_cap() {
        let home = tempfile::tempdir().unwrap();
        let long = format!("/work/{}", "d".repeat(250));
        let project = Path::new(&long);
        add_transcript(home.path(), project, "s-1", cwd_line(project).as_bytes(), at(1)).await;

        let projects = list(home.path()).await.unwrap();

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].path, PathBuf::from(&long));
    }

    #[tokio::test]
    async fn list_projects_round_trips_with_list_sessions_for_the_recovered_path() {
        let home = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let cwd = project_dir.path().canonicalize().unwrap();
        add_transcript(home.path(), &cwd, "s-1", cwd_line(&cwd).as_bytes(), at(1)).await;
        add_transcript(home.path(), &cwd, "s-2", cwd_line(&cwd).as_bytes(), at(2)).await;

        let projects = list(home.path()).await.unwrap();
        let sessions = ClaudeSessionAdapter::new(home.path().to_path_buf())
            .list_sessions(&projects[0].path)
            .await
            .unwrap();

        let ids: Vec<SessionId> = sessions.into_iter().map(|s| s.id).collect();
        assert_eq!(ids, vec![SessionId("s-2".to_string()), SessionId("s-1".to_string())]);
        assert_eq!(projects[0].session_count, 2);
    }
}
```

- [ ] **Step 3: Run the tests and see them fail**

Run: `cargo test -p forge-adapter-claude-fs projects::`
Expected: it compiles; `15 failed` with `not implemented: implemented in Task 3, Step 4`.

- [ ] **Step 4: Implement**

Replace everything in `projects.rs` **above** `#[cfg(test)] mod tests` (the scaffold) with:

```rust
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::ProjectSummary;
use forge_ports::{ProjectRepositoryError, ProjectRepositoryPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::sessions::ClaudeSessionAdapter;

// How many lines of one transcript are searched for a `cwd`. Real transcripts carry one within
// their first few lines; the bound only stops a pathological file from being read in full.
const MAX_CWD_SCAN_LINES: usize = 60;

pub struct ClaudeProjectAdapter {
    claude_home: PathBuf,
}

impl ClaudeProjectAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[async_trait]
impl ProjectRepositoryPort for ClaudeProjectAdapter {
    async fn list_projects(&self) -> Result<Vec<ProjectSummary>, ProjectRepositoryError> {
        let projects_dir = self.claude_home.join("projects");
        let mut entries = match tokio::fs::read_dir(&projects_dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(ProjectRepositoryError::ListFailed(format!(
                    "{}: {}",
                    projects_dir.display(),
                    e
                )))
            }
        };
        let sessions = ClaudeSessionAdapter::new(self.claude_home.clone());

        let mut found: Vec<(SystemTime, ProjectSummary)> = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ProjectRepositoryError::ListFailed(format!("{}: {}", projects_dir.display(), e))
        })? {
            let dir = entry.path();
            let Ok(metadata) = tokio::fs::metadata(&dir).await else {
                continue;
            };
            if !metadata.is_dir() {
                continue;
            }

            let transcripts = transcripts_newest_first(&dir).await;
            let Some(&(newest, _)) = transcripts.first() else {
                continue;
            };
            let Ok(last_active) = time::OffsetDateTime::from(newest).format(&Rfc3339) else {
                continue;
            };
            let Some(path) = recover_cwd(&transcripts).await else {
                continue;
            };

            // Keep a directory only if `ClaudeSessionAdapter` finds it again from the recovered
            // path, so `list_sessions(project.path)` always lists exactly this directory.
            if sessions.project_dir(&path).await != dir {
                continue;
            }

            let exists = tokio::fs::metadata(&path)
                .await
                .map(|m| m.is_dir())
                .unwrap_or(false);
            found.push((
                newest,
                ProjectSummary {
                    path,
                    last_active,
                    session_count: transcripts.len(),
                    exists,
                },
            ));
        }

        found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
        Ok(found.into_iter().map(|(_, project)| project).collect())
    }
}

// Every `*.jsonl` file in `dir` with its modification time, newest first (ties broken by path so
// the order is deterministic). Unreadable entries are skipped, never an error.
async fn transcripts_newest_first(dir: &Path) -> Vec<(SystemTime, PathBuf)> {
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut transcripts = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
            continue;
        }
        let Ok(metadata) = tokio::fs::metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Ok(modified) = metadata.modified() else {
            continue;
        };
        transcripts.push((modified, path));
    }
    transcripts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    transcripts
}

// The first `cwd` found, trying transcripts newest first.
async fn recover_cwd(transcripts: &[(SystemTime, PathBuf)]) -> Option<PathBuf> {
    for (_, path) in transcripts {
        if let Some(cwd) = read_cwd(path).await {
            return Some(cwd);
        }
    }
    None
}

// The first non-empty `cwd` string in the first `MAX_CWD_SCAN_LINES` lines. A line that is not
// valid JSON (a partial final line of a transcript still being appended to, or invalid UTF-8) is
// skipped rather than failing the scan.
async fn read_cwd(path: &Path) -> Option<PathBuf> {
    let file = tokio::fs::File::open(path).await.ok()?;
    let mut lines = BufReader::new(file).split(b'\n');
    for _ in 0..MAX_CWD_SCAN_LINES {
        let Ok(Some(line)) = lines.next_segment().await else {
            return None;
        };
        let Ok(value) = serde_json::from_slice::<Value>(&line) else {
            continue;
        };
        if let Some(cwd) = value
            .get("cwd")
            .and_then(Value::as_str)
            .filter(|cwd| !cwd.is_empty())
        {
            return Some(PathBuf::from(cwd));
        }
    }
    None
}
```

- [ ] **Step 5: Run the tests and see them pass**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: `87 passed` (72 before + 15 new), no warnings.

- [ ] **Step 6: Check that the tests bite** (this is what stops a test that passes with its subject deleted)

Break `projects.rs` in each way below, run `cargo test -p forge-adapter-claude-fs projects::`, confirm the named test fails, then undo the change.

| Break this | This test must fail |
|---|---|
| delete the `if sessions.project_dir(&path).await != dir { continue; }` block | `list_projects_skips_a_directory_that_does_not_round_trip_to_its_recovered_path` |
| make `recover_cwd` look only at `transcripts.first()` | `list_projects_falls_back_to_an_older_transcript_when_the_newest_has_no_cwd` |
| in `read_cwd`, turn `for _ in 0..MAX_CWD_SCAN_LINES {` into `loop {` (careful: the same text is in the tests) | `list_projects_does_not_search_past_the_scan_bound` |
| in `read_cwd`, return `None` instead of `continue` for a line that is not JSON | `list_projects_tolerates_invalid_utf8_and_a_partial_final_line` |
| sort ascending in `transcripts_newest_first` | `list_projects_uses_the_newest_transcripts_cwd_when_transcripts_disagree` |
| `unwrap_or(false)` → `unwrap_or(true)` when computing `exists` | `list_projects_recovers_the_real_path_from_the_cwd_not_the_lossy_directory_name` |
| delete the final `found.sort_by(…)` | `list_projects_sorts_newest_first_and_breaks_ties_by_path` |

- [ ] **Step 7: Acceptance check against the real `~/.claude` (read-only; never committed)**

Create `crates/forge-adapter-claude-fs/examples/smoke_projects.rs`:

```rust
// Read-only acceptance check against the real ~/.claude. Never committed.
use forge_adapter_claude_fs::{default_claude_home, ClaudeProjectAdapter, ClaudeSessionAdapter};
use forge_ports::{ProjectRepositoryPort, SessionRepositoryPort};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let home = default_claude_home().expect("a home directory");
    let projects = ClaudeProjectAdapter::new(home.clone())
        .list_projects()
        .await
        .expect("list_projects");
    let sessions = ClaudeSessionAdapter::new(home);

    let mut total = 0;
    for project in &projects {
        let listed = sessions
            .list_sessions(&project.path)
            .await
            .expect("list_sessions");
        assert_eq!(
            listed.len(),
            project.session_count,
            "session_count must equal list_sessions for {}",
            project.path.display()
        );
        total += listed.len();
        println!(
            "{:>3} sessions  exists={:<5}  {}  {}",
            listed.len(),
            project.exists,
            project.last_active,
            project.path.display()
        );
    }
    println!("projects={} sessions={}", projects.len(), total);
}
```

Run: `cargo run -q -p forge-adapter-claude-fs --example smoke_projects`

Expected: one line per project and a last line `projects=N sessions=M`; the program itself asserts that every project's `session_count` equals what `ClaudeSessionAdapter::list_sessions` returns for the recovered path. On the reference machine (2026-09-29) it printed `projects=12 sessions=87`, with 8 of the 12 shown as `exists=false` (four cleaned-up worktree directories, three `/private/tmp` spike directories and one deleted project). Other machines will differ; the assertion is what matters. Then delete the example — `cargo test` would otherwise keep compiling it:

```bash
rm -r crates/forge-adapter-claude-fs/examples
git status --short
```

Expected: only `src/lib.rs`, `src/sessions.rs` and `src/projects.rs` under `crates/forge-adapter-claude-fs` are listed.

- [ ] **Step 8: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: implement ProjectRepositoryPort by recovering project paths from transcript cwd fields

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 4: Spawn environment and `SidecarProcessFactory`

**Files:**
- Create: `crates/forge-adapter-sidecar/src/factory.rs`, `crates/forge-adapter-sidecar/tests/factory_test.rs`
- Modify: `crates/forge-adapter-sidecar/src/process.rs`, `src/lib.rs`, `Cargo.toml`, `tests/contract_test.rs`, `tests/fixtures/fake_sidecar.js`, `examples/spike_drive_sidecar.rs`

**Interfaces:**
- Consumes: `forge_ports::{ConfigRepositoryPort, SidecarFactoryPort, SidecarPort, SidecarError}`; `forge_domain::AppConfig` (`api_key: Option<String>`, `base_url: Option<String>`); `MockConfigRepositoryPort` (via `forge-ports`' `mocks` feature).
- Produces: `SidecarProcess::spawn(node_binary: &str, script_path: &str, env: &[(String, String)]) -> Result<Self, SidecarError>` (was two arguments); `pub struct SidecarProcessFactory` with `pub fn new(node_binary: String, script_path: String, config: Arc<dyn ConfigRepositoryPort>) -> Self`, implementing `SidecarFactoryPort`, re-exported from `lib.rs`. The fixture sidecar understands the message text `TRIGGER_ENV:<NAME>` and answers with an `assistant_delta` of `env:<value>` (or `env:<unset>`).

Behaviour (spec §15.4): every `spawn()` loads the Forge config afresh and sets `ANTHROPIC_API_KEY` from `api_key` and `ANTHROPIC_BASE_URL` from `base_url` **only when set and non-empty**; anything else is inherited from the parent environment. A missing config file is not an error (the config adapter returns `AppConfig::default()`); a config that cannot be loaded is `SidecarError::SpawnFailed("failed to load the Forge config: …")`. If Task 1 found different variable names, use those in `credentials_env` and its tests.

- [ ] **Step 1: Write the failing contract tests (they use the new three-argument `spawn`)**

In `crates/forge-adapter-sidecar/tests/fixtures/fake_sidecar.js` replace

```js
      emit({ type: "assistant_delta", text: `echo:${command.text}` });
```

with

```js
      if (command.text.startsWith("TRIGGER_ENV:")) {
        const name = command.text.slice("TRIGGER_ENV:".length);
        emit({ type: "assistant_delta", text: `env:${process.env[name] ?? "<unset>"}` });
        return;
      }
      emit({ type: "assistant_delta", text: `echo:${command.text}` });
```

In `crates/forge-adapter-sidecar/tests/contract_test.rs` replace **all five** occurrences of

```rust
SidecarProcess::spawn(&node_binary(), &fixture_path()).unwrap()
```

with

```rust
SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap()
```

and append these two tests at the end of the file:

```rust

#[tokio::test]
async fn spawn_applies_the_given_environment_variables() {
    let env = vec![("FORGE_TEST_VALUE".to_string(), "from-spawn".to_string())];
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &env).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_VALUE".to_string()).await.unwrap();

    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "env:from-spawn"),
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_without_extra_environment_leaves_the_variable_unset() {
    let sidecar = SidecarProcess::spawn(&node_binary(), &fixture_path(), &[]).unwrap();
    sidecar.start_session(default_params()).await.unwrap();
    sidecar.next_event().await.unwrap();

    sidecar.send_message("TRIGGER_ENV:FORGE_TEST_UNSET_VALUE".to_string()).await.unwrap();

    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => assert_eq!(text, "env:<unset>"),
        other => panic!("unexpected event: {other:?}"),
    }
}
```

In `crates/forge-adapter-sidecar/examples/spike_drive_sidecar.rs` replace

```rust
SidecarProcess::spawn("node", &sidecar_script)
```

with

```rust
SidecarProcess::spawn("node", &sidecar_script, &[])
```

(`cargo test` builds examples too, so this call site must change with the others.)

- [ ] **Step 2: Run and see the compile error**

Run: `cargo test -p forge-adapter-sidecar`
Expected: `error[E0061]: this function takes 2 arguments but 3 arguments were supplied`.

- [ ] **Step 3: Give `spawn` an environment**

In `crates/forge-adapter-sidecar/src/process.rs` replace

```rust
    pub fn spawn(node_binary: &str, script_path: &str) -> Result<Self, SidecarError> {
        let mut child = Command::new(node_binary)
            .arg(script_path)
```

with

```rust
    pub fn spawn(
        node_binary: &str,
        script_path: &str,
        env: &[(String, String)],
    ) -> Result<Self, SidecarError> {
        let mut child = Command::new(node_binary)
            .arg(script_path)
            .envs(env.iter().map(|(name, value)| (name.as_str(), value.as_str())))
```

Run: `cargo test -p forge-adapter-sidecar`
Expected: unit `2 passed`; `contract_test` `7 passed` (5 old + 2 new); the example compiles; no warnings.

- [ ] **Step 4: Add the factory scaffold, its tests and the wiring**

In `crates/forge-adapter-sidecar/Cargo.toml` replace

```toml
[dev-dependencies]
tokio = { version = "1", features = ["full"] }
```

with

```toml
[dev-dependencies]
forge-ports = { path = "../forge-ports", features = ["mocks"] }
tokio = { version = "1", features = ["full"] }
```

In `crates/forge-adapter-sidecar/src/lib.rs` replace `mod process;` with

```rust
mod factory;
mod process;
```

and replace `pub use process::SidecarProcess;` with

```rust
pub use factory::SidecarProcessFactory;
pub use process::SidecarProcess;
```

`crates/forge-adapter-sidecar/src/factory.rs` is **one file**: this scaffold followed by the unit-test module.

```rust
use std::sync::Arc;

use async_trait::async_trait;
use forge_domain::AppConfig;
use forge_ports::{ConfigRepositoryPort, SidecarError, SidecarFactoryPort, SidecarPort};

pub struct SidecarProcessFactory {
    node_binary: String,
    script_path: String,
    config: Arc<dyn ConfigRepositoryPort>,
}

impl SidecarProcessFactory {
    pub fn new(node_binary: String, script_path: String, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { node_binary, script_path, config }
    }
}

#[async_trait]
impl SidecarFactoryPort for SidecarProcessFactory {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError> {
        let _ = (&self.node_binary, &self.script_path, &self.config);
        unimplemented!("implemented in Task 4, Step 6")
    }
}

pub(crate) fn credentials_env(config: &AppConfig) -> Vec<(String, String)> {
    let _ = config;
    unimplemented!("implemented in Task 4, Step 6")
}
```

```rust

#[cfg(test)]
mod tests {
    use super::*;

    fn config(api_key: Option<&str>, base_url: Option<&str>) -> AppConfig {
        AppConfig {
            api_key: api_key.map(str::to_string),
            base_url: base_url.map(str::to_string),
            ..AppConfig::default()
        }
    }

    #[test]
    fn credentials_env_injects_both_values_when_both_are_set() {
        let env = credentials_env(&config(Some("sk-test"), Some("https://proxy.example.test")));

        assert_eq!(
            env,
            vec![
                ("ANTHROPIC_API_KEY".to_string(), "sk-test".to_string()),
                ("ANTHROPIC_BASE_URL".to_string(), "https://proxy.example.test".to_string()),
            ]
        );
    }

    #[test]
    fn credentials_env_injects_nothing_for_a_default_config() {
        assert!(credentials_env(&AppConfig::default()).is_empty());
    }

    #[test]
    fn credentials_env_treats_empty_strings_as_unset() {
        assert!(credentials_env(&config(Some(""), Some(""))).is_empty());
    }

    #[test]
    fn credentials_env_injects_only_the_value_that_is_set() {
        let env = credentials_env(&config(None, Some("https://proxy.example.test")));

        assert_eq!(
            env,
            vec![("ANTHROPIC_BASE_URL".to_string(), "https://proxy.example.test".to_string())]
        );
    }
}
```

`crates/forge-adapter-sidecar/tests/factory_test.rs` (the tests that really spawn the fixture sidecar):

```rust
use std::sync::Arc;

use forge_adapter_sidecar::SidecarProcessFactory;
use forge_domain::{AppConfig, PermissionMode};
use forge_ports::{
    ConfigError, MockConfigRepositoryPort, SidecarError, SidecarEvent, SidecarFactoryPort,
    SidecarPort, StartSessionParams,
};

fn fixture_path() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/fake_sidecar.js").to_string()
}

fn node_binary() -> String {
    std::env::var("NODE_BINARY").unwrap_or_else(|_| "node".to_string())
}

fn factory_with(config: MockConfigRepositoryPort) -> SidecarProcessFactory {
    SidecarProcessFactory::new(node_binary(), fixture_path(), Arc::new(config))
}

async fn start(sidecar: &dyn SidecarPort) {
    sidecar
        .start_session(StartSessionParams {
            cwd: "/tmp".to_string(),
            resume_session_id: None,
            permission_mode: PermissionMode::Default,
            model: None,
        })
        .await
        .unwrap();
    assert!(matches!(
        sidecar.next_event().await.unwrap(),
        SidecarEvent::SessionStarted { .. }
    ));
}

// Asks the fixture sidecar to echo one of its own environment variables.
async fn ask_env(sidecar: &dyn SidecarPort, name: &str) -> String {
    sidecar.send_message(format!("TRIGGER_ENV:{name}")).await.unwrap();
    match sidecar.next_event().await.unwrap() {
        SidecarEvent::AssistantDelta { text } => text,
        other => panic!("unexpected event: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_injects_the_configured_credentials_into_the_sidecar_environment() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| {
        Ok(AppConfig {
            api_key: Some("sk-test-123".to_string()),
            base_url: Some("https://proxy.example.test".to_string()),
            ..AppConfig::default()
        })
    });
    let factory = factory_with(config);

    let sidecar = factory.spawn().await.unwrap();

    start(sidecar.as_ref()).await;
    assert_eq!(ask_env(sidecar.as_ref(), "ANTHROPIC_API_KEY").await, "env:sk-test-123");
    assert_eq!(
        ask_env(sidecar.as_ref(), "ANTHROPIC_BASE_URL").await,
        "env:https://proxy.example.test"
    );
}

#[tokio::test]
async fn spawn_reloads_the_config_every_time_so_edits_apply_to_the_next_session() {
    let mut config = MockConfigRepositoryPort::new();
    let mut calls = 0;
    config.expect_load().times(2).returning(move || {
        calls += 1;
        Ok(AppConfig {
            api_key: Some(format!("key-{calls}")),
            ..AppConfig::default()
        })
    });
    let factory = factory_with(config);

    let first = factory.spawn().await.unwrap();
    let second = factory.spawn().await.unwrap();

    start(first.as_ref()).await;
    start(second.as_ref()).await;
    assert_eq!(ask_env(first.as_ref(), "ANTHROPIC_API_KEY").await, "env:key-1");
    assert_eq!(ask_env(second.as_ref(), "ANTHROPIC_API_KEY").await, "env:key-2");
}

#[tokio::test]
async fn spawn_with_a_default_config_still_starts_a_working_sidecar() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| Ok(AppConfig::default()));
    let factory = factory_with(config);

    let sidecar = factory.spawn().await.unwrap();

    start(sidecar.as_ref()).await;
    assert_eq!(ask_env(sidecar.as_ref(), "FORGE_TEST_UNSET_VALUE").await, "env:<unset>");
}

#[tokio::test]
async fn spawn_fails_with_spawn_failed_when_the_config_cannot_be_loaded() {
    let mut config = MockConfigRepositoryPort::new();
    config
        .expect_load()
        .returning(|| Err(ConfigError::ParseFailed("bad json".to_string())));
    let factory = factory_with(config);

    let Err(error) = factory.spawn().await else {
        panic!("spawn should fail when the config cannot be loaded");
    };

    match error {
        SidecarError::SpawnFailed(message) => {
            assert!(message.contains("failed to load the Forge config"), "unexpected message: {message}");
            assert!(message.contains("bad json"), "unexpected message: {message}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[tokio::test]
async fn spawn_fails_with_spawn_failed_when_the_node_binary_does_not_exist() {
    let mut config = MockConfigRepositoryPort::new();
    config.expect_load().returning(|| Ok(AppConfig::default()));
    let factory = SidecarProcessFactory::new(
        "forge-test-no-such-node-binary".to_string(),
        fixture_path(),
        Arc::new(config),
    );

    let result = factory.spawn().await;

    assert!(matches!(result, Err(SidecarError::SpawnFailed(_))));
}
```

- [ ] **Step 5: Run and see them fail**

Run:

```bash
cargo test -p forge-adapter-sidecar --lib factory::
cargo test -p forge-adapter-sidecar --test factory_test
```

Expected: both compile without warnings; the first reports `4 failed` and the second `5 failed`, all with `not implemented: implemented in Task 4, Step 6`. (The integration tests are a separate test binary, so a name filter on `factory` would not select them.)

- [ ] **Step 6: Implement the factory**

Replace everything in `factory.rs` **above** `#[cfg(test)] mod tests` (the scaffold) with:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use forge_domain::AppConfig;
use forge_ports::{ConfigRepositoryPort, SidecarError, SidecarFactoryPort, SidecarPort};

use crate::process::SidecarProcess;

/// Spawns one Node sidecar per session, with Forge's own credentials in its environment.
pub struct SidecarProcessFactory {
    node_binary: String,
    script_path: String,
    config: Arc<dyn ConfigRepositoryPort>,
}

impl SidecarProcessFactory {
    pub fn new(node_binary: String, script_path: String, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { node_binary, script_path, config }
    }
}

#[async_trait]
impl SidecarFactoryPort for SidecarProcessFactory {
    async fn spawn(&self) -> Result<Arc<dyn SidecarPort>, SidecarError> {
        // Loaded on every spawn, so an edit to config.json applies to the next session opened.
        // A missing config file is not an error (the adapter returns `AppConfig::default()`); a
        // malformed one is, so a broken config is reported instead of silently ignored.
        let config = self
            .config
            .load()
            .await
            .map_err(|e| SidecarError::SpawnFailed(format!("failed to load the Forge config: {e}")))?;
        let process = SidecarProcess::spawn(&self.node_binary, &self.script_path, &credentials_env(&config))?;
        Ok(Arc::new(process))
    }
}

// Only values that are set and non-empty are injected; everything else is inherited from the
// parent environment. Never format `config` itself (its `Debug` output contains the API key).
pub(crate) fn credentials_env(config: &AppConfig) -> Vec<(String, String)> {
    let mut env = Vec::new();
    if let Some(api_key) = config.api_key.as_deref().filter(|value| !value.is_empty()) {
        env.push(("ANTHROPIC_API_KEY".to_string(), api_key.to_string()));
    }
    if let Some(base_url) = config.base_url.as_deref().filter(|value| !value.is_empty()) {
        env.push(("ANTHROPIC_BASE_URL".to_string(), base_url.to_string()));
    }
    env
}
```

- [ ] **Step 7: Run and see everything pass**

Run: `cargo test -p forge-adapter-sidecar`
Expected: unit `6 passed`, `contract_test` `7 passed`, `factory_test` `5 passed`, no warnings.

Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'` — Expected: `166`.

- [ ] **Step 8: Commit**

```bash
git add crates/forge-adapter-sidecar
git commit -m "feat: give SidecarProcess::spawn an environment and add SidecarProcessFactory

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 5: `LiveSession` — one task per open session

**Files:**
- Create: `crates/forge-application/src/live_session.rs`, `crates/forge-application/src/live_session_actor.rs`, `crates/forge-application/src/test_support.rs` (test-only)
- Modify: `crates/forge-application/src/lib.rs`, `crates/forge-application/Cargo.toml`

**Interfaces:**
- Consumes: `forge_ports::{SidecarFactoryPort, SidecarPort, SidecarEvent, StartSessionParams, SidecarError}`; `crate::{apply_sidecar_event, SessionUpdate}`; the use-cases `SendMessageUseCase::execute(&self, &mut Session, String)`, `ApprovePermissionUseCase::execute(&self, String, PermissionDecision)`, `InterruptTurnUseCase::execute(&self)`; `forge_domain::{Session, SessionId, PermissionRequest, PermissionDecision}`.
- Produces (public, re-exported from `lib.rs`): `LiveSessionId(pub u64)`; `Phase { Idle, Running, Failed }`; `LiveSessionView { id, project_path, session, claude_session_id, phase, pending_permissions, last_error, can_send }`; `LiveCommand { Send, Approve, Interrupt, Close }`; `LiveSessionHandle` (`Clone`; `id()`, `send(String)`, `approve(String, PermissionDecision)`, `interrupt()`, `close()`, `subscribe() -> watch::Receiver<LiveSessionView>`, `snapshot()`, `claude_session_id()`, `is_closed()`). Crate-internal, used by Task 6: `LiveSessionInit { id, project_path, session, claude_session_id, model, can_send, factory }` and `spawn_live_session(LiveSessionInit) -> LiveSessionHandle` (must be called inside a tokio runtime).

Behaviour (spec §15.5). The task `select!`s, in priority order, between UI commands, the sidecar's next event, and publishing; it publishes a clone of its view through a `watch` channel only when neither of the first two is immediately ready, so a burst of events is applied first and published once. **No sidecar exists until the first `Send`**: it then spawns one through the factory, calls `start_session` (with `resume_session_id` for a resumed session and the session's own permission mode) directly on the `SidecarPort` — `StartSessionUseCase` cannot carry a resume id and `ResumeSessionUseCase` loads history eagerly — and sends the message through `SendMessageUseCase`. Failures and events follow the table in §15.5; every `Error` event discards the sidecar (the protocol cannot tell fatal from non-fatal). `SendMessageUseCase` records the message *before* writing it, so a failed write removes that message again.

- [ ] **Step 1: Add `tokio` to the crate**

In `crates/forge-application/Cargo.toml`, in `[dependencies]`, replace

```toml
async-trait = "0.1"
serde_json = "1"
thiserror = "1"
```

with

```toml
async-trait = "0.1"
serde_json = "1"
thiserror = "1"
tokio = { version = "1", features = ["sync", "rt", "macros"] }
```

and in `[dev-dependencies]` replace

```toml
tokio = { version = "1", features = ["macros", "rt"] }
```

with

```toml
tokio = { version = "1", features = ["macros", "rt", "time"] }
```

- [ ] **Step 2: Add the test doubles and wire the modules**

`crates/forge-application/src/test_support.rs` (compiled only under `cfg(test)`). `MockSidecarPort` cannot script a sidecar that is alive but quiet, so this is a small fake driven through a control, plus a factory built on `MockSidecarFactoryPort`:

```rust
//! Test doubles for the session tasks. `MockSidecarPort` cannot script "alive but quiet" (a
//! `next_event` that stays pending), so the sidecar here is a small fake driven through a control.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::PermissionDecision;
use forge_ports::{
    MockSidecarFactoryPort, SidecarError, SidecarEvent, SidecarFactoryPort, SidecarPort,
    StartSessionParams,
};
use tokio::sync::{mpsc, watch};

use crate::live_session::LiveSessionView;

/// Every command the fake sidecar received, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    Start(StartSessionParams),
    Send(String),
    Permission(String, PermissionDecision),
    Interrupt,
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct FakeControl {
    events: Arc<Mutex<Option<mpsc::UnboundedSender<SidecarEvent>>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    fail_start: Arc<AtomicBool>,
    fail_send: Arc<AtomicBool>,
}

impl FakeControl {
    /// Queues an event for the session to read.
    pub(crate) fn push(&self, event: SidecarEvent) {
        if let Some(events) = self.events.lock().unwrap().as_ref() {
            let _ = events.send(event);
        }
    }

    /// Ends the event stream, like a sidecar process that has died.
    pub(crate) fn end_stream(&self) {
        self.events.lock().unwrap().take();
    }

    pub(crate) fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn fail_start(&self) {
        self.fail_start.store(true, Ordering::SeqCst);
    }

    pub(crate) fn fail_send(&self) {
        self.fail_send.store(true, Ordering::SeqCst);
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

struct FakeSidecar {
    events: tokio::sync::Mutex<mpsc::UnboundedReceiver<SidecarEvent>>,
    control: FakeControl,
}

pub(crate) fn fake_sidecar() -> (Arc<dyn SidecarPort>, FakeControl) {
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let control = FakeControl {
        events: Arc::new(Mutex::new(Some(events_tx))),
        calls: Arc::new(Mutex::new(Vec::new())),
        fail_start: Arc::new(AtomicBool::new(false)),
        fail_send: Arc::new(AtomicBool::new(false)),
    };
    let sidecar = FakeSidecar {
        events: tokio::sync::Mutex::new(events_rx),
        control: control.clone(),
    };
    (Arc::new(sidecar), control)
}

#[async_trait]
impl SidecarPort for FakeSidecar {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError> {
        self.control.record(Call::Start(params));
        if self.control.fail_start.load(Ordering::SeqCst) {
            return Err(SidecarError::WriteFailed("fake start failure".to_string()));
        }
        Ok(())
    }

    async fn send_message(&self, text: String) -> Result<(), SidecarError> {
        self.control.record(Call::Send(text));
        if self.control.fail_send.load(Ordering::SeqCst) {
            return Err(SidecarError::ProcessExited);
        }
        Ok(())
    }

    async fn respond_to_permission(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        self.control.record(Call::Permission(permission_request_id, decision));
        Ok(())
    }

    async fn interrupt(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Interrupt);
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Shutdown);
        Ok(())
    }

    async fn next_event(&self) -> Option<SidecarEvent> {
        self.events.lock().await.recv().await
    }
}

/// A factory that hands out the scripted sidecars in order, and a counter of how many times it was
/// asked. Asking for more sidecars than were scripted panics inside the session task.
pub(crate) fn factory_returning(
    sidecars: Vec<Result<Arc<dyn SidecarPort>, SidecarError>>,
) -> (Arc<dyn SidecarFactoryPort>, Arc<AtomicUsize>) {
    let queue = Mutex::new(VecDeque::from(sidecars));
    let spawns = Arc::new(AtomicUsize::new(0));
    let counter = spawns.clone();
    let mut factory = MockSidecarFactoryPort::new();
    factory.expect_spawn().returning(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        queue
            .lock()
            .unwrap()
            .pop_front()
            .expect("the test scripted fewer sidecars than were spawned")
    });
    (Arc::new(factory), spawns)
}

const WAIT: Duration = Duration::from_secs(2);

/// Waits until the published view satisfies `condition` and returns that view.
pub(crate) async fn wait_for(
    view: &mut watch::Receiver<LiveSessionView>,
    what: &str,
    condition: impl Fn(&LiveSessionView) -> bool,
) -> LiveSessionView {
    let waiting = async {
        loop {
            {
                let current = view.borrow_and_update();
                if condition(&current) {
                    return current.clone();
                }
            }
            if view.changed().await.is_err() {
                panic!("the session ended while waiting for {what}");
            }
        }
    };
    tokio::time::timeout(WAIT, waiting)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// Waits until the session's task has ended (its view channel closes).
pub(crate) async fn wait_closed(view: &mut watch::Receiver<LiveSessionView>) {
    let waiting = async { while view.changed().await.is_ok() {} };
    tokio::time::timeout(WAIT, waiting)
        .await
        .expect("the session should have closed");
}
```

Replace `crates/forge-application/src/lib.rs` with:

```rust
mod event;
mod live_session;
mod live_session_actor;
mod use_cases;

#[cfg(test)]
mod test_support;

pub use event::{apply_sidecar_event, push_user_message, SessionUpdate};
pub use live_session::{
    LiveCommand, LiveSessionHandle, LiveSessionId, LiveSessionView, Phase,
};
pub use use_cases::{
    ApprovePermissionUseCase, InterruptTurnUseCase, ResumeSessionError, ResumeSessionUseCase,
    SendMessageUseCase, StartSessionUseCase, ToggleMcpServerUseCase, TogglePluginUseCase,
    ToggleSkillUseCase,
};
```

- [ ] **Step 3: Create `live_session.rs` (types, handle, spawn and all tests) and the actor scaffold**

`crates/forge-application/src/live_session.rs` is **one file**: the code below, immediately followed by the test module.

```rust
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use forge_domain::{PermissionDecision, PermissionRequest, Session, SessionId};
use forge_ports::SidecarFactoryPort;
use tokio::sync::{mpsc, watch};

use crate::live_session_actor::Actor;

/// App-local handle for an open session. Claude's own `SessionId` is unknown for a brand-new
/// session until the sidecar reports `session_started`, so it cannot be the key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LiveSessionId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// No turn in flight (a sidecar may or may not exist).
    Idle,
    Running,
    /// The last sidecar failed; the next `send` respawns one.
    Failed,
}

/// Everything the UI needs to draw one open session.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSessionView {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    /// `session.id` is empty until the Claude session id is known.
    pub session: Session,
    pub claude_session_id: Option<SessionId>,
    pub phase: Phase,
    /// Oldest first; the front is the one the UI shows.
    pub pending_permissions: VecDeque<PermissionRequest>,
    pub last_error: Option<String>,
    /// `false` when the project folder no longer exists: the history is readable only.
    pub can_send: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LiveCommand {
    Send(String),
    Approve { id: String, decision: PermissionDecision },
    Interrupt,
    Close,
}

/// A cheap, cloneable remote control for one open session. Every command is fire-and-forget: the
/// outcome shows up in the view.
#[derive(Clone)]
pub struct LiveSessionHandle {
    id: LiveSessionId,
    commands: mpsc::UnboundedSender<LiveCommand>,
    view: watch::Receiver<LiveSessionView>,
}

impl LiveSessionHandle {
    pub fn id(&self) -> LiveSessionId {
        self.id
    }

    pub fn send(&self, text: String) {
        let _ = self.commands.send(LiveCommand::Send(text));
    }

    pub fn approve(&self, permission_request_id: String, decision: PermissionDecision) {
        let _ = self.commands.send(LiveCommand::Approve {
            id: permission_request_id,
            decision,
        });
    }

    pub fn interrupt(&self) {
        let _ = self.commands.send(LiveCommand::Interrupt);
    }

    pub fn close(&self) {
        let _ = self.commands.send(LiveCommand::Close);
    }

    /// A receiver that wakes on every published change and reports `Err` once the session is closed.
    pub fn subscribe(&self) -> watch::Receiver<LiveSessionView> {
        self.view.clone()
    }

    pub fn snapshot(&self) -> LiveSessionView {
        self.view.borrow().clone()
    }

    /// The Claude session id, once known (immediately for a resumed session). Cheaper than
    /// `snapshot`, which clones the whole history.
    pub fn claude_session_id(&self) -> Option<SessionId> {
        self.view.borrow().claude_session_id.clone()
    }

    /// `true` once the session's task has ended (after `close`).
    pub fn is_closed(&self) -> bool {
        self.commands.is_closed()
    }
}

pub(crate) struct LiveSessionInit {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub session: Session,
    pub claude_session_id: Option<SessionId>,
    pub model: Option<String>,
    pub can_send: bool,
    pub factory: Arc<dyn SidecarFactoryPort>,
}

/// Starts the session's task. Must be called from inside a tokio runtime.
pub(crate) fn spawn_live_session(init: LiveSessionInit) -> LiveSessionHandle {
    let view = LiveSessionView {
        id: init.id,
        project_path: init.project_path,
        session: init.session,
        claude_session_id: init.claude_session_id,
        phase: Phase::Idle,
        pending_permissions: VecDeque::new(),
        last_error: None,
        can_send: init.can_send,
    };
    let (view_tx, view_rx) = watch::channel(view.clone());
    let (command_tx, command_rx) = mpsc::unbounded_channel();
    let actor = Actor::new(view, view_tx, command_rx, init.factory, init.model);
    tokio::spawn(actor.run());
    LiveSessionHandle {
        id: init.id,
        commands: command_tx,
        view: view_rx,
    }
}
```

```rust

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use forge_domain::{MessageRole, PermissionMode};
    use forge_ports::{SidecarError, SidecarEvent, StartSessionParams};

    use super::*;
    use crate::test_support::{factory_returning, fake_sidecar, wait_closed, wait_for, Call};

    fn empty_session() -> Session {
        Session {
            id: SessionId(String::new()),
            project_path: PathBuf::from("/work/a"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::AcceptEdits,
        }
    }

    fn new_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        LiveSessionInit {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: empty_session(),
            claude_session_id: None,
            model: Some("model-x".to_string()),
            can_send: true,
            factory,
        }
    }

    fn resumed_init(factory: Arc<dyn SidecarFactoryPort>) -> LiveSessionInit {
        let mut session = empty_session();
        session.id = SessionId("claude-1".to_string());
        session.messages.push(forge_domain::Message {
            role: MessageRole::User,
            content: "earlier".to_string(),
            tool_calls: Vec::new(),
        });
        LiveSessionInit {
            claude_session_id: Some(SessionId("claude-1".to_string())),
            session,
            ..new_init(factory)
        }
    }

    fn start_params(resume: Option<&str>) -> StartSessionParams {
        StartSessionParams {
            cwd: "/work/a".to_string(),
            resume_session_id: resume.map(str::to_string),
            permission_mode: PermissionMode::AcceptEdits,
            model: Some("model-x".to_string()),
        }
    }

    fn permission_request(id: &str) -> SidecarEvent {
        SidecarEvent::PermissionRequest {
            permission_request_id: id.to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({ "command": "cargo test" }),
        }
    }

    fn delta(text: &str) -> SidecarEvent {
        SidecarEvent::AssistantDelta { text: text.to_string() }
    }

    fn turn_complete() -> SidecarEvent {
        SidecarEvent::TurnComplete { input_tokens: 1, output_tokens: 2 }
    }

    #[tokio::test]
    async fn opening_a_session_spawns_no_sidecar() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert_eq!(handle.snapshot().session.messages.len(), 1);
        assert_eq!(handle.snapshot().phase, Phase::Idle);
    }

    #[tokio::test]
    async fn the_first_send_spawns_a_sidecar_starts_the_session_then_sends_the_message() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hello".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(None)), Call::Send("hello".to_string())]
        );
        assert_eq!(seen.session.messages.len(), 1);
        assert_eq!(seen.session.messages[0].role, MessageRole::User);
        assert_eq!(seen.session.messages[0].content, "hello");
    }

    #[tokio::test]
    async fn resuming_an_existing_session_passes_its_id_as_the_resume_session_id() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.send("continue".to_string());
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        assert_eq!(
            control.calls(),
            vec![Call::Start(start_params(Some("claude-1"))), Call::Send("continue".to_string())]
        );
        assert_eq!(seen.session.messages.len(), 2, "history plus the new message");
    }

    #[tokio::test]
    async fn session_started_records_the_claude_session_id() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(SidecarEvent::SessionStarted { session_id: "claude-9".to_string() });
        let seen = wait_for(&mut view, "the session id", |v| v.claude_session_id.is_some()).await;

        assert_eq!(seen.claude_session_id, Some(SessionId("claude-9".to_string())));
        assert_eq!(seen.session.id, SessionId("claude-9".to_string()));
    }

    #[tokio::test]
    async fn streamed_events_build_the_assistant_message_and_turn_complete_returns_to_idle() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(delta("Hel"));
        control.push(delta("lo"));
        control.push(SidecarEvent::ToolUse {
            tool_call_id: "t-1".to_string(),
            name: "Read".to_string(),
            input: serde_json::json!({ "file_path": "a.rs" }),
        });
        control.push(SidecarEvent::ToolResult {
            tool_call_id: "t-1".to_string(),
            output: "contents".to_string(),
            is_error: false,
        });
        control.push(turn_complete());
        let seen = wait_for(&mut view, "the turn to complete", |v| {
            v.phase == Phase::Idle && v.session.messages.len() == 2
        })
        .await;

        let reply = &seen.session.messages[1];
        assert_eq!(reply.role, MessageRole::Assistant);
        assert_eq!(reply.content, "Hello");
        assert_eq!(reply.tool_calls.len(), 1);
        assert_eq!(reply.tool_calls[0].result.as_ref().unwrap().output, "contents");
    }

    #[tokio::test]
    async fn permission_requests_queue_in_order_and_approving_answers_the_sidecar_and_pops_it() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("run tests".to_string());
        control.push(permission_request("p-1"));
        control.push(permission_request("p-2"));
        let queued = wait_for(&mut view, "two queued requests", |v| v.pending_permissions.len() == 2).await;
        assert_eq!(queued.pending_permissions[0].id, "p-1");

        handle.approve("p-1".to_string(), PermissionDecision::AllowOnce);
        let after = wait_for(&mut view, "one request left", |v| v.pending_permissions.len() == 1).await;

        assert_eq!(after.pending_permissions[0].id, "p-2");
        assert!(control
            .calls()
            .contains(&Call::Permission("p-1".to_string(), PermissionDecision::AllowOnce)));
    }

    #[tokio::test]
    async fn approving_an_unknown_request_does_not_reach_the_sidecar() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.approve("nope".to_string(), PermissionDecision::Deny);
        handle.interrupt(); // commands run in order, so once this lands the approval was handled
        for _ in 0..200 {
            if control.calls().contains(&Call::Interrupt) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        assert!(control.calls().contains(&Call::Interrupt));
        assert!(!control
            .calls()
            .iter()
            .any(|call| matches!(call, Call::Permission(..))));
    }

    #[tokio::test]
    async fn interrupt_reaches_the_sidecar_and_is_a_no_op_without_one() {
        let (factory_idle, spawns) = factory_returning(vec![]);
        let idle = spawn_live_session(new_init(factory_idle));
        let mut idle_view = idle.subscribe();
        idle.interrupt();
        idle.close();
        wait_closed(&mut idle_view).await;
        assert_eq!(spawns.load(Ordering::SeqCst), 0);

        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.interrupt();
        handle.close();
        wait_closed(&mut view).await;

        assert!(control.calls().contains(&Call::Interrupt));
    }

    #[tokio::test]
    async fn an_error_event_shuts_the_sidecar_down_clears_permissions_and_fails_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        control.push(permission_request("p-1"));
        wait_for(&mut view, "a queued request", |v| !v.pending_permissions.is_empty()).await;
        control.push(SidecarEvent::Error { message: "boom".to_string() });
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.last_error, Some("boom".to_string()));
        assert!(seen.pending_permissions.is_empty());
        assert!(control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn sending_after_a_failure_respawns_and_resumes_with_the_known_session_id() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::SessionStarted { session_id: "claude-1".to_string() });
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        let seen = wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(
            second_control.calls(),
            vec![Call::Start(start_params(Some("claude-1"))), Call::Send("two".to_string())]
        );
        assert_eq!(seen.last_error, None, "a successful send clears the error");
    }

    #[tokio::test]
    async fn sending_after_a_failure_that_came_before_any_session_id_starts_fresh() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(first), Ok(second)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("one".to_string());
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        handle.send("two".to_string());
        wait_for(&mut view, "the second turn", |v| v.phase == Phase::Running).await;

        assert_eq!(second_control.calls()[0], Call::Start(start_params(None)));
    }

    #[tokio::test]
    async fn a_spawn_failure_fails_the_session_and_leaves_the_history_untouched() {
        let (factory, spawns) = factory_returning(vec![Err(SidecarError::SpawnFailed("no node".to_string()))]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert!(seen.last_error.unwrap().contains("no node"));
        assert!(seen.session.messages.is_empty());
    }

    #[tokio::test]
    async fn a_start_session_failure_fails_the_session_and_never_sends_the_message() {
        let (sidecar, control) = fake_sidecar();
        control.fail_start();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("fake start failure"));
        assert!(seen.session.messages.is_empty());
        assert_eq!(control.calls(), vec![Call::Start(start_params(None))], "the message was never sent");
    }

    #[tokio::test]
    async fn a_failed_write_removes_the_undelivered_message_from_the_history() {
        let (sidecar, control) = fake_sidecar();
        control.fail_send();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(resumed_init(factory));
        let mut view = handle.subscribe();

        handle.send("lost".to_string());
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert_eq!(seen.session.messages.len(), 1, "only the earlier history remains");
        assert_eq!(seen.session.messages[0].content, "earlier");
    }

    #[tokio::test]
    async fn the_event_stream_ending_fails_the_session_without_asking_the_dead_process_to_shut_down() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.end_stream();
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("exited unexpectedly"));
        assert!(!control.calls().contains(&Call::Shutdown));
    }

    #[tokio::test]
    async fn an_unexpected_session_ended_event_fails_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());

        control.push(SidecarEvent::SessionEnded);
        let seen = wait_for(&mut view, "the failure", |v| v.phase == Phase::Failed).await;

        assert!(seen.last_error.unwrap().contains("ended"));
    }

    #[tokio::test]
    async fn sending_to_a_session_whose_folder_is_missing_sets_an_error_and_never_spawns() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(LiveSessionInit {
            can_send: false,
            ..resumed_init(factory)
        });
        let mut view = handle.subscribe();

        handle.send("hi".to_string());
        let seen = wait_for(&mut view, "the error", |v| v.last_error.is_some()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert_eq!(seen.phase, Phase::Idle);
        assert!(!seen.can_send);
        assert_eq!(seen.session.messages.len(), 1, "the history stays readable");
    }

    #[tokio::test]
    async fn close_shuts_the_sidecar_down_and_ends_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        handle.close();
        wait_closed(&mut view).await;

        assert!(control.calls().contains(&Call::Shutdown));
        assert!(handle.is_closed());
    }

    #[tokio::test]
    async fn closing_without_a_sidecar_just_ends_the_session() {
        let (factory, spawns) = factory_returning(vec![]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn commands_and_events_interleave_without_losing_either() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("one".to_string());
        for _ in 0..50 {
            control.push(delta("a"));
        }
        handle.send("two".to_string());
        for _ in 0..50 {
            control.push(delta("b"));
        }
        control.push(turn_complete());
        let seen = wait_for(&mut view, "both turns' events", |v| {
            v.phase == Phase::Idle && v.session.messages.iter().map(|m| m.content.len()).sum::<usize>() >= 106
        })
        .await;

        assert_eq!(
            control.calls(),
            vec![
                Call::Start(start_params(None)),
                Call::Send("one".to_string()),
                Call::Send("two".to_string()),
            ]
        );
        let total: usize = seen.session.messages.iter().map(|m| m.content.len()).sum();
        assert_eq!(total, "one".len() + "two".len() + 100);
    }

    #[tokio::test]
    async fn a_burst_of_events_is_applied_completely_but_published_only_a_few_times() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();

        handle.send("go".to_string());
        for _ in 0..1000 {
            control.push(delta("x"));
        }
        control.push(turn_complete());

        let mut updates = 0;
        loop {
            match tokio::time::timeout(Duration::from_millis(300), view.changed()).await {
                Ok(Ok(())) => {
                    updates += 1;
                    if view.borrow().phase == Phase::Idle {
                        break;
                    }
                }
                _ => break,
            }
        }

        let last = handle.snapshot();
        assert_eq!(last.phase, Phase::Idle);
        assert_eq!(last.session.messages[1].content.len(), 1000);
        assert!(updates <= 5, "1001 events were published {updates} times");
    }
}
```

`crates/forge-application/src/live_session_actor.rs` (scaffold):

```rust
use std::sync::Arc;

use forge_ports::SidecarFactoryPort;
use tokio::sync::{mpsc, watch};

use crate::live_session::{LiveCommand, LiveSessionView};

pub(crate) struct Actor {
    view: LiveSessionView,
    view_tx: watch::Sender<LiveSessionView>,
    commands: mpsc::UnboundedReceiver<LiveCommand>,
    factory: Arc<dyn SidecarFactoryPort>,
    model: Option<String>,
}

impl Actor {
    pub(crate) fn new(
        view: LiveSessionView,
        view_tx: watch::Sender<LiveSessionView>,
        commands: mpsc::UnboundedReceiver<LiveCommand>,
        factory: Arc<dyn SidecarFactoryPort>,
        model: Option<String>,
    ) -> Self {
        Self { view, view_tx, commands, factory, model }
    }

    pub(crate) async fn run(self) {
        let _ = (&self.view, &self.view_tx, &self.commands, &self.factory, &self.model);
        unimplemented!("implemented in Task 5, Step 5")
    }
}
```

- [ ] **Step 4: Run the tests and see them fail**

Run: `cargo test -p forge-application live_session::`
Expected: it compiles without warnings; `19 failed, 2 passed`. The two that pass need no actor: `opening_a_session_spawns_no_sidecar` and `closing_without_a_sidecar_just_ends_the_session` (the scaffold's panic ends the task, which closes the channel those tests wait for). The failures read `the session ended while waiting for …`.

- [ ] **Step 5: Implement the actor**

Replace the whole of `crates/forge-application/src/live_session_actor.rs` with:

```rust
use std::future::pending;
use std::sync::Arc;

use forge_domain::PermissionDecision;
use forge_ports::{SidecarEvent, SidecarFactoryPort, SidecarPort, StartSessionParams};
use tokio::sync::{mpsc, watch};

use crate::event::{apply_sidecar_event, SessionUpdate};
use crate::live_session::{LiveCommand, LiveSessionView, Phase};
use crate::use_cases::{ApprovePermissionUseCase, InterruptTurnUseCase, SendMessageUseCase};

const CANNOT_SEND: &str = "the project folder no longer exists, so this session can only be viewed";

/// The one task that owns a session. It `select!`s between UI commands and the sidecar's event
/// stream, so nothing is shared and no lock is needed.
pub(crate) struct Actor {
    view: LiveSessionView,
    view_tx: watch::Sender<LiveSessionView>,
    commands: mpsc::UnboundedReceiver<LiveCommand>,
    factory: Arc<dyn SidecarFactoryPort>,
    model: Option<String>,
    sidecar: Option<Arc<dyn SidecarPort>>,
    /// The view changed since it was last published.
    dirty: bool,
}

impl Actor {
    pub(crate) fn new(
        view: LiveSessionView,
        view_tx: watch::Sender<LiveSessionView>,
        commands: mpsc::UnboundedReceiver<LiveCommand>,
        factory: Arc<dyn SidecarFactoryPort>,
        model: Option<String>,
    ) -> Self {
        Self {
            view,
            view_tx,
            commands,
            factory,
            model,
            sidecar: None,
            dirty: false,
        }
    }

    pub(crate) async fn run(mut self) {
        loop {
            tokio::select! {
                // Priority order: commands, then sidecar events, then publishing. The view is
                // only published when neither of the others is immediately ready, so a burst of
                // events is applied first and published once.
                biased;
                command = self.commands.recv() => match command {
                    None | Some(LiveCommand::Close) => {
                        self.discard_sidecar(true).await;
                        break;
                    }
                    Some(command) => self.handle_command(command).await,
                },
                event = next_event(&self.sidecar) => match event {
                    Some(event) => self.apply_event(event).await,
                    None => self.fail("the sidecar process exited unexpectedly".to_string(), false).await,
                },
                _ = std::future::ready(()), if self.dirty => self.publish(),
            }
        }
    }

    fn publish(&mut self) {
        self.view_tx.send_replace(self.view.clone());
        self.dirty = false;
    }

    async fn handle_command(&mut self, command: LiveCommand) {
        match command {
            LiveCommand::Send(text) => self.send(text).await,
            LiveCommand::Approve { id, decision } => self.approve(id, decision).await,
            LiveCommand::Interrupt => self.interrupt().await,
            LiveCommand::Close => {}
        }
    }

    async fn send(&mut self, text: String) {
        if !self.view.can_send {
            self.view.last_error = Some(CANNOT_SEND.to_string());
            self.dirty = true;
            return;
        }
        if self.sidecar.is_none() {
            if let Err(message) = self.start_sidecar().await {
                self.fail(message, false).await;
                return;
            }
        }
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };

        let history_len = self.view.session.messages.len();
        match SendMessageUseCase::new(sidecar).execute(&mut self.view.session, text).await {
            Ok(()) => {
                self.view.phase = Phase::Running;
                self.view.last_error = None;
                self.dirty = true;
            }
            Err(error) => {
                // The use-case records the message before writing it; a message that was never
                // delivered must not stay in the history.
                self.view.session.messages.truncate(history_len);
                self.fail(error.to_string(), false).await;
            }
        }
    }

    // Spawns a sidecar and starts (or resumes) the session on it. The sidecar is only kept once
    // both steps succeed; otherwise it is dropped, which ends its process.
    async fn start_sidecar(&mut self) -> Result<(), String> {
        let sidecar = self.factory.spawn().await.map_err(|e| e.to_string())?;
        sidecar
            .start_session(StartSessionParams {
                cwd: self.view.project_path.to_string_lossy().to_string(),
                resume_session_id: self.view.claude_session_id.as_ref().map(|id| id.0.clone()),
                permission_mode: self.view.session.permission_mode,
                model: self.model.clone(),
            })
            .await
            .map_err(|e| e.to_string())?;
        self.sidecar = Some(sidecar);
        Ok(())
    }

    async fn approve(&mut self, id: String, decision: PermissionDecision) {
        let Some(position) = self.view.pending_permissions.iter().position(|request| request.id == id) else {
            return;
        };
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        self.view.pending_permissions.remove(position);
        self.dirty = true;
        if let Err(error) = ApprovePermissionUseCase::new(sidecar).execute(id, decision).await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn interrupt(&mut self) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        if let Err(error) = InterruptTurnUseCase::new(sidecar).execute().await {
            self.fail(error.to_string(), false).await;
        }
    }

    async fn apply_event(&mut self, event: SidecarEvent) {
        match apply_sidecar_event(&mut self.view.session, &event) {
            SessionUpdate::SessionStarted { session_id } => {
                self.view.session.id = session_id.clone();
                self.view.claude_session_id = Some(session_id);
            }
            SessionUpdate::PermissionRequested(request) => {
                self.view.pending_permissions.push_back(request);
            }
            SessionUpdate::TurnCompleted { .. } => self.view.phase = Phase::Idle,
            // The sidecar protocol does not distinguish fatal from non-fatal errors, so every
            // `Error` ends this sidecar: its query is over and `start_session` cannot be re-sent.
            SessionUpdate::Errored { message } => self.fail(message, true).await,
            SessionUpdate::SessionEnded => {
                self.fail("the session ended unexpectedly".to_string(), false).await;
            }
            SessionUpdate::MessageDelta { .. }
            | SessionUpdate::ToolCallStarted { .. }
            | SessionUpdate::ToolCallCompleted { .. } => {}
        }
        self.dirty = true;
    }

    // Marks the session failed and drops its sidecar. Permission requests that can no longer be
    // answered are cleared. `shutdown` asks a still-running sidecar to exit first.
    async fn fail(&mut self, message: String, shutdown: bool) {
        self.discard_sidecar(shutdown).await;
        self.view.phase = Phase::Failed;
        self.view.last_error = Some(message);
        self.view.pending_permissions.clear();
        self.dirty = true;
    }

    async fn discard_sidecar(&mut self, shutdown: bool) {
        if let Some(sidecar) = self.sidecar.take() {
            if shutdown {
                let _ = sidecar.shutdown().await;
            }
        }
    }
}

// Waits for the next sidecar event; with no sidecar it never resolves, so `select!` ignores it.
async fn next_event(sidecar: &Option<Arc<dyn SidecarPort>>) -> Option<SidecarEvent> {
    match sidecar {
        Some(sidecar) => sidecar.next_event().await,
        None => pending().await,
    }
}
```

- [ ] **Step 6: Run the tests and see them pass**

Run: `cargo test -p forge-application`
Expected: `50 passed` (29 before + 21 new), no warnings. Run it three or four times: these tests are asynchronous, and a flaky one is a bug in the test or the actor, not something to retry past.

- [ ] **Step 7: Check that the tests bite**

Break `live_session_actor.rs` in each way below, run `cargo test -p forge-application live_session::`, confirm the named test fails, then undo.

| Break this | This test must fail |
|---|---|
| `self.view.session.messages.truncate(history_len);` → `let _ = history_len;` | `a_failed_write_removes_the_undelivered_message_from_the_history` |
| delete `self.view.pending_permissions.clear();` from `fail` | `an_error_event_shuts_the_sidecar_down_clears_permissions_and_fails_the_session` |
| delete the `biased;` line in `run` | `a_burst_of_events_is_applied_completely_but_published_only_a_few_times` |
| `SessionUpdate::Errored { message } => self.fail(message, true).await` → `false` | `an_error_event_shuts_the_sidecar_down_clears_permissions_and_fails_the_session` |
| `resume_session_id: …` → `resume_session_id: None,` | `resuming_an_existing_session_passes_its_id_as_the_resume_session_id` and `sending_after_a_failure_respawns_and_resumes_with_the_known_session_id` |
| `if !self.view.can_send {` → `if false {` | `sending_to_a_session_whose_folder_is_missing_sets_an_error_and_never_spawns` |

- [ ] **Step 8: Commit**

```bash
git add crates/forge-application
git commit -m "feat: add LiveSession, one task per open session with a lazily spawned sidecar

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 6: `SessionRegistry`

**Files:**
- Create: `crates/forge-application/src/registry.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: Task 5's `spawn_live_session`, `LiveSessionInit`, `LiveSessionHandle` (`claude_session_id()`, `is_closed()`, `close()`, `id()`); `forge_ports::{SessionRepositoryPort, SessionRepositoryError, SidecarFactoryPort}`; `forge_domain::{ProjectSummary, Session, SessionId, PermissionMode}`; Task 5's `test_support`.
- Produces (re-exported from `lib.rs`): `SessionRegistry::new(factory: Arc<dyn SidecarFactoryPort>, sessions: Arc<dyn SessionRepositoryPort>)`; `open_new(&self, &ProjectSummary, PermissionMode, Option<String>) -> Result<LiveSessionHandle, OpenError>` (sync; needs a tokio runtime); `async open_existing(&self, &ProjectSummary, SessionId) -> Result<OpenOutcome, OpenError>`; `get(LiveSessionId) -> Option<LiveSessionHandle>`; `handles() -> Vec<LiveSessionHandle>` (oldest first); `close(LiveSessionId)`; `OpenOutcome { handle, already_open }`; `OpenError { ProjectMissing(PathBuf), Load(SessionRepositoryError) }`.

Behaviour (spec §15.5): opening never starts a process. `open_new` on a folder that no longer exists is `OpenError::ProjectMissing`. `open_existing` returns the existing handle (`already_open = true`) when a live session already holds that Claude session id — including a new session that has since learned its id — and otherwise loads the history read-only; `can_send` is the project's `exists`. Because loading is asynchronous, two racing opens of the same session are reconciled under the lock **after** loading, so exactly one handle is registered. The handle map lock is never held across an `.await`.

- [ ] **Step 1: Create `registry.rs` with the scaffold and all tests, and export it**

Replace `crates/forge-application/src/lib.rs` with:

```rust
mod event;
mod live_session;
mod live_session_actor;
mod registry;
mod use_cases;

#[cfg(test)]
mod test_support;

pub use event::{apply_sidecar_event, push_user_message, SessionUpdate};
pub use live_session::{
    LiveCommand, LiveSessionHandle, LiveSessionId, LiveSessionView, Phase,
};
pub use registry::{OpenError, OpenOutcome, SessionRegistry};
pub use use_cases::{
    ApprovePermissionUseCase, InterruptTurnUseCase, ResumeSessionError, ResumeSessionUseCase,
    SendMessageUseCase, StartSessionUseCase, ToggleMcpServerUseCase, TogglePluginUseCase,
    ToggleSkillUseCase,
};
```

`crates/forge-application/src/registry.rs` is **one file**: this scaffold followed by the test module.

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use forge_domain::{PermissionMode, ProjectSummary, SessionId};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort, SidecarFactoryPort};
use thiserror::Error;

use crate::live_session::{LiveSessionHandle, LiveSessionId};

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("the project folder no longer exists: {}", .0.display())]
    ProjectMissing(PathBuf),
    #[error("failed to load the session: {0}")]
    Load(#[from] SessionRepositoryError),
}

pub struct OpenOutcome {
    pub handle: LiveSessionHandle,
    /// `true` when the session was already open and `handle` is the existing one.
    pub already_open: bool,
}

pub struct SessionRegistry {
    factory: Arc<dyn SidecarFactoryPort>,
    sessions: Arc<dyn SessionRepositoryPort>,
    next_id: AtomicU64,
    handles: Mutex<HashMap<LiveSessionId, LiveSessionHandle>>,
}

impl SessionRegistry {
    pub fn new(factory: Arc<dyn SidecarFactoryPort>, sessions: Arc<dyn SessionRepositoryPort>) -> Self {
        Self { factory, sessions, next_id: AtomicU64::new(1), handles: Mutex::new(HashMap::new()) }
    }

    pub fn open_new(
        &self,
        project: &ProjectSummary,
        permission_mode: PermissionMode,
        model: Option<String>,
    ) -> Result<LiveSessionHandle, OpenError> {
        let _ = (&self.factory, &self.next_id, &self.handles, project, permission_mode, model);
        unimplemented!("implemented in Task 6, Step 3")
    }

    pub async fn open_existing(
        &self,
        project: &ProjectSummary,
        session_id: SessionId,
    ) -> Result<OpenOutcome, OpenError> {
        let _ = (&self.sessions, project, session_id);
        unimplemented!("implemented in Task 6, Step 3")
    }

    pub fn get(&self, id: LiveSessionId) -> Option<LiveSessionHandle> {
        let _ = id;
        unimplemented!("implemented in Task 6, Step 3")
    }

    pub fn handles(&self) -> Vec<LiveSessionHandle> {
        unimplemented!("implemented in Task 6, Step 3")
    }

    pub fn close(&self, id: LiveSessionId) {
        let _ = id;
        unimplemented!("implemented in Task 6, Step 3")
    }
}
```

```rust

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;
    use forge_domain::{
        Message, MessageRole, PermissionMode, ProjectSummary, Session, SessionId, SessionSummary,
    };
    use forge_ports::{
        MockSessionRepositoryPort, SessionRepositoryError, SessionRepositoryPort, SidecarError,
        SidecarEvent, SidecarPort,
    };

    use super::*;
    use crate::live_session::{LiveSessionId, Phase};
    use crate::test_support::{factory_returning, fake_sidecar, wait_closed, wait_for};

    fn project(path: &str, exists: bool) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 1,
            exists,
        }
    }

    fn past_session(id: &str) -> Session {
        Session {
            id: SessionId(id.to_string()),
            project_path: PathBuf::from("/work/a"),
            title: Some("A title".to_string()),
            messages: vec![
                Message { role: MessageRole::User, content: "earlier".to_string(), tool_calls: Vec::new() },
                Message { role: MessageRole::Assistant, content: "reply".to_string(), tool_calls: Vec::new() },
            ],
            permission_mode: PermissionMode::Default,
        }
    }

    fn repository_returning(session_id: &str, loads: usize) -> Arc<dyn SessionRepositoryPort> {
        let id = session_id.to_string();
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_load_session()
            .times(loads)
            .returning(move |_, _| Ok(past_session(&id)));
        Arc::new(repository)
    }

    fn registry(
        sidecars: Vec<Result<Arc<dyn SidecarPort>, SidecarError>>,
        sessions: Arc<dyn SessionRepositoryPort>,
    ) -> (SessionRegistry, Arc<AtomicUsize>) {
        let (factory, spawns) = factory_returning(sidecars);
        (SessionRegistry::new(factory, sessions), spawns)
    }

    #[tokio::test]
    async fn open_new_returns_an_empty_session_that_can_send() {
        let (registry, spawns) = registry(vec![], repository_returning("unused", 0));

        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, Some("m".to_string()))
            .unwrap();

        let view = handle.snapshot();
        assert!(view.session.messages.is_empty());
        assert_eq!(view.session.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(view.claude_session_id, None);
        assert!(view.can_send);
        assert_eq!(spawns.load(Ordering::SeqCst), 0, "opening spawns nothing");
        assert_eq!(registry.handles().len(), 1);
    }

    #[tokio::test]
    async fn open_new_rejects_a_missing_folder_and_registers_nothing() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));

        let result = registry.open_new(&project("/gone", false), PermissionMode::Default, None);

        assert!(matches!(result, Err(OpenError::ProjectMissing(path)) if path == Path::new("/gone")));
        assert!(registry.handles().is_empty());
    }

    #[tokio::test]
    async fn open_existing_loads_the_history_read_only_and_spawns_nothing() {
        let (registry, spawns) = registry(vec![], repository_returning("claude-1", 1));

        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!outcome.already_open);
        let view = outcome.handle.snapshot();
        assert_eq!(view.session.messages.len(), 2);
        assert_eq!(view.claude_session_id, Some(SessionId("claude-1".to_string())));
        assert_eq!(view.phase, Phase::Idle);
        assert!(view.can_send);
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn open_existing_for_a_missing_folder_is_view_only() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 1));

        let outcome = registry
            .open_existing(&project("/gone", false), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!outcome.handle.snapshot().can_send);
        assert_eq!(outcome.handle.snapshot().session.messages.len(), 2);
    }

    #[tokio::test]
    async fn opening_an_already_open_session_returns_the_same_handle_without_loading_again() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 1));
        let first = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        let second = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(second.already_open);
        assert_eq!(second.handle.id(), first.handle.id());
        assert_eq!(registry.handles().len(), 1);
    }

    #[tokio::test]
    async fn a_new_session_counts_as_open_once_it_has_learned_its_claude_id() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry(vec![Ok(sidecar)], repository_returning("unused", 0));
        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::Default, None)
            .unwrap();
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        control.push(SidecarEvent::SessionStarted { session_id: "claude-7".to_string() });
        wait_for(&mut view, "the session id", |v| v.claude_session_id.is_some()).await;

        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-7".to_string()))
            .await
            .unwrap();

        assert!(outcome.already_open);
        assert_eq!(outcome.handle.id(), handle.id());
    }

    #[tokio::test]
    async fn open_existing_propagates_a_load_failure_and_registers_nothing() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_load_session()
            .returning(|_, id| Err(SessionRepositoryError::NotFound(id.clone())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry
            .open_existing(&project("/work/a", true), SessionId("nope".to_string()))
            .await;

        assert!(matches!(result, Err(OpenError::Load(SessionRepositoryError::NotFound(_)))));
        assert!(registry.handles().is_empty());
    }

    #[tokio::test]
    async fn close_shuts_the_session_down_and_forgets_it_so_it_can_be_opened_again() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 2));
        let first = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let mut view = first.handle.subscribe();

        registry.close(first.handle.id());
        wait_closed(&mut view).await;
        let again = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!again.already_open);
        assert_ne!(again.handle.id(), first.handle.id());
        assert_eq!(registry.handles().len(), 1);
        assert!(registry.get(first.handle.id()).is_none());
    }

    #[tokio::test]
    async fn handles_are_listed_oldest_first() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));
        let a = registry.open_new(&project("/work/a", true), PermissionMode::Default, None).unwrap();
        let b = registry.open_new(&project("/work/b", true), PermissionMode::Default, None).unwrap();
        let c = registry.open_new(&project("/work/c", true), PermissionMode::Default, None).unwrap();

        let ids: Vec<LiveSessionId> = registry.handles().iter().map(|h| h.id()).collect();

        assert_eq!(ids, vec![a.id(), b.id(), c.id()]);
    }

    #[tokio::test]
    async fn sessions_run_independently_and_one_failing_leaves_the_other_running() {
        let (first_sidecar, first_control) = fake_sidecar();
        let (second_sidecar, second_control) = fake_sidecar();
        let (registry, spawns) = registry(
            vec![Ok(first_sidecar), Ok(second_sidecar)],
            repository_returning("unused", 0),
        );
        let first = registry.open_new(&project("/work/a", true), PermissionMode::Default, None).unwrap();
        let second = registry.open_new(&project("/work/b", true), PermissionMode::Default, None).unwrap();
        let mut first_view = first.subscribe();
        let mut second_view = second.subscribe();

        first.send("one".to_string());
        second.send("two".to_string());
        wait_for(&mut first_view, "first running", |v| v.phase == Phase::Running).await;
        wait_for(&mut second_view, "second running", |v| v.phase == Phase::Running).await;
        first_control.push(SidecarEvent::AssistantDelta { text: "from-first".to_string() });
        second_control.push(SidecarEvent::AssistantDelta { text: "from-second".to_string() });
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        let failed = wait_for(&mut first_view, "first failed", |v| v.phase == Phase::Failed).await;
        let running = wait_for(&mut second_view, "second's text", |v| {
            v.session.messages.iter().any(|m| m.content == "from-second")
        })
        .await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert!(failed.session.messages.iter().all(|m| m.content != "from-second"));
        assert_eq!(running.phase, Phase::Running);
        assert!(running.session.messages.iter().all(|m| m.content != "from-first"));
    }

    // A repository whose load yields once, so two `open_existing` calls really interleave.
    struct SlowRepository;

    #[async_trait]
    impl SessionRepositoryPort for SlowRepository {
        async fn list_sessions(&self, _project_path: &Path) -> Result<Vec<SessionSummary>, SessionRepositoryError> {
            Ok(Vec::new())
        }

        async fn load_session(&self, _project_path: &Path, session_id: &SessionId) -> Result<Session, SessionRepositoryError> {
            tokio::task::yield_now().await;
            Ok(past_session(&session_id.0))
        }
    }

    #[tokio::test]
    async fn two_concurrent_opens_of_the_same_session_end_up_with_one_handle() {
        let (registry, _spawns) = registry(vec![], Arc::new(SlowRepository));
        let project = project("/work/a", true);

        let (first, second) = tokio::join!(
            registry.open_existing(&project, SessionId("claude-1".to_string())),
            registry.open_existing(&project, SessionId("claude-1".to_string())),
        );
        let (first, second) = (first.unwrap(), second.unwrap());

        assert_eq!(registry.handles().len(), 1);
        assert_eq!(first.handle.id(), second.handle.id());
        assert_eq!(
            [first.already_open, second.already_open].iter().filter(|opened| **opened).count(),
            1,
            "exactly one of the two found the other's handle"
        );
    }
}
```

- [ ] **Step 2: Run the tests and see them fail**

Run: `cargo test -p forge-application registry::`
Expected: it compiles without warnings; `11 failed` with `not implemented: implemented in Task 6, Step 3`.

- [ ] **Step 3: Implement**

Replace everything in `registry.rs` **above** `#[cfg(test)] mod tests` (the scaffold) with:

```rust
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use forge_domain::{PermissionMode, ProjectSummary, Session, SessionId};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort, SidecarFactoryPort};
use thiserror::Error;

use crate::live_session::{spawn_live_session, LiveSessionHandle, LiveSessionId, LiveSessionInit};

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("the project folder no longer exists: {}", .0.display())]
    ProjectMissing(PathBuf),
    #[error("failed to load the session: {0}")]
    Load(#[from] SessionRepositoryError),
}

pub struct OpenOutcome {
    pub handle: LiveSessionHandle,
    /// `true` when the session was already open and `handle` is the existing one.
    pub already_open: bool,
}

/// Every open session. Opening never starts a process: a sidecar is spawned lazily by the
/// session itself on its first `send`.
pub struct SessionRegistry {
    factory: Arc<dyn SidecarFactoryPort>,
    sessions: Arc<dyn SessionRepositoryPort>,
    next_id: AtomicU64,
    // Never held across an `.await`.
    handles: Mutex<HashMap<LiveSessionId, LiveSessionHandle>>,
}

impl SessionRegistry {
    pub fn new(factory: Arc<dyn SidecarFactoryPort>, sessions: Arc<dyn SessionRepositoryPort>) -> Self {
        Self {
            factory,
            sessions,
            next_id: AtomicU64::new(1),
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Opens an empty session in `project`. Fails if the folder is gone: nothing can be started there.
    /// Must be called from inside a tokio runtime.
    pub fn open_new(
        &self,
        project: &ProjectSummary,
        permission_mode: PermissionMode,
        model: Option<String>,
    ) -> Result<LiveSessionHandle, OpenError> {
        if !project.exists {
            return Err(OpenError::ProjectMissing(project.path.clone()));
        }
        let session = Session {
            id: SessionId(String::new()),
            project_path: project.path.clone(),
            title: None,
            messages: Vec::new(),
            permission_mode,
        };
        let mut handles = self.handles.lock().unwrap();
        Ok(self.insert(&mut handles, project, session, None, model))
    }

    /// Opens a past session read-only, or returns the handle it already has. Two sidecars must
    /// never write the same transcript, so a Claude session id is opened at most once.
    /// Must be called from inside a tokio runtime.
    pub async fn open_existing(
        &self,
        project: &ProjectSummary,
        session_id: SessionId,
    ) -> Result<OpenOutcome, OpenError> {
        if let Some(handle) = self.find_open(&session_id) {
            return Ok(OpenOutcome { handle, already_open: true });
        }
        let session = self.sessions.load_session(&project.path, &session_id).await?;

        // Another open of the same session may have finished while this one was loading, so look
        // again under the lock before registering.
        let mut handles = self.handles.lock().unwrap();
        if let Some(handle) = find_in(&handles, &session_id) {
            return Ok(OpenOutcome { handle, already_open: true });
        }
        let handle = self.insert(&mut handles, project, session, Some(session_id), None);
        Ok(OpenOutcome { handle, already_open: false })
    }

    pub fn get(&self, id: LiveSessionId) -> Option<LiveSessionHandle> {
        self.handles.lock().unwrap().get(&id).cloned()
    }

    /// Open sessions, oldest first.
    pub fn handles(&self) -> Vec<LiveSessionHandle> {
        let mut handles: Vec<LiveSessionHandle> = self.handles.lock().unwrap().values().cloned().collect();
        handles.sort_by_key(|handle| handle.id());
        handles
    }

    /// Closes the session (which shuts its sidecar down) and forgets it.
    pub fn close(&self, id: LiveSessionId) {
        if let Some(handle) = self.handles.lock().unwrap().remove(&id) {
            handle.close();
        }
    }

    fn find_open(&self, session_id: &SessionId) -> Option<LiveSessionHandle> {
        find_in(&self.handles.lock().unwrap(), session_id)
    }

    fn insert(
        &self,
        handles: &mut HashMap<LiveSessionId, LiveSessionHandle>,
        project: &ProjectSummary,
        session: Session,
        claude_session_id: Option<SessionId>,
        model: Option<String>,
    ) -> LiveSessionHandle {
        let id = LiveSessionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let handle = spawn_live_session(LiveSessionInit {
            id,
            project_path: project.path.clone(),
            session,
            claude_session_id,
            model,
            can_send: project.exists,
            factory: self.factory.clone(),
        });
        handles.insert(id, handle.clone());
        handle
    }
}

// The open handle for a Claude session id, if any. A new session counts once it has learned its id.
fn find_in(handles: &HashMap<LiveSessionId, LiveSessionHandle>, session_id: &SessionId) -> Option<LiveSessionHandle> {
    handles
        .values()
        .find(|handle| !handle.is_closed() && handle.claude_session_id().as_ref() == Some(session_id))
        .cloned()
}
```

- [ ] **Step 4: Run the tests and see them pass**

Run: `cargo test -p forge-application`
Expected: `61 passed` (29 + 21 + 11), no warnings.

- [ ] **Step 5: Check that the tests bite**

| Break this in `registry.rs` | This test must fail |
|---|---|
| in `open_existing`, delete the second `find_in` check under the lock (the "another open finished while loading" block) | `two_concurrent_opens_of_the_same_session_end_up_with_one_handle` |
| in `open_new`, delete the `if !project.exists { … }` guard | `open_new_rejects_a_missing_folder_and_registers_nothing` |
| make `find_in` return `None` (`.find(\|_\| false)`) | `opening_an_already_open_session_returns_the_same_handle_without_loading_again`, `a_new_session_counts_as_open_once_it_has_learned_its_claude_id`, `two_concurrent_opens_of_the_same_session_end_up_with_one_handle` |
| `can_send: project.exists` → `can_send: true` | `open_existing_for_a_missing_folder_is_view_only` |
| in `close`, replace `handle.close();` with `let _ = handle;` | `close_shuts_the_session_down_and_forgets_it_so_it_can_be_opened_again` |
| delete `handles.sort_by_key(…)` from `handles()` | `handles_are_listed_oldest_first` (a `HashMap` iterates in random order, so run it a dozen times: it fails almost every time) |

- [ ] **Step 6: Whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'`
Expected: `198`, no warnings other than the `block v0.1.6` notice (it only appears once the Dioxus crates exist; before Task 7 there is no warning at all).

- [ ] **Step 7: Commit**

```bash
git add crates/forge-application
git commit -m "feat: add SessionRegistry that opens, de-duplicates and closes live sessions

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

- [ ] **Step 8: CHECKPOINT — stop and report to the user before Task 7**

The session core and both adapters are complete and fully tested; nothing above this line has a UI dependency. Report: the workspace test count (198), the smoke result from Task 3 Step 7, the spike findings and any amendments, and anything that surprised you. Ask whether to continue with the UI (Tasks 7–11) now. The user decided on a single plan (2026-09-29), so continuing is the default; the checkpoint exists so the core can be reviewed while it is still small.

---

### Task 7: `forge-shared` and `forge-entities`

**Files:**
- Create: `crates/forge-shared/{Cargo.toml, src/lib.rs, src/theme.rs, src/theme.css, src/components.rs}`, `crates/forge-entities/{Cargo.toml, src/lib.rs, src/tool_summary.rs, src/relative_time.rs, src/message.rs, src/permission.rs, src/sidebar_item.rs}`
- Modify: `Cargo.lock` (Dioxus and its dependencies enter the lock file here)

**Interfaces:**
- Consumes: `forge_domain::{Message, MessageRole, ToolCall, ToolResult, PermissionRequest}`; Dioxus 0.7.10 (`dioxus::prelude::*`, `#[component]`, `EventHandler`, `MouseEvent`, `KeyboardEvent`).
- Produces: `forge_shared::{THEME_CSS: &str, Button { label, kind, disabled, onclick }, ButtonKind { Default, Primary, Danger }, Select { value, options: Vec<(String, String)>, disabled, onchange: EventHandler<String> }, TextArea { value, placeholder, disabled, oninput: EventHandler<String>, onkeydown: EventHandler<KeyboardEvent> }, StatusDot { tone }, Tone { Muted, Active, Warning }}`; `forge_entities::{MessageBubble { message }, ToolCallLine { tool_call }, PermissionCard { request, children }, SidebarItem { title, subtitle, marker, active, onclick }, SessionMarker { Idle, Running, NeedsAttention }, tool_input_summary(&serde_json::Value) -> String, tool_call_status(&ToolCall) -> ToolCallStatus, ToolCallStatus { Running, Done, Failed }, relative_time(&str, time::OffsetDateTime) -> String}`.

These two crates hold presentation only. `forge-entities` must not depend on `forge-application` (spec §6/§15.6): the sidebar marker is a UI enum here, mapped from a session's phase by `forge-features`. Both UI library crates use `default-features = false, features = ["lib"]`; only `forge-app` (Task 10) turns on `desktop`. The theme is deliberately neutral — visual parity with Claude Code desktop is out of scope (§15.1).

- [ ] **Step 1: Create `forge-shared`**

`crates/forge-shared/Cargo.toml`:

```toml
[package]
name = "forge-shared"
version.workspace = true
edition.workspace = true

[dependencies]
dioxus = { version = "=0.7.10", default-features = false, features = ["lib"] }
```

`crates/forge-shared/src/lib.rs`:

```rust
mod components;
mod theme;

pub use components::{Button, ButtonKind, Select, StatusDot, TextArea, Tone};
pub use theme::THEME_CSS;
```

`crates/forge-shared/src/theme.rs` (its tests check that the stylesheet defines every class the components use, and has a dark mode):

```rust
/// The app's stylesheet. Neutral on purpose: visual parity with Claude Code desktop is out of
/// scope for the walking skeleton (spec §15.1).
pub const THEME_CSS: &str = include_str!("theme.css");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stylesheet_defines_the_layout_classes_the_components_use() {
        for class in [
            ".workspace",
            ".sidebar",
            ".sidebar-item",
            ".main-pane",
            ".messages",
            ".message",
            ".tool-call",
            ".permission-card",
            ".composer",
            ".status-dot",
            ".banner",
        ] {
            assert!(THEME_CSS.contains(class), "theme.css is missing {class}");
        }
    }

    #[test]
    fn the_stylesheet_has_a_dark_mode() {
        assert!(THEME_CSS.contains("prefers-color-scheme: dark"));
    }
}
```

`crates/forge-shared/src/theme.css`:

```css
:root {
  --bg: #ffffff;
  --bg-sidebar: #f5f5f4;
  --bg-hover: #e7e5e4;
  --fg: #1c1917;
  --fg-muted: #78716c;
  --border: #e7e5e4;
  --accent: #c2410c;
  --accent-fg: #ffffff;
  --warning: #ca8a04;
  --danger: #b91c1c;
  --radius: 8px;
  --font: -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --mono: ui-monospace, "SF Mono", Menlo, monospace;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #1c1917;
    --bg-sidebar: #171412;
    --bg-hover: #292524;
    --fg: #f5f5f4;
    --fg-muted: #a8a29e;
    --border: #292524;
    --accent: #fb923c;
    --accent-fg: #1c1917;
    --warning: #facc15;
    --danger: #f87171;
  }
}

html, body, #main { height: 100%; margin: 0; }
body { font-family: var(--font); font-size: 14px; background: var(--bg); color: var(--fg); }

.workspace { display: flex; height: 100vh; outline: none; }

.sidebar { width: 260px; flex: none; display: flex; flex-direction: column; overflow-y: auto; background: var(--bg-sidebar); border-right: 1px solid var(--border); padding: 8px 0; }
.sidebar-new { margin: 4px 12px 8px; }
.sidebar-group-title { padding: 10px 12px 4px; font-size: 12px; font-weight: 600; color: var(--fg-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.sidebar-group-title.missing { text-decoration: line-through; }
.sidebar-item { display: flex; align-items: center; gap: 8px; padding: 6px 12px; cursor: pointer; }
.sidebar-item:hover { background: var(--bg-hover); }
.sidebar-item.active { background: var(--bg-hover); font-weight: 600; }
.sidebar-item .title { flex: 1; min-width: 0; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.sidebar-item .subtitle { flex: none; font-size: 11px; color: var(--fg-muted); }

.status-dot { flex: none; width: 8px; height: 8px; border-radius: 50%; box-sizing: border-box; }
.status-dot.muted { border: 1px solid var(--fg-muted); }
.status-dot.active { background: var(--accent); }
.status-dot.warning { background: var(--warning); }

.main-pane { flex: 1; min-width: 0; display: flex; flex-direction: column; }
.pane-header { padding: 10px 24px; border-bottom: 1px solid var(--border); font-weight: 600; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.messages { flex: 1; overflow-y: auto; padding: 16px 24px; }
.message { margin-bottom: 16px; }
.message .role { font-size: 12px; color: var(--fg-muted); margin-bottom: 2px; }
.message .content { white-space: pre-wrap; overflow-wrap: anywhere; }
.tool-call { font-family: var(--mono); font-size: 12px; color: var(--fg-muted); margin-top: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.tool-call.failed { color: var(--danger); }

.permission-card { margin: 8px 24px; padding: 12px; border: 1px solid var(--warning); border-radius: var(--radius); }
.permission-card .tool { font-weight: 600; }
.permission-card .input { font-family: var(--mono); font-size: 12px; color: var(--fg-muted); margin: 4px 0 8px; overflow-wrap: anywhere; }
.permission-actions { display: flex; gap: 8px; }

.banner { margin: 8px 24px; padding: 8px 12px; border-radius: var(--radius); font-size: 13px; }
.banner.error { border: 1px solid var(--danger); color: var(--danger); }
.banner.info { border: 1px solid var(--border); color: var(--fg-muted); }

.composer { display: flex; flex-direction: column; gap: 8px; padding: 12px 24px; border-top: 1px solid var(--border); }
.composer-row { display: flex; align-items: center; gap: 8px; }
.composer-row .spacer { flex: 1; }
.textarea { width: 100%; box-sizing: border-box; min-height: 64px; padding: 8px; resize: vertical; font: inherit; color: inherit; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); }
.select { font: inherit; color: inherit; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); padding: 4px 8px; }

.button { font: inherit; color: inherit; background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); padding: 5px 12px; cursor: pointer; }
.button:hover:not(:disabled) { background: var(--bg-hover); }
.button:disabled { opacity: 0.5; cursor: default; }
.button.primary { background: var(--accent); color: var(--accent-fg); border-color: var(--accent); }
.button.danger { color: var(--danger); border-color: var(--danger); }

.new-session { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 12px; padding: 24px; max-width: 720px; margin: 0 auto; width: 100%; box-sizing: border-box; }
.new-session .field { display: flex; align-items: center; gap: 12px; }
.new-session .field label { width: 64px; color: var(--fg-muted); }
.empty-state { margin: auto; color: var(--fg-muted); }
```

`crates/forge-shared/src/components.rs`:

```rust
use dioxus::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonKind {
    #[default]
    Default,
    Primary,
    Danger,
}

impl ButtonKind {
    fn class(self) -> &'static str {
        match self {
            ButtonKind::Default => "button",
            ButtonKind::Primary => "button primary",
            ButtonKind::Danger => "button danger",
        }
    }
}

#[component]
pub fn Button(
    label: String,
    #[props(default)] kind: ButtonKind,
    #[props(default)] disabled: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: kind.class(),
            disabled,
            onclick: move |evt| onclick.call(evt),
            "{label}"
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Active,
    Warning,
}

impl Tone {
    fn class(self) -> &'static str {
        match self {
            Tone::Muted => "status-dot muted",
            Tone::Active => "status-dot active",
            Tone::Warning => "status-dot warning",
        }
    }
}

#[component]
pub fn StatusDot(tone: Tone) -> Element {
    rsx! { span { class: tone.class() } }
}

/// A dropdown over `(value, label)` pairs.
#[component]
pub fn Select(
    value: String,
    options: Vec<(String, String)>,
    #[props(default)] disabled: bool,
    onchange: EventHandler<String>,
) -> Element {
    rsx! {
        select {
            class: "select",
            disabled,
            value: "{value}",
            onchange: move |evt| onchange.call(evt.value()),
            for (option_value, label) in options.iter() {
                option {
                    key: "{option_value}",
                    value: "{option_value}",
                    selected: *option_value == value,
                    "{label}"
                }
            }
        }
    }
}

#[component]
pub fn TextArea(
    value: String,
    #[props(default)] placeholder: String,
    #[props(default)] disabled: bool,
    oninput: EventHandler<String>,
    #[props(default)] onkeydown: EventHandler<KeyboardEvent>,
) -> Element {
    rsx! {
        textarea {
            class: "textarea",
            value: "{value}",
            placeholder: "{placeholder}",
            disabled,
            oninput: move |evt| oninput.call(evt.value()),
            onkeydown: move |evt| onkeydown.call(evt),
        }
    }
}
```

- [ ] **Step 2: Build and test it**

Run: `cargo test -p forge-shared`
Expected: `2 passed`, no warnings from this crate. The first build downloads and compiles Dioxus (a minute or two); `Cargo.lock` changes.

- [ ] **Step 3: Create `forge-entities` (manifest, exports, and the components)**

`crates/forge-entities/Cargo.toml`:

```toml
[package]
name = "forge-entities"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-shared = { path = "../forge-shared" }
dioxus = { version = "=0.7.10", default-features = false, features = ["lib"] }
serde_json = "1"
time = { version = "0.3.55", features = ["parsing"] }
```

`crates/forge-entities/src/lib.rs`:

```rust
mod message;
mod permission;
mod relative_time;
mod sidebar_item;
mod tool_summary;

pub use message::{MessageBubble, ToolCallLine};
pub use permission::PermissionCard;
pub use relative_time::relative_time;
pub use sidebar_item::{SessionMarker, SidebarItem};
pub use tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};
```

`crates/forge-entities/src/message.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::{Message, MessageRole, ToolCall};

use crate::tool_summary::{tool_call_status, tool_input_summary, ToolCallStatus};

#[component]
pub fn MessageBubble(message: Message) -> Element {
    let (role_class, role_label) = match message.role {
        MessageRole::User => ("user", "You"),
        MessageRole::Assistant => ("assistant", "Claude"),
    };
    rsx! {
        div { class: "message {role_class}",
            div { class: "role", "{role_label}" }
            if !message.content.is_empty() {
                div { class: "content", "{message.content}" }
            }
            for tool_call in message.tool_calls.iter() {
                ToolCallLine { key: "{tool_call.id}", tool_call: tool_call.clone() }
            }
        }
    }
}

/// One collapsed line per tool call: `▸ Read  src/lib.rs`.
#[component]
pub fn ToolCallLine(tool_call: ToolCall) -> Element {
    let status_class = match tool_call_status(&tool_call) {
        ToolCallStatus::Running => "running",
        ToolCallStatus::Done => "done",
        ToolCallStatus::Failed => "failed",
    };
    let summary = tool_input_summary(&tool_call.input);
    rsx! {
        div { class: "tool-call {status_class}", "▸ {tool_call.name}  {summary}" }
    }
}
```

`crates/forge-entities/src/permission.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::PermissionRequest;

use crate::tool_summary::tool_input_summary;

/// Shows what Claude wants to do. The buttons are the caller's `children`, so this entity stays
/// presentational and the approve-permission feature supplies the behaviour.
#[component]
pub fn PermissionCard(request: PermissionRequest, children: Element) -> Element {
    let summary = tool_input_summary(&request.input);
    rsx! {
        div { class: "permission-card",
            div { class: "tool", "{request.tool_name}" }
            div { class: "input", "{summary}" }
            {children}
        }
    }
}
```

`crates/forge-entities/src/sidebar_item.rs`:

```rust
use dioxus::prelude::*;
use forge_shared::{StatusDot, Tone};

/// What the marker beside a sidebar entry says. Defined here (not in `forge-application`) so this
/// crate depends only on the domain and the UI kit; a widget maps a session's phase onto it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMarker {
    Idle,
    Running,
    NeedsAttention,
}

impl SessionMarker {
    fn tone(self) -> Tone {
        match self {
            SessionMarker::Idle => Tone::Muted,
            SessionMarker::Running => Tone::Active,
            SessionMarker::NeedsAttention => Tone::Warning,
        }
    }
}

#[component]
pub fn SidebarItem(
    title: String,
    subtitle: String,
    marker: SessionMarker,
    active: bool,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let class = if active { "sidebar-item active" } else { "sidebar-item" };
    rsx! {
        div { class, onclick: move |evt| onclick.call(evt),
            StatusDot { tone: marker.tone() }
            span { class: "title", "{title}" }
            span { class: "subtitle", "{subtitle}" }
        }
    }
}
```

- [ ] **Step 4: Create the two pure-logic modules as stubs plus their tests**

`crates/forge-entities/src/tool_summary.rs` is **one file**: the stub, immediately followed by the tests.

```rust
use forge_domain::ToolCall;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Done,
    Failed,
}

pub fn tool_input_summary(input: &Value) -> String {
    let _ = input;
    unimplemented!("implemented in Task 7, Step 6")
}

pub fn tool_call_status(tool_call: &ToolCall) -> ToolCallStatus {
    let _ = tool_call;
    unimplemented!("implemented in Task 7, Step 6")
}
```

```rust

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::ToolResult;
    use serde_json::json;

    fn call(result: Option<ToolResult>) -> ToolCall {
        ToolCall {
            id: "t-1".to_string(),
            name: "Bash".to_string(),
            input: json!({}),
            result,
        }
    }

    #[test]
    fn summary_prefers_the_command() {
        let input = json!({ "file_path": "a.rs", "command": "cargo test" });
        assert_eq!(tool_input_summary(&input), "cargo test");
    }

    #[test]
    fn summary_falls_back_through_the_known_keys() {
        assert_eq!(tool_input_summary(&json!({ "file_path": "src/lib.rs" })), "src/lib.rs");
        assert_eq!(tool_input_summary(&json!({ "pattern": "fn main" })), "fn main");
    }

    #[test]
    fn summary_falls_back_to_compact_json_for_unknown_shapes() {
        assert_eq!(tool_input_summary(&json!({ "n": 1 })), r#"{"n":1}"#);
        assert_eq!(tool_input_summary(&json!("plain")), r#""plain""#);
    }

    #[test]
    fn summary_of_nothing_is_empty() {
        assert_eq!(tool_input_summary(&Value::Null), "");
        assert_eq!(tool_input_summary(&json!({})), "");
    }

    #[test]
    fn summary_is_a_single_line() {
        assert_eq!(tool_input_summary(&json!({ "command": "a\nb" })), "a b");
    }

    #[test]
    fn summary_is_cut_to_80_characters_with_an_ellipsis() {
        let summary = tool_input_summary(&json!({ "command": "x".repeat(200) }));
        assert_eq!(summary.chars().count(), 80);
        assert!(summary.ends_with('…'));
    }

    #[test]
    fn summary_never_cuts_inside_a_multi_byte_character() {
        let summary = tool_input_summary(&json!({ "command": "한".repeat(200) }));
        assert_eq!(summary.chars().count(), 80);
        assert!(summary.starts_with('한'));
    }

    #[test]
    fn a_short_summary_is_left_alone() {
        assert_eq!(tool_input_summary(&json!({ "command": "ls" })), "ls");
    }

    #[test]
    fn status_follows_the_result() {
        assert_eq!(tool_call_status(&call(None)), ToolCallStatus::Running);
        assert_eq!(
            tool_call_status(&call(Some(ToolResult { output: String::new(), is_error: false }))),
            ToolCallStatus::Done
        );
        assert_eq!(
            tool_call_status(&call(Some(ToolResult { output: String::new(), is_error: true }))),
            ToolCallStatus::Failed
        );
    }
}
```

`crates/forge-entities/src/relative_time.rs` is **one file**: the stub, immediately followed by the tests.

```rust
use time::OffsetDateTime;

pub fn relative_time(updated_at: &str, now: OffsetDateTime) -> String {
    let _ = (updated_at, now);
    unimplemented!("implemented in Task 7, Step 6")
}
```

```rust

#[cfg(test)]
mod tests {
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-09-29T12:00:00Z", &Rfc3339).unwrap()
    }

    #[test]
    fn under_a_minute_is_just_now() {
        assert_eq!(relative_time("2026-09-29T11:59:30Z", now()), "just now");
    }

    #[test]
    fn a_timestamp_in_the_future_is_just_now() {
        assert_eq!(relative_time("2026-09-29T12:05:00Z", now()), "just now");
    }

    #[test]
    fn minutes_hours_and_days_are_relative() {
        assert_eq!(relative_time("2026-09-29T11:55:00Z", now()), "5m ago");
        assert_eq!(relative_time("2026-09-29T09:00:00Z", now()), "3h ago");
        assert_eq!(relative_time("2026-09-27T12:00:00Z", now()), "2d ago");
    }

    #[test]
    fn a_week_or_older_shows_the_date() {
        assert_eq!(relative_time("2026-09-22T12:00:00Z", now()), "2026-09-22");
        assert_eq!(relative_time("2025-01-05T08:00:00Z", now()), "2025-01-05");
    }

    #[test]
    fn fractional_seconds_are_understood() {
        assert_eq!(relative_time("2026-09-29T11:55:00.960415885Z", now()), "4m ago");
    }

    #[test]
    fn an_unparseable_timestamp_is_an_empty_label() {
        assert_eq!(relative_time("not a time", now()), "");
        assert_eq!(relative_time("", now()), "");
    }
}
```

- [ ] **Step 5: Run and see them fail**

Run: `cargo test -p forge-entities`
Expected: it compiles without warnings; `15 failed` (9 in `tool_summary`, 6 in `relative_time`) with `not implemented: implemented in Task 7, Step 6`.

- [ ] **Step 6: Implement**

In each file replace everything **above** `#[cfg(test)] mod tests` (the stub) with the implementation.

`tool_summary.rs`:

```rust
use forge_domain::ToolCall;
use serde_json::Value;

// The first of these keys with a string value is what a person wants to see about a tool call.
const SUMMARY_KEYS: [&str; 7] = [
    "command",
    "file_path",
    "path",
    "pattern",
    "url",
    "query",
    "description",
];
const MAX_SUMMARY_CHARS: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallStatus {
    Running,
    Done,
    Failed,
}

/// A one-line, at most 80-character description of a tool call's input.
pub fn tool_input_summary(input: &Value) -> String {
    let preferred = input
        .as_object()
        .and_then(|object| SUMMARY_KEYS.iter().find_map(|key| object.get(*key)?.as_str()))
        .map(str::to_string);
    let text = preferred.unwrap_or_else(|| match input {
        Value::Null => String::new(),
        Value::Object(object) if object.is_empty() => String::new(),
        other => other.to_string(),
    });
    truncate_chars(&text.replace('\n', " "), MAX_SUMMARY_CHARS)
}

// Counts characters, not bytes, so multi-byte text is never cut inside a character.
fn truncate_chars(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}

pub fn tool_call_status(tool_call: &ToolCall) -> ToolCallStatus {
    match &tool_call.result {
        None => ToolCallStatus::Running,
        Some(result) if result.is_error => ToolCallStatus::Failed,
        Some(_) => ToolCallStatus::Done,
    }
}
```

`relative_time.rs`:

```rust
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// "just now", "5m ago", "3h ago", "2d ago", or the date for anything older than a week. An
/// unparseable timestamp gives an empty string rather than an error: it is only a label.
pub fn relative_time(updated_at: &str, now: OffsetDateTime) -> String {
    let Ok(then) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
        return String::new();
    };
    let seconds = (now - then).whole_seconds();
    match seconds {
        s if s < 60 => "just now".to_string(),
        s if s < 3_600 => format!("{}m ago", s / 60),
        s if s < 86_400 => format!("{}h ago", s / 3_600),
        s if s < 7 * 86_400 => format!("{}d ago", s / 86_400),
        _ => format!("{:04}-{:02}-{:02}", then.year(), u8::from(then.month()), then.day()),
    }
}
```

- [ ] **Step 7: Run and see everything pass**

Run: `cargo test -p forge-entities` — Expected: `15 passed`.
Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'` — Expected: `215`. The only warning is the `block v0.1.6` notice.

- [ ] **Step 8: Commit** (`Cargo.lock` goes with it)

```bash
git add Cargo.lock crates/forge-shared crates/forge-entities
git commit -m "feat: add forge-shared UI kit and forge-entities presentational components

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 8: `forge-features` — workspace state, sidebar merge, shortcuts, interactions

**Files:**
- Create: `crates/forge-features/Cargo.toml`, `src/lib.rs`, `src/sidebar.rs`, `src/shortcut.rs`, `src/permission_mode.rs`, `src/workspace.rs`, `src/send_message.rs`, `src/approve_permission.rs`, `src/new_session.rs`

**Interfaces:**
- Consumes: from Tasks 5–6 `forge_application::{SessionRegistry, LiveSessionHandle, LiveSessionId, LiveSessionView, Phase, OpenError…}`; from Task 2 the repository ports and `ProjectSummary`; from Task 7 `forge_entities::SessionMarker` and `forge_shared::{Button, ButtonKind, Select, TextArea}`; `forge_domain::{PermissionDecision, PermissionMode, ProjectSummary, SessionId, SessionSummary}`.
- Produces (all re-exported from `lib.rs`):
  - state: `WorkspaceServices { registry: Arc<SessionRegistry>, projects: Arc<dyn ProjectRepositoryPort>, sessions: Arc<dyn SessionRepositoryPort> }` (`Clone`); `Active { NewSession, Live(LiveSessionId) }`; `Workspace` (`Copy`) with `active()`, `notice()`, `dismiss_notice()`, `sidebar() -> Vec<SidebarGroup>`, `new_session_projects() -> Vec<ProjectSummary>`, `live_view(id) -> Option<Signal<LiveSessionView>>`, `handle(id)`, `show_new_session()`, `select(id)`, `answer_permission(id, request_id, decision)`, `reload_past()`, `open_past(project, session_id)`, `start_new(project, mode, model, text)`, `close(id)`, `close_active()`, `cycle(forward)`, `on_key(&KeyboardEvent) -> bool`; `use_workspace_provider() -> Workspace` (call once, in the page) and `use_workspace() -> Workspace`.
  - pure logic: `EntryKey`, `SidebarEntry { key, session_id, title, updated_at, marker }`, `SidebarGroup { project, entries }`, `LiveSummary`, `marker_for`, `merge_sidebar`, `open_order`, `cycle_target`; `Shortcut`, `shortcut_for`; `mode_options`, `mode_value`, `parse_mode`.
  - components: `SendMessageBox { id, running, can_send }`, `PermissionActions { id, request_id }`, `NewSessionComposer {}`.

Design points worth knowing before reading the code (spec §15.6): each open session runs under tokio, while signals live on the UI thread, so `Workspace::attach` starts a Dioxus-local task per session that waits on the session's `watch` receiver and `set`s that session's signal; the sidebar is built from a small `LiveSummary` per open session so drawing it never clones a conversation; a past session is opened read-only and only gets a sidecar when its first message is sent; the message boxes ignore an Enter that confirms an IME composition (`is_composing()`); shortcuts are handled by `Workspace::on_key`, called from the page's `onkeydown`.

- [ ] **Step 1: Create the crate manifest and exports**

`crates/forge-features/Cargo.toml`:

```toml
[package]
name = "forge-features"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
forge-application = { path = "../forge-application" }
forge-entities = { path = "../forge-entities" }
forge-shared = { path = "../forge-shared" }
dioxus = { version = "=0.7.10", default-features = false, features = ["lib"] }

[dev-dependencies]
serde_json = "1"
```

`crates/forge-features/src/lib.rs`:

```rust
mod approve_permission;
mod new_session;
mod permission_mode;
mod send_message;
mod shortcut;
mod sidebar;
mod workspace;

pub use approve_permission::PermissionActions;
pub use new_session::NewSessionComposer;
pub use permission_mode::{mode_options, mode_value, parse_mode};
pub use send_message::SendMessageBox;
pub use shortcut::{shortcut_for, Shortcut};
pub use sidebar::{
    cycle_target, marker_for, merge_sidebar, open_order, EntryKey, LiveSummary, SidebarEntry,
    SidebarGroup,
};
pub use workspace::{use_workspace, use_workspace_provider, Active, Workspace, WorkspaceServices};
```

- [ ] **Step 2: Add the permission-mode table** (a static table; its three tests are round-trip checks, so there is no separate failing step)

`crates/forge-features/src/permission_mode.rs`:

```rust
use forge_domain::PermissionMode;

const MODES: [(PermissionMode, &str, &str); 4] = [
    (PermissionMode::Default, "default", "Ask before edits"),
    (PermissionMode::AcceptEdits, "accept_edits", "Accept edits"),
    (PermissionMode::Plan, "plan", "Plan mode"),
    (PermissionMode::BypassPermissions, "bypass_permissions", "Bypass permissions"),
];

/// `(value, label)` pairs for the mode selector.
pub fn mode_options() -> Vec<(String, String)> {
    MODES
        .iter()
        .map(|(_, value, label)| (value.to_string(), label.to_string()))
        .collect()
}

pub fn mode_value(mode: PermissionMode) -> &'static str {
    MODES
        .iter()
        .find(|(candidate, _, _)| *candidate == mode)
        .map(|(_, value, _)| *value)
        .unwrap_or("default")
}

/// An unknown value falls back to `Default`, the safest mode.
pub fn parse_mode(value: &str) -> PermissionMode {
    MODES
        .iter()
        .find(|(_, candidate, _)| *candidate == value)
        .map(|(mode, _, _)| *mode)
        .unwrap_or(PermissionMode::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_mode_round_trips_through_its_value() {
        for mode in [
            PermissionMode::Default,
            PermissionMode::AcceptEdits,
            PermissionMode::Plan,
            PermissionMode::BypassPermissions,
        ] {
            assert_eq!(parse_mode(mode_value(mode)), mode);
        }
    }

    #[test]
    fn an_unknown_value_falls_back_to_default() {
        assert_eq!(parse_mode("nonsense"), PermissionMode::Default);
        assert_eq!(parse_mode(""), PermissionMode::Default);
    }

    #[test]
    fn the_selector_offers_all_four_modes_with_distinct_values() {
        let options = mode_options();
        assert_eq!(options.len(), 4);
        let mut values: Vec<&String> = options.iter().map(|(value, _)| value).collect();
        values.sort();
        values.dedup();
        assert_eq!(values.len(), 4);
        assert_eq!(options[0].0, "default", "the safest mode is listed first");
    }
}
```

- [ ] **Step 3: Add `sidebar.rs` as a stub plus its tests**

`crates/forge-features/src/sidebar.rs` is **one file**: the stub, immediately followed by the tests.

```rust
use std::path::PathBuf;

use forge_application::{LiveSessionId, LiveSessionView};
use forge_domain::{ProjectSummary, SessionId, SessionSummary};
use forge_entities::SessionMarker;

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKey {
    Live(LiveSessionId),
    Past { project: PathBuf, session: SessionId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarEntry {
    pub key: EntryKey,
    pub session_id: Option<SessionId>,
    pub title: String,
    pub updated_at: Option<String>,
    pub marker: SessionMarker,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarGroup {
    pub project: ProjectSummary,
    pub entries: Vec<SidebarEntry>,
}

/// The few facts about an open session that the sidebar needs, so drawing it never clones a
/// whole conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSummary {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub claude_session_id: Option<SessionId>,
    pub title: Option<String>,
    pub first_user_message: Option<String>,
    pub marker: SessionMarker,
    pub can_send: bool,
}

impl LiveSummary {
    pub fn from_view(view: &LiveSessionView) -> Self {
        let _ = view;
        unimplemented!("implemented in Task 8, Step 7")
    }
}

pub fn marker_for(view: &LiveSessionView) -> SessionMarker {
    let _ = view;
    unimplemented!("implemented in Task 8, Step 7")
}

pub fn merge_sidebar(
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    live: &[LiveSummary],
) -> Vec<SidebarGroup> {
    let _ = (past, live);
    unimplemented!("implemented in Task 8, Step 7")
}

pub fn open_order(groups: &[SidebarGroup]) -> Vec<LiveSessionId> {
    let _ = groups;
    unimplemented!("implemented in Task 8, Step 7")
}

pub fn cycle_target(
    order: &[LiveSessionId],
    current: Option<LiveSessionId>,
    forward: bool,
) -> Option<LiveSessionId> {
    let _ = (order, current, forward);
    unimplemented!("implemented in Task 8, Step 7")
}
```

```rust

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::PathBuf;

    use forge_application::{LiveSessionId, LiveSessionView, Phase};
    use forge_domain::{
        Message, MessageRole, PermissionMode, PermissionRequest, ProjectSummary, Session, SessionId,
        SessionSummary,
    };
    use forge_entities::SessionMarker;

    use super::*;

    fn project(path: &str) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 0,
            exists: true,
        }
    }

    fn summary(id: &str, title: Option<&str>) -> SessionSummary {
        SessionSummary {
            id: SessionId(id.to_string()),
            title: title.map(str::to_string),
            updated_at: format!("2026-09-29T00:00:0{}Z", id.len() % 10),
        }
    }

    fn open(id: u64, path: &str, claude_id: Option<&str>, marker: SessionMarker) -> LiveSummary {
        LiveSummary {
            id: LiveSessionId(id),
            project_path: PathBuf::from(path),
            claude_session_id: claude_id.map(|id| SessionId(id.to_string())),
            title: None,
            first_user_message: None,
            marker,
            can_send: true,
        }
    }

    fn view(phase: Phase, pending: usize) -> LiveSessionView {
        let request = PermissionRequest {
            id: "p".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({}),
        };
        LiveSessionView {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: Session {
                id: SessionId(String::new()),
                project_path: PathBuf::from("/work/a"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            },
            claude_session_id: None,
            phase,
            pending_permissions: VecDeque::from(vec![request; pending]),
            last_error: None,
            can_send: true,
        }
    }

    fn titles(group: &SidebarGroup) -> Vec<&str> {
        group.entries.iter().map(|entry| entry.title.as_str()).collect()
    }

    #[test]
    fn groups_and_entries_keep_the_order_they_were_given() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("first")), summary("s-22", Some("second"))]),
            (project("/work/b"), vec![summary("s-333", Some("third"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project.path, PathBuf::from("/work/a"));
        assert_eq!(titles(&groups[0]), vec!["first", "second"]);
        assert_eq!(titles(&groups[1]), vec!["third"]);
        assert!(groups[0].entries.iter().all(|e| e.marker == SessionMarker::Idle));
    }

    #[test]
    fn an_untitled_past_session_gets_a_placeholder() {
        let past = vec![(project("/work/a"), vec![summary("s-1", None)])];

        assert_eq!(titles(&merge_sidebar(&past, &[])[0]), vec!["Untitled session"]);
    }

    #[test]
    fn an_open_session_replaces_the_past_entry_with_the_same_claude_id() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("old title")), summary("s-2", Some("other"))])];
        let live = vec![open(7, "/work/a", Some("s-1"), SessionMarker::Running)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entries.len(), 2, "replaced in place, not duplicated");
        assert_eq!(groups[0].entries[0].key, EntryKey::Live(LiveSessionId(7)));
        assert_eq!(groups[0].entries[0].marker, SessionMarker::Running);
        assert_eq!(groups[0].entries[0].title, "old title");
        assert!(matches!(groups[0].entries[1].key, EntryKey::Past { .. }));
    }

    #[test]
    fn a_brand_new_open_session_goes_to_the_top_of_its_projects_group() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("first"))])];
        let live = vec![open(7, "/work/a", None, SessionMarker::Idle)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 1);
        assert_eq!(titles(&groups[0]), vec!["New session", "first"]);
        assert_eq!(groups[0].entries[0].updated_at, None);
    }

    #[test]
    fn open_sessions_for_an_unlisted_project_form_a_new_group_at_the_top() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("first"))])];
        let live = vec![open(7, "/work/z", None, SessionMarker::Idle), open(8, "/work/z", None, SessionMarker::Running)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project.path, PathBuf::from("/work/z"));
        assert_eq!(groups[0].entries.len(), 2);
        assert_eq!(groups[0].entries[0].key, EntryKey::Live(LiveSessionId(7)), "in the order opened");
        assert_eq!(groups[1].project.path, PathBuf::from("/work/a"));
    }

    #[test]
    fn a_new_session_is_titled_by_its_first_message() {
        let mut new = open(7, "/work/a", None, SessionMarker::Idle);
        new.first_user_message = Some("fix the login bug".to_string());

        let groups = merge_sidebar(&[], &[new]);

        assert_eq!(titles(&groups[0]), vec!["fix the login bug"]);
    }

    #[test]
    fn a_blank_first_message_falls_back_to_new_session() {
        let mut new = open(7, "/work/a", None, SessionMarker::Idle);
        new.first_user_message = Some("   ".to_string());

        assert_eq!(titles(&merge_sidebar(&[], &[new])[0]), vec!["New session"]);
    }

    #[test]
    fn a_session_listed_under_two_projects_appears_once() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("dup"))]),
            (project("/work/b"), vec![summary("s-1", Some("dup")), summary("s-22", Some("kept"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(titles(&groups[0]), vec!["dup"]);
        assert_eq!(titles(&groups[1]), vec!["kept"]);
    }

    #[test]
    fn two_spellings_of_one_project_path_merge_into_one_group() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("one"))]),
            (project("/work/a"), vec![summary("s-22", Some("two"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(groups.len(), 1);
        assert_eq!(titles(&groups[0]), vec!["one", "two"]);
    }

    #[test]
    fn markers_rank_needs_attention_above_running_above_idle() {
        assert_eq!(marker_for(&view(Phase::Idle, 0)), SessionMarker::Idle);
        assert_eq!(marker_for(&view(Phase::Running, 0)), SessionMarker::Running);
        assert_eq!(marker_for(&view(Phase::Running, 1)), SessionMarker::NeedsAttention);
        assert_eq!(marker_for(&view(Phase::Failed, 0)), SessionMarker::NeedsAttention);
    }

    #[test]
    fn a_summary_carries_only_what_the_sidebar_needs() {
        let mut open_view = view(Phase::Running, 0);
        open_view.session.messages.push(Message {
            role: MessageRole::User,
            content: "한".repeat(100),
            tool_calls: Vec::new(),
        });

        let summary = LiveSummary::from_view(&open_view);

        assert_eq!(summary.marker, SessionMarker::Running);
        assert_eq!(summary.first_user_message.as_ref().unwrap().chars().count(), 40);
        assert!(summary.can_send);
    }

    #[test]
    fn open_order_follows_the_sidebar_and_skips_past_entries() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("a1")), summary("s-22", Some("a2"))]),
            (project("/work/b"), vec![summary("s-333", Some("b1"))]),
        ];
        let live = vec![
            open(1, "/work/b", Some("s-333"), SessionMarker::Idle),
            open(2, "/work/a", Some("s-22"), SessionMarker::Idle),
        ];

        let order = open_order(&merge_sidebar(&past, &live));

        assert_eq!(order, vec![LiveSessionId(2), LiveSessionId(1)]);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let order = [LiveSessionId(1), LiveSessionId(2), LiveSessionId(3)];

        assert_eq!(cycle_target(&order, Some(LiveSessionId(1)), true), Some(LiveSessionId(2)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(3)), true), Some(LiveSessionId(1)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(1)), false), Some(LiveSessionId(3)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(2)), false), Some(LiveSessionId(1)));
    }

    #[test]
    fn cycling_with_no_current_session_starts_at_an_end() {
        let order = [LiveSessionId(1), LiveSessionId(2)];

        assert_eq!(cycle_target(&order, None, true), Some(LiveSessionId(1)));
        assert_eq!(cycle_target(&order, None, false), Some(LiveSessionId(2)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(99)), true), Some(LiveSessionId(1)));
    }

    #[test]
    fn cycling_with_nothing_open_is_none() {
        assert_eq!(cycle_target(&[], None, true), None);
        assert_eq!(cycle_target(&[], Some(LiveSessionId(1)), false), None);
    }

    #[test]
    fn cycling_with_one_open_session_stays_on_it() {
        let order = [LiveSessionId(5)];

        assert_eq!(cycle_target(&order, Some(LiveSessionId(5)), true), Some(LiveSessionId(5)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(5)), false), Some(LiveSessionId(5)));
    }
}
```

- [ ] **Step 4: Add `shortcut.rs` as a stub plus its tests**

`crates/forge-features/src/shortcut.rs` is **one file**: the stub, immediately followed by the tests.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    NextSession,
    PreviousSession,
    CloseSession,
}

pub fn shortcut_for(key: &str, ctrl: bool, meta: bool, shift: bool, mac: bool) -> Option<Shortcut> {
    let _ = (key, ctrl, meta, shift, mac);
    unimplemented!("implemented in Task 8, Step 7")
}
```

```rust

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmd_n_and_cmd_w_on_a_mac() {
        assert_eq!(shortcut_for("n", false, true, false, true), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for("w", false, true, false, true), Some(Shortcut::CloseSession));
        assert_eq!(shortcut_for("N", false, true, false, true), Some(Shortcut::NewSession), "caps lock");
    }

    #[test]
    fn ctrl_n_and_ctrl_w_elsewhere() {
        assert_eq!(shortcut_for("n", true, false, false, false), Some(Shortcut::NewSession));
        assert_eq!(shortcut_for("w", true, false, false, false), Some(Shortcut::CloseSession));
    }

    #[test]
    fn the_wrong_modifier_for_the_platform_does_nothing() {
        assert_eq!(shortcut_for("n", true, false, false, true), None, "Ctrl+N on a Mac");
        assert_eq!(shortcut_for("w", false, true, false, false), None, "Meta+W elsewhere");
    }

    #[test]
    fn plain_keys_and_shifted_commands_do_nothing() {
        assert_eq!(shortcut_for("n", false, false, false, true), None);
        assert_eq!(shortcut_for("n", false, true, true, true), None, "Cmd+Shift+N");
        assert_eq!(shortcut_for("x", false, true, false, true), None);
    }

    #[test]
    fn ctrl_tab_cycles_on_every_platform_and_shift_reverses() {
        for mac in [true, false] {
            assert_eq!(shortcut_for("Tab", true, false, false, mac), Some(Shortcut::NextSession));
            assert_eq!(shortcut_for("Tab", true, false, true, mac), Some(Shortcut::PreviousSession));
        }
    }

    #[test]
    fn tab_without_ctrl_is_left_alone() {
        assert_eq!(shortcut_for("Tab", false, false, false, true), None);
        assert_eq!(shortcut_for("Tab", false, true, false, true), None);
    }
}
```

- [ ] **Step 5: Add the hook and the three interaction components** (compile-checked only; nothing here can be unit-tested without a renderer)

`crates/forge-features/src/workspace.rs`:

```rust
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{LiveSessionHandle, LiveSessionId, LiveSessionView, SessionRegistry};
use forge_domain::{PermissionDecision, PermissionMode, ProjectSummary, SessionId, SessionSummary};
use forge_ports::{ProjectRepositoryPort, SessionRepositoryPort};

use crate::shortcut::{shortcut_for, Shortcut};
use crate::sidebar::{cycle_target, merge_sidebar, open_order, LiveSummary, SidebarGroup};

/// Everything the workspace needs from the outside, injected once by the composition root.
#[derive(Clone)]
pub struct WorkspaceServices {
    pub registry: Arc<SessionRegistry>,
    pub projects: Arc<dyn ProjectRepositoryPort>,
    pub sessions: Arc<dyn SessionRepositoryPort>,
}

/// What the main pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Active {
    NewSession,
    Live(LiveSessionId),
}

#[derive(Clone)]
struct LiveEntry {
    handle: LiveSessionHandle,
    view: Signal<LiveSessionView>,
}

/// The UI-side session store: the sidebar's past sessions, the open sessions and the active one.
/// It is `Copy` (every field is a signal), so event handlers can capture it freely.
#[derive(Clone, Copy)]
pub struct Workspace {
    services: Signal<WorkspaceServices>,
    past: Signal<Vec<(ProjectSummary, Vec<SessionSummary>)>>,
    live: Signal<Vec<LiveEntry>>,
    active: Signal<Active>,
    notice: Signal<Option<String>>,
}

/// Creates the workspace, provides it to every descendant and loads the sidebar once. Call it in
/// the page at the top of the tree; descendants call `use_workspace`.
pub fn use_workspace_provider() -> Workspace {
    let services = use_context::<WorkspaceServices>();
    let workspace = use_hook(move || Workspace {
        services: Signal::new(services),
        past: Signal::new(Vec::new()),
        live: Signal::new(Vec::new()),
        active: Signal::new(Active::NewSession),
        notice: Signal::new(None),
    });
    use_context_provider(move || workspace);
    use_hook(move || workspace.reload_past());
    workspace
}

pub fn use_workspace() -> Workspace {
    use_context::<Workspace>()
}

impl Workspace {
    pub fn active(&self) -> Active {
        *self.active.read()
    }

    /// A message for the user (a failed open, a failed listing), shown until dismissed.
    pub fn notice(&self) -> Option<String> {
        self.notice.read().clone()
    }

    pub fn dismiss_notice(&self) {
        let mut notice = self.notice;
        notice.set(None);
    }

    /// The sidebar as drawn: past sessions with the open ones merged in.
    pub fn sidebar(&self) -> Vec<SidebarGroup> {
        let open: Vec<LiveSummary> = self
            .live
            .read()
            .iter()
            .map(|entry| LiveSummary::from_view(&entry.view.read()))
            .collect();
        merge_sidebar(&self.past.read(), &open)
    }

    /// The project folders a new session can start in: known projects that still exist.
    pub fn new_session_projects(&self) -> Vec<ProjectSummary> {
        self.past
            .read()
            .iter()
            .filter(|(project, _)| project.exists)
            .map(|(project, _)| project.clone())
            .collect()
    }

    pub fn live_view(&self, id: LiveSessionId) -> Option<Signal<LiveSessionView>> {
        self.live
            .read()
            .iter()
            .find(|entry| entry.handle.id() == id)
            .map(|entry| entry.view)
    }

    pub fn handle(&self, id: LiveSessionId) -> Option<LiveSessionHandle> {
        self.live
            .read()
            .iter()
            .find(|entry| entry.handle.id() == id)
            .map(|entry| entry.handle.clone())
    }

    pub fn show_new_session(&self) {
        let mut active = self.active;
        active.set(Active::NewSession);
    }

    pub fn select(&self, id: LiveSessionId) {
        let mut active = self.active;
        active.set(Active::Live(id));
    }

    pub fn answer_permission(&self, id: LiveSessionId, request_id: String, decision: PermissionDecision) {
        if let Some(handle) = self.handle(id) {
            handle.approve(request_id, decision);
        }
    }

    /// Reloads the sidebar's past sessions from disk.
    pub fn reload_past(&self) {
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        let mut notice = self.notice;
        spawn(async move {
            let (loaded, problem) = load_past(&services).await;
            past.set(loaded);
            if let Some(problem) = problem {
                notice.set(Some(problem));
            }
        });
    }

    /// Opens a past session read-only (or focuses it if it is already open).
    pub fn open_past(&self, project: ProjectSummary, session_id: SessionId) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            match services.registry.open_existing(&project, session_id).await {
                Ok(outcome) => {
                    let id = this.ensure_attached(outcome.handle);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Opens a new session in `project` and sends its first message.
    pub fn start_new(&self, project: ProjectSummary, mode: PermissionMode, model: Option<String>, text: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            match services.registry.open_new(&project, mode, model) {
                Ok(handle) => {
                    let id = this.attach(handle.clone());
                    handle.send(text);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    pub fn close(&self, id: LiveSessionId) {
        let services = (*self.services.peek()).clone();
        services.registry.close(id);
        let mut live = self.live;
        live.write().retain(|entry| entry.handle.id() != id);
        if self.active() == Active::Live(id) {
            let next = self.live.peek().last().map(|entry| entry.handle.id());
            match next {
                Some(next) => self.select(next),
                None => self.show_new_session(),
            }
        }
        self.reload_past();
    }

    pub fn close_active(&self) {
        if let Active::Live(id) = self.active() {
            self.close(id);
        }
    }

    /// Focuses the next (or previous) open session in the order the sidebar draws them.
    pub fn cycle(&self, forward: bool) {
        let order = open_order(&self.sidebar());
        let current = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession => None,
        };
        if let Some(target) = cycle_target(&order, current, forward) {
            self.select(target);
        }
    }

    /// Handles the session shortcuts; returns whether the key press was one of them.
    pub fn on_key(&self, event: &KeyboardEvent) -> bool {
        let key = match event.key() {
            Key::Tab => "Tab".to_string(),
            Key::Character(text) => text,
            _ => return false,
        };
        let modifiers = event.modifiers();
        let Some(shortcut) = shortcut_for(
            &key,
            modifiers.ctrl(),
            modifiers.meta(),
            modifiers.shift(),
            cfg!(target_os = "macos"),
        ) else {
            return false;
        };
        match shortcut {
            Shortcut::NewSession => self.show_new_session(),
            Shortcut::NextSession => self.cycle(true),
            Shortcut::PreviousSession => self.cycle(false),
            Shortcut::CloseSession => self.close_active(),
        }
        event.prevent_default();
        true
    }

    fn report(&self, message: String) {
        let mut notice = self.notice;
        notice.set(Some(message));
    }

    fn ensure_attached(&self, handle: LiveSessionHandle) -> LiveSessionId {
        let id = handle.id();
        if self.handle(id).is_some() {
            id
        } else {
            self.attach(handle)
        }
    }

    // Adds the session to the store and starts the task that copies its published state into a
    // signal. The session runs under tokio; this task runs on the UI thread, which is why the
    // state crosses through a `watch` channel instead of the session touching a signal itself.
    fn attach(&self, handle: LiveSessionHandle) -> LiveSessionId {
        let id = handle.id();
        let mut view = Signal::new_in_scope(handle.snapshot(), ScopeId::APP);
        let mut live = self.live;
        live.write().push(LiveEntry { handle: handle.clone(), view });
        spawn(async move {
            let mut updates = handle.subscribe();
            while updates.changed().await.is_ok() {
                let latest = updates.borrow_and_update().clone();
                view.set(latest);
            }
        });
        id
    }
}

// Every known project with its sessions. A project whose listing fails is kept (empty) and the
// first failure is reported, so one bad directory never blanks the sidebar.
async fn load_past(
    services: &WorkspaceServices,
) -> (Vec<(ProjectSummary, Vec<SessionSummary>)>, Option<String>) {
    let projects = match services.projects.list_projects().await {
        Ok(projects) => projects,
        Err(error) => return (Vec::new(), Some(error.to_string())),
    };
    let mut loaded = Vec::with_capacity(projects.len());
    let mut problem = None;
    for project in projects {
        match services.sessions.list_sessions(&project.path).await {
            Ok(sessions) => loaded.push((project, sessions)),
            Err(error) => {
                problem.get_or_insert_with(|| format!("{}: {error}", project.path.display()));
                loaded.push((project, Vec::new()));
            }
        }
    }
    (loaded, problem)
}
```

`crates/forge-features/src/send_message.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_shared::{Button, ButtonKind, TextArea};

use crate::workspace::use_workspace;

/// The message box of an open session: Enter sends, Shift+Enter adds a line, and while a turn is
/// running a Stop button interrupts it.
#[component]
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool) -> Element {
    let workspace = use_workspace();
    let mut draft = use_signal(String::new);

    let mut submit = move || {
        let text = draft.read().trim().to_string();
        if text.is_empty() {
            return;
        }
        if let Some(handle) = workspace.handle(id) {
            handle.send(text);
            draft.set(String::new());
        }
    };
    let stop = move |_| {
        if let Some(handle) = workspace.handle(id) {
            handle.interrupt();
        }
    };

    rsx! {
        div { class: "composer",
            TextArea {
                value: draft(),
                placeholder: "Message Claude…",
                disabled: !can_send,
                oninput: move |text| draft.set(text),
                onkeydown: move |event: KeyboardEvent| {
                    // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                    if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                        event.prevent_default();
                        submit();
                    }
                },
            }
            div { class: "composer-row",
                span { class: "spacer" }
                if running {
                    Button { label: "Stop", kind: ButtonKind::Danger, onclick: stop }
                }
                Button {
                    label: "Send",
                    kind: ButtonKind::Primary,
                    disabled: !can_send,
                    onclick: move |_| submit(),
                }
            }
        }
    }
}
```

`crates/forge-features/src/approve_permission.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::PermissionDecision;
use forge_shared::{Button, ButtonKind};

use crate::workspace::use_workspace;

/// Allow once / allow always / deny for one permission request.
#[component]
pub fn PermissionActions(id: LiveSessionId, request_id: String) -> Element {
    let workspace = use_workspace();
    let (once, always, deny) = (request_id.clone(), request_id.clone(), request_id);

    rsx! {
        div { class: "permission-actions",
            Button {
                label: "Allow",
                kind: ButtonKind::Primary,
                onclick: move |_| workspace.answer_permission(id, once.clone(), PermissionDecision::AllowOnce),
            }
            Button {
                label: "Always allow",
                onclick: move |_| workspace.answer_permission(id, always.clone(), PermissionDecision::AllowAlways),
            }
            Button {
                label: "Deny",
                kind: ButtonKind::Danger,
                onclick: move |_| workspace.answer_permission(id, deny.clone(), PermissionDecision::Deny),
            }
        }
    }
}
```

`crates/forge-features/src/new_session.rs`:

```rust
use dioxus::prelude::*;
use forge_shared::{Button, ButtonKind, Select, TextArea};

use crate::permission_mode::{mode_options, parse_mode};
use crate::workspace::use_workspace;

/// The new-session panel: pick a folder and permission mode, type the first message. The session
/// is only created when that message is sent.
#[component]
pub fn NewSessionComposer() -> Element {
    let workspace = use_workspace();
    let mut folder = use_signal(|| None::<String>);
    let mut mode = use_signal(|| "default".to_string());
    let mut text = use_signal(String::new);

    let projects = workspace.new_session_projects();
    let options: Vec<(String, String)> = projects
        .iter()
        .map(|project| {
            let path = project.path.to_string_lossy().to_string();
            (path.clone(), path)
        })
        .collect();
    let selected = folder
        .read()
        .clone()
        .or_else(|| options.first().map(|(value, _)| value.clone()))
        .unwrap_or_default();

    let mut submit = move || {
        let body = text.read().trim().to_string();
        if body.is_empty() {
            return;
        }
        let projects = workspace.new_session_projects();
        let chosen = match folder.read().clone() {
            Some(path) => projects.into_iter().find(|project| project.path.to_string_lossy() == path),
            None => projects.into_iter().next(),
        };
        let Some(project) = chosen else {
            return;
        };
        workspace.start_new(project, parse_mode(&mode.read()), None, body);
        text.set(String::new());
    };
    let has_folder = !options.is_empty();

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", "No existing project folder was found in your Claude Code history." }
            }
            div { class: "field",
                label { "Folder" }
                Select {
                    value: selected,
                    options,
                    disabled: !has_folder,
                    onchange: move |path| folder.set(Some(path)),
                }
            }
            div { class: "field",
                label { "Model" }
                // Only the default model is offered: nothing lists models yet (spec §15.1).
                Select {
                    value: "default",
                    options: vec![("default".to_string(), "Default model".to_string())],
                    onchange: |_| {},
                }
            }
            div { class: "field",
                label { "Mode" }
                Select {
                    value: mode(),
                    options: mode_options(),
                    onchange: move |value| mode.set(value),
                }
            }
            TextArea {
                value: text(),
                placeholder: "What should Claude do?",
                disabled: !has_folder,
                oninput: move |value| text.set(value),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                        event.prevent_default();
                        submit();
                    }
                },
            }
            div { class: "composer-row",
                span { class: "spacer" }
                Button {
                    label: "Start",
                    kind: ButtonKind::Primary,
                    disabled: !has_folder,
                    onclick: move |_| submit(),
                }
            }
        }
    }
}
```

- [ ] **Step 6: Run and see the logic tests fail**

Run: `cargo test -p forge-features`
Expected: it compiles without warnings; `22 failed, 3 passed` — 16 in `sidebar::`, 6 in `shortcut::` (all `not implemented: implemented in Task 8, Step 7`), and the 3 `permission_mode::` tests pass.

- [ ] **Step 7: Implement the two pure modules**

In `sidebar.rs` replace everything **above** `#[cfg(test)] mod tests` (the stub) with:

```rust
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use forge_application::{LiveSessionId, LiveSessionView, Phase};
use forge_domain::{MessageRole, ProjectSummary, SessionId, SessionSummary};
use forge_entities::SessionMarker;

const UNTITLED: &str = "Untitled session";
const NEW_SESSION: &str = "New session";
const MAX_TITLE_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKey {
    Live(LiveSessionId),
    Past { project: PathBuf, session: SessionId },
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarEntry {
    pub key: EntryKey,
    pub session_id: Option<SessionId>,
    pub title: String,
    pub updated_at: Option<String>,
    pub marker: SessionMarker,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarGroup {
    pub project: ProjectSummary,
    pub entries: Vec<SidebarEntry>,
}

/// The few facts about an open session that the sidebar needs, so drawing it never clones a
/// whole conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSummary {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub claude_session_id: Option<SessionId>,
    pub title: Option<String>,
    pub first_user_message: Option<String>,
    pub marker: SessionMarker,
    pub can_send: bool,
}

impl LiveSummary {
    pub fn from_view(view: &LiveSessionView) -> Self {
        Self {
            id: view.id,
            project_path: view.project_path.clone(),
            claude_session_id: view.claude_session_id.clone(),
            title: view.session.title.clone(),
            first_user_message: view
                .session
                .messages
                .iter()
                .find(|message| message.role == MessageRole::User)
                .map(|message| truncate_chars(&message.content, MAX_TITLE_CHARS)),
            marker: marker_for(view),
            can_send: view.can_send,
        }
    }

    fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.first_user_message.clone().filter(|text| !text.trim().is_empty()))
            .unwrap_or_else(|| NEW_SESSION.to_string())
    }
}

/// A session needing the user (a permission to answer, or a failure to look at) outranks one that
/// is merely running. Own choice: the real app's markers are not documented (spec §15.1).
pub fn marker_for(view: &LiveSessionView) -> SessionMarker {
    if !view.pending_permissions.is_empty() || view.phase == Phase::Failed {
        SessionMarker::NeedsAttention
    } else if view.phase == Phase::Running {
        SessionMarker::Running
    } else {
        SessionMarker::Idle
    }
}

/// Past sessions grouped by project in the order given (the adapter's newest-first order is kept),
/// with each open session drawn in place of the past entry that has the same Claude session id.
/// Open sessions with no past entry (a brand-new session, or one not listed yet) go to the top of
/// their project's group; a project only they belong to becomes a new group at the top. Sessions
/// are de-duplicated by id and projects by path.
pub fn merge_sidebar(
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    live: &[LiveSummary],
) -> Vec<SidebarGroup> {
    let live_by_claude_id: HashMap<&SessionId, &LiveSummary> = live
        .iter()
        .filter_map(|summary| summary.claude_session_id.as_ref().map(|id| (id, summary)))
        .collect();
    let mut seen: HashSet<SessionId> = HashSet::new();
    let mut groups: Vec<SidebarGroup> = Vec::new();

    for (project, summaries) in past {
        let index = match groups.iter().position(|group| group.project.path == project.path) {
            Some(index) => index,
            None => {
                groups.push(SidebarGroup { project: project.clone(), entries: Vec::new() });
                groups.len() - 1
            }
        };
        for summary in summaries {
            if !seen.insert(summary.id.clone()) {
                continue;
            }
            let entry = match live_by_claude_id.get(&summary.id) {
                Some(open) => SidebarEntry {
                    key: EntryKey::Live(open.id),
                    session_id: Some(summary.id.clone()),
                    title: open
                        .title
                        .clone()
                        .or_else(|| summary.title.clone())
                        .unwrap_or_else(|| UNTITLED.to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    marker: open.marker,
                },
                None => SidebarEntry {
                    key: EntryKey::Past { project: project.path.clone(), session: summary.id.clone() },
                    session_id: Some(summary.id.clone()),
                    title: summary.title.clone().unwrap_or_else(|| UNTITLED.to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    marker: SessionMarker::Idle,
                },
            };
            groups[index].entries.push(entry);
        }
    }

    // Open sessions that no past entry stands for, in the order they were opened.
    let mut extras: Vec<(PathBuf, bool, Vec<SidebarEntry>)> = Vec::new();
    for open in live {
        let listed = open
            .claude_session_id
            .as_ref()
            .is_some_and(|id| seen.contains(id));
        if listed {
            continue;
        }
        let entry = SidebarEntry {
            key: EntryKey::Live(open.id),
            session_id: open.claude_session_id.clone(),
            title: open.display_title(),
            updated_at: None,
            marker: open.marker,
        };
        match extras.iter_mut().find(|(path, _, _)| *path == open.project_path) {
            Some((_, _, entries)) => entries.push(entry),
            None => extras.push((open.project_path.clone(), open.can_send, vec![entry])),
        }
    }

    let mut new_groups: Vec<SidebarGroup> = Vec::new();
    for (path, exists, entries) in extras {
        match groups.iter_mut().find(|group| group.project.path == path) {
            Some(group) => {
                group.entries.splice(0..0, entries);
            }
            None => new_groups.push(SidebarGroup {
                project: ProjectSummary { path, last_active: String::new(), session_count: 0, exists },
                entries,
            }),
        }
    }
    new_groups.extend(groups);
    new_groups
}

/// The open sessions in the order the sidebar draws them; Ctrl+Tab cycles through this.
pub fn open_order(groups: &[SidebarGroup]) -> Vec<LiveSessionId> {
    groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .filter_map(|entry| match entry.key {
            EntryKey::Live(id) => Some(id),
            EntryKey::Past { .. } => None,
        })
        .collect()
}

/// The next (or previous) open session, wrapping around. With no current session, forward starts
/// at the first and backward at the last.
pub fn cycle_target(
    order: &[LiveSessionId],
    current: Option<LiveSessionId>,
    forward: bool,
) -> Option<LiveSessionId> {
    if order.is_empty() {
        return None;
    }
    let position = current.and_then(|id| order.iter().position(|candidate| *candidate == id));
    let index = match (position, forward) {
        (None, true) => 0,
        (None, false) => order.len() - 1,
        (Some(index), true) => (index + 1) % order.len(),
        (Some(index), false) => (index + order.len() - 1) % order.len(),
    };
    Some(order[index])
}

// Counts characters, not bytes, so multi-byte text is never cut inside a character.
fn truncate_chars(text: &str, max: usize) -> String {
    let flattened = text.replace('\n', " ");
    if flattened.chars().count() <= max {
        return flattened;
    }
    let mut truncated: String = flattened.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}
```

In `shortcut.rs` replace everything **above** `#[cfg(test)] mod tests` (the stub) with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shortcut {
    NewSession,
    NextSession,
    PreviousSession,
    CloseSession,
}

/// Claude Code desktop's session shortcuts: Cmd+N new session, Cmd+W close the session,
/// Ctrl+Tab / Ctrl+Shift+Tab cycle. "Cmd" is Ctrl on platforms other than macOS. `key` is the
/// pressed key: `"Tab"`, or a character in any case.
pub fn shortcut_for(key: &str, ctrl: bool, meta: bool, shift: bool, mac: bool) -> Option<Shortcut> {
    if key == "Tab" && ctrl {
        return Some(if shift { Shortcut::PreviousSession } else { Shortcut::NextSession });
    }
    let command = if mac { meta && !ctrl } else { ctrl && !meta };
    if !command || shift {
        return None;
    }
    match key.to_lowercase().as_str() {
        "n" => Some(Shortcut::NewSession),
        "w" => Some(Shortcut::CloseSession),
        _ => None,
    }
}
```

- [ ] **Step 8: Run and see everything pass**

Run: `cargo test -p forge-features` — Expected: `25 passed`, no warnings.
Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'` — Expected: `240`.

- [ ] **Step 9: Check that the tests bite**

Break `sidebar.rs` or `shortcut.rs` in each way below, run `cargo test -p forge-features`, confirm the named test fails, then undo.

| Break this | This test must fail |
|---|---|
| in `merge_sidebar`, replace `if !seen.insert(summary.id.clone()) { continue; }` with `seen.insert(summary.id.clone());` | `a_session_listed_under_two_projects_appears_once` |
| replace `group.entries.splice(0..0, entries);` with `group.entries.extend(entries);` | `a_brand_new_open_session_goes_to_the_top_of_its_projects_group` |
| replace `match live_by_claude_id.get(&summary.id) {` with `match None::<&&LiveSummary> {` | `an_open_session_replaces_the_past_entry_with_the_same_claude_id` and `open_order_follows_the_sidebar_and_skips_past_entries` |
| in `cycle_target`, make the backward arm `(index + 1) % order.len()` | `cycling_wraps_in_both_directions` |
| in `marker_for`, drop `\|\| view.phase == Phase::Failed` | `markers_rank_needs_attention_above_running_above_idle` |
| in `truncate_chars`, slice bytes (`flattened[..max - 1].to_string()`) | `a_summary_carries_only_what_the_sidebar_needs` |
| in `shortcut_for`, change `if !command \|\| shift {` to `if !command {` | `plain_keys_and_shifted_commands_do_nothing` |
| in `shortcut_for`, let a Mac accept Ctrl (`meta \|\| ctrl`) | `the_wrong_modifier_for_the_platform_does_nothing` |

- [ ] **Step 10: Commit**

```bash
git add crates/forge-features Cargo.lock
git commit -m "feat: add forge-features with the workspace store, sidebar merge, shortcuts and interactions

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 9: `forge-widgets` and `forge-pages`

**Files:**
- Create: `crates/forge-widgets/{Cargo.toml, src/lib.rs, src/sidebar.rs, src/chat_window.rs, src/new_session_panel.rs}`, `crates/forge-pages/{Cargo.toml, src/lib.rs, src/workspace_page.rs}`

**Interfaces:**
- Consumes: `forge_features::{use_workspace, use_workspace_provider, Active, EntryKey, PermissionActions, SendMessageBox, NewSessionComposer}` and the `Workspace` methods from Task 8; `forge_entities::{MessageBubble, PermissionCard, SidebarItem, relative_time}`; `forge_shared::{Button, ButtonKind}`; `forge_application::{LiveSessionId, Phase}`; `time::OffsetDateTime`.
- Produces: `forge_widgets::{Sidebar {}, ChatWindow { id: LiveSessionId }, NewSessionPanel {}, project_label(&Path) -> String}`; `forge_pages::WorkspacePage {}` (needs a `WorkspaceServices` in the root context, which Task 10 provides).

The widgets compose Task 8's features into the left sidebar and the right main pane; the page is the whole screen and the root that receives the shortcuts. Only `project_label` is pure logic (the folder's own name for a sidebar heading); everything else is compile-checked and exercised in Task 10 and Task 11.

- [ ] **Step 1: Create `forge-widgets` (manifest, exports, and the two components that need no test)**

`crates/forge-widgets/Cargo.toml`:

```toml
[package]
name = "forge-widgets"
version.workspace = true
edition.workspace = true

[dependencies]
forge-application = { path = "../forge-application" }
forge-entities = { path = "../forge-entities" }
forge-features = { path = "../forge-features" }
forge-shared = { path = "../forge-shared" }
dioxus = { version = "=0.7.10", default-features = false, features = ["lib"] }
time = "0.3.55"
```

`crates/forge-widgets/src/lib.rs`:

```rust
mod chat_window;
mod new_session_panel;
mod sidebar;

pub use chat_window::ChatWindow;
pub use new_session_panel::NewSessionPanel;
pub use sidebar::{project_label, Sidebar};
```

`crates/forge-widgets/src/chat_window.rs`:

```rust
use dioxus::prelude::*;
use forge_application::{LiveSessionId, Phase};
use forge_entities::{MessageBubble, PermissionCard};
use forge_features::{use_workspace, PermissionActions, SendMessageBox};

use crate::sidebar::project_label;

/// One open session: its messages, any permission request waiting for an answer, and the message box.
#[component]
pub fn ChatWindow(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! { div { class: "empty-state", "This session is no longer open." } };
    };
    let view = view_signal.read();
    let heading = format!(
        "{} · {}",
        project_label(&view.project_path),
        view.session.title.as_deref().unwrap_or("Session")
    );

    rsx! {
        div { class: "main-pane",
            div { class: "pane-header", "{heading}" }
            div { class: "messages",
                for (index, message) in view.session.messages.iter().enumerate() {
                    MessageBubble { key: "{index}", message: message.clone() }
                }
            }
            if let Some(error) = view.last_error.as_ref() {
                div { class: "banner error", "{error}" }
            }
            if let Some(request) = view.pending_permissions.front() {
                PermissionCard { request: request.clone(),
                    PermissionActions { id, request_id: request.id.clone() }
                }
            }
            if view.can_send {
                SendMessageBox { id, running: view.phase == Phase::Running, can_send: true }
            } else {
                div { class: "banner info",
                    "The project folder no longer exists, so this session can only be viewed."
                }
            }
        }
    }
}
```

`crates/forge-widgets/src/new_session_panel.rs`:

```rust
use dioxus::prelude::*;
use forge_features::NewSessionComposer;

#[component]
pub fn NewSessionPanel() -> Element {
    rsx! {
        div { class: "main-pane",
            div { class: "pane-header", "New session" }
            NewSessionComposer {}
        }
    }
}
```

- [ ] **Step 2: Create `sidebar.rs` with `project_label` stubbed**

`crates/forge-widgets/src/sidebar.rs` is **one file**, made of these four parts in order: the imports, the stubbed `project_label`, the `Sidebar` component, and the tests.

```rust
use std::path::Path;

use dioxus::prelude::*;
use forge_entities::{relative_time, SidebarItem};
use forge_features::{use_workspace, Active, EntryKey};
use forge_shared::{Button, ButtonKind};
use time::OffsetDateTime;
```

```rust
/// The folder's own name for a sidebar heading; the full path stays available as a tooltip. A
/// path with no final component (the filesystem root) is shown whole.
pub fn project_label(path: &Path) -> String {
    let _ = path;
    unimplemented!("implemented in Task 9, Step 5")
}
```

```rust
/// Sessions of every project, grouped by project, with a "+ New session" button on top.
#[component]
pub fn Sidebar() -> Element {
    let workspace = use_workspace();
    let groups = workspace.sidebar();
    let active = workspace.active();
    let notice = workspace.notice();
    let now = OffsetDateTime::now_utc();

    rsx! {
        div { class: "sidebar",
            div { class: "sidebar-new",
                Button {
                    label: "+ New session",
                    kind: ButtonKind::Primary,
                    onclick: move |_| workspace.show_new_session(),
                }
            }
            if let Some(message) = notice {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| workspace.dismiss_notice() }
                }
            }
            for group in groups {
                div { key: "{group.project.path.display()}",
                    div {
                        class: if group.project.exists { "sidebar-group-title" } else { "sidebar-group-title missing" },
                        title: "{group.project.path.display()}",
                        "{project_label(&group.project.path)}"
                    }
                    for entry in group.entries.iter() {
                        SidebarItem {
                            key: "{entry.title}-{entry.session_id:?}-{entry.updated_at:?}",
                            title: entry.title.clone(),
                            subtitle: entry
                                .updated_at
                                .as_deref()
                                .map(|updated_at| relative_time(updated_at, now))
                                .unwrap_or_default(),
                            marker: entry.marker,
                            active: match &entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            onclick: {
                                let project = group.project.clone();
                                let key = entry.key.clone();
                                move |_| match &key {
                                    EntryKey::Live(id) => workspace.select(*id),
                                    EntryKey::Past { session, .. } => {
                                        workspace.open_past(project.clone(), session.clone())
                                    }
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
```

```rust

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_label_is_the_folders_own_name() {
        assert_eq!(project_label(Path::new("/Users/me/Developer/forge")), "forge");
        assert_eq!(project_label(Path::new("/Users/me/my project")), "my project");
        assert_eq!(project_label(Path::new("/work/한글 폴더")), "한글 폴더");
    }

    #[test]
    fn a_trailing_slash_does_not_hide_the_name() {
        assert_eq!(project_label(Path::new("/work/forge/")), "forge");
    }

    #[test]
    fn the_root_is_shown_whole() {
        assert_eq!(project_label(Path::new("/")), "/");
    }
}
```

- [ ] **Step 3: Create `forge-pages`**

`crates/forge-pages/Cargo.toml`:

```toml
[package]
name = "forge-pages"
version.workspace = true
edition.workspace = true

[dependencies]
forge-features = { path = "../forge-features" }
forge-widgets = { path = "../forge-widgets" }
dioxus = { version = "=0.7.10", default-features = false, features = ["lib"] }
```

`crates/forge-pages/src/lib.rs`:

```rust
mod workspace_page;

pub use workspace_page::WorkspacePage;
```

`crates/forge-pages/src/workspace_page.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let workspace = use_workspace_provider();

    rsx! {
        div {
            class: "workspace",
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                workspace.on_key(&event);
            },
            Sidebar {}
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
        }
    }
}
```

- [ ] **Step 4: Run and see the label tests fail**

Run: `cargo test -p forge-widgets`
Expected: it compiles without warnings; `3 failed` with `not implemented: implemented in Task 9, Step 5`.

- [ ] **Step 5: Implement `project_label`**

In `sidebar.rs` replace the stubbed function

```rust
/// The folder's own name for a sidebar heading; the full path stays available as a tooltip. A
/// path with no final component (the filesystem root) is shown whole.
pub fn project_label(path: &Path) -> String {
    let _ = path;
    unimplemented!("implemented in Task 9, Step 5")
}
```

with

```rust
/// The folder's own name for a sidebar heading; the full path stays available as a tooltip. A
/// path with no final component (the filesystem root) is shown whole.
pub fn project_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}
```

- [ ] **Step 6: Run and see everything pass**

Run: `cargo test -p forge-widgets` — Expected: `3 passed`.
Run: `cargo check -p forge-pages` — Expected: `Finished`, no warnings.
Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'` — Expected: `243`.

- [ ] **Step 7: Commit**

```bash
git add crates/forge-widgets crates/forge-pages Cargo.lock
git commit -m "feat: add forge-widgets and forge-pages: sidebar, chat window and the workspace page

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 10: `forge-app` — composition root and the first real run

**Files:**
- Create: `crates/forge-app/Cargo.toml`, `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: every earlier crate; concrete adapters `ClaudeSessionAdapter::new(claude_home)`, `ClaudeProjectAdapter::new(claude_home)`, `ConfigFileAdapter::new(path)`, `SidecarProcessFactory::new(node, script, config)`; `default_claude_home()`, `default_config_path()`.
- Produces: the `forge-app` binary. Environment variables it reads: `FORGE_NODE` (default `node`) and `FORGE_SIDECAR_SCRIPT` (default `<repo>/sidecar/dist/index.js`, resolved at compile time from the crate's location). It is the **only** place that names concrete adapters, and the only crate enabling `dioxus/desktop`.

- [ ] **Step 1: Create the crate**

`crates/forge-app/Cargo.toml`:

```toml
[package]
name = "forge-app"
version.workspace = true
edition.workspace = true

[dependencies]
forge-adapter-claude-fs = { path = "../forge-adapter-claude-fs" }
forge-adapter-config = { path = "../forge-adapter-config" }
forge-adapter-sidecar = { path = "../forge-adapter-sidecar" }
forge-application = { path = "../forge-application" }
forge-features = { path = "../forge-features" }
forge-pages = { path = "../forge-pages" }
forge-shared = { path = "../forge-shared" }
dioxus = { version = "=0.7.10", features = ["desktop"] }
```

`crates/forge-app/src/main.rs` (one small test checks that the default sidecar path points into this repository's `sidecar/` directory, beside its sources):

```rust
use std::sync::Arc;

use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use forge_adapter_claude_fs::{default_claude_home, ClaudeProjectAdapter, ClaudeSessionAdapter};
use forge_adapter_config::{default_config_path, ConfigFileAdapter};
use forge_adapter_sidecar::SidecarProcessFactory;
use forge_application::SessionRegistry;
use forge_features::WorkspaceServices;
use forge_pages::WorkspacePage;
use forge_shared::THEME_CSS;

// Development default: the sidecar built in this repository (`npm run build` in `sidecar/`).
// Locating a bundled sidecar is packaging's job (spec §9), so `FORGE_SIDECAR_SCRIPT` overrides it.
const DEFAULT_SIDECAR_SCRIPT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../sidecar/dist/index.js");

fn main() {
    dioxus::LaunchBuilder::desktop()
        .with_cfg(window_config())
        .with_context(build_services())
        .launch(App);
}

fn window_config() -> Config {
    Config::new().with_window(
        WindowBuilder::new()
            .with_title("Forge")
            .with_inner_size(LogicalSize::new(1200.0, 800.0)),
    )
}

// The composition root: the only place that names concrete adapters.
fn build_services() -> WorkspaceServices {
    let claude_home = default_claude_home().expect("could not resolve the home directory");
    let config_path = default_config_path().expect("could not resolve the config directory");
    let node = std::env::var("FORGE_NODE").unwrap_or_else(|_| "node".to_string());
    let script = std::env::var("FORGE_SIDECAR_SCRIPT").unwrap_or_else(|_| DEFAULT_SIDECAR_SCRIPT.to_string());

    let config = Arc::new(ConfigFileAdapter::new(config_path));
    let factory = Arc::new(SidecarProcessFactory::new(node, script, config));
    let sessions = Arc::new(ClaudeSessionAdapter::new(claude_home.clone()));
    let projects = Arc::new(ClaudeProjectAdapter::new(claude_home));
    let registry = Arc::new(SessionRegistry::new(factory, sessions.clone()));
    WorkspaceServices { registry, projects, sessions }
}

#[component]
fn App() -> Element {
    rsx! {
        style { "{THEME_CSS}" }
        WorkspacePage {}
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn the_default_sidecar_script_points_into_this_repositorys_sidecar_directory() {
        let script = Path::new(DEFAULT_SIDECAR_SCRIPT);
        assert!(script.ends_with("sidecar/dist/index.js"), "{DEFAULT_SIDECAR_SCRIPT}");
        assert!(
            script.parent().unwrap().parent().unwrap().join("src/index.ts").exists(),
            "the sidecar sources should sit beside dist/: {DEFAULT_SIDECAR_SCRIPT}"
        );
    }
}
```

- [ ] **Step 2: Test and build**

Run: `cargo test -p forge-app` — Expected: `1 passed`.
Run: `cargo build -p forge-app` — Expected: `Finished`, no warnings except the `block v0.1.6` notice.
Run: `cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s}'` — Expected: `244`.

- [ ] **Step 3: First real run** (this is where components that were only compiled meet a window)

```bash
ls sidecar/dist/index.js || (cd sidecar && npm ci && npm run build)
dx serve -p forge-app --platform desktop      # or the command Task 1 Step 5 found to work
```

Expected: a 1200×800 window titled "Forge"; on the left "+ New session" and one group per project with sessions (folders that no longer exist are struck through); on the right the new-session panel with a **Folder** dropdown that lists only folders that still exist, a Model dropdown with a single "Default model", a Mode dropdown, and a message box. Clicking a past session opens its history read-only. No credentials are needed to reach this point.

If the sidebar is empty, look for a red banner at the top of the sidebar (a failed listing) before suspecting the UI. If the window is blank, run the binary from a terminal and read its output. Anything that fails here that the compile step could not catch is expected to be small: fix it, add a test if the fix is in logic, commit it as `fix: …`, and record it in Task 11's notes. If a failure contradicts the design, stop and write an amendment instead.

Credentials, when Task 11 first sends a message: with no Forge config file the sidecar inherits this process's environment (so `ANTHROPIC_API_KEY`, or Claude Code's own login, is used). To use Forge's own settings, create `~/Library/Application Support/Forge/config.json` (macOS; the location comes from `directories::ProjectDirs::from("", "", "Forge")`) with `{ "base_url": "https://api.anthropic.com", "api_key": "…" }` and `chmod 600` it. Never paste a key into a commit, a log or a chat.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-app Cargo.lock
git commit -m "feat: add forge-app, the composition root that wires the adapters into the workspace UI

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 11: Manual verification, spec finalisation and final review

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (`§15.11 As built`), and any code a checklist item shows to be wrong (each fix in its own commit)

**Interfaces:** none. This task changes no public interface unless a fix demands it.

Dioxus has no mature automated desktop UI testing (spec §8), so the app is verified by hand, and this checklist is the record. It uses the real `~/.claude`; use a scratch project folder (for example `mkdir ~/forge-scratch`) for anything that makes Claude edit files.

- [ ] **Step 1: Run the checklist** (`dx serve -p forge-app --platform desktop`; use a second terminal for `ps`). Tick each item only when the *Expect* column happened.

| # | Do | Expect |
|---|---|---|
| 1 | Launch. | Sidebar groups match the projects in `~/.claude/projects` (12 groups on the reference machine); newest project first; deleted folders struck through. |
| 2 | Click a past session in an existing folder. | Its history shows read-only. `ps aux \| grep sidecar/dist` shows **no** sidecar process. |
| 3 | Type a message in that session and send. | A single `node …/sidecar/dist/index.js` process appears; the reply streams in; the same transcript file under `~/.claude/projects/…` grows (open it or run `claude --resume` in that folder to confirm it is the same session). |
| 4 | ⌘N. Pick the scratch folder and a mode, type a message, Enter. | The main pane switches to the new session; it appears at the top of its project's group titled by your first message; `●` while running, `○` after the turn. |
| 5 | In `Ask before edits` mode ask Claude to create a file. | A permission card appears with Allow / Always allow / Deny; try each on separate requests; the sidebar shows the attention marker while a request waits (switch to another session to see it). |
| 6 | Ask for a very long answer, then press Stop. | The turn ends, Stop disappears, the box is usable again. (If Task 1 (g) found no `TurnComplete` after an interrupt and the amendment was applied, this is where it shows.) |
| 7 | Start a long answer in session A, switch to session B (another session) and start one there. | Both stream; switching back to A shows its text complete. Two `node` processes exist. |
| 8 | Click the same past session in the sidebar twice, and click a session that is already open. | It focuses; no second process for the same session. |
| 9 | ⌘W on a running session. | Its `node` process disappears; the other session keeps streaming; the closed session is still listed (as a past session) and can be reopened. |
| 10 | With three sessions open press Ctrl+Tab and Ctrl+Shift+Tab. | Focus cycles through the open sessions in sidebar order and wraps. |
| 11 | Open a session whose folder is gone (a struck-through project). | History is readable, a banner says the folder no longer exists, the message box is not offered, and that folder is absent from the new-session Folder dropdown. |
| 12 | Switch to a Korean input source, type a syllable and press Enter to commit it, then press Enter again. | The first Enter does **not** send; the second does. |
| 13 | While a session runs, `kill` its `node` process from a terminal. | An error banner appears and the marker shows attention; the next message respawns a sidecar and resumes the same session (same transcript file continues). |
| 14 | Put invalid JSON in `~/Library/Application Support/Forge/config.json`, then send a message in a session that has no sidecar. | A banner says `failed to load the Forge config`; fixing the file makes the next send work. Delete the file afterwards if you created it. |
| 15 | Ask Claude to print a large file (several hundred KB). | Typing in the message box stays responsive. If it does not, that is the known "clone the whole conversation on every render" limitation: note the size at which it degrades. |

- [ ] **Step 2: Fix what the checklist found**

For each failed item: write a failing test first if the bug is in logic (Tasks 3–6 and 8 have the harnesses), fix it, and commit it as its own `fix: …` commit. If the fix changes the design, write an `## Amendment` in this plan first. Re-run the affected checklist item.

- [ ] **Step 3: Record what was built**

Append `### 15.11 As built` to `docs/superpowers/specs/2026-09-27-forge-design.md`: the final test count, the checklist result, every runtime fix from Steps 1–2 and Task 10, and every place the built system differs from §15. The differences already known when this plan was written are recorded in §15.4–§15.9; add only new ones.

- [ ] **Step 4: Final verification**

```bash
cargo test --workspace 2>&1 | grep -E '^test result' | awk '{s+=$4} END {print s " tests passed"}'
cargo build --workspace 2>&1 | grep -E '^(warning|error)'
( cd sidecar && npm test 2>&1 | tail -4 )
git status -sb
```

Expected: at least `244 tests passed`; the only build warning is the `block v0.1.6` notice; `Tests  18 passed (18)` for the sidecar; a clean tree on `plan-5`.

- [ ] **Step 5: Whole-branch review, then integrate**

Invoke `superpowers:requesting-code-review` on the whole `plan-5` branch (base: `master`). Fix Critical and Important findings; list the rest as deferred in a commit message or the spec. Then invoke `superpowers:finishing-a-development-branch` (Plan 4 was integrated by fast-forward merge to `master`; the user decides again here).

- [ ] **Step 6: Commit the spec**

```bash
git add docs/superpowers
git commit -m "docs: record Plan 5 as built

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

