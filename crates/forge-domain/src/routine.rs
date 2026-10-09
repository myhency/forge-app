use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::PermissionMode;

/// A run more than this far past its due time is not run (Forge was closed or the computer asleep).
pub const MISSED_AFTER_MS: i64 = 30 * 60 * 1000;
/// The random delay the real app mentions, fixed per routine: 0 up to this.
pub const MAX_DELAY_MS: i64 = 5 * 60 * 1000;
/// Schedules with two occurrences closer than this get no delay (it would reorder them).
const MIN_GAP_FOR_DELAY_MS: i64 = 10 * 60 * 1000;
/// History kept per routine.
pub const KEEP_RUNS: usize = 50;
/// How many occurrences `decide` walks before it stops looking for the latest one.
const SCAN_LIMIT: usize = 2000;

const DAY_NAMES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const DAY_WORDS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const FIVE_FIELDS: &str = "A cron expression has five fields: minute hour day month weekday";

/// When a routine runs. These are the six tabs of the form. `day` is 0 (Sunday) to 6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Schedule {
    Manual,
    Hourly,
    Daily { hour: u8, minute: u8 },
    Weekdays { hour: u8, minute: u8 },
    Weekly { day: u8, hour: u8, minute: u8 },
    Cron { expression: String },
}

impl Schedule {
    /// The expression the `cron` crate runs: seconds first, weekdays by name (the crate counts them
    /// from another starting number than the usual `0` or `7` for Sunday).
    fn engine_text(&self) -> Result<Option<String>, String> {
        let text = match self {
            Schedule::Manual => return Ok(None),
            Schedule::Hourly => "0 0 * * * *".to_string(),
            Schedule::Daily { hour, minute } => format!("0 {minute} {hour} * * *"),
            Schedule::Weekdays { hour, minute } => format!("0 {minute} {hour} * * MON-FRI"),
            Schedule::Weekly { day, hour, minute } => {
                let name = DAY_NAMES.get(usize::from(*day)).ok_or("The day must be 0 to 6")?;
                format!("0 {minute} {hour} * * {name}")
            }
            Schedule::Cron { expression } => engine_expression(expression)?,
        };
        Ok(Some(text))
    }

    fn engine(&self) -> Result<Option<cron::Schedule>, String> {
        match self.engine_text()? {
            None => Ok(None),
            Some(text) => cron::Schedule::from_str(&text).map(Some).map_err(|error| error.to_string()),
        }
    }

    /// Why this schedule cannot be used, in words for the form.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Schedule::Daily { hour, minute } | Schedule::Weekdays { hour, minute } | Schedule::Weekly { hour, minute, .. }
                if *hour > 23 || *minute > 59 =>
            {
                return Err("The time must be 00:00 to 23:59".to_string());
            }
            _ => {}
        }
        self.engine().map(|_| ())
    }

    /// The first time strictly after `after`; `None` for `Manual` and for a schedule that cannot be read.
    pub fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        let engine = self.engine().ok()??;
        engine.after(after).next()
    }

    /// How a card and the detail screen say it: "Every day at 9:00 AM".
    pub fn label(&self) -> String {
        match self {
            Schedule::Manual => "Manual".to_string(),
            Schedule::Hourly => "Every hour".to_string(),
            Schedule::Daily { hour, minute } => format!("Every day at {}", clock(*hour, *minute)),
            Schedule::Weekdays { hour, minute } => format!("Weekdays at {}", clock(*hour, *minute)),
            Schedule::Weekly { day, hour, minute } => {
                format!("Every {} at {}", DAY_WORDS.get(usize::from(*day)).copied().unwrap_or("day"), clock(*hour, *minute))
            }
            Schedule::Cron { expression } => format!("Cron {expression}"),
        }
    }
}

fn clock(hour: u8, minute: u8) -> String {
    let (twelve, suffix) = match hour {
        0 => (12, "AM"),
        1..=11 => (hour, "AM"),
        12 => (12, "PM"),
        _ => (hour - 12, "PM"),
    };
    format!("{twelve}:{minute:02} {suffix}")
}

/// Five usual fields in, the six the engine wants out. A bad count is refused here, in words.
fn engine_expression(expression: &str) -> Result<String, String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(FIVE_FIELDS.to_string());
    }
    Ok(format!("0 {} {} {} {} {}", fields[0], fields[1], fields[2], fields[3], weekday_names(fields[4])))
}

