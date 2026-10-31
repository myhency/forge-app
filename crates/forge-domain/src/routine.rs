use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, LocalResult, NaiveDateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

use crate::PermissionMode;
use crate::tr;

/// A run more than this far past its due time is not run (Forge was closed or the computer asleep).
pub const MISSED_AFTER_MS: i64 = 30 * 60 * 1000;
/// The random delay the real app mentions, fixed per routine: 0 up to this.
pub const MAX_DELAY_MS: i64 = 5 * 60 * 1000;
/// Schedules with two occurrences closer than this get no delay (it would reorder them).
const MIN_GAP_FOR_DELAY_MS: i64 = 10 * 60 * 1000;
/// History kept per routine.
pub const KEEP_RUNS: usize = 50;
/// How many occurrences `decide` walks inside one look-back window.
const SCAN_LIMIT: usize = 2000;
/// How far back from "now" `decide` looks for the latest occurrence, narrowest first (milliseconds).
const LOOKBACKS_MS: [i64; 5] = [3_600_000, 86_400_000, 8 * 86_400_000, 40 * 86_400_000, 400 * 86_400_000];

const DAY_NAMES: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
const MONTH_NAMES: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

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
                let name = DAY_NAMES.get(usize::from(*day)).ok_or(tr!("The day must be 0 to 6"))?;
                format!("0 {minute} {hour} * * {name}")
            }
            Schedule::Cron { expression } => engine_expression(expression)?,
        };
        Ok(Some(text))
    }

    fn engine(&self) -> Result<Option<cron::Schedule>, String> {
        match self.engine_text()? {
            None => Ok(None),
            Some(text) => cron::Schedule::from_str(&text).map(Some).map_err(|_| tr!("That cron expression cannot be scheduled").to_string()),
        }
    }

    /// The schedule ready for repeated use (parsed once); `None` for `Manual` and for one that cannot be read.
    fn prepare(&self) -> Option<Engine> {
        self.engine().ok()?.map(|schedule| Engine { schedule })
    }

    /// Why this schedule cannot be used, in words for the form.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Schedule::Daily { hour, minute } | Schedule::Weekdays { hour, minute } | Schedule::Weekly { hour, minute, .. }
                if *hour > 23 || *minute > 59 =>
            {
                return Err(tr!("The time must be 00:00 to 23:59").to_string());
            }
            _ => {}
        }
        match self.engine()? {
            Some(schedule) if schedule.upcoming(Utc).next().is_none() => Err(tr!("That expression never runs (check the day and the month)").to_string()),
            _ => Ok(()),
        }
    }

    /// The first time strictly after `after`; `None` for `Manual` and for a schedule that cannot be read.
    pub fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        self.prepare()?.next_after(after)
    }

    /// How a card and the detail screen say it: "Every day at 9:00 AM".
    pub fn label(&self) -> String {
        match self {
            Schedule::Manual => tr!("Manual").to_string(),
            Schedule::Hourly => tr!("Every hour").to_string(),
            Schedule::Daily { hour, minute } => tr!("Every day at {time}", time = clock(*hour, *minute)),
            Schedule::Weekdays { hour, minute } => tr!("Weekdays at {time}", time = clock(*hour, *minute)),
            Schedule::Weekly { day, hour, minute } => {
                tr!("Every {day} at {time}", day = day_word(*day), time = clock(*hour, *minute))
            }
            Schedule::Cron { expression } => tr!("Cron {expression}", expression = expression),
        }
    }
}

/// "9:05 AM" (or "오전 9:05"): the time as the cards say it.
pub fn clock(hour: u8, minute: u8) -> String {
    let (twelve, pm) = match hour {
        0 => (12, false),
        1..=11 => (hour, false),
        12 => (12, true),
        _ => (hour - 12, true),
    };
    let time = format!("{twelve}:{minute:02}");
    if pm { tr!("{time} PM", time = time) } else { tr!("{time} AM", time = time) }
}

/// The weekday's name in the language on screen; 0 is Sunday.
fn day_word(day: u8) -> &'static str {
    match day {
        0 => tr!("Sunday"),
        1 => tr!("Monday"),
        2 => tr!("Tuesday"),
        3 => tr!("Wednesday"),
        4 => tr!("Thursday"),
        5 => tr!("Friday"),
        6 => tr!("Saturday"),
        _ => tr!("day"),
    }
}

