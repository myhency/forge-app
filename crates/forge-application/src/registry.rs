use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use forge_domain::{PermissionMode, ProjectSummary, Session, SessionId};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort, SidecarFactoryPort};
use thiserror::Error;

use crate::use_cases::ExtensionsUseCase;
use crate::live_session::{spawn_live_session, LiveSessionHandle, LiveSessionId, LiveSessionInit};

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("the project folder no longer exists: {}", .0.display())]
    ProjectMissing(PathBuf),
    #[error("failed to load the session: {0}")]
    Load(#[from] SessionRepositoryError),
}

pub struct OpenOutcome {
    pub handle: LiveSessionHandle,
    /// `true` when the session was already open and `handle` is the existing one.
    pub already_open: bool,
}

/// Every open session. Opening never starts a process: a sidecar is spawned lazily by the
/// session itself on its first `send`.
pub struct SessionRegistry {
    factory: Arc<dyn SidecarFactoryPort>,
    sessions: Arc<dyn SessionRepositoryPort>,
    extensions: Arc<ExtensionsUseCase>,
    next_id: AtomicU64,
    // Never held across an `.await`.
    handles: Mutex<HashMap<LiveSessionId, LiveSessionHandle>>,
}

impl SessionRegistry {
    pub fn new(
        factory: Arc<dyn SidecarFactoryPort>,
        sessions: Arc<dyn SessionRepositoryPort>,
        extensions: Arc<ExtensionsUseCase>,
    ) -> Self {
        Self {
            factory,
            sessions,
            extensions,
            next_id: AtomicU64::new(1),
            handles: Mutex::new(HashMap::new()),
        }
    }

    /// Opens an empty session in `project`. Fails if the folder is gone: nothing can be started there.
    /// Must be called from inside a tokio runtime.
    pub fn open_new(
        &self,
        project: &ProjectSummary,
        permission_mode: PermissionMode,
        model: Option<String>,
    ) -> Result<LiveSessionHandle, OpenError> {
        if !project.exists {
            return Err(OpenError::ProjectMissing(project.path.clone()));
        }
        let session = Session {
            id: SessionId(String::new()),
            project_path: project.path.clone(),
            title: None,
            messages: Vec::new(),
            permission_mode,
        };
        let mut handles = self.handles.lock().unwrap();
        Ok(self.insert(&mut handles, project, session, None, model))
    }

    /// Opens a past session read-only, or returns the handle it already has. Two sidecars must
    /// never write the same transcript, so a Claude session id is opened at most once.
    /// Must be called from inside a tokio runtime.
    pub async fn open_existing(
        &self,
        project: &ProjectSummary,
        session_id: SessionId,
    ) -> Result<OpenOutcome, OpenError> {
        if let Some(handle) = self.find_open(&session_id) {
            return Ok(OpenOutcome { handle, already_open: true });
        }
        let session = self.sessions.load_session(&project.path, &session_id).await?;

        // Another open of the same session may have finished while this one was loading, so look
        // again under the lock before registering.
        let mut handles = self.handles.lock().unwrap();
        if let Some(handle) = find_in(&handles, &session_id) {
            return Ok(OpenOutcome { handle, already_open: true });
        }
        let handle = self.insert(&mut handles, project, session, Some(session_id), None);
        Ok(OpenOutcome { handle, already_open: false })
    }

    pub fn get(&self, id: LiveSessionId) -> Option<LiveSessionHandle> {
        self.handles.lock().unwrap().get(&id).cloned()
    }

    /// Open sessions, oldest first.
    pub fn handles(&self) -> Vec<LiveSessionHandle> {
        let mut handles: Vec<LiveSessionHandle> = self.handles.lock().unwrap().values().cloned().collect();
        handles.sort_by_key(|handle| handle.id());
        handles
    }

    /// Closes the session (which shuts its sidecar down) and forgets it.
    pub fn close(&self, id: LiveSessionId) {
        if let Some(handle) = self.handles.lock().unwrap().remove(&id) {
            handle.close();
        }
    }

    fn find_open(&self, session_id: &SessionId) -> Option<LiveSessionHandle> {
        find_in(&self.handles.lock().unwrap(), session_id)
    }

    fn insert(
        &self,
        handles: &mut HashMap<LiveSessionId, LiveSessionHandle>,
        project: &ProjectSummary,
        session: Session,
        claude_session_id: Option<SessionId>,
        model: Option<String>,
    ) -> LiveSessionHandle {
        let id = LiveSessionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let handle = spawn_live_session(LiveSessionInit {
            id,
            project_path: project.path.clone(),
            session,
            claude_session_id,
            model,
            can_send: project.exists,
            factory: self.factory.clone(),
            extensions: self.extensions.clone(),
        });
        handles.insert(id, handle.clone());
        handle
    }
}

