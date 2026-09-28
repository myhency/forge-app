mod event;
mod live_session;
mod live_session_actor;
mod registry;
mod use_cases;

#[cfg(test)]
mod test_support;

pub use event::{apply_sidecar_event, push_user_message, SessionUpdate};
pub use live_session::{
    LiveCommand, LiveSessionHandle, LiveSessionId, LiveSessionView, Phase,
};
pub use registry::{OpenError, OpenOutcome, SessionRegistry};
pub use use_cases::{
    ApprovePermissionUseCase, InterruptTurnUseCase, ResumeSessionError, ResumeSessionUseCase,
    SendMessageUseCase, StartSessionUseCase, ToggleMcpServerUseCase, TogglePluginUseCase,
    ToggleSkillUseCase,
};
