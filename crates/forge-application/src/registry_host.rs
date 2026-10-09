use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use forge_domain::ProjectSummary;
use tokio::sync::mpsc;

use crate::live_session::{LiveSessionHandle, LiveSessionId, LiveSessionView, Phase};
use crate::registry::SessionRegistry;
use crate::routine_runner::{RoutineHost, RunRequest};
use crate::use_cases::RoutinesUseCase;

/// A session this young is never called finished: right after the send its phase may still be Idle.
const GRACE: Duration = Duration::from_secs(60);

/// Whether a routine's session has done its work and nobody needs it: idle, started a turn at some
/// point, nothing waiting for approval, not failed (a failure is for the person to look at).
pub fn is_finished(view: &LiveSessionView, age: Duration) -> bool {
    age > GRACE && view.phase == Phase::Idle && view.pending_permissions.is_empty() && view.claude_session_id.is_some()
}

/// Whether a routine's session may still be doing something in its folder: running, waiting for an
/// answer, or too young to tell. A failed session runs nothing, and an idle old one is done.
pub fn is_busy(view: &LiveSessionView, age: Duration) -> bool {
    view.phase != Phase::Failed && (view.phase == Phase::Running || !view.pending_permissions.is_empty() || age <= GRACE)
}

/// A routine's session that is waiting for the person to answer a permission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingRun {
    pub routine_id: String,
    pub tool_name: String,
}

struct Tracked {
    routine_id: String,
    handle: LiveSessionHandle,
    folder: PathBuf,
    started: Instant,
    notified: bool,
    /// The person has opened this session: Forge no longer closes it for them.
    kept: bool,
}

/// Starts routine sessions in the session registry and keeps track of them: which folders they
/// work in, which are finished, which wait for the person.
pub struct RegistryHost {
    registry: Arc<SessionRegistry>,
    routines: Arc<RoutinesUseCase>,
    started: mpsc::UnboundedSender<LiveSessionHandle>,
    // Never held across an `.await`.
    tracked: Mutex<HashMap<LiveSessionId, Tracked>>,
}

impl RegistryHost {
    /// The receiver gets every session this host starts, so the UI can show it.
    pub fn new(registry: Arc<SessionRegistry>, routines: Arc<RoutinesUseCase>) -> (Self, mpsc::UnboundedReceiver<LiveSessionHandle>) {
        let (started, receiver) = mpsc::unbounded_channel();
        (Self { registry, routines, started, tracked: Mutex::new(HashMap::new()) }, receiver)
    }

    /// Sessions that can be closed: finished, see `is_finished`, and not opened by the person.
    pub fn finished(&self) -> Vec<LiveSessionId> {
        self.finished_with_age(Duration::ZERO)
    }

    /// `finished` as if every session were `extra` older (for tests, which cannot wait a minute).
    fn finished_with_age(&self, extra: Duration) -> Vec<LiveSessionId> {
        let mut tracked = self.tracked.lock().unwrap();
        // A session the person closed is gone: its last published state may still say "waiting".
        tracked.retain(|_, tracked| !tracked.handle.is_closed());
        tracked
            .iter()
            .filter(|(_, tracked)| !tracked.kept && is_finished(&tracked.handle.snapshot(), tracked.started.elapsed() + extra))
            .map(|(id, _)| *id)
            .collect()
    }

    /// The person has opened this session: from now on Forge leaves it open (a reply or an edit in it
    /// would be lost with it). It still counts for the busy check and the waiting marker.
    pub fn keep_open(&self, id: LiveSessionId) {
        if let Some(tracked) = self.tracked.lock().unwrap().get_mut(&id) {
            tracked.kept = true;
        }
    }

    /// Stops tracking a session (it was closed).
    pub fn forget(&self, id: LiveSessionId) {
        self.tracked.lock().unwrap().remove(&id);
    }

