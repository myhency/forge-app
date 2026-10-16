use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_ports::{BrowserPort, Surface, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};
use forge_shared::tr;

use crate::browser_host::{should_show, ShowInput};
use crate::preview_tabs::{PreviewTabId, PreviewTabs};

/// The Preview tabs of every open session and what to do with their web views. It holds no
/// signals: the workspace keeps one in a signal and calls it with the ports.
#[derive(Debug, Default)]
pub struct PreviewHost {
    sessions: HashMap<LiveSessionId, PreviewTabs>,
}

impl PreviewHost {
    pub fn tabs(&self, id: LiveSessionId) -> PreviewTabs {
        self.sessions.get(&id).cloned().unwrap_or_default()
    }

    pub fn change(&mut self, id: LiveSessionId, change: impl FnOnce(&mut PreviewTabs)) {
        change(self.sessions.entry(id).or_default());
    }

    /// Shows `url` in the tab: the tab's first address makes its web view, later ones navigate it.
    /// The address must be one a web view may load; a file address must name an existing file.
    /// When the web view cannot be made the address opens in the system browser, and the error
    /// says why.
    pub fn go(
        &mut self,
        id: LiveSessionId,
        tab: PreviewTabId,
        url: &str,
        surface: Option<Surface>,
        webviews: &dyn WebViewPort,
        browser: &dyn BrowserPort,
    ) -> Result<(), String> {
        if !forge_entities::page_address(url) {
            return Err(tr!("Enter a web address, such as localhost:3000").to_string());
        }
        if let Some(path) = forge_entities::path_of_file_address(url) {
            if !std::path::Path::new(&path).is_file() {
                return Err(tr!("This file cannot be shown.").to_string());
            }
        }
        // Asked before the view loads: the failure shows at once, the page in the view stays as it
        // was, and the history only ever holds pages that loaded.
        if let Some(reason) = webviews.unreachable(url) {
            self.change(id, |tabs| tabs.fail(tab, url.to_string(), reason));
            return Ok(());
        }
        let first = self.tabs(id).current(tab).is_none();
        let view = WebViewTabId(tab.0);
        let result = if first {
            webviews.create(view, url, surface.unwrap_or(Surface { x: 0.0, y: 0.0, width: 1.0, height: 1.0 }))
        } else {
            webviews.navigate(view, url)
        };
        if let Err(reason) = result {
            let _ = browser.open(url);
            return Err(format!("{} {reason}", tr!("The page could not be shown here, so it was opened in your browser.")));
        }
        // Only a page that is there is recorded; a tab that failed to get one stays empty and tries again.
        self.change(id, |tabs| tabs.loaded(tab, url.to_string()));
        Ok(())
    }

    pub fn close_tab(&mut self, id: LiveSessionId, tab: PreviewTabId, webviews: &dyn WebViewPort) {
        webviews.destroy(WebViewTabId(tab.0));
        self.change(id, |tabs| tabs.close(tab));
    }

    /// A session closed: every web view it had goes.
    pub fn forget_session(&mut self, id: LiveSessionId, webviews: &dyn WebViewPort) {
        if let Some(tabs) = self.sessions.remove(&id) {
            for tab in tabs.ids() {
                webviews.destroy(WebViewTabId(tab.0));
            }
        }
    }

    /// Takes a page event into the tab it names. A request for a new window comes back with the
    /// session that asked, so the workspace can open a tab for it.
    pub fn apply(&mut self, event: WebViewEvent) -> Option<(LiveSessionId, String)> {
        let tab = PreviewTabId(event.tab.0);
        let (&session, _) = self.sessions.iter().find(|(_, tabs)| tabs.contains(tab))?;
        let tabs = self.sessions.get_mut(&session)?;
        match event.kind {
            WebViewEventKind::Navigated(url) => tabs.loaded(tab, url),
            WebViewEventKind::Title(text) => tabs.set_title(tab, text),
            WebViewEventKind::Loading(loading) => tabs.set_loading(tab, loading),
            WebViewEventKind::LoadFailed { url, reason } => tabs.fail(tab, url, reason),
            // A page may only open a web address: a `file://` link would open a local file.
            WebViewEventKind::NewWindow(url) => return forge_entities::web_address(&url).then_some((session, url)),
            WebViewEventKind::Key(_) => {}
        }
        None
    }

