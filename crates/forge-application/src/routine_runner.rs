use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::TimeZone;
use forge_domain::{decide, Decision, PermissionMode, Routine, RoutineRun, RunOutcome, SkipReason};

use crate::use_cases::{BaselineMove, RoutineError, RoutinesUseCase};

/// What a host needs to start one run.
#[derive(Debug, Clone, PartialEq)]
pub struct RunRequest {
    pub routine_id: String,
    /// When the routine was created: a late report from a deleted routine's session must not reach a new
    /// routine with the same id.
    pub routine_created: i64,
    /// The due time of this run; the host reports the session id against it.
    pub due_ms: i64,
    pub folder: PathBuf,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
    pub prompt: String,
    pub title: String,
    pub allowed_tools: Vec<String>,
}

/// What the runner needs from the outside world: whether a folder exists, whether a routine's
/// session is still working in it, and a way to start a session.
#[async_trait]
pub trait RoutineHost: Send + Sync {
    fn folder_exists(&self, folder: &Path) -> bool;
    /// Whether a session this host started is still working (or waiting) in `folder`. `start` must
    /// make this true at once, so two routines due in one tick cannot both start there.
    fn busy_in(&self, folder: &Path) -> bool;
    async fn start(&self, request: RunRequest) -> Result<(), String>;
}

pub struct RoutineRunner {
    routines: Arc<RoutinesUseCase>,
    host: Arc<dyn RoutineHost>,
    /// The latest due time each routine was handled for. The config's baseline says the same, but a
    /// save can fail; without this a routine would start again on every tick until the save worked.
    handled: Mutex<HashMap<String, i64>>,
}

impl RoutineRunner {
    pub fn new(routines: Arc<RoutinesUseCase>, host: Arc<dyn RoutineHost>) -> Self {
        Self { routines, host, handled: Mutex::new(HashMap::new()) }
    }

    fn already_handled(&self, routine_id: &str, due_ms: i64) -> bool {
        self.handled.lock().unwrap().get(routine_id).is_some_and(|handled| *handled >= due_ms)
    }

    fn mark_handled(&self, routine_id: &str, due_ms: i64) {
        self.handled.lock().unwrap().insert(routine_id.to_string(), due_ms);
    }

    /// One look at every routine. Called every 30 seconds, and once at start so the time Forge was
    /// closed is judged at once. A config that cannot be read ends the tick with the error.
    pub async fn tick<Tz: TimeZone>(&self, now_ms: i64, zone: &Tz) -> Result<(), RoutineError>
    where
        Tz::Offset: Display,
    {
        let (routines, _) = self.routines.snapshot().await?;
        // A save that fails for one routine must not stop the others; the first failure is reported at the end.
        let mut first_error = None;
        for routine in routines {
            match decide(&routine, now_ms, zone) {
                Decision::Wait => {}
                Decision::Missed { due_ms } | Decision::Run { due_ms } if self.already_handled(&routine.id, due_ms) => {}
                Decision::Missed { due_ms } => {
                    self.mark_handled(&routine.id, due_ms);
                    let run = RoutineRun {
                        routine_id: routine.id.clone(),
                        due_ms,
                        started_ms: now_ms,
                        outcome: RunOutcome::Skipped(SkipReason::Missed),
                        session_id: None,
                        routine_created: routine.created_at,
                    };
                    if let Err(error) = self.routines.record_run(run, Some(BaselineMove { to: now_ms, seen: routine.clone() })).await {
                        first_error.get_or_insert(error);
                    }
                }
                Decision::Run { due_ms } => {
                    self.mark_handled(&routine.id, due_ms);
                    let outcome = match self.try_start(&routine, due_ms, now_ms, zone).await {
                        Ok(()) => RunOutcome::Started,
                        Err(reason) => RunOutcome::Skipped(reason),
                    };
                    let run = RoutineRun { routine_id: routine.id.clone(), due_ms, started_ms: now_ms, outcome, session_id: None, routine_created: routine.created_at };
                    if let Err(error) = self.routines.record_run(run, Some(BaselineMove { to: due_ms, seen: routine.clone() })).await {
                        first_error.get_or_insert(error);
                    }
                }
            }
        }
        first_error.map_or(Ok(()), Err)
    }