    /// Routine ids that have a session waiting for an answer right now.
    pub fn waiting_now(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .tracked
            .lock()
            .unwrap()
            .values()
            .filter(|tracked| !tracked.handle.is_closed() && !tracked.handle.snapshot().pending_permissions.is_empty())
            .map(|tracked| tracked.routine_id.clone())
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Sessions that began waiting for approval since the last call, each reported once.
    pub fn waiting(&self) -> Vec<WaitingRun> {
        let mut found = Vec::new();
        for tracked in self.tracked.lock().unwrap().values_mut() {
            if tracked.handle.is_closed() {
                continue;
            }
            let view = tracked.handle.snapshot();
            if let Some(request) = view.pending_permissions.front() {
                if !tracked.notified {
                    tracked.notified = true;
                    found.push(WaitingRun { routine_id: tracked.routine_id.clone(), tool_name: request.tool_name.clone() });
                }
            } else {
                tracked.notified = false;
            }
        }
        found
    }

    /// Reports the Claude session id of a run once it is known, and keeps the rules the person
    /// answered "always allow" with the routine.
    fn watch(&self, request: &RunRequest, handle: &LiveSessionHandle) {
        let routines = self.routines.clone();
        let routine_id = request.routine_id.clone();
        let due_ms = request.due_ms;
        let mut updates = handle.subscribe();
        // Runs on the app's tokio runtime. Ends when the session closes (the channel ends).
        tokio::spawn(async move {
            let mut reported_session = false;
            let mut known_rules: Vec<String> = Vec::new();
            while updates.changed().await.is_ok() {
                let view = updates.borrow_and_update().clone();
                if !reported_session {
                    if let Some(id) = &view.claude_session_id {
                        reported_session = true;
                        let _ = routines.set_run_session(&routine_id, due_ms, id.0.clone()).await;
                    }
                }
                if view.allowed_rules != known_rules {
                    let new: Vec<String> = view.allowed_rules.iter().filter(|rule| !known_rules.contains(rule)).cloned().collect();
                    known_rules = view.allowed_rules.clone();
                    let _ = routines.add_allowed(&routine_id, new).await;
                }
            }
        });
    }
}

#[async_trait]
impl RoutineHost for RegistryHost {
    fn folder_exists(&self, folder: &Path) -> bool {
        folder.is_dir()
    }

    fn busy_in(&self, folder: &Path) -> bool {
        self.tracked.lock().unwrap().values().any(|tracked| {
            tracked.folder == folder && !tracked.handle.is_closed() && is_busy(&tracked.handle.snapshot(), tracked.started.elapsed())
        })
    }