    /// For every tab, whether its web view may be seen now.
    pub fn visibility(
        &self,
        active: Option<LiveSessionId>,
        panel_open: &dyn Fn(LiveSessionId) -> bool,
        overlay_open: bool,
        surface: Option<Surface>,
    ) -> Vec<(WebViewTabId, bool)> {
        let mut plan = Vec::new();
        for (&session, tabs) in &self.sessions {
            for tab in tabs.ids() {
                let show = should_show(&ShowInput {
                    session_active: active == Some(session),
                    panel_open: panel_open(session),
                    tab_active: tabs.active() == Some(tab),
                    overlay_open,
                    surface,
                    failed: tabs.failure(tab).is_some(),
                });
                plan.push((WebViewTabId(tab.0), show));
            }
        }
        plan
    }
}

#[cfg(test)]
mod tests {
    use forge_ports::{MockBrowserPort, MockWebViewPort, WebViewEventKind};

    use super::*;

    const SESSION: LiveSessionId = LiveSessionId(1);

    fn surface() -> Surface {
        Surface { x: 10.0, y: 20.0, width: 400.0, height: 300.0 }
    }

    fn reachable_views() -> MockWebViewPort {
        let mut views = MockWebViewPort::new();
        views.expect_unreachable().returning(|_| None);
        views
    }

    fn quiet_browser() -> MockBrowserPort {
        MockBrowserPort::new()
    }

