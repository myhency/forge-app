# Plan 44: Preview child web view Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Preview panel shows pages inside the panel through a native child web view, and a click on an html, pdf or svg path in the chat opens it in a new Preview tab.

**Architecture:** A `WebViewPort` in `forge-ports` (with a mock) hides wry. `forge-adapter-webview` builds one `wry::WebView` per tab with `build_as_child` on the app window and reports page events through a channel. `PreviewTabs` moves from the panel component into the workspace (per session) so web views survive a session switch. A pure rule (`browser_host::should_show`) decides when a web view is visible; a `WebViewHost` component feeds it the surface rectangle and the overlay state with two small scripts, and pumps events back into `PreviewTabs`.

**Tech Stack:** Rust workspace (Dioxus 0.7.10 desktop), `wry = "=0.53.5"` (same as dioxus-desktop), `forge-ports`, `forge-entities`, `forge-features`, `forge-widgets`, `forge-shared`, `forge-app`.

**Spec:** `docs/superpowers/specs/2026-10-16-preview-webview-design.md`

## Spec corrections found while planning (apply to the spec in Task 11)

1. wry 0.53.5 has **no** back/forward/can-go-back API. Back and forward run `history.back()` / `history.forward()` through `evaluate_script`; the Forge side keeps the shadow history it already has (`PreviewTabs`) and moves its position when a reported address equals the previous or next entry. A single-page app that only calls `pushState` does not report an address (known limit).
2. wry has **no** load-failed callback. Before a `localhost`, `127.0.0.1` or `[::1]` address typed in the bar is loaded, the adapter tries a TCP connect (1.5 s); if it fails the event is `LoadFailed`. Other hosts are not probed (a company proxy would give false failures); the engine's own error page shows.
3. wry has **no** permission-request hook. The engine's default prompt is kept. Downloads are refused (`with_download_started_handler` returns `false`).
4. The surface rectangle is read by a `requestAnimationFrame` loop that sends only on change, not by `ResizeObserver` (which does not fire when the sidebar moves the surface without resizing it).
5. `app_menu()` has no ⌘K or ⌘/ items; those keys are handled by the page root's `onkeydown`. A web view with focus will not deliver them, so key forwarding (Task 9) is part of the plan, not a fallback.

## Global Constraints

- `wry = "=0.53.5"` exactly (the version `dioxus-desktop 0.7.10` resolves); no other new dependency except what the adapter needs from std.
- Allowed page addresses: `http`, `https`, `file` only. A `file` address must name an existing file (not a folder).
- The page gets **no** IPC handler, except the key forwarder of Task 9, whose message carries only `key`, `code` and four modifier flags.
- Web views are created, touched and dropped on the window thread only.
- Switching session hides web views; closing a tab or a session destroys them; quitting leaves no child process.
- One default cookie/storage store for the whole app; no cookie modes (SET-13).
- If a web view cannot be created, the address opens in the system browser through `BrowserPort::open` and the panel says why in one line.
- A new window request (`target=_blank`, `window.open`) opens a new Preview tab; downloads are refused.
- Every `tr!` sentence needs a Korean text in `crates/forge-shared/src/ko.rs` (alphabetical by English text), and every entry there must be used (`i18n.rs` tests check both ways). No placeholder arguments inside `tr!`.
- Match surrounding code style and comment density; tests live next to the code in `mod tests`. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- The worktree has no `sidecar/node_modules`; this plan does not need the sidecar.
- Known unrelated failures to ignore: `forge-adapter-claude-plugins --test round_trip`, and the duplicate `SessionUpdate::PromptSuggestion` arm warning in `live_session_actor.rs`.
- Windows: unit tests and code review only; the final note says "Windows unchecked".

## Review Focus

1. A page link or typed address with a scheme outside `http`/`https`/`file` (`javascript:`, `data:`, `ftp:`) must never load. Task 2 and Task 5.
2. A `file://` address for a folder, a missing path or a path with spaces and `#` must be refused or encoded correctly. Task 2.
3. A web view must never stay visible over an open overlay, a closed panel, another session or a zero-size surface. Task 4.
4. Closing the last tab must close the panel and destroy that web view; closing a session must destroy all its web views, no leak. Task 7.
5. A server that is off (`localhost:9`) must show the failed notice and `Try again`, not a blank panel. Task 5 and Task 8.
6. Typing in the page, then pressing ⌘K or ⌘/ must still reach Forge. Task 9.
7. Creating a web view must fail soft: the system-browser fallback with a reason. Task 8.

---

## File Structure

- `crates/forge-ports/src/webview_port.rs` (new): `WebViewPort`, `WebViewTabId`, `Surface`, `WebViewEvent`, `WebViewEventKind`, mock.
- `crates/forge-ports/src/lib.rs`: exports.
- `crates/forge-entities/src/preview.rs`: `page_address`, `file_address`, `open_target`, `OpenTarget`.
- `crates/forge-features/src/preview_tabs.rs`: `PreviewTabs` driven by events (title, loading, failure).
- `crates/forge-features/src/browser_host.rs` (new): `should_show`.
- `crates/forge-features/src/workspace.rs`: `webviews` service, per-session `previews`, requests, event application, routing of `open_file`.
- `crates/forge-adapter-webview/` (new crate): `WryWebViews`.
- `crates/forge-app/src/main.rs`: builds the adapter, attaches the window.
- `crates/forge-widgets/src/preview_panel.rs`, `web_view_host.rs` (new): panel, surface, scripts, event pump, key forwarding.
- `crates/forge-shared/src/ko.rs`, `theme.css`: texts and styles.
- Docs: spec "As built", roadmap, observations, memory.

---

### Task 0: Spike on macOS (throwaway)

**Files:**
- Modify (throwaway, reverted at the end): `crates/forge-app/src/main.rs`, `crates/forge-app/Cargo.toml`
- Modify: `docs/superpowers/specs/2026-10-16-preview-webview-design.md` (append "Spike result")

**Interfaces:**
- Produces: a written answer to the five spike questions; later tasks assume yes to 1, 4 and 5 and use Task 9 for 2.

- [ ] **Step 1: Add the dependency and a throwaway child web view**

In `crates/forge-app/Cargo.toml` add under `[dependencies]`: `wry = "=0.53.5"`. In `main.rs`, inside `App` (find it with `grep -n "fn App" crates/forge-app/src/main.rs`), add at the top of the component body:

```rust
// SPIKE (throwaway): a child web view when FORGE_SPIKE_URL is set.
use_hook(|| {
    if let Ok(url) = std::env::var("FORGE_SPIKE_URL") {
        let window = dioxus::desktop::window();
        eprintln!("spike: component thread {:?}", std::thread::current().id());
        let view = wry::WebViewBuilder::new()
            .with_url(&url)
            .with_devtools(true)
            .with_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(300.0, 120.0).into(),
                size: wry::dpi::LogicalSize::new(600.0, 400.0).into(),
            })
            .build_as_child(&*window.window)
            .expect("spike web view");
        // Keep it alive for the life of the app.
        std::mem::forget(view);
    }
});
```

- [ ] **Step 2: Run it against a local server and a remote page**

```bash
cd /tmp && python3 -m http.server 8123 --directory /tmp >/dev/null 2>&1 &
cd /Users/james/orca/workspaces/forge/features
FORGE_SPIKE_URL=http://localhost:8123 cargo run -p forge-app 2>&1 | tee /tmp/spike.log
```

(Use the harness way of starting the app from `scripts/harness` if `cargo run` is not how the app is launched; see memory `driving-the-forge-app-visually`.) Repeat with `FORGE_SPIKE_URL=https://example.com`.