    /// Starts a routine at once. It does not move the count and adds no line when it is refused:
    /// the person is told why instead.
    pub async fn run_now<Tz: TimeZone>(&self, id: &str, now_ms: i64, zone: &Tz) -> Result<(), RoutineError>
    where
        Tz::Offset: Display,
    {
        let (routines, _) = self.routines.snapshot().await?;
        let routine = routines.into_iter().find(|routine| routine.id == id).ok_or_else(|| RoutineError::NotFound(id.to_string()))?;
        self.try_start(&routine, now_ms, now_ms, zone).await.map_err(|reason| {
            RoutineError::Refused(match reason {
                SkipReason::Busy => "A session of a routine is still working in this folder.".to_string(),
                SkipReason::FolderMissing => "The folder no longer exists.".to_string(),
                SkipReason::StartFailed(error) => error,
                SkipReason::Missed => "Not run.".to_string(),
            })
        })?;
        let run = RoutineRun { routine_id: routine.id, due_ms: now_ms, started_ms: now_ms, outcome: RunOutcome::Started, session_id: None, routine_created: routine.created_at };
        self.routines.record_run(run, None).await
    }

    async fn try_start<Tz: TimeZone>(&self, routine: &Routine, due_ms: i64, now_ms: i64, zone: &Tz) -> Result<(), SkipReason>
    where
        Tz::Offset: Display,
    {
        if !self.host.folder_exists(&routine.folder) {
            return Err(SkipReason::FolderMissing);
        }
        if self.host.busy_in(&routine.folder) {
            return Err(SkipReason::Busy);
        }
        let prompt = match routine.instructions.trim() {
            "" => routine.description.trim().to_string(),
            text => text.to_string(),
        };
        let date = zone.timestamp_millis_opt(now_ms).single().map(|time| time.format("%b %-d").to_string()).unwrap_or_default();
        let request = RunRequest {
            routine_id: routine.id.clone(),
            routine_created: routine.created_at,
            due_ms,
            folder: routine.folder.clone(),
            permission_mode: routine.permission_mode,
            model: routine.model.clone(),
            prompt,
            title: format!("{date} – {}", routine.name),
            allowed_tools: routine.always_allowed.clone(),
        };
        self.host.start(request).await.map_err(SkipReason::StartFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};
    use forge_domain::{AppConfig, PermissionMode, RoutineDraft, Schedule};
    use forge_ports::{ConfigError, MockConfigRepositoryPort};
    use std::sync::Mutex;

    fn zone() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    fn at(day: u32, hour: u32, minute: u32) -> i64 {
        zone().with_ymd_and_hms(2026, 10, day, hour, minute, 0).unwrap().timestamp_millis()
    }

    /// A host that records what it was asked and can be told a folder is busy or gone. Starting a
    /// routine in a folder makes that folder busy, like the real host does.
    #[derive(Default)]
    struct FakeHost {
        started: Mutex<Vec<RunRequest>>,
        busy: Mutex<Vec<PathBuf>>,
        missing: Mutex<Vec<PathBuf>>,
        fail_with: Mutex<Option<String>>,
    }

    #[async_trait]
    impl RoutineHost for FakeHost {
        fn folder_exists(&self, folder: &Path) -> bool {
            !self.missing.lock().unwrap().iter().any(|missing| missing == folder)
        }
        fn busy_in(&self, folder: &Path) -> bool {
            self.busy.lock().unwrap().iter().any(|busy| busy == folder)
        }
        async fn start(&self, request: RunRequest) -> Result<(), String> {
            if let Some(error) = self.fail_with.lock().unwrap().clone() {
                return Err(error);
            }
            self.busy.lock().unwrap().push(request.folder.clone());
            self.started.lock().unwrap().push(request);
            Ok(())
        }
    }

    struct Setup {
        runner: RoutineRunner,
        routines: Arc<RoutinesUseCase>,
        host: Arc<FakeHost>,
        store: Arc<Mutex<AppConfig>>,
    }

    fn setup() -> Setup {
        let store = Arc::new(Mutex::new(AppConfig::default()));
        let mut mock = MockConfigRepositoryPort::new();
        let loading = store.clone();
        mock.expect_load().returning(move || Ok(loading.lock().unwrap().clone()));
        let saving = store.clone();
        mock.expect_save().returning(move |config: &AppConfig| {
            *saving.lock().unwrap() = config.clone();
            Ok(())
        });
        let routines = Arc::new(RoutinesUseCase::new(Arc::new(mock)));
        let host = Arc::new(FakeHost::default());
        let runner = RoutineRunner::new(routines.clone(), host.clone());
        Setup { runner, routines, host, store }
    }

    fn draft(name: &str, folder: &str, schedule: Schedule) -> RoutineDraft {
        RoutineDraft {
            name: name.to_string(),
            description: "d".to_string(),
            instructions: "Look at the commits".to_string(),
            folder: Some(folder.into()),
            permission_mode: PermissionMode::AcceptEdits,
            model: Some("m".to_string()),
            schedule,
        }
    }

    fn runs(setup: &Setup) -> Vec<RoutineRun> {
        setup.store.lock().unwrap().routine_runs.clone()
    }

    #[tokio::test]
    async fn a_due_routine_starts_a_session_with_its_settings_and_the_baseline_moves_to_the_due_time() {
        let setup = setup();
        let routine = setup.routines.create(draft("Review", "/work", Schedule::Daily { hour: 9, minute: 0 }), at(9, 12, 0)).await.unwrap();
        setup.routines.add_allowed("review", at(9, 12, 0), vec!["Read".to_string()]).await.unwrap();
        let due = at(10, 9, 0) + forge_domain::delay_ms("review");

        setup.runner.tick(due + 5_000, &zone()).await.unwrap();

        let started = setup.host.started.lock().unwrap().clone();
        assert_eq!(started.len(), 1);
        let request = &started[0];
        assert_eq!((request.routine_id.as_str(), request.due_ms), ("review", due));
        assert_eq!(request.folder, PathBuf::from("/work"));
        assert_eq!(request.permission_mode, PermissionMode::AcceptEdits);
        assert_eq!(request.model.as_deref(), Some("m"));
        assert_eq!(request.prompt, "Look at the commits");
        assert_eq!(request.title, "Oct 10 – Review");
        assert_eq!(request.allowed_tools, vec!["Read".to_string()]);
        let runs = runs(&setup);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].outcome, RunOutcome::Started);
        assert_eq!(setup.store.lock().unwrap().routines[0].baseline, due);
        assert_eq!(routine.baseline, at(9, 12, 0));

