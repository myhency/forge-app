use std::sync::Arc;

use forge_domain::{push_run, routine_id, validate_draft, AppConfig, Routine, RoutineDraft, RoutineRun};
use forge_ports::{ConfigError, ConfigRepositoryPort};
use thiserror::Error;

use super::config_lock::ConfigLock;

#[derive(Debug, Error)]
pub enum RoutineError {
    /// The form is not valid; the text says what to fix.
    #[error("{0}")]
    Invalid(String),
    #[error("no routine called {0}")]
    NotFound(String),
    /// Run now was not possible; the text says why.
    #[error("{0}")]
    Refused(String),
    #[error("failed to read or write the Forge config: {0}")]
    Config(#[from] ConfigError),
}

/// Keeps the routines and their history in Forge's config. Every change loads, edits and saves
/// under the shared lock, and a config that cannot be read is never replaced.
pub struct RoutinesUseCase {
    config: Arc<dyn ConfigRepositoryPort>,
    write_lock: ConfigLock,
}

impl RoutinesUseCase {
    pub fn new(config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { config, write_lock: ConfigLock::default() }
    }

    /// Shares the lock the other config writers hold (see `ConfigLock`).
    pub fn with_config_lock(mut self, lock: ConfigLock) -> Self {
        self.write_lock = lock;
        self
    }

    pub async fn snapshot(&self) -> Result<(Vec<Routine>, Vec<RoutineRun>), RoutineError> {
        let config = self.config.load().await?;
        Ok((config.routines, config.routine_runs))
    }

    async fn edit<T>(&self, change: impl FnOnce(&mut AppConfig) -> Result<T, RoutineError>) -> Result<T, RoutineError> {
        let _saving = self.write_lock.hold().await;
        let mut config = self.config.load().await?;
        let value = change(&mut config)?;
        self.config.save(&config).await?;
        Ok(value)
    }

    pub async fn create(&self, draft: RoutineDraft, now_ms: i64) -> Result<Routine, RoutineError> {
        validate_draft(&draft).map_err(RoutineError::Invalid)?;
        self.edit(|config| {
            let taken: Vec<String> = config.routines.iter().map(|routine| routine.id.clone()).collect();
            let routine = Routine {
                id: routine_id(&draft.name, &taken),
                name: draft.name.trim().to_string(),
                description: draft.description.trim().to_string(),
                instructions: draft.instructions,
                folder: draft.folder.unwrap_or_default(),
                permission_mode: draft.permission_mode,
                model: draft.model,
                schedule: draft.schedule,
                enabled: true,
                created_at: now_ms,
                baseline: now_ms,
                always_allowed: Vec::new(),
            };
            config.routines.push(routine.clone());
            Ok(routine)
        })
        .await
    }

    /// Changes everything the form holds. The id, the creation time and the allowed rules stay; the
    /// count restarts from `now_ms` only when the schedule changed.
    pub async fn update(&self, id: &str, draft: RoutineDraft, now_ms: i64) -> Result<Routine, RoutineError> {
        validate_draft(&draft).map_err(RoutineError::Invalid)?;
        self.edit(|config| {
            let routine = find(config, id)?;
            if routine.schedule != draft.schedule {
                routine.baseline = now_ms;
            }
            routine.name = draft.name.trim().to_string();
            routine.description = draft.description.trim().to_string();
            routine.instructions = draft.instructions;
            routine.folder = draft.folder.unwrap_or_default();
            routine.permission_mode = draft.permission_mode;
            routine.model = draft.model;
            routine.schedule = draft.schedule;
            Ok(routine.clone())
        })
        .await
    }

    /// Removes the routine and its history. The sessions it made stay.
    pub async fn delete(&self, id: &str) -> Result<(), RoutineError> {
        self.edit(|config| {
            find(config, id)?;
            config.routines.retain(|routine| routine.id != id);
            config.routine_runs.retain(|run| run.routine_id != id);
            Ok(())
        })
        .await
    }

    /// Switching on counts from `now_ms`, so nothing that was due while it was off runs.
    pub async fn set_enabled(&self, id: &str, enabled: bool, now_ms: i64) -> Result<Routine, RoutineError> {
        self.edit(|config| {
            let routine = find(config, id)?;
            if enabled && !routine.enabled {
                routine.baseline = now_ms;
            }
            routine.enabled = enabled;
            Ok(routine.clone())
        })
        .await
    }

    /// Adds a history line and, when `baseline` is given, moves the routine's count there. A routine
    /// deleted meanwhile only loses the baseline; the line is kept.
    pub async fn record_run(&self, run: RoutineRun, baseline: Option<i64>) -> Result<(), RoutineError> {
        self.edit(|config| {
            if let Some(baseline) = baseline {
                if let Some(routine) = config.routines.iter_mut().find(|routine| routine.id == run.routine_id) {
                    routine.baseline = baseline;
                }
            }
            push_run(&mut config.routine_runs, run);
            Ok(())
        })
        .await
    }

