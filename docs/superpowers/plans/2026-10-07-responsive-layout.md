# Responsive layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forge reflows like the Claude desktop app from 1500 pt down to 600 pt: the sidebar folds into an overlay, an open dock panel pushes the chat and then floats over it, the chat header shrinks, and the new-session screen shrinks.

**Architecture:** One pure function in `forge-domain` (`layout_for`) turns the window width, the sidebar width and the dock width into a `Layout` (sidebar docked or overlay, dock push or float, the chat width). A `use_window_width()` signal in `forge-features` feeds it from the webview's `resize` event. `WorkspacePage` puts the result on `.workspace` as classes (`sidebar-overlay`, `dock-float`, `chat-narrow`); everything visual hangs on those classes in `theme.css`. Smooth shrinking (card, heatmap, margins) is plain CSS.

**Tech Stack:** Rust, Dioxus 0.6 desktop (WKWebView), `theme.css`, Python harness (`scripts/harness/`).

**Spec:** `docs/superpowers/specs/2026-10-07-responsive-layout-design.md` (amended in Task 8 to match the decisions below). Observations: `docs/superpowers/roadmap/observations.md`, round 10.

## Global Constraints

- Smallest window 600 pt wide (`MIN_WINDOW_WIDTH = 600`); the window minimum height is 400 pt (a guess, checked in Task 7).
- Sidebar is an overlay at widths up to and including 700 pt, docked from 701 pt (observed: overlay at 700, docked at 710).
- The overlay sidebar is 280 pt wide, on top of the pane, not dimmed, not pushing; the toggle shows and hides it.
- A dock panel never makes the chat narrower than 300 pt (`MIN_CHAT_WIDTH`); below that the dock floats over the chat. A floating column is at most 400 pt wide.
- Header goes narrow (class `chat-narrow`) when the chat is under 560 pt.
- Home and new-session column: at most 770 pt of content, side margin 40 pt (25 pt at windows of 850 pt and below); usage card at most 480 pt.
- `prefs.hidden` and `prefs.width` are never written by the overlay.
- No horizontal scroll bar and nothing cut off (except titles with an ellipsis) at 600 pt.
- Code style: match the surrounding crates (doc comments in plain English on public items, tests named as sentences, `use_workspace()` handles are `Copy`).

## Review Focus

- A window exactly at the thresholds (600, 700, 701; chat exactly 300 and 299 pt) picks the right mode: unit tests in Task 1.
- A saved sidebar width of 480 with a window of 701 pt: the dock must still float instead of crushing the chat (Task 1 test).
- The overlay sidebar opens, then the window grows past 700 pt and shrinks again: it must not pop open by itself (Task 3 effect; checked in Task 7).
- Choosing a session in the overlay sidebar closes it (Task 3; checked in Task 7).
- An expanded dock column (fills the pane) must not float (dock width counts as 0 when expanded, Task 3).
- Fixed-position menus and the picker backdrops keep covering the whole window (this is why the header uses a class from the width signal and not CSS container queries, which would contain them; Task 5 check).
- A very long session title, and the view switch (Normal / Thinking / Detailed), at a chat width of 300 pt (Task 5 check).

---

## File structure

| File | Change |
| --- | --- |
| `crates/forge-domain/src/layout.rs` | new: `layout_for`, `workspace_class`, constants |
| `crates/forge-domain/src/lib.rs` | export the new items |
| `crates/forge-features/src/window_width.rs` | new: `use_window_width()` |
| `crates/forge-features/src/lib.rs` | export it |
| `crates/forge-pages/src/workspace_page.rs` | layout classes, overlay sidebar, toggle |
| `crates/forge-widgets/src/dock.rs` | a `.dock` wrapper around the columns |
| `crates/forge-widgets/src/session_tools.rs` | menu items for Terminal / Changes / Preview, shown when narrow |
| `crates/forge-app/src/main.rs` | minimum window size |
| `crates/forge-shared/src/theme.css` | all the visual rules |
| `scripts/harness/geometry.py`, `test_geometry.py`, `resize.py`, `README.md` | `resize.py W H` |
| `docs/superpowers/roadmap/*`, the spec | status and "As built" |