- [ ] **Step 3: Answer the five questions by hand and with screenshots**

1. The page appears at (300,120) 600x400, clicks, scrolling and typing in a form work.
2. Click inside the page, then press ⌘K and ⌘/: does Forge react? (Expected: no.)
3. Right click, Inspect Element opens the developer tools.
4. In the page's console run `typeof window.ipc` and `typeof window.__TAURI__`; expected `undefined`. The page cannot reach Forge handlers.
5. Resize the window: the web view stays where it was (expected, it is not tracking yet). Then confirm in the log that the component thread equals the window thread: run `eprintln!("main {:?}", std::thread::current().id())` in `main()` before launch (add it temporarily) and compare with the `spike: component thread` line.

- [ ] **Step 4: Record the result and revert the throwaway code**

Append a "Spike result (2026-10-16)" section to the spec with one line per question (yes/no and what was seen). Then:

```bash
git checkout crates/forge-app/src/main.rs crates/forge-app/Cargo.toml
```

If question 1 is no, or question 5's thread check shows the component runs on another thread, **stop** and tell the user; the adapter then needs the tao event-loop proxy and the plan is revised. If question 2 is yes (keys reach Forge), skip Task 9.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-10-16-preview-webview-design.md
git commit -m "docs: spike result for the Preview child web view (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 1: The `WebViewPort`

**Files:**
- Create: `crates/forge-ports/src/webview_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub struct WebViewTabId(pub u64)` (`Debug, Clone, Copy, PartialEq, Eq, Hash`)
  - `pub struct Surface { pub x: f64, pub y: f64, pub width: f64, pub height: f64 }` (`Debug, Clone, Copy, PartialEq`), logical pixels from the window's top left.
  - `pub enum WebViewEventKind { Navigated(String), Title(String), Loading(bool), LoadFailed { url: String, reason: String }, NewWindow(String), Key(KeyForward) }`
  - `pub struct KeyForward { pub key: String, pub code: String, pub ctrl: bool, pub meta: bool, pub alt: bool, pub shift: bool }`
  - `pub struct WebViewEvent { pub tab: WebViewTabId, pub kind: WebViewEventKind }`
  - `pub trait WebViewPort: Send + Sync` with `create(&self, tab: WebViewTabId, url: &str, surface: Surface) -> Result<(), String>`, `destroy(&self, tab)`, `navigate(&self, tab, url: &str) -> Result<(), String>`, `back(&self, tab)`, `forward(&self, tab)`, `reload(&self, tab)`, `set_surface(&self, tab, surface: Surface)`, `set_visible(&self, tab, visible: bool)`, `take_events(&self) -> Option<tokio::sync::mpsc::UnboundedReceiver<WebViewEvent>>`.

- [ ] **Step 1: Write the failing test (the mock can refuse)**

In `webview_port.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_refuse_to_create() {
        let mut mock = MockWebViewPort::new();
        mock.expect_create().returning(|_, _, _| Err("no web view".to_string()));
        let surface = Surface { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        assert_eq!(mock.create(WebViewTabId(1), "http://localhost", surface), Err("no web view".to_string()));
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-ports the_mock_can_refuse_to_create`
Expected: FAIL (module missing).

- [ ] **Step 3: Write the port**

```rust
use tokio::sync::mpsc::UnboundedReceiver;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WebViewTabId(pub u64);

/// Where a web view sits: logical pixels from the top left of the window's content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// A key pressed inside a page, sent on so Forge's own shortcuts still work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyForward {
    pub key: String,
    pub code: String,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WebViewEventKind {
    /// A page started loading this address (a typed one, a link, or back and forward).
    Navigated(String),
    Title(String),
    Loading(bool),
    /// The address could not be reached; only reported for addresses on this computer.
    LoadFailed { url: String, reason: String },
    /// The page asked for a new window (`target=_blank`, `window.open`).
    NewWindow(String),
    Key(KeyForward),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebViewEvent {
    pub tab: WebViewTabId,
    pub kind: WebViewEventKind,
}

/// Native web views laid over the window, one per Preview tab. Every call comes from the window
/// thread; an error is a message for the person.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait WebViewPort: Send + Sync {
    /// Makes a web view for the tab and loads `url`. Only `http`, `https` and `file` addresses load.
    fn create(&self, tab: WebViewTabId, url: &str, surface: Surface) -> Result<(), String>;
    fn destroy(&self, tab: WebViewTabId);
    fn navigate(&self, tab: WebViewTabId, url: &str) -> Result<(), String>;
    fn back(&self, tab: WebViewTabId);
    fn forward(&self, tab: WebViewTabId);
    fn reload(&self, tab: WebViewTabId);
    fn set_surface(&self, tab: WebViewTabId, surface: Surface);
    fn set_visible(&self, tab: WebViewTabId, visible: bool);
    /// The stream of page events. It can be taken once; later calls give `None`.
    fn take_events(&self) -> Option<UnboundedReceiver<WebViewEvent>>;
}
```

In `lib.rs` add `mod webview_port;`, `pub use webview_port::{KeyForward, Surface, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};` and under the mocks cfg `pub use webview_port::MockWebViewPort;`.

- [ ] **Step 4: Run to see it pass**