    /// Fills in the Claude session id of a run once it is known. A run that is gone, or already has
    /// one, is left alone.
    pub async fn set_run_session(&self, routine_id: &str, due_ms: i64, session_id: String) -> Result<(), RoutineError> {
        self.edit(|config| {
            if let Some(run) = config
                .routine_runs
                .iter_mut()
                .find(|run| run.routine_id == routine_id && run.due_ms == due_ms && run.session_id.is_none())
            {
                run.session_id = Some(session_id);
            }
            Ok(())
        })
        .await
    }

    pub async fn add_allowed(&self, id: &str, rules: Vec<String>) -> Result<Routine, RoutineError> {
        self.edit(|config| {
            let routine = find(config, id)?;
            for rule in rules {
                if !routine.always_allowed.contains(&rule) {
                    routine.always_allowed.push(rule);
                }
            }
            Ok(routine.clone())
        })
        .await
    }

    pub async fn remove_allowed(&self, id: &str, rule: &str) -> Result<Routine, RoutineError> {
        self.edit(|config| {
            let routine = find(config, id)?;
            routine.always_allowed.retain(|allowed| allowed != rule);
            Ok(routine.clone())
        })
        .await
    }
}

fn find<'a>(config: &'a mut AppConfig, id: &str) -> Result<&'a mut Routine, RoutineError> {
    config.routines.iter_mut().find(|routine| routine.id == id).ok_or_else(|| RoutineError::NotFound(id.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{PermissionMode, RunOutcome, Schedule, SkipReason};
    use forge_ports::MockConfigRepositoryPort;
    use std::sync::Mutex;

    /// A config kept in memory: what was saved is what the next load reads.
    fn memory() -> (RoutinesUseCase, Arc<Mutex<AppConfig>>) {
        let store = Arc::new(Mutex::new(AppConfig::default()));
        let mut mock = MockConfigRepositoryPort::new();
        let loading = store.clone();
        mock.expect_load().returning(move || Ok(loading.lock().unwrap().clone()));
        let saving = store.clone();
        mock.expect_save().returning(move |config: &AppConfig| {
            *saving.lock().unwrap() = config.clone();
            Ok(())
        });
        (RoutinesUseCase::new(Arc::new(mock)), store)
    }

    fn draft(name: &str) -> RoutineDraft {
        RoutineDraft {
            name: name.to_string(),
            description: "d".to_string(),
            instructions: "do it".to_string(),
            folder: Some("/work".into()),
            permission_mode: PermissionMode::Default,
            model: None,
            schedule: Schedule::Hourly,
        }
    }

    fn run(routine: &str, due: i64) -> RoutineRun {
        RoutineRun { routine_id: routine.to_string(), due_ms: due, started_ms: due, outcome: RunOutcome::Started, session_id: None }
    }

    #[tokio::test]
    async fn create_saves_a_routine_that_starts_counting_from_now() {
        let (routines, store) = memory();
        let made = routines.create(draft("Daily review"), 1_000).await.unwrap();
        assert_eq!(made.id, "daily-review");
        assert!(made.enabled);
        assert_eq!((made.created_at, made.baseline), (1_000, 1_000));
        assert_eq!(store.lock().unwrap().routines, vec![made]);
    }

    #[tokio::test]
    async fn two_routines_with_one_name_get_two_ids() {
        let (routines, _) = memory();
        let first = routines.create(draft("Same"), 1).await.unwrap();
        let second = routines.create(draft("Same"), 2).await.unwrap();
        assert_eq!((first.id.as_str(), second.id.as_str()), ("same", "same-2"));
    }

    #[tokio::test]
    async fn an_invalid_draft_is_refused_and_nothing_is_saved() {
        let (routines, store) = memory();
        let error = routines.create(RoutineDraft { name: " ".into(), ..draft("x") }, 1).await.unwrap_err();
        assert!(matches!(error, RoutineError::Invalid(ref text) if text == "Name is required"), "{error}");
        assert!(store.lock().unwrap().routines.is_empty());
    }

    #[tokio::test]
    async fn update_keeps_the_id_and_the_allowed_rules_and_restarts_the_count_only_when_the_schedule_changes() {
        let (routines, _) = memory();
        let made = routines.create(draft("A"), 10).await.unwrap();
        routines.add_allowed("a", vec!["Bash(npm test:*)".to_string()]).await.unwrap();
        let same_schedule = routines.update("a", RoutineDraft { description: "new".into(), ..draft("Renamed") }, 99).await.unwrap();
        assert_eq!(same_schedule.id, "a", "the id never changes");
        assert_eq!(same_schedule.name, "Renamed");
        assert_eq!(same_schedule.baseline, made.baseline, "same schedule: the count is kept");
        assert_eq!(same_schedule.always_allowed, vec!["Bash(npm test:*)".to_string()]);
        let moved = routines.update("a", RoutineDraft { schedule: Schedule::Daily { hour: 9, minute: 0 }, ..draft("Renamed") }, 99).await.unwrap();
        assert_eq!(moved.baseline, 99, "a new schedule counts from now");
    }

    #[tokio::test]
    async fn switching_on_counts_from_now_so_nothing_runs_for_the_paused_time() {
        let (routines, _) = memory();
        routines.create(draft("A"), 10).await.unwrap();
        let off = routines.set_enabled("a", false, 20).await.unwrap();
        assert!(!off.enabled);
        assert_eq!(off.baseline, 10, "switching off changes nothing else");
        let on = routines.set_enabled("a", true, 500).await.unwrap();
        assert!(on.enabled);
        assert_eq!(on.baseline, 500);
    }

    #[tokio::test]
    async fn delete_removes_the_routine_and_its_history() {
        let (routines, store) = memory();
        routines.create(draft("A"), 1).await.unwrap();
        routines.create(draft("B"), 1).await.unwrap();
        routines.record_run(run("a", 1), None).await.unwrap();
        routines.record_run(run("b", 1), None).await.unwrap();
        routines.delete("a").await.unwrap();
        let config = store.lock().unwrap().clone();
        assert_eq!(config.routines.iter().map(|routine| routine.id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert_eq!(config.routine_runs.iter().map(|run| run.routine_id.as_str()).collect::<Vec<_>>(), vec!["b"]);
        assert!(matches!(routines.delete("a").await, Err(RoutineError::NotFound(_))));
    }

    #[tokio::test]
    async fn record_run_moves_the_baseline_only_when_asked_and_ignores_a_deleted_routine() {
        let (routines, store) = memory();
        routines.create(draft("A"), 1).await.unwrap();
        routines.record_run(run("a", 5), None).await.unwrap();
        assert_eq!(store.lock().unwrap().routines[0].baseline, 1);
        routines.record_run(run("a", 6), Some(77)).await.unwrap();
        assert_eq!(store.lock().unwrap().routines[0].baseline, 77);
        routines.record_run(run("gone", 7), Some(1)).await.unwrap();
        assert_eq!(store.lock().unwrap().routine_runs.len(), 3, "the line is kept");
    }

    #[tokio::test]
    async fn a_session_id_is_attached_once_to_the_matching_run_and_a_missing_run_is_ignored() {
        let (routines, store) = memory();
        routines.create(draft("A"), 1).await.unwrap();
        routines.record_run(run("a", 5), None).await.unwrap();
        routines.set_run_session("a", 5, "s-1".to_string()).await.unwrap();
        routines.set_run_session("a", 5, "s-2".to_string()).await.unwrap();
        routines.set_run_session("a", 999, "s-3".to_string()).await.unwrap();
        routines.set_run_session("gone", 5, "s-4".to_string()).await.unwrap();
        assert_eq!(store.lock().unwrap().routine_runs[0].session_id.as_deref(), Some("s-1"));
    }

    #[tokio::test]
    async fn skips_are_kept_with_their_reason() {
        let (routines, store) = memory();
        routines.create(draft("A"), 1).await.unwrap();
        let skipped = RoutineRun { outcome: RunOutcome::Skipped(SkipReason::Busy), ..run("a", 5) };
        routines.record_run(skipped.clone(), Some(5)).await.unwrap();
        assert_eq!(store.lock().unwrap().routine_runs, vec![skipped]);
    }

    #[tokio::test]
    async fn allowed_rules_are_added_once_and_can_be_removed() {
        let (routines, _) = memory();
        routines.create(draft("A"), 1).await.unwrap();
        routines.add_allowed("a", vec!["Read".to_string(), "Bash(ls:*)".to_string()]).await.unwrap();
        let routine = routines.add_allowed("a", vec!["Read".to_string()]).await.unwrap();
        assert_eq!(routine.always_allowed, vec!["Read".to_string(), "Bash(ls:*)".to_string()]);
        let routine = routines.remove_allowed("a", "Read").await.unwrap();
        assert_eq!(routine.always_allowed, vec!["Bash(ls:*)".to_string()]);
    }

    #[tokio::test]
    async fn a_config_that_cannot_be_read_is_never_replaced() {
        let mut mock = MockConfigRepositoryPort::new();
        mock.expect_load().returning(|| Err(forge_ports::ConfigError::ParseFailed("bad json".to_string())));
        mock.expect_save().never();
        let routines = RoutinesUseCase::new(Arc::new(mock));
        assert!(matches!(routines.create(draft("A"), 1).await, Err(RoutineError::Config(_))));
        assert!(matches!(routines.set_enabled("a", true, 1).await, Err(RoutineError::Config(_))));
        assert!(matches!(routines.snapshot().await, Err(RoutineError::Config(_))));
    }
}
