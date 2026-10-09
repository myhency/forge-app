use std::path::PathBuf;

use forge_application::Phase;
use forge_domain::{PermissionMode, Routine, RoutineDraft, RoutineRun, RunOutcome, Schedule, SkipReason};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutineSort {
    #[default]
    Name,
    LastRun,
    Created,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RoutineFilter {
    #[default]
    All,
    On,
    Paused,
}

/// What the list shows for one routine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineCard {
    pub id: String,
    pub name: String,
    pub description: String,
    /// "On", "Paused" or "Manual".
    pub state: String,
    pub schedule: String,
    /// A session of this routine waits for a permission answer.
    pub waiting: bool,
}

impl RoutineCard {
    /// The card's second line: "On · Every hour"; a manual routine says "Manual" once.
    pub fn meta(&self) -> String {
        if self.state == self.schedule {
            self.state.clone()
        } else {
            format!("{} · {}", self.state, self.schedule)
        }
    }
}

fn card_state(routine: &Routine) -> &'static str {
    if !routine.enabled {
        "Paused"
    } else if matches!(routine.schedule, Schedule::Manual) {
        "Manual"
    } else {
        "On"
    }
}

/// The cards for the list: filtered, searched and sorted. A manual routine is "on" for the filter
/// (it is not paused).
pub fn cards(routines: &[Routine], runs: &[RoutineRun], waiting: &[String], query: &str, filter: RoutineFilter, sort: RoutineSort) -> Vec<RoutineCard> {
    let query = query.trim().to_lowercase();
    let last_run = |id: &str| runs.iter().filter(|run| run.routine_id == id).map(|run| run.due_ms).max();
    let mut chosen: Vec<&Routine> = routines
        .iter()
        .filter(|routine| match filter {
            RoutineFilter::All => true,
            RoutineFilter::On => routine.enabled,
            RoutineFilter::Paused => !routine.enabled,
        })
        .filter(|routine| query.is_empty() || routine.name.to_lowercase().contains(&query) || routine.description.to_lowercase().contains(&query))
        .collect();
    match sort {
        RoutineSort::Name => chosen.sort_by_key(|routine| routine.name.to_lowercase()),
        RoutineSort::Created => chosen.sort_by_key(|routine| std::cmp::Reverse(routine.created_at)),
        RoutineSort::LastRun => chosen.sort_by_key(|routine| std::cmp::Reverse(last_run(&routine.id))),
    }
    chosen
        .into_iter()
        .map(|routine| RoutineCard {
            id: routine.id.clone(),
            name: routine.name.clone(),
            description: routine.description.clone(),
            state: card_state(routine).to_string(),
            schedule: routine.schedule.label(),
            waiting: waiting.contains(&routine.id),
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleKind {
    Manual,
    Hourly,
    Daily,
    Weekdays,
    Weekly,
    Custom,
}

/// What the form holds while it is open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormDraft {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub folder: Option<PathBuf>,
    pub mode: PermissionMode,
    pub model: Option<String>,
    pub kind: ScheduleKind,
    pub hour: u8,
    pub minute: u8,
    pub day: u8,
    pub cron: String,
}

impl FormDraft {
    /// An empty form: every day, and a time to start from.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            instructions: String::new(),
            folder: None,
            mode: PermissionMode::Default,
            model: None,
            kind: ScheduleKind::Daily,
            hour: 9,
            minute: 0,
            day: 1,
            cron: "0 9 * * *".to_string(),
        }
    }

    pub fn from_routine(routine: &Routine) -> Self {
        let mut form = Self {
            name: routine.name.clone(),
            description: routine.description.clone(),
            instructions: routine.instructions.clone(),
            folder: Some(routine.folder.clone()),
            mode: routine.permission_mode,
            model: routine.model.clone(),
            ..Self::new()
        };
        match &routine.schedule {
            Schedule::Manual => form.kind = ScheduleKind::Manual,
            Schedule::Hourly => form.kind = ScheduleKind::Hourly,
            Schedule::Daily { hour, minute } => (form.kind, form.hour, form.minute) = (ScheduleKind::Daily, *hour, *minute),
            Schedule::Weekdays { hour, minute } => (form.kind, form.hour, form.minute) = (ScheduleKind::Weekdays, *hour, *minute),
            Schedule::Weekly { day, hour, minute } => (form.kind, form.day, form.hour, form.minute) = (ScheduleKind::Weekly, *day, *hour, *minute),
            Schedule::Cron { expression } => (form.kind, form.cron) = (ScheduleKind::Custom, expression.clone()),
        }
        form
    }

    fn schedule(&self) -> Schedule {
        match self.kind {
            ScheduleKind::Manual => Schedule::Manual,
            ScheduleKind::Hourly => Schedule::Hourly,
            ScheduleKind::Daily => Schedule::Daily { hour: self.hour, minute: self.minute },
            ScheduleKind::Weekdays => Schedule::Weekdays { hour: self.hour, minute: self.minute },
            ScheduleKind::Weekly => Schedule::Weekly { day: self.day, hour: self.hour, minute: self.minute },
            ScheduleKind::Custom => Schedule::Cron { expression: self.cron.trim().to_string() },
        }
    }

    pub fn to_draft(&self) -> RoutineDraft {
        RoutineDraft {
            name: self.name.clone(),
            description: self.description.clone(),
            instructions: self.instructions.clone(),
            folder: self.folder.clone(),
            permission_mode: self.mode,
            model: self.model.clone(),
            schedule: self.schedule(),
        }
    }

    /// The first thing wrong with the form, in words; `None` when it can be saved.
    pub fn error(&self) -> Option<String> {
        forge_domain::validate_draft(&self.to_draft()).err()
    }

    pub fn can_submit(&self) -> bool {
        self.error().is_none()
    }

    /// Whether anything differs from the form as it was opened.
    pub fn is_dirty_against(&self, start: &FormDraft) -> bool {
        self != start
    }
}

