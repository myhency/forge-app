use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use forge_domain::ProjectSummary;
use tokio::sync::mpsc;

use crate::live_session::{LiveSessionHandle, LiveSessionId, LiveSessionView, Phase};
use crate::registry::SessionRegistry;
use crate::routine_runner::{RoutineHost, RunRequest, StartFailure};
use crate::use_cases::RoutinesUseCase;

/// How long and how often `watch` retries a write that needs the history line to exist.
const RETRY: Duration = Duration::from_millis(250);
const RETRIES: u32 = 40;

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
    failures: Arc<Mutex<Vec<StartFailure>>>,
    // Never held across an `.await`.
    tracked: Mutex<HashMap<LiveSessionId, Tracked>>,
}

impl RegistryHost {
    /// The receiver gets every session this host starts, so the UI can show it.
    pub fn new(registry: Arc<SessionRegistry>, routines: Arc<RoutinesUseCase>) -> (Self, mpsc::UnboundedReceiver<LiveSessionHandle>) {
        let (started, receiver) = mpsc::unbounded_channel();
        (Self { registry, routines, started, failures: Arc::new(Mutex::new(Vec::new())), tracked: Mutex::new(HashMap::new()) }, receiver)
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
            .filter(|(_, tracked)| !tracked.kept && tracked.handle.with_view(|view| is_finished(view, tracked.started.elapsed() + extra)))
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

    /// The sessions that never got going since the last call, so the person can be told.
    pub fn take_failures(&self) -> Vec<StartFailure> {
        std::mem::take(&mut *self.failures.lock().unwrap())
    }

    /// Whether this host started the session for a routine run.
    pub fn is_routine_session(&self, id: LiveSessionId) -> bool {
        self.tracked.lock().unwrap().contains_key(&id)
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
            .filter(|tracked| !tracked.handle.is_closed() && tracked.handle.with_view(|view| !view.pending_permissions.is_empty()))
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
            let first_waiting = tracked.handle.with_view(|view| view.pending_permissions.front().map(|request| request.tool_name.clone()));
            if let Some(tool_name) = first_waiting {
                if !tracked.notified {
                    tracked.notified = true;
                    found.push(WaitingRun { routine_id: tracked.routine_id.clone(), tool_name });
                }
            } else {
                tracked.notified = false;
            }
        }
        found
    }