    async fn start(&self, request: RunRequest) -> Result<(), String> {
        let project = ProjectSummary { path: request.folder.clone(), last_active: String::new(), session_count: 0, exists: true };
        let handle = self
            .registry
            .open_new(&project, request.permission_mode, request.model.clone(), Vec::new())
            .map_err(|error| error.to_string())?;
        if !request.allowed_tools.is_empty() {
            handle.allow_tools(request.allowed_tools.clone());
        }
        handle.set_title(request.title.clone());
        self.watch(&request, &handle);
        // Counted as busy from here on, before anything is sent.
        self.tracked.lock().unwrap().insert(
            handle.id(),
            Tracked { routine_id: request.routine_id.clone(), handle: handle.clone(), folder: request.folder.clone(), started: Instant::now(), notified: false, kept: false },
        );
        handle.send(request.prompt);
        let _ = self.started.send(handle);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::path::PathBuf;

    use forge_domain::{PermissionMode, PermissionRequest, Session, SessionId, SessionUsage};

    fn view(phase: Phase, pending: usize, claude_id: Option<&str>) -> LiveSessionView {
        let request = PermissionRequest { id: "p".to_string(), tool_name: "Bash".to_string(), input: serde_json::json!({}), rules: Vec::new() };
        LiveSessionView {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: Session {
                id: SessionId(String::new()),
                project_path: PathBuf::from("/work/a"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
                usage: SessionUsage::default(),
            },
            claude_session_id: claude_id.map(|id| SessionId(id.to_string())),
            model: None,
            effort: None,
            available_models: Vec::new(),
            phase,
            turn_started: None,
            background_tasks: Vec::new(),
            suggestion: None,
            pending_permissions: VecDeque::from(vec![request; pending]),
            last_error: None,
            can_send: true,
            unsent: None,
            unsent_attachments: Vec::new(),
            failed_sends: 0,
            context: None,
            commands: Vec::new(),
            allowed_rules: Vec::new(),
        }
    }

    const OLD: Duration = Duration::from_secs(61);

    #[test]
    fn a_session_is_finished_when_idle_and_old_enough_with_nothing_waiting() {
        assert!(is_finished(&view(Phase::Idle, 0, Some("s")), OLD));
    }

    #[test]
    fn a_young_session_is_not_finished_whatever_its_phase_says() {
        // Right after the send the phase may still read Idle.
        assert!(!is_finished(&view(Phase::Idle, 0, Some("s")), Duration::from_secs(5)));
    }

    #[test]
    fn a_session_is_busy_while_running_waiting_or_young_and_not_when_failed_or_idle_and_old() {
        assert!(is_busy(&view(Phase::Running, 0, Some("s")), OLD));
        assert!(is_busy(&view(Phase::Idle, 1, Some("s")), OLD), "waiting for approval");
        assert!(is_busy(&view(Phase::Idle, 0, None), Duration::from_secs(5)), "just started");
        assert!(!is_busy(&view(Phase::Failed, 0, Some("s")), OLD), "a failed session runs nothing");
        assert!(!is_busy(&view(Phase::Idle, 0, None), OLD), "nothing is running in it");
        assert!(!is_busy(&view(Phase::Idle, 0, Some("s")), OLD));
    }

    #[test]
    fn a_running_a_waiting_a_failed_or_unstarted_session_is_not_finished() {
        assert!(!is_finished(&view(Phase::Running, 0, Some("s")), OLD));
        assert!(!is_finished(&view(Phase::Idle, 1, Some("s")), OLD), "waiting for approval");
        assert!(!is_finished(&view(Phase::Failed, 0, Some("s")), OLD), "a failure is for the person to see");
        assert!(!is_finished(&view(Phase::Idle, 0, None), OLD), "no turn ever ran");
    }

    use crate::test_support::{factory_returning, fake_sidecar, no_extensions, wait_for, Call};
    use forge_domain::{ActiveExtensions, AppConfig, PermissionDecision, RoutineDraft, Schedule};
    use forge_ports::{MockConfigRepositoryPort, MockSessionRepositoryPort, SidecarEvent, StartSessionParams};

    fn memory_routines() -> (Arc<RoutinesUseCase>, Arc<Mutex<AppConfig>>) {
        let store = Arc::new(Mutex::new(AppConfig::default()));
        let mut mock = MockConfigRepositoryPort::new();
        let loading = store.clone();
        mock.expect_load().returning(move || Ok(loading.lock().unwrap().clone()));
        let saving = store.clone();
        mock.expect_save().returning(move |config: &AppConfig| {
            *saving.lock().unwrap() = config.clone();
            Ok(())
        });
        (Arc::new(RoutinesUseCase::new(Arc::new(mock))), store)
    }

    fn request(allowed: Vec<&str>) -> RunRequest {
        RunRequest {
            routine_id: "a".to_string(),
            due_ms: 5,
            folder: PathBuf::from("/work/a"),
            permission_mode: PermissionMode::Plan,
            model: Some("m".to_string()),
            prompt: "go".to_string(),
            title: "Oct 9 – A".to_string(),
            allowed_tools: allowed.into_iter().map(str::to_string).collect(),
        }
    }

    #[tokio::test]
    async fn start_makes_the_folder_busy_at_once_hands_out_the_session_and_sends_the_prompt() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, _) = memory_routines();
        let (host, mut started) = RegistryHost::new(registry, routines);

        host.start(request(vec!["Read"])).await.unwrap();

        assert!(host.busy_in(Path::new("/work/a")), "busy before anything came back");
        assert!(!host.busy_in(Path::new("/other")));
        let handle = started.try_recv().expect("the session is handed out");
        let mut view = handle.subscribe();
        let seen = wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        let expected = StartSessionParams {
            cwd: "/work/a".to_string(),
            resume_session_id: None,
            permission_mode: PermissionMode::Plan,
            model: Some("m".to_string()),
            effort: None,
            extensions: ActiveExtensions::default(),
            additional_directories: Vec::new(),
            allowed_tools: vec!["Read".to_string()],
        };
        assert_eq!(control.calls(), vec![Call::Start(expected), Call::Send("go".to_string())]);
        assert_eq!(seen.session.title.as_deref(), Some("Oct 9 – A"));
        assert!(host.finished().is_empty(), "a young session is not finished");
        host.forget(handle.id());
        assert!(!host.busy_in(Path::new("/work/a")), "a forgotten session is not counted");
    }