---

### Task 1: The layout function

**Files:**
- Create: `crates/forge-domain/src/layout.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Produces (used by Task 3):
  - `pub const MIN_WINDOW_WIDTH: u32 = 600; SIDEBAR_OVERLAY_UP_TO: u32 = 700; SIDEBAR_RAIL_WIDTH: u32 = 36; MIN_CHAT_WIDTH: u32 = 300; NARROW_CHAT_BELOW: u32 = 560;`
  - `pub enum SidebarMode { Docked, Overlay }`, `pub enum DockMode { Push, Float }` (both `Debug, Clone, Copy, PartialEq, Eq`)
  - `pub struct Layout { pub sidebar: SidebarMode, pub dock: DockMode, pub chat_width: u32 }` (`Debug, Clone, Copy, PartialEq, Eq`)
  - `pub fn layout_for(window_width: u32, sidebar_width: u32, dock_width: u32) -> Layout` — `sidebar_width` is what a docked sidebar (or its rail) takes; `dock_width` is the sum of the open columns, 0 when none or when one is expanded.
  - `pub fn workspace_class(layout: Layout, resizing: bool) -> String`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/layout.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sidebar_is_an_overlay_up_to_700_and_docked_from_701() {
        assert_eq!(layout_for(600, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(700, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(701, 264, 0).sidebar, SidebarMode::Docked);
        assert_eq!(layout_for(1200, 264, 0).sidebar, SidebarMode::Docked);
    }

    #[test]
    fn a_window_below_the_minimum_is_treated_like_the_smallest() {
        assert_eq!(layout_for(0, 264, 0).sidebar, SidebarMode::Overlay);
        assert_eq!(layout_for(599, 264, 480).dock, DockMode::Float);
    }

    #[test]
    fn the_dock_pushes_the_chat_until_the_chat_would_be_under_300() {
        // 1060 - 280 - 480 = 300: exactly enough.
        assert_eq!(layout_for(1060, 280, 480).dock, DockMode::Push);
        assert_eq!(layout_for(1060, 280, 480).chat_width, 300);
        // One point less and it floats; the chat keeps all the room beside the sidebar.
        assert_eq!(layout_for(1059, 280, 480).dock, DockMode::Float);
        assert_eq!(layout_for(1059, 280, 480).chat_width, 779);
    }

    #[test]
    fn an_overlay_sidebar_takes_no_room_from_the_dock() {
        // 650 - 0 - 350 = 300 even though the saved sidebar width is 480.
        let layout = layout_for(650, 480, 350);
        assert_eq!(layout.sidebar, SidebarMode::Overlay);
        assert_eq!(layout.dock, DockMode::Push);
        assert_eq!(layout.chat_width, 300);
    }

    #[test]
    fn a_wide_saved_sidebar_in_a_small_docked_window_floats_the_dock() {
        let layout = layout_for(701, 480, 260);
        assert_eq!(layout.sidebar, SidebarMode::Docked);
        assert_eq!(layout.dock, DockMode::Float);
        assert_eq!(layout.chat_width, 221);
    }

    #[test]
    fn no_dock_is_always_a_push_and_the_chat_gets_what_is_left() {
        let layout = layout_for(701, 480, 0);
        assert_eq!(layout.dock, DockMode::Push);
        assert_eq!(layout.chat_width, 221);
        assert_eq!(layout_for(1200, 264, 0).chat_width, 936);
    }

    #[test]
    fn a_dock_wider_than_the_window_does_not_underflow() {
        let layout = layout_for(600, 264, 4000);
        assert_eq!(layout.dock, DockMode::Float);
        assert_eq!(layout.chat_width, 600);
    }

    #[test]
    fn the_class_list_names_each_mode_that_is_on() {
        assert_eq!(workspace_class(layout_for(1500, 264, 0), false), "workspace");
        assert_eq!(workspace_class(layout_for(1500, 264, 0), true), "workspace resizing");
        // 650 pt: overlay sidebar, a 350 pt dock still pushes (chat exactly 300), the chat is narrow.
        assert_eq!(workspace_class(layout_for(650, 264, 350), false), "workspace sidebar-overlay chat-narrow");
        // 1000 pt: the 480 pt dock would leave 240 pt, so it floats and the chat keeps 720 pt.
        assert_eq!(workspace_class(layout_for(1000, 280, 480), false), "workspace dock-float");
        // 601 pt: overlay sidebar, floating dock, the chat has all 601 pt so it is not narrow.
        assert_eq!(workspace_class(layout_for(601, 264, 480), false), "workspace sidebar-overlay dock-float");
    }

    #[test]
    fn the_header_is_narrow_under_560() {
        assert_eq!(workspace_class(layout_for(823, 264, 0), false), "workspace chat-narrow"); // chat 559
        assert_eq!(workspace_class(layout_for(824, 264, 0), false), "workspace"); // chat 560
    }
}
```

