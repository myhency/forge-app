use forge_shared::tr;
/// A web address from what was typed in the address bar: an address with an `http` or `https`
/// scheme is kept; `localhost`, `127.0.0.1` and `[::1]` get `http://`, any other bare host
/// `https://`. Anything else (another scheme, spaces, nothing) gives `None`.
pub fn normalize_url(input: &str) -> Option<String> {
    let text = input.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return None;
    }
    if let Some((scheme, rest)) = text.split_once("://") {
        let web = scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https");
        return (web && !rest.is_empty()).then(|| text.to_string());
    }
    let authority = text.split(['/', '?', '#']).next().unwrap_or("");
    if authority.is_empty() {
        return None;
    }
    // A port is digits; `javascript:alert(1)` and `mailto:a@b.c` have a colon but no port.
    if !authority.starts_with('[') {
        if let Some((_, port)) = authority.rsplit_once(':') {
            if port.is_empty() || !port.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
        }
    }
    let host = authority.split(':').next().unwrap_or("");
    let local = host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || authority.starts_with("[::1]");
    Some(format!("{}://{text}", if local { "http" } else { "https" }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewTabId(pub u64);

// One counter for the whole process, so a tab id is unique across sessions and names its web view.
static NEXT_TAB: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq)]
struct PreviewTab {
    id: PreviewTabId,
    history: Vec<String>,
    // Index into `history` of the current address; meaningless while the history is empty.
    position: usize,
    title: Option<String>,
    loading: bool,
    // The address that could not be reached and why.
    failure: Option<(String, String)>,
}

/// The tabs of one Preview panel. The page owns its real history; this keeps a copy of the
/// addresses it reported, so the back and forward buttons know when they can be used.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewTabs {
    tabs: Vec<PreviewTab>,
    active: Option<PreviewTabId>,
}

impl PreviewTabs {
    /// Adds an empty tab at the right and makes it the active one.
    pub fn open(&mut self) -> PreviewTabId {
        let id = PreviewTabId(NEXT_TAB.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
        self.tabs.push(PreviewTab { id, history: Vec::new(), position: 0, title: None, loading: false, failure: None });
        self.active = Some(id);
        id
    }

    pub fn close(&mut self, id: PreviewTabId) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else { return };
        self.tabs.remove(index);
        if self.active == Some(id) {
            self.active = self.tabs.get(index).or_else(|| self.tabs.last()).map(|tab| tab.id);
        }
    }

    pub fn select(&mut self, id: PreviewTabId) {
        if self.contains(id) {
            self.active = Some(id);
        }
    }

    pub fn contains(&self, id: PreviewTabId) -> bool {
        self.tabs.iter().any(|tab| tab.id == id)
    }

    pub fn ids(&self) -> Vec<PreviewTabId> {
        self.tabs.iter().map(|tab| tab.id).collect()
    }

