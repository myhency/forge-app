mod probe;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::ThreadId;

use forge_entities::{page_address, web_address};
use forge_ports::{KeyForward, Surface, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{NewWindowResponse, PageLoadEvent, Rect, WebView, WebViewBuilder};

thread_local! {
    // `WebView` cannot leave the window thread, so the views live here and the port is only a handle.
    static VIEWS: RefCell<HashMap<u64, WebView>> = RefCell::new(HashMap::new());
}

// The key forwarder: a chord with Command or Control goes to Forge, which decides whether it is one of its
// shortcuts. The page still gets the key. The message carries a nonce that only this script knows, and only a
// real key press (`isTrusted`) is sent, so a page cannot make Forge act by posting a message of its own.
fn key_script(nonce: &str) -> String {
    format!(
        r#"(function () {{
  const post = window.ipc.postMessage.bind(window.ipc);
  const stringify = JSON.stringify.bind(JSON);
  const nonce = "{nonce}";
  window.addEventListener('keydown', function (e) {{
    if (!e.isTrusted || !(e.metaKey || e.ctrlKey)) return;
    post(stringify({{ n: nonce, k: e.key, c: e.code, ctrl: e.ctrlKey, meta: e.metaKey, alt: e.altKey, shift: e.shiftKey }}));
  }}, true);
}})();"#
    )
}

// Keeps the page's console messages (the last 500) and uncaught errors for the browser tool
// `read_console_messages`, which reads them back with a script. The page's own console still gets them.
const CONSOLE_SCRIPT: &str = r#"(function () {
  if (window.__forgeConsole) return;
  const kept = [];
  Object.defineProperty(window, '__forgeConsole', { value: kept, enumerable: false });
  const keep = function (level, args) {
    let text;
    try { text = Array.prototype.map.call(args, function (a) { return typeof a === 'string' ? a : (a instanceof Error ? a.stack || a.message : JSON.stringify(a)); }).join(' '); } catch (e) { text = String(args[0]); }
    kept.push({ level: level, text: String(text).slice(0, 2000), time: Date.now() });
    if (kept.length > 500) kept.shift();
  };
  ['log', 'info', 'warn', 'error', 'debug'].forEach(function (level) {
    const original = console[level].bind(console);
    console[level] = function () { keep(level, arguments); return original.apply(null, arguments); };
  });
  window.addEventListener('error', function (e) { keep('exception', [e.message + (e.filename ? ' (' + e.filename + ':' + e.lineno + ')' : '')]); });
  window.addEventListener('unhandledrejection', function (e) { keep('exception', ['Unhandled rejection: ' + (e.reason && (e.reason.stack || e.reason.message) || String(e.reason))]); });
})();"#;

// An AppKit image as PNG bytes.
#[cfg(target_os = "macos")]
fn png_of(image: &objc2_app_kit::NSImage) -> Option<Vec<u8>> {
    let tiff = image.TIFFRepresentation()?;
    let bitmap = objc2_app_kit::NSBitmapImageRep::imageRepWithData(&tiff)?;
    // SAFETY: an empty property dictionary is a valid one.
    let png = unsafe { bitmap.representationUsingType_properties(objc2_app_kit::NSBitmapImageFileType::PNG, &objc2_foundation::NSDictionary::new()) }?;
    Some(png.to_vec())
}

// Keeps the page's network requests (the last 500) for `read_network_requests`: fetch and XHR with
// their method and status, and every other load the page made (documents, scripts, images) from
// the resource timing list when asked. Query values named like credentials are shown as REDACTED.
const NETWORK_SCRIPT: &str = r#"(function () {
  if (window.__forgeNetwork) return;
  const kept = [];
  Object.defineProperty(window, '__forgeNetwork', { value: kept, enumerable: false });
  const redact = function (url) {
    try { const u = new URL(url, location.href); u.searchParams.forEach(function (v, k) { if (/token|key|secret|pass|auth|session|sig/i.test(k)) u.searchParams.set(k, 'REDACTED'); }); return u.href; } catch (e) { return String(url); }
  };
  const keep = function (entry) { kept.push(entry); if (kept.length > 500) kept.shift(); return entry; };
  const fetchOriginal = window.fetch && window.fetch.bind(window);
  if (fetchOriginal) {
    window.fetch = function (input, init) {
      const url = redact(typeof input === 'string' ? input : (input && input.url) || String(input));
      const entry = keep({ type: 'fetch', method: ((init && init.method) || (input && input.method) || 'GET').toUpperCase(), url: url, status: null, time: Date.now() });
      return fetchOriginal(input, init).then(function (r) { entry.status = r.status; entry.ms = Date.now() - entry.time; return r; }, function (e) { entry.status = 'failed: ' + (e && e.message || e); throw e; });
    };
  }
  const open = XMLHttpRequest.prototype.open, send = XMLHttpRequest.prototype.send;
  XMLHttpRequest.prototype.open = function (method, url) { this.__forge = { type: 'xhr', method: String(method).toUpperCase(), url: redact(url), status: null }; return open.apply(this, arguments); };
  XMLHttpRequest.prototype.send = function () {
    const entry = this.__forge; if (entry) { entry.time = Date.now(); keep(entry); this.addEventListener('loadend', function () { entry.status = this.status || 'failed'; entry.ms = Date.now() - entry.time; }); }
    return send.apply(this, arguments);
  };
})();"#;

