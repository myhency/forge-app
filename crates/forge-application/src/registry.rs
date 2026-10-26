use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use forge_domain::{PermissionMode, ProjectSummary, Session, SessionId, SlashCommand};
use forge_ports::{SessionRepositoryError, SessionRepositoryPort, SidecarFactoryPort};
use thiserror::Error;

use crate::command_probe::{spawn_probe, CommandProbe};
use crate::use_cases::ExtensionsUseCase;
use crate::live_session::{spawn_live_session, LiveSessionHandle, LiveSessionId, LiveSessionInit};

#[derive(Debug, Error)]
pub enum OpenError {
    #[error("the project folder no longer exists: {}", .0.display())]
    ProjectMissing(PathBuf),
    #[error("failed to load the session: {0}")]
    Load(#[from] SessionRepositoryError),
}

const MAX_TITLE_CHARS: usize = 200;

#[derive(Debug, Error)]
pub enum ManageSessionError {
    #[error("the title cannot be empty")]
    EmptyTitle,
    #[error("the title can be at most {max} characters")]
    TitleTooLong { max: usize },
    #[error("Close the session first ({}) to delete it.", forge_domain::KEY_CLOSE_SESSION)]
    SessionOpen,
    #[error("{0}")]
    Repository(#[from] SessionRepositoryError),
}

/// A session title as it will be stored: trimmed, not empty, at most 200 characters.
pub fn normalize_title(raw: &str) -> Result<String, ManageSessionError> {
    let title = raw.trim();
    if title.is_empty() {
        return Err(ManageSessionError::EmptyTitle);
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ManageSessionError::TitleTooLong { max: MAX_TITLE_CHARS });
    }
    Ok(title.to_string())
}

const FORK_SUFFIX: &str = " (fork)";

/// The title of a fork of a session called `title`: the title and " (fork)", cut so the whole stays
/// within the title limit. A blank title gives "Session (fork)".
pub fn fork_title(title: &str) -> String {
    let base = title.trim();
    let base = if base.is_empty() { "Session" } else { base };
    let room = MAX_TITLE_CHARS - FORK_SUFFIX.chars().count();
    let base: String = base.chars().take(room).collect();
    format!("{base}{FORK_SUFFIX}")
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
    /// Start each opened session's sidecar at once (so its slash commands are known) instead of at
    /// its first send. Off by default: every open session then holds a process.
    warm_up: bool,
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
            warm_up: false,
            next_id: AtomicU64::new(1),
            handles: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_warm_up(mut self) -> Self {
        self.warm_up = true;
        self
    }

    /// Starts a probe for `project`'s slash commands; `None` when the folder is gone. Must be called
    /// from inside a tokio runtime.
    pub fn probe_commands(&self, project: &ProjectSummary) -> Option<CommandProbe> {
        if !project.exists {
            return None;
        }
        Some(spawn_probe(self.factory.clone(), self.extensions.clone(), project.path.clone()))
    }

    /// Opens an empty session in `project`. Fails if the folder is gone: nothing can be started there.
    /// Must be called from inside a tokio runtime.
    pub fn open_new(
        &self,
        project: &ProjectSummary,
        permission_mode: PermissionMode,
        model: Option<String>,
        commands: Vec<SlashCommand>,
    ) -> Result<LiveSessionHandle, OpenError> {
        self.open_new_with_effort(project, permission_mode, model, None, commands)
    }

    /// `open_new` with an effort level (`None`: the SDK's default) the session starts with. It is
    /// given here, not set afterwards, because a warmed-up sidecar would be asked to change it
    /// before its session runs.
    pub fn open_new_with_effort(
        &self,
        project: &ProjectSummary,
        permission_mode: PermissionMode,
        model: Option<String>,
        effort: Option<forge_domain::EffortLevel>,
        commands: Vec<SlashCommand>,
    ) -> Result<LiveSessionHandle, OpenError> {
        if !project.exists {
            return Err(OpenError::ProjectMissing(project.path.clone()));
        }
        let session = Session {
            finished_tasks: Vec::new(),
            id: SessionId(String::new()),
            project_path: project.path.clone(),
            title: None,
            messages: Vec::new(),
            permission_mode,
            usage: forge_domain::SessionUsage::default(),
        };
        let mut handles = self.handles.lock().unwrap();
        Ok(self.insert(&mut handles, project, session, None, model, effort, commands))
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
        let handle = self.insert(&mut handles, project, session, Some(session_id), None, None, Vec::new());
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

    /// Renames a past or open session. The title is stored first; an open session's view is
    /// updated only once that succeeded. Returns the title as stored.
    pub async fn rename(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<String, ManageSessionError> {
        let title = normalize_title(title)?;
        self.sessions.rename_session(project_path, session_id, &title).await?;
        if let Some(handle) = self.find_open(session_id) {
            handle.set_title(title.clone());
        }
        Ok(title)
    }

    /// Moves a session that is not open to the trash. An open session is refused: its view would
    /// go on showing (and could resume) a transcript that no longer exists.
    pub async fn delete(&self, project_path: &Path, session_id: &SessionId) -> Result<(), ManageSessionError> {
        if self.find_open(session_id).is_some() {
            return Err(ManageSessionError::SessionOpen);
        }
        self.sessions.delete_session(project_path, session_id).await?;
        Ok(())
    }

    /// Copies a past or open session into a new one called "<title> (fork)" and returns the new id.
    /// The source is untouched; an open session's copy holds what is on disk now.
    pub async fn fork(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        title: &str,
    ) -> Result<SessionId, ManageSessionError> {
        Ok(self.sessions.fork_session(project_path, session_id, &fork_title(title)).await?)
    }

    /// Like `fork`, but the copy ends just before the user message `message_uuid` (the conversation
    /// as it was before that message).
    pub async fn fork_before(
        &self,
        project_path: &Path,
        session_id: &SessionId,
        message_uuid: &str,
        title: &str,
    ) -> Result<SessionId, ManageSessionError> {
        Ok(self.sessions.fork_session_before(project_path, session_id, message_uuid, &fork_title(title)).await?)
    }

    /// Cuts a session's transcript back to just before its user message `message_uuid` (the original
    /// is kept beside it). The session must not be running.
    pub async fn cut_back(&self, project_path: &Path, session_id: &SessionId, message_uuid: &str) -> Result<(), ManageSessionError> {
        Ok(self.sessions.truncate_session_before(project_path, session_id, message_uuid).await?)
    }

    /// Moves a session's transcript to another project directory, so it lists under the new folder.
    pub async fn move_to(
        &self,
        from_project: &Path,
        to_project: &Path,
        session_id: &SessionId,
    ) -> Result<(), ManageSessionError> {
        Ok(self.sessions.move_session(from_project, to_project, session_id).await?)
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
        effort: Option<forge_domain::EffortLevel>,
        commands: Vec<SlashCommand>,
    ) -> LiveSessionHandle {
        let id = LiveSessionId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let handle = spawn_live_session(LiveSessionInit {
            id,
            project_path: project.path.clone(),
            session,
            claude_session_id,
            model,
            effort,
            can_send: project.exists,
            commands,
            factory: self.factory.clone(),
            extensions: self.extensions.clone(),
        });
        handles.insert(id, handle.clone());
        if self.warm_up && project.exists {
            handle.warm_up();
        }
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
        Message, PermissionMode, ProjectSummary, Session, SessionId, SessionSummary,
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
            finished_tasks: Vec::new(),
            id: SessionId(id.to_string()),
            project_path: PathBuf::from("/work/a"),
            title: Some("A title".to_string()),
            messages: vec![
                Message::user("earlier"),
                Message::assistant_text("reply"),
            ],
            permission_mode: PermissionMode::Default,
            usage: forge_domain::SessionUsage::default(),
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
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, Some("m".to_string()), Vec::new())
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
    async fn open_new_with_effort_shows_the_effort_at_once() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));

        let handle = registry
            .open_new_with_effort(&project("/work/a", true), PermissionMode::Default, None, Some(forge_domain::EffortLevel::Low), Vec::new())
            .unwrap();

        assert_eq!(handle.snapshot().effort, Some(forge_domain::EffortLevel::Low));
    }

