mod browser_port;
mod config_port;
mod extension_ports;
mod file_picker_port;
mod files_port;
mod folder_picker_port;
mod git_port;
mod project_port;
mod session_port;
mod sidecar_factory_port;
mod sidecar_port;
mod sleep_port;
mod terminal_port;
mod usage_stats_port;

pub use browser_port::BrowserPort;
pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use files_port::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};
pub use git_port::{ChangeStatus, ChangedFile, GitError, GitPort};
pub use extension_ports::{
    McpRegistryError, McpRegistryPort, PluginRepositoryError, PluginRepositoryPort,
    SkillRepositoryError, SkillRepositoryPort,
};
pub use file_picker_port::FilePickerPort;
pub use folder_picker_port::FolderPickerPort;
pub use project_port::{ProjectRepositoryError, ProjectRepositoryPort};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_factory_port::SidecarFactoryPort;
pub use sleep_port::SleepPort;
pub use terminal_port::{TerminalError, TerminalEvent, TerminalId, TerminalPort, TerminalSession, TerminalSize, OUTPUT_BATCH_LIMIT};
pub use usage_stats_port::{UsageStatsError, UsageStatsPort};
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

#[cfg(any(test, feature = "mocks"))]
pub use browser_port::MockBrowserPort;
#[cfg(any(test, feature = "mocks"))]
pub use files_port::MockFilesPort;
#[cfg(any(test, feature = "mocks"))]
pub use git_port::MockGitPort;
#[cfg(any(test, feature = "mocks"))]
pub use sleep_port::MockSleepPort;
#[cfg(any(test, feature = "mocks"))]
pub use usage_stats_port::MockUsageStatsPort;
#[cfg(any(test, feature = "mocks"))]
pub use terminal_port::MockTerminalPort;
#[cfg(any(test, feature = "mocks"))]
pub use config_port::MockConfigRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use extension_ports::{MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort};
#[cfg(any(test, feature = "mocks"))]
pub use file_picker_port::MockFilePickerPort;
#[cfg(any(test, feature = "mocks"))]
pub use folder_picker_port::MockFolderPickerPort;
#[cfg(any(test, feature = "mocks"))]
pub use project_port::MockProjectRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use session_port::MockSessionRepositoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_factory_port::MockSidecarFactoryPort;
#[cfg(any(test, feature = "mocks"))]
pub use sidecar_port::MockSidecarPort;
