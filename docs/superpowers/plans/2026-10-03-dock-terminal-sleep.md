# Plan 21A: the right-hand dock, the terminal and prevent sleep — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A column dock to the right of the chat, a terminal panel in it (real shell in the session folder, tabs, expand, close) and a per-session "Prevent sleep" switch.

**Architecture:** Pure dock state and terminal-tab state in `forge-features` (unit tested). Two new ports, `TerminalPort` and `SleepPort`, with adapters `forge-adapter-terminal` (`portable-pty`) and `forge-adapter-sleep` (`caffeinate`). The screen is a vendored `xterm.js` injected into the window's `<head>`; Rust and xterm talk through Dioxus's `document::eval` bridge. Widgets: `Dock`, `PanelFrame`, `TerminalPanel`, `SessionTools` (toolbar button and `⋮` menu).

**Tech Stack:** Rust, Dioxus 0.7.10 desktop, `portable-pty`, `tokio` (sync, macros), `base64`, xterm.js 5.5.0 + addon-fit 0.10.0 (MIT, vendored).

**Spec:** `docs/superpowers/specs/2026-10-03-right-dock-panels-design.md` (sections "The dock" and "Terminal and prevent sleep"). Plan 21B (Changes, Files, Preview) comes after this plan is built.

## Global Constraints

- Layers as in the repo: pure logic in `forge-features`, I/O only in adapters, the composition root (`forge-app/src/main.rs`) is the only place that names concrete adapters. `forge-widgets` never names an adapter.
- UI text is English. Panel and menu strings: "Terminal", "New terminal", "Close tab", "Expand", "Collapse", "Close", "Prevent sleep", "This session only", "Restart".
- Missing folder message, exactly: `Working directory "<path>" does not exist`.
- Default column width 480 px, clamped to 280–900 px.
- The shell is the user's `$SHELL` (fallback `/bin/zsh`), started with `-l`, in the session folder, `TERM=xterm-256color`.
- Prevent sleep runs `caffeinate -i -w <forge pid>`: off by default, per session, not saved, ends when Forge exits.
- No network at run time: xterm.js and its CSS are inlined in the window head.
- Tests first for pure logic and adapters; widgets are checked in the real app (Task 11). Every task ends with `cargo test` for the crates it touched passing and a commit with the trailer `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Base branch is `master`; work stays on branch `plan-21`.

## Review Focus

1. A session whose folder is gone: the terminal shows `Working directory "…" does not exist` in the panel and starts no process (Task 4 test, Task 9).
2. The shell exits (`exit 3`): the tab shows "Process exited (3)" with a Restart button, further keystrokes are ignored and nothing panics (Task 4 test, Task 9).
3. A tab hidden with `display: none` has a 0 × 0 host: the bridge never reports a size of zero, and `resize` on a closed terminal returns an error instead of panicking (Task 4 test, Task 9).
4. Korean and other multi-byte text: output bytes reach xterm raw (base64) so a character split across two reads is not corrupted; typed text is sent as UTF-8 (Task 4 test).
5. Closing a tab, the panel or the session, or switching to another session, kills the shell and the `caffeinate` child; no process is left behind (Task 4 and 5 tests, Task 6).

---

### Task 1: Dock state

**Files:**
- Create: `crates/forge-features/src/dock.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Modify: `docs/superpowers/specs/2026-10-03-right-dock-panels-design.md` (one sentence)

**Interfaces:**
- Produces: `Panel` (`Terminal`; `title()`), `Column { panels: Vec<Panel>, width: f32 }`, `DockState { columns, expanded }` with `is_open`, `toggle`, `close`, `expand`, `collapse`, `resize(column, width)`, and `DockStates` with `get(id) -> DockState`, `change(id, f)`, `forget(id)`; constants `DEFAULT_COLUMN_WIDTH`, `MIN_COLUMN_WIDTH`, `MAX_COLUMN_WIDTH`. Plan 21B adds the `Changes`, `Preview` and `Files` variants and the rule that Changes and Preview share a column.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/dock.rs` with only the tests first (the types are added in Step 3; the file will not compile yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_a_closed_panel_opens_it_in_a_new_column_at_the_default_width() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        assert!(dock.is_open(Panel::Terminal));
        assert_eq!(dock.columns, vec![Column { panels: vec![Panel::Terminal], width: DEFAULT_COLUMN_WIDTH }]);
    }

    #[test]
    fn toggling_an_open_panel_closes_it_and_its_column() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.toggle(Panel::Terminal);
        assert!(!dock.is_open(Panel::Terminal));
        assert!(dock.columns.is_empty());
    }

    #[test]
    fn expanding_a_closed_panel_does_nothing() {
        let mut dock = DockState::default();
        dock.expand(Panel::Terminal);
        assert_eq!(dock.expanded, None);
    }

    #[test]
    fn an_open_panel_expands_and_collapses() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.expand(Panel::Terminal);
        assert_eq!(dock.expanded, Some(Panel::Terminal));
        dock.collapse();
        assert_eq!(dock.expanded, None);
    }

    #[test]
    fn closing_the_expanded_panel_collapses_first() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.expand(Panel::Terminal);
        dock.close(Panel::Terminal);
        assert_eq!(dock, DockState::default());
    }

    #[test]
    fn resize_clamps_to_the_minimum_and_maximum() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize(0, 10.0);
        assert_eq!(dock.columns[0].width, MIN_COLUMN_WIDTH);
        dock.resize(0, 5000.0);
        assert_eq!(dock.columns[0].width, MAX_COLUMN_WIDTH);
        dock.resize(0, 500.0);
        assert_eq!(dock.columns[0].width, 500.0);
    }

    #[test]
    fn resize_ignores_a_missing_column_and_a_width_that_is_not_a_number() {
        let mut dock = DockState::default();
        dock.toggle(Panel::Terminal);
        dock.resize(3, 500.0);
        dock.resize(0, f32::NAN);
        assert_eq!(dock.columns[0].width, DEFAULT_COLUMN_WIDTH);
    }

    #[test]
    fn each_session_has_its_own_dock_and_forgets_it_on_close() {
        let mut docks = DockStates::default();
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        assert!(docks.get(LiveSessionId(1)).is_open(Panel::Terminal));
        assert!(!docks.get(LiveSessionId(2)).is_open(Panel::Terminal));
        docks.forget(LiveSessionId(1));
        assert_eq!(docks.get(LiveSessionId(1)), DockState::default());
    }

    #[test]
    fn a_dock_that_is_back_to_empty_is_not_kept() {
        let mut docks = DockStates::default();
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        docks.change(LiveSessionId(1), |dock| dock.toggle(Panel::Terminal));
        assert!(docks.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features dock::`
Expected: compile error (`Panel`, `DockState` not found). First add `mod dock;` to `crates/forge-features/src/lib.rs` (after `mod composer_state;`) so the file is built.