// A random token for one web view: two numbers from the standard library's randomly keyed hasher.
fn fresh_nonce() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        hasher.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

// What a view may navigate to. Frames pass through the same check as the page, so a blank or
// srcdoc frame and a blob are let through; `data:` and `javascript:` are not.
fn navigation_allowed(url: &str) -> bool {
    page_address(url) || url == "about:blank" || url == "about:srcdoc" || url.starts_with("blob:")
}

const REFUSED: &str = "Only http, https and file addresses can be shown";

/// Native web views, one per Preview tab, laid over the app window as child views.
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

fn parse_key(body: &str, nonce: &str) -> Option<KeyForward> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if value.get("n")?.as_str()? != nonce {
        return None;
    }
    Some(KeyForward {
        key: value.get("k")?.as_str()?.to_string(),
        code: value.get("c")?.as_str()?.to_string(),
        ctrl: value.get("ctrl")?.as_bool()?,
        meta: value.get("meta")?.as_bool()?,
        alt: value.get("alt")?.as_bool()?,
        shift: value.get("shift")?.as_bool()?,
    })
}

// Runs `act` on the tab's view when it has one.
fn with_view(tab: WebViewTabId, act: impl FnOnce(&WebView)) {
    VIEWS.with(|views| {
        if let Some(view) = views.borrow().get(&tab.0) {
            act(view);
        }
    });
}