    fn temp_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-preview-host-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, "<html></html>").unwrap();
        path
    }

    #[test]
    fn the_first_address_creates_the_web_view_and_the_next_one_navigates() {
        let mut host = PreviewHost::default();
        let tab = {
            let mut made = None;
            host.change(SESSION, |tabs| made = Some(tabs.open()));
            made.unwrap()
        };
        let mut views = reachable_views();
        views
            .expect_create()
            .withf(move |id, url, shown| *id == WebViewTabId(tab.0) && url == "http://localhost:3000" && *shown == surface())
            .times(1)
            .returning(|_, _, _| Ok(()));
        views
            .expect_navigate()
            .withf(move |id, url| *id == WebViewTabId(tab.0) && url == "http://localhost:3000/next")
            .times(1)
            .returning(|_, _| Ok(()));
        let browser = quiet_browser();

        host.go(SESSION, tab, "http://localhost:3000", Some(surface()), &views, &browser).unwrap();
        host.go(SESSION, tab, "http://localhost:3000/next", Some(surface()), &views, &browser).unwrap();

        assert_eq!(host.tabs(SESSION).current(tab).as_deref(), Some("http://localhost:3000/next"));
    }

    #[test]
    fn an_address_with_another_scheme_never_reaches_the_web_view() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let (views, browser) = (MockWebViewPort::new(), quiet_browser());
        for bad in ["javascript:alert(1)", "data:text/html,hi", "ftp://x", "about:blank", ""] {
            assert!(host.go(SESSION, tab.unwrap(), bad, Some(surface()), &views, &browser).is_err(), "{bad:?}");
        }
        assert_eq!(host.tabs(SESSION).current(tab.unwrap()), None);
    }

    #[test]
    fn a_file_address_must_name_an_existing_file() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let tab = tab.unwrap();
        let file = temp_file("page file#1.html");
        let folder = file.parent().unwrap().to_string_lossy().to_string();
        let mut views = reachable_views();
        views.expect_create().times(1).returning(|_, _, _| Ok(()));
        let browser = quiet_browser();

        let missing = forge_entities::file_address(&format!("{folder}/missing.html"));
        assert!(host.go(SESSION, tab, &missing, Some(surface()), &views, &browser).is_err());
        assert!(host.go(SESSION, tab, &forge_entities::file_address(&folder), Some(surface()), &views, &browser).is_err());
        host.go(SESSION, tab, &forge_entities::file_address(&file.to_string_lossy()), Some(surface()), &views, &browser).unwrap();
    }

    #[test]
    fn a_web_view_that_cannot_be_made_opens_the_system_browser_and_says_why() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let mut views = reachable_views();
        views.expect_create().times(1).returning(|_, _, _| Err("WebView2 is missing".to_string()));
        let mut browser = MockBrowserPort::new();
        browser.expect_open().withf(|url| url == "http://localhost:3000").times(1).returning(|_| Ok(()));

        let error = host.go(SESSION, tab.unwrap(), "http://localhost:3000", Some(surface()), &views, &browser).unwrap_err();

        assert!(error.contains("WebView2 is missing"), "{error}");
    }

    #[test]
    fn a_failed_create_leaves_the_tab_without_an_address_so_the_next_try_creates_again() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let tab = tab.unwrap();
        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = calls.clone();
        let mut views = reachable_views();
        views.expect_create().times(2).returning(move |_, _, _| {
            if counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 { Err("WebView2 is missing".to_string()) } else { Ok(()) }
        });
        views.expect_navigate().never();
        let mut browser = MockBrowserPort::new();
        browser.expect_open().times(1).returning(|_| Ok(()));

        assert!(host.go(SESSION, tab, "http://localhost:3000", Some(surface()), &views, &browser).is_err());
        assert_eq!(host.tabs(SESSION).current(tab), None, "nothing is recorded for a page that is not there");
        host.go(SESSION, tab, "http://localhost:3000", Some(surface()), &views, &browser).unwrap();
        assert_eq!(host.tabs(SESSION).current(tab).as_deref(), Some("http://localhost:3000"));
    }

    #[test]
    fn an_unreachable_address_fails_the_tab_at_once_and_leaves_its_page_and_history_alone() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let tab = tab.unwrap();
        let mut views = MockWebViewPort::new();
        views.expect_unreachable().withf(|url| url == "http://localhost:9").returning(|_| Some("refused".to_string()));
        views.expect_unreachable().returning(|_| None);
        views.expect_create().times(1).returning(|_, _, _| Ok(()));
        views.expect_navigate().never();
        let browser = quiet_browser();

        host.go(SESSION, tab, "http://localhost:3000", Some(surface()), &views, &browser).unwrap();
        host.go(SESSION, tab, "http://localhost:9", Some(surface()), &views, &browser).unwrap();

        let tabs = host.tabs(SESSION);
        assert_eq!(tabs.current(tab).as_deref(), Some("http://localhost:3000"), "the page in the view is still the current one");
        assert!(!tabs.can_back(tab), "a page that never loaded is not in the history");
        assert_eq!(tabs.failure(tab), Some(("http://localhost:9".to_string(), "refused".to_string())));
    }

    #[test]
    fn an_unreachable_first_address_makes_no_web_view_and_the_next_try_makes_it() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let tab = tab.unwrap();
        let mut views = MockWebViewPort::new();
        views.expect_unreachable().times(1).returning(|_| Some("refused".to_string()));
        views.expect_unreachable().times(1).returning(|_| None);
        views.expect_create().times(1).returning(|_, _, _| Ok(()));
        let browser = quiet_browser();

        host.go(SESSION, tab, "http://localhost:9", Some(surface()), &views, &browser).unwrap();
        assert_eq!(host.tabs(SESSION).current(tab), None);
        assert!(host.tabs(SESSION).failure(tab).is_some());
        host.go(SESSION, tab, "http://localhost:9", Some(surface()), &views, &browser).unwrap();

        assert_eq!(host.tabs(SESSION).current(tab).as_deref(), Some("http://localhost:9"));
        assert_eq!(host.tabs(SESSION).failure(tab), None, "a page that loads clears the failure");
    }

    #[test]
    fn a_page_may_only_ask_for_a_web_address_in_a_new_window() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let id = WebViewTabId(tab.unwrap().0);
        for refused in ["file:///Users/x/notes.html", "javascript:alert(1)", "data:text/html,hi"] {
            assert_eq!(host.apply(WebViewEvent { tab: id, kind: WebViewEventKind::NewWindow(refused.to_string()) }), None, "{refused}");
        }
        assert!(host.apply(WebViewEvent { tab: id, kind: WebViewEventKind::NewWindow("https://example.com".to_string()) }).is_some());
    }

    #[test]
    fn closing_a_tab_destroys_its_web_view_and_closing_a_session_destroys_them_all() {
        let mut host = PreviewHost::default();
        let (mut one, mut two, mut three) = (None, None, None);
        host.change(SESSION, |tabs| {
            one = Some(tabs.open());
            two = Some(tabs.open());
        });
        host.change(LiveSessionId(2), |tabs| three = Some(tabs.open()));
        let (one, two, three) = (one.unwrap(), two.unwrap(), three.unwrap());
        let mut views = MockWebViewPort::new();
        views.expect_destroy().withf(move |id| *id == WebViewTabId(one.0)).times(1).return_const(());
        views.expect_destroy().withf(move |id| *id == WebViewTabId(two.0)).times(1).return_const(());

        host.close_tab(SESSION, one, &views);
        assert!(!host.tabs(SESSION).contains(one));
        host.forget_session(SESSION, &views);

        assert!(host.tabs(SESSION).is_empty());
        assert!(host.tabs(LiveSessionId(2)).contains(three), "another session keeps its tabs");
    }

    #[test]
    fn page_events_update_the_tab_they_name_and_an_unknown_tab_is_ignored() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let tab = tab.unwrap();
        let id = WebViewTabId(tab.0);
        let event = |kind| WebViewEvent { tab: id, kind };

        host.apply(event(WebViewEventKind::Navigated("http://localhost:3000/".to_string())));
        host.apply(event(WebViewEventKind::Title("My app".to_string())));
        host.apply(event(WebViewEventKind::Loading(true)));
        let tabs = host.tabs(SESSION);
        assert_eq!(tabs.current(tab).as_deref(), Some("http://localhost:3000/"));
        assert_eq!(tabs.label(tab), "My app");
        assert!(tabs.loading(tab));

        host.apply(event(WebViewEventKind::LoadFailed { url: "http://localhost:9".to_string(), reason: "refused".to_string() }));
        assert_eq!(host.tabs(SESSION).failure(tab).map(|(url, _)| url).as_deref(), Some("http://localhost:9"));

        let ignored = host.apply(WebViewEvent { tab: WebViewTabId(u64::MAX), kind: WebViewEventKind::Title("x".to_string()) });
        assert_eq!(ignored, None);
    }

    #[test]
    fn a_new_window_request_names_the_session_that_asked() {
        let mut host = PreviewHost::default();
        let mut tab = None;
        host.change(SESSION, |tabs| tab = Some(tabs.open()));
        let asked = host.apply(WebViewEvent {
            tab: WebViewTabId(tab.unwrap().0),
            kind: WebViewEventKind::NewWindow("https://example.com/popup".to_string()),
        });
        assert_eq!(asked, Some((SESSION, "https://example.com/popup".to_string())));
    }

    #[test]
    fn only_the_active_tab_of_the_active_session_with_its_panel_open_is_visible() {
        let mut host = PreviewHost::default();
        let (mut a, mut b, mut c) = (None, None, None);
        host.change(SESSION, |tabs| {
            a = Some(tabs.open());
            b = Some(tabs.open());
        });
        host.change(LiveSessionId(2), |tabs| c = Some(tabs.open()));
        let (a, b, c) = (a.unwrap(), b.unwrap(), c.unwrap());
        host.change(SESSION, |tabs| tabs.select(a));
        let open = |_: LiveSessionId| true;

        let shown = |plan: &[(WebViewTabId, bool)], tab: PreviewTabId| plan.iter().find(|(id, _)| *id == WebViewTabId(tab.0)).map(|(_, show)| *show);

        let plan = host.visibility(Some(SESSION), &open, false, Some(surface()));
        assert_eq!(shown(&plan, a), Some(true));
        assert_eq!(shown(&plan, b), Some(false), "another tab");
        assert_eq!(shown(&plan, c), Some(false), "another session");

        let plan = host.visibility(Some(SESSION), &|_| false, false, Some(surface()));
        assert_eq!(shown(&plan, a), Some(false), "panel closed");
        let plan = host.visibility(Some(SESSION), &open, true, Some(surface()));
        assert_eq!(shown(&plan, a), Some(false), "an overlay is open");
        let plan = host.visibility(Some(SESSION), &open, false, None);
        assert_eq!(shown(&plan, a), Some(false), "no surface yet");
        let plan = host.visibility(None, &open, false, Some(surface()));
        assert_eq!(shown(&plan, a), Some(false), "no session on screen");
    }
}