- [ ] **Step 3: Write the implementation**

Put this above the `#[cfg(test)]` block in `dock.rs`:

```rust
use std::collections::HashMap;

use forge_application::LiveSessionId;

pub const DEFAULT_COLUMN_WIDTH: f32 = 480.0;
pub const MIN_COLUMN_WIDTH: f32 = 280.0;
pub const MAX_COLUMN_WIDTH: f32 = 900.0;

/// A panel that can sit in the dock. Plan 21B adds Changes, Preview and Files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Panel {
    Terminal,
}

impl Panel {
    pub fn title(self) -> &'static str {
        match self {
            Panel::Terminal => "Terminal",
        }
    }
}

/// One column of the dock: the panels stacked in it, top to bottom, and its width in pixels.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
    pub panels: Vec<Panel>,
    pub width: f32,
}

/// The dock of one session: columns right of the chat, left to right, and the panel that fills
/// the whole area right of the sidebar, if any.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockState {
    pub columns: Vec<Column>,
    pub expanded: Option<Panel>,
}

impl DockState {
    pub fn is_open(&self, panel: Panel) -> bool {
        self.columns.iter().any(|column| column.panels.contains(&panel))
    }

    /// Opens a closed panel in a new column at the far right, or closes an open one. Opening a
    /// panel never closes another.
    pub fn toggle(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.close(panel);
        } else {
            self.columns.push(Column { panels: vec![panel], width: DEFAULT_COLUMN_WIDTH });
        }
    }

    pub fn close(&mut self, panel: Panel) {
        for column in &mut self.columns {
            column.panels.retain(|open| *open != panel);
        }
        self.columns.retain(|column| !column.panels.is_empty());
        if self.expanded == Some(panel) {
            self.expanded = None;
        }
    }

    /// Makes an open panel fill the area right of the sidebar. A closed panel is ignored.
    pub fn expand(&mut self, panel: Panel) {
        if self.is_open(panel) {
            self.expanded = Some(panel);
        }
    }

    pub fn collapse(&mut self) {
        self.expanded = None;
    }

    /// Sets a column's width, kept between the minimum and the maximum. A column that does not
    /// exist and a width that is not a number are ignored.
    pub fn resize(&mut self, column: usize, width: f32) {
        if !width.is_finite() {
            return;
        }
        if let Some(column) = self.columns.get_mut(column) {
            column.width = width.clamp(MIN_COLUMN_WIDTH, MAX_COLUMN_WIDTH);
        }
    }
}

/// The dock of each open session. It lives in memory only (spec: panel state is not saved).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DockStates {
    states: HashMap<LiveSessionId, DockState>,
}

impl DockStates {
    pub fn get(&self, id: LiveSessionId) -> DockState {
        self.states.get(&id).cloned().unwrap_or_default()
    }

    pub fn change(&mut self, id: LiveSessionId, change: impl FnOnce(&mut DockState)) {
        let state = self.states.entry(id).or_default();
        change(state);
        if *state == DockState::default() {
            self.states.remove(&id);
        }
    }

    /// Drops what is kept for a session that was closed.
    pub fn forget(&mut self, id: LiveSessionId) {
        self.states.remove(&id);
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}
```

In `lib.rs` add the export line (keep the list's style):

```rust
pub use dock::{Column, DockState, DockStates, Panel, DEFAULT_COLUMN_WIDTH, MAX_COLUMN_WIDTH, MIN_COLUMN_WIDTH};
```

In the spec, replace the sentence "`resize(column, width)` clamps to a minimum and leaves the chat at least a minimum width." with "`resize(column, width)` clamps the width to 280–900 px; the chat column keeps a CSS `min-width`."

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features dock::`
Expected: 9 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features docs/superpowers/specs/2026-10-03-right-dock-panels-design.md
git commit -m "feat: dock state, one per session" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Terminal tab model

**Files:**
- Create: `crates/forge-features/src/terminal_tabs.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces: `TabId(pub u64)`; `TerminalTabs` with `open() -> TabId`, `close(TabId)`, `select(TabId)`, `tabs() -> &[TabId]`, `active() -> Option<TabId>`, `is_empty()`. Ids are never reused within one `TerminalTabs`.

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_tab_becomes_the_active_one() {
        let mut tabs = TerminalTabs::default();
        let first = tabs.open();
        let second = tabs.open();
        assert_eq!(tabs.tabs(), &[first, second]);
        assert_eq!(tabs.active(), Some(second));
    }

    #[test]
    fn ids_are_not_reused_after_a_close() {
        let mut tabs = TerminalTabs::default();
        let first = tabs.open();
        tabs.close(first);
        let second = tabs.open();
        assert_ne!(first, second);
    }

    #[test]
    fn closing_the_active_tab_activates_its_right_neighbour_else_the_left() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        let b = tabs.open();
        let c = tabs.open();
        tabs.select(b);
        tabs.close(b);
        assert_eq!(tabs.active(), Some(c));
        tabs.close(c);
        assert_eq!(tabs.active(), Some(a));
        tabs.close(a);
        assert_eq!(tabs.active(), None);
        assert!(tabs.is_empty());
    }

    #[test]
    fn closing_a_tab_that_is_not_active_keeps_the_active_one() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        let b = tabs.open();
        tabs.close(a);
        assert_eq!(tabs.active(), Some(b));
    }

    #[test]
    fn selecting_or_closing_a_tab_that_does_not_exist_is_ignored() {
        let mut tabs = TerminalTabs::default();
        let a = tabs.open();
        tabs.select(TabId(99));
        tabs.close(TabId(99));
        assert_eq!(tabs.tabs(), &[a]);
        assert_eq!(tabs.active(), Some(a));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add `mod terminal_tabs;` to `lib.rs` (after `mod shortcut;`), then run `cargo test -p forge-features terminal_tabs::`
Expected: compile error (`TerminalTabs` not found).

- [ ] **Step 3: Write the implementation** (above the tests)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

/// The tabs of one terminal panel. The panel owns it for as long as it is open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TerminalTabs {
    tabs: Vec<TabId>,
    active: Option<TabId>,
    next: u64,
}

impl TerminalTabs {
    /// Adds a tab at the right and makes it the active one.
    pub fn open(&mut self) -> TabId {
        let id = TabId(self.next);
        self.next += 1;
        self.tabs.push(id);
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: TabId) {
        let Some(position) = self.tabs.iter().position(|tab| *tab == id) else {
            return;
        };
        self.tabs.remove(position);
        if self.active == Some(id) {
            self.active = self.tabs.get(position).or_else(|| self.tabs.last()).copied();
        }
    }

    pub fn select(&mut self, id: TabId) {
        if self.tabs.contains(&id) {
            self.active = Some(id);
        }
    }

    pub fn tabs(&self) -> &[TabId] {
        &self.tabs
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }
}
```

Export in `lib.rs`: `pub use terminal_tabs::{TabId, TerminalTabs};`

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features terminal_tabs::` → 5 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: terminal tab model" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The two ports