/// `1-5`, `0,6` and `*/2` in the usual numbers become names; `*` and names stay.
fn weekday_names(field: &str) -> String {
    field
        .split(',')
        .map(|part| {
            let (range, step) = match part.split_once('/') {
                Some((range, step)) => (range, Some(step)),
                None => (part, None),
            };
            let range = range.split('-').map(day_name).collect::<Vec<_>>().join("-");
            match step {
                Some(step) => format!("{range}/{step}"),
                None => range,
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

fn day_name(token: &str) -> String {
    match token {
        "0" | "7" => "SUN",
        "1" => "MON",
        "2" => "TUE",
        "3" => "WED",
        "4" => "THU",
        "5" => "FRI",
        "6" => "SAT",
        other => other,
    }
    .to_string()
}

/// A saved routine. Times are epoch milliseconds. `baseline` is where the next due time is counted
/// from: set when the routine is made, switched on or edited, and moved after each run or skip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Routine {
    pub id: String,
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub folder: PathBuf,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
    pub schedule: Schedule,
    pub enabled: bool,
    pub created_at: i64,
    pub baseline: i64,
    /// Permission rules the person answered "always allow" to in this routine's sessions.
    #[serde(default)]
    pub always_allowed: Vec<String>,
}

/// What the form holds when it is saved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutineDraft {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub folder: Option<PathBuf>,
    pub permission_mode: PermissionMode,
    pub model: Option<String>,
    pub schedule: Schedule,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Forge was closed or the computer asleep when it was due.
    Missed,
    /// A session of a routine was still working in the folder.
    Busy,
    FolderMissing,
    StartFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunOutcome {
    Started,
    Skipped(SkipReason),
}

/// One line of a routine's history. Whether a started run is still working, waiting or done is not
/// stored: it is read from the session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutineRun {
    pub routine_id: String,
    pub due_ms: i64,
    pub started_ms: i64,
    pub outcome: RunOutcome,
    /// The Claude session id, known a moment after the run started.
    pub session_id: Option<String>,
}

/// What a tick should do about one routine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Wait,
    Run { due_ms: i64 },
    Missed { due_ms: i64 },
}

/// A fixed offset of 0 to `MAX_DELAY_MS`, from a hash of the id (FNV-1a), so the same routine is
/// always late by the same amount.
pub fn delay_ms(id: &str) -> i64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    (hash % MAX_DELAY_MS as u64) as i64
}

fn delay_for<Tz: TimeZone>(schedule: &Schedule, id: &str, first: &DateTime<Tz>) -> i64 {
    match schedule.next_after(first) {
        Some(second) if second.timestamp_millis() - first.timestamp_millis() >= MIN_GAP_FOR_DELAY_MS => delay_ms(id),
        Some(_) => 0,
        None => delay_ms(id),
    }
}

/// What to do about `routine` at `now_ms`. Of all the times that passed since the baseline only the
/// latest counts: it runs when it is at most `MISSED_AFTER_MS` late and is recorded as one skip when
/// it is later.
pub fn decide<Tz: TimeZone>(routine: &Routine, now_ms: i64, zone: &Tz) -> Decision {
    if !routine.enabled || matches!(routine.schedule, Schedule::Manual) {
        return Decision::Wait;
    }
    let Some(start) = zone.timestamp_millis_opt(routine.baseline).single() else {
        return Decision::Wait;
    };
    let Some(first) = routine.schedule.next_after(&start) else {
        return Decision::Wait;
    };
    let delay = delay_for(&routine.schedule, &routine.id, &first);
    let mut latest = None;
    let mut current = first;
    for _ in 0..SCAN_LIMIT {
        let due = current.timestamp_millis() + delay;
        if due > now_ms {
            break;
        }
        latest = Some(due);
        match routine.schedule.next_after(&current) {
            Some(next) => current = next,
            None => break,
        }
    }
    match latest {
        None => Decision::Wait,
        Some(due_ms) if now_ms - due_ms <= MISSED_AFTER_MS => Decision::Run { due_ms },
        Some(due_ms) => Decision::Missed { due_ms },
    }
}

