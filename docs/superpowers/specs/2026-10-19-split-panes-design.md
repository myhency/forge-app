# Split panes: two sessions side by side (OTH-04, first step)

Date: 2026-10-19. Branch `plan-50d` (a new plan branch is made when the plan is written). Evidence: observation round 20 and its addendum (`docs/superpowers/roadmap/observations.md`, OTH-04).

## Goal

Forge can show two sessions next to each other in the main area, the way the desktop app does. Each pane has its own header, messages, composer and panels, so the person can watch one session while typing in the other.

Success, checked in the real window:
- ⌥+click on a sidebar row opens that session in a second pane at the right. The first pane keeps its session.
- Each pane scrolls, sends and shows its own Files / Changes / Terminal / Preview panels independently.
- A plain click on a sidebar row, while split, replaces the session of the focused pane. It never makes a third pane.
- × on a pane header closes it; the other pane takes the whole width.
- Dragging the divider changes the widths; a pane that gets narrow switches to the compact header.
- With one pane, nothing looks or behaves differently from today.

## Decided with the user (2026-10-19)

- Horizontal two-way split only: no vertical stacking, no more than two panes.
- The native Preview web view is drawn only in the focused pane. The other pane's Preview panel keeps its tabs and addresses and shows an empty body until that pane is clicked.
- A plain sidebar click replaces the focused pane's session.
- Split state is not saved: after a restart there is one pane.
- Side chat (`⌘;`), "move split down", and keys that move focus between panes are not in this step (the last was not observed in the desktop app).

## What the desktop app does (round 20, summary)