**Files:**
- Create: `crates/forge-ports/src/terminal_port.rs`, `crates/forge-ports/src/sleep_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`, `crates/forge-ports/Cargo.toml`

**Interfaces:**
- Produces:
  - `TerminalId(pub u64)`, `TerminalSize { cols: u16, rows: u16 }`, `TerminalEvent { Output(Vec<u8>), Exited(Option<u32>) }`, `TerminalError { MissingDirectory(PathBuf), Failed(String) }` (Display of `MissingDirectory` is exactly `Working directory "<path>" does not exist`), `TerminalSession { id: TerminalId, events: UnboundedReceiver<TerminalEvent> }`.
  - `trait TerminalPort: Send + Sync { fn open(&self, cwd: &Path, size: TerminalSize) -> Result<TerminalSession, TerminalError>; fn write(&self, id: TerminalId, data: &[u8]) -> Result<(), TerminalError>; fn resize(&self, id: TerminalId, size: TerminalSize) -> Result<(), TerminalError>; fn close(&self, id: TerminalId); }`
  - `trait SleepPort: Send + Sync { fn hold(&self, key: u64) -> Result<(), String>; fn release(&self, key: u64); }`
  - `MockTerminalPort`, `MockSleepPort` under `any(test, feature = "mocks")`.

- [ ] **Step 1: Write the failing tests**

`terminal_port.rs` ends with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_reads_as_the_message_the_panel_shows() {
        let error = TerminalError::MissingDirectory(PathBuf::from("/work/gone"));
        assert_eq!(error.to_string(), "Working directory \"/work/gone\" does not exist");
    }

    #[test]
    fn the_mock_opens_a_session_whose_events_can_be_read() {
        let mut mock = MockTerminalPort::new();
        mock.expect_open().returning(|_, _| {
            let (sender, events) = tokio::sync::mpsc::unbounded_channel();
            sender.send(TerminalEvent::Output(b"hi".to_vec())).unwrap();
            Ok(TerminalSession { id: TerminalId(7), events })
        });
        let mut session = mock.open(Path::new("/work/a"), TerminalSize { cols: 80, rows: 24 }).unwrap();
        assert_eq!(session.id, TerminalId(7));
        assert_eq!(session.events.try_recv().unwrap(), TerminalEvent::Output(b"hi".to_vec()));
    }
}
```

`sleep_port.rs` ends with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_refuse_to_hold() {
        let mut mock = MockSleepPort::new();
        mock.expect_hold().returning(|_| Err("no caffeinate".to_string()));
        assert_eq!(mock.hold(1), Err("no caffeinate".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Add to `crates/forge-ports/Cargo.toml` under `[dependencies]`: `tokio = { version = "1", features = ["sync"] }`. Add `mod sleep_port;` and `mod terminal_port;` to `lib.rs`. Run `cargo test -p forge-ports terminal_port:: sleep_port::` → compile error.

- [ ] **Step 3: Write the implementation**

`terminal_port.rs`, above the tests:

```rust
use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::sync::mpsc::UnboundedReceiver;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Raw bytes from the shell. A multi-byte character can be split between two events.
    Output(Vec<u8>),
    /// The shell ended; the exit code when it is known. Always the last event.
    Exited(Option<u32>),
}

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Working directory \"{}\" does not exist", .0.display())]
    MissingDirectory(PathBuf),
    #[error("{0}")]
    Failed(String),
}

/// A shell that was started: its id and the stream of what it printed.
pub struct TerminalSession {
    pub id: TerminalId,
    pub events: UnboundedReceiver<TerminalEvent>,
}

/// Runs shells in a pseudo-terminal. Calls return at once (a write to a pty does not wait).
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait TerminalPort: Send + Sync {
    fn open(&self, cwd: &Path, size: TerminalSize) -> Result<TerminalSession, TerminalError>;
    fn write(&self, id: TerminalId, data: &[u8]) -> Result<(), TerminalError>;
    fn resize(&self, id: TerminalId, size: TerminalSize) -> Result<(), TerminalError>;
    /// Ends the shell. A terminal that is already closed is ignored.
    fn close(&self, id: TerminalId);
}
```

`sleep_port.rs`, above the tests:

```rust
#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Keeps the computer awake while something holds it. Each holder has its own key, so two
/// sessions can hold at once and each releases only its own.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait SleepPort: Send + Sync {
    /// Starts holding for `key`; holding twice for the same key is one hold. The error is a
    /// message for the person.
    fn hold(&self, key: u64) -> Result<(), String>;
    fn release(&self, key: u64);
}
```

In `lib.rs` add: `pub use sleep_port::SleepPort;`, `pub use terminal_port::{TerminalError, TerminalEvent, TerminalId, TerminalPort, TerminalSession, TerminalSize};` and, in the `#[cfg(any(test, feature = "mocks"))]` block, `pub use sleep_port::MockSleepPort;` and `pub use terminal_port::MockTerminalPort;`.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-ports` → all pass (the two new test modules included).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: TerminalPort and SleepPort" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The terminal adapter

**Files:**
- Create: `crates/forge-adapter-terminal/Cargo.toml`, `crates/forge-adapter-terminal/src/lib.rs`

**Interfaces:**
- Consumes: `TerminalPort`, `TerminalId`, `TerminalSize`, `TerminalEvent`, `TerminalError`, `TerminalSession` from Task 3.
- Produces: `PtyTerminal::new(shell: impl Into<String>) -> PtyTerminal` implementing `TerminalPort`.

- [ ] **Step 1: Create the crate and write the failing tests**

`crates/forge-adapter-terminal/Cargo.toml`:

```toml
[package]
name = "forge-adapter-terminal"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }
tokio = { version = "1", features = ["sync"] }

[dev-dependencies]
tokio = { version = "1", features = ["sync", "macros", "rt", "time"] }
tempfile = "3"
```

Then run `cargo add -p forge-adapter-terminal portable-pty` (it picks the current release; the code below uses its `native_pty_system`, `CommandBuilder`, `PtySize`, `MasterPty`, `ChildKiller` and `Child::wait`, `ExitStatus::exit_code`).

`src/lib.rs` starts with the tests (the adapter is added in Step 3):