impl Default for FormDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// One line of a routine's history, as drawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryRow {
    pub due_ms: i64,
    pub text: String,
    /// The session to open on a click; `None` for a skip.
    pub session_id: Option<String>,
}

pub fn skip_text(reason: &SkipReason) -> String {
    match reason {
        SkipReason::Missed => "Skipped: Forge was closed or the computer was asleep".to_string(),
        SkipReason::Busy => "Skipped: a session of this routine's folder was still working".to_string(),
        SkipReason::FolderMissing => "Skipped: the folder no longer exists".to_string(),
        SkipReason::StartFailed(error) => format!("Skipped: could not start ({error})"),
    }
}

/// What a started run is now, read from its session: `open` is the session's phase and how many
/// permission requests it holds, or `None` when the session is not open any more.
pub fn run_state_text(open: Option<(Phase, usize)>) -> &'static str {
    match open {
        Some((_, pending)) if pending > 0 => "Waiting for approval",
        Some((Phase::Running, _)) => "Running",
        _ => "Done",
    }
}

/// The history of one routine, newest first. `state_of` names what a started run is now ("Running",
/// "Waiting for approval", "Done"); the caller reads it from the run's session.
pub fn history_rows(runs: &[RoutineRun], routine_id: &str, state_of: &dyn Fn(&str) -> &'static str) -> Vec<HistoryRow> {
    let mut rows: Vec<&RoutineRun> = runs.iter().filter(|run| run.routine_id == routine_id).collect();
    rows.sort_by_key(|run| std::cmp::Reverse(run.due_ms));
    rows.into_iter()
        .map(|run| match &run.outcome {
            RunOutcome::Started => HistoryRow {
                due_ms: run.due_ms,
                text: state_of(run.session_id.as_deref().unwrap_or("")).to_string(),
                session_id: run.session_id.clone(),
            },
            RunOutcome::Skipped(reason) => HistoryRow { due_ms: run.due_ms, text: skip_text(reason), session_id: None },
        })
        .collect()
}