// The open handle for a Claude session id, if any. A new session counts once it has learned its id.
fn find_in(handles: &HashMap<LiveSessionId, LiveSessionHandle>, session_id: &SessionId) -> Option<LiveSessionHandle> {
    handles
        .values()
        .find(|handle| !handle.is_closed() && handle.claude_session_id().as_ref() == Some(session_id))
        .cloned()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;
    use forge_domain::{
        Message, MessageRole, PermissionMode, ProjectSummary, Session, SessionId, SessionSummary,
    };
    use forge_ports::{
        MockSessionRepositoryPort, SessionRepositoryError, SessionRepositoryPort, SidecarError,
        SidecarEvent, SidecarPort,
    };

    use super::*;
    use crate::live_session::{LiveSessionId, Phase};
    use crate::test_support::{factory_returning, fake_sidecar, wait_closed, wait_for};

    fn project(path: &str, exists: bool) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 1,
            exists,
        }
    }

    fn past_session(id: &str) -> Session {
        Session {
            id: SessionId(id.to_string()),
            project_path: PathBuf::from("/work/a"),
            title: Some("A title".to_string()),
            messages: vec![
                Message { role: MessageRole::User, content: "earlier".to_string(), tool_calls: Vec::new() },
                Message { role: MessageRole::Assistant, content: "reply".to_string(), tool_calls: Vec::new() },
            ],
            permission_mode: PermissionMode::Default,
        }
    }

    fn repository_returning(session_id: &str, loads: usize) -> Arc<dyn SessionRepositoryPort> {
        let id = session_id.to_string();
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_load_session()
            .times(loads)
            .returning(move |_, _| Ok(past_session(&id)));
        Arc::new(repository)
    }

    fn registry(
        sidecars: Vec<Result<Arc<dyn SidecarPort>, SidecarError>>,
        sessions: Arc<dyn SessionRepositoryPort>,
    ) -> (SessionRegistry, Arc<AtomicUsize>) {
        let (factory, spawns) = factory_returning(sidecars);
        (SessionRegistry::new(factory, sessions, crate::test_support::no_extensions()), spawns)
    }

    #[tokio::test]
    async fn open_new_returns_an_empty_session_that_can_send() {
        let (registry, spawns) = registry(vec![], repository_returning("unused", 0));

        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, Some("m".to_string()))
            .unwrap();

        let view = handle.snapshot();
        assert!(view.session.messages.is_empty());
        assert_eq!(view.session.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(view.claude_session_id, None);
        assert!(view.can_send);
        assert_eq!(spawns.load(Ordering::SeqCst), 0, "opening spawns nothing");
        assert_eq!(registry.handles().len(), 1);
    }

    #[tokio::test]
    async fn open_new_rejects_a_missing_folder_and_registers_nothing() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));

        let result = registry.open_new(&project("/gone", false), PermissionMode::Default, None);

        assert!(matches!(result, Err(OpenError::ProjectMissing(path)) if path == Path::new("/gone")));
        assert!(registry.handles().is_empty());
    }

    #[tokio::test]
    async fn open_existing_loads_the_history_read_only_and_spawns_nothing() {
        let (registry, spawns) = registry(vec![], repository_returning("claude-1", 1));

        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!outcome.already_open);
        let view = outcome.handle.snapshot();
        assert_eq!(view.session.messages.len(), 2);
        assert_eq!(view.claude_session_id, Some(SessionId("claude-1".to_string())));
        assert_eq!(view.phase, Phase::Idle);
        assert!(view.can_send);
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn open_existing_for_a_missing_folder_is_view_only() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 1));

        let outcome = registry
            .open_existing(&project("/gone", false), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!outcome.handle.snapshot().can_send);
        assert_eq!(outcome.handle.snapshot().session.messages.len(), 2);
    }

    #[tokio::test]
    async fn opening_an_already_open_session_returns_the_same_handle_without_loading_again() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 1));
        let first = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        let second = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(second.already_open);
        assert_eq!(second.handle.id(), first.handle.id());
        assert_eq!(registry.handles().len(), 1);
    }

    #[tokio::test]
    async fn a_new_session_counts_as_open_once_it_has_learned_its_claude_id() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry(vec![Ok(sidecar)], repository_returning("unused", 0));
        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::Default, None)
            .unwrap();
        let mut view = handle.subscribe();
        handle.send("hi".to_string());
        control.push(SidecarEvent::SessionStarted { session_id: "claude-7".to_string() });
        wait_for(&mut view, "the session id", |v| v.claude_session_id.is_some()).await;

        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-7".to_string()))
            .await
            .unwrap();

        assert!(outcome.already_open);
        assert_eq!(outcome.handle.id(), handle.id());
    }

    #[tokio::test]
    async fn open_existing_propagates_a_load_failure_and_registers_nothing() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_load_session()
            .returning(|_, id| Err(SessionRepositoryError::NotFound(id.clone())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry
            .open_existing(&project("/work/a", true), SessionId("nope".to_string()))
            .await;

        assert!(matches!(result, Err(OpenError::Load(SessionRepositoryError::NotFound(_)))));
        assert!(registry.handles().is_empty());
    }

    #[tokio::test]
    async fn close_shuts_the_session_down_and_forgets_it_so_it_can_be_opened_again() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 2));
        let first = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let mut view = first.handle.subscribe();

        registry.close(first.handle.id());
        wait_closed(&mut view).await;
        let again = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        assert!(!again.already_open);
        assert_ne!(again.handle.id(), first.handle.id());
        assert_eq!(registry.handles().len(), 1);
        assert!(registry.get(first.handle.id()).is_none());
    }

    #[tokio::test]
    async fn handles_are_listed_oldest_first() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));
        let a = registry.open_new(&project("/work/a", true), PermissionMode::Default, None).unwrap();
        let b = registry.open_new(&project("/work/b", true), PermissionMode::Default, None).unwrap();
        let c = registry.open_new(&project("/work/c", true), PermissionMode::Default, None).unwrap();

        let ids: Vec<LiveSessionId> = registry.handles().iter().map(|h| h.id()).collect();

        assert_eq!(ids, vec![a.id(), b.id(), c.id()]);
    }

    #[tokio::test]
    async fn sessions_run_independently_and_one_failing_leaves_the_other_running() {
        let (first_sidecar, first_control) = fake_sidecar();
        let (second_sidecar, second_control) = fake_sidecar();
        let (registry, spawns) = registry(
            vec![Ok(first_sidecar), Ok(second_sidecar)],
            repository_returning("unused", 0),
        );
        let first = registry.open_new(&project("/work/a", true), PermissionMode::Default, None).unwrap();
        let second = registry.open_new(&project("/work/b", true), PermissionMode::Default, None).unwrap();
        let mut first_view = first.subscribe();
        let mut second_view = second.subscribe();

        first.send("one".to_string());
        second.send("two".to_string());
        wait_for(&mut first_view, "first running", |v| v.phase == Phase::Running).await;
        wait_for(&mut second_view, "second running", |v| v.phase == Phase::Running).await;
        first_control.push(SidecarEvent::AssistantDelta { text: "from-first".to_string() });
        second_control.push(SidecarEvent::AssistantDelta { text: "from-second".to_string() });
        first_control.push(SidecarEvent::Error { message: "boom".to_string() });
        let failed = wait_for(&mut first_view, "first failed", |v| v.phase == Phase::Failed).await;
        let running = wait_for(&mut second_view, "second's text", |v| {
            v.session.messages.iter().any(|m| m.content == "from-second")
        })
        .await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert!(failed.session.messages.iter().all(|m| m.content != "from-second"));
        assert_eq!(running.phase, Phase::Running);
        assert!(running.session.messages.iter().all(|m| m.content != "from-first"));
    }

    // A repository whose load yields once, so two `open_existing` calls really interleave.
    struct SlowRepository;

    #[async_trait]
    impl SessionRepositoryPort for SlowRepository {
        async fn list_sessions(&self, _project_path: &Path) -> Result<Vec<SessionSummary>, SessionRepositoryError> {
            Ok(Vec::new())
        }

        async fn load_session(&self, _project_path: &Path, session_id: &SessionId) -> Result<Session, SessionRepositoryError> {
            tokio::task::yield_now().await;
            Ok(past_session(&session_id.0))
        }

        async fn rename_session(&self, _project_path: &Path, _session_id: &SessionId, _title: &str) -> Result<(), SessionRepositoryError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn two_concurrent_opens_of_the_same_session_end_up_with_one_handle() {
        let (registry, _spawns) = registry(vec![], Arc::new(SlowRepository));
        let project = project("/work/a", true);

        let (first, second) = tokio::join!(
            registry.open_existing(&project, SessionId("claude-1".to_string())),
            registry.open_existing(&project, SessionId("claude-1".to_string())),
        );
        let (first, second) = (first.unwrap(), second.unwrap());

        assert_eq!(registry.handles().len(), 1);
        assert_eq!(first.handle.id(), second.handle.id());
        assert_eq!(
            [first.already_open, second.already_open].iter().filter(|opened| **opened).count(),
            1,
            "exactly one of the two found the other's handle"
        );
    }
}