```rust
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;

    use super::*;

    const SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };

    fn terminal() -> PtyTerminal {
        PtyTerminal::new("/bin/sh")
    }

    async fn next(events: &mut UnboundedReceiver<TerminalEvent>) -> TerminalEvent {
        tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("no event within 10 s")
            .expect("the event stream ended")
    }

    // Reads output until `needle` has been printed.
    async fn read_until(events: &mut UnboundedReceiver<TerminalEvent>, needle: &str) -> String {
        let mut seen = Vec::new();
        loop {
            if let TerminalEvent::Output(bytes) = next(events).await {
                seen.extend(bytes);
                let text = String::from_utf8_lossy(&seen).into_owned();
                if text.contains(needle) {
                    return text;
                }
            }
        }
    }

    #[tokio::test]
    async fn a_command_typed_into_the_shell_runs_and_its_output_arrives() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        // The typed line is echoed with `$((1+2))` in it; only a shell that ran it prints `forge-3`.
        terminal.write(session.id, b"echo forge-$((1+2))\n").unwrap();
        read_until(&mut session.events, "forge-3").await;
        terminal.close(session.id);
    }

    #[tokio::test]
    async fn the_shell_starts_in_the_folder_it_was_given() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_string_lossy().into_owned();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        // The typed line holds `$(pwd)`, not the folder's name: only a shell that ran it prints the name.
        terminal.write(session.id, b"echo here=$(pwd)\n").unwrap();
        read_until(&mut session.events, &name).await;
        terminal.close(session.id);
    }

    #[tokio::test]
    async fn korean_text_goes_through_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.write(session.id, "echo 한글\n".as_bytes()).unwrap();
        read_until(&mut session.events, "한글").await;
        terminal.close(session.id);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused_with_its_path() {
        let terminal = terminal();
        let error = terminal.open(std::path::Path::new("/definitely/not/here"), SIZE).err().expect("should fail");
        assert!(matches!(error, TerminalError::MissingDirectory(ref path) if path == std::path::Path::new("/definitely/not/here")));
    }

    #[tokio::test]
    async fn the_exit_code_of_a_shell_that_ends_is_reported_last() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.write(session.id, b"exit 3\n").unwrap();
        loop {
            if let TerminalEvent::Exited(code) = next(&mut session.events).await {
                assert_eq!(code, Some(3));
                break;
            }
        }
    }

    #[tokio::test]
    async fn closing_a_terminal_ends_its_shell() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.close(session.id);
        loop {
            if let TerminalEvent::Exited(_) = next(&mut session.events).await {
                break;
            }
        }
    }

    #[test]
    fn writing_or_resizing_a_closed_terminal_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.close(session.id);
        assert!(terminal.write(session.id, b"x").is_err());
        assert!(terminal.resize(session.id, SIZE).is_err());
        terminal.close(session.id); // closing twice is ignored
    }

    #[test]
    fn a_resize_reaches_the_shell() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let session = terminal.open(dir.path(), SIZE).unwrap();
        assert!(terminal.resize(session.id, TerminalSize { cols: 120, rows: 40 }).is_ok());
        terminal.close(session.id);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-adapter-terminal` → compile error (`PtyTerminal` not found).

- [ ] **Step 3: Write the implementation** (above the tests)

```rust
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use forge_ports::{TerminalError, TerminalEvent, TerminalId, TerminalPort, TerminalSession, TerminalSize};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc::unbounded_channel;

struct Live {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// Shells in a pseudo-terminal, through `portable-pty`. Each shell has a thread that copies what
/// it prints into the session's event stream and reports its exit.
pub struct PtyTerminal {
    shell: String,
    next: AtomicU64,
    live: Mutex<HashMap<TerminalId, Live>>,
}

impl PtyTerminal {
    pub fn new(shell: impl Into<String>) -> Self {
        Self { shell: shell.into(), next: AtomicU64::new(1), live: Mutex::new(HashMap::new()) }
    }
}

fn failed(error: impl std::fmt::Display) -> TerminalError {
    TerminalError::Failed(error.to_string())
}

fn pty_size(size: TerminalSize) -> PtySize {
    PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 }
}

impl TerminalPort for PtyTerminal {
    fn open(&self, cwd: &Path, size: TerminalSize) -> Result<TerminalSession, TerminalError> {
        if !cwd.is_dir() {
            return Err(TerminalError::MissingDirectory(cwd.to_path_buf()));
        }
        let pair = native_pty_system().openpty(pty_size(size)).map_err(failed)?;
        // A login shell, so the person's PATH from their profile is there: an app started from
        // the Finder has only a minimal one.
        let mut command = CommandBuilder::new(&self.shell);
        command.arg("-l");
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        let mut child = pair.slave.spawn_command(command).map_err(failed)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(failed)?;
        let writer = pair.master.take_writer().map_err(failed)?;
        let killer = child.clone_killer();

        let (sender, events) = unbounded_channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if sender.send(TerminalEvent::Output(buffer[..read].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            let code = child.wait().ok().map(|status| status.exit_code());
            let _ = sender.send(TerminalEvent::Exited(code));
        });

        let id = TerminalId(self.next.fetch_add(1, Ordering::Relaxed));
        self.live.lock().unwrap().insert(id, Live { master: pair.master, writer, killer });
        Ok(TerminalSession { id, events })
    }

    fn write(&self, id: TerminalId, data: &[u8]) -> Result<(), TerminalError> {
        let mut live = self.live.lock().unwrap();
        let terminal = live.get_mut(&id).ok_or_else(|| failed("the terminal is closed"))?;
        terminal.writer.write_all(data).map_err(failed)?;
        terminal.writer.flush().map_err(failed)
    }

    fn resize(&self, id: TerminalId, size: TerminalSize) -> Result<(), TerminalError> {
        let live = self.live.lock().unwrap();
        let terminal = live.get(&id).ok_or_else(|| failed("the terminal is closed"))?;
        terminal.master.resize(pty_size(size)).map_err(failed)
    }

    fn close(&self, id: TerminalId) {
        let removed = self.live.lock().unwrap().remove(&id);
        if let Some(mut terminal) = removed {
            let _ = terminal.killer.kill();
        }
    }
}

impl Drop for PtyTerminal {
    fn drop(&mut self) {
        if let Ok(live) = self.live.get_mut() {
            for (_, mut terminal) in live.drain() {
                let _ = terminal.killer.kill();
            }
        }
    }
}
```

Note for the executor: if `portable-pty`'s current release names a type differently (for example `ExitStatus::exit_code` or the `ChildKiller` bounds), follow its docs; the behaviour above is what the tests pin.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-adapter-terminal` → 8 passed. Run it three times in a row: the tests spawn real shells, and a flaky one means a timing bug to fix, not a test to loosen.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-terminal Cargo.lock
git commit -m "feat: terminal adapter on portable-pty" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The sleep adapter

**Files:**
- Create: `crates/forge-adapter-sleep/Cargo.toml`, `crates/forge-adapter-sleep/src/lib.rs`

**Interfaces:**
- Consumes: `SleepPort` from Task 3.
- Produces: `CaffeinateSleep::new()` (runs `caffeinate -i -w <forge pid>`), `CaffeinateSleep::with_command(program, args)` (for tests), both implementing `SleepPort`.

- [ ] **Step 1: Create the crate and write the failing tests**

`Cargo.toml`:

```toml
[package]
name = "forge-adapter-sleep"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }
```

