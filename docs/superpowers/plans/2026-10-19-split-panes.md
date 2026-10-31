# Split Panes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forge shows two sessions side by side (⌥+click on a sidebar row), each pane with its own header, messages, composer and panels, a draggable divider, and the Preview web view in the focused pane only.

**Architecture:** A pure `Panes` value (1 or 2 targets, a focus index, a ratio) replaces the single `active` signal in `Workspace`. `Workspace::active()` returns the focused pane's target, so shortcuts, rename, fork, archive, history and the title menu keep acting on the focused pane unchanged. `WorkspacePage` draws one `Pane` component per target; each pane gets its own responsive classes computed from its width (pure functions in `forge-domain`). Document-level JS selectors are scoped to `.pane.focused` or to the owning pane (`[data-pane="N"]`).

**Tech Stack:** Rust, Dioxus 0.7 (webview), wry (native Preview view), the `tr!` i18n table, the `scripts/harness` real-window tools.

**Spec:** `docs/superpowers/specs/2026-10-19-split-panes-design.md`

## Global Constraints

- Horizontal two-way split only; never more than two panes; split state is not saved (one pane after a restart).
- Each pane keeps at least `MIN_PANE_WIDTH = 420` px; with a main area under `2 * 420 = 840` px, ⌥+click does the plain `select`.
- A plain sidebar click replaces the focused pane's target; it never opens a third pane.
- A target (a session, New session, Customize, Routines) is never shown in both panes: opening one that is already shown only moves focus there.
- The native Preview view is drawn only in the focused pane; the other pane's Preview panel keeps its tabs and shows `Click to show the page` in place of the surface.
- Every new user-visible string goes through `tr!` and gets a Korean entry in `crates/forge-shared/src/ko.rs`, which stays sorted in byte order (a test checks it).
- With one pane nothing may look or behave differently from today.
- Match the surrounding code: short doc comments that say why, no new dependencies.

## Review Focus

1. A session closed (⌘W, archive-and-close, closing from the sidebar menu) while shown in the **unfocused** pane: that pane closes, no pane is left pointing at a closed id. Test in Task 1 (`remove_target`) and Task 3.
2. A window too narrow for two panes (main area under 840 px): ⌥+click acts as a plain click; a window shrunk while split keeps both panes (compact headers) and does not panic on a negative width. Test in Task 1 (`open_beside` without room, `widths` with a tiny area).
3. The same session opened twice (⌥+click on a row already shown, or Back to a screen shown in the other pane): focus moves, nothing duplicates, history does not loop. Test in Task 1 (`show`, `open_beside` duplicates).
4. A session started from a New-session pane that is **not** focused when the first reply arrives: the pane that held New session becomes the session, the focused pane is untouched. Test in Task 1 (`replace`), wired in Task 3.
5. File-link clicks, the scroll-to-end button, composer refocus and the Preview address focus act on the right pane, not the first `.messages` in the document. Checked in the real window in Task 7 (no unit test can see the DOM).

---

## File Structure

- Create `crates/forge-features/src/panes.rs`: the pure `Panes` type, `MIN_PANE_WIDTH`, `DEFAULT_RATIO`, tests.
- Create `crates/forge-features/src/pane_context.rs`: `PaneIndex` context, `use_pane_index()`, `use_pane_focused()`.
- Modify `crates/forge-features/src/lib.rs`: `mod panes; mod pane_context;` and re-exports.
- Modify `crates/forge-features/src/workspace.rs`: `panes` signal instead of `active`, new methods, close/start_new/open_past wiring.
- Modify `crates/forge-features/src/shortcut.rs`: `Shortcut::NewSessionBeside`, `cmd_ctrl`, the `wanted` fix, table row, tests.
- Modify `crates/forge-features/src/refocus.rs`: composer selector scoped to the focused pane.
- Modify `crates/forge-domain/src/layout.rs`: `pane_class`, `workspace_class` without the pane-level classes, tests.
- Modify `crates/forge-entities/src/sidebar_item.rs`: `shown` prop (soft highlight).
- Modify `crates/forge-widgets/src/session_row.rs`, `sidebar.rs`: ⌥+click, `shown`.
- Modify `crates/forge-pages/src/workspace_page.rs`: the `Pane` loop, divider, main width.
- Create `crates/forge-widgets/src/pane_close.rs` (`PaneClose` button) and `crates/forge-widgets/src/file_link_watcher.rs`; modify `lib.rs` there.
- Modify `crates/forge-widgets/src/{chat_window,session_tools,preview_panel,web_view_host,files_panel,changes_stack}.rs`: pane-scoped scripts, menu items, idle Preview body.
- Modify `crates/forge-shared/src/{theme.css,ko.rs}`.
- Modify `scripts/harness/click.py` (`--option`), `docs/superpowers/roadmap/backlog.md`, the spec (two small notes, Task 7).

Commands used throughout (run from the repository root of this worktree):
- Unit tests of one crate: `cargo test -p forge-features panes`
- Whole workspace: `cargo test --workspace`
- Type check: `cargo check --workspace`

---

### Task 1: The `Panes` type (pure rules, tested first)

**Files:**
- Create: `crates/forge-features/src/panes.rs`
- Modify: `crates/forge-features/src/lib.rs` (add `mod panes;` and `pub use panes::{Panes, DEFAULT_RATIO, MIN_PANE_WIDTH};`)