/// A parsed schedule. It works on calendar (wall clock) times carried as UTC and turns each result into a
/// real instant in the person's zone: so a time that does not exist (the hour skipped when the clocks go
/// forward) runs when the gap ends, and a repeated hour (clocks back) runs once.
struct Engine {
    schedule: cron::Schedule,
}

impl Engine {
    fn next_after<Tz: TimeZone>(&self, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
        let zone = after.timezone();
        let mut from = Utc.from_utc_datetime(&after.naive_local());
        for _ in 0..8 {
            let nominal = self.schedule.after(&from).next()?;
            if let Some(instant) = place(&zone, nominal.naive_utc(), after) {
                return Some(instant);
            }
            from = nominal;
        }
        None
    }
}

/// The instant of calendar time `wall` in `zone`, when it is later than `after`.
fn place<Tz: TimeZone>(zone: &Tz, wall: NaiveDateTime, after: &DateTime<Tz>) -> Option<DateTime<Tz>> {
    match zone.from_local_datetime(&wall) {
        LocalResult::Single(instant) => (instant > *after).then_some(instant),
        LocalResult::Ambiguous(first, second) => [first, second].into_iter().find(|instant| instant > after),
        LocalResult::None => {
            // Inside the gap: the first quarter hour that exists.
            let mut probe = wall;
            for _ in 0..16 {
                probe += chrono::Duration::minutes(15);
                match zone.from_local_datetime(&probe) {
                    LocalResult::Single(instant) => return (instant > *after).then_some(instant),
                    LocalResult::Ambiguous(first, _) => return (first > *after).then_some(first),
                    LocalResult::None => {}
                }
            }
            None
        }
    }
}

/// Five usual fields in, the six the engine wants out. A field that cannot work is named in words.
fn engine_expression(expression: &str) -> Result<String, String> {
    let fields: Vec<&str> = expression.split_whitespace().collect();
    if fields.len() != 5 {
        return Err(tr!("A cron expression has five fields: minute hour day month weekday").to_string());
    }
    check_field(tr!("Minute"), fields[0], 0, 59, &[])?;
    check_field(tr!("Hour"), fields[1], 0, 23, &[])?;
    check_field(tr!("Day of the month"), fields[2], 1, 31, &[])?;
    check_field(tr!("Month"), fields[3], 1, 12, &MONTH_NAMES)?;
    let weekdays = weekday_names(fields[4])?;
    // Standard cron runs on either; the engine wants both. Asking for both is refused rather than misread.
    if fields[2] != "*" && weekdays != "*" {
        return Err(tr!("Use either a day of the month or a weekday, not both").to_string());
    }
    Ok(format!("0 {} {} {} {} {}", fields[0], fields[1], fields[2], fields[3], weekdays))
}

/// Checks one field of a cron expression: lists, ranges and steps of numbers (or `names`) inside `min..=max`.
fn check_field(label: &str, text: &str, min: u32, max: u32, names: &[&str]) -> Result<(), String> {
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, Some(step)),
            None => (part, None),
        };
        if let Some(step) = step {
            if !matches!(step.parse::<u32>(), Ok(count) if count >= 1) {
                return Err(tr!("{label}: the step must be a whole number of at least 1", label = label));
            }
        }
        if range == "*" {
            continue;
        }
        let tokens: Vec<&str> = range.split('-').collect();
        if tokens.len() > 2 {
            return Err(tr!("{label} has a value that is not understood", label = label));
        }
        let values = tokens.iter().map(|token| field_value(label, token, min, max, names)).collect::<Result<Vec<u32>, String>>()?;
        if values.len() == 2 && values[0] > values[1] {
            return Err(tr!("{label}: the range runs backwards", label = label));
        }
    }
    Ok(())
}

fn field_value(label: &str, token: &str, min: u32, max: u32, names: &[&str]) -> Result<u32, String> {
    if let Ok(number) = token.parse::<u32>() {
        return if (min..=max).contains(&number) { Ok(number) } else { Err(tr!("{label} must be {min} to {max}", label = label, min = min, max = max)) };
    }
    match names.iter().position(|name| name.eq_ignore_ascii_case(token)) {
        Some(index) => Ok(min + index as u32),
        None => Err(tr!("{label} has a value that is not understood", label = label)),
    }
}

