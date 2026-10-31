use std::collections::{BTreeMap, BTreeSet};

use crate::{SessionId, TokenUsage};

/// The model of a row that counts the person's own messages; such a row has no tokens.
pub const NO_MODEL: &str = "";

/// Columns of the heatmap: weeks, the newest at the right.
pub const HEATMAP_WEEKS: usize = 27;

/// A calendar day in the person's time zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Day {
    pub year: i32,
    pub month: u8,
    pub day: u8,
}

impl Day {
    pub fn new(year: i32, month: u8, day: u8) -> Day {
        Day { year, month, day }
    }

    /// Days since 1970-01-01 (negative before it). Howard Hinnant's `days_from_civil`.
    pub fn epoch_days(self) -> i64 {
        let y = i64::from(self.year) - i64::from(self.month <= 2);
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (i64::from(self.month) + 9) % 12;
        let doy = (153 * mp + 2) / 5 + i64::from(self.day) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    /// The inverse of `epoch_days` (Hinnant's `civil_from_days`).
    pub fn from_epoch_days(days: i64) -> Day {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u8;
        let month = (if mp < 10 { mp + 3 } else { mp - 9 }) as u8;
        let year = yoe + era * 400 + i64::from(month <= 2);
        Day { year: year as i32, month, day }
    }

    /// 0 is Sunday. 1970-01-01 was a Thursday.
    pub fn weekday(self) -> u8 {
        (self.epoch_days() + 4).rem_euclid(7) as u8
    }

    pub fn plus_days(self, days: i64) -> Day {
        Day::from_epoch_days(self.epoch_days() + days)
    }
}

/// What one session did in one hour with one model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatsRow {
    pub day: Day,
    /// 0 to 23, local time.
    pub hour: u8,
    pub model: String,
    pub messages: u64,
    pub tokens: TokenUsage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStats {
    pub session: SessionId,
    pub rows: Vec<StatsRow>,
}

/// Every session's rows. The dashboard asks it questions; it holds no I/O.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UsageStats {
    pub sessions: Vec<SessionStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsRange {
    All,
    Last30Days,
    Last7Days,
}

impl StatsRange {
    /// The first day inside the range, `None` when it has no start.
    pub fn first_day(self, today: Day) -> Option<Day> {
        match self {
            StatsRange::All => None,
            StatsRange::Last30Days => Some(today.plus_days(-29)),
            StatsRange::Last7Days => Some(today.plus_days(-6)),
        }
    }