**Interfaces:**
- Consumes: `crate::workspace::Active` (`Copy + PartialEq`).
- Produces (later tasks rely on these exact signatures):
  - `pub const MIN_PANE_WIDTH: u32 = 420;` `pub const DEFAULT_RATIO: f32 = 0.5;`
  - `Panes::new(first: Active) -> Panes`
  - `len(&self) -> usize`, `is_split(&self) -> bool`, `focused(&self) -> usize`, `focused_target(&self) -> Active`, `target(&self, index: usize) -> Option<Active>`, `targets(&self) -> &[Active]`, `pane_of(&self, target: Active) -> Option<usize>`, `ratio(&self) -> f32`
  - `show(&mut self, target: Active)`
  - `open_beside(&mut self, target: Active, room: bool)`
  - `replace(&mut self, from: Active, to: Active) -> bool`
  - `remove_target(&mut self, target: Active) -> bool` (true when a pane was closed because of it)
  - `close(&mut self, index: usize)`, `swap(&mut self)`, `focus(&mut self, index: usize) -> bool` (true when focus changed)
  - `set_ratio(&mut self, ratio: f32, area: u32)`
  - `widths(&self, area: u32) -> (u32, u32)` (`(area, 0)` when not split)
  - `can_split(area: u32) -> bool` (associated fn)

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/panes.rs` with only the tests module first (the type does not exist, so it fails to compile — that is the expected failure):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::Active;
    use forge_application::LiveSessionId;

    fn live(n: u64) -> Active {
        Active::Live(LiveSessionId::for_test(n))
    }

    #[test]
    fn one_pane_shows_its_target_and_is_not_split() {
        let panes = Panes::new(Active::NewSession);
        assert_eq!(panes.len(), 1);
        assert!(!panes.is_split());
        assert_eq!(panes.focused_target(), Active::NewSession);
        assert_eq!(panes.widths(1000), (1000, 0));
    }

    #[test]
    fn show_replaces_the_focused_pane_and_never_makes_a_third() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!((panes.len(), panes.focused()), (2, 1));
        panes.show(live(3));
        assert_eq!(panes.targets(), &[live(1), live(3)]);
        panes.focus(0);
        panes.show(live(4));
        assert_eq!(panes.targets(), &[live(4), live(3)]);
    }

    #[test]
    fn showing_a_target_already_in_the_other_pane_only_moves_focus() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.show(live(1));
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 0);
    }

    #[test]
    fn open_beside_adds_a_second_pane_on_the_right_and_focuses_it() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 1);
    }

    #[test]
    fn open_beside_while_split_replaces_the_right_pane_even_when_the_left_is_focused() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.focus(0);
        panes.open_beside(live(3), true);
        assert_eq!(panes.targets(), &[live(1), live(3)]);
        assert_eq!(panes.focused(), 1);
    }

    #[test]
    fn open_beside_a_target_that_is_shown_only_focuses_it() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.open_beside(live(1), true);
        assert_eq!(panes.targets(), &[live(1), live(2)]);
        assert_eq!(panes.focused(), 0);
        // One pane, and the target is the one shown: still one pane.
        let mut single = Panes::new(live(1));
        single.open_beside(live(1), true);
        assert_eq!(single.len(), 1);
    }

    #[test]
    fn open_beside_without_room_does_a_plain_show() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), false);
        assert_eq!(panes.targets(), &[live(2)]);
        assert!(!panes.is_split());
    }

    #[test]
    fn closing_a_pane_gives_its_space_to_the_other_and_resets_the_ratio() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.7, 2000);
        panes.close(0);
        assert_eq!(panes.targets(), &[live(2)]);
        assert_eq!(panes.focused(), 0);
        assert_eq!(panes.ratio(), DEFAULT_RATIO);
        // Closing the only pane does nothing.
        panes.close(0);
        assert_eq!(panes.len(), 1);
    }

    #[test]
    fn swap_exchanges_the_panes_and_focus_follows_the_content() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.4, 2000);
        panes.swap();
        assert_eq!(panes.targets(), &[live(2), live(1)]);
        assert_eq!(panes.focused(), 0);
        assert!((panes.ratio() - 0.6).abs() < 1e-6);
        // Not split: nothing happens.
        let mut single = Panes::new(live(1));
        single.swap();
        assert_eq!(single.targets(), &[live(1)]);
    }

    #[test]
    fn the_ratio_keeps_both_panes_at_least_the_minimum_width() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.set_ratio(0.01, 1000);
        assert_eq!(panes.widths(1000), (MIN_PANE_WIDTH, 1000 - MIN_PANE_WIDTH));
        panes.set_ratio(0.99, 1000);
        assert_eq!(panes.widths(1000), (1000 - MIN_PANE_WIDTH, MIN_PANE_WIDTH));
        panes.set_ratio(0.5, 1000);
        assert_eq!(panes.widths(1000), (500, 500));
    }

    #[test]
    fn a_tiny_area_splits_in_half_and_never_underflows() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        assert_eq!(panes.widths(600), (300, 300));
        assert_eq!(panes.widths(0), (0, 0));
        panes.set_ratio(0.9, 600);
        assert_eq!(panes.widths(600), (300, 300));
    }

    #[test]
    fn can_split_needs_room_for_two_minimum_panes() {
        assert!(!Panes::can_split(2 * MIN_PANE_WIDTH - 1));
        assert!(Panes::can_split(2 * MIN_PANE_WIDTH));
    }

    #[test]
    fn replace_turns_the_new_session_pane_into_the_started_session_wherever_focus_is() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(Active::NewSession, true);
        panes.focus(0);
        assert!(panes.replace(Active::NewSession, live(9)));
        assert_eq!(panes.targets(), &[live(1), live(9)]);
        assert_eq!(panes.focused(), 0, "focus stays where it was");
        assert!(!panes.replace(Active::NewSession, live(10)), "no New session pane is left");
    }

    #[test]
    fn remove_target_closes_the_pane_that_shows_it_when_split_and_does_nothing_otherwise() {
        let mut panes = Panes::new(live(1));
        panes.open_beside(live(2), true);
        panes.focus(1);
        assert!(panes.remove_target(live(1)));
        assert_eq!(panes.targets(), &[live(2)]);
        assert_eq!(panes.focused(), 0);
        // Single pane: the caller decides what to show instead.
        assert!(!panes.remove_target(live(2)));
        assert_eq!(panes.targets(), &[live(2)]);
        // A target that is not shown.
        assert!(!panes.remove_target(live(7)));
    }

    #[test]
    fn focus_reports_whether_it_changed_and_ignores_an_index_that_is_not_there() {
        let mut panes = Panes::new(live(1));
        assert!(!panes.focus(1));
        panes.open_beside(live(2), true);
        assert!(!panes.focus(1));
        assert!(panes.focus(0));
    }
}
```

`LiveSessionId::for_test` may not exist. Check with `grep -n "impl LiveSessionId" -A12 crates/forge-application/src/*.rs`. If the id has no public constructor, build the ids the way other tests in `crates/forge-features/src/workspace.rs`/`preview_host.rs` do (grep `LiveSessionId::` in those test modules) and use that helper in `live()` instead.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-features panes`
Expected: compile error, `cannot find type Panes` (the module is not declared yet) — declare `mod panes;` in `lib.rs` first, then the error names `Panes`.

- [ ] **Step 3: Write the implementation** (above the tests in the same file)

```rust
//! Which screens the main area shows: one pane, or two side by side. Pure rules, no Dioxus, so the
//! rules for opening, closing and focusing are tested without a window.
use crate::workspace::Active;

/// A pane never gets narrower than this when the main area has room for two.
pub const MIN_PANE_WIDTH: u32 = 420;
pub const DEFAULT_RATIO: f32 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub struct Panes {
    /// One or two screens, left to right.
    targets: Vec<Active>,
    focused: usize,
    /// The left pane's share of the main area, when split.
    ratio: f32,
}

impl Panes {
    pub fn new(first: Active) -> Self {
        Self { targets: vec![first], focused: 0, ratio: DEFAULT_RATIO }
    }

    pub fn len(&self) -> usize {
        self.targets.len()
    }

    pub fn is_split(&self) -> bool {
        self.targets.len() == 2
    }

    pub fn focused(&self) -> usize {
        self.focused
    }

    pub fn focused_target(&self) -> Active {
        self.targets[self.focused]
    }

    pub fn target(&self, index: usize) -> Option<Active> {
        self.targets.get(index).copied()
    }

    pub fn targets(&self) -> &[Active] {
        &self.targets
    }

    pub fn pane_of(&self, target: Active) -> Option<usize> {
        self.targets.iter().position(|&shown| shown == target)
    }