Run: `cargo test -p forge-ports`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: WebViewPort for native web views over the window (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Address and open-target rules (pure)

**Files:**
- Modify: `crates/forge-entities/src/preview.rs` (add after `preview_kind`)
- Modify: `crates/forge-entities/src/lib.rs` (export)

**Interfaces:**
- Produces:
  - `pub fn page_address(url: &str) -> bool` — true for `http`, `https`, `file` with something after `://` (scheme case-insensitive), no whitespace.
  - `pub fn file_address(absolute_path: &str) -> String` — `file:///…` with `%20`, `#`, `?`, `%` encoded; Windows `C:\a b` becomes `file:///C:/a%20b`.
  - `pub enum OpenTarget { Preview, Files, Lightbox }` and `pub fn open_target(path: &str) -> OpenTarget` — Html, Pdf, Svg → Preview; Image → Lightbox; everything else → Files.

- [ ] **Step 1: Write the failing tests** (in the existing `mod tests` of `preview.rs`)

```rust
#[test]
fn only_web_and_file_addresses_are_pages() {
    for ok in ["http://localhost:3000", "HTTPS://example.com/a", "file:///tmp/a.html"] {
        assert!(page_address(ok), "{ok}");
    }
    for bad in ["javascript:alert(1)", "data:text/html,hi", "ftp://x", "http://", "file://", "http://a b", "about:blank", ""] {
        assert!(!page_address(bad), "{bad:?}");
    }
}

#[test]
fn a_file_address_encodes_what_would_end_the_path() {
    assert_eq!(file_address("/tmp/a b/c#1?.html"), "file:///tmp/a%20b/c%231%3F.html");
    assert_eq!(file_address("/tmp/100%.html"), "file:///tmp/100%25.html");
    assert_eq!(file_address(r"C:\work\a b.html"), "file:///C:/work/a%20b.html");
}

#[test]
fn html_pdf_and_svg_open_in_preview_pictures_in_the_lightbox_the_rest_in_files() {
    for (path, target) in [
        ("a.html", OpenTarget::Preview),
        ("A.HTM", OpenTarget::Preview),
        ("doc.pdf", OpenTarget::Preview),
        ("pic.svg", OpenTarget::Preview),
        ("pic.png", OpenTarget::Lightbox),
        ("data.csv", OpenTarget::Files),
        ("README.md", OpenTarget::Files),
        ("main.rs", OpenTarget::Files),
        ("Makefile", OpenTarget::Files),
    ] {
        assert_eq!(open_target(path), target, "{path}");
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-entities page_address file_address open_target`
Expected: FAIL (items missing).

- [ ] **Step 3: Implement**

```rust
/// Whether a page may be loaded in a Preview web view: `http`, `https` or `file`, with something
/// after the scheme and no whitespace.
pub fn page_address(url: &str) -> bool {
    if url.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    let web = ["http", "https", "file"].iter().any(|allowed| scheme.eq_ignore_ascii_case(allowed));
    web && !rest.is_empty()
}

/// The `file://` address of an absolute path. Characters that would end or change the path are
/// percent-encoded; a Windows drive path gets forward slashes.
pub fn file_address(absolute_path: &str) -> String {
    let slashed = absolute_path.replace('\\', "/");
    let mut out = String::from("file://");
    if !slashed.starts_with('/') {
        out.push('/');
    }
    for ch in slashed.chars() {
        match ch {
            ' ' => out.push_str("%20"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            '%' => out.push_str("%25"),
            other => out.push(other),
        }
    }
    out
}

/// Where a click on a path in the chat goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenTarget {
    Preview,
    Files,
    Lightbox,
}

pub fn open_target(path: &str) -> OpenTarget {
    match preview_kind(path) {
        PreviewKind::Html | PreviewKind::Pdf | PreviewKind::Svg => OpenTarget::Preview,
        PreviewKind::Image => OpenTarget::Lightbox,
        _ => OpenTarget::Files,
    }
}
```

Export `page_address, file_address, open_target, OpenTarget` from `crates/forge-entities/src/lib.rs` next to `preview_kind`.

- [ ] **Step 4: Run to see them pass**

Run: `cargo test -p forge-entities`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: page address and open-target rules for Preview (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `PreviewTabs` driven by page events

**Files:**
- Modify: `crates/forge-features/src/preview_tabs.rs`
- Modify: `crates/forge-features/src/lib.rs` (export new names if the file lists them)

**Interfaces:**
- Consumes: `forge_ports::{WebViewEventKind}` is **not** used here (keep the crate boundary: this task takes plain values).
- Produces, on `PreviewTabs` (keep `open`, `close`, `select`, `ids`, `active`, `is_empty`, `normalize_url`):
  - `PreviewTabId` ids come from one process-wide counter (`static NEXT_TAB: AtomicU64`), so `PreviewTabId(n)` is unique across sessions and `WebViewTabId(n)` can reuse `n`.
  - `pub fn contains(&self, id) -> bool`
  - `pub fn loaded(&mut self, id, url: String)`: a page started loading `url`: if it equals the previous history entry the position steps back; if it equals the next entry it steps forward; if it equals the current entry nothing changes; otherwise forward entries are dropped and `url` is pushed. Clears the failure of the tab.
  - `pub fn set_title(&mut self, id, title: String)`, `pub fn title(&self, id) -> Option<String>`
  - `pub fn set_loading(&mut self, id, loading: bool)`, `pub fn loading(&self, id) -> bool`
  - `pub fn fail(&mut self, id, url: String, reason: String)`, `pub fn failure(&self, id) -> Option<(String, String)>`
  - `label(id)` returns the title when there is one, otherwise the host, otherwise "New tab".
  - `current`, `can_back`, `can_forward` as now. `navigate` is renamed `loaded`; `back` and `forward` no longer move the position (the page reports it); they return whether a step is possible.

- [ ] **Step 1: Rewrite the tests that used `navigate`/`back`/`forward`, and add new ones**

Replace the old history tests (read them first: `grep -n "fn .*" crates/forge-features/src/preview_tabs.rs | sed -n '1,60p'`) with:

```rust
#[test]
fn a_new_address_is_pushed_and_drops_the_forward_pages() {
    let mut tabs = PreviewTabs::default();
    let tab = tabs.open();
    tabs.loaded(tab, "http://a".into());
    tabs.loaded(tab, "http://b".into());
    tabs.loaded(tab, "http://c".into());
    // The page goes back twice, then follows a different link.
    tabs.loaded(tab, "http://b".into());
    tabs.loaded(tab, "http://a".into());
    assert_eq!(tabs.current(tab).as_deref(), Some("http://a"));
    assert!(tabs.can_forward(tab));
    tabs.loaded(tab, "http://d".into());
    assert_eq!(tabs.current(tab).as_deref(), Some("http://d"));
    assert!(!tabs.can_forward(tab));
    assert!(tabs.can_back(tab));
}

#[test]
fn the_same_address_reported_twice_changes_nothing() {
    let mut tabs = PreviewTabs::default();
    let tab = tabs.open();
    tabs.loaded(tab, "http://a".into());
    tabs.loaded(tab, "http://a".into());
    assert!(!tabs.can_back(tab));
}

#[test]
fn a_report_for_a_tab_that_is_gone_is_ignored() {
    let mut tabs = PreviewTabs::default();
    let tab = tabs.open();
    tabs.close(tab);
    tabs.loaded(tab, "http://a".into());
    tabs.set_title(tab, "x".into());
    assert!(tabs.is_empty());
}

#[test]
fn the_label_is_the_title_then_the_host() {
    let mut tabs = PreviewTabs::default();
    let tab = tabs.open();
    assert_eq!(tabs.label(tab), "New tab");
    tabs.loaded(tab, "http://localhost:3000/x".into());
    assert_eq!(tabs.label(tab), "localhost:3000");
    tabs.set_title(tab, "My app".into());
    assert_eq!(tabs.label(tab), "My app");
}

#[test]
fn a_failure_is_kept_until_the_next_page_loads() {
    let mut tabs = PreviewTabs::default();
    let tab = tabs.open();
    tabs.fail(tab, "http://localhost:9".into(), "refused".into());
    assert_eq!(tabs.failure(tab), Some(("http://localhost:9".to_string(), "refused".to_string())));
    tabs.loaded(tab, "http://localhost:3000".into());
    assert_eq!(tabs.failure(tab), None);
}

#[test]
fn tab_ids_are_unique_across_panels() {
    let (mut one, mut two) = (PreviewTabs::default(), PreviewTabs::default());
    assert_ne!(one.open(), two.open());
}
```

Note: the last test also needs `PreviewTabId` to implement `PartialEq` (it does).

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features preview_tabs`
Expected: FAIL (methods missing).

- [ ] **Step 3: Implement**

Change `PreviewTab` to hold `title: Option<String>`, `loading: bool`, `failure: Option<(String, String)>` next to `history` and `position`. Replace `next: u64` with a `static NEXT_TAB: std::sync::atomic::AtomicU64` used by `open`. Implement:

```rust
pub fn loaded(&mut self, id: PreviewTabId, url: String) {
    let Some(tab) = self.tab_mut(id) else { return };
    tab.failure = None;
    if tab.history.get(tab.position) == Some(&url) {
        return;
    }
    if tab.position > 0 && tab.history.get(tab.position - 1) == Some(&url) {
        tab.position -= 1;
        return;
    }
    if tab.history.get(tab.position + 1) == Some(&url) {
        tab.position += 1;
        return;
    }
    if !tab.history.is_empty() {
        tab.history.truncate(tab.position + 1);
    }
    tab.history.push(url);
    tab.position = tab.history.len() - 1;
    tab.title = None;
}
```

(`tab.title = None` on a new page so the host shows until the page names itself.) Add `set_title`, `title`, `set_loading`, `loading`, `fail` (also sets `loading = false`), `failure`, `contains`; `label` returns `title` first. Remove `navigate`, and change `back`/`forward` to `pub fn back(&self, id) -> bool { self.can_back(id) }` (same for forward). Keep `normalize_url` and its tests untouched.

- [ ] **Step 4: Fix callers so the crate builds**

`PreviewPanel` is the only caller; it is rewritten in Task 8. To keep this task green, leave `preview_panel.rs` compiling by replacing its uses with `loaded` temporarily is **not** needed: run `cargo test -p forge-features` (the widgets crate is not built by that command). Do not commit a broken workspace build: run `cargo check --workspace` and expect failures **only** in `forge-widgets/src/preview_panel.rs`; fix them minimally there (`tabs.write().navigate(` becomes `tabs.write().loaded(`, and the `back`/`forward` closures use `tabs.read().current(tab)` after calling `loaded` is not needed because Task 8 replaces the file) so the workspace builds.

- [ ] **Step 5: Run to see them pass, then commit**

Run: `cargo test -p forge-features && cargo check --workspace`
Expected: PASS.

```bash
git add crates/forge-features crates/forge-widgets/src/preview_panel.rs
git commit -m "feat: PreviewTabs follows what the page reports (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: When a web view is visible (pure)

**Files:**
- Create: `crates/forge-features/src/browser_host.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod browser_host; pub use browser_host::{should_show, ShowInput};`)

**Interfaces:**
- Produces: `pub struct ShowInput { pub session_active: bool, pub panel_open: bool, pub tab_active: bool, pub overlay_open: bool, pub surface: Option<Surface>, pub failed: bool }` and `pub fn should_show(input: &ShowInput) -> bool`. (`Surface` is `forge_ports::Surface`; add `forge-ports` to the crate imports if not present, it is already a dependency since `WorkspaceServices` uses ports.)

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn all_good() -> ShowInput {
        ShowInput {
            session_active: true,
            panel_open: true,
            tab_active: true,
            overlay_open: false,
            surface: Some(Surface { x: 10.0, y: 10.0, width: 400.0, height: 300.0 }),
            failed: false,
        }
    }

    #[test]
    fn it_shows_when_nothing_is_in_the_way() {
        assert!(should_show(&all_good()));
    }

    #[test]
    fn each_reason_alone_hides_it() {
        let cases: [(&str, fn(&mut ShowInput)); 6] = [
            ("another session", |i| i.session_active = false),
            ("closed panel", |i| i.panel_open = false),
            ("another tab", |i| i.tab_active = false),
            ("an overlay", |i| i.overlay_open = true),
            ("no surface yet", |i| i.surface = None),
            ("the failed notice", |i| i.failed = true),
        ];
        for (name, change) in cases {
            let mut input = all_good();
            change(&mut input);
            assert!(!should_show(&input), "{name}");
        }
    }

    #[test]
    fn an_empty_surface_hides_it() {
        for (w, h) in [(0.0, 300.0), (400.0, 0.0), (0.5, 300.0)] {
            let mut input = all_good();
            input.surface = Some(Surface { x: 0.0, y: 0.0, width: w, height: h });
            assert!(!should_show(&input), "{w}x{h}");
        }
    }

    #[test]
    fn it_shows_again_once_the_reason_is_gone() {
        let mut input = all_good();
        input.overlay_open = true;
        assert!(!should_show(&input));
        input.overlay_open = false;
        assert!(should_show(&input));
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features browser_host`
Expected: FAIL.

- [ ] **Step 3: Implement**

```rust
use forge_ports::Surface;

/// What decides whether a tab's web view may be seen.
#[derive(Debug, Clone, PartialEq)]
pub struct ShowInput {
    pub session_active: bool,
    pub panel_open: bool,
    pub tab_active: bool,
    /// Any menu, dialog, palette or lightbox of the app is open. A native view sits above the page, so it must step aside.
    pub overlay_open: bool,
    pub surface: Option<Surface>,
    /// The panel shows the "cannot connect" notice instead of the page.
    pub failed: bool,
}

pub fn should_show(input: &ShowInput) -> bool {
    let sized = input.surface.is_some_and(|surface| surface.width >= 1.0 && surface.height >= 1.0);
    input.session_active && input.panel_open && input.tab_active && !input.overlay_open && !input.failed && sized
}
```

- [ ] **Step 4: Run to see them pass, then commit**

Run: `cargo test -p forge-features browser_host`
Expected: PASS.

```bash
git add crates/forge-features
git commit -m "feat: rule for when a Preview web view is visible (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The wry adapter

**Files:**
- Create: `crates/forge-adapter-webview/Cargo.toml`, `crates/forge-adapter-webview/src/lib.rs`, `crates/forge-adapter-webview/src/probe.rs`
- Workspace members already use `crates/*`, so nothing else to register.

**Interfaces:**
- Consumes: `forge_ports::{WebViewPort, WebViewTabId, Surface, WebViewEvent, WebViewEventKind, KeyForward}`; `forge_entities::page_address`.
- Produces: `pub struct WryWebViews` with `WryWebViews::new() -> Self` (no window yet), `fn attach(&self, window: Arc<tao::window::Window>)` (called once from the window thread; records the thread id), and `impl WebViewPort for WryWebViews`.
- `probe::loopback_unreachable(url: &str) -> Option<String>`: for an `http(s)` address whose host is `localhost`, `127.0.0.1` or `[::1]`, tries a TCP connect with a 1.5 s timeout and returns `Some(reason)` when it fails; `None` for any other host or when it connects.

`Cargo.toml`:

```toml
[package]
name = "forge-adapter-webview"
version.workspace = true
edition.workspace = true

[dependencies]
forge-ports = { path = "../forge-ports" }
forge-entities = { path = "../forge-entities" }
tokio = { version = "1", features = ["sync"] }
wry = "=0.53.5"
tao = "=0.34.8"
```

(Use the exact `tokio` requirement the other adapters use: `grep tokio crates/forge-adapter-terminal/Cargo.toml`.)

- [ ] **Step 1: Write the failing probe tests** (`probe.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    #[test]
    fn a_listening_local_port_is_reachable() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(loopback_unreachable(&format!("http://localhost:{port}/x")), None);
        assert_eq!(loopback_unreachable(&format!("http://127.0.0.1:{port}")), None);
    }

    #[test]
    fn a_closed_local_port_is_reported() {
        let port = {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
        };
        assert!(loopback_unreachable(&format!("http://127.0.0.1:{port}")).is_some());
    }

    #[test]
    fn other_hosts_and_other_schemes_are_never_probed() {
        assert_eq!(loopback_unreachable("https://example.invalid"), None);
        assert_eq!(loopback_unreachable("file:///tmp/a.html"), None);
    }

    #[test]
    fn the_default_port_follows_the_scheme() {
        assert_eq!(default_port("http"), 80);
        assert_eq!(default_port("https"), 443);
    }
}
```

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-adapter-webview`
Expected: FAIL (functions missing).

- [ ] **Step 3: Implement the probe**

```rust
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

fn default_port(scheme: &str) -> u16 {
    if scheme.eq_ignore_ascii_case("https") { 443 } else { 80 }
}

/// For an address on this computer, tries to connect and says why it cannot. Other hosts are not
/// probed: behind a company proxy a direct connect fails while the page would load.
pub fn loopback_unreachable(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split(['/', '?', '#']).next()?;
    let (host, port) = if let Some(inner) = authority.strip_prefix('[') {
        let (host, tail) = inner.split_once(']')?;
        (host, tail.strip_prefix(':').and_then(|p| p.parse().ok()).unwrap_or_else(|| default_port(scheme)))
    } else if let Some((host, port)) = authority.rsplit_once(':') {
        (host, port.parse().ok()?)
    } else {
        (authority, default_port(scheme))
    };
    if !(host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1") {
        return None;
    }
    let addresses: Vec<SocketAddr> = (host, port).to_socket_addrs().ok()?.collect();
    let mut last = String::from("no address");
    for address in addresses {
        match TcpStream::connect_timeout(&address, Duration::from_millis(1500)) {
            Ok(_) => return None,
            Err(error) => last = error.to_string(),
        }
    }
    Some(last)
}
```

Run: `cargo test -p forge-adapter-webview`. Expected: probe tests PASS.

- [ ] **Step 4: Write the adapter**

`lib.rs` (keep one responsibility: own the views, report events):

```rust
mod probe;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use forge_entities::page_address;
use forge_ports::{KeyForward, Surface, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder};

thread_local! {
    // `WebView` cannot leave the window thread, so the views live here and the port is only a handle.
    static VIEWS: RefCell<HashMap<u64, WebView>> = RefCell::new(HashMap::new());
}

// The key forwarder: a chord with Command or Control (or Escape) goes to Forge. Nothing else leaves the page.
const KEY_SCRIPT: &str = r#"
window.addEventListener('keydown', function (e) {
  if (!(e.metaKey || e.ctrlKey || e.key === 'Escape')) return;
  window.ipc.postMessage(JSON.stringify({ k: e.key, c: e.code, ctrl: e.ctrlKey, meta: e.metaKey, alt: e.altKey, shift: e.shiftKey }));
}, true);
"#;

pub struct WryWebViews {
    window: Mutex<Option<(Arc<tao::window::Window>, ThreadId)>>,
    events_tx: UnboundedSender<WebViewEvent>,
    events_rx: Mutex<Option<UnboundedReceiver<WebViewEvent>>>,
}

impl WryWebViews {
    pub fn new() -> Self {
        let (events_tx, events_rx) = unbounded_channel();
        Self { window: Mutex::new(None), events_tx, events_rx: Mutex::new(Some(events_rx)) }
    }

    /// Gives the adapter the window. Called once, from the window thread.
    pub fn attach(&self, window: Arc<tao::window::Window>) {
        *self.window.lock().unwrap() = Some((window, std::thread::current().id()));
    }

    fn window(&self) -> Result<Arc<tao::window::Window>, String> {
        let guard = self.window.lock().unwrap();
        let (window, thread) = guard.as_ref().ok_or("The window is not ready")?;
        if *thread != std::thread::current().id() {
            return Err("A web view can only be used from the window thread".to_string());
        }
        Ok(window.clone())
    }

    fn send(&self, tab: WebViewTabId, kind: WebViewEventKind) {
        let _ = self.events_tx.send(WebViewEvent { tab, kind });
    }
}

impl Default for WryWebViews {
    fn default() -> Self {
        Self::new()
    }
}

fn rect(surface: Surface) -> Rect {
    Rect {
        position: LogicalPosition::new(surface.x, surface.y).into(),
        size: LogicalSize::new(surface.width.max(1.0), surface.height.max(1.0)).into(),
    }
}

fn parse_key(body: &str) -> Option<KeyForward> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    Some(KeyForward {
        key: value.get("k")?.as_str()?.to_string(),
        code: value.get("c")?.as_str()?.to_string(),
        ctrl: value.get("ctrl")?.as_bool()?,
        meta: value.get("meta")?.as_bool()?,
        alt: value.get("alt")?.as_bool()?,
        shift: value.get("shift")?.as_bool()?,
    })
}

impl WebViewPort for WryWebViews {
    fn create(&self, tab: WebViewTabId, url: &str, surface: Surface) -> Result<(), String> {
        let window = self.window()?;
        if !page_address(url) {
            return Err("Only http, https and file addresses can be shown".to_string());
        }
        if let Some(reason) = probe::loopback_unreachable(url) {
            self.send(tab, WebViewEventKind::LoadFailed { url: url.to_string(), reason });
        }
        let (nav, load, title, new_window, ipc) =
            (self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone());
        let view = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(rect(surface))
            .with_devtools(cfg!(debug_assertions))
            .with_initialization_script(KEY_SCRIPT)
            .with_navigation_handler(|url| page_address(&url))
            .with_new_window_req_handler(move |url, _| {
                if page_address(&url) {
                    let _ = new_window.send(WebViewEvent { tab, kind: WebViewEventKind::NewWindow(url) });
                }
                NewWindowResponse::Deny
            })
            .with_download_started_handler(|_, _| false)
            .with_document_title_changed_handler(move |text| {
                let _ = title.send(WebViewEvent { tab, kind: WebViewEventKind::Title(text) });
            })
            .with_on_page_load_handler(move |event, url| {
                let kind = match event {
                    PageLoadEvent::Started => {
                        let _ = nav.send(WebViewEvent { tab, kind: WebViewEventKind::Navigated(url) });
                        WebViewEventKind::Loading(true)
                    }
                    PageLoadEvent::Finished => WebViewEventKind::Loading(false),
                };
                let _ = load.send(WebViewEvent { tab, kind });
            })
            .with_ipc_handler(move |request| {
                if let Some(key) = parse_key(request.body()) {
                    let _ = ipc.send(WebViewEvent { tab, kind: WebViewEventKind::Key(key) });
                }
            })
            .build_as_child(&*window)
            .map_err(|error| error.to_string())?;
        VIEWS.with(|views| views.borrow_mut().insert(tab.0, view));
        Ok(())
    }

    fn destroy(&self, tab: WebViewTabId) {
        VIEWS.with(|views| views.borrow_mut().remove(&tab.0));
    }

    fn navigate(&self, tab: WebViewTabId, url: &str) -> Result<(), String> {
        if !page_address(url) {
            return Err("Only http, https and file addresses can be shown".to_string());
        }
        if let Some(reason) = probe::loopback_unreachable(url) {
            self.send(tab, WebViewEventKind::LoadFailed { url: url.to_string(), reason });
            return Ok(());
        }
        VIEWS.with(|views| match views.borrow().get(&tab.0) {
            Some(view) => view.load_url(url).map_err(|error| error.to_string()),
            None => Err("This tab has no page".to_string()),
        })
    }

    fn back(&self, tab: WebViewTabId) {
        VIEWS.with(|views| if let Some(view) = views.borrow().get(&tab.0) { let _ = view.evaluate_script("history.back()"); });
    }

    fn forward(&self, tab: WebViewTabId) {
        VIEWS.with(|views| if let Some(view) = views.borrow().get(&tab.0) { let _ = view.evaluate_script("history.forward()"); });
    }

    fn reload(&self, tab: WebViewTabId) {
        VIEWS.with(|views| if let Some(view) = views.borrow().get(&tab.0) { let _ = view.reload(); });
    }

    fn set_surface(&self, tab: WebViewTabId, surface: Surface) {
        VIEWS.with(|views| if let Some(view) = views.borrow().get(&tab.0) { let _ = view.set_bounds(rect(surface)); });
    }

    fn set_visible(&self, tab: WebViewTabId, visible: bool) {
        VIEWS.with(|views| if let Some(view) = views.borrow().get(&tab.0) { let _ = view.set_visible(visible); });
    }

    fn take_events(&self) -> Option<UnboundedReceiver<WebViewEvent>> {
        self.events_rx.lock().unwrap().take()
    }
}
```

Add `serde_json = "1"` to the crate's dependencies (use the version the other crates use). If wry's `with_ipc_handler` closure type is `Fn(Request<String>)`, `request.body()` is `&String`; if the compiler says otherwise, adjust to the signature at `~/.cargo/registry/src/*/wry-0.53.5/src/lib.rs:1130`. If `build_as_child(&*window)` does not accept `&tao::window::Window`, check the `HasWindowHandle` bound at line 1452 (tao's `Window` implements it with the `rwh_06` feature, which dioxus-desktop turns on).

A navigation that is not allowed (a link with a scheme outside the list) returns `false` from `with_navigation_handler` and is dropped silently.

- [ ] **Step 5: Add a pure test for the key parser, build, commit**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_message_is_read_and_a_wrong_one_is_dropped() {
        let ok = parse_key(r#"{"k":"k","c":"KeyK","ctrl":false,"meta":true,"alt":false,"shift":false}"#).unwrap();
        assert_eq!((ok.key.as_str(), ok.code.as_str(), ok.meta), ("k", "KeyK", true));
        assert!(parse_key("not json").is_none());
        assert!(parse_key(r#"{"k":1}"#).is_none());
    }
}
```

Run: `cargo test -p forge-adapter-webview && cargo build -p forge-adapter-webview`
Expected: PASS and a clean build.

```bash
git add crates/forge-adapter-webview Cargo.lock
git commit -m "feat: wry adapter for Preview web views (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Put the adapter in the app

**Files:**
- Modify: `crates/forge-app/Cargo.toml` (add `forge-adapter-webview = { path = "../forge-adapter-webview" }`)
- Modify: `crates/forge-app/src/main.rs`
- Modify: `crates/forge-features/src/workspace.rs` (`WorkspaceServices` gets `pub webviews: Arc<dyn WebViewPort>`; accessor `pub fn webviews(&self) -> Arc<dyn WebViewPort>` beside `browser()`)
- Modify: every place that builds `WorkspaceServices` in tests (find with `grep -rn "WorkspaceServices {" crates`; the only non-test one is `build_services` in `main.rs`; test builders, if any, use `MockWebViewPort::new()`)

**Interfaces:**
- Consumes: `WryWebViews::new()`, `WryWebViews::attach`.
- Produces: `workspace.webviews()`.

- [ ] **Step 1: Add the field and the accessor, build the failure**

In `WorkspaceServices` add `pub webviews: Arc<dyn WebViewPort>,` after `browser`; import `WebViewPort` from `forge_ports`. Add the accessor:

```rust
pub fn webviews(&self) -> Arc<dyn WebViewPort> {
    self.services.peek().webviews.clone()
}
```

Run: `cargo check --workspace`
Expected: FAIL at `build_services` (missing field).

- [ ] **Step 2: Build the adapter once and share it with the window**

`build_services()` runs before the window exists, so the same `Arc<WryWebViews>` is also stored where `App` can reach it. In `main.rs`:

```rust
use forge_adapter_webview::WryWebViews;

// The one web view host: the services hold it as a port, `App` attaches the window to it.
static WEBVIEWS: std::sync::OnceLock<Arc<WryWebViews>> = std::sync::OnceLock::new();

fn webviews() -> Arc<WryWebViews> {
    WEBVIEWS.get_or_init(|| Arc::new(WryWebViews::new())).clone()
}
```

In `build_services` add `webviews: webviews(),` to the `WorkspaceServices` literal. In `App` (`grep -n "fn App" crates/forge-app/src/main.rs`) add at the top of the body:

```rust
// The web views of the Preview panel are children of this window.
use_hook(|| webviews().attach(dioxus::desktop::window().window.clone()));
```

- [ ] **Step 3: Build and run the existing tests**

Run: `cargo check --workspace && cargo test -p forge-features`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/forge-app crates/forge-features Cargo.lock
git commit -m "feat: the app builds the web view host and attaches its window (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Preview state per session in the workspace

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`

**Interfaces:**
- Consumes: `PreviewTabs` (Task 3), `should_show`/`ShowInput` (Task 4), `open_target`, `file_address`, `page_address` (Task 2), `WebViewPort` (Task 1).
- Produces on `Workspace`:
  - `pub fn previews(&self, id: LiveSessionId) -> PreviewTabs`
  - `pub fn change_previews(&self, id, change: impl FnOnce(&mut PreviewTabs))`
  - `pub fn preview_go(&self, id, tab: PreviewTabId, typed: &str, surface: Option<Surface>) -> Result<(), String>`: normalizes, creates the web view on the tab's first address or navigates, and on creation failure opens the system browser (spec fallback) and returns `Err(reason)`.
  - `pub fn open_in_preview(&self, id, absolute_path_or_url: String)`: opens the Preview panel, adds a tab, loads it.
  - `pub fn apply_web_view_event(&self, event: WebViewEvent)`
  - `pub fn set_overlay_open(&self, open: bool)`, `pub fn overlay_open(&self) -> bool`
  - `pub fn set_surface(&self, surface: Option<Surface>)`, `pub fn surface(&self) -> Option<Surface>`
  - `pub fn sync_web_views(&self)`: applies `should_show` and `set_surface` to every tab of every session.
  - `open_file` routes by `open_target`; closing a session destroys its web views; `close_tab(id, tab)` destroys one.

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` of `workspace.rs`, following how the existing workspace tests build a workspace (read the nearest test, e.g. `grep -n "fn .*file_request\|fn open_file" crates/forge-features/src/workspace.rs`, and copy its setup). Use `MockWebViewPort` with `.expect_*` calls:

```rust
#[test]
fn a_click_on_an_html_path_opens_a_preview_tab_and_not_the_files_panel() { /* open_file(id, "<folder>/page.html"): dock has Preview open, Files closed; previews(id) has one tab whose current() is file_address(path); mock create called once */ }

#[test]
fn a_click_on_a_csv_path_still_goes_to_files() { /* open_file(id, "<folder>/data.csv"): Files open, no preview tab, mock create never called */ }

#[test]
fn closing_a_tab_destroys_its_web_view() { /* mock expect_destroy().times(1) with WebViewTabId(tab.0) */ }

#[test]
fn closing_a_session_destroys_every_web_view_of_it() { /* two tabs, close session: expect_destroy().times(2) */ }

#[test]
fn a_failed_create_opens_the_system_browser_and_says_why() { /* mock create returns Err("WebView2 is missing"); MockBrowserPort expect_open().times(1); preview_go returns Err containing the reason */ }

#[test]
fn a_page_event_for_a_known_tab_updates_it_and_an_unknown_tab_is_ignored() { /* apply_web_view_event Navigated/Title/Loading/LoadFailed/NewWindow; NewWindow adds a tab and navigates it */ }
```

Write each test body fully when implementing (the comments above name the exact assertions); every test must assert the mock call counts with `.times(n)`.

- [ ] **Step 2: Run to see them fail**

Run: `cargo test -p forge-features workspace::tests::a_click`
Expected: FAIL (methods missing).

- [ ] **Step 3: Implement**

Add fields next to `docks`/`file_requests`: `previews: Signal<HashMap<LiveSessionId, PreviewTabs>>`, `preview_surface: Signal<Option<Surface>>`, `overlay_open: Signal<bool>`, initialised in the constructor like `lightbox: Signal::new(None)`. Implement the methods above:

```rust
pub fn preview_go(&self, id: LiveSessionId, tab: PreviewTabId, typed: &str, surface: Option<Surface>) -> Result<(), String> {
    let url = if forge_entities::page_address(typed) { typed.to_string() } else {
        normalize_url(typed).ok_or_else(|| tr!("Enter a web address, such as localhost:3000").to_string())?
    };
    if url.starts_with("file") && !file_exists(&url) {
        return Err(tr!("This file cannot be shown.").to_string());
    }
    let first = self.previews(id).current(tab).is_none();
    self.change_previews(id, |tabs| tabs.loaded(tab, url.clone()));
    let port = self.webviews();
    let id_tab = WebViewTabId(tab.0);
    let result = if first {
        let surface = surface.unwrap_or(Surface { x: 0.0, y: 0.0, width: 1.0, height: 1.0 });
        port.create(id_tab, &url, surface)
    } else {
        port.navigate(id_tab, &url)
    };
    if let Err(reason) = result {
        let _ = self.browser().open(&url);
        return Err(format!("{} {reason}", tr!("The page could not be shown here, so it was opened in your browser.")));
    }
    self.sync_web_views();
    Ok(())
}
```

`file_exists(url)` decodes the `%20`, `%23`, `%3F`, `%25` and checks `std::path::Path::is_file` (put it as a private pure helper `path_of_file_address` in `forge-entities/preview.rs` with its own test in Task 2's style, if not already there; add `pub fn path_of_file_address(url: &str) -> Option<String>` there and its test `a_file_address_round_trips`). `apply_web_view_event` finds the session whose `PreviewTabs.contains(PreviewTabId(event.tab.0))` and maps: `Navigated(url)` -> `loaded`, `Title` -> `set_title`, `Loading` -> `set_loading`, `LoadFailed` -> `fail` (then `sync_web_views`), `NewWindow(url)` -> a new tab + `preview_go`, `Key(_)` is handled by the host component (Task 9), not here. `open_file` becomes:

```rust
match forge_entities::open_target(&relative) {
    OpenTarget::Lightbox => { /* existing lightbox code */ return; }
    OpenTarget::Preview => { self.open_in_preview(id, absolute_path); return; }
    OpenTarget::Files => {}
}
```

where `open_in_preview` opens the Preview panel (`dock.toggle(Panel::Preview)` only if closed), adds a tab with `change_previews(id, |t| t.open())`, and calls `preview_go(id, tab, &file_address(&absolute_path), self.surface())`. It keeps the existing "inside the session folder" notice check (that check runs before the match, as today). `sync_web_views` loops over `previews` and calls `should_show(ShowInput { session_active: self.active() == Active::Live(id), panel_open: self.dock(id).is_open(Panel::Preview), tab_active: tabs.active() == Some(tab), overlay_open: self.overlay_open(), surface: self.surface(), failed: tabs.failure(tab).is_some() })`, then `set_surface` when shown and `set_visible`. Closing: add `close_tab(&self, id, tab)` (destroy, `change_previews(.. close)`, close the panel when empty) and call `destroy` for every tab inside the existing session-close path (find it: `grep -n "fn close_active\|fn close_session\|docks.write().remove" crates/forge-features/src/workspace.rs`).

- [ ] **Step 4: Run to see them pass**

Run: `cargo test -p forge-features`
Expected: PASS (new tests and the old ones).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features crates/forge-entities
git commit -m "feat: Preview tabs live in the workspace per session and chat paths open them (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The panel, the surface and the event pump

**Files:**
- Modify: `crates/forge-widgets/src/preview_panel.rs`
- Create: `crates/forge-widgets/src/web_view_host.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod web_view_host;` and re-export `WebViewHost`)
- Modify: `crates/forge-pages/src/workspace_page.rs` (render `WebViewHost {}` once, inside the root `div`)
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: `workspace.previews/change_previews/preview_go/close_tab/apply_web_view_event/set_overlay_open/set_surface/sync_web_views` (Task 7).
- Produces: `#[component] pub fn WebViewHost() -> Element` (renders nothing; owns the pump and the two scripts).

- [ ] **Step 1: Write the `WebViewHost`**

Follow `use_window_width` in `forge-features` for the `document::eval` pattern (`grep -rn "document::eval" crates --include=*.rs | head -3`). Three jobs, each a `use_hook`/`use_future` spawned once:

```rust
use dioxus::prelude::*;
use forge_features::use_workspace;
use forge_ports::Surface;

// Reports the Preview surface rectangle when it changes, one message per change.
const SURFACE_SCRIPT: &str = r#"
let last = '';
function tick() {
  const el = document.querySelector('.preview-surface');
  const r = el ? el.getBoundingClientRect() : null;
  const text = r ? [r.x, r.y, r.width, r.height].join(',') : '';
  if (text !== last) { last = text; dioxus.send(text); }
  requestAnimationFrame(tick);
}
tick();
"#;

// Reports whether any backdrop (menu, dialog, palette, lightbox) is in the document.
const OVERLAY_SCRIPT: &str = r#"
let last = null;
function check() {
  const open = document.querySelector('[class*="backdrop"]') !== null;
  if (open !== last) { last = open; dioxus.send(open); }
}
new MutationObserver(check).observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['class'] });
check();
"#;

#[component]
pub fn WebViewHost() -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        let mut eval = document::eval(SURFACE_SCRIPT);
        while let Ok(text) = eval.recv::<String>().await {
            let parts: Vec<f64> = text.split(',').filter_map(|p| p.parse().ok()).collect();
            workspace.set_surface(match parts.as_slice() {
                [x, y, width, height] => Some(Surface { x: *x, y: *y, width: *width, height: *height }),
                _ => None,
            });
            workspace.sync_web_views();
        }
    });
    use_future(move || async move {
        let mut eval = document::eval(OVERLAY_SCRIPT);
        while let Ok(open) = eval.recv::<bool>().await {
            workspace.set_overlay_open(open);
            workspace.sync_web_views();
        }
    });
    use_future(move || async move {
        let Some(mut events) = workspace.webviews().take_events() else { return };
        while let Some(event) = events.recv().await {
            workspace.apply_web_view_event(event);
        }
    });
    // A session switch, a panel toggle or a tab switch changes what may be seen.
    use_effect(move || {
        let _ = (workspace.active(), workspace.active_dock_open_preview());
        workspace.sync_web_views();
    });
    rsx! {}
}
```

Add `pub fn active_dock_open_preview(&self) -> bool` to `Workspace` (true when the active session's dock has Preview open; reads the `docks` signal so the effect re-runs).

- [ ] **Step 2: Rewrite `PreviewPanel` on the workspace state**

`PreviewPanel(id)` reads `workspace.previews(id)`; the tab buttons call `change_previews(id, |t| t.select(tab))` and `workspace.close_tab(id, tab)`; the `+` button calls `change_previews(id, |t| { t.open(); })`; Enter in the address field calls `workspace.preview_go(id, tab, &input(), workspace.surface())` and sets `notice` from an `Err`; back/forward call `workspace.webviews().back/forward(WebViewTabId(tab.0))`; reload calls `reload`. The body is:

```rust
div { class: "preview-body",
    if let Some((url, _reason)) = failure {
        div { class: "preview-failed",
            p { {tr!("Cannot connect to this address.")} }
            p { class: "preview-failed-url", "{url}" }
            button { class: "preview-open", r#type: "button", onclick: move |_| retry(), {tr!("Try again")} }
        }
    } else {
        div { class: "preview-surface" }
    }
    if let Some(message) = notice() { span { class: "error", "{message}" } }
}
```

`retry` calls `preview_go` with the failed URL. When a tab has no address yet, show the existing hint text "Enter an address above." (translated string already exists in `ko.rs`; remove the strings that are no longer used: "Open in browser", "This page is open in your browser.", "Enter an address above. The page opens in your browser." — the `i18n` test lists unused entries). The address field's `input` signal follows the active tab's current address (`use_effect` on `previews(id).current(active)`).

- [ ] **Step 3: Styles**

In `theme.css` add `.preview-body { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; } .preview-surface { flex: 1; min-height: 0; } .preview-failed { padding: 24px 16px; text-align: center; color: var(--text-muted); }` (match the variable names used by neighbouring rules: `grep -n "dock-note" crates/forge-shared/src/theme.css`).

- [ ] **Step 4: Korean texts**

Add to `ko.rs`, alphabetical by English text: "Cannot connect to this address." (`이 주소에 연결할 수 없습니다.`), "Try again" (`다시 시도`), "The page could not be shown here, so it was opened in your browser." (`여기에서 페이지를 보여 줄 수 없어 브라우저에서 열었습니다.`), "This file cannot be shown." (`이 파일은 표시할 수 없습니다.`). Remove entries that are no longer used.

- [ ] **Step 5: Build, test, commit**

Run: `cargo test --workspace --exclude forge-adapter-claude-plugins 2>&1 | tail -20` then `cargo test -p forge-adapter-claude-plugins --lib`.
Expected: PASS (i18n tests included).

```bash
git add crates
git commit -m "feat: the Preview panel shows the page in a child web view (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Keys pressed inside the page reach Forge

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (`apply_web_view_event` returns the forwarded key to the host instead of dropping it) or handle it in `WebViewHost`
- Modify: `crates/forge-widgets/src/web_view_host.rs`

**Interfaces:**
- Consumes: `WebViewEventKind::Key(KeyForward)` (Task 1), emitted by the adapter's `KEY_SCRIPT` (Task 5).
- Produces: a synthetic `keydown` on Forge's page root.

Skip this task only if the spike (Task 0, question 2) showed ⌘K and ⌘/ already reach Forge after a click in the page; then also delete `KEY_SCRIPT` and `with_ipc_handler` from the adapter.

- [ ] **Step 1: Write the test for the script builder**

The host turns a `KeyForward` into JS. Make that a pure function in `web_view_host.rs` so it is testable:

```rust
fn key_script(key: &KeyForward) -> String {
    let quote = |text: &str| serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(function(){{var t=document.querySelector('.workspace')||document.body;t.focus();\
         t.dispatchEvent(new KeyboardEvent('keydown',{{key:{},code:{},ctrlKey:{},metaKey:{},altKey:{},shiftKey:{},bubbles:true,cancelable:true}}));}})();",
        quote(&key.key), quote(&key.code), key.ctrl, key.meta, key.alt, key.shift
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_script_carries_the_chord_and_quotes_the_text() {
        let script = key_script(&KeyForward { key: "k".into(), code: "KeyK".into(), ctrl: false, meta: true, alt: false, shift: false });
        assert!(script.contains("key:\"k\""));
        assert!(script.contains("metaKey:true"));
        let hostile = key_script(&KeyForward { key: "\"});alert(1);//".into(), code: "x".into(), ctrl: false, meta: false, alt: false, shift: false });
        assert!(!hostile.contains("alert(1);//\"") || hostile.contains("\\\""));
    }
}
```

(Add `serde_json` to `forge-widgets` if it is not a dependency; check `crates/forge-widgets/Cargo.toml`.)

- [ ] **Step 2: Run to see it fail, implement, run to see it pass**

Run: `cargo test -p forge-widgets key_script` — FAIL, then add `key_script` and a fourth `use_future` in `WebViewHost` that listens to the same events. Because the events receiver is taken once (Task 8), `apply_web_view_event` must hand keys on: give `Workspace` a `key_forward: Signal<Option<KeyForward>>` set by `apply_web_view_event` on `Key`, and let `WebViewHost` run `document::eval(&key_script(&key))` in a `use_effect` that reads it, then clears it. Verify the class: `grep -n "fn workspace_class" -A6 crates/forge-pages/src/workspace_page.rs crates/forge-domain/src/*.rs` and use that class name instead of `.workspace` if it differs.

- [ ] **Step 3: Run the tests and commit**

Run: `cargo test -p forge-widgets -p forge-features`
Expected: PASS.

```bash
git add crates
git commit -m "feat: shortcuts pressed inside a page reach Forge (PNL-03)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Check in the real window

**Files:**
- Modify: `docs/superpowers/roadmap/observations.md` (differences found)

Use `scripts/harness` and the way the app is driven (memory: `driving-the-forge-app-visually`; `FORGE_CLAUDE_HOME` fixture, `FORGE_CONFIG_PATH`). Never send a message to the real Claude app; this plan only drives Forge.

- [ ] **Step 1:** Start a small dev server (`python3 -m http.server 8123` in a temp folder with an `index.html` that has a link to `second.html` and a text field). Open Preview, type `localhost:8123`. Expected: the page is inside the panel; clicking the link changes the address bar and the tab name; back, forward, reload work; typing in the field works.
- [ ] **Step 2:** Drag the panel's left edge, expand and collapse the panel, resize the window, fold and unfold the sidebar. Expected: the page always fills the panel body and nothing else.
- [ ] **Step 3:** With the page open, open the account menu, the search palette (⌘K), a session's context menu, the shortcut list (⌘/) and Settings (⌘,); open a picture from a chat path (lightbox). Expected: the page hides while each is open and returns when it closes.
- [ ] **Step 4:** Click inside the page, then press ⌘K and ⌘/. Expected: the palette and the list open (Task 9).
- [ ] **Step 5:** Switch to another session and back; close a tab; close a session; quit the app. Expected: the page and its scroll survive the switch; `pgrep -fl Forge` and `pgrep -fl WebKit` show no leftovers of the app after quit.
- [ ] **Step 6:** Type `localhost:9`. Expected: "Cannot connect to this address." and `Try again`; start the server on that port and press `Try again`: the page loads.
- [ ] **Step 7:** Make a session whose reply names an `.html`, a `.pdf` and an `.svg` file in the session folder (use the fixture; do not call the real SDK for it: write the message into the fixture transcript) and click each path; then a `.csv`, `.md` and `.png`. Expected: the first three open as new Preview tabs; csv and md in Files; png in the lightbox.
- [ ] **Step 8:** Type `javascript:alert(1)` and `data:text/html,hi` in the address bar. Expected: the notice "Enter a web address, such as localhost:3000" and no load.
- [ ] **Step 9:** Record what differed from the real app's rounds 11 and 12 in `observations.md` (new heading "Round 17"), commit.

```bash
git add docs
git commit -m "docs: Preview web view checked in the real window

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Documents and memory

**Files:**
- Modify: `docs/superpowers/specs/2026-10-16-preview-webview-design.md` (apply the five corrections at the top of this plan; add "As built")
- Modify: `docs/superpowers/roadmap/backlog.md` (PNL-03 row: Plan column and notes; SET-13 note; "Left" list)
- Create: `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/plan-44-preview-webview.md` and a line in `MEMORY.md`

- [ ] **Step 1:** Edit the spec: replace "the real history reported by events" with the shadow-history rule; replace the `ResizeObserver` description with the `requestAnimationFrame` loop; add the loopback probe; state that permission requests keep the engine default; state that key forwarding is built (Task 9) unless skipped. Add an "As built" section listing what was checked and "Windows unchecked".
- [ ] **Step 2:** Update PNL-03 in `backlog.md`: Plan column `Plan 44 (built: page in a child web view, tabs, history, chat path click)`; Notes: what is left (cookie import, element picker, layout icon, panel `⋮`, snapshot while hidden, downloads, `pushState`-only address changes).
- [ ] **Step 3:** Write the memory file (type `project`, with **Why** and **How to apply**; link `[[plan-14-roadmap]]` and `[[plan-28-file-previews]]`) and add its one-line pointer to `MEMORY.md`.
- [ ] **Step 4: Commit and report**

```bash
git add docs
git commit -m "docs: Plan 44 as built, roadmap updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

Report to the user with the results of Task 10 and the branch name (`plan-44`); merging to `master` is the user's decision (use `superpowers:finishing-a-development-branch`).