impl WebViewPort for WryWebViews {
    fn create(&self, tab: WebViewTabId, url: &str, surface: Surface) -> Result<(), String> {
        let window = self.window()?;
        if !page_address(url) {
            return Err(REFUSED.to_string());
        }
        let nonce = fresh_nonce();
        let nonce_check = nonce.clone();
        let (nav, load, title, new_window, ipc) =
            (self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone(), self.events_tx.clone());
        let view = WebViewBuilder::new()
            .with_url(url)
            .with_bounds(rect(surface))
            .with_devtools(cfg!(debug_assertions))
            .with_initialization_script(key_script(&nonce))
            .with_initialization_script(CONSOLE_SCRIPT)
            .with_initialization_script(NETWORK_SCRIPT)
            .with_navigation_handler(|url| navigation_allowed(&url))
            .with_new_window_req_handler(move |url, _| {
                if web_address(&url) {
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
                if let Some(key) = parse_key(request.body(), &nonce_check) {
                    let _ = ipc.send(WebViewEvent { tab, kind: WebViewEventKind::Key(key) });
                }
            })
            .build_as_child(&*window)
            .map_err(|error| error.to_string())?;
        VIEWS.with(|views| views.borrow_mut().insert(tab.0, view));
        Ok(())
    }

    fn unreachable(&self, url: &str) -> Option<String> {
        probe::loopback_unreachable(url)
    }

    fn destroy(&self, tab: WebViewTabId) {
        VIEWS.with(|views| views.borrow_mut().remove(&tab.0));
    }

    fn navigate(&self, tab: WebViewTabId, url: &str) -> Result<(), String> {
        if !page_address(url) {
            return Err(REFUSED.to_string());
        }
        VIEWS.with(|views| match views.borrow().get(&tab.0) {
            Some(view) => view.load_url(url).map_err(|error| error.to_string()),
            None => Err("This tab has no page".to_string()),
        })
    }

    fn evaluate(&self, tab: WebViewTabId, script: &str, reply: tokio::sync::oneshot::Sender<Result<String, String>>) {
        // wry calls back with the result serialised as JSON; the callback may be called once only here.
        let reply = Arc::new(Mutex::new(Some(reply)));
        let answer = reply.clone();
        let started = VIEWS.with(|views| match views.borrow().get(&tab.0) {
            Some(view) => view
                .evaluate_script_with_callback(script, move |result| {
                    if let Some(reply) = answer.lock().ok().and_then(|mut reply| reply.take()) {
                        let _ = reply.send(Ok(result));
                    }
                })
                .map_err(|error| error.to_string()),
            None => Err("This tab has no page on screen. Open the session and its Preview tab.".to_string()),
        });
        if let Err(message) = started {
            if let Some(reply) = reply.lock().ok().and_then(|mut reply| reply.take()) {
                let _ = reply.send(Err(message));
            }
        }
    }

    #[cfg(target_os = "macos")]
    fn snapshot(&self, tab: WebViewTabId, reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>) {
        use wry::WebViewExtMacOS;
        let reply = Arc::new(Mutex::new(Some(reply)));
        let answer = reply.clone();
        let started = VIEWS.with(|views| match views.borrow().get(&tab.0) {
            Some(view) => {
                let page = view.webview();
                let handler = block2::RcBlock::new(move |image: *mut objc2_app_kit::NSImage, _error: *mut objc2_foundation::NSError| {
                    // SAFETY: WebKit hands a valid image or null for the length of the call.
                    let png = unsafe { image.as_ref() }.and_then(png_of).ok_or_else(|| "The page could not be captured.".to_string());
                    if let Some(reply) = answer.lock().ok().and_then(|mut reply| reply.take()) {
                        let _ = reply.send(png);
                    }
                });
                // SAFETY: called on the window thread with a live web view; no configuration means the whole visible page.
                unsafe { page.takeSnapshotWithConfiguration_completionHandler(None, &handler) };
                Ok(())
            }
            None => Err("This tab has no page on screen. Open the session and its Preview tab.".to_string()),
        });
        if let Err(message) = started {
            if let Some(reply) = reply.lock().ok().and_then(|mut reply| reply.take()) {
                let _ = reply.send(Err(message));
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn snapshot(&self, _tab: WebViewTabId, reply: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>) {
        let _ = reply.send(Err("Screenshots of the Browser pane are only available on macOS.".to_string()));
    }

    fn back(&self, tab: WebViewTabId) {
        with_view(tab, |view| {
            let _ = view.evaluate_script("history.back()");
        });
    }

    fn forward(&self, tab: WebViewTabId) {
        with_view(tab, |view| {
            let _ = view.evaluate_script("history.forward()");
        });
    }

    fn reload(&self, tab: WebViewTabId) {
        with_view(tab, |view| {
            let _ = view.reload();
        });
    }

    fn set_surface(&self, tab: WebViewTabId, surface: Surface) {
        with_view(tab, |view| {
            let _ = view.set_bounds(rect(surface));
        });
    }

    fn set_visible(&self, tab: WebViewTabId, visible: bool) {
        with_view(tab, |view| {
            let _ = view.set_visible(visible);
        });
    }

    fn focus_app(&self, tab: WebViewTabId) {
        with_view(tab, |view| {
            let _ = view.focus_parent();
        });
    }

    fn take_events(&self) -> Option<UnboundedReceiver<WebViewEvent>> {
        self.events_rx.lock().unwrap().take()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_message_is_read_only_with_the_right_nonce_and_a_wrong_one_is_dropped() {
        let ok = parse_key(r#"{"n":"abc","k":"k","c":"KeyK","ctrl":false,"meta":true,"alt":false,"shift":false}"#, "abc").unwrap();
        assert_eq!((ok.key.as_str(), ok.code.as_str(), ok.meta), ("k", "KeyK", true));
        assert!(parse_key(r#"{"n":"zzz","k":"k","c":"KeyK","ctrl":false,"meta":true,"alt":false,"shift":false}"#, "abc").is_none(), "a page does not know the nonce");
        assert!(parse_key(r#"{"k":"k","c":"KeyK","ctrl":false,"meta":true,"alt":false,"shift":false}"#, "abc").is_none(), "no nonce at all");
        assert!(parse_key("not json", "abc").is_none());
        assert!(parse_key(r#"{"n":"abc","k":1}"#, "abc").is_none());
    }

    #[test]
    fn the_key_script_carries_the_nonce_and_only_trusted_key_presses() {
        let script = key_script("deadbeef");
        assert!(script.contains("\"deadbeef\""), "{script}");
        assert!(script.contains("e.isTrusted"), "a page cannot play a key press itself");
        assert!(script.contains("window.ipc.postMessage.bind"), "the post function is taken before the page can change it");
    }

    #[test]
    fn nonces_differ_from_one_view_to_the_next() {
        let (one, two) = (fresh_nonce(), fresh_nonce());
        assert_eq!(one.len(), 32);
        assert_ne!(one, two);
    }

    #[test]
    fn navigation_follows_the_page_rule_and_lets_blank_frames_and_blobs_through() {
        for ok in ["http://localhost:3000/", "https://example.com", "file:///tmp/a.html", "about:blank", "about:srcdoc", "blob:https://example.com/1234"] {
            assert!(navigation_allowed(ok), "{ok}");
        }
        for refused in ["javascript:alert(1)", "data:text/html,hi", "ftp://x", "about:config", ""] {
            assert!(!navigation_allowed(refused), "{refused:?}");
        }
    }
}