    /// Follows one run's session. It writes the Claude session id to the run's history line once the line
    /// exists, turns the line into a failed start when the session never got going, and keeps the rules the
    /// person answered "always allow" with the routine. The line is written by the runner after `start`
    /// returns, so what is seen first is retried for a few seconds.
    fn watch(&self, request: &RunRequest, handle: &LiveSessionHandle) {
        let routines = self.routines.clone();
        let failures = self.failures.clone();
        let (routine_id, created, due_ms) = (request.routine_id.clone(), request.routine_created, request.due_ms);
        let mut updates = handle.subscribe();
        // Runs on the app's tokio runtime. Ends when the session closes (the channel ends).
        tokio::spawn(async move {
            let mut session: Option<String> = None;
            let mut session_stored = false;
            let mut failure: Option<String> = None;
            let mut failure_stored = false;
            let mut known_rules: Vec<String> = Vec::new();
            let mut attempts = 0u32;
            loop {
                let pending = (session.is_some() && !session_stored) || (failure.is_some() && !failure_stored);
                tokio::select! {
                    changed = updates.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(RETRY), if pending && attempts < RETRIES => {
                        attempts += 1;
                    }
                }
                let (claude_id, phase, last_error, rules) = {
                    let view = updates.borrow_and_update();
                    (view.claude_session_id.clone(), view.phase, view.last_error.clone(), view.allowed_rules.clone())
                };
                if session.is_none() {
                    session = claude_id.map(|id| id.0);
                }
                if failure.is_none() && session.is_none() && phase == Phase::Failed {
                    failure = Some(last_error.unwrap_or_else(|| "The session could not start".to_string()));
                }
                if let (Some(id), false) = (&session, session_stored) {
                    session_stored = routines.set_run_session(&routine_id, due_ms, id.clone()).await.unwrap_or(false);
                }
                if let (Some(message), false) = (&failure, failure_stored) {
                    failure_stored = routines.fail_run(&routine_id, due_ms, message.clone()).await.unwrap_or(false);
                    if failure_stored {
                        failures.lock().unwrap().push(StartFailure { routine_id: routine_id.clone(), reason: message.clone() });
                    }
                }
                if rules != known_rules {
                    let new: Vec<String> = rules.iter().filter(|rule| !known_rules.contains(rule)).cloned().collect();
                    known_rules = rules;
                    let _ = routines.add_allowed(&routine_id, created, new).await;
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
            tracked.folder == folder && !tracked.handle.is_closed() && tracked.handle.with_view(|view| is_busy(view, tracked.started.elapsed()))
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
                finished_tasks: Vec::new(),
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
            routine_created: 1,
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
            env: forge_domain::EnvVars::default(),
            defaults: forge_domain::SessionDefaults::default(),
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
        assert!(host.is_routine_session(handle.id()), "the host knows the sessions it started");
        assert!(!host.is_routine_session(LiveSessionId(u64::MAX)), "and no others");
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

    /// A routine "A" created at 1, as the runner would have it when it starts a run.
    async fn routine_a(routines: &RoutinesUseCase) {
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
    }

    fn started_line() -> forge_domain::RoutineRun {
        forge_domain::RoutineRun {
            routine_id: "a".to_string(),
            due_ms: 5,
            started_ms: 5,
            outcome: forge_domain::RunOutcome::Started,
            session_id: None,
            routine_created: 1,
        }
    }

    #[tokio::test]
    async fn a_sidecar_that_cannot_start_turns_the_started_line_into_a_failed_start_even_if_the_line_is_written_late() {
        let (factory, _spawns) = factory_returning(vec![Err(forge_ports::SidecarError::SpawnFailed("no node".to_string()))]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, store) = memory_routines();
        routine_a(&routines).await;
        let (host, _started) = RegistryHost::new(registry, routines.clone());

        host.start(request(vec![])).await.unwrap();
        // The runner records the line after `start` returns; the session may have failed by then.
        tokio::time::sleep(Duration::from_millis(300)).await;
        routines.record_run(started_line(), None).await.unwrap();

        for _ in 0..100 {
            if matches!(store.lock().unwrap().routine_runs[0].outcome, forge_domain::RunOutcome::Skipped(_)) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let outcome = store.lock().unwrap().routine_runs[0].outcome.clone();
        match outcome {
            forge_domain::RunOutcome::Skipped(forge_domain::SkipReason::StartFailed(message)) => assert!(message.contains("no node"), "{message}"),
            other => panic!("expected a failed start, got {other:?}"),
        }
        let failures = host.take_failures();
        assert_eq!(failures.len(), 1, "the failure is kept for a notification");
        assert_eq!(failures[0].routine_id, "a");
        assert!(failures[0].reason.contains("no node"), "{}", failures[0].reason);
        assert!(host.take_failures().is_empty());
    }

    #[tokio::test]
    async fn the_session_id_is_stored_even_when_it_arrives_before_the_line_is_written() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let registry = Arc::new(SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions()));
        let (routines, store) = memory_routines();
        routine_a(&routines).await;
        let (host, mut started) = RegistryHost::new(registry, routines.clone());
        host.start(request(vec![])).await.unwrap();
        let handle = started.try_recv().unwrap();
        let mut view = handle.subscribe();
        wait_for(&mut view, "the turn to start", |v| v.phase == Phase::Running).await;
        control.push(SidecarEvent::SessionStarted { session_id: "s-9".to_string() });
        wait_for(&mut view, "the session id", |v| v.claude_session_id.is_some()).await;

        tokio::time::sleep(Duration::from_millis(300)).await;
        routines.record_run(started_line(), None).await.unwrap();

        for _ in 0..100 {
            if store.lock().unwrap().routine_runs[0].session_id.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert_eq!(store.lock().unwrap().routine_runs[0].session_id.as_deref(), Some("s-9"));
    }
}