    pub fn ratio(&self) -> f32 {
        self.ratio
    }

    pub fn can_split(area: u32) -> bool {
        area >= 2 * MIN_PANE_WIDTH
    }

    /// Puts `target` in the focused pane. A target the other pane already shows takes the focus
    /// instead, so two panes never show the same screen (two New-session panes would share one
    /// unsent text).
    pub fn show(&mut self, target: Active) {
        match self.pane_of(target) {
            Some(index) => self.focused = index,
            None => self.targets[self.focused] = target,
        }
    }

    /// ⌥+click: `target` goes in the right pane, which is added when there is only one and `room`
    /// says the main area can hold two. Without room it is a plain `show`.
    pub fn open_beside(&mut self, target: Active, room: bool) {
        if let Some(index) = self.pane_of(target) {
            self.focused = index;
            return;
        }
        if self.is_split() {
            self.targets[1] = target;
            self.focused = 1;
        } else if room {
            self.targets.push(target);
            self.focused = 1;
        } else {
            self.show(target);
        }
    }

    /// A New-session pane whose first message started a session becomes that session. Focus stays
    /// where it was. False when `from` is not shown.
    pub fn replace(&mut self, from: Active, to: Active) -> bool {
        match self.pane_of(from) {
            Some(index) => {
                self.targets[index] = to;
                true
            }
            None => false,
        }
    }

    /// A session that was closed. With two panes the one that shows it closes; with one the caller
    /// picks what to show. True when a pane was closed.
    pub fn remove_target(&mut self, target: Active) -> bool {
        match self.pane_of(target) {
            Some(index) if self.is_split() => {
                self.close(index);
                true
            }
            _ => false,
        }
    }

    pub fn close(&mut self, index: usize) {
        if !self.is_split() || index >= self.targets.len() {
            return;
        }
        self.targets.remove(index);
        self.focused = 0;
        self.ratio = DEFAULT_RATIO;
    }

    /// "Move split view left": the two panes trade places and focus stays on the same content.
    pub fn swap(&mut self) {
        if self.is_split() {
            self.targets.swap(0, 1);
            self.focused = 1 - self.focused;
            self.ratio = 1.0 - self.ratio;
        }
    }

    /// True when the focus moved.
    pub fn focus(&mut self, index: usize) -> bool {
        if index < self.targets.len() && index != self.focused {
            self.focused = index;
            true
        } else {
            false
        }
    }

    pub fn set_ratio(&mut self, ratio: f32, area: u32) {
        self.ratio = clamp_ratio(ratio, area);
    }

    /// The left and right widths in px for a main area `area` wide; `(area, 0)` with one pane.
    pub fn widths(&self, area: u32) -> (u32, u32) {
        if !self.is_split() {
            return (area, 0);
        }
        let left = ((area as f32) * clamp_ratio(self.ratio, area)).round() as u32;
        let left = left.min(area);
        (left, area - left)
    }
}

