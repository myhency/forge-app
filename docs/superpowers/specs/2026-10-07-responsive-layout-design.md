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

- **`layout_for(window_width, sidebar_width, dock_width) -> Layout`**, a pure function in `forge-domain` (`layout.rs`). `Layout { sidebar: SidebarMode, dock: DockMode, chat_width: u32 }`, `SidebarMode::{Docked, Overlay}`, `DockMode::{Push, Float}`. Constants `MIN_WINDOW_WIDTH = 600`, `SIDEBAR_OVERLAY_UP_TO = 700` (overlay at 700 and below, docked from 701), `SIDEBAR_RAIL_WIDTH = 36`, `MIN_CHAT_WIDTH = 300`, `NARROW_CHAT_BELOW = 560`. An overlay sidebar takes no room from the dock. `workspace_class(layout, resizing)` builds the class list of `.workspace` (`sidebar-overlay`, `dock-float`, `chat-narrow`).
- **`use_window_width()`**, in `forge-features`: a signal that follows `window.innerWidth` through a `resize` listener (`document::eval`, like the file-link watcher), written only when the whole-point width changes.
- **Sidebar overlay**: `peek: Signal<bool>` in `WorkspacePage`, not saved. The toggle is a floating `☰` button at the top left of the window (no rail). Choosing a session, Escape, a click on the backdrop, or growing the window past 700 pt closes it. The overlay never writes `prefs.hidden` or `prefs.width`. In overlay mode the pane header and the home column get extra left padding for the button.
- **Dock float**: the dock's columns sit in one `.dock` wrapper (`display: contents` when pushing). When floating, the whole wrapper is a card over the chat (top, right, bottom 8 pt, shadow), each column at most 400 pt. An expanded column counts as 0 wide, so it never floats.
- **Compact header**, by the `chat-narrow` class (chat under 560 pt) from the width signal, not by CSS container queries: layout containment would make the header the containing block of the fixed-position menu backdrops. Forge's header has no folder tag, so the title's ellipsis does that job; the Terminal, Changes and Preview icons move into the `⋮` menu and the Normal/Thinking/Detailed switch wraps onto a second row; side margins of the messages and the composer shrink to 16 pt.
- **Window minimum**: `with_min_inner_size(600, 400)`; the window then bottoms out at 600 x 432 pt (400 for the page, 32 for the title bar).
- **CSS for the rest**: `.main-pane { min-width: 0 }`; `.home-top` and `.new-session` at most 850 pt wide with 40 pt side padding (25 pt at 850 pt and below); `.usage-card` at most 480 pt and full width below; the heatmap uses `grid-auto-columns: 1fr`; `.composer-row` wraps and `.picker-button` does not break its label.

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

## As built (checked in the real window with `scripts/harness/resize.py`)

- 1200 pt: unchanged. 1100 pt with Changes open: pushes, chat 356 pt, compact header (only `⋮`), the view switch on a second row, the control row of the message box wraps.
- 900 pt with Changes open: floats as a card with a margin; the expand button makes it fill the pane beside the sidebar (no float).
- 800 pt: chat 536 pt, compact header; the `⋮` menu lists Terminal, Changes, Preview, Files, Prevent sleep; a click outside closes it (the backdrop covers the whole window).
- 700, 650 and 600 pt: no sidebar, `☰` at the top left; it opens a 280 pt overlay; choosing a session, Escape, and the backdrop close it; 650 to 900 to 650 leaves it closed.
- 600 x 400 asked: the window stops at 600 x 432 and still shows the header and the message box.
- The usage card was not on screen in the harness (the fixture has no usage data), so its 480 pt maximum and the smaller heatmap were not looked at: left for the next time the home screen is checked with real data.

## Not in this plan (later)

- Settings and the Files panel at narrow widths; the usage card and heatmap at narrow widths with real data; the window height; several dock columns at once (check the real app first); remembering the window size between runs.
