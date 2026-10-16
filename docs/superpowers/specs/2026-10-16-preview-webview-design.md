# Plan 44: Preview shows pages inside the panel (PNL-03, child web view foundation)

Date: 2026-10-16. Roadmap row: PNL-03 (`docs/superpowers/roadmap/backlog.md`). Seen in observation rounds 5, 11 and 12 (`observations.md`): the Preview panel with its tabs and address bar, and the Browser pane opened by a click on an html, pdf or svg path in the chat.

## Goal

The Preview panel shows a web page inside the panel, as the desktop app does, using a native child web view laid over the panel's body. It is the base for later work: cookie import, the element picker, and Claude driving the browser.

Success:

- `localhost:PORT` and any `https` address load inside the panel; links, scrolling, typing and the dev server's hot reload work.
- Back, forward and reload act on the page; the address bar and the tab name follow the page, also when a link inside the page is clicked.
- The web view always covers exactly the panel body: while the panel is dragged, expanded, closed, or the window or sidebar changes size.
- A menu, dialog, search palette, lightbox or any other overlay is never hidden behind the web view.
- Each tab keeps its page when the person switches session and comes back.
- A click on an html, pdf or svg path in the chat opens it in a new Preview tab, as in the real app.
- If a web view cannot be created, the panel falls back to opening the address in the system browser, as it does today.

## Decisions made with the user

- Purpose C: the panel is meant to grow into cookie import, element picker and Claude-driven browsing. This plan builds only the foundation.
- Scope B: foundation, plus the chat path click (html, pdf, svg open in Preview). Cookie storage choices (SET-13) are left for the next plan; this plan uses the web view's one default store for the whole app.
- Approach: a native child web view (wry `build_as_child`) over the panel body. Not a `forge://` proxy into an `<iframe>` (HMR sockets, cookies and absolute paths break), and not the current system browser hand-off.
- The overlay problem is solved centrally by hiding the web view while any overlay exists, not by changing each widget.
- A spike on macOS comes first; the design below is the plan if the spike agrees, and the plan is revised if it does not.

## Background

Plan 21B found that an `<iframe>` of an `http(s)` page stays blank: `dioxus-desktop` 0.7.10 (`webview.rs`) refuses every `http(s)` navigation of its main web view and opens it outside. That check belongs to the main web view. A web view created by Forge itself through `wry::WebViewBuilder::build_as_child(&window)` is a separate view and is not subject to it. `DesktopService` exposes `window: Arc<Window>`; wry 0.53.5 (the version dioxus uses) has `set_bounds`, `set_visible`, `load_url`, `evaluate_script`, `with_navigation_handler`, `with_new_window_req_handler` and `with_on_page_load_handler`. On Windows the same wry uses WebView2.

A native view is always above the page's DOM, so any DOM overlay would be hidden behind it.

## Pieces

| Where | What |
| --- | --- |
| `forge-ports` `WebViewPort` (with `MockWebViewPort`) | `create(tab, url)`, `destroy(tab)`, `navigate(tab, url)`, `back`, `forward`, `reload`, `set_bounds(tab, rect)`, `set_visible(tab, bool)`, and a stream of `WebViewEvent`: `Navigated { url, can_back, can_forward }`, `Title(text)`, `Loading(bool)`, `LoadFailed { url, reason }`, `NewWindow(url)` |
| `forge-adapter-webview` (new crate) | Holds `TabId -> wry::WebView`. Depends on `wry = "=0.53.5"` directly. A web view is built and touched only on the main thread; calls from elsewhere go through the tao event loop proxy. Allows `http`, `https`, `file`; refuses everything else. No IPC handler. Denies downloads and permission requests |
| `forge-features` `PreviewTabs` | Replaces the list of "addresses opened from the tab" with the real history reported by events (current address, title, can-go-back, can-go-forward). `normalize_url` stays |
| `forge-features` `BrowserHost` | Signals for the active session's active tab, the panel rectangle and whether an overlay is open. From them it decides each web view's `set_bounds` and `set_visible` |
| `forge-features` open-path rule | One function that says where a clicked path goes: html, pdf, svg to Preview; csv and md to Files; png to the lightbox (the rule seen in rounds 11 and 12, now in one place with a table test) |
| `forge-widgets` `preview_panel.rs` | Tabs, address bar and buttons as now. The body is one empty `div.preview-surface`; a small script reports its rectangle (`ResizeObserver` on it and on `window` resize). Shows the load-failed notice and the fallback notice |
| `forge-shared` script | A `MutationObserver` that tells `BrowserHost` whether any element whose class contains `backdrop` exists (`picker-backdrop`, `menu-backdrop`, `settings-backdrop`, `palette-backdrop`, `lightbox-backdrop`, `usage-backdrop`, `trusted-backdrop`) |
| `forge-app` | Builds the adapter with the window and the event loop proxy and puts it in the workspace; shuts every web view down at exit |

`BrowserPort::open` stays: it is the fallback when a web view cannot be created.

## Data flow