    pub fn active(&self) -> Option<PreviewTabId> {
        self.active
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    fn tab(&self, id: PreviewTabId) -> Option<&PreviewTab> {
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn tab_mut(&mut self, id: PreviewTabId) -> Option<&mut PreviewTab> {
        self.tabs.iter_mut().find(|tab| tab.id == id)
    }

    /// A page started loading `url`. The address the page went back to or forward to moves the
    /// position; the address it already has changes nothing; any other drops the forward pages
    /// and is added.
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

    pub fn set_title(&mut self, id: PreviewTabId, title: String) {
        if let Some(tab) = self.tab_mut(id) {
            tab.title = Some(title).filter(|text| !text.is_empty());
        }
    }

    pub fn title(&self, id: PreviewTabId) -> Option<String> {
        self.tab(id)?.title.clone()
    }

    pub fn set_loading(&mut self, id: PreviewTabId, loading: bool) {
        if let Some(tab) = self.tab_mut(id) {
            tab.loading = loading;
        }
    }

    pub fn loading(&self, id: PreviewTabId) -> bool {
        self.tab(id).is_some_and(|tab| tab.loading)
    }

    /// The address could not be reached.
    pub fn fail(&mut self, id: PreviewTabId, url: String, reason: String) {
        if let Some(tab) = self.tab_mut(id) {
            tab.loading = false;
            tab.failure = Some((url, reason));
        }
    }

    pub fn failure(&self, id: PreviewTabId) -> Option<(String, String)> {
        self.tab(id)?.failure.clone()
    }

    pub fn current(&self, id: PreviewTabId) -> Option<String> {
        let tab = self.tab(id)?;
        tab.history.get(tab.position).cloned()
    }

    pub fn can_back(&self, id: PreviewTabId) -> bool {
        self.tab(id).is_some_and(|tab| !tab.history.is_empty() && tab.position > 0)
    }

    pub fn can_forward(&self, id: PreviewTabId) -> bool {
        self.tab(id).is_some_and(|tab| tab.position + 1 < tab.history.len())
    }

    /// The page's title, else the host (and port) of its address, else "New tab".
    pub fn label(&self, id: PreviewTabId) -> String {
        if let Some(title) = self.title(id) {
            return title;
        }
        let Some(url) = self.current(id) else { return tr!("New tab").to_string() };
        let rest = url.split_once("://").map_or(url.as_str(), |(_, rest)| rest);
        rest.split(['/', '?', '#']).next().unwrap_or(rest).to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_hosts_get_http_and_other_hosts_get_https() {
        assert_eq!(normalize_url("localhost:3000").as_deref(), Some("http://localhost:3000"));
        assert_eq!(normalize_url("localhost").as_deref(), Some("http://localhost"));
        assert_eq!(normalize_url("LOCALHOST:8080/a?b=1").as_deref(), Some("http://LOCALHOST:8080/a?b=1"));
        assert_eq!(normalize_url("127.0.0.1:5173").as_deref(), Some("http://127.0.0.1:5173"));
        assert_eq!(normalize_url("[::1]:3000").as_deref(), Some("http://[::1]:3000"));
        assert_eq!(normalize_url("example.com").as_deref(), Some("https://example.com"));
        assert_eq!(normalize_url("  example.com/path  ").as_deref(), Some("https://example.com/path"));
    }

    #[test]
    fn an_address_with_a_web_scheme_is_kept_as_typed() {
        assert_eq!(normalize_url("http://example.com").as_deref(), Some("http://example.com"));
        assert_eq!(normalize_url("HTTPS://example.com/x").as_deref(), Some("HTTPS://example.com/x"));
    }

    #[test]
    fn input_that_is_not_a_web_address_gives_none() {
        for input in ["", "   ", "file:///etc/passwd", "javascript:alert(1)", "ftp://x", "foo bar", "mailto:a@b.c", "http://", "a:b"] {
            assert_eq!(normalize_url(input), None, "{input:?}");
        }
    }

    #[test]
    fn a_new_tab_is_empty_and_active() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        assert_eq!(tabs.active(), Some(tab));
        assert_eq!(tabs.current(tab), None);
        assert_eq!(tabs.label(tab), "New tab");
        assert!(!tabs.can_back(tab) && !tabs.can_forward(tab));
    }

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
    fn going_forward_again_follows_the_next_entry() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        for url in ["http://a", "http://b", "http://c"] {
            tabs.loaded(tab, url.into());
        }
        tabs.loaded(tab, "http://b".into());
        assert!(tabs.can_forward(tab));
        tabs.loaded(tab, "http://c".into());
        assert_eq!(tabs.current(tab).as_deref(), Some("http://c"));
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
        tabs.set_loading(tab, true);
        tabs.fail(tab, "http://a".into(), "no".into());
        assert!(tabs.is_empty());
        assert!(!tabs.contains(tab));
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
        tabs.loaded(tab, "https://example.com".into());
        assert_eq!(tabs.label(tab), "example.com", "a new page forgets the old title");
    }

    #[test]
    fn a_failure_is_kept_until_the_next_page_loads() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        tabs.set_loading(tab, true);
        tabs.fail(tab, "http://localhost:9".into(), "refused".into());
        assert_eq!(tabs.failure(tab), Some(("http://localhost:9".to_string(), "refused".to_string())));
        assert!(!tabs.loading(tab), "a failed load is not loading");
        tabs.loaded(tab, "http://localhost:3000".into());
        assert_eq!(tabs.failure(tab), None);
    }

    #[test]
    fn tab_ids_are_unique_across_panels() {
        let (mut one, mut two) = (PreviewTabs::default(), PreviewTabs::default());
        assert_ne!(one.open(), two.open());
    }

    #[test]
    fn closing_the_active_tab_selects_a_neighbour_and_unknown_tabs_are_ignored() {
        let mut tabs = PreviewTabs::default();
        let (a, b, c) = (tabs.open(), tabs.open(), tabs.open());
        tabs.select(b);
        tabs.close(b);
        assert_eq!(tabs.active(), Some(c));
        tabs.close(c);
        assert_eq!(tabs.active(), Some(a));
        tabs.close(PreviewTabId(u64::MAX));
        tabs.loaded(PreviewTabId(u64::MAX), "http://x/".to_string());
        tabs.close(a);
        assert!(tabs.is_empty());
        assert_eq!(tabs.active(), None);
    }
}
