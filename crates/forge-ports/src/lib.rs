mod config_port;
mod extension_ports;
mod file_picker_port;
mod folder_picker_port;
mod project_port;
mod session_port;
mod sidecar_factory_port;
mod sidecar_port;

pub use config_port::{ConfigError, ConfigRepositoryPort};
pub use extension_ports::{
    McpRegistryError, McpRegistryPort, PluginRepositoryError, PluginRepositoryPort,
    SkillRepositoryError, SkillRepositoryPort,
};
pub use file_picker_port::FilePickerPort;
pub use folder_picker_port::FolderPickerPort;
pub use project_port::{ProjectRepositoryError, ProjectRepositoryPort};
pub use session_port::{SessionRepositoryError, SessionRepositoryPort};
pub use sidecar_factory_port::SidecarFactoryPort;
pub use sidecar_port::{
    PermissionRequestId, SidecarError, SidecarEvent, SidecarPort, StartSessionParams,
};

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
