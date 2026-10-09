use std::path::PathBuf;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Where the files of the local MCP servers are, for the Developer page. Only finds paths; the page opens them.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait McpLogPort: Send + Sync {
    /// The newest log file Claude Code wrote for the server, over every project; `None` when there is none.
    fn latest_log(&self, server: &str) -> Option<PathBuf>;
    /// The file that defines the person's own servers (`~/.claude.json`).
    fn config_file(&self) -> PathBuf;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mock_can_find_no_log() {
        let mut mock = MockMcpLogPort::new();
        mock.expect_latest_log().returning(|_| None);
        assert_eq!(mock.latest_log("fs"), None);
    }
}
