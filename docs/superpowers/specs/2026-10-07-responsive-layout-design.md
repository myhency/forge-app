# Responsive layout (Plan 26)

Forge has no rule for the window width: there is no `@media` query, no minimum window size, the sidebar never folds and the dock columns keep a fixed minimum, so a narrow window overflows. The real Claude desktop app reflows. Observation round 10 (`docs/superpowers/roadmap/observations.md`) recorded how, at widths from 1500 down to 600 pt. This plan copies that behaviour. The user asked for it on 2026-10-07 and chose this over every roadmap feature.

## Scope, decided with the user on 2026-10-07

- In: a minimum window size; the sidebar folding to an overlay; the dock panel pushing the chat and then floating over it; the chat header shrinking; the new-session screen and the Home card shrinking; checking at 1500, 900, 700 and 600 pt.
- Approach C of three: CSS does everything that changes smoothly; Rust only decides the modes (sidebar overlay or docked, dock pushing or floating) from the window width, in one pure function. Pure CSS `@media` was rejected (the sidebar width is inline, the overlay needs state, and nothing is testable); doing all of it in Rust was rejected (too much for smooth shrinking).
- Out, left in observations as "not seen": the Settings window and the Files panel at narrow widths, the window height, and several dock columns open at once. Short windows are not changed.

## What was seen (round 10)

- Smallest window 600 pt. Sidebar 280 pt and fixed down to 710 pt; at 700 pt and below it is gone and the main pane fills the window. The toggle shows the sidebar as a 280 pt overlay on the pane (no dimming, the pane is not pushed).
- New-session screen: one centred column, window minus sidebar minus a side margin of 40 pt (25 pt from about 850 pt down), at most about 770 pt. Greeting and card sit at its left edge. The card is at most 480 pt and shrinks below that, its heatmap cells with it. The composer fills the column and its rows never wrap or hide at 600 pt.
- A session: centred text column, about 735 pt at 1100 pt.
- Panel open: the panel pushes the chat until the chat is about 300 pt, then floats over it (350 to 400 pt wide, rounded card, margin all round) and the chat stops shrinking. Header: the terminal icon goes first, the folder tag becomes an icon, the title gets an ellipsis; at the narrowest the title is cut to about one letter.

## Units

- **`layout_for(window_width, sidebar_width, dock_width) -> Layout`**, a pure function in `forge-domain` (`layout.rs`). `Layout { sidebar: SidebarMode, dock: DockMode }`, `SidebarMode::{Docked, Overlay}`, `DockMode::{Push, Float}`. Constants `MIN_WINDOW_WIDTH = 600`, `SIDEBAR_OVERLAY_BELOW = 710` (Overlay for widths up to 700), `MIN_CHAT_WIDTH = 300`, `OVERLAY_SIDEBAR_WIDTH = 280`. Docked below the threshold never happens; Push when `window - sidebar - dock >= MIN_CHAT_WIDTH`, else Float. With the sidebar in Overlay mode, the sidebar counts as 0 for the dock.
- **`use_window_width()`**, in `forge-features`: a signal that follows `window.innerWidth` through a `resize` listener (the same bridge style as the dock's pointer drag), set once at mount and on every resize, rounded to whole points. The workspace provider exposes `layout()` computed from it, the sidebar prefs width and the dock's total width.
- **Sidebar overlay state**: `sidebar_peek: Signal<bool>` in the workspace, not saved. Docked: `prefs.hidden` decides as today. Overlay: `sidebar_peek` decides; the toggle flips it; choosing a session or pressing Escape closes it; going Docked again clears it. The resize handle is not drawn in Overlay. `prefs.hidden` and `prefs.width` are never written by the overlay.
- **`WorkspacePage`** puts a class on `.workspace` (`sidebar-overlay`, `dock-float`) from `layout()`. All visual rules hang on those classes and on container queries.
- **Window minimum**: `with_min_inner_size(LogicalSize::new(600.0, 400.0))` in `forge-app/src/main.rs`. The height 400 is a guess (the real app's minimum height was not seen); it only stops the window from becoming unusable, and the plan checks that the composer and header still fit at 400.
- **CSS (`theme.css`)**:
  - `.sidebar-overlay .sidebar { position: absolute; z-index: 30; width: 280px; height: 100%; box-shadow: var(--shadow) }`, shown only while `sidebar_peek`; no handle, no rail beside the pane (the rail's `›` button moves into the header as the toggle).
  - `.main-pane { min-width: 0 }` (was 400px).
  - `.dock-float .dock-column { position: absolute; right: 8px; top: 8px; bottom: 8px; width: clamp(260px, 40vw, 400px); z-index: 25; max-width: none }`. With several columns the last one floats and the others stay in the row (assumption; several columns were not seen).
  - `.pane-header { container-type: inline-size }` and `@container` rules in this order as the header narrows: terminal and Changes buttons into the `⋮` menu (Rust renders both forms and CSS shows one), folder tag to its icon, title shortens (already an ellipsis).
  - `.new-session` and `.home-top`: `max-width: 770px; padding: 0 40px`, and `@media (max-width: 850px) { padding: 0 25px }`; `.usage-card { width: 100%; max-width: 480px }`; the heatmap uses a CSS grid with `1fr` columns and square cells (`aspect-ratio: 1`).
  - The composer rows are checked at 600 pt and given `flex-wrap: wrap` only if one does not fit.

## Behaviour

1. At any width from 600 up the app has no horizontal scroll bar and nothing is cut off except long titles (ellipsis).
2. Dragging the window across 700/710 pt flips the sidebar between docked and overlay without losing the selected session, the sidebar scroll position, or `prefs.hidden`.
3. An open dock panel never makes the chat narrower than 300 pt; below that the panel floats and the chat keeps its width.
4. The expanded dock column (`.expanded`) still fills the pane in both modes.
5. The palette, the settings overlay and menus keep working at 600 pt (they are centred or anchored; see testing).

## Testing

- `layout_for`: unit tests at 599 (clamped, treated as 600), 600, 700, 709, 710, and for the dock at the boundary (chat exactly 300 pt and 299 pt), with the sidebar docked and in overlay, and with a dock of 0.
- `sidebar_peek` rules as plain functions: toggle, close on choose, cleared when Docked again.
- Real window, with the harness (`docs/superpowers/roadmap` process, `scripts/harness/`): capture the new-session screen and a session with the Changes panel at 1500, 1100, 900, 700 and 600 pt; compare the numbers in round 10 (sidebar 280 pt, column margins, card 480 pt, panel floating once the window is under about 1060 pt with a 480 pt panel). Add `scripts/harness/resize.py W H` (osascript `set size of window 1` on the Forge process) for this. The search palette, Settings and a context menu are opened at 600 pt to check they stay inside the window.

## Risks and checks first

- The webview's support for container queries (WKWebView on this macOS): check in the first task; if missing, the header rules use the width signal and classes instead.
- `window.innerWidth` is in CSS pixels, which equal the points of the observation; checked at 1200 in the harness first.
- A resize event storm while dragging: the signal is only written when the rounded width changes.

## Not in this plan (later)

- Settings and the Files panel at narrow widths; the window height; several dock columns at once (check the real app first); remembering the window size between runs.