1. An address typed in the bar goes `normalize_url` -> `PreviewTabs.navigate` -> `WebViewPort::navigate`. The first address of a tab creates its web view.
2. The page reports `Navigated`, `Title` and `Loading`; `PreviewTabs` updates; the address bar and the tab name change. A click on a link inside the page therefore keeps the history right.
3. The surface rectangle changes -> `BrowserHost` -> `set_bounds`.
4. Hidden means `set_visible(false)`: another session is active, the Preview panel is closed, another tab is active, an overlay is open, the rectangle is empty, or the load-failed notice is showing. Closing a tab destroys its web view; closing a session destroys all of that session's web views; switching session only hides.
5. A click on an html, pdf or svg path in the chat (a path that passes the existing Plan 28 checks: absolute, exists) opens the Preview panel, adds a tab and navigates it to the `file://` address.

## Errors and edge cases

- **Load failed** (server off, host not found): the panel body shows "Cannot connect to `host`" with a `Try again` button, and the web view is hidden. Own wording, not the engine's error page.
- **Address not allowed** (`javascript:`, `data:`, other schemes): nothing happens in the web view; the bar shows the existing "Enter a web address" notice. A link inside a page with such a scheme is ignored.
- **A `file://` address** must name an existing file; a folder or a missing path is refused with a notice.
- **Web view cannot be created** (for example WebView2 missing on Windows): the address opens in the system browser through `BrowserPort::open`, and the panel shows the reason in one line.
- **New window** (`target=_blank`, `window.open`): opens as a new Preview tab. The real app's behaviour was not seen; this is an assumption.
- **Downloads and permission requests** (camera, location, and the like): refused.

## Focus and keyboard

Clicking the page moves the keyboard to the web view, so Forge's keys (⌘K, ⌘/ and the others) may stop working. The spike checks this first.

- Preferred: keys that are menu items of `app_menu()` already work while the web view has focus on macOS.
- If a needed key does not reach Forge, a small page script sends `keydown` for those keys to Rust. This is the only place a script is injected; the message carries only a key code and modifiers, never page content, and no other IPC is offered to the page.
- Escape closes overlays as now; an open overlay hides the web view, so it keeps working.

## Lifetime

- A web view is created with its tab and destroyed with it.
- Switching session hides and does not destroy, so scroll position and typed text survive.
- Closing a session, and quitting the app, destroy all of them. No child process may remain after quit.
- No limit on the number of open web views; if memory proves a problem it is handled later.
- Cookies and storage: the web view's one default store, shared by every tab and session. Choosing a keep-cookies mode is SET-13.

## Spike (first task of the plan)

On macOS, with a throwaway child web view in `forge-app`:

1. It loads `http://localhost:PORT` and an `https` page, and accepts clicks, typing and scrolling.
2. After a click inside it, ⌘K and ⌘/ still reach Forge (or which keys do not).
3. The developer tools open on it.
4. The page cannot call the main view's IPC or reach any Forge handler.
5. `set_bounds` follows a resize with no visible lag, and `set_visible(false)` hides it at once when an overlay appears.

Outcome is written into this spec ("As built"). If 1 or 5 fails, the plan stops and is redone; if 2 fails the key fallback above is used.

## Tests

Unit tests (no window):

- `PreviewTabs`: events update address, title and history; can-go-back and can-go-forward; closing a tab moves the active one.
- `BrowserHost`: each reason to hide (other session, closed panel, other tab, overlay, empty rectangle, failed notice) hides, and removing the last reason shows again.
- Address rule: `http`, `https`, `file` pass; `javascript:`, `data:`, a folder, a missing file are refused.
- Open-path rule: a table of extensions to Preview, Files or lightbox.
- Widgets against `MockWebViewPort`: the calls made for navigate, new tab, close tab, close session.

Checked in the real window (`scripts/harness`):

1. A small dev server on `localhost`: the page shows, a link inside it changes the bar and the tab name, back, forward, reload.
2. Drag, expand and collapse the panel, resize the window, fold the sidebar: the web view stays exactly on the body.
3. Overlays: account menu, search palette, lightbox, Settings, a context menu. The web view hides and returns.
4. Switch session and come back: the page and its scroll are kept. Close a tab, close a session, quit: no leftover process.
5. Server off: the failed notice and `Try again`.
6. Chat path click for html, pdf and svg opens a new tab; csv, md and png go where they went before.
7. Compare with rounds 11 and 12 and record differences in `observations.md`.

Windows: unit tests and code review only. Recorded as "Windows unchecked", as in earlier plans.

## Not in this plan

Cookie import and the keep-cookies modes (SET-13); allowed-sites list; element picker; layout icon; Claude driving the browser; the panel's `⋮` menu (not seen in the real app); a snapshot in place of the hidden web view; allowing downloads and permissions. The Files tree keeps rendering html itself, as the real app does when a file is clicked in the tree.

## Roadmap updates when done

PNL-03 Plan column and notes (what is built and what is left), SET-13 note that the web view exists, `observations.md` with any difference found in the real window, and a memory file `plan-44-preview-webview.md`.