/// The notification for a routine whose session waits for the person: `(title, body)`.
pub fn waiting_notice(routine_name: &str, tool_name: &str) -> (String, String) {
    ("Routine needs your approval".to_string(), format!("{routine_name} wants to use {tool_name}"))
}

/// The UTC offset `zone` has at `ms`: summer and winter times each get their own.
pub fn offset_at<Tz: chrono::TimeZone>(zone: &Tz, ms: i64) -> time::UtcOffset {
    use chrono::Offset;
    zone.timestamp_millis_opt(ms)
        .single()
        .and_then(|instant| time::UtcOffset::from_whole_seconds(instant.offset().fix().local_minus_utc()).ok())
        .unwrap_or(time::UtcOffset::UTC)
}

/// `when_label` in the person's own time zone, with the offset that was in force at `ms`.
pub fn local_when_label(ms: i64) -> String {
    when_label(ms, offset_at(&chrono::Local, ms))
}

const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/// "Oct 9, 10:19 AM" in `offset`.
pub fn when_label(ms: i64, offset: time::UtcOffset) -> String {
    let Ok(utc) = time::OffsetDateTime::from_unix_timestamp(ms.div_euclid(1000)) else {
        return String::new();
    };
    let local = utc.to_offset(offset);
    let hour = local.hour();
    let (twelve, suffix) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };
    format!("{} {}, {twelve}:{:02} {suffix}", MONTHS[usize::from(u8::from(local.month())) - 1], local.day(), local.minute())
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_application::Phase;
    use forge_domain::{RunOutcome, Schedule, SkipReason};

    fn routine(id: &str, name: &str, enabled: bool, schedule: Schedule, created: i64) -> Routine {
        Routine {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("about {name}"),
            instructions: String::new(),
            folder: "/work".into(),
            permission_mode: PermissionMode::Default,
            model: None,
            schedule,
            enabled,
            created_at: created,
            baseline: created,
            always_allowed: Vec::new(),
        }
    }

    fn run(id: &str, due: i64) -> RoutineRun {
        RoutineRun { routine_id: id.to_string(), due_ms: due, started_ms: due, outcome: RunOutcome::Started, session_id: Some("s".to_string()) }
    }

    fn sample() -> Vec<Routine> {
        vec![
            routine("b", "Beta", true, Schedule::Hourly, 2),
            routine("a", "alpha", false, Schedule::Daily { hour: 9, minute: 49 }, 3),
            routine("c", "Gamma", true, Schedule::Manual, 1),
        ]
    }

    #[test]
    fn a_card_says_on_paused_or_manual_with_the_schedule() {
        let all = cards(&sample(), &[], &[], "", RoutineFilter::All, RoutineSort::Created);
        let by_id = |id: &str| all.iter().find(|card| card.id == id).unwrap().clone();
        assert_eq!((by_id("b").state.as_str(), by_id("b").schedule.as_str()), ("On", "Every hour"));
        assert_eq!((by_id("a").state.as_str(), by_id("a").schedule.as_str()), ("Paused", "Every day at 9:49 AM"));
        assert_eq!((by_id("c").state.as_str(), by_id("c").schedule.as_str()), ("Manual", "Manual"));
    }

    #[test]
    fn a_cards_second_line_does_not_say_manual_twice() {
        let all = cards(&sample(), &[], &[], "", RoutineFilter::All, RoutineSort::Name);
        let meta = |id: &str| all.iter().find(|card| card.id == id).unwrap().meta();
        assert_eq!(meta("c"), "Manual", "a manual routine is not 'Manual · Manual'");
        assert_eq!(meta("b"), "On · Every hour");
        assert_eq!(meta("a"), "Paused · Every day at 9:49 AM");
    }

    #[test]
    fn the_filter_keeps_on_or_paused_and_a_manual_routine_counts_as_on() {
        let ids = |filter| cards(&sample(), &[], &[], "", filter, RoutineSort::Name).into_iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(ids(RoutineFilter::On), vec!["b", "c"]);
        assert_eq!(ids(RoutineFilter::Paused), vec!["a"]);
        assert_eq!(ids(RoutineFilter::All), vec!["a", "b", "c"]);
    }

    #[test]
    fn search_matches_name_and_description_without_case() {
        let ids = |query: &str| cards(&sample(), &[], &[], query, RoutineFilter::All, RoutineSort::Name).into_iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(ids("ALPHA"), vec!["a"]);
        assert_eq!(ids("about g"), vec!["c"]);
        assert!(ids("zzz").is_empty());
    }

    #[test]
    fn sorting_by_name_ignores_case_by_created_puts_the_newest_first_and_by_last_run_the_latest_first() {
        let runs = vec![run("c", 50), run("b", 90), run("c", 70)];
        let order = |sort| cards(&sample(), &runs, &[], "", RoutineFilter::All, sort).into_iter().map(|card| card.id).collect::<Vec<_>>();
        assert_eq!(order(RoutineSort::Name), vec!["a", "b", "c"]);
        assert_eq!(order(RoutineSort::Created), vec!["a", "b", "c"]);
        assert_eq!(order(RoutineSort::LastRun), vec!["b", "c", "a"], "a routine that never ran goes last");
    }

    #[test]
    fn a_waiting_routine_is_marked() {
        let all = cards(&sample(), &[], &["b".to_string()], "", RoutineFilter::All, RoutineSort::Name);
        assert!(all.iter().find(|card| card.id == "b").unwrap().waiting);
        assert!(!all.iter().find(|card| card.id == "a").unwrap().waiting);
    }

    #[test]
    fn the_form_starts_empty_on_a_daily_schedule_and_cannot_be_saved() {
        let form = FormDraft::new();
        assert_eq!(form.kind, ScheduleKind::Daily);
        assert!(!form.can_submit());
        assert_eq!(form.error().as_deref(), Some("Name is required"));
    }

    #[test]
    fn each_schedule_kind_turns_into_its_schedule_and_back() {
        for schedule in [
            Schedule::Manual,
            Schedule::Hourly,
            Schedule::Daily { hour: 7, minute: 30 },
            Schedule::Weekdays { hour: 18, minute: 0 },
            Schedule::Weekly { day: 3, hour: 8, minute: 15 },
            Schedule::Cron { expression: "*/10 * * * *".to_string() },
        ] {
            let mut source = routine("a", "A", true, schedule.clone(), 1);
            source.model = Some("m".to_string());
            let form = FormDraft::from_routine(&source);
            assert_eq!(form.to_draft().schedule, schedule);
            assert_eq!(form.to_draft().model.as_deref(), Some("m"));
            assert_eq!(form.to_draft().folder, Some(PathBuf::from("/work")));
        }
    }

    #[test]
    fn a_form_with_everything_filled_can_be_saved_and_a_bad_cron_blocks_it_with_the_reason() {
        let mut form = FormDraft::new();
        form.name = "Review".into();
        form.description = "Look".into();
        assert_eq!(form.error().as_deref(), Some("Choose a folder"));
        form.folder = Some("/work".into());
        assert!(form.can_submit());
        form.kind = ScheduleKind::Custom;
        form.cron = "* * *".into();
        assert!(!form.can_submit());
        assert_eq!(form.error().as_deref(), Some("A cron expression has five fields: minute hour day month weekday"));
    }

    #[test]
    fn the_form_is_dirty_only_when_something_differs_from_where_it_started() {
        let start = FormDraft::new();
        let mut form = start.clone();
        assert!(!form.is_dirty_against(&start));
        form.kind = ScheduleKind::Hourly;
        assert!(form.is_dirty_against(&start), "switching the schedule tab counts, as in the real app");
        form.kind = start.kind;
        form.name = "x".into();
        assert!(form.is_dirty_against(&start));
    }

    #[test]
    fn history_rows_are_newest_first_with_the_session_state_or_the_skip_reason() {
        let mut runs = vec![run("a", 1_000), run("a", 3_000), run("b", 2_000)];
        runs.push(RoutineRun { outcome: RunOutcome::Skipped(SkipReason::Missed), session_id: None, ..run("a", 2_000) });
        runs.push(RoutineRun { outcome: RunOutcome::Skipped(SkipReason::StartFailed("no node".to_string())), session_id: None, ..run("a", 500) });
        let rows = history_rows(&runs, "a", &|_| "Done");
        let texts: Vec<&str> = rows.iter().map(|row| row.text.as_str()).collect();
        assert_eq!(texts, vec!["Done", "Skipped: Forge was closed or the computer was asleep", "Done", "Skipped: could not start (no node)"]);
        assert_eq!(rows[0].session_id.as_deref(), Some("s"));
        assert_eq!(rows[1].session_id, None);
    }

    #[test]
    fn the_skip_reasons_read_in_words() {
        assert_eq!(skip_text(&SkipReason::Busy), "Skipped: a session of this routine's folder was still working");
        assert_eq!(skip_text(&SkipReason::FolderMissing), "Skipped: the folder no longer exists");
    }

    #[test]
    fn a_started_run_reads_running_waiting_or_done_from_its_session() {
        assert_eq!(run_state_text(Some((Phase::Running, 0))), "Running");
        assert_eq!(run_state_text(Some((Phase::Running, 1))), "Waiting for approval");
        assert_eq!(run_state_text(Some((Phase::Idle, 1))), "Waiting for approval");
        assert_eq!(run_state_text(Some((Phase::Idle, 0))), "Done");
        assert_eq!(run_state_text(Some((Phase::Failed, 0))), "Done");
        assert_eq!(run_state_text(None), "Done", "a session that is not open any more is done");
    }

    #[test]
    fn the_waiting_notice_names_the_routine_and_the_tool() {
        assert_eq!(
            waiting_notice("Daily review", "Bash"),
            ("Routine needs your approval".to_string(), "Daily review wants to use Bash".to_string())
        );
    }

    #[test]
    fn the_offset_follows_the_date_so_a_summer_and_a_winter_time_are_both_right() {
        use chrono_tz::America::New_York;
        let winter = time::macros::datetime!(2026-01-15 17:00:00 UTC).unix_timestamp() * 1000;
        let summer = time::macros::datetime!(2026-07-15 17:00:00 UTC).unix_timestamp() * 1000;
        assert_eq!(offset_at(&New_York, winter), time::UtcOffset::from_hms(-5, 0, 0).unwrap());
        assert_eq!(offset_at(&New_York, summer), time::UtcOffset::from_hms(-4, 0, 0).unwrap());
        assert_eq!(when_label(winter, offset_at(&New_York, winter)), "Jan 15, 12:00 PM");
        assert_eq!(when_label(summer, offset_at(&New_York, summer)), "Jul 15, 1:00 PM");
        let india = chrono::FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
        assert_eq!(offset_at(&india, winter), time::UtcOffset::from_hms(5, 30, 0).unwrap());
    }

    #[test]
    fn a_time_reads_like_oct_9_10_19_am_in_the_given_offset() {
        let nine = time::UtcOffset::from_hms(9, 0, 0).unwrap();
        // 2026-10-09 01:19:00 UTC is 10:19 in +09:00.
        let ms = time::macros::datetime!(2026-10-09 01:19:00 UTC).unix_timestamp() * 1000;
        assert_eq!(when_label(ms, nine), "Oct 9, 10:19 AM");
        let ms = time::macros::datetime!(2026-10-09 12:05:00 UTC).unix_timestamp() * 1000;
        assert_eq!(when_label(ms, time::UtcOffset::UTC), "Oct 9, 12:05 PM");
        let ms = time::macros::datetime!(2026-10-09 00:00:00 UTC).unix_timestamp() * 1000;
        assert_eq!(when_label(ms, time::UtcOffset::UTC), "Oct 9, 12:00 AM");
    }
}
