mod command_probe;
mod event;
mod live_session;
mod live_session_actor;
mod probe_slot;
mod registry;
mod registry_host;
mod routine_runner;
mod use_cases;

#[cfg(test)]
mod test_support;

pub use command_probe::CommandProbe;
pub use event::{apply_sidecar_event, push_user_message, SessionUpdate};
pub use live_session::{LiveCommand, LiveSessionHandle, LiveSessionId, LiveSessionView, Phase, QueuedMessage, RewindState, SideAnswer, SideQuestion};
pub use probe_slot::ProbeSlot;
pub use registry_host::{is_busy, is_finished, RegistryHost, WaitingRun};
pub use routine_runner::{RoutineHost, RoutineRunner, RunRequest, StartFailure};
pub use registry::{fork_title, normalize_title, ManageSessionError, OpenError, OpenOutcome, SessionRegistry};
pub use use_cases::{
    AnswerQuestionUseCase, ApprovePermissionUseCase, StopTaskUseCase, ExtensionsError, ExtensionsUseCase, InterruptTurnUseCase,
    ResumeSessionError, ResumeSessionUseCase, SendMessageUseCase, SettingsUseCase,
    StartSessionUseCase, ToggleMcpServerUseCase, TogglePluginUseCase, ToggleSkillUseCase,
};
pub use use_cases::{AddedConnector, ConnectorCatalogUseCase, ConnectorView, ConnectorsError};
pub use use_cases::{AddedSkill, CatalogView, ConfigLock, PluginCatalogError, PluginCatalogUseCase, SkillCatalogUseCase, SkillView, SkillsError};
pub use use_cases::{BaselineMove, RoutineError, RoutinesUseCase};
pub use use_cases::{ScratchRelease, ScratchUseCase};
pub use use_cases::{Kept, Release, WorktreeUseCaseError, WorktreesUseCase};