`src/lib.rs` starts with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn holder() -> CaffeinateSleep {
        // `sleep 30` stands in for caffeinate: a child that stays alive until killed.
        CaffeinateSleep::with_command("sleep", vec!["30".to_string()])
    }

    #[test]
    fn holding_starts_a_child_and_releasing_ends_it() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        assert!(sleep.is_running(1));
        sleep.release(1);
        assert!(!sleep.is_running(1));
    }

    #[test]
    fn holding_twice_for_one_key_is_one_child() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        sleep.hold(1).unwrap();
        assert_eq!(sleep.held_count(), 1);
        sleep.release(1);
    }

    #[test]
    fn two_keys_hold_and_release_independently() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        sleep.hold(2).unwrap();
        sleep.release(1);
        assert!(!sleep.is_running(1));
        assert!(sleep.is_running(2));
        sleep.release(2);
    }

    #[test]
    fn releasing_a_key_that_is_not_held_is_ignored() {
        holder().release(9);
    }

    #[test]
    fn a_program_that_cannot_start_is_reported_as_a_message() {
        let sleep = CaffeinateSleep::with_command("forge-no-such-program", Vec::new());
        let error = sleep.hold(1).unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
        assert_eq!(sleep.held_count(), 0);
    }

    #[test]
    fn dropping_the_holder_ends_its_children() {
        let sleep = holder();
        sleep.hold(1).unwrap();
        let pid = sleep.held.lock().unwrap().get(&1).unwrap().id();
        drop(sleep);
        // `kill -0` fails once the process is gone and reaped.
        let alive = std::process::Command::new("kill").args(["-0", &pid.to_string()]).status().unwrap().success();
        assert!(!alive);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-adapter-sleep` → compile error.

- [ ] **Step 3: Write the implementation** (above the tests)

```rust
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use forge_ports::SleepPort;

/// Keeps the Mac awake with one `caffeinate` child per holder. `-w <pid>` ties each child to
/// Forge's own process, so even a crash cannot leave one running.
pub struct CaffeinateSleep {
    program: String,
    args: Vec<String>,
    held: Mutex<HashMap<u64, Child>>,
}

impl CaffeinateSleep {
    pub fn new() -> Self {
        Self::with_command("caffeinate", vec!["-i".to_string(), "-w".to_string(), std::process::id().to_string()])
    }

    pub fn with_command(program: impl Into<String>, args: Vec<String>) -> Self {
        Self { program: program.into(), args, held: Mutex::new(HashMap::new()) }
    }

    #[cfg(test)]
    fn is_running(&self, key: u64) -> bool {
        match self.held.lock().unwrap().get_mut(&key) {
            Some(child) => matches!(child.try_wait(), Ok(None)),
            None => false,
        }
    }

    #[cfg(test)]
    fn held_count(&self) -> usize {
        self.held.lock().unwrap().len()
    }
}

impl Default for CaffeinateSleep {
    fn default() -> Self {
        Self::new()
    }
}

impl SleepPort for CaffeinateSleep {
    fn hold(&self, key: u64) -> Result<(), String> {
        let mut held = self.held.lock().unwrap();
        if held.contains_key(&key) {
            return Ok(());
        }
        let child = Command::new(&self.program)
            .args(&self.args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("could not start {}: {error}", self.program))?;
        held.insert(key, child);
        Ok(())
    }

    fn release(&self, key: u64) {
        let removed = self.held.lock().unwrap().remove(&key);
        if let Some(mut child) = removed {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for CaffeinateSleep {
    fn drop(&mut self) {
        if let Ok(held) = self.held.get_mut() {
            for (_, mut child) in held.drain() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-adapter-sleep` → 6 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-sleep Cargo.lock
git commit -m "feat: sleep adapter on caffeinate" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Workspace wiring and the composition root

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (struct `WorkspaceServices` line ~25, struct `Workspace` line ~54, provider line ~73, `close` line ~609)
- Modify: `crates/forge-app/src/main.rs` (imports; `build_services`)
- Modify: `crates/forge-app/Cargo.toml`

**Interfaces:**
- Consumes: `DockStates`, `DockState` (Task 1); `TerminalPort`, `SleepPort` (Task 3); the two adapters (Tasks 4, 5).
- Produces on `Workspace`: `dock(id) -> DockState`, `change_dock(id, impl FnOnce(&mut DockState))`, `terminal() -> Arc<dyn TerminalPort>`, `prevents_sleep(id) -> bool`, `set_prevent_sleep(id, on: bool)`. `WorkspaceServices` gets `terminal: Arc<dyn TerminalPort>` and `sleep: Arc<dyn SleepPort>`.

- [ ] **Step 1: Add the services and the state**

In `workspace.rs`, change the `use` lines: `use std::collections::HashSet;` (add to the existing `std` imports), `use forge_ports::{FilePickerPort, FolderPickerPort, ProjectRepositoryPort, SessionRepositoryPort, SleepPort, TerminalPort};` and `use crate::dock::{DockState, DockStates};`.

Add to `WorkspaceServices` (after `file_picker`):

```rust
    pub terminal: Arc<dyn TerminalPort>,
    pub sleep: Arc<dyn SleepPort>,
```

Add to `Workspace` (after `archived`):

```rust
    docks: Signal<DockStates>,
    sleeping: Signal<HashSet<LiveSessionId>>,
```

and to the struct literal in `use_workspace_provider` (after `archived: Signal::new(Vec::new()),`):

```rust
            docks: Signal::new(DockStates::default()),
            sleeping: Signal::new(HashSet::new()),
```

- [ ] **Step 2: Add the methods** (in `impl Workspace`, after `set_transcript_view`)

```rust
    /// The dock of one session: which panels are open and how wide.
    pub fn dock(&self, id: LiveSessionId) -> DockState {
        self.docks.read().get(id)
    }

    pub fn change_dock(&self, id: LiveSessionId, change: impl FnOnce(&mut DockState)) {
        let mut docks = self.docks;
        docks.write().change(id, change);
    }

    pub fn terminal(&self) -> Arc<dyn TerminalPort> {
        self.services.peek().terminal.clone()
    }

    pub fn prevents_sleep(&self, id: LiveSessionId) -> bool {
        self.sleeping.read().contains(&id)
    }

    /// Turns "Prevent sleep" on or off for one session. If the computer cannot be held awake the
    /// switch stays off and the person is told why.
    pub fn set_prevent_sleep(&self, id: LiveSessionId, on: bool) {
        let services = (*self.services.peek()).clone();
        let mut sleeping = self.sleeping;
        if on {
            match services.sleep.hold(id.0) {
                Ok(()) => {
                    sleeping.write().insert(id);
                }
                Err(message) => self.report(message),
            }
        } else {
            services.sleep.release(id.0);
            sleeping.write().remove(&id);
        }
    }
```

In `close`, after `views.write().forget(id);` add:

```rust
        let mut docks = self.docks;
        docks.write().forget(id);
        self.set_prevent_sleep(id, false);
```

- [ ] **Step 3: Compose the adapters**

`crates/forge-app/Cargo.toml` `[dependencies]`: add `forge-adapter-sleep = { path = "../forge-adapter-sleep" }` and `forge-adapter-terminal = { path = "../forge-adapter-terminal" }`.

`main.rs`: add `use forge_adapter_sleep::CaffeinateSleep;` and `use forge_adapter_terminal::PtyTerminal;`. In `build_services`, before the `WorkspaceServices { ... }` line add `let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());` and change the literal to:

```rust
    WorkspaceServices {
        registry,
        projects,
        sessions,
        settings,
        extensions,
        folder_picker: picker.clone(),
        file_picker: picker,
        terminal: Arc::new(PtyTerminal::new(shell)),
        sleep: Arc::new(CaffeinateSleep::new()),
    }
```

- [ ] **Step 4: Build and run all tests**

Run: `cargo build --workspace && cargo test --workspace`
Expected: builds; all tests pass (the earlier count plus the new ones). Fix any other place that constructs `WorkspaceServices` (grep found only `main.rs`).

- [ ] **Step 5: Commit**

```bash
git add crates Cargo.lock
git commit -m "feat: the workspace owns each session's dock and prevent-sleep switch" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Vendored xterm.js in the window head

**Files:**
- Create: `scripts/vendor_xterm.sh`, `crates/forge-shared/assets/xterm.js`, `crates/forge-shared/assets/addon-fit.js`, `crates/forge-shared/assets/xterm.css`, `crates/forge-shared/assets/XTERM-LICENSE`, `crates/forge-shared/src/xterm.rs`
- Modify: `crates/forge-shared/src/lib.rs`, `crates/forge-app/src/main.rs` (`window_config`)

**Interfaces:**
- Produces: `forge_shared::xterm_head() -> String`: `<style>` with xterm's CSS and two `<script>` tags (xterm, fit addon); `Config::with_custom_head` puts it in the window's head, so `Terminal` and `FitAddon.FitAddon` exist as globals.

- [ ] **Step 1: Write the vendoring script and run it**

`scripts/vendor_xterm.sh`:

```sh
#!/usr/bin/env bash
# Copies xterm.js and its fit addon from npm into crates/forge-shared/assets/ (committed, so the
# build needs no network). Run it again only to change the versions.
set -euo pipefail
XTERM=5.5.0
FIT=0.10.0
dest="$(cd "$(dirname "$0")/.." && pwd)/crates/forge-shared/assets"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
mkdir -p "$dest"
cd "$work"
npm pack "@xterm/xterm@$XTERM" --silent
mkdir x && tar xzf xterm-xterm-"$XTERM".tgz -C x
cp x/package/lib/xterm.js "$dest/xterm.js"
cp x/package/css/xterm.css "$dest/xterm.css"
cp x/package/LICENSE "$dest/XTERM-LICENSE"
npm pack "@xterm/addon-fit@$FIT" --silent
mkdir f && tar xzf xterm-addon-fit-"$FIT".tgz -C f
cp f/package/lib/addon-fit.js "$dest/addon-fit.js"
echo "vendored xterm $XTERM and addon-fit $FIT into $dest"
```

Run: `chmod +x scripts/vendor_xterm.sh && scripts/vendor_xterm.sh && ls -l crates/forge-shared/assets`
Expected: four files plus the license, `xterm.js` several hundred KB. If `npm pack` names the tarballs differently, list the work directory and adjust the two `tar` lines.

- [ ] **Step 2: Write the failing test**

`crates/forge-shared/src/xterm.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_carries_the_stylesheet_and_exactly_two_scripts() {
        let head = xterm_head();
        assert!(head.starts_with("<style>"));
        assert_eq!(head.matches("<script>").count(), 2, "xterm and its fit addon");
        // A `</script>` inside the vendored code would end a script early.
        assert_eq!(head.matches("</script>").count(), 2);
    }

    #[test]
    fn the_scripts_define_the_two_globals_the_bridge_uses() {
        let head = xterm_head();
        assert!(head.contains("Terminal"));
        assert!(head.contains("FitAddon"));
    }
}
```

Add `mod xterm;` and `pub use xterm::xterm_head;` to `crates/forge-shared/src/lib.rs`. Run `cargo test -p forge-shared xterm::` → compile error (`xterm_head` missing).

- [ ] **Step 3: Write the implementation** (above the tests)

```rust
const XTERM_CSS: &str = include_str!("../assets/xterm.css");
const XTERM_JS: &str = include_str!("../assets/xterm.js");
const XTERM_FIT_JS: &str = include_str!("../assets/addon-fit.js");

/// The HTML that goes into the window's `<head>`: xterm.js, its fit addon and its stylesheet,
/// inline, so the terminal needs no network (MIT licence: `assets/XTERM-LICENSE`).
pub fn xterm_head() -> String {
    format!("<style>{XTERM_CSS}</style><script>{XTERM_JS}</script><script>{XTERM_FIT_JS}</script>")
}
```

In `main.rs`, `window_config()` becomes:

```rust
fn window_config() -> Config {
    Config::new()
        .with_window(
            WindowBuilder::new()
                .with_title("Forge")
                .with_inner_size(LogicalSize::new(1200.0, 800.0)),
        )
        .with_menu(app_menu())
        .with_custom_head(forge_shared::xterm_head())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-shared xterm::` → 2 passed. Then `cargo build -p forge-app` → builds.

- [ ] **Step 5: Commit**

```bash
git add scripts/vendor_xterm.sh crates/forge-shared crates/forge-app
git commit -m "feat: xterm.js vendored into the window head" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The dock and its panel frame

**Files:**
- Create: `crates/forge-widgets/src/dock.rs`, `crates/forge-widgets/src/panel_frame.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `Workspace::dock`, `Workspace::change_dock`, `Panel` (Tasks 1, 6); `TerminalPanel { id, cwd }` (Task 9, so this task's `Dock` is finished together with Task 9; build it after Task 9's file exists — see Step 4).
- Produces: `Dock { id: LiveSessionId, cwd: PathBuf }`; `PanelFrame { id, panel, tools: Element, children: Element }`.

- [ ] **Step 1: Write `panel_frame.rs`**

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

/// The box every dock panel sits in: a header with its title, the panel's own tools, expand and
/// close, and the body below.
#[component]
pub fn PanelFrame(id: LiveSessionId, panel: Panel, tools: Element, children: Element) -> Element {
    let workspace = use_workspace();
    let expanded = workspace.dock(id).expanded == Some(panel);
    rsx! {
        section { class: "dock-panel", "aria-label": "{panel.title()}",
            header { class: "dock-header",
                span { class: "dock-title", "{panel.title()}" }
                div { class: "dock-tools", {tools} }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: if expanded { "Collapse" } else { "Expand" },
                    "aria-label": if expanded { "Collapse" } else { "Expand" },
                    onclick: move |_| {
                        workspace.change_dock(id, |dock| if expanded { dock.collapse() } else { dock.expand(panel) })
                    },
                    if expanded { "⤡" } else { "⤢" }
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: "Close",
                    "aria-label": "Close",
                    onclick: move |_| workspace.change_dock(id, |dock| dock.close(panel)),
                    "✕"
                }
            }
            div { class: "dock-body", {children} }
        }
    }
}
```

- [ ] **Step 2: Write `dock.rs`**

```rust
use std::path::PathBuf;

use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::{use_workspace, Panel};

use crate::terminal_panel::TerminalPanel;

// Where a column's left edge was grabbed: the pointer's x and the column's width at that moment.
#[derive(Clone, Copy)]
struct Grab {
    column: usize,
    x: f64,
    width: f32,
}

/// The columns right of the chat. Each column's left edge is a handle that resizes it; while one
/// is held a transparent shield covers the window, so the drag follows the pointer even over the
/// terminal.
#[component]
pub fn Dock(id: LiveSessionId, cwd: PathBuf) -> Element {
    let workspace = use_workspace();
    let state = workspace.dock(id);
    let mut grab = use_signal(|| None::<Grab>);
    if state.columns.is_empty() {
        return rsx! {};
    }
    let expanded = state.expanded;
    rsx! {
        if grab().is_some() {
            div {
                class: "dock-drag-shield",
                onmousemove: move |event| {
                    if let Some(start) = grab() {
                        let width = start.width + (start.x - event.client_coordinates().x) as f32;
                        workspace.change_dock(id, |dock| dock.resize(start.column, width));
                    }
                },
                onmouseup: move |_| grab.set(None),
            }
        }
        for (index, column) in state.columns.iter().enumerate() {
            if expanded.is_none_or(|panel| column.panels.contains(&panel)) {
                {
                    let width = column.width;
                    let style = if expanded.is_some() { String::new() } else { format!("width: {width}px") };
                    rsx! {
                        div {
                            key: "{index}",
                            class: if expanded.is_some() { "dock-column expanded" } else { "dock-column" },
                            style: "{style}",
                            if expanded.is_none() {
                                div {
                                    class: "dock-resize",
                                    onmousedown: move |event| {
                                        event.prevent_default();
                                        grab.set(Some(Grab { column: index, x: event.client_coordinates().x, width }));
                                    },
                                }
                            }
                            for panel in column.panels.iter().copied() {
                                match panel {
                                    Panel::Terminal => rsx! { TerminalPanel { key: "{panel:?}", id, cwd: cwd.clone() } },
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Add the CSS** (append to `theme.css`; the toolbar rules are in Task 10)

```css
/* The dock: columns right of the chat. */
.dock-column { position: relative; flex: none; display: flex; flex-direction: column; min-width: 0; max-width: 70vw; padding: 8px; box-sizing: border-box; background: var(--bg-sidebar); border-left: 1px solid var(--border); }
.dock-column.expanded { flex: 1; max-width: none; }
.dock-resize { position: absolute; left: -3px; top: 0; bottom: 0; width: 5px; cursor: col-resize; z-index: 7; }
.dock-resize:hover { background: var(--accent-soft); }
.dock-drag-shield { position: fixed; inset: 0; z-index: 20; cursor: col-resize; }
.dock-panel { flex: 1; min-height: 0; display: flex; flex-direction: column; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); overflow: hidden; }
.dock-header { flex: none; display: flex; align-items: center; gap: 4px; padding: 4px 6px; border-bottom: 1px solid var(--border); font-size: 12px; }
.dock-title { font-weight: 600; margin-right: 4px; }
.dock-tools { flex: 1; min-width: 0; display: flex; align-items: center; gap: 4px; overflow-x: auto; }
.dock-body { flex: 1; min-height: 0; display: flex; flex-direction: column; position: relative; }
.main-pane.dock-expanded { display: none; }
```

- [ ] **Step 4: Register the modules and build**

`lib.rs`: add `mod dock;`, `mod panel_frame;`, `mod terminal_panel;` (Task 9 creates the last) and `pub use dock::Dock;`, `pub use panel_frame::PanelFrame;`. Do Task 9's file before building. Run `cargo build -p forge-widgets` after Task 9 Step 1.

- [ ] **Step 5: Commit** (together with Task 9, since `Dock` names `TerminalPanel`)

Commit after Task 9.

---

### Task 9: The terminal panel

**Files:**
- Create: `crates/forge-widgets/src/terminal_panel.rs`
- Modify: `crates/forge-widgets/Cargo.toml`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `TerminalTabs`, `TabId`, `Panel`, `use_workspace`, `Workspace::terminal`, `change_dock` (Tasks 1, 2, 6); `PanelFrame` (Task 8); `TerminalPort` types (Task 3); the `Terminal`/`FitAddon` globals (Task 7).
- Produces: `TerminalPanel { id: LiveSessionId, cwd: PathBuf }`.

- [ ] **Step 1: Add the dependencies and write the panel**

Run: `cargo add -p forge-widgets forge-ports --path crates/forge-ports && cargo add -p forge-widgets base64@0.22 serde_json@1 && cargo add -p forge-widgets serde@1 --features derive && cargo add -p forge-widgets tokio@1 --features macros,sync`

`terminal_panel.rs`:

```rust
use std::cell::Cell;
use std::path::{Path, PathBuf};
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
    let mut phase = use_signal(|| Phase::Starting);
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
```

Note: `Path` is imported for completeness only if you pass `&Path`; remove unused imports the compiler flags.

- [ ] **Step 2: Add the terminal CSS** (append to `theme.css`)

```css
.terminal-tab { display: inline-flex; align-items: center; flex: none; border-radius: 6px; }
.terminal-tab.active { background: var(--bg-hover); }
.terminal-tab-label, .terminal-tab-close { border: none; background: transparent; color: var(--fg); font: inherit; font-size: 12px; cursor: pointer; padding: 2px 6px; }
.terminal-tab-close { color: var(--fg-muted); padding: 2px 4px; }
.terminal-view { position: relative; flex: 1; min-height: 0; display: none; padding: 4px 0 0 6px; box-sizing: border-box; }
.terminal-view.active { display: flex; flex-direction: column; }
.terminal-host { flex: 1; min-height: 0; }
.terminal-exited { flex: none; display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 4px 8px; border-top: 1px solid var(--border); font-size: 12px; color: var(--fg-muted); }
.terminal-restart { border: 1px solid var(--border); border-radius: 6px; background: var(--bg-card); color: var(--fg); font: inherit; font-size: 12px; padding: 2px 8px; cursor: pointer; }
.terminal-restart:hover { background: var(--bg-hover); }
```

- [ ] **Step 3: Build**

Run: `cargo build -p forge-widgets` → builds without warnings. Fix what the compiler says (the likely spots: the `use_future` closure captures, the `rsx!` in `for` loops, an unused `Path` import).

- [ ] **Step 4: Commit Tasks 8 and 9 together**

```bash
git add crates scripts
git commit -m "feat: the dock widget and the terminal panel" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: The toolbar and the layout

**Files:**
- Create: `crates/forge-widgets/src/session_tools.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `Workspace::dock`, `change_dock`, `prevents_sleep`, `set_prevent_sleep` (Task 6); `Dock` (Task 8).
- Produces: `SessionTools { id: LiveSessionId, terminal: bool }`; `ChatWindow` lays out the chat and the dock side by side.

- [ ] **Step 1: Look at the existing menu CSS you will reuse**

Run: `grep -n "picker-menu\|picker-item\|picker-backdrop\|\.picker\b\|\.picker-button\|\.icon-button" crates/forge-shared/src/theme.css`
Expected: the rules for `.picker`, `.picker-button`, `.picker-menu` (it opens upward from the composer), `.picker-item`, `.picker-backdrop`, `.icon-button`.

- [ ] **Step 2: Write `session_tools.rs`**

```rust
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
```

- [ ] **Step 3: Add the toolbar CSS** (append to `theme.css`)

```css
.pane-tools { margin-left: auto; display: flex; align-items: center; gap: 4px; font-weight: 400; }
.icon-button.active { background: var(--bg-hover); color: var(--accent); }
/* The `⋮` menu in the header opens downwards, below its button. */
.picker-menu.down { top: 100%; bottom: auto; left: auto; right: 0; margin-top: 4px; }
```

If Step 1 showed that `.picker-menu` positions itself with other properties (for example `transform`), reset those in `.picker-menu.down` too.

- [ ] **Step 4: Change `chat_window.rs`**

Add to the imports: `use crate::dock::Dock;` and `use crate::session_tools::SessionTools;`. After the `archived` binding add:

```rust
    let dock_expanded = workspace.dock(id).expanded.is_some() && !archived;
```

Change the first lines of the `rsx!` so the chat and the dock are siblings, and put the tools in the header. The result:

```rust
    rsx! {
        div { class: if dock_expanded { "main-pane dock-expanded" } else { "main-pane" },
            div { class: "pane-header",
                span { class: "pane-title", "{heading}" }
                div { class: "view-switch", role: "radiogroup", "aria-label": "Transcript view",
                    // ... the existing buttons, unchanged ...
                }
                SessionTools { id, terminal: !archived }
            }
            // ... everything else in the pane, unchanged ...
        }
        if !archived {
            Dock { id, cwd: view.project_path.clone() }
        }
    }
```

(Only the opening `div`'s class, the `SessionTools` line and the trailing `if !archived { Dock }` are new; the rest of the pane is untouched.)

`lib.rs`: add `mod session_tools;` and `pub use session_tools::SessionTools;`.

- [ ] **Step 5: Build and run all tests**

Run: `cargo build --workspace && cargo test --workspace`
Expected: builds; all tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: terminal toggle and prevent-sleep menu in the chat header" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Check it in the real app, then the docs

**Files:**
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-01, PNL-06), `docs/superpowers/roadmap/README.md` (plan table), `docs/superpowers/specs/2026-10-03-right-dock-panels-design.md` (an "As built (Plan 21A)" section), `docs/superpowers/roadmap/observations.md` (what Forge showed vs the real app)
- Create: a memory file `plan-21-dock-terminal.md` and its line in `MEMORY.md` (in `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/`)

- [ ] **Step 1: Launch with the harness**

Run: `scripts/harness/launch.sh` then `python3 scripts/harness/capture.py` and open `/private/tmp/forge-harness-shot.png`. Open the session of `my-app` (its folder exists in the fixture). Rules from `scripts/harness/README.md` apply: no message is sent.

- [ ] **Step 2: Walk this checklist, one capture per line**

1. The header shows `>_` and `⋮` at the right; the `>_` is highlighted only while the panel is open.
2. Click `>_`: a column opens at the right, about 480 px, the chat narrows; a shell prompt appears and the cursor blinks.
3. Click into the terminal and press keys with `osascript key code` (type `echo`, space, `hi`, return): `hi` is printed. (Korean input: `osascript keystroke` drops letters under an IME; type with the US layout.)
4. `ls` lists the fixture folder, so the shell started in the session folder.
5. Drag the column's left edge: the width follows the pointer and stops at 280 and at 900; the shell re-wraps (run `tput cols`).
6. `+` opens "Shell 2"; clicking the tab labels switches; `×` closes one; closing the last closes the panel.
7. Expand (⤢): the terminal fills the area right of the sidebar and the chat is hidden; Collapse (⤡) returns.
8. Run `exit 3`: the tab shows "Process exited (3)" and Restart; Restart opens a fresh shell.
9. Open the `my_app` session (its folder is missing in the fixture): the terminal shows `Working directory "…" does not exist` and Restart.
10. `⋮` → "Prevent sleep" switches to "On · This session only"; in a shell outside the app run `pgrep -fl caffeinate` and see one; switch off and see it gone; switch on, close the session, and see it gone.
11. Switch to another session and back: the panel is still open and a fresh shell runs (the old one ended: `ps` shows no extra shell of the old tab).
12. Quit the app: `pgrep -fl "caffeinate|-l"` shows nothing left over from it.

Fix anything that fails before going on; each fix is its own commit.

- [ ] **Step 3: Stop the harness and record what was seen**

Run: `scripts/harness/stop.sh`. Add to `observations.md` a short "Forge vs the real app, Plan 21A" list: where Forge differs (for example the shell ends when the session is left, the toolbar glyphs). Do not copy screenshots.

- [ ] **Step 4: Update the roadmap and spec**

`backlog.md`: set PNL-01 and PNL-06 `Plan` to `Plan 21A (built)` and add one sentence each on what was built (`>_` toggle, tabs, expand, missing-folder message, Restart; `⋮` Prevent sleep, per session, `caffeinate -i -w <pid>`). `README.md` plan table: add a row "21A | Dock, terminal, prevent sleep (built) | PNL-01, PNL-06" and mark 21B as the Changes, Files and Preview plan. The spec gets "As built (Plan 21A)": what changed from the design (the width clamp is a fixed 280–900 px with CSS `min-width`; shells end when the session is left) and the checklist result.

- [ ] **Step 5: Save the memory and commit**

Write `plan-21-dock-terminal.md` (type `project`, per the memory format: what was built, branch `plan-21`, the xterm bridge lesson and the `with_custom_head` choice, what is left: Plan 21B) and add its line to `MEMORY.md`. Then:

```bash
git add docs
git commit -m "docs: Plan 21A as built, backlog and roadmap updated" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Finish the branch**

Use superpowers:finishing-a-development-branch (base branch is `master`).
