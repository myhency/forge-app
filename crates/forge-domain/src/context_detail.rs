/// What a row of the context breakdown stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextKind {
    Used,
    Free,
    Buffer,
    Deferred,
}

impl ContextKind {
    /// `None` for a word the app does not know, so that row is left out.
    pub fn from_sdk(kind: &str) -> Option<ContextKind> {
        match kind {
            "used" => Some(ContextKind::Used),
            "free" => Some(ContextKind::Free),
            "buffer" => Some(ContextKind::Buffer),
            "deferred" => Some(ContextKind::Deferred),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRow {
    pub name: String,
    pub tokens: u64,
    pub kind: ContextKind,
}

/// An MCP tool (grouped by its server) or an agent (no group) and its cost in the context.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextItem {
    pub group: Option<String>,
    pub name: String,
    pub tokens: u64,
}

/// The breakdown behind the context ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBreakdown {
    pub rows: Vec<ContextRow>,
    pub mcp_tools: Vec<ContextItem>,
    pub agents: Vec<ContextItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpStatus {
    Connected,
    NeedsAuth,
    Failed,
    Pending,
    Disabled,
}

impl McpStatus {
    /// Any word the app does not know counts as `Pending`.
    pub fn from_sdk(status: &str) -> McpStatus {
        match status {
            "connected" => McpStatus::Connected,
            "failed" => McpStatus::Failed,
            "needs-auth" => McpStatus::NeedsAuth,
            "disabled" => McpStatus::Disabled,
            _ => McpStatus::Pending,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerState {
    pub name: String,
    pub status: McpStatus,
    pub scope: Option<String>,
    pub source: Option<String>,
    /// `Some(false)` for a local process, `Some(true)` for a network server, `None` when unknown.
    pub remote: Option<bool>,
    pub tool_count: u64,
    pub error: Option<String>,
}

impl McpServerState {
    pub fn remote_from_transport(transport: Option<&str>) -> Option<bool> {
        match transport? {
            "stdio" => Some(false),
            "http" | "sse" | "claudeai-proxy" => Some(true),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unknown_kind_and_an_unknown_status_are_handled() {
        assert_eq!(ContextKind::from_sdk("deferred"), Some(ContextKind::Deferred));
        assert_eq!(ContextKind::from_sdk("weird"), None);
        assert_eq!(McpStatus::from_sdk("needs-auth"), McpStatus::NeedsAuth);
        assert_eq!(McpStatus::from_sdk("connected"), McpStatus::Connected);
        assert_eq!(McpStatus::from_sdk("failed"), McpStatus::Failed);
        assert_eq!(McpStatus::from_sdk("disabled"), McpStatus::Disabled);
        assert_eq!(McpStatus::from_sdk("pending"), McpStatus::Pending);
        assert_eq!(McpStatus::from_sdk("something-new"), McpStatus::Pending);
    }

    #[test]
    fn remote_follows_the_transport() {
        assert_eq!(McpServerState::remote_from_transport(Some("stdio")), Some(false));
        for transport in ["http", "sse", "claudeai-proxy"] {
            assert_eq!(McpServerState::remote_from_transport(Some(transport)), Some(true));
        }
        assert_eq!(McpServerState::remote_from_transport(Some("sdk")), None);
        assert_eq!(McpServerState::remote_from_transport(None), None);
    }
}