    #[tokio::test]
    async fn open_new_rejects_a_missing_folder_and_registers_nothing() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));

        let result = registry.open_new(&project("/gone", false), PermissionMode::Default, None, Vec::new());

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
    async fn two_folders_of_one_claude_directory_open_independently_one_view_only() {
        let (registry, _spawns) = registry(vec![], repository_returning("claude-1", 2));

        let gone = registry
            .open_existing(&project("/work/my_app", false), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let present = registry
            .open_existing(&project("/work/my-app", true), SessionId("claude-2".to_string()))
            .await
            .unwrap();

        assert!(!gone.handle.snapshot().can_send, "the folder that is gone is view-only");
        assert!(present.handle.snapshot().can_send, "its sibling folder can still send");
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
            .open_new(&project("/work/a", true), PermissionMode::Default, None, Vec::new())
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
        let a = registry.open_new(&project("/work/a", true), PermissionMode::Default, None, Vec::new()).unwrap();
        let b = registry.open_new(&project("/work/b", true), PermissionMode::Default, None, Vec::new()).unwrap();
        let c = registry.open_new(&project("/work/c", true), PermissionMode::Default, None, Vec::new()).unwrap();

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
        let first = registry.open_new(&project("/work/a", true), PermissionMode::Default, None, Vec::new()).unwrap();
        let second = registry.open_new(&project("/work/b", true), PermissionMode::Default, None, Vec::new()).unwrap();
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
            v.session.messages.iter().any(|m| m.text() == "from-second")
        })
        .await;

        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert!(failed.session.messages.iter().all(|m| m.text() != "from-second"));
        assert_eq!(running.phase, Phase::Running);
        assert!(running.session.messages.iter().all(|m| m.text() != "from-first"));
    }

    // A repository whose load yields once, so two `open_existing` calls really interleave.
    struct SlowRepository;

    #[async_trait]
    impl SessionRepositoryPort for SlowRepository {
        async fn search_sessions(&self, _query: &str, _limit: usize) -> Result<Vec<forge_domain::SearchHit>, SessionRepositoryError> {
            Ok(Vec::new())
        }

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

        async fn fork_session(&self, _project_path: &Path, _session_id: &SessionId, _title: &str) -> Result<SessionId, SessionRepositoryError> {
            unimplemented!("not used by these tests")
        }

        async fn fork_session_before(&self, _project_path: &Path, _session_id: &SessionId, _message_uuid: &str, _title: &str) -> Result<SessionId, SessionRepositoryError> {
            unimplemented!("not used by these tests")
        }

        async fn truncate_session_before(&self, _project_path: &Path, _session_id: &SessionId, _message_uuid: &str) -> Result<(), SessionRepositoryError> {
            unimplemented!("not used by these tests")
        }

        async fn delete_session(&self, _project_path: &Path, _session_id: &SessionId) -> Result<(), SessionRepositoryError> {
            Ok(())
        }

        async fn move_session(&self, _from_project: &Path, _to_project: &Path, _session_id: &SessionId) -> Result<(), SessionRepositoryError> {
            unimplemented!("not used by these tests")
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

    #[test]
    fn titles_are_trimmed_and_must_not_be_empty() {
        assert_eq!(normalize_title("  Fix the build \n").unwrap(), "Fix the build");
        assert!(matches!(normalize_title(""), Err(ManageSessionError::EmptyTitle)));
        assert!(matches!(normalize_title("  \t\n "), Err(ManageSessionError::EmptyTitle)));
    }

    #[test]
    fn a_title_may_be_200_characters_but_not_201() {
        let at_limit = "a".repeat(200);
        assert_eq!(normalize_title(&at_limit).unwrap(), at_limit);
        assert!(matches!(
            normalize_title(&"a".repeat(201)),
            Err(ManageSessionError::TitleTooLong { max: 200 })
        ));
    }

    #[test]
    fn the_title_limit_counts_characters_not_bytes() {
        let korean = "한".repeat(200); // 600 bytes
        assert_eq!(normalize_title(&korean).unwrap(), korean);
        assert!(normalize_title(&"한".repeat(201)).is_err());
    }

    #[test]
    fn the_limit_applies_after_trimming() {
        let padded = format!("  {}  ", "a".repeat(200));
        assert_eq!(normalize_title(&padded).unwrap().chars().count(), 200);
    }

    #[test]
    fn the_fork_title_adds_the_suffix_and_stays_within_the_limit() {
        assert_eq!(fork_title("Mine"), "Mine (fork)");
        assert_eq!(fork_title("  Mine  "), "Mine (fork)");
        assert_eq!(fork_title("   "), "Session (fork)");
        let long = fork_title(&"a".repeat(300));
        assert_eq!(long.chars().count(), 200);
        assert!(long.ends_with(" (fork)"));
    }

    #[tokio::test]
    async fn fork_asks_the_repository_for_a_copy_with_the_fork_title() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_fork_session()
            .withf(|p: &Path, id: &SessionId, title: &str| p == Path::new("/work/a") && id.0 == "claude-1" && title == "Mine (fork)")
            .times(1)
            .returning(|_, _, _| Ok(SessionId("claude-2".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let new_id = registry.fork(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Mine").await.unwrap();

        assert_eq!(new_id.0, "claude-2");
    }

    #[tokio::test]
    async fn move_to_asks_the_repository_once() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_move_session()
            .withf(|from: &Path, to: &Path, id: &SessionId| from == Path::new("/work/a") && to == Path::new("/work/b") && id.0 == "claude-1")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        registry.move_to(Path::new("/work/a"), Path::new("/work/b"), &SessionId("claude-1".to_string())).await.unwrap();
    }

    #[tokio::test]
    async fn a_failed_fork_is_an_error_from_the_repository() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_fork_session().returning(|_, _, _| Err(SessionRepositoryError::ForkFailed("disk full".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.fork(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Mine").await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::ForkFailed(_)))));
    }

    #[tokio::test]
    async fn rename_stores_the_trimmed_title_and_returns_it() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_rename_session()
            .withf(|p: &Path, id: &SessionId, title: &str| p == Path::new("/work/a") && id.0 == "claude-1" && title == "Mine")
            .times(1)
            .returning(|_, _, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let stored = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "  Mine ").await.unwrap();

        assert_eq!(stored, "Mine");
    }

    #[tokio::test]
    async fn rename_with_an_invalid_title_never_reaches_the_repository() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_rename_session().times(0);
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "   ").await;

        assert!(matches!(result, Err(ManageSessionError::EmptyTitle)));
    }

    #[tokio::test]
    async fn rename_of_an_open_session_updates_its_live_title() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_rename_session().times(1).returning(|_, _, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let mut view = outcome.handle.subscribe();

        registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Renamed").await.unwrap();

        let updated = wait_for(&mut view, "the new title", |v| v.session.title.as_deref() == Some("Renamed")).await;
        assert_eq!(updated.session.title.as_deref(), Some("Renamed"));
    }

    #[tokio::test]
    async fn a_failed_rename_leaves_the_open_sessions_title_alone() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository
            .expect_rename_session()
            .times(1)
            .returning(|_, _, _| Err(SessionRepositoryError::RenameFailed("read-only".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();

        let result = registry.rename(Path::new("/work/a"), &SessionId("claude-1".to_string()), "Renamed").await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::RenameFailed(_)))));
        tokio::task::yield_now().await;
        assert_eq!(outcome.handle.snapshot().session.title.as_deref(), Some("A title"));
    }

    #[tokio::test]
    async fn delete_goes_to_the_repository_for_a_session_that_is_not_open() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_delete_session().times(1).returning(|_, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-9".to_string())).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn delete_of_an_open_session_is_refused_and_touches_nothing() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_delete_session().times(0);
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        registry.open_existing(&project("/work/a", true), SessionId("claude-1".to_string())).await.unwrap();

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-1".to_string())).await;

        assert!(matches!(result, Err(ManageSessionError::SessionOpen)));
        assert_eq!(
            result.unwrap_err().to_string(),
            format!("Close the session first ({}) to delete it.", forge_domain::KEY_CLOSE_SESSION)
        );
    }

    #[tokio::test]
    async fn delete_works_again_once_the_session_is_closed() {
        let mut repository = MockSessionRepositoryPort::new();
        repository.expect_load_session().times(1).returning(|_, _| Ok(past_session("claude-1")));
        repository.expect_delete_session().times(1).returning(|_, _| Ok(()));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));
        let outcome = registry
            .open_existing(&project("/work/a", true), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        registry.close(outcome.handle.id());

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-1".to_string())).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn a_repository_failure_on_delete_is_reported() {
        let mut repository = MockSessionRepositoryPort::new();
        repository
            .expect_delete_session()
            .times(1)
            .returning(|_, _| Err(SessionRepositoryError::DeleteFailed("no trash".to_string())));
        let (registry, _spawns) = registry(vec![], Arc::new(repository));

        let result = registry.delete(Path::new("/work/a"), &SessionId("claude-9".to_string())).await;

        assert!(matches!(result, Err(ManageSessionError::Repository(SessionRepositoryError::DeleteFailed(_)))));
    }

    #[tokio::test]
    async fn a_registry_that_warms_up_starts_the_sidecar_of_an_opened_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = SessionRegistry::new(factory, repository_returning("unused", 0), crate::test_support::no_extensions())
            .with_warm_up();

        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, Some("m".to_string()), Vec::new())
            .unwrap();
        let command = forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None };
        control.push(forge_ports::SidecarEvent::CommandsAvailable { commands: vec![command.clone()] });
        let mut view = handle.subscribe();
        let seen = wait_for(&mut view, "the commands", |v| !v.commands.is_empty()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(seen.commands, vec![command]);
    }

    #[tokio::test]
    async fn a_registry_that_warms_up_leaves_a_session_whose_folder_is_gone_alone() {
        let (factory, spawns) = factory_returning(vec![]);
        let registry = SessionRegistry::new(factory, repository_returning("claude-1", 1), crate::test_support::no_extensions())
            .with_warm_up();

        let outcome = registry
            .open_existing(&project("/work/a", false), SessionId("claude-1".to_string()))
            .await
            .unwrap();
        let mut view = outcome.handle.subscribe();
        outcome.handle.close();
        wait_closed(&mut view).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
    }
    #[tokio::test]
    async fn open_new_starts_the_session_with_the_given_commands() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));
        let commands = vec![forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None }];

        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, None, commands.clone())
            .unwrap();

        assert_eq!(handle.snapshot().commands, commands);
    }
}