// Both panes keep `MIN_PANE_WIDTH`; an area too small for that splits in half.
fn clamp_ratio(ratio: f32, area: u32) -> f32 {
    if !Panes::can_split(area) {
        return DEFAULT_RATIO;
    }
    let low = MIN_PANE_WIDTH as f32 / area as f32;
    ratio.clamp(low, 1.0 - low)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features panes`
Expected: all tests in `panes::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features/src/panes.rs crates/forge-features/src/lib.rs
git commit -m "feat: the pure rules for one or two panes (OTH-04)"
```
(End the commit message with the attribution line the session's system reminder gives.)

---

### Task 2: Per-pane responsive classes (pure, `forge-domain`)

**Files:**
- Modify: `crates/forge-domain/src/layout.rs`

**Interfaces:**
- Consumes: `MIN_CHAT_WIDTH`, `NARROW_CHAT_BELOW`, `Layout`.
- Produces: `pub fn pane_class(pane_width: u32, dock_width: u32) -> String` returning `"pane"` plus ` dock-float` and/or ` chat-narrow` (in that order, as `workspace_class` did); `workspace_class(layout, resizing)` now returns only `workspace`, ` resizing`, ` sidebar-overlay`. Also re-export `pane_class` from `crates/forge-domain/src/lib.rs` the way `workspace_class` is.

- [ ] **Step 1: Write the failing tests**

In `layout.rs` tests, replace the two tests that expected `chat-narrow`/`dock-float` on the workspace (`the_class_list_names_each_mode_that_is_on`, `the_header_is_narrow_under_560`) with:

```rust
    #[test]
    fn the_workspace_class_names_the_sidebar_mode_and_the_drag() {
        assert_eq!(workspace_class(layout_for(1500, 264, 0), false), "workspace");
        assert_eq!(workspace_class(layout_for(1500, 264, 0), true), "workspace resizing");
        assert_eq!(workspace_class(layout_for(650, 264, 350), false), "workspace sidebar-overlay");
    }

    #[test]
    fn a_pane_names_each_mode_that_is_on() {
        assert_eq!(pane_class(1236, 0), "pane");
        // 650 wide with a 350 dock still pushes (the chat keeps exactly 300) and is narrow.
        assert_eq!(pane_class(650, 350), "pane chat-narrow");
        // A 480 dock would leave 240, so it floats and the chat keeps the whole 1000.
        assert_eq!(pane_class(1000, 480), "pane dock-float");
        // Floating over a 601 wide pane: the chat has all 601, which is not narrow.
        assert_eq!(pane_class(601, 480), "pane dock-float");
    }

    #[test]
    fn the_header_is_narrow_under_560() {
        assert_eq!(pane_class(559, 0), "pane chat-narrow");
        assert_eq!(pane_class(560, 0), "pane");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-domain layout`
Expected: FAIL (`pane_class` not found).

- [ ] **Step 3: Implement**

In `layout.rs` change `workspace_class` to drop the last two `if` blocks (dock float, chat narrow), and add:

```rust
/// The class list of one pane: the responsive rules that depend on the pane's own width. With one
/// pane `pane_width` is the whole main area, so this is what the workspace used to say.
pub fn pane_class(pane_width: u32, dock_width: u32) -> String {
    let pushed = pane_width.saturating_sub(dock_width);
    let floats = dock_width > 0 && pushed < MIN_CHAT_WIDTH;
    let chat_width = if floats { pane_width } else { pushed };
    let mut class = String::from("pane");
    if floats {
        class.push_str(" dock-float");
    }
    if chat_width < NARROW_CHAT_BELOW {
        class.push_str(" chat-narrow");
    }
    class
}
```
Add `pane_class` to the `pub use layout::{…}` line in `crates/forge-domain/src/lib.rs`.

Check that `layout_for(...).dock` / `.chat_width` are still used elsewhere (`grep -rn "\.chat_width\|\.dock ==" crates`); leave them if used, keep the struct as is.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-domain`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain
git commit -m "feat: pane_class gives each pane its own narrow and floating-dock classes (OTH-04)"
```

---

### Task 3: `Workspace` holds `Panes`

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (the `active` signal at line ~108 and its init ~177; `active()` ~230; `display` ~1480; `close` ~1676; `open_past` ~1561; `start_new` ~1578; `context_project`, `sync_web_views` need no change)
- Create: `crates/forge-features/src/pane_context.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `Panes` (Task 1).
- Produces on `Workspace` (all `&self`):
  - `panes(&self) -> Panes` (a clone of the signal's value; subscribes the reader)
  - `focused_pane(&self) -> usize`
  - `focus_pane(&self, index: usize)` (writes only when the focus changes)
  - `open_session_beside(&self, id: LiveSessionId)`
  - `open_new_session_beside(&self)`
  - `open_past_beside(&self, project: ProjectSummary, session_id: SessionId)`
  - `close_pane(&self, index: usize)`
  - `swap_panes(&self)`
  - `set_pane_ratio(&self, ratio: f32)`
  - `set_main_width(&self, width: u32)` / `main_width(&self) -> u32`
  - `pane_widths(&self) -> (u32, u32)`
- Produces in `pane_context.rs`: `pub struct PaneIndex(pub Signal<usize>)`, `pub fn use_pane_index() -> usize` (0 when there is no provider), `pub fn use_pane_focused() -> bool`.
- Existing `active()`, `select`, `show_new_session`, `go_back/go_forward`, `navigate`, `display` keep their signatures.

There is no Dioxus runtime in the unit tests of `Workspace`, so its new methods stay thin wrappers over `Panes` (tested in Task 1). Verification here is the type check, the existing test suite and Task 7's real window.

- [ ] **Step 1: Replace the signal**

In the `Workspace` struct replace `active: Signal<Active>,` by `panes: Signal<Panes>,` and add `main_width: Signal<u32>,`. In `use_workspace_provider` initialise `panes: Signal::new(Panes::new(Active::NewSession))` and `main_width: Signal::new(1200)`. Change:

```rust
    pub fn active(&self) -> Active {
        self.panes.read().focused_target()
    }
```
and in `display`:
```rust
        let mut panes = self.panes;
        panes.write().show(to);
```
(replacing the `active.set(to)` lines; keep `close_lightbox()` and `keep_open`).

Run `cargo check -p forge-features` and fix any other use of `self.active` the compiler finds.

- [ ] **Step 2: Add the methods** (next to `select`)

```rust
    pub fn panes(&self) -> Panes {
        self.panes.read().clone()
    }

    pub fn focused_pane(&self) -> usize {
        self.panes.read().focused()
    }

    /// A click or focus inside a pane. Writes only when the focus moves, so a click in the focused
    /// pane redraws nothing.
    pub fn focus_pane(&self, index: usize) {
        if self.panes.peek().focused() == index {
            return;
        }
        let mut panes = self.panes;
        if panes.write().focus(index) {
            self.clear_rename_request();
        }
    }

    /// ⌥+click on an open session: it opens in the right pane (or takes focus if it is shown).
    pub fn open_session_beside(&self, id: LiveSessionId) {
        self.clear_rename_request();
        self.open_beside(Active::Live(id));
    }

    /// `⌃⌘\`: a New-session pane beside the focused one.
    pub fn open_new_session_beside(&self) {
        self.clear_rename_request();
        self.open_beside(Active::NewSession);
    }

    fn open_beside(&self, target: Active) {
        let room = Panes::can_split(self.main_width());
        let mut panes = self.panes;
        panes.write().open_beside(target, room);
        self.after_pane_change(target);
    }

    // What `display` does for the screen that now has the focus: the lightbox goes, a routine's
    // session stays open, and the history records the screen (so Back returns to it).
    fn after_pane_change(&self, target: Active) {
        self.close_lightbox();
        if let Active::Live(id) = target {
            self.services.peek().routines.host.keep_open(id);
        }
        let mut history = self.history;
        history.write().visit(target);
    }

    pub fn close_pane(&self, index: usize) {
        let mut panes = self.panes;
        panes.write().close(index);
    }

    pub fn swap_panes(&self) {
        let mut panes = self.panes;
        panes.write().swap();
    }

    pub fn set_pane_ratio(&self, ratio: f32) {
        let area = self.main_width();
        let mut panes = self.panes;
        panes.write().set_ratio(ratio, area);
    }

    /// The width of the area right of the docked sidebar; the page reports it.
    pub fn set_main_width(&self, width: u32) {
        if *self.main_width.peek() != width {
            let mut slot = self.main_width;
            slot.set(width);
        }
    }

    pub fn main_width(&self) -> u32 {
        *self.main_width.peek()
    }

    /// The left and right pane widths for the current main area (reads, so a drag redraws).
    pub fn pane_widths(&self) -> (u32, u32) {
        let area = *self.main_width.read();
        self.panes.read().widths(area)
    }
```

- [ ] **Step 3: Past sessions beside**

Rename the body of `open_past` into `fn open_past_in(&self, project, session_id, beside: bool)` and make `open_past` call it with `false`; add `pub fn open_past_beside(&self, project, session_id) { self.open_past_in(project, session_id, true) }`. In the spawned task replace `this.select(id);` by:

```rust
                    if beside {
                        this.open_session_beside(id);
                    } else {
                        this.select(id);
                    }
```

- [ ] **Step 4: A session that ends while shown in a pane**

In `close` (the `if self.active() == Active::Live(id)` block near line 1676) replace the block by:

```rust
        let mut panes = self.panes;
        // The pane that shows it closes when there are two; with one pane the next open session (or
        // the New-session page) takes its place, as before.
        if !panes.write().remove_target(Active::Live(id)) && self.active() == Active::Live(id) {
            let next = self.live.peek().last().map(|entry| entry.handle.id());
            match next {
                Some(next) => self.select(next),
                None => self.show_new_session(),
            }
        }
```
If another path removes sessions (grep `fn archive_session`, `close_remembering`), make sure they go through `close`; `archive_active`/`close_active` already do.

- [ ] **Step 5: A started session takes the New-session pane**

In `start_new`, find where the spawned task calls `select(…)` for the new session (`grep -n "select(" crates/forge-features/src/workspace.rs` between lines 1578 and 1700). Replace that call by `this.show_started(id)` and add:

```rust
    // The first message of a New-session pane turns that very pane into the session, even if the
    // person has clicked into the other pane since.
    fn show_started(&self, id: LiveSessionId) {
        let mut panes = self.panes;
        if panes.write().replace(Active::NewSession, Active::Live(id)) {
            self.after_pane_change(Active::Live(id));
        } else {
            self.select(id);
        }
    }
```
(`replace` keeps focus where it was, so `after_pane_change` records history for a screen that may not be the focused one; to avoid that, call `after_pane_change` only `if panes.read().focused_target() == Active::Live(id)`.)

- [ ] **Step 6: Pane context**

Create `crates/forge-features/src/pane_context.rs`:

```rust
use dioxus::prelude::*;

use crate::use_workspace;

/// Which pane a component is drawn in. A signal, because closing the left pane makes the right one
/// pane 0 without remounting it.
#[derive(Clone, Copy)]
pub struct PaneIndex(pub Signal<usize>);

/// The index of the pane this component is in; 0 outside any pane.
pub fn use_pane_index() -> usize {
    try_use_context::<PaneIndex>().map(|index| *index.0.read()).unwrap_or(0)
}

/// Whether the pane this component is in has the focus. True outside any pane.
pub fn use_pane_focused() -> bool {
    let workspace = use_workspace();
    match try_use_context::<PaneIndex>() {
        Some(index) => workspace.focused_pane() == *index.0.read(),
        None => true,
    }
}
```
Declare it in `lib.rs` and re-export `PaneIndex, use_pane_index, use_pane_focused, Panes, MIN_PANE_WIDTH`.

- [ ] **Step 7: Verify**

Run: `cargo check --workspace && cargo test -p forge-features`
Expected: compiles; all existing tests still pass.

- [ ] **Step 8: Commit**

```bash
git add crates/forge-features
git commit -m "feat: the workspace shows one or two panes; active() is the focused pane (OTH-04)"
```

---

### Task 4: The `⌃⌘\` shortcut and its strings

**Files:**
- Modify: `crates/forge-features/src/shortcut.rs`
- Modify: `crates/forge-features/src/workspace.rs` (the `on_key` match, ~line 1857)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Produces: `Shortcut::NewSessionBeside`, bound to `⌃⌘\` on a Mac and `Ctrl+\` elsewhere; row label `msg!("New session beside this one")`.

- [ ] **Step 1: Write the failing tests** in `shortcut.rs`'s test module (follow the style of the existing binding tests; find one with `grep -n "fn .*binding\|display(true)" crates/forge-features/src/shortcut.rs`):

```rust
    #[test]
    fn the_new_session_beside_key_is_control_command_backslash_on_a_mac_and_control_backslash_elsewhere() {
        let row = SHORTCUTS.iter().find(|row| row.action == Shortcut::NewSessionBeside).expect("listed");
        assert_eq!(row.keys[0].display(true), "⌃⌘\\");
        assert_eq!(row.keys[0].display(false), "Ctrl+\\");
        let mac = KeyPress { code: "Backslash", ctrl: true, meta: true, alt: false, shift: false };
        assert!(row.keys[0].modifiers_match(&mac, true));
        // ⌘\ alone, and ⌘⇧\ (Expand the panel), are different keys.
        let command_only = KeyPress { meta: true, ctrl: false, ..mac };
        assert!(!row.keys[0].modifiers_match(&command_only, true));
        let windows = KeyPress { code: "Backslash", ctrl: true, meta: false, alt: false, shift: false };
        assert!(row.keys[0].modifiers_match(&windows, false));
    }
```
(`Shortcut` derives `PartialEq`; `modifiers_match` is private to the module, which the tests share.)

- [ ] **Step 2: Run, expect FAIL**

Run: `cargo test -p forge-features shortcut`
Expected: compile error, no `NewSessionBeside`.

- [ ] **Step 3: Implement**

- Add the variant `NewSessionBeside,` after `NewSessionHere` with a doc line `/// Opens a New-session pane beside the focused one.`
- Add `const fn cmd_ctrl(code: &'static str) -> Binding { binding(code, true, true, false, false) }`.
- Fix `Binding::wanted` so a command binding can also ask for Control:
```rust
        if self.command {
            if mac { (true, self.ctrl) } else { (false, true) }
        } else {
            (false, self.ctrl)
        }
```
- Add the row after "New session in this folder":
```rust
    ShortcutRow { group: Group::General, label: msg!("New session beside this one"), action: Shortcut::NewSessionBeside, keys: &[cmd_ctrl("Backslash")] },
```
- In `workspace.rs::on_key` add `Shortcut::NewSessionBeside => self.open_new_session_beside(),` after `NewSessionHere`.
- Fix any exhaustive `match` the compiler reports (`shortcut_allowed`, `swallowed_behind_overlay`, etc.): treat `NewSessionBeside` like `NewSession`.
- `ko.rs`: add `("New session beside this one", "이 세션 옆에 새 세션"),` in byte-sorted position (after `"New session"` entries — check the neighbours; the sort test names the offending line if wrong).

- [ ] **Step 4: Run, expect PASS**

Run: `cargo test -p forge-features shortcut && cargo test -p forge-shared`
Expected: PASS (including the existing `wanted`/display tests and the ko sort test).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features crates/forge-shared
git commit -m "feat: ⌃⌘\\ opens a New-session pane beside the focused one (OTH-04)"
```

---

### Task 5: Drawing the panes, the divider, ⌥+click

**Files:**
- Modify: `crates/forge-pages/src/workspace_page.rs`
- Modify: `crates/forge-entities/src/sidebar_item.rs`, `crates/forge-widgets/src/session_row.rs`, `crates/forge-widgets/src/sidebar.rs`
- Create: `crates/forge-widgets/src/pane_close.rs`; modify `crates/forge-widgets/src/lib.rs` (`mod pane_close; pub use pane_close::PaneClose;`)
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: Task 1–3 APIs, `pane_class` (Task 2).
- Produces: `PaneClose { index: usize }` widget (a `×` button calling `workspace.close_pane(index)`, title `Close split view`); the `shown: bool` prop on `SidebarItem` and `SessionRow`; DOM contract used by Task 6: every pane is `div.pane` with `data-pane="{index}"`, class `focused` on the focused one, `split` on both when split.

- [ ] **Step 1: Sidebar item shows the soft highlight**

`SidebarItem` gets `#[props(default)] shown: bool` and
```rust
    let class = match (active, shown) {
        (true, _) => "sidebar-item active",
        (false, true) => "sidebar-item shown",
        (false, false) => "sidebar-item",
    };
```
`SessionRow` gets `#[props(default)] shown: bool` and passes it through; its `onopen: EventHandler<()>` becomes `EventHandler<bool>` (true = beside), and its `onclick: move |event| { mode.set(RowMode::Normal); onopen.call(event.modifiers().alt()); }` (`event` is a `MouseEvent`; `modifiers()` returns `Modifiers`, `.alt()` is `contains(Modifiers::ALT)` — use `event.modifiers().contains(Modifiers::ALT)` if `.alt()` does not exist in this Dioxus version). Other callers of `SessionRow` (grep) pass `|_|` and keep working.

In `sidebar.rs` the row call becomes:
```rust
                            active: match &item.entry.key {
                                EntryKey::Live(id) => active == Active::Live(*id),
                                EntryKey::Past { .. } => false,
                            },
                            shown: match &item.entry.key {
                                EntryKey::Live(id) => panes.pane_of(Active::Live(*id)).is_some(),
                                EntryKey::Past { .. } => false,
                            },
                            onopen: {
                                let project = item.project.clone();
                                let key = item.entry.key.clone();
                                move |beside: bool| match (&key, beside) {
                                    (EntryKey::Live(id), false) => workspace.select(*id),
                                    (EntryKey::Live(id), true) => workspace.open_session_beside(*id),
                                    (EntryKey::Past { session, .. }, false) => workspace.open_past(project.clone(), session.clone()),
                                    (EntryKey::Past { session, .. }, true) => workspace.open_past_beside(project.clone(), session.clone()),
                                }
                            },
```
with `let panes = workspace.panes();` next to where `active` is read in that component. `active` there is `workspace.active()`; with two panes the focused pane's session is "active" and the other is "shown".

- [ ] **Step 2: `PaneClose`**

```rust
use dioxus::prelude::*;
use forge_features::use_workspace;
use forge_shared::tr;

/// The × on a pane's header while the screen is split.
#[component]
pub fn PaneClose(index: usize) -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "icon-button pane-close",
            r#type: "button",
            title: tr!("Close split view"),
            "aria-label": tr!("Close split view"),
            onclick: move |_| workspace.close_pane(index),
            "×"
        }
    }
}
```

- [ ] **Step 3: The page**

In `workspace_page.rs` add the imports (`pane_class`, `PaneIndex`, `Panes`, `PaneClose`). Replace the `dock_width` and `layout` computation and the `match workspace.active()` as follows.

Compute the main area and keep the workspace in step:

```rust
    let docked_sidebar = if hidden { SIDEBAR_RAIL_WIDTH } else { prefs.clamped_width() };
    let layout = layout_for(window_width(), docked_sidebar, 0);
    let sidebar_mode = layout.sidebar;
    // The area right of the docked sidebar (the whole window with an overlay sidebar).
    let main_area = match sidebar_mode {
        SidebarMode::Docked => window_width().saturating_sub(docked_sidebar),
        SidebarMode::Overlay => window_width(),
    };
    use_effect(use_reactive!(|main_area| workspace.set_main_width(main_area)));
```
(`layout_for(…, 0)` suffices for `workspace_class`: it no longer reads the dock.) The existing rename-request effect keeps working.

Drag state for the divider next to `dragging`:
```rust
    let mut pane_dragging = use_signal(|| false);
```
Extend the root's `onmousemove`/`onmouseup`:

```rust
            onmousemove: move |event| {
                if dragging() {
                    …unchanged…
                }
                if pane_dragging() {
                    let area = main_area.max(1) as f32;
                    let left_edge = window_width().saturating_sub(main_area) as f32;
                    workspace.set_pane_ratio((event.client_coordinates().x as f32 - left_edge) / area);
                }
            },
            onmouseup: move |_| {
                if dragging() { …unchanged… }
                pane_dragging.set(false);
            },
```
and `class: workspace_class(layout, dragging() || pane_dragging())` (the `resizing` class sets `col-resize` and `user-select: none`).

Replace the content match by:

```rust
            {pane_row(workspace, window_width(), main_area, pane_dragging)}
```
with, below the component:

```rust
// The panes and the divider. A pane is keyed by its screen, so closing the left one does not
// remount the right one.
fn pane_row(workspace: Workspace, _window_width: u32, _main_area: u32, mut pane_dragging: Signal<bool>) -> Element {
    let panes = workspace.panes();
    let (left, _right) = workspace.pane_widths();
    let split = panes.is_split();
    rsx! {
        div { class: if split { "panes split" } else { "panes" },
            for (index, target) in panes.targets().iter().copied().enumerate() {
                if index == 1 {
                    div {
                        class: if pane_dragging() { "pane-divider dragging" } else { "pane-divider" },
                        onmousedown: move |event| {
                            event.prevent_default();
                            pane_dragging.set(true);
                        },
                    }
                }
                Pane { key: "{target:?}", index, target, width: if index == 0 { left } else { workspace.pane_widths().1 } }
            }
        }
    }
}
```
(`Workspace` is imported from `forge_features`. `_window_width`/`_main_area` are not needed — drop those parameters; keep `pane_dragging`.)

The pane component:

```rust
/// One screen of the main area. It tells its descendants which pane they are in and moves the
/// focus here on any click or focus inside.
#[component]
fn Pane(index: usize, target: Active, width: u32) -> Element {
    let workspace = use_workspace();
    let mut index_signal = use_context_provider(|| PaneIndex(Signal::new(index)));
    use_effect(use_reactive!(|index| {
        if *index_signal.0.peek() != index {
            index_signal.0.set(index);
        }
    }));
    let panes = workspace.panes();
    let split = panes.is_split();
    let dock_width: u32 = match target {
        Active::Live(id) => {
            let dock = workspace.dock(id);
            if dock.expanded.is_some() { 0 } else { dock.columns.iter().map(|column| column.width as u32).sum() }
        }
        Active::NewSession | Active::Customize | Active::Routines => 0,
    };
    let mut class = pane_class(width, dock_width);
    if split {
        class.push_str(" split");
    }
    if panes.focused() == index {
        class.push_str(" focused");
    }
    let style = if split && index == 0 { format!("width: {width}px; flex: none;") } else { String::new() };
    rsx! {
        div {
            class,
            style,
            "data-pane": "{index}",
            onmousedown: move |_| workspace.focus_pane(index),
            onfocusin: move |_| workspace.focus_pane(index),
            match target {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Customize => rsx! { CustomizePage {} },
                Active::Routines => rsx! { RoutinesPage {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            if split && !matches!(target, Active::Live(_)) {
                PaneClose { index }
            }
        }
    }
}
```
The right pane has no width style: it takes the rest (`flex: 1`), so the two always fill the area. With one pane there is no style and `.pane` is `flex: 1`.

- [ ] **Step 4: CSS** (`theme.css`)

Add near `.workspace` (line 63) and adapt line 507:

```css
.panes { flex: 1; min-width: 0; display: flex; position: relative; }
.pane { flex: 1; min-width: 0; display: flex; position: relative; }
.pane.split.focused { box-shadow: inset 0 0 0 1px var(--accent); }
.pane-divider { flex: none; width: 1px; background: var(--border); position: relative; cursor: col-resize; z-index: 5; }
.pane-divider::before { content: ""; position: absolute; top: 0; bottom: 0; left: -3px; right: -3px; }
.pane-divider.dragging { background: var(--accent); }
.pane-close { position: absolute; top: 10px; right: 10px; z-index: 6; }
.pane.split .pane-header { padding-right: 48px; }
.sidebar-item.shown { background: var(--bg-hover); }
.preview-idle { flex: 1; display: flex; align-items: center; justify-content: center; }
```
Use the variable names the file already has (`grep -n "^\s*--" crates/forge-shared/src/theme.css | head -40`; if `--accent` or `--border` do not exist, use the closest existing ones). Replace the rule at line 507 with:
```css
.sidebar-overlay .pane:first-child .pane-header, .sidebar-overlay .pane:first-child.chat-narrow .pane-header { padding-left: 48px; }
```
and keep `.pane.split .pane-header` ordering such that the padding-right applies with the narrow header too (it sets only the right side). The dock-float rules (`.dock-float .dock`, line ~547) now find `.pane.dock-float` as an ancestor and `.pane { position: relative }`; compare the header offset in the real window in Task 7.

- [ ] **Step 5: Strings**

`ko.rs`: `("Close split view", "분할 보기 닫기"),` (byte-sorted).

- [ ] **Step 6: Verify**

Run: `cargo check --workspace && cargo test --workspace`
Expected: compiles, all tests pass. (Real-window checks are Task 7.)

- [ ] **Step 7: Commit**

```bash
git add crates
git commit -m "feat: two panes with a draggable divider, ⌥+click on a row opens a session beside (OTH-04)"
```

---

### Task 6: Pane-scoped scripts, header menu, idle Preview

**Files:**
- Modify: `crates/forge-widgets/src/web_view_host.rs` (`SURFACE_SCRIPT`, plus its test)
- Create: `crates/forge-widgets/src/file_link_watcher.rs`; modify `chat_window.rs` (remove the `WATCH_FILE_LINKS` future and constant), `crates/forge-widgets/src/lib.rs`, `workspace_page.rs` (render `FileLinkWatcher {}` after `WebViewHost {}`)
- Modify: `crates/forge-widgets/src/chat_window.rs` (`WATCH_SCROLL`, `ScrollToEnd`)
- Modify: `crates/forge-features/src/refocus.rs` (`COMPOSER_TEXTAREA`)
- Modify: `crates/forge-widgets/src/preview_panel.rs` (`focus_address`, idle body)
- Modify: `crates/forge-widgets/src/files_panel.rs` (~line 444), `crates/forge-widgets/src/changes_stack.rs` (`scroll_to_diff`)
- Modify: `crates/forge-widgets/src/session_tools.rs` (menu items and the × button)
- Modify: `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: the DOM contract of Task 5 (`.pane`, `.pane.focused`, `[data-pane="N"]`), `use_pane_index`, `use_pane_focused`.
- Produces: `FileLinkWatcher` component; none for later tasks.

- [ ] **Step 1: Failing tests for the scripts**

Add to the `web_view_host.rs` tests:
```rust
    #[test]
    fn the_surface_is_the_one_in_the_focused_pane() {
        assert!(SURFACE_SCRIPT.contains(".pane.focused .preview-surface"));
        assert!(!SURFACE_SCRIPT.contains("querySelector('.preview-surface')"));
    }
```
and to the `refocus.rs` tests:
```rust
    #[test]
    fn the_composer_selector_is_limited_to_the_focused_pane() {
        assert!(COMPOSER_TEXTAREA.starts_with(".pane.focused .composer textarea"));
        assert!(COMPOSER_TEXTAREA.contains(".pane.focused .new-session .composer-box textarea"));
    }
```
Update the existing `refocus.rs` composer-selector test so it still holds with the new strings (it asserts `.composer textarea` and `.new-session .composer-box textarea` are contained — both remain substrings).

Run: `cargo test -p forge-widgets web_view_host && cargo test -p forge-features refocus` — Expected: FAIL.

- [ ] **Step 2: Surface and composer selectors**

- `SURFACE_SCRIPT`: `document.querySelector('.pane.focused .preview-surface')`.
- `COMPOSER_TEXTAREA`: `".pane.focused .composer textarea, .pane.focused .new-session .composer-box textarea"`.
- `OVERLAY_SCRIPT` and `key_script` stay (one workspace per window).

- [ ] **Step 3: File links, one watcher for the window**

`file_link_watcher.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{use_workspace, Active};

// A click on a file name in an answer: the page sends "<pane index>\n<path>" to Rust. One handler
// serves every pane; it is replaced each time (`dioxus.send` belongs to the eval that created it).
const WATCH_FILE_LINKS: &str = "if (window.__forgeFileLinkHandler) { document.removeEventListener('click', window.__forgeFileLinkHandler); } window.__forgeFileLinkHandler = (event) => { const link = event.target && event.target.closest ? event.target.closest('.file-link') : null; if (link && link.dataset.path) { const pane = link.closest('[data-pane]'); dioxus.send((pane ? pane.dataset.pane : '0') + '\\n' + link.dataset.path); } }; document.addEventListener('click', window.__forgeFileLinkHandler);";

/// Draws nothing. Opens the file a person clicked in the chat of the pane they clicked in.
#[component]
pub fn FileLinkWatcher() -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        let mut links = document::eval(WATCH_FILE_LINKS);
        while let Ok(text) = links.recv::<String>().await {
            let Some((pane, path)) = text.split_once('\n') else { continue };
            let Ok(index) = pane.parse::<usize>() else { continue };
            if let Some(Active::Live(id)) = workspace.panes().target(index) {
                workspace.open_file(id, path.to_string());
            }
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handler_names_the_pane_of_the_link_and_replaces_its_old_self() {
        assert!(WATCH_FILE_LINKS.contains("removeEventListener('click'"));
        assert!(WATCH_FILE_LINKS.contains("closest('[data-pane]')"));
        assert!(WATCH_FILE_LINKS.contains("dataset.pane"));
    }
}
```
Remove the `WATCH_FILE_LINKS` constant and the `use_future` that reads it from `ChatWindow` (and the `open_file_in_active` call there; keep `open_file_in_active` itself if other code uses it — `grep`).

- [ ] **Step 4: Scroll watcher and the end button, per pane**

`WATCH_SCROLL`: loop over every list:

```rust
const WATCH_SCROLL: &str = "if (!window.__forgeScrollWatch) { window.__forgeScrollWatch = setInterval(() => { for (const list of document.querySelectorAll('.messages')) { if (list.dataset.watching) continue; list.dataset.watching = '1'; const pane = list.closest('.main-pane'); const update = () => { const away = list.scrollHeight - list.scrollTop - list.clientHeight > 80; pane?.setAttribute('data-away', away ? 'true' : 'false'); }; list.addEventListener('scroll', update); const resizes = new ResizeObserver(update); resizes.observe(list); const content = list.firstElementChild; if (content) resizes.observe(content); update(); } }, 400); }";
```
`ScrollToEnd` takes the index: `let pane = forge_features::use_pane_index();` and the click script becomes
```rust
document::eval(&format!("const list = document.querySelector('[data-pane=\"{pane}\"] .messages'); list?.scrollTo({{ top: list.scrollHeight, behavior: 'smooth' }});"));
```
(In a `format!`, double the braces as shown.)

- [ ] **Step 5: Preview**

- `focus_address()` script: `document.querySelector('.pane.focused .preview-url')`.
- In `PreviewPanel`, `let focused = forge_features::use_pane_focused();` and render, in place of the `div.preview-surface` branch:

```rust
                } else if !focused {
                    // Only the focused pane has the native view; this one comes back on a click.
                    div { class: "preview-idle", div { class: "dock-note", {tr!("Click to show the page")} } }
                } else {
                    div { class: "preview-surface", … unchanged … }
                }
```
- `ko.rs`: `("Click to show the page", "클릭하면 페이지가 보입니다"),`.

- [ ] **Step 6: Files and Changes jumps**

In `files_panel.rs` (~444) and `changes_stack.rs::scroll_to_diff` change `document.querySelectorAll('[data-path]')` → `document.querySelectorAll('.pane.focused [data-path]')` and `'[data-diff-path]'` → `'.pane.focused [data-diff-path]'`.

- [ ] **Step 7: Header menu and ×**

In `SessionTools` (`session_tools.rs`): `let workspace = use_workspace(); let pane = use_pane_index(); let panes = workspace.panes();`. After the `⋮` picker div, inside `.pane-tools`, when `panes.is_split()` render `PaneClose { index: pane }`. In the `picker-menu`, after the Files item add (only when split):

```rust
                        if panes.is_split() {
                            if pane == 1 {
                                button {
                                    class: "picker-item",
                                    r#type: "button",
                                    role: "menuitem",
                                    onclick: move |_| {
                                        workspace.swap_panes();
                                        open.set(false);
                                    },
                                    span { {tr!("Move split view left")} }
                                }
                            }
                            button {
                                class: "picker-item",
                                r#type: "button",
                                role: "menuitem",
                                onclick: move |_| {
                                    workspace.close_pane(pane);
                                    open.set(false);
                                },
                                span { {tr!("Close split view")} }
                            }
                        }
```
`ko.rs`: `("Move split view left", "분할 화면을 왼쪽으로 이동"),`. `.chat-narrow .pane-tools > .icon-button { display: none; }` (theme.css 577) would hide the × in a narrow pane; add `.chat-narrow .pane-tools > .icon-button.pane-close { display: inline-flex; }` after it. The `pane-close` has `position: absolute` in CSS for the page-level case; inside `.pane-tools` override: `.pane-tools .pane-close { position: static; }`.

- [ ] **Step 8: Verify**

Run: `cargo test --workspace`
Expected: PASS (new script tests included; ko tables consistent).

- [ ] **Step 9: Commit**

```bash
git add crates
git commit -m "feat: scripts, Preview and the header menu follow the pane (OTH-04)"
```

---

### Task 7: Real window, harness ⌥+click, docs

**Files:**
- Modify: `scripts/harness/click.py` (and `geometry.py` only if needed)
- Modify: `docs/superpowers/roadmap/backlog.md` (OTH-04 → built, first step)
- Modify: `docs/superpowers/specs/2026-10-19-split-panes-design.md` (two notes)

- [ ] **Step 1: `--option` for the harness click**

In `click.py` give `click_at` an `option=False` argument and set the flag on the down/up events:

```python
def click_at(x, y, option=False):
    for event_type in (Quartz.kCGEventMouseMoved, Quartz.kCGEventLeftMouseDown, Quartz.kCGEventLeftMouseUp):
        event = Quartz.CGEventCreateMouseEvent(None, event_type, (x, y), Quartz.kCGMouseButtonLeft)
        if option and event_type != Quartz.kCGEventMouseMoved:
            Quartz.CGEventSetFlags(event, Quartz.kCGEventFlagMaskAlternate)
        Quartz.CGEventPost(Quartz.kCGHIDEventTap, event)
        time.sleep(0.05)
```
plus `parser.add_argument("--option", action="store_true")` and `click_at(*point, option=args.option)`. Update the module docstring and `scripts/harness/README.md` with the flag.

- [ ] **Step 2: Launch and check each success line of the spec**

`scripts/harness/launch.sh`, then `python3 scripts/harness/capture.py` and Read the PNG after each action (screenshots may display at 1/1.2; multiply by the factor printed). Check in order and fix any defect at its task before moving on:

1. One pane: identical to before (header, dock, narrow window at 650 pt).
2. `click.py X Y --option` on a second open session's row: it opens at the right; both headers, composers and messages show; the focused pane has the accent outline.
3. Type into each composer; scroll each message list; the `↓` button of one pane scrolls only that pane.
4. Open the Files panel in the right session and the Terminal in the left; each pane keeps its own.
5. Plain click on a third row: replaces the session in the focused pane; still two panes.
6. Click a file name in the left pane's chat: it opens in the **left** pane's Files panel.
7. Preview: open Preview in both sessions with an address; the web view shows only in the focused pane; the other shows `Click to show the page`; clicking it moves the view.
8. Drag the divider both ways: widths follow; neither goes under 420 pt; a narrow pane gets the compact header (icons fold into ⋮).
9. ⋮ → `Move split view left` swaps; `Close split view` and × close; the remaining pane fills the width.
10. `⌃⌘\` (osascript key code 42 with control and command) opens a New-session pane beside; send a first message from it while the other pane is focused: that pane becomes the session.
11. Archive-and-close (⌥⌘A) the session in the focused pane while split: the pane closes. Use the sidebar `⋯` menu on the session in the **other** pane: that pane closes.
12. Shrink the window to 700 pt wide with the sidebar docked: ⌥+click does a plain open.

Dark mode and Korean typing are left to the user; say so in the report.

- [ ] **Step 3: Full test run and clippy-level check**

Run: `cargo test --workspace && cargo check --workspace`
Expected: PASS.

- [ ] **Step 4: Docs**

- `backlog.md`: OTH-04 row → `built: Plan 51, horizontal 2-split (Round 20)`; list under it what is left: vertical stacking, trees, more than two panes, saved split, focus keys, side chat (OTH-05).
- Spec notes: the pane width is computed from the window width, the sidebar width and the ratio (pure, tested) instead of a `ResizeObserver`; the × and the ⋮ items sit in `SessionTools`, with a page-level × for the non-session panes.
- Commit:

```bash
git add scripts docs
git commit -m "docs: split panes built; the harness click takes --option (OTH-04)"
```

- [ ] **Step 5: Hand off** — report which of the 12 checks passed, which were not run, and that the merge into master waits for the user.

---

## Self-review

- **Spec coverage:** state and rules (Task 1, 3), opening by ⌥+click and `⌃⌘\` (Task 4, 5), close/× and move-left (Task 5, 6), sizes and minimum width (Task 1, 2, 5), drawing and per-pane classes (Task 2, 5), scripts and Preview (Task 6), shortcuts/title menu unchanged (none needed), saved state not saved (nothing to do), testing (Task 1, 2, 4, 6 unit; Task 7 window). The spec's "ResizeObserver" is replaced by a pure width computation — recorded in the spec in Task 7.
- **Types:** `Panes` method names (`show`, `open_beside(target, room)`, `replace`, `remove_target`, `close`, `swap`, `focus`, `set_ratio(ratio, area)`, `widths(area)`, `can_split(area)`) match their uses in Tasks 3, 5, 6. `pane_class(pane_width, dock_width)` matches Task 5's call. `PaneIndex(Signal<usize>)` is provided in Task 5 and consumed through `use_pane_index`/`use_pane_focused` in Task 6.
- **Known unknowns to resolve while coding, not placeholders:** the `LiveSessionId` test constructor (Task 1 Step 1 says how to find it), the exact CSS variable names (Task 5 Step 4 says how to look), `MouseEvent::modifiers().alt()` spelling (Task 5 Step 1 gives the fallback).
