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

#[derive(Debug, Clone, PartialEq)]
struct PreviewTab {
    id: PreviewTabId,
    history: Vec<String>,
    // Index into `history` of the current address; meaningless while the history is empty.
    position: usize,
}

/// The tabs of one Preview panel, each with its own history. The page itself opens in the
/// system browser, so a history is only the addresses that were opened from the tab.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewTabs {
    tabs: Vec<PreviewTab>,
    active: Option<PreviewTabId>,
    next: u64,
}

impl PreviewTabs {
    /// Adds an empty tab at the right and makes it the active one.
    pub fn open(&mut self) -> PreviewTabId {
        let id = PreviewTabId(self.next);
        self.next += 1;
        self.tabs.push(PreviewTab { id, history: Vec::new(), position: 0 });
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
        if self.tabs.iter().any(|tab| tab.id == id) {
            self.active = Some(id);
        }
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

    /// Makes `url` the tab's current address, dropping any forward pages. The address it already
    /// has is not added again.
    pub fn navigate(&mut self, id: PreviewTabId, url: String) {
        let Some(tab) = self.tab_mut(id) else { return };
        if tab.history.get(tab.position) == Some(&url) {
            return;
        }
        if !tab.history.is_empty() {
            tab.history.truncate(tab.position + 1);
        }
        tab.history.push(url);
        tab.position = tab.history.len() - 1;
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

    /// Steps back and returns the address now current, or `None` at the start.
    pub fn back(&mut self, id: PreviewTabId) -> Option<String> {
        if !self.can_back(id) {
            return None;
        }
        self.tab_mut(id)?.position -= 1;
        self.current(id)
    }

    pub fn forward(&mut self, id: PreviewTabId) -> Option<String> {
        if !self.can_forward(id) {
            return None;
        }
        self.tab_mut(id)?.position += 1;
        self.current(id)
    }

    /// The host (and port) of the current address, or "New tab".
    pub fn label(&self, id: PreviewTabId) -> String {
        let Some(url) = self.current(id) else { return "New tab".to_string() };
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
    fn navigating_builds_a_history_that_back_and_forward_walk() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        tabs.navigate(tab, "http://a.test/".to_string());
        tabs.navigate(tab, "http://b.test/".to_string());
        assert_eq!(tabs.current(tab).as_deref(), Some("http://b.test/"));
        assert!(tabs.can_back(tab) && !tabs.can_forward(tab));
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a.test/"));
        assert!(!tabs.can_back(tab) && tabs.can_forward(tab));
        assert_eq!(tabs.back(tab), None, "already at the start");
        assert_eq!(tabs.forward(tab).as_deref(), Some("http://b.test/"));
        assert_eq!(tabs.forward(tab), None, "already at the end");
    }

    #[test]
    fn navigating_after_going_back_drops_the_forward_pages_and_the_same_address_twice_is_one_entry() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        for url in ["http://a/", "http://b/", "http://c/"] {
            tabs.navigate(tab, url.to_string());
        }
        tabs.back(tab);
        tabs.back(tab);
        tabs.navigate(tab, "http://d/".to_string());
        assert!(!tabs.can_forward(tab));
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a/"));
        tabs.forward(tab);
        tabs.navigate(tab, "http://d/".to_string());
        assert_eq!(tabs.back(tab).as_deref(), Some("http://a/"), "no duplicate entry");
    }

    #[test]
    fn the_label_is_the_host_of_the_current_address() {
        let mut tabs = PreviewTabs::default();
        let tab = tabs.open();
        tabs.navigate(tab, "http://localhost:3000/app?x=1".to_string());
        assert_eq!(tabs.label(tab), "localhost:3000");
        tabs.navigate(tab, "https://example.com".to_string());
        assert_eq!(tabs.label(tab), "example.com");
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
        tabs.close(PreviewTabId(999));
        tabs.navigate(PreviewTabId(999), "http://x/".to_string());
        assert_eq!(tabs.back(PreviewTabId(999)), None);
        tabs.close(a);
        assert!(tabs.is_empty());
        assert_eq!(tabs.active(), None);
    }
}