    fn contains(self, day: Day, today: Day) -> bool {
        day <= today && self.first_day(today).is_none_or(|first| day >= first)
    }
}

/// Input, output, cache reads and cache writes: what the dashboard calls tokens.
pub fn all_tokens(tokens: &TokenUsage) -> u64 {
    tokens.input + tokens.output + tokens.cache_read + tokens.cache_write()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelTotals {
    pub model: String,
    pub input: u64,
    pub output: u64,
    /// With cache reads and writes.
    pub total: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RangeStats {
    pub session_count: usize,
    pub message_count: u64,
    pub total_tokens: u64,
    pub active_days: usize,
    pub peak_hour: Option<u8>,
    pub favourite_model: Option<String>,
    /// Most tokens first, then by name.
    pub models: Vec<ModelTotals>,
    /// Tokens per day per model.
    pub days: BTreeMap<Day, BTreeMap<String, u64>>,
}

impl UsageStats {
    pub fn is_empty(&self) -> bool {
        self.sessions.iter().all(|session| session.rows.is_empty())
    }

    pub fn in_range(&self, range: StatsRange, today: Day) -> RangeStats {
        let mut out = RangeStats::default();
        let mut active = BTreeSet::new();
        let mut hours = [0u64; 24];
        let mut per_model: BTreeMap<String, (u64, u64, u64)> = BTreeMap::new();
        for session in &self.sessions {
            let mut seen = false;
            for row in session.rows.iter().filter(|row| range.contains(row.day, today)) {
                seen = true;
                active.insert(row.day);
                out.message_count += row.messages;
                if let Some(slot) = hours.get_mut(usize::from(row.hour)) {
                    *slot += row.messages;
                }
                let total = all_tokens(&row.tokens);
                out.total_tokens += total;
                if row.model != NO_MODEL && total > 0 {
                    let line = per_model.entry(row.model.clone()).or_default();
                    line.0 += row.tokens.input;
                    line.1 += row.tokens.output;
                    line.2 += total;
                    *out.days.entry(row.day).or_default().entry(row.model.clone()).or_default() += total;
                }
            }
            if seen {
                out.session_count += 1;
            }
        }
        out.active_days = active.len();
        let mut best: Option<(u8, u64)> = None;
        for (hour, count) in hours.iter().enumerate() {
            if *count > 0 && best.is_none_or(|(_, top)| *count > top) {
                best = Some((hour as u8, *count));
            }
        }
        out.peak_hour = best.map(|(hour, _)| hour);
        let mut models: Vec<ModelTotals> = per_model
            .into_iter()
            .map(|(model, (input, output, total))| ModelTotals { model, input, output, total })
            .collect();
        models.sort_by(|a, b| b.total.cmp(&a.total).then_with(|| a.model.cmp(&b.model)));
        out.favourite_model = models.first().map(|line| line.model.clone());
        out.models = models;
        out
    }

    /// Tokens per day for the `HEATMAP_WEEKS` weeks ending with the week of `today`: 7 rows (Sunday
    /// first) of `HEATMAP_WEEKS` cells. A day after `today` is `None`; a day without use is `Some(0)`.
    /// Every session counts, whatever range the dashboard shows.
    pub fn heatmap(&self, today: Day) -> Vec<Vec<Option<u64>>> {
        let mut per_day: BTreeMap<Day, u64> = BTreeMap::new();
        for row in self.sessions.iter().flat_map(|session| session.rows.iter()) {
            *per_day.entry(row.day).or_default() += all_tokens(&row.tokens);
        }
        let last_week_start = today.plus_days(-i64::from(today.weekday()));
        (0..7)
            .map(|weekday| {
                (0..HEATMAP_WEEKS)
                    .map(|column| {
                        let weeks_back = (HEATMAP_WEEKS - 1 - column) as i64;
                        let day = last_week_start.plus_days(-7 * weeks_back + weekday);
                        (day <= today).then(|| per_day.get(&day).copied().unwrap_or(0))
                    })
                    .collect()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(year: i32, month: u8, day: u8) -> Day {
        Day::new(year, month, day)
    }

    fn row(day: Day, hour: u8, model: &str, messages: u64, input: u64, output: u64) -> StatsRow {
        StatsRow { day, hour, model: model.to_string(), messages, tokens: TokenUsage { input, output, ..Default::default() } }
    }

    fn stats(sessions: Vec<Vec<StatsRow>>) -> UsageStats {
        UsageStats {
            sessions: sessions
                .into_iter()
                .enumerate()
                .map(|(i, rows)| SessionStats { session: SessionId(format!("s{i}")), rows })
                .collect(),
        }
    }

    #[test]
    fn day_arithmetic_matches_known_dates() {
        assert_eq!(d(1970, 1, 1).epoch_days(), 0);
        assert_eq!(d(2000, 3, 1).epoch_days(), 11_017);
        assert_eq!(d(1970, 1, 1).weekday(), 4, "a Thursday");
        assert_eq!(d(2026, 10, 3).weekday(), 6, "a Saturday");
        assert_eq!(d(2026, 10, 4).weekday(), 0, "a Sunday");
        assert_eq!(d(2026, 3, 1).plus_days(-1), d(2026, 2, 28));
        assert_eq!(d(2028, 3, 1).plus_days(-1), d(2028, 2, 29), "a leap year");
        assert_eq!(d(2026, 12, 31).plus_days(1), d(2027, 1, 1));
        for days in [-800_i64, -1, 0, 1, 11_017, 20_000] {
            assert_eq!(Day::from_epoch_days(days).epoch_days(), days);
        }
    }

    #[test]
    fn the_ranges_include_today_and_the_days_before_it() {
        let today = d(2026, 10, 3);
        let stats = stats(vec![vec![
            row(today, 9, "m", 1, 10, 0),
            row(today.plus_days(-6), 9, "m", 1, 20, 0),
            row(today.plus_days(-7), 9, "m", 1, 40, 0),
            row(today.plus_days(-29), 9, "m", 1, 80, 0),
            row(today.plus_days(-30), 9, "m", 1, 160, 0),
        ]]);
        assert_eq!(stats.in_range(StatsRange::Last7Days, today).total_tokens, 30);
        assert_eq!(stats.in_range(StatsRange::Last30Days, today).total_tokens, 150);
        assert_eq!(stats.in_range(StatsRange::All, today).total_tokens, 310);
    }

    #[test]
    fn a_row_dated_after_today_is_in_no_range() {
        let today = d(2026, 10, 3);
        let stats = stats(vec![vec![row(today.plus_days(1), 9, "m", 5, 100, 0), row(today, 9, "m", 1, 1, 0)]]);
        for range in [StatsRange::All, StatsRange::Last30Days, StatsRange::Last7Days] {
            let found = stats.in_range(range, today);
            assert_eq!(found.total_tokens, 1);
            assert_eq!(found.message_count, 1);
        }
    }

    #[test]
    fn empty_stats_give_empty_answers() {
        let found = UsageStats::default().in_range(StatsRange::All, d(2026, 10, 3));
        assert_eq!(found, RangeStats::default());
        assert_eq!(found.peak_hour, None);
        assert_eq!(found.favourite_model, None);
        assert!(UsageStats::default().is_empty());
    }

    #[test]
    fn sessions_count_when_active_in_the_range_and_days_are_distinct() {
        let today = d(2026, 10, 3);
        let stats = stats(vec![
            vec![row(today, 9, "m", 1, 1, 0), row(today, 10, "m", 1, 1, 0)],
            vec![row(today.plus_days(-40), 9, "m", 1, 1, 0)],
            vec![row(today.plus_days(-1), 9, "m", 1, 1, 0)],
        ]);
        let week = stats.in_range(StatsRange::Last7Days, today);
        assert_eq!(week.session_count, 2);
        assert_eq!(week.active_days, 2);
        assert_eq!(stats.in_range(StatsRange::All, today).session_count, 3);
        assert_eq!(stats.in_range(StatsRange::All, today).active_days, 3);
    }

    #[test]
    fn total_tokens_include_cache_and_the_model_lines_split_input_and_output() {
        let today = d(2026, 10, 3);
        let cached = StatsRow {
            day: today,
            hour: 1,
            model: "a".to_string(),
            messages: 1,
            tokens: TokenUsage { input: 1, output: 2, cache_read: 4, cache_write_5m: 8, cache_write_1h: 16, web_search_requests: 99 },
        };
        let found = stats(vec![vec![cached]]).in_range(StatsRange::All, today);
        assert_eq!(found.total_tokens, 31, "web searches are not tokens");
        assert_eq!(found.models, vec![ModelTotals { model: "a".to_string(), input: 1, output: 2, total: 31 }]);
    }

    #[test]
    fn the_peak_hour_is_the_busiest_and_a_tie_goes_to_the_earlier_hour() {
        let today = d(2026, 10, 3);
        let tied = stats(vec![vec![row(today, 21, "m", 3, 0, 0), row(today, 9, "m", 3, 0, 0), row(today, 12, "m", 1, 0, 0)]]);
        assert_eq!(tied.in_range(StatsRange::All, today).peak_hour, Some(9));
        let lone = stats(vec![vec![row(today, 21, "m", 4, 0, 0), row(today, 9, "m", 3, 0, 0)]]);
        assert_eq!(lone.in_range(StatsRange::All, today).peak_hour, Some(21));
    }

    #[test]
    fn models_are_ordered_by_tokens_then_name_and_the_favourite_is_the_first() {
        let today = d(2026, 10, 3);
        let found = stats(vec![vec![
            row(today, 1, "b", 1, 50, 0),
            row(today, 1, "a", 1, 50, 0),
            row(today, 1, "c", 1, 70, 0),
            row(today, 1, NO_MODEL, 9, 0, 0),
        ]])
        .in_range(StatsRange::All, today);
        let names: Vec<_> = found.models.iter().map(|m| m.model.as_str()).collect();
        assert_eq!(names, vec!["c", "a", "b"], "the user-message row is not a model");
        assert_eq!(found.favourite_model.as_deref(), Some("c"));
        assert_eq!(found.message_count, 12, "user rows still count as messages");
    }

    #[test]
    fn per_day_tokens_are_kept_per_model() {
        let today = d(2026, 10, 3);
        let found = stats(vec![vec![row(today, 1, "a", 1, 5, 0), row(today, 2, "a", 1, 5, 0), row(today, 3, "b", 1, 1, 0)]])
            .in_range(StatsRange::All, today);
        let day = &found.days[&today];
        assert_eq!(day["a"], 10);
        assert_eq!(day["b"], 1);
    }

    fn cell(map: &[Vec<Option<u64>>], row: usize, col: usize) -> Option<u64> {
        map[row][col]
    }

    #[test]
    fn the_heatmap_ends_with_the_week_of_today_and_ignores_later_days() {
        let saturday = d(2026, 10, 3);
        let stats = stats(vec![vec![row(saturday, 1, "m", 1, 7, 0), row(saturday.plus_days(-6), 1, "m", 1, 3, 0)]]);
        let map = stats.heatmap(saturday);
        assert_eq!(map.len(), 7);
        assert!(map.iter().all(|r| r.len() == HEATMAP_WEEKS));
        assert_eq!(cell(&map, 6, 26), Some(7), "Saturday of the last week");
        assert_eq!(cell(&map, 0, 26), Some(3), "Sunday of the last week");
        assert_eq!(cell(&map, 3, 26), Some(0), "a day with no use is zero, not missing");
        assert_eq!(cell(&map, 0, 0), Some(0));

        let sunday = d(2026, 10, 4);
        let map = stats.heatmap(sunday);
        assert_eq!(cell(&map, 0, 26), Some(0), "today is the first row of the last column");
        assert_eq!(cell(&map, 1, 26), None, "Monday has not happened");
        assert_eq!(cell(&map, 6, 25), Some(7), "yesterday is the previous column's Saturday");
    }

    #[test]
    fn the_heatmap_does_not_depend_on_a_range() {
        let today = d(2026, 10, 3);
        let stats = stats(vec![vec![row(today.plus_days(-100), 1, "m", 1, 9, 0)]]);
        let total: u64 = stats.heatmap(today).iter().flatten().flatten().sum();
        assert_eq!(total, 9);
    }
}