        // The same tick again does nothing: it was counted.
        setup.host.busy.lock().unwrap().clear();
        setup.runner.tick(due + 35_000, &zone()).await.unwrap();
        assert_eq!(setup.host.started.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn the_description_is_the_prompt_when_there_are_no_instructions() {
        let setup = setup();
        setup.routines.create(RoutineDraft { instructions: "  ".into(), ..draft("A", "/w", Schedule::Hourly) }, at(9, 6, 0)).await.unwrap();
        setup.runner.tick(at(9, 10, 10), &zone()).await.unwrap();
        assert_eq!(setup.host.started.lock().unwrap()[0].prompt, "d");
    }

    #[tokio::test]
    async fn nothing_before_the_time_and_nothing_for_a_paused_or_manual_routine() {
        let setup = setup();
        setup.routines.create(draft("Soon", "/a", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        setup.routines.create(draft("Manual", "/b", Schedule::Manual), at(9, 6, 0)).await.unwrap();
        setup.routines.create(draft("Paused", "/c", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        setup.routines.set_enabled("paused", false, at(9, 6, 0)).await.unwrap();
        setup.runner.tick(at(9, 6, 30), &zone()).await.unwrap();
        assert!(setup.host.started.lock().unwrap().is_empty());
        setup.runner.tick(at(9, 10, 10), &zone()).await.unwrap();
        let started: Vec<String> = setup.host.started.lock().unwrap().iter().map(|request| request.routine_id.clone()).collect();
        assert_eq!(started, vec!["soon".to_string()]);
    }

    #[tokio::test]
    async fn a_run_more_than_thirty_minutes_late_is_one_missed_line_and_the_count_jumps_to_now() {
        let setup = setup();
        setup.routines.create(draft("A", "/w", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        let now = at(9, 12, 40) + forge_domain::delay_ms("a");
        setup.runner.tick(now, &zone()).await.unwrap();
        assert!(setup.host.started.lock().unwrap().is_empty());
        let runs = runs(&setup);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].outcome, RunOutcome::Skipped(SkipReason::Missed));
        assert_eq!(setup.store.lock().unwrap().routines[0].baseline, now);
        // The next tick finds nothing new.
        setup.runner.tick(now + 30_000, &zone()).await.unwrap();
        assert_eq!(setup.store.lock().unwrap().routine_runs.len(), 1);
    }

    #[tokio::test]
    async fn a_once_a_minute_routine_woken_after_days_leaves_one_line_and_history_stays_capped() {
        let setup = setup();
        setup.routines.create(draft("Often", "/w", Schedule::Cron { expression: "* * * * *".into() }), at(1, 0, 0)).await.unwrap();
        setup.runner.tick(at(9, 12, 0), &zone()).await.unwrap();
        assert_eq!(runs(&setup).len(), 1);
        for minute in 0..70 {
            let now = at(9, 12, 1) + minute * 60_000;
            setup.host.busy.lock().unwrap().clear();
            setup.runner.tick(now, &zone()).await.unwrap();
        }
        assert!(runs(&setup).len() <= forge_domain::KEEP_RUNS);
    }

    #[tokio::test]
    async fn a_second_routine_in_a_busy_folder_is_skipped_as_busy() {
        let setup = setup();
        setup.routines.create(draft("First", "/same", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        setup.routines.create(draft("Second", "/same", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        // Both are due in this tick, whatever their delays are.
        setup.runner.tick(at(9, 10, 10), &zone()).await.unwrap();
        assert_eq!(setup.host.started.lock().unwrap().len(), 1, "one started");
        let outcomes: Vec<RunOutcome> = runs(&setup).into_iter().map(|run| run.outcome).collect();
        assert!(outcomes.contains(&RunOutcome::Started));
        assert!(outcomes.contains(&RunOutcome::Skipped(SkipReason::Busy)));
    }

    #[tokio::test]
    async fn a_missing_folder_and_a_failed_start_are_recorded_and_not_retried_every_tick() {
        let setup = setup();
        setup.routines.create(draft("Gone", "/gone", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        setup.host.missing.lock().unwrap().push("/gone".into());
        setup.runner.tick(at(9, 10, 10), &zone()).await.unwrap();
        assert_eq!(runs(&setup)[0].outcome, RunOutcome::Skipped(SkipReason::FolderMissing));
        setup.runner.tick(at(9, 10, 11), &zone()).await.unwrap();
        assert_eq!(runs(&setup).len(), 1);

        setup.routines.create(draft("Broken", "/ok", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        *setup.host.fail_with.lock().unwrap() = Some("no sidecar".to_string());
        setup.runner.tick(at(9, 10, 12), &zone()).await.unwrap();
        let last = runs(&setup).pop().unwrap();
        assert_eq!(last.outcome, RunOutcome::Skipped(SkipReason::StartFailed("no sidecar".to_string())));
    }

    #[tokio::test]
    async fn a_routine_with_a_broken_cron_expression_is_never_due_and_does_not_stop_the_others() {
        let setup = setup();
        setup.routines.create(draft("Fine", "/a", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        // A config edited by hand can hold what the form would refuse. (Clone first: the store's
        // mutex is not re-entrant, so it must not be locked twice in one expression.)
        let template = setup.store.lock().unwrap().routines[0].clone();
        let bad = forge_domain::Routine { schedule: Schedule::Cron { expression: "*/0 * * * *".into() }, id: "bad".into(), ..template };
        setup.store.lock().unwrap().routines.push(bad);
        setup.runner.tick(at(9, 10, 10), &zone()).await.unwrap();
        let started: Vec<String> = setup.host.started.lock().unwrap().iter().map(|request| request.routine_id.clone()).collect();
        assert_eq!(started, vec!["fine".to_string()]);
    }

    #[tokio::test]
    async fn run_now_starts_at_once_without_moving_the_count_and_says_why_when_it_cannot() {
        let setup = setup();
        setup.routines.create(draft("A", "/w", Schedule::Manual), at(9, 6, 0)).await.unwrap();
        setup.runner.run_now("a", at(9, 7, 0), &zone()).await.unwrap();
        assert_eq!(setup.host.started.lock().unwrap().len(), 1);
        assert_eq!(setup.store.lock().unwrap().routines[0].baseline, at(9, 6, 0));
        assert_eq!(runs(&setup).len(), 1);

        // The folder is busy now (the fake marks it): refused, and no line is added.
        let error = setup.runner.run_now("a", at(9, 7, 5), &zone()).await.unwrap_err();
        assert!(matches!(error, RoutineError::Refused(ref text) if text.contains("still working")), "{error}");
        assert_eq!(runs(&setup).len(), 1);

        setup.host.busy.lock().unwrap().clear();
        setup.host.missing.lock().unwrap().push("/w".into());
        let error = setup.runner.run_now("a", at(9, 7, 6), &zone()).await.unwrap_err();
        assert!(matches!(error, RoutineError::Refused(ref text) if text.contains("no longer exists")), "{error}");
        assert!(matches!(setup.runner.run_now("nope", 0, &zone()).await, Err(RoutineError::NotFound(_))));
    }

    #[tokio::test]
    async fn a_save_that_fails_after_a_start_does_not_start_the_routine_again_and_the_others_still_run() {
        use std::sync::atomic::{AtomicBool, Ordering};
        let store = Arc::new(Mutex::new(AppConfig::default()));
        let fail_saves = Arc::new(AtomicBool::new(false));
        let mut mock = MockConfigRepositoryPort::new();
        let loading = store.clone();
        mock.expect_load().returning(move || Ok(loading.lock().unwrap().clone()));
        let (saving, failing) = (store.clone(), fail_saves.clone());
        mock.expect_save().returning(move |config: &AppConfig| {
            if failing.load(Ordering::SeqCst) {
                return Err(ConfigError::WriteFailed("read-only".to_string()));
            }
            *saving.lock().unwrap() = config.clone();
            Ok(())
        });
        let routines = Arc::new(RoutinesUseCase::new(Arc::new(mock)));
        let host = Arc::new(FakeHost::default());
        let runner = RoutineRunner::new(routines.clone(), host.clone());
        routines.create(draft("First", "/a", Schedule::Hourly), at(9, 6, 0)).await.unwrap();
        routines.create(draft("Second", "/b", Schedule::Hourly), at(9, 6, 0)).await.unwrap();

        fail_saves.store(true, Ordering::SeqCst);
        let result = runner.tick(at(9, 10, 10), &zone()).await;
        assert!(matches!(result, Err(RoutineError::Config(_))), "the failure is reported");
        assert_eq!(host.started.lock().unwrap().len(), 2, "the second routine was still looked at");

        // The baseline never moved on disk, but the runner remembers what it handled.
        host.busy.lock().unwrap().clear();
        let _ = runner.tick(at(9, 10, 11), &zone()).await;
        assert_eq!(host.started.lock().unwrap().len(), 2, "not started again");
    }

    #[tokio::test]
    async fn a_config_that_cannot_be_read_stops_the_tick_and_writes_nothing() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(|| Err(ConfigError::ParseFailed("bad".to_string())));
        mock.expect_save().never();
        let host = Arc::new(FakeHost::default());
        let runner = RoutineRunner::new(Arc::new(RoutinesUseCase::new(Arc::new(mock))), host.clone());
        assert!(matches!(runner.tick(at(9, 10, 0), &zone()).await, Err(RoutineError::Config(_))));
        assert!(host.started.lock().unwrap().is_empty());
    }
}