    #[tokio::test]
    async fn a_waiting_session_is_reported_once_and_an_always_allow_answer_reaches_the_routine() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, store) = memory_routines();
        routines
            .create(
                RoutineDraft {
                    name: "A".into(),
                    description: "d".into(),
                    instructions: String::new(),
                    folder: Some("/work/a".into()),
                    permission_mode: PermissionMode::Default,
                    model: None,
                    schedule: Schedule::Manual,
                },
                1,
            )
            .await
            .unwrap();
        let (host, mut started) = RegistryHost::new(registry, routines);
        host.start(request(vec![])).await.unwrap();
        let handle = started.try_recv().unwrap();
        let mut view = handle.subscribe();
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;

        control.push(SidecarEvent::PermissionRequest {
            permission_request_id: "p1".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({}),
            rules: vec!["Bash(ls:*)".to_string()],
        });
        wait_for(&mut view, "the request", |v| v.pending_permissions.len() == 1).await;
        assert_eq!(host.waiting_now(), vec!["a".to_string()]);
        assert_eq!(host.waiting(), vec![WaitingRun { routine_id: "a".to_string(), tool_name: "Bash".to_string() }]);
        assert!(host.waiting().is_empty(), "reported once");

        handle.approve("p1".to_string(), PermissionDecision::AllowAlways);
        for _ in 0..200 {
            if !store.lock().unwrap().routines[0].always_allowed.is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(store.lock().unwrap().routines[0].always_allowed, vec!["Bash(ls:*)".to_string()]);
        assert!(host.waiting_now().is_empty());
    }

    #[tokio::test]
    async fn closing_a_waiting_session_clears_its_marker_and_frees_its_folder() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, _) = memory_routines();
        let (host, mut started) = RegistryHost::new(registry, routines);
        host.start(request(vec![])).await.unwrap();
        let handle = started.try_recv().unwrap();
        let mut view = handle.subscribe();
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        control.push(SidecarEvent::PermissionRequest {
            permission_request_id: "p1".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({}),
            rules: Vec::new(),
        });
        wait_for(&mut view, "the request", |v| v.pending_permissions.len() == 1).await;
        assert_eq!(host.waiting_now(), vec!["a".to_string()]);

        // The person closes the session instead of answering.
        handle.close();
        for _ in 0..200 {
            if handle.is_closed() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(handle.is_closed());
        assert!(host.waiting_now().is_empty(), "no marker for a closed session");
        assert!(host.waiting().is_empty());
        assert!(!host.busy_in(Path::new("/work/a")), "its folder is free again");
        assert!(host.finished().is_empty(), "and it is forgotten, not offered for closing");
    }

    #[tokio::test]
    async fn a_session_the_person_has_taken_over_is_never_offered_for_closing() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, _) = memory_routines();
        let (host, mut started) = RegistryHost::new(registry, routines);
        host.start(request(vec![])).await.unwrap();
        let handle = started.try_recv().unwrap();
        let mut view = handle.subscribe();
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        control.push(SidecarEvent::SessionStarted { session_id: "s-1".to_string() });
        control.push(SidecarEvent::TurnComplete { usage: Default::default() });
        wait_for(&mut view, "the turn to end", |v| v.phase == Phase::Idle && v.claude_session_id.is_some()).await;

        // Old enough, idle, nothing waiting: Forge may close it ...
        assert_eq!(host.finished_with_age(OLD), vec![handle.id()]);
        // ... until the person has looked at it: a draft in it must not be thrown away.
        host.keep_open(handle.id());
        assert!(host.finished_with_age(OLD).is_empty());
    }
}