/// The weekday field for the engine, which counts weekdays from another start than the usual `0` or `7`
/// for Sunday: numbers become names, and a range or a step is written out (`1-7` is every day, `5-7` is
/// Friday to Sunday, `1/2` is Monday, Wednesday, Friday: a start with a step runs to Saturday). `*` and names
/// stay as they are.
fn weekday_names(field: &str) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();
    for part in field.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => (range, Some(step)),
            None => (part, None),
        };
        let every: usize = match step {
            Some(text) => text.parse::<usize>().ok().filter(|count| *count >= 1).ok_or(tr!("Weekday: the step must be a whole number of at least 1"))?,
            None => 1,
        };
        if range == "*" {
            parts.push(match step {
                Some(text) => format!("*/{text}"),
                None => "*".to_string(),
            });
            continue;
        }
        let tokens: Vec<&str> = range.split('-').collect();
        let numbers: Option<Vec<usize>> = tokens.iter().map(|token| token.parse::<usize>().ok()).collect();
        match numbers {
            Some(numbers) if numbers.len() <= 2 => {
                if numbers.iter().any(|number| *number > 7) {
                    return Err(tr!("Weekday must be 0 to 7 (or SUN to SAT)").to_string());
                }
                let low = numbers[0];
                let high = match numbers.get(1) {
                    Some(high) => *high,
                    None if step.is_some() => 6.max(low),
                    None => low,
                };
                if low > high {
                    return Err(tr!("Weekday: the range runs backwards").to_string());
                }
                let mut days: Vec<&str> = Vec::new();
                for day in (low..=high).step_by(every) {
                    let name = DAY_NAMES[day % 7];
                    if !days.contains(&name) {
                        days.push(name);
                    }
                }
                parts.push(days.join(","));
            }
            Some(_) => return Err(tr!("Weekday has a value that is not understood").to_string()),
            None => {
                // Names: SUN-SAT, optionally as a range.
                if !tokens.iter().all(|token| DAY_NAMES.iter().any(|name| name.eq_ignore_ascii_case(token))) {
                    return Err(tr!("Weekday must be 0 to 7 (or SUN to SAT)").to_string());
                }
                parts.push(match step {
                    Some(text) => format!("{}/{text}", range.to_uppercase()),
                    None => range.to_uppercase(),
                });
            }
        }
    }
    Ok(parts.join(","))
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
    /// When the routine this line belongs to was created, so a late line of a deleted routine is not taken
    /// for one of a new routine with the same id. `0` for a line written before this was kept.
    #[serde(default)]
    pub routine_created: i64,
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

fn delay_for<Tz: TimeZone>(engine: &Engine, id: &str, first: &DateTime<Tz>) -> i64 {
    match engine.next_after(first) {
        Some(second) if second.timestamp_millis() - first.timestamp_millis() >= MIN_GAP_FOR_DELAY_MS => delay_ms(id),
        Some(_) => 0,
        None => delay_ms(id),
    }
}

/// The latest occurrence at or before `limit_ms`, given that `first` (the first one after the baseline) is
/// known to be due. It looks back from `limit_ms` in growing steps, so a long closed time costs a few steps,
/// not one per missed occurrence.
fn latest_occurrence<Tz: TimeZone>(engine: &Engine, zone: &Tz, first: &DateTime<Tz>, limit_ms: i64) -> i64 {
    let first_ms = first.timestamp_millis();
    for lookback in LOOKBACKS_MS {
        let start_ms = first_ms.max(limit_ms - lookback);
        let Some(mut cursor) = zone.timestamp_millis_opt(start_ms).single() else {
            continue;
        };
        let mut latest = None;
        for _ in 0..SCAN_LIMIT {
            let Some(next) = engine.next_after(&cursor) else { break };
            if next.timestamp_millis() > limit_ms {
                break;
            }
            latest = Some(next.timestamp_millis());
            cursor = next;
        }
        if let Some(latest) = latest {
            return latest;
        }
        if start_ms == first_ms {
            break;
        }
    }
    first_ms
}