⌥+click on a row opens the session in a second pane to the right. `⌃⌘\` opens a new-session pane beside the focused pane. A narrow pane header shows the title with its chevron, ⋮ and ×; a wide pane keeps the toolbar icons. The ⋮ menu has the panel toggles and, side by side, "Move split view left" and "Close split view". The divider drags. Closing a pane gives its space to the neighbour.

## Approach

`Workspace` keeps a list of at most two panes and a focus index. Each pane holds one `Active` value, as today's single `active` does. The existing `active()` returns the focused pane's value, so shortcuts, rename, fork, archive, the sidebar highlight and the title menu keep working on the focused pane with no change. The other two approaches were rejected: one independent `Workspace` per pane (too much shared state: sessions, preferences, sidebar), and a read-only second pane (cannot have its own composer and panels).

## Design

### 1. State (`forge-features`, `workspace.rs` and a new `panes.rs`)

A pure type `Panes` in `panes.rs`, with no Dioxus, holds the rules and is unit-tested first:

```rust
pub struct Panes { targets: Vec<Active> /* 1 or 2 */, focused: usize, ratio: f32 }
```

- `focused_target()`, `target(i)`, `is_split()`, `focused()`, `ratio()`.
- `show(target)`: put `target` in the focused pane, unless it is already shown in the other pane, in which case focus that pane instead. This one rule covers the sidebar click, Home, Customize, Routines, New session and Back/Forward, and it is why two panes never show the same session or both show New session (they would share the unsent text).
- `open_beside(target)`: ⌥+click. With one pane, add `target` as the second pane and focus it. With two, the same as `show` on the second pane (not the focused one) and focus it. A target already shown just takes focus.
- `open_new_session_beside()`: `⌃⌘\`, `open_beside(Active::NewSession)`.
- `close(i)`: remove the pane; the other becomes the only one and keeps its target; focus goes to it.
- `swap()`: "Move split view left".
- `focus(i)`, `set_ratio(r)` (clamped, see section 4).

`Workspace.active` (a `Signal<Active>`) is replaced by `panes: Signal<Panes>`. `active()` reads `panes.focused_target()`. `display(to)` becomes `panes.show(to)`. The history (`NavHistory<Active>`) records the focused pane's targets; Back and Forward call `show` for the pane in focus, so a target that is open in the other pane moves focus there instead of duplicating. `reachable` is unchanged.

A session that is closed (archive with close, ⌥⌘A) while shown in a pane: that pane is closed if split, and shows New session if it is the only pane (today's behaviour).

New `Workspace` methods: `panes()`, `focus_pane(i)`, `open_session_beside(id)`, `open_new_session_beside()`, `close_pane(i)`, `swap_panes()`, `set_pane_ratio(r)`.

### 2. Opening

- Sidebar row: `onclick` reads the Option modifier. With it: `open_session_beside(id)`; without: `select(id)` as today. The row also marks every session shown in a pane (focused pane's row in the strong highlight, the other in the soft one).
- Search palette results and "Open" paths keep using `select`.
- Shortcut `⌃⌘\` is added to the shortcut table and the shortcut list; it calls `open_new_session_beside()`.

### 3. Drawing (`forge-pages/src/workspace_page.rs`, `forge-widgets`)

- The `match active { ... }` becomes a loop over the panes. Each pane is a `div.pane` (flex item, width from the ratio, `min-width: 0`) holding the same component as today: `ChatWindow { key: id, id }`, the new-session panel, Customize or Routines. The divider is a `div.pane-divider` between them with a drag handler (pointer events, the same way the sidebar and dock resizers already work).
- With one pane the markup is `div.pane` with width 100%: the same layout as now.
- Each pane gets a context value `PaneIndex(usize)`. A `pointerdown`/`focusin` anywhere in a pane calls `focus_pane(i)`. The focused pane has a 1 px accent outline only when split.
- Pane header: when split, the toolbar's Close button (×) is added at the right of every pane header. The ⋮ menu gets "Move split view left" (right pane only) and "Close split view".
- The dock (`dock_width`, `layout_for(window_width(), …)`): the layout decision uses the width of the pane, not the window. Each pane measures itself with a `ResizeObserver` and passes its width to `layout_for`; the sidebar state still comes from the window. With one pane the pane width equals today's main width, so nothing changes.
- The sidebar overlay (narrow window) is unchanged.

### 4. Sizes

- Default ratio 0.5. The divider drag sets the left pane's share. Each pane keeps at least 420 px: the ratio is clamped to that, and when the window is too narrow for two panes of 420 px (plus the docked sidebar) the second pane is not opened; ⌥+click then does the plain `select` instead.
- A pane under the compact threshold (the existing Plan 26 value) uses the compact header (toolbar icons fold into ⋮), as for a narrow window.

### 5. Web selectors and the Preview

Document-level selectors are scoped to the pane in focus or to the pane that owns the element:
- `SURFACE_SCRIPT` (`web_view_host.rs`) finds `.pane.focused .preview-surface`. `Workspace::set_surface` is called by the focused pane only; when focus moves, the old pane clears its surface and the new one reports its own. The web view follows the surface; `sync_web_views` already hides the view when there is no surface.
- An unfocused pane shows its Preview panel's tab bar and address bar and an empty body with a short line ("Click to show the page").
- `OVERLAY_SCRIPT` and `key_script` keep working on `.workspace` (one per window).
- The scroll helpers (`.messages`, `__forgeScrollWatch`), the composer focus script and `WATCH_FILE_LINKS` are called with the owning pane's element (`#pane-<i> …`) instead of `document.querySelector`. A pane id is part of the script text where the script is built.

### 6. Shortcuts and title menu

They act on `active()`, which is the focused pane, so no change. The title menu (PNL-07) already reads `active_title_menu()`.

### 7. Out of scope (kept as follow-ups)

Vertical stacking and a tree layout; more than two panes; the new-session pane's own "move" items; saving the split; focus keys; side chat; "move split down".

## Testing

- Unit tests for `Panes` (all rules above: show with a duplicate, open_beside with 1 and 2 panes, close, swap, ratio clamp, focus after close) and for `Workspace` history with two panes (Back moves the focused pane; Back to a target shown in the other pane moves focus).
- Sidebar highlight test for the two-pane case.
- Real window, by the harness (`scripts/harness/click.py` gets an `--option` flag first): open two sessions, type in each, drag the divider, close a pane, check the Preview body moves with focus, check the compact header at a narrow pane. Dark mode and Korean typing are left to the user.

## Risks

- The pane-scoped scripts are the main risk: a leftover `document.querySelector('.messages')` would act on the wrong pane. The plan lists every selector (`grep` for `querySelector` in `forge-widgets` and `forge-features`) and converts each.
- The native Preview view moves between panes; the surface rect must update on focus change and on divider drag. It is checked in the real window.