/// A slug of `name` that no id in `taken` has: `a`, then `a-2`, `a-3`. Letters of any language stay.
pub fn routine_id(name: &str, taken: &[String]) -> String {
    let mut slug = String::new();
    let mut dash = false;
    for character in name.trim().chars() {
        if character.is_alphanumeric() {
            if dash && !slug.is_empty() {
                slug.push('-');
            }
            dash = false;
            slug.extend(character.to_lowercase());
        } else {
            dash = true;
        }
    }
    if slug.is_empty() {
        slug = "routine".to_string();
    }
    let mut candidate = slug.clone();
    let mut number = 2;
    while taken.iter().any(|id| *id == candidate) {
        candidate = format!("{slug}-{number}");
        number += 1;
    }
    candidate
}

/// Why a draft cannot be saved, in words for the form (the first problem only).
pub fn validate_draft(draft: &RoutineDraft) -> Result<(), String> {
    if draft.name.trim().is_empty() {
        return Err("Name is required".to_string());
    }
    if draft.description.trim().is_empty() {
        return Err("Description is required".to_string());
    }
    if draft.folder.is_none() {
        return Err("Choose a folder".to_string());
    }
    draft.schedule.validate()
}

/// Adds a run and drops the oldest of the same routine beyond `KEEP_RUNS`.
pub fn push_run(runs: &mut Vec<RoutineRun>, run: RoutineRun) {
    let routine = run.routine_id.clone();
    runs.push(run);
    let mut extra = runs.iter().filter(|run| run.routine_id == routine).count().saturating_sub(KEEP_RUNS);
    runs.retain(|run| {
        if extra > 0 && run.routine_id == routine {
            extra -= 1;
            false
        } else {
            true
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{FixedOffset, TimeZone};

    fn zone() -> FixedOffset {
        FixedOffset::east_opt(9 * 3600).unwrap()
    }

    /// Epoch milliseconds of 2026-10-<day> <hour>:<minute> in the test zone.
    fn at(day: u32, hour: u32, minute: u32) -> i64 {
        zone().with_ymd_and_hms(2026, 10, day, hour, minute, 0).unwrap().timestamp_millis()
    }

    fn routine(schedule: Schedule, baseline: i64) -> Routine {
        Routine {
            id: "daily-review".to_string(),
            name: "Daily review".to_string(),
            description: "d".to_string(),
            instructions: String::new(),
            folder: "/work".into(),
            permission_mode: PermissionMode::Default,
            model: None,
            schedule,
            enabled: true,
            created_at: baseline,
            baseline,
            always_allowed: Vec::new(),
        }
    }

    fn daily_nine() -> Schedule {
        Schedule::Daily { hour: 9, minute: 0 }
    }

    #[test]
    fn every_tab_has_a_next_time() {
        let from = zone().with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap(); // a Friday
        let next = |schedule: Schedule| schedule.next_after(&from).map(|time| time.timestamp_millis());
        assert_eq!(next(Schedule::Hourly), Some(at(9, 11, 0)));
        assert_eq!(next(daily_nine()), Some(at(10, 9, 0)));
        // Friday 10:00: the next weekday 9:00 is Monday the 12th.
        assert_eq!(next(Schedule::Weekdays { hour: 9, minute: 0 }), Some(at(12, 9, 0)));
        // 0 is Sunday: the 11th.
        assert_eq!(next(Schedule::Weekly { day: 0, hour: 8, minute: 30 }), Some(at(11, 8, 30)));
        assert_eq!(next(Schedule::Manual), None);
    }

    #[test]
    fn a_cron_expression_uses_the_usual_weekday_numbers() {
        let from = zone().with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap(); // Friday
        let cron = |text: &str| Schedule::Cron { expression: text.to_string() }.next_after(&from).map(|time| time.timestamp_millis());
        assert_eq!(cron("0 9 * * 1-5"), Some(at(12, 9, 0)), "1-5 is Monday to Friday");
        assert_eq!(cron("0 9 * * 0"), Some(at(11, 9, 0)), "0 is Sunday");
        assert_eq!(cron("0 9 * * 7"), Some(at(11, 9, 0)), "7 is Sunday too");
        assert_eq!(cron("30 9 * * 5"), Some(at(16, 9, 30)), "5 is Friday: 9:30 has passed today, so next week, the 16th");
        assert_eq!(cron("*/15 * * * *"), Some(at(9, 10, 15)));
    }

    #[test]
    fn a_bad_cron_expression_says_why_and_is_never_due() {
        for text in ["", "* * * *", "* * * * * *", "61 * * * *", "*/0 * * * *", "nonsense here we go"] {
            let schedule = Schedule::Cron { expression: text.to_string() };
            assert!(schedule.validate().is_err(), "{text:?} should be refused");
            let from = zone().with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap();
            assert_eq!(schedule.next_after(&from), None, "{text:?}");
        }
        assert_eq!(
            Schedule::Cron { expression: "* * * *".to_string() }.validate().unwrap_err(),
            "A cron expression has five fields: minute hour day month weekday"
        );
        assert!(Schedule::Daily { hour: 24, minute: 0 }.validate().is_err());
        assert!(Schedule::Daily { hour: 9, minute: 60 }.validate().is_err());
        assert!(Schedule::Weekly { day: 7, hour: 9, minute: 0 }.validate().is_err());
    }

    #[test]
    fn labels_read_like_the_card_text() {
        assert_eq!(Schedule::Manual.label(), "Manual");
        assert_eq!(Schedule::Hourly.label(), "Every hour");
        assert_eq!(daily_nine().label(), "Every day at 9:00 AM");
        assert_eq!(Schedule::Daily { hour: 21, minute: 5 }.label(), "Every day at 9:05 PM");
        assert_eq!(Schedule::Daily { hour: 0, minute: 0 }.label(), "Every day at 12:00 AM");
        assert_eq!(Schedule::Weekdays { hour: 9, minute: 49 }.label(), "Weekdays at 9:49 AM");
        assert_eq!(Schedule::Weekly { day: 1, hour: 12, minute: 0 }.label(), "Every Monday at 12:00 PM");
        assert_eq!(Schedule::Cron { expression: "0 9 * * *".to_string() }.label(), "Cron 0 9 * * *");
    }

    #[test]
    fn the_delay_is_fixed_per_routine_and_under_five_minutes() {
        assert_eq!(delay_ms("daily-review"), delay_ms("daily-review"));
        for id in ["a", "daily-review", "아침-점검", ""] {
            assert!((0..MAX_DELAY_MS).contains(&delay_ms(id)), "{id:?}");
        }
        assert_ne!(delay_ms("a"), delay_ms("b"));
    }

    #[test]
    fn nothing_is_due_before_the_time() {
        let routine = routine(daily_nine(), at(9, 10, 0));
        let delay = delay_ms(&routine.id);
        assert_eq!(decide(&routine, at(10, 9, 0) + delay - 1, &zone()), Decision::Wait);
    }

    #[test]
    fn a_routine_runs_when_due_and_up_to_thirty_minutes_late() {
        let routine = routine(daily_nine(), at(9, 10, 0));
        let due = at(10, 9, 0) + delay_ms(&routine.id);
        assert_eq!(decide(&routine, due, &zone()), Decision::Run { due_ms: due });
        assert_eq!(decide(&routine, due + MISSED_AFTER_MS, &zone()), Decision::Run { due_ms: due });
        assert_eq!(decide(&routine, due + MISSED_AFTER_MS + 1, &zone()), Decision::Missed { due_ms: due });
    }

    #[test]
    fn many_missed_hours_give_one_decision_for_the_latest() {
        // Hourly, last counted at 06:00; the app wakes at 10:10. The 10:00 one is 10 minutes late (plus
        // the delay) and runs; the earlier ones are not run and not listed.
        let routine = routine(Schedule::Hourly, at(9, 6, 0));
        let due = at(9, 10, 0) + delay_ms(&routine.id);
        assert_eq!(decide(&routine, at(9, 10, 10), &zone()), Decision::Run { due_ms: due });
        // Woken at 12:40: the 12:00 one is 40 minutes late, so it is one Missed, not five.
        let due = at(9, 12, 0) + delay_ms(&routine.id);
        assert_eq!(decide(&routine, at(9, 12, 40) + delay_ms(&routine.id), &zone()), Decision::Missed { due_ms: due });
    }

    #[test]
    fn a_once_a_minute_schedule_after_days_closed_is_still_one_decision() {
        let routine = routine(Schedule::Cron { expression: "* * * * *".to_string() }, at(1, 0, 0));
        match decide(&routine, at(9, 12, 0), &zone()) {
            Decision::Missed { .. } => {}
            other => panic!("expected one Missed, got {other:?}"),
        }
    }

    #[test]
    fn a_schedule_closer_than_ten_minutes_gets_no_delay() {
        let routine = routine(Schedule::Cron { expression: "* * * * *".to_string() }, at(9, 10, 0));
        assert_eq!(decide(&routine, at(9, 10, 1), &zone()), Decision::Run { due_ms: at(9, 10, 1) });
    }

    #[test]
    fn manual_disabled_and_broken_routines_wait() {
        assert_eq!(decide(&routine(Schedule::Manual, 0), at(9, 12, 0), &zone()), Decision::Wait);
        let mut off = routine(Schedule::Hourly, at(9, 6, 0));
        off.enabled = false;
        assert_eq!(decide(&off, at(9, 12, 0), &zone()), Decision::Wait);
        let broken = routine(Schedule::Cron { expression: "*/0 * * * *".to_string() }, at(9, 6, 0));
        assert_eq!(decide(&broken, at(9, 12, 0), &zone()), Decision::Wait);
    }

    #[test]
    fn a_baseline_in_the_future_waits() {
        // The clock was set back: nothing runs until the time catches up.
        let routine = routine(Schedule::Hourly, at(9, 15, 0));
        assert_eq!(decide(&routine, at(9, 10, 0), &zone()), Decision::Wait);
    }

    #[test]
    fn ids_come_from_the_name_and_never_clash() {
        assert_eq!(routine_id("Daily code review", &[]), "daily-code-review");
        assert_eq!(routine_id("  Hello,   World!! ", &[]), "hello-world");
        assert_eq!(routine_id("아침 점검", &[]), "아침-점검");
        assert_eq!(routine_id("!!!", &[]), "routine");
        let taken = vec!["a".to_string(), "a-2".to_string()];
        assert_eq!(routine_id("a", &taken), "a-3");
    }

    #[test]
    fn a_draft_needs_a_name_a_description_a_folder_and_a_good_schedule() {
        let good = RoutineDraft {
            name: "n".into(),
            description: "d".into(),
            instructions: String::new(),
            folder: Some("/work".into()),
            permission_mode: PermissionMode::Default,
            model: None,
            schedule: Schedule::Manual,
        };
        assert_eq!(validate_draft(&good), Ok(()));
        assert_eq!(validate_draft(&RoutineDraft { name: "  ".into(), ..good.clone() }), Err("Name is required".to_string()));
        assert_eq!(validate_draft(&RoutineDraft { description: String::new(), ..good.clone() }), Err("Description is required".to_string()));
        assert_eq!(validate_draft(&RoutineDraft { folder: None, ..good.clone() }), Err("Choose a folder".to_string()));
        assert!(validate_draft(&RoutineDraft { schedule: Schedule::Cron { expression: "x".into() }, ..good }).is_err());
    }

    fn run(routine: &str, due: i64) -> RoutineRun {
        RoutineRun { routine_id: routine.to_string(), due_ms: due, started_ms: due, outcome: RunOutcome::Started, session_id: None }
    }

    #[test]
    fn history_keeps_the_latest_fifty_per_routine() {
        let mut runs = Vec::new();
        for due in 0..60 {
            push_run(&mut runs, run("a", due));
        }
        push_run(&mut runs, run("b", 1));
        let a: Vec<i64> = runs.iter().filter(|run| run.routine_id == "a").map(|run| run.due_ms).collect();
        assert_eq!(a.len(), KEEP_RUNS);
        assert_eq!(a.first(), Some(&10), "the oldest ten are gone");
        assert_eq!(a.last(), Some(&59));
        assert_eq!(runs.iter().filter(|run| run.routine_id == "b").count(), 1);
    }

    #[test]
    fn a_routine_survives_a_round_trip_and_an_unknown_schedule_field_is_not_needed() {
        let original = routine(Schedule::Weekly { day: 2, hour: 7, minute: 5 }, 1);
        let json = serde_json::to_string(&original).unwrap();
        assert_eq!(serde_json::from_str::<Routine>(&json).unwrap(), original);
        let manual: Schedule = serde_json::from_str(r#"{"kind":"manual"}"#).unwrap();
        assert_eq!(manual, Schedule::Manual);
    }
}