/// What to do about `routine` at `now_ms`. Of all the times that passed since the baseline only the
/// latest counts: it runs when it is at most `MISSED_AFTER_MS` late and is recorded as one skip when
/// it is later.
pub fn decide<Tz: TimeZone>(routine: &Routine, now_ms: i64, zone: &Tz) -> Decision {
    if !routine.enabled || matches!(routine.schedule, Schedule::Manual) {
        return Decision::Wait;
    }
    let Some(engine) = routine.schedule.prepare() else {
        return Decision::Wait;
    };
    let Some(start) = zone.timestamp_millis_opt(routine.baseline).single() else {
        return Decision::Wait;
    };
    let Some(first) = engine.next_after(&start) else {
        return Decision::Wait;
    };
    let delay = delay_for(&engine, &routine.id, &first);
    let limit_ms = now_ms - delay;
    if first.timestamp_millis() > limit_ms {
        return Decision::Wait;
    }
    let due_ms = latest_occurrence(&engine, zone, &first, limit_ms) + delay;
    if now_ms - due_ms <= MISSED_AFTER_MS {
        Decision::Run { due_ms }
    } else {
        Decision::Missed { due_ms }
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
        return Err(tr!("Name is required").to_string());
    }
    if draft.description.trim().is_empty() {
        return Err(tr!("Description is required").to_string());
    }
    if draft.folder.is_none() {
        return Err(tr!("Choose a folder").to_string());
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
    fn a_once_a_minute_schedule_after_days_closed_is_one_decision_for_the_latest_minute() {
        let routine = routine(Schedule::Cron { expression: "* * * * *".to_string() }, at(1, 0, 0));
        assert_eq!(decide(&routine, at(9, 12, 0), &zone()), Decision::Run { due_ms: at(9, 12, 0) });
    }

    #[test]
    fn days_closed_under_a_daily_schedule_is_one_missed_line_for_the_latest_day() {
        let routine = routine(daily_nine(), at(1, 10, 0));
        let due = at(9, 9, 0) + delay_ms(&routine.id);
        assert_eq!(decide(&routine, at(9, 12, 0), &zone()), Decision::Missed { due_ms: due });
    }

    #[test]
    fn a_long_gap_finds_the_latest_occurrence_not_the_first_after_the_baseline() {
        let routine = routine(Schedule::Weekly { day: 1, hour: 9, minute: 0 }, at(1, 0, 0) - 40 * 86_400_000);
        match decide(&routine, at(9, 12, 0), &zone()) {
            Decision::Missed { due_ms } => assert!(due_ms > at(1, 0, 0), "the latest Monday, not the first one after the baseline: {due_ms}"),
            other => panic!("{other:?}"),
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
    fn standard_weekday_ranges_and_steps_are_accepted() {
        let from = zone().with_ymd_and_hms(2026, 10, 9, 10, 0, 0).unwrap(); // Friday
        let next = |text: &str| Schedule::Cron { expression: text.to_string() }.next_after(&from).map(|time| time.timestamp_millis());
        assert_eq!(next("0 9 * * 1-7"), Some(at(10, 9, 0)), "1-7 is every day");
        assert_eq!(next("0 9 * * 5-7"), Some(at(10, 9, 0)), "Friday to Sunday: the next 9:00 is Saturday");
        assert_eq!(next("0 9 * * 6-7"), Some(at(10, 9, 0)));
        assert_eq!(next("0 9 * * 0,6"), Some(at(10, 9, 0)), "Saturday and Sunday");
        assert_eq!(next("0 9 * * 1/2"), Some(at(12, 9, 0)), "1/2 runs from Monday to Saturday in steps of two: Monday, Wednesday, Friday; Friday 9:00 has passed");
        assert_eq!(next("0 9 * * MON-FRI"), Some(at(12, 9, 0)), "names still work");
    }

    #[test]
    fn a_bad_field_is_named_in_plain_words() {
        let why = |text: &str| Schedule::Cron { expression: text.to_string() }.validate().unwrap_err();
        assert_eq!(why("61 * * * *"), "Minute must be 0 to 59");
        assert_eq!(why("0 24 * * *"), "Hour must be 0 to 23");
        assert_eq!(why("0 0 32 * *"), "Day of the month must be 1 to 31");
        assert_eq!(why("0 0 * 13 *"), "Month must be 1 to 12");
        assert_eq!(why("0 0 * * 8"), "Weekday must be 0 to 7 (or SUN to SAT)");
        assert_eq!(why("*/0 * * * *"), "Minute: the step must be a whole number of at least 1");
        assert_eq!(why("5-1 * * * *"), "Minute: the range runs backwards");
        assert_eq!(why("a * * * *"), "Minute has a value that is not understood");
        assert_eq!(why("0 9 1 * 1"), "Use either a day of the month or a weekday, not both");
        for text in ["0 9 1 * *", "0 9 * * 1", "0 9 * * *"] {
            assert!(Schedule::Cron { expression: text.to_string() }.validate().is_ok(), "{text}");
        }
    }

    #[test]
    fn an_expression_that_never_runs_is_refused() {
        let error = Schedule::Cron { expression: "0 0 31 2 *".to_string() }.validate().unwrap_err();
        assert!(error.contains("never runs"), "{error}");
    }

    #[test]
    fn negative_and_half_hour_offsets_give_the_right_instants() {
        let new_york_winter = FixedOffset::west_opt(5 * 3600).unwrap();
        let from = new_york_winter.with_ymd_and_hms(2026, 1, 15, 10, 0, 0).unwrap();
        let next = daily_nine().next_after(&from).unwrap();
        assert_eq!(next.timestamp_millis(), new_york_winter.with_ymd_and_hms(2026, 1, 16, 9, 0, 0).unwrap().timestamp_millis());
        let india = FixedOffset::east_opt(5 * 3600 + 1800).unwrap();
        let from = india.with_ymd_and_hms(2026, 1, 15, 8, 0, 0).unwrap();
        let next = daily_nine().next_after(&from).unwrap();
        assert_eq!(next.timestamp_millis(), india.with_ymd_and_hms(2026, 1, 15, 9, 0, 0).unwrap().timestamp_millis());
    }

    #[test]
    fn a_time_inside_the_spring_forward_gap_runs_when_the_gap_ends() {
        use chrono_tz::America::New_York;
        // 2026-03-08: 02:00 to 03:00 does not exist in New York.
        let from = New_York.with_ymd_and_hms(2026, 3, 7, 12, 0, 0).unwrap();
        let schedule = Schedule::Daily { hour: 2, minute: 30 };
        let gap_day = schedule.next_after(&from).unwrap();
        assert_eq!(gap_day.format("%Y-%m-%d %H:%M").to_string(), "2026-03-08 03:00", "moved to the end of the gap, not skipped");
        let after_it = schedule.next_after(&gap_day).unwrap();
        assert_eq!(after_it.format("%Y-%m-%d %H:%M").to_string(), "2026-03-09 02:30", "and once only");
    }

    #[test]
    fn the_repeated_hour_of_fall_back_does_not_run_twice() {
        use chrono_tz::America::New_York;
        // 2026-11-01: 01:00 to 02:00 happens twice in New York.
        let from = New_York.with_ymd_and_hms(2026, 10, 31, 12, 0, 0).unwrap();
        let schedule = Schedule::Daily { hour: 1, minute: 30 };
        let first = schedule.next_after(&from).unwrap();
        assert_eq!(first.format("%Y-%m-%d %H:%M %Z").to_string(), "2026-11-01 01:30 EDT");
        let second = schedule.next_after(&first).unwrap();
        assert_eq!(second.format("%Y-%m-%d %H:%M").to_string(), "2026-11-02 01:30", "not again at 01:30 EST");
    }

    #[test]
    fn hourly_across_the_gap_is_not_doubled() {
        use chrono_tz::America::New_York;
        let from = New_York.with_ymd_and_hms(2026, 3, 8, 1, 30, 0).unwrap();
        let hour_after_gap = Schedule::Hourly.next_after(&from).unwrap();
        assert_eq!(hour_after_gap.format("%H:%M").to_string(), "03:00");
        let next = Schedule::Hourly.next_after(&hour_after_gap).unwrap();
        assert_eq!(next.format("%H:%M").to_string(), "04:00");
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
        assert_eq!(validate_draft(&RoutineDraft { name: "  ".into(), ..good.clone() }), Err(tr!("Name is required").to_string()));
        assert_eq!(validate_draft(&RoutineDraft { description: String::new(), ..good.clone() }), Err(tr!("Description is required").to_string()));
        assert_eq!(validate_draft(&RoutineDraft { folder: None, ..good.clone() }), Err(tr!("Choose a folder").to_string()));
        assert!(validate_draft(&RoutineDraft { schedule: Schedule::Cron { expression: "x".into() }, ..good }).is_err());
    }

    fn run(routine: &str, due: i64) -> RoutineRun {
        RoutineRun { routine_id: routine.to_string(), due_ms: due, started_ms: due, outcome: RunOutcome::Started, session_id: None, routine_created: 0 }
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
