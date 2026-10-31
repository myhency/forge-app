use std::path::PathBuf;

use forge_ports::{BrowserPort, McpLogPort};
use forge_shared::tr;

/// The log file to open for a server, or the notice that there is none yet.
pub fn log_to_open(port: &dyn McpLogPort, server: &str) -> Result<PathBuf, String> {
    port.latest_log(server).ok_or_else(|| tr!("No log for {name} yet. Claude Code writes one when a session starts the server.", name = server))
}

/// Opens a log with the default app. Few computers have one for `.jsonl`, so on a Mac a failure is tried again in
/// TextEdit; the first error is the one to show when that fails too.
pub fn open_log(browser: &dyn BrowserPort, path: &std::path::Path) -> Result<(), String> {
    match browser.open_with(path, None) {
        Err(first) if cfg!(target_os = "macos") => browser.open_with(path, Some("TextEdit")).map_err(|_| first),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_ports::{MockBrowserPort, MockMcpLogPort};

    #[test]
    fn a_log_that_opens_with_the_default_app_is_done() {
        let mut browser = MockBrowserPort::new();
        browser.expect_open_with().withf(|_, app| app.is_none()).times(1).returning(|_, _| Ok(()));
        assert_eq!(open_log(&browser, std::path::Path::new("/c/a.jsonl")), Ok(()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn with_no_default_app_a_mac_tries_textedit() {
        let mut browser = MockBrowserPort::new();
        browser.expect_open_with().withf(|_, app| app.is_none()).returning(|_, _| Err("no app".to_string()));
        browser.expect_open_with().withf(|_, app| *app == Some("TextEdit")).times(1).returning(|_, _| Ok(()));
        assert_eq!(open_log(&browser, std::path::Path::new("/c/a.jsonl")), Ok(()));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn when_both_fail_the_first_error_is_shown() {
        let mut browser = MockBrowserPort::new();
        browser.expect_open_with().withf(|_, app| app.is_none()).returning(|_, _| Err("no app".to_string()));
        browser.expect_open_with().withf(|_, app| app.is_some()).returning(|_, _| Err("no textedit".to_string()));
        assert_eq!(open_log(&browser, std::path::Path::new("/c/a.jsonl")), Err("no app".to_string()));
    }

    #[test]
    fn a_server_with_a_log_gives_its_path() {
        let mut port = MockMcpLogPort::new();
        port.expect_latest_log().withf(|name| name == "fs").returning(|_| Some(PathBuf::from("/c/a.jsonl")));
        assert_eq!(log_to_open(&port, "fs"), Ok(PathBuf::from("/c/a.jsonl")));
    }

    #[test]
    fn a_server_without_a_log_gives_a_notice_with_its_name() {
        let mut port = MockMcpLogPort::new();
        port.expect_latest_log().returning(|_| None);
        let message = log_to_open(&port, "fs").unwrap_err();
        assert!(message.contains("fs"), "{message}");
    }
}
