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
            return Err(REFUSED.to_string());
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

    fn take_events(&self) -> Option<UnboundedReceiver<WebViewEvent>> {
        self.events_rx.lock().unwrap().take()
    }
}

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
