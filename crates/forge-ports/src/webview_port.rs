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
    /// Why `url` cannot be reached, for an address on this computer that nothing answers; `None`
    /// when it can be or is not checked. It may wait a moment, so the caller asks before it loads.
    fn unreachable(&self, url: &str) -> Option<String>;
    fn destroy(&self, tab: WebViewTabId);
    fn navigate(&self, tab: WebViewTabId, url: &str) -> Result<(), String>;
    fn back(&self, tab: WebViewTabId);
    fn forward(&self, tab: WebViewTabId);
    fn reload(&self, tab: WebViewTabId);
    fn set_surface(&self, tab: WebViewTabId, surface: Surface);
    fn set_visible(&self, tab: WebViewTabId, visible: bool);
    /// Gives the keyboard back to the app's own view (the page it came from keeps the mouse).
    fn focus_app(&self, tab: WebViewTabId);
    /// The stream of page events. It can be taken once; later calls give `None`.
    fn take_events(&self) -> Option<UnboundedReceiver<WebViewEvent>>;
}

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