Add to `crates/forge-domain/src/lib.rs`, with the other `mod` lines `mod layout;` and with the other exports:

```rust
pub use layout::{
    layout_for, workspace_class, DockMode, Layout, SidebarMode, MIN_CHAT_WIDTH, MIN_WINDOW_WIDTH, NARROW_CHAT_BELOW, SIDEBAR_OVERLAY_UP_TO,
    SIDEBAR_RAIL_WIDTH,
};
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-domain layout`
Expected: compile error, `layout_for` not found.

- [ ] **Step 3: Implement**

Put above the tests in `layout.rs`:

```rust
//! How the window width decides the layout. Round 10 of the roadmap observations (the real Claude
//! app at 1500 pt down to 600 pt) is where the numbers come from.

/// The window cannot be made narrower than this (the real app refuses too).
pub const MIN_WINDOW_WIDTH: u32 = 600;
/// At this width and below the sidebar is an overlay that the toggle shows.
pub const SIDEBAR_OVERLAY_UP_TO: u32 = 700;
/// What the folded sidebar's rail takes in a docked layout.
pub const SIDEBAR_RAIL_WIDTH: u32 = 36;
/// An open dock panel pushes the chat only while the chat keeps this much.
pub const MIN_CHAT_WIDTH: u32 = 300;
/// A chat narrower than this gets the compact header.
pub const NARROW_CHAT_BELOW: u32 = 560;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarMode {
    Docked,
    Overlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockMode {
    Push,
    Float,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    pub sidebar: SidebarMode,
    pub dock: DockMode,
    /// The width the chat column gets.
    pub chat_width: u32,
}

/// `sidebar_width` is what a docked sidebar (or its rail) takes; `dock_width` is the sum of the open
/// dock columns, 0 when there is none or when one is expanded over the pane.
pub fn layout_for(window_width: u32, sidebar_width: u32, dock_width: u32) -> Layout {
    let window_width = window_width.max(MIN_WINDOW_WIDTH);
    let sidebar = if window_width <= SIDEBAR_OVERLAY_UP_TO { SidebarMode::Overlay } else { SidebarMode::Docked };
    let taken = match sidebar {
        SidebarMode::Docked => sidebar_width,
        SidebarMode::Overlay => 0,
    };
    let beside = window_width.saturating_sub(taken);
    let pushed = beside.saturating_sub(dock_width);
    if dock_width == 0 || pushed >= MIN_CHAT_WIDTH {
        Layout { sidebar, dock: DockMode::Push, chat_width: pushed }
    } else {
        Layout { sidebar, dock: DockMode::Float, chat_width: beside }
    }
}

/// The class list of `.workspace`: every visual rule of the responsive layout hangs on one of these.
pub fn workspace_class(layout: Layout, resizing: bool) -> String {
    let mut class = String::from("workspace");
    if resizing {
        class.push_str(" resizing");
    }
    if layout.sidebar == SidebarMode::Overlay {
        class.push_str(" sidebar-overlay");
    }
    if layout.dock == DockMode::Float {
        class.push_str(" dock-float");
    }
    if layout.chat_width < NARROW_CHAT_BELOW {
        class.push_str(" chat-narrow");
    }
    class
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-domain layout`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: layout_for decides sidebar and dock modes from the window width (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The window width signal

**Files:**
- Create: `crates/forge-features/src/window_width.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces: `pub fn use_window_width() -> Signal<u32>` — the webview's `window.innerWidth` in CSS pixels, rounded; 1200 until the first reading. Written only when the value changes.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::WATCH_WIDTH;

    #[test]
    fn the_listener_replaces_its_old_self_and_reports_at_once_and_on_every_resize() {
        // `dioxus.send` belongs to the eval that installed the listener; a new page scope must take
        // over, so the old handler is removed first.
        assert!(WATCH_WIDTH.contains("removeEventListener('resize'"), "{WATCH_WIDTH}");
        assert!(WATCH_WIDTH.contains("addEventListener('resize'"));
        assert!(WATCH_WIDTH.contains("dioxus.send"));
        assert!(WATCH_WIDTH.contains("Math.round(window.innerWidth)"));
        assert!(WATCH_WIDTH.trim_end().ends_with("window.__forgeWidthHandler();"), "reports once at the start");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test -p forge-features window_width`
Expected: compile error (module missing). Add `mod window_width;` and `pub use window_width::use_window_width;` to `crates/forge-features/src/lib.rs` next to the other `mod` and `pub use` lines first.

- [ ] **Step 3: Implement**

```rust
use dioxus::prelude::*;

// The page tells Rust its width. The handler is replaced each time (`dioxus.send` belongs to the
// eval that created it) and reports once at the start.
const WATCH_WIDTH: &str = "if (window.__forgeWidthHandler) { window.removeEventListener('resize', window.__forgeWidthHandler); } window.__forgeWidthHandler = () => dioxus.send(Math.round(window.innerWidth)); window.addEventListener('resize', window.__forgeWidthHandler); window.__forgeWidthHandler();";

/// The width of the window in CSS pixels, which are the points of the screen. It reads 1200 (the
/// size the window opens at) until the page has reported, and changes only when the width does.
pub fn use_window_width() -> Signal<u32> {
    let mut width = use_signal(|| 1200u32);
    use_future(move || async move {
        let mut changes = document::eval(WATCH_WIDTH);
        while let Ok(next) = changes.recv::<u32>().await {
            if width() != next {
                width.set(next);
            }
        }
    });
    width
}
```

- [ ] **Step 4: Run the test**

Run: `cargo test -p forge-features window_width`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: a signal that follows the window width (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Sidebar overlay, window minimum, classes on the workspace

**Files:**
- Modify: `crates/forge-pages/src/workspace_page.rs`
- Modify: `crates/forge-app/src/main.rs:88-93`
- Modify: `crates/forge-shared/src/theme.css` (near `.sidebar-rail`, line ~416; and `.main-pane`, line 95)

**Interfaces:**
- Consumes: `layout_for`, `workspace_class`, `SidebarMode`, `SIDEBAR_OVERLAY_UP_TO`, `SIDEBAR_RAIL_WIDTH` (`forge_domain`), `use_window_width` (`forge_features`), `workspace.dock(id)` returning `DockState { columns: Vec<Column { width: f32, .. }>, expanded: Option<Panel> }`.
- Produces: `.workspace` classes `sidebar-overlay`, `dock-float`, `chat-narrow`; `.sidebar-toggle` button; `.sidebar-backdrop`.

- [ ] **Step 1: Window minimum**

In `crates/forge-app/src/main.rs` change `window_config`:

```rust
            WindowBuilder::new()
                .with_title("Forge")
                .with_inner_size(LogicalSize::new(1200.0, 800.0))
                // The real Claude window refuses to go below 600 pt wide.
                .with_min_inner_size(LogicalSize::new(600.0, 400.0)),
```

Run: `cargo check -p forge-app` — expected: builds.

- [ ] **Step 2: WorkspacePage**

Replace the imports and the body of `WorkspacePage` in `crates/forge-pages/src/workspace_page.rs`. The imports become:

```rust
use dioxus::prelude::*;
use forge_domain::{layout_for, workspace_class, SidebarMode, SIDEBAR_OVERLAY_UP_TO, SIDEBAR_RAIL_WIDTH};
use forge_features::{refocus_workspace, use_settings_provider, use_window_width, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, SearchPalette, SettingsOverlay, Sidebar};
```

After the existing `use_effect` that calls `settings.set_project(...)`, replace `let hidden = workspace.prefs().hidden;` with:

```rust
    let window_width = use_window_width();
    // Whether the overlay sidebar is out. Not saved: `prefs.hidden` and `prefs.width` belong to the
    // docked sidebar and the overlay never writes them.
    let mut peek = use_signal(|| false);
    // Choosing a session, or opening the new-session screen, puts the overlay away.
    use_effect(move || {
        let _ = workspace.active();
        peek.set(false);
    });
    // Growing past the overlay width forgets an open overlay, so it does not come back by itself.
    use_effect(move || {
        if window_width() > SIDEBAR_OVERLAY_UP_TO {
            peek.set(false);
        }
    });

    let prefs = workspace.prefs();
    let hidden = prefs.hidden;
    // The room the open dock columns ask for; an expanded column fills the pane and asks for none.
    let dock_width: u32 = match workspace.active() {
        Active::Live(id) => {
            let dock = workspace.dock(id);
            if dock.expanded.is_some() { 0 } else { dock.columns.iter().map(|column| column.width as u32).sum() }
        }
        Active::NewSession => 0,
    };
    let docked_sidebar = if hidden { SIDEBAR_RAIL_WIDTH } else { prefs.clamped_width() };
    let layout = layout_for(window_width(), docked_sidebar, dock_width);
```

Change the root `div`'s class and the key handler:

```rust
        div {
            class: workspace_class(layout, dragging()),
            tabindex: "0",
            onmounted: ...unchanged...,
            onkeydown: move |event| {
                // Escape puts the overlay sidebar away, unless an overlay of the app is on top of it.
                if peek() && event.key() == Key::Escape && !settings.is_open() && !workspace.search_open() {
                    peek.set(false);
                    return;
                }
                // ...the existing lines
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open()) {
                    settings.on_key(&event);
                }
            },
```

Replace the `if hidden { ... } else { ... }` block with:

```rust
            match layout.sidebar {
                SidebarMode::Docked => rsx! {
                    if hidden {
                        div { class: "sidebar-rail",
                            button {
                                class: "icon-button",
                                title: "Show the sidebar",
                                onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = false),
                                "›"
                            }
                        }
                    } else {
                        Sidebar {}
                        div {
                            class: if dragging() { "sidebar-resize dragging" } else { "sidebar-resize" },
                            onmousedown: move |event| {
                                event.prevent_default();
                                dragging.set(true);
                            },
                        }
                    }
                },
                SidebarMode::Overlay => rsx! {
                    button {
                        class: "icon-button sidebar-toggle",
                        r#type: "button",
                        title: "Toggle the sidebar",
                        "aria-label": "Toggle the sidebar",
                        "aria-expanded": "{peek()}",
                        onclick: move |_| peek.set(!peek()),
                        "☰"
                    }
                    if peek() {
                        div { class: "sidebar-backdrop", onclick: move |_| peek.set(false) }
                        Sidebar {}
                    }
                },
            }
```

(Keep the `match workspace.active()` block and the two overlays after it.)

Run: `cargo check -p forge-pages`
Expected: builds. (`Key`, `Active::NewSession` and `DockState` fields are what `session_tools.rs` / `dock.rs` already use; if `Active` has other variants, `cargo check` names them: add them to the `0` arm.)

- [ ] **Step 3: CSS**

In `theme.css` change line 95 to `.main-pane { flex: 1; min-width: 0; display: flex; flex-direction: column; }` and add after the `.sidebar-rail` rule:

```css
/* Narrow window: the sidebar is an overlay the toggle shows (observation round 10). */
.sidebar-toggle { position: absolute; left: 8px; top: 10px; z-index: 31; }
.sidebar-backdrop { position: absolute; inset: 0; z-index: 29; }
.sidebar-overlay .sidebar { position: absolute; left: 0; top: 0; bottom: 0; z-index: 30; width: 280px !important; padding-top: 48px; box-shadow: var(--shadow); }
.sidebar-overlay .pane-header, .sidebar-overlay.chat-narrow .pane-header { padding-left: 48px; }
.sidebar-overlay .home-top { padding-top: 52px; }
```

- [ ] **Step 4: Run all tests and look at it**

Run: `cargo test --workspace` — expected: PASS.
Then `scripts/harness/launch.sh`, `python3 scripts/harness/capture.py` (1200 wide: unchanged look), and drag the window narrower by hand or wait for Task 7's `resize.py`. At this point only check that 1200 looks as before.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: the sidebar folds into an overlay at 700 pt and below (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The dock floats over a narrow chat

**Files:**
- Modify: `crates/forge-widgets/src/dock.rs:33-79`
- Modify: `crates/forge-shared/src/theme.css` (dock block, line ~446)

**Interfaces:**
- Consumes: the `dock-float` class on `.workspace` (Task 3).
- Produces: a `.dock` wrapper around all columns of one session (`display: contents` normally, a floating row in `dock-float`).

- [ ] **Step 1: Wrap the columns**

In `dock.rs` the component returns `rsx! { if grab().is_some() {...} for (index, column) in ... {...} }`. Wrap both children in one element:

```rust
    rsx! {
        div { class: "dock",
            if grab().is_some() {
                // ...unchanged shield...
            }
            for (index, column) in state.columns.iter().enumerate() {
                // ...unchanged...
            }
        }
    }
```

(Indent the existing children one level; no other change.)

- [ ] **Step 2: CSS**

After `.dock-column.expanded { ... }` add:

```css
/* The columns of one session. Beside the chat they are plain flex items; when the chat would get
   too narrow they float over it as one rounded card (observation round 10). */
.dock { display: contents; }
.dock-float .dock { display: flex; position: absolute; top: 8px; right: 8px; bottom: 8px; z-index: 25; max-width: calc(100% - 16px); border-radius: var(--radius-lg); overflow: hidden; box-shadow: var(--shadow); }
.dock-float .dock-column { flex: none; max-width: 400px; }
.dock-float .dock-column.expanded { max-width: none; }
```

- [ ] **Step 3: Check**

Run: `cargo test --workspace` (PASS), `cargo check -p forge-widgets`.
Real window: after Task 7's `resize.py` exists, open Changes at 900 pt: the card floats with a margin, the chat keeps its width. For now check at 1200 that the push layout is unchanged (`.dock { display: contents }` must not change anything): open Terminal, compare with the capture before this task.

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: the dock floats over the chat when the chat would get narrow (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: A compact chat header

**Files:**
- Modify: `crates/forge-widgets/src/session_tools.rs:50-90`
- Modify: `crates/forge-shared/src/theme.css` (after `.pane-tools`, line ~471)

**Interfaces:**
- Consumes: class `chat-narrow` on `.workspace` (Task 1, set in Task 3).

Decision: Forge's header has no folder tag (the heading is "folder · title"), so the title's ellipsis does that job. The terminal, Changes and Preview icons move into the `⋮` menu when narrow; the view switch (Normal / Thinking / Detailed) wraps to a second row instead of disappearing, because the real app has it in a menu and Forge has no menu for it.

- [ ] **Step 1: Menu items for the three icons**

In `session_tools.rs`, inside the `picker-menu` div, before the existing `if terminal { ... Files ... }`, add:

```rust
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
                                span { "Terminal" }
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
                                span { "Changes" }
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
                                span { "Preview" }
                            }
                        }
```

- [ ] **Step 2: CSS**

```css
/* A narrow chat (under 560 pt): the header and the side margins shrink. */
.menu-narrow { display: none; }
.chat-narrow .picker-item.menu-narrow { display: flex; }
.chat-narrow .pane-tools > .icon-button { display: none; }
.chat-narrow .pane-header { flex-wrap: wrap; row-gap: 6px; padding: 12px 16px; }
.chat-narrow .pane-title { flex: 1 1 120px; }
.chat-narrow .messages { padding: 8px 16px 16px; }
.chat-narrow .composer, .chat-narrow .permission-panel, .chat-narrow .question-panel, .chat-narrow .banner { width: calc(100% - 32px); }
```

The overlay rule from Task 3 (`.sidebar-overlay.chat-narrow .pane-header`) is more specific, so the header keeps room for the `☰` button.

- [ ] **Step 3: Check**

Run: `cargo test --workspace`; `cargo check -p forge-widgets`. Real window in Task 7: at 600 pt with a session open the header shows `☰`, the title (ellipsis), `⋮`; the `⋮` menu lists Terminal, Changes, Preview, Files, Prevent sleep; the view switch is on its own row.

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: the chat header gets compact when the chat is narrow (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: New-session screen and Home card

**Files:**
- Modify: `crates/forge-shared/src/theme.css` (lines 197, 257, 259, 271, and the new-session block ~515)

- [ ] **Step 1: Column and margins**

Change `.home-top` and `.new-session`:

```css
.home-top { flex: none; width: 100%; max-width: 850px; margin: 0 auto; padding: 32px 40px 0; box-sizing: border-box; }
.new-session { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 14px; padding: 24px 40px; max-width: 850px; margin: 0 auto; width: 100%; box-sizing: border-box; }
.usage-card { width: 100%; max-width: 480px; box-sizing: border-box; padding: 14px; border-radius: var(--radius-lg); background: var(--bg-sidebar); }
@media (max-width: 850px) {
  .home-top { padding-left: 25px; padding-right: 25px; }
  .new-session { padding-left: 25px; padding-right: 25px; }
}
```

(850 px of content plus the 40 px margins is 770 + 80.) `.sidebar-overlay .home-top { padding-top: 52px }` from Task 3 stays.

- [ ] **Step 2: Heatmap shrinks with the card**

`.heatmap` is `grid-auto-flow: column; grid-template-rows: repeat(7, 1fr)` with square cells. Add `grid-auto-columns: 1fr;` so the weeks share the card width and the cells shrink with it:

```css
.heatmap { display: grid; grid-auto-flow: column; grid-auto-columns: 1fr; grid-template-rows: repeat(7, 1fr); gap: 3px; }
```

- [ ] **Step 3: Check**

Run `cargo test --workspace`. Real window (Task 7): at 1500 the card is 480 pt wide, the column is centred at about 770 pt; at 750 the card is about 420 pt and the heatmap cells are smaller, not cut off; at 600 nothing scrolls sideways. If the heatmap overflows the card, `min-width: 0` on `.heatmap` and `.usage-card > *` fixes it.

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: the new-session column and the usage card shrink with the window (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: A resize command for the harness, and the check in the real window

**Files:**
- Modify: `scripts/harness/geometry.py`, `scripts/harness/test_geometry.py`, `scripts/harness/README.md`
- Create: `scripts/harness/resize.py`

**Interfaces:**
- Produces: `geometry.resize_script(pid, width, height) -> str`; `python3 scripts/harness/resize.py WIDTH HEIGHT`.

- [ ] **Step 1: Failing test**

In `test_geometry.py` add `resize_script` to the import and:

```python
class ResizeScriptTest(unittest.TestCase):
    def test_it_sets_the_size_of_the_window_of_that_process_only(self):
        script = resize_script(4242, 900, 800)
        self.assertIn("unix id is 4242", script)
        self.assertIn("set size of window 1", script)
        self.assertIn("{900, 800}", script)
```

Run: `python3 -m unittest discover -s scripts/harness` — expected: ImportError.

- [ ] **Step 2: Implement**

Append to `geometry.py`:

```python
def resize_script(pid, width, height):
    """AppleScript that sets the size of the first window of the process with this pid, in points."""
    return (
        'tell application "System Events" to set size of window 1 of '
        f"(first process whose unix id is {int(pid)}) to {{{int(width)}, {int(height)}}}"
    )
```

Create `scripts/harness/resize.py`:

```python
#!/usr/bin/env python3
"""resize.py <width> <height>: set the size of the Forge window in points, then print its new bounds."""
import subprocess
import sys
import time

from geometry import resize_script
from window import find_window, harness_pid


def main():
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    pid = harness_pid()
    if pid is None:
        raise SystemExit("the harness app is not running (launch.sh)")
    subprocess.run(["osascript", "-e", resize_script(pid, sys.argv[1], sys.argv[2])], check=True)
    time.sleep(1.0)
    window = find_window()
    print(f"window bounds in points (x, y, w, h): {window['bounds']}" if window else "no Forge window")


if __name__ == "__main__":
    main()
```

Add one line to the usage block of `README.md`: `python3 scripts/harness/resize.py W H  # set the window size in points (Accessibility needed), then capture`.

Run the unittest again — expected: PASS. Commit:

```bash
git add scripts/harness
git commit -m "feat: harness resize command (Plan 26)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 3: Check in the real window**

`scripts/harness/launch.sh`, then for each row `python3 scripts/harness/resize.py W 800`, `python3 scripts/harness/capture.py`, and Read `/private/tmp/forge-harness-shot.png` (a Retina capture has 2 pixels per point). Compare with the round 10 numbers. Fix what differs (CSS only unless a test shows a logic fault), one commit per fix.

| Width | New-session screen | A session |
| --- | --- | --- |
| 1500 | sidebar docked; column centred, about 770 pt; card 480 pt | normal |
| 900 | sidebar docked; margin 25 pt; card 480 pt | open Changes: panel floats (window under 1060 pt with 480 pt panel), chat keeps its width |
| 750 | card about 420 pt, heatmap cells smaller | header compact if the chat is under 560 |
| 700 | no sidebar; `☰` at the top left; column left aligned, 25 pt margin | `☰`, title, `⋮` |
| 600 | the composer's rows all fit; no sideways scroll | view switch on a second row; `⋮` lists Terminal, Changes, Preview |

Also check (click through, then capture):
1. At 650: click `☰` — the 280 pt sidebar overlays the pane (not pushed, not dimmed). Click a session — the overlay closes and the session shows. Open it again and press Escape — it closes. Click on the pane outside it — it closes.
2. Open it at 650, resize to 900, resize back to 650: it is closed.
3. At 600 open the search palette (⌘K), Settings, a session row menu and the `⋮` menu: each stays inside the window and its backdrop covers the whole window (click outside closes it).
4. With a session open at 1200, expand the Changes panel: it fills the pane (does not float); collapse it again.
5. Height: resize to 600 x 400 and look at a session: the header and the composer are both visible. If not, raise the minimum height in `main.rs` and write the number into the spec.
6. At 701 pt with a saved sidebar width of 480 (drag the handle to the right before shrinking) and Changes open: the panel floats and the chat is not crushed.

`scripts/harness/stop.sh` at the end.

---

### Task 8: Roadmap and spec bookkeeping

**Files:**
- Modify: `docs/superpowers/specs/2026-10-07-responsive-layout-design.md`
- Modify: `docs/superpowers/roadmap/backlog.md`, `docs/superpowers/roadmap/README.md`

- [ ] **Step 1: Amend the spec's Units to what was built**

The plan decided differently from the first spec text in these places; change the spec to match (and add an "As built" section with the Task 7 numbers):
- the sidebar overlay threshold is `SIDEBAR_OVERLAY_UP_TO = 700` (overlay up to and including 700);
- the toggle is a floating `☰` button at the top left of the window (not a rail, not in the header), and the pane's header and greeting get extra left padding in overlay mode;
- the header is switched by the `chat-narrow` class from the width signal (not by CSS container queries: layout containment would turn the header into the containing block of fixed-position menu backdrops);
- Forge's header has no folder tag; the three icons go into the `⋮` menu and the view switch wraps to a second row;
- when the dock floats, the whole row of its columns floats as one card (not only the last column), each column at most 400 pt;
- `layout_for` also returns `chat_width`; a docked rail counts as 36 pt.

- [ ] **Step 2: Backlog and README**

Add a row to `backlog.md` under a new heading `## UI: window behaviour` with id `UI-01`: "Responsive layout: sidebar overlay, dock float, compact header, shrinking home card", Seen `seen` (round 10), Forge `has`, size M, Plan `Plan 26 (built)`, and the not-built notes (Settings, Files panel, window height, several columns). Add to the README plan-order table: `| 26 | Responsive layout (built) | UI-01 | The user asked for it before any new feature |`.

- [ ] **Step 3: Final run and commit**

Run `cargo test --workspace` and `python3 -m unittest discover -s scripts/harness` — expected: PASS.

```bash
git add docs
git commit -m "docs: Plan 26 as built, roadmap rows

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
