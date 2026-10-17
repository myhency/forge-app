mod browser_port;
mod config_port;
mod connector_catalog_port;
mod extension_ports;
mod file_picker_port;
mod files_port;
mod folder_picker_port;
mod git_port;
mod mcp_log_port;
mod mcp_manager_port;
mod notify_port;
mod plugin_manager_port;
mod project_port;
mod session_port;
mod sidecar_factory_port;
mod skill_catalog_port;
mod sidecar_port;
mod terminal_port;
mod usage_stats_port;
mod webview_port;
mod worktree_port;

pub use browser_port::BrowserPort;
pub use webview_port::{KeyForward, Surface, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};
pub use worktree_port::{BranchList, WorktreeError, WorktreePort, WorktreeState};
pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use connector_catalog_port::{ConnectorCatalogError, ConnectorCatalogPort};
pub use mcp_log_port::McpLogPort;
pub use mcp_manager_port::{McpManagerError, McpManagerPort};
pub use notify_port::{ClickHandler, NotifyPort};
pub use files_port::{ContentHit, SearchOptions, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};
pub use git_port::{BranchInfo, ChangeSource, ChangeStatus, ChangedFile, CommitInfo, DiffOptions, FileChange, GitError, GitPort, PatchState};
pub use extension_ports::{
    McpRegistryError, McpRegistryPort, PluginRepositoryError, PluginRepositoryPort,
    SkillRepositoryError, SkillRepositoryPort,
};
pub use file_picker_port::FilePickerPort;
pub use folder_picker_port::FolderPickerPort;
pub use plugin_manager_port::{PluginManagerError, PluginManagerPort};
pub use project_port::{ProjectRepositoryError, ProjectRepositoryPort};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_factory_port::SidecarFactoryPort;
pub use skill_catalog_port::{SkillCatalogError, SkillCatalogPort};
pub use terminal_port::{TerminalError, TerminalEvent, TerminalId, TerminalPort, TerminalSession, TerminalSize, OUTPUT_BATCH_LIMIT};
pub use usage_stats_port::{UsageStatsError, UsageStatsPort};
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use browser_port::MockBrowserPort;
#[cfg(any(test, feature = "mocks"))]
pub use webview_port::MockWebViewPort;
#[cfg(any(test, feature = "mocks"))]
pub use files_port::MockFilesPort;
#[cfg(any(test, feature = "mocks"))]
pub use git_port::MockGitPort;
#[cfg(any(test, feature = "mocks"))]
#[cfg(any(test, feature = "mocks"))]
pub use notify_port::MockNotifyPort;
#[cfg(any(test, feature = "mocks"))]
pub use usage_stats_port::MockUsageStatsPort;
#[cfg(any(test, feature = "mocks"))]
pub use terminal_port::MockTerminalPort;
#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use connector_catalog_port::MockConnectorCatalogPort;
#[cfg(any(test, feature = "mocks"))]
pub use mcp_log_port::MockMcpLogPort;
#[cfg(any(test, feature = "mocks"))]
pub use mcp_manager_port::MockMcpManagerPort;
#[cfg(any(test, feature = "mocks"))]
pub use extension_ports::{MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort};
#[cfg(any(test, feature = "mocks"))]
pub use file_picker_port::MockFilePickerPort;
#[cfg(any(test, feature = "mocks"))]
pub use folder_picker_port::MockFolderPickerPort;
#[cfg(any(test, feature = "mocks"))]
pub use plugin_manager_port::MockPluginManagerPort;
#[cfg(any(test, feature = "mocks"))]
pub use project_port::MockProjectRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use session_port::MockSessionRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_factory_port::MockSidecarFactoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
#[cfg(any(test, feature = "mocks"))]
pub use skill_catalog_port::MockSkillCatalogPort;
#[cfg(any(test, feature = "mocks"))]
pub use worktree_port::MockWorktreePort;
