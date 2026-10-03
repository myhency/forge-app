# Home Dashboard and New-Session Composer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a usage dashboard (tiles, heatmap, per-model chart) on the new-session screen, and reshape that screen's folder, mode, model and send controls to match Claude Code desktop.

**Architecture:** A pure domain type (`UsageStats`) holds per-session rows of (day, hour, model, messages, tokens) and answers range questions. A new port `UsageStatsPort` is implemented by a `forge-adapter-claude-fs` adapter that streams every transcript once and caches the rows per file. A pure view-model module in `forge-entities` turns `RangeStats` into strings, heat levels and chart geometry; a `UsageCard` widget draws it. The new-session composer gets a draft (mode and model chosen before the session exists) that is passed to `open_new`.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 (desktop), tokio, `time` 0.3, serde_json, mockall, tempfile.

**Spec:** `docs/superpowers/specs/2026-10-03-home-dashboard-design.md`

## Global Constraints

- Hexagonal layout stays: domain has no I/O and no `time` dependency; ports in `forge-ports`; only `forge-app/src/main.rs` names concrete adapters.
- Total tokens = input + output + cache read + cache write (5m + 1h). Web-search requests are not tokens.
- Days and hours are in the person's local time zone; the port takes `utc_offset_seconds: i32` and the adapter converts.
- The heatmap ignores the range (always all sessions): 7 rows (Sunday first) by 27 columns (weeks, newest right, the last column holds today).
- Ranges: `All`, `Last30Days` (today and the 29 days before), `Last7Days` (today and the 6 days before).
- The book caption, microphone, "No folder", other environments and the effort picker on the new-session screen are not built.
- UI text is English. Colours come from CSS variables in `crates/forge-shared/src/theme.css` (light and dark).
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Work on branch `plan-22`; run commands from `/Users/james/orca/workspaces/forge/plan-22`.

## Review Focus

1. A transcript line stamped in the future (clock skew): not drawn in any range, nothing panics (Task 1 test).
2. A line whose `timestamp` is missing or not RFC 3339: skipped, not counted (Task 3 test).
3. A negative UTC offset moves a message to the previous local day (Task 3 test).
4. Model ids the app does not list (`claude-sonnet-5-5`, dated `claude-haiku-4-5-20251001`) read as "Sonnet 5.5", "Haiku 4.5" in the legend (Task 5 test).
5. A transcript that is rewritten (same path, new size) is read again, and one that did not change is not (Task 3 test).

---

### Task 1: Domain statistics

**Files:**
- Create: `crates/forge-domain/src/usage_stats.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Produces (all `pub`, exported from `forge_domain`):
  - `Day { year: i32, month: u8, day: u8 }` with `Day::new(year, month, day)`, `epoch_days(self) -> i64`, `from_epoch_days(i64) -> Day`, `weekday(self) -> u8` (0 = Sunday), `plus_days(self, i64) -> Day`. Derives `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash`.
  - `StatsRow { day: Day, hour: u8, model: String, messages: u64, tokens: TokenUsage }`
  - `SessionStats { session: SessionId, rows: Vec<StatsRow> }`
  - `UsageStats { sessions: Vec<SessionStats> }` (`Default`) with `in_range(&self, StatsRange, today: Day) -> RangeStats`, `heatmap(&self, today: Day) -> Vec<Vec<Option<u64>>>` (7 rows of `HEATMAP_WEEKS` cells), `is_empty(&self) -> bool`
  - `StatsRange { All, Last30Days, Last7Days }` (`Copy`) with `first_day(self, today: Day) -> Option<Day>`
  - `RangeStats { session_count: usize, message_count: u64, total_tokens: u64, active_days: usize, peak_hour: Option<u8>, favourite_model: Option<String>, models: Vec<ModelTotals>, days: BTreeMap<Day, BTreeMap<String, u64>> }` (`Default`)
  - `ModelTotals { model: String, input: u64, output: u64, total: u64 }`
  - `all_tokens(&TokenUsage) -> u64`, `NO_MODEL: &str = ""` (the model of a row that counts user messages), `HEATMAP_WEEKS: usize = 27`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/usage_stats.rs` containing only the test module for now (the types come in Step 3; the file will not compile until then, which is the expected failure):

```rust
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
        let stats = stats(vec![vec![row(today, 21, "m", 3, 0, 0), row(today, 9, "m", 3, 0, 0), row(today, 12, "m", 1, 0, 0)]]);
        assert_eq!(stats.in_range(StatsRange::All, today).peak_hour, Some(9));
        let lone = self::stats(vec![vec![row(today, 21, "m", 4, 0, 0), row(today, 9, "m", 3, 0, 0)]]);
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-domain usage_stats`
Expected: compile error (`Day`, `StatsRow`, ... not found). The module is not declared yet, so first add `mod usage_stats;` and the `pub use` line to `crates/forge-domain/src/lib.rs`:

```rust
mod usage_stats;
```
and, after the existing `pub use usage::...` line:
```rust
pub use usage_stats::{
    all_tokens, Day, ModelTotals, RangeStats, SessionStats, StatsRange, StatsRow, UsageStats, HEATMAP_WEEKS, NO_MODEL,
};
```
Run again; expected: compile errors for the missing types.

- [ ] **Step 3: Write the implementation**

Put this above the `#[cfg(test)]` module in `crates/forge-domain/src/usage_stats.rs`:

```rust
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
```

`Option::is_none_or` needs Rust 1.82; if `rustc --version` is older, write `.map_or(true, |..| ..)` instead.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-domain usage_stats`
Expected: all tests in the module PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain/src/usage_stats.rs crates/forge-domain/src/lib.rs
git commit -m "feat: usage statistics in the domain, with ranges and a heatmap

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The port

**Files:**
- Create: `crates/forge-ports/src/usage_stats_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::UsageStats`
- Produces: `trait UsageStatsPort { async fn load(&self, utc_offset_seconds: i32) -> Result<UsageStats, UsageStatsError> }`, `UsageStatsError::ReadFailed(String)`, and under `mocks`/test `MockUsageStatsPort`.

- [ ] **Step 1: Write the port with its test**

Create `crates/forge-ports/src/usage_stats_port.rs`:

```rust
use async_trait::async_trait;
use forge_domain::UsageStats;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Error)]
pub enum UsageStatsError {
    #[error("failed to read usage: {0}")]
    ReadFailed(String),
}

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait UsageStatsPort: Send + Sync {
    /// What every session did, by day and hour in the time zone `utc_offset_seconds` east of UTC.
    /// A first call may read every transcript; later calls read only what changed.
    async fn load(&self, utc_offset_seconds: i32) -> Result<UsageStats, UsageStatsError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mock_usage_stats_port_returns_what_it_is_told() {
        let mut mock = MockUsageStatsPort::new();
        mock.expect_load().withf(|offset| *offset == 32_400).times(1).returning(|_| Ok(UsageStats::default()));
        assert!(mock.load(32_400).await.unwrap().is_empty());
    }
}
```

In `crates/forge-ports/src/lib.rs` add `mod usage_stats_port;`, `pub use usage_stats_port::{UsageStatsError, UsageStatsPort};` and, with the other mock exports:

```rust
#[cfg(any(test, feature = "mocks"))]
pub use usage_stats_port::MockUsageStatsPort;
```

- [ ] **Step 2: Run the test**

Run: `cargo test -p forge-ports usage_stats`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: UsageStatsPort

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The adapter

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/usage_stats.rs`
- Modify: `crates/forge-adapter-claude-fs/src/usage.rs` (make `token_usage` `pub(crate)`), `crates/forge-adapter-claude-fs/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{Day, SessionId, SessionStats, StatsRow, TokenUsage, UsageStats, NO_MODEL}`, `forge_ports::{UsageStatsError, UsageStatsPort}`, `crate::usage::token_usage(&Value) -> Option<TokenUsage>`.
- Produces: `ClaudeUsageStatsAdapter::new(claude_home: PathBuf)`, implementing `UsageStatsPort`; test-only `files_read(&self) -> usize`.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-adapter-claude-fs/src/usage.rs` change `fn token_usage(` to `pub(crate) fn token_usage(`. In `lib.rs` add `mod usage_stats;` and `pub use usage_stats::ClaudeUsageStatsAdapter;`.

Create `crates/forge-adapter-claude-fs/src/usage_stats.rs` with the tests first (the implementation in Step 3 goes above them):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;

    const UTC: i32 = 0;
    const JST: i32 = 9 * 3600;

    fn write(home: &Path, project: &str, id: &str, lines: &[Value]) -> PathBuf {
        let dir = home.join("projects").join(project);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(format!("{id}.jsonl"));
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        fs::write(&path, text).unwrap();
        path
    }

    fn assistant(ts: &str, id: &str, model: &str, input: u64, output: u64) -> Value {
        json!({"type": "assistant", "timestamp": ts,
               "message": {"id": id, "model": model, "content": [],
                           "usage": {"input_tokens": input, "output_tokens": output}}})
    }

    fn user_text(ts: &str, text: &str) -> Value {
        json!({"type": "user", "timestamp": ts, "message": {"role": "user", "content": text}})
    }

    async fn rows_of(home: &Path, offset: i32) -> Vec<StatsRow> {
        let adapter = ClaudeUsageStatsAdapter::new(home.to_path_buf());
        let stats = adapter.load(offset).await.unwrap();
        stats.sessions.into_iter().flat_map(|s| s.rows).collect()
    }

    #[tokio::test]
    async fn a_message_written_on_several_lines_counts_once_with_the_last_usage() {
        let home = tempfile::tempdir().unwrap();
        write(
            home.path(),
            "p",
            "s1",
            &[
                assistant("2026-10-03T10:00:00Z", "m1", "claude-sonnet-5", 5, 1),
                assistant("2026-10-03T10:00:05Z", "m1", "claude-sonnet-5", 5, 9),
            ],
        );
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].messages, 1);
        assert_eq!((rows[0].tokens.input, rows[0].tokens.output), (5, 9));
        assert_eq!((rows[0].day, rows[0].hour), (Day::new(2026, 10, 3), 10));
    }

    #[tokio::test]
    async fn sidechain_meta_synthetic_and_tool_result_lines_are_not_counted() {
        let home = tempfile::tempdir().unwrap();
        let mut sidechain = assistant("2026-10-03T10:00:00Z", "m2", "claude-sonnet-5", 100, 100);
        sidechain["isSidechain"] = json!(true);
        let mut meta = user_text("2026-10-03T10:00:00Z", "caveat");
        meta["isMeta"] = json!(true);
        let tool_result = json!({"type": "user", "timestamp": "2026-10-03T10:00:00Z", "toolUseResult": {"x": 1},
            "message": {"role": "user", "content": [{"type": "tool_result", "tool_use_id": "t", "content": "ok"}]}});
        write(
            home.path(),
            "p",
            "s1",
            &[
                sidechain,
                meta,
                tool_result,
                assistant("2026-10-03T10:00:00Z", "m3", "<synthetic>", 1, 1),
                user_text("2026-10-03T10:00:00Z", "hello"),
            ],
        );
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.len(), 1, "only the person's own message is left");
        assert_eq!(rows[0].model, NO_MODEL);
        assert_eq!(rows[0].messages, 1);
    }

    #[tokio::test]
    async fn a_line_without_a_usable_timestamp_or_with_broken_json_is_skipped() {
        let home = tempfile::tempdir().unwrap();
        let path = write(
            home.path(),
            "p",
            "s1",
            &[
                json!({"type": "user", "message": {"role": "user", "content": "no stamp"}}),
                user_text("not a time", "bad stamp"),
                user_text("2026-10-03T10:00:00Z", "fine"),
            ],
        );
        let mut text = fs::read_to_string(&path).unwrap();
        text.push_str("{ this is not json\n");
        fs::write(&path, text).unwrap();
        let rows = rows_of(home.path(), UTC).await;
        assert_eq!(rows.iter().map(|r| r.messages).sum::<u64>(), 1);
    }

    #[tokio::test]
    async fn the_utc_offset_moves_a_late_message_to_the_next_local_day() {
        let home = tempfile::tempdir().unwrap();
        write(home.path(), "p", "s1", &[assistant("2026-10-03T23:30:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let east = rows_of(home.path(), JST).await;
        assert_eq!((east[0].day, east[0].hour), (Day::new(2026, 10, 4), 8));
        let west = rows_of(home.path(), -8 * 3600).await;
        assert_eq!((west[0].day, west[0].hour), (Day::new(2026, 10, 3), 15));
        let early = tempfile::tempdir().unwrap();
        write(early.path(), "p", "s1", &[assistant("2026-10-03T03:00:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let back = rows_of(early.path(), -8 * 3600).await;
        assert_eq!((back[0].day, back[0].hour), (Day::new(2026, 10, 2), 19), "a negative offset reaches the day before");
    }

    #[tokio::test]
    async fn an_unchanged_file_is_not_read_again_and_a_rewritten_one_is() {
        let home = tempfile::tempdir().unwrap();
        let path = write(home.path(), "p", "s1", &[assistant("2026-10-03T10:00:00Z", "m1", "claude-sonnet-5", 1, 1)]);
        let adapter = ClaudeUsageStatsAdapter::new(home.path().to_path_buf());
        adapter.load(UTC).await.unwrap();
        adapter.load(UTC).await.unwrap();
        assert_eq!(adapter.files_read(), 1);

        adapter.load(JST).await.unwrap();
        assert_eq!(adapter.files_read(), 2, "another offset needs another reading");

        let mut text = fs::read_to_string(&path).unwrap();
        text.push_str(&format!("{}\n", assistant("2026-10-03T11:00:00Z", "m2", "claude-sonnet-5", 2, 2)));
        fs::write(&path, text).unwrap();
        let stats = adapter.load(JST).await.unwrap();
        assert_eq!(adapter.files_read(), 3);
        assert_eq!(stats.sessions[0].rows.iter().map(|r| r.messages).sum::<u64>(), 2);
    }

    #[tokio::test]
    async fn a_deleted_transcript_leaves_the_stats() {
        let home = tempfile::tempdir().unwrap();
        let path = write(home.path(), "p", "s1", &[user_text("2026-10-03T10:00:00Z", "hi")]);
        let adapter = ClaudeUsageStatsAdapter::new(home.path().to_path_buf());
        assert_eq!(adapter.load(UTC).await.unwrap().sessions.len(), 1);
        fs::remove_file(path).unwrap();
        assert!(adapter.load(UTC).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_missing_projects_folder_is_empty_stats_and_sessions_are_named_by_file() {
        let home = tempfile::tempdir().unwrap();
        assert!(ClaudeUsageStatsAdapter::new(home.path().to_path_buf()).load(UTC).await.unwrap().is_empty());
        write(home.path(), "p", "abc-123", &[user_text("2026-10-03T10:00:00Z", "hi")]);
        fs::write(home.path().join("projects/p/notes.txt"), "x").unwrap();
        let stats = ClaudeUsageStatsAdapter::new(home.path().to_path_buf()).load(UTC).await.unwrap();
        assert_eq!(stats.sessions.len(), 1);
        assert_eq!(stats.sessions[0].session, SessionId("abc-123".to_string()));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p forge-adapter-claude-fs usage_stats`
Expected: compile errors (`ClaudeUsageStatsAdapter` not found).

- [ ] **Step 3: Write the implementation**

Put above the test module in `crates/forge-adapter-claude-fs/src/usage_stats.rs`:

```rust
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use async_trait::async_trait;
use forge_domain::{Day, SessionId, SessionStats, StatsRow, TokenUsage, UsageStats, NO_MODEL};
use forge_ports::{UsageStatsError, UsageStatsPort};
use serde_json::Value;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::usage::token_usage;

/// Folds one transcript into rows of (day, hour, model). The rules are `UsageCollector`'s: Claude Code
/// writes one line per content block, so an assistant message is kept once per `message.id`, the
/// last usage winning and the first line's time. Sidechain and meta lines, and messages Claude Code
/// wrote itself (`<synthetic>`), are skipped. A person's message is a user line with text in it (a
/// tool result is not). A line without a time that parses is skipped.
struct StatsCollector {
    offset: UtcOffset,
    by_message: HashMap<String, Entry>,
    unnamed: Vec<Entry>,
    prompts: Vec<(Day, u8)>,
}

struct Entry {
    when: (Day, u8),
    model: String,
    usage: TokenUsage,
}

impl StatsCollector {
    fn new(offset: UtcOffset) -> StatsCollector {
        StatsCollector { offset, by_message: HashMap::new(), unnamed: Vec::new(), prompts: Vec::new() }
    }

    fn when(&self, line: &Value) -> Option<(Day, u8)> {
        let stamp = line.get("timestamp").and_then(Value::as_str)?;
        let local = OffsetDateTime::parse(stamp, &Rfc3339).ok()?.to_offset(self.offset);
        let date = local.date();
        Some((Day::new(date.year(), u8::from(date.month()), date.day()), local.hour()))
    }

    fn see(&mut self, line: &Value) {
        let flagged = |key: &str| line.get(key).and_then(Value::as_bool) == Some(true);
        if flagged("isSidechain") || flagged("isMeta") {
            return;
        }
        let Some(when) = self.when(line) else { return };
        match line.get("type").and_then(Value::as_str) {
            Some("assistant") => self.see_assistant(line, when),
            Some("user") if is_prompt(line) => self.prompts.push(when),
            _ => {}
        }
    }

    fn see_assistant(&mut self, line: &Value, when: (Day, u8)) {
        let Some(message) = line.get("message") else { return };
        let Some(model) = message.get("model").and_then(Value::as_str) else { return };
        if model.is_empty() || model.starts_with('<') {
            return;
        }
        let usage = message.get("usage").and_then(token_usage).unwrap_or_default();
        match message.get("id").and_then(Value::as_str) {
            Some(id) => {
                let entry = self.by_message.entry(id.to_string()).or_insert_with(|| Entry { when, model: model.to_string(), usage });
                entry.model = model.to_string();
                entry.usage = usage;
            }
            None => self.unnamed.push(Entry { when, model: model.to_string(), usage }),
        }
    }

    fn finish(self) -> Vec<StatsRow> {
        let mut grouped: BTreeMap<(Day, u8, String), (u64, TokenUsage)> = BTreeMap::new();
        for entry in self.by_message.into_values().chain(self.unnamed) {
            let slot = grouped.entry((entry.when.0, entry.when.1, entry.model)).or_default();
            slot.0 += 1;
            slot.1 += entry.usage;
        }
        for (day, hour) in self.prompts {
            grouped.entry((day, hour, NO_MODEL.to_string())).or_default().0 += 1;
        }
        grouped
            .into_iter()
            .map(|((day, hour, model), (messages, tokens))| StatsRow { day, hour, model, messages, tokens })
            .collect()
    }
}

// A user line the person wrote: text, not a tool result.
fn is_prompt(line: &Value) -> bool {
    if line.get("toolUseResult").is_some() {
        return false;
    }
    match line.pointer("/message/content") {
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Array(blocks)) => blocks.iter().any(|block| block.get("type").and_then(Value::as_str) == Some("text")),
        _ => false,
    }
}

#[derive(Clone, PartialEq)]
struct FileKey {
    modified: Option<SystemTime>,
    len: u64,
    offset: i32,
}

struct Cached {
    key: FileKey,
    rows: Vec<StatsRow>,
}

/// Reads the statistics from the transcripts under `<claude home>/projects/*/*.jsonl`. The rows of
/// each file are kept in memory, keyed by the file's modified time, size and the UTC offset asked for,
/// so only a new or changed transcript is read again.
pub struct ClaudeUsageStatsAdapter {
    claude_home: PathBuf,
    cache: Mutex<HashMap<PathBuf, Cached>>,
    reads: AtomicUsize,
}

impl ClaudeUsageStatsAdapter {
    pub fn new(claude_home: PathBuf) -> ClaudeUsageStatsAdapter {
        ClaudeUsageStatsAdapter { claude_home, cache: Mutex::new(HashMap::new()), reads: AtomicUsize::new(0) }
    }

    #[cfg(test)]
    fn files_read(&self) -> usize {
        self.reads.load(Ordering::SeqCst)
    }
}

async fn transcript_files(projects: &Path) -> Result<Vec<PathBuf>, UsageStatsError> {
    let mut folders = match tokio::fs::read_dir(projects).await {
        Ok(folders) => folders,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(UsageStatsError::ReadFailed(format!("{}: {error}", projects.display()))),
    };
    let mut files = Vec::new();
    while let Ok(Some(folder)) = folders.next_entry().await {
        let Ok(mut inside) = tokio::fs::read_dir(folder.path()).await else { continue };
        while let Ok(Some(entry)) = inside.next_entry().await {
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "jsonl") {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

async fn read_rows(path: &Path, offset: UtcOffset) -> Vec<StatsRow> {
    let Ok(file) = tokio::fs::File::open(path).await else { return Vec::new() };
    let mut reader = BufReader::new(file);
    let mut collector = StatsCollector::new(offset);
    let mut buffer = Vec::new();
    loop {
        buffer.clear();
        match reader.read_until(b'\n', &mut buffer).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(line) = std::str::from_utf8(&buffer) else { continue };
        if let Ok(value) = serde_json::from_str::<Value>(line) {
            collector.see(&value);
        }
    }
    collector.finish()
}

#[async_trait]
impl UsageStatsPort for ClaudeUsageStatsAdapter {
    async fn load(&self, utc_offset_seconds: i32) -> Result<UsageStats, UsageStatsError> {
        let offset = UtcOffset::from_whole_seconds(utc_offset_seconds).unwrap_or(UtcOffset::UTC);
        let files = transcript_files(&self.claude_home.join("projects")).await?;
        let mut sessions = Vec::new();
        let mut kept = HashSet::new();
        for path in files {
            let Ok(meta) = tokio::fs::metadata(&path).await else { continue };
            let key = FileKey { modified: meta.modified().ok(), len: meta.len(), offset: utc_offset_seconds };
            let cached = self.cache.lock().unwrap().get(&path).filter(|cached| cached.key == key).map(|cached| cached.rows.clone());
            let rows = match cached {
                Some(rows) => rows,
                None => {
                    let rows = read_rows(&path, offset).await;
                    self.reads.fetch_add(1, Ordering::SeqCst);
                    self.cache.lock().unwrap().insert(path.clone(), Cached { key, rows: rows.clone() });
                    rows
                }
            };
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()).filter(|_| !rows.is_empty()) {
                sessions.push(SessionStats { session: SessionId(stem.to_string()), rows });
            }
            kept.insert(path);
        }
        self.cache.lock().unwrap().retain(|path, _| kept.contains(path));
        Ok(UsageStats { sessions })
    }
}
```

`tokio` already lists `fs`, `io-util`, `rt` for this crate; `forge_domain::TokenUsage` implements `AddAssign` (used with `+=`).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-adapter-claude-fs usage_stats`
Expected: PASS. Then `cargo test -p forge-adapter-claude-fs` to check the existing usage tests still pass after the visibility change.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: claude-fs adapter reads usage statistics with a per-file cache

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Wire the port into the app

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (struct `WorkspaceServices` at line 28; accessor beside `git()` at line 460), `crates/forge-app/src/main.rs` (`build_services`, imports)
- Modify: `crates/forge-features/Cargo.toml` only if `forge-ports` is not already a dependency (it is: `git()` returns `Arc<dyn GitPort>`).

**Interfaces:**
- Consumes: `forge_ports::UsageStatsPort`, `ClaudeUsageStatsAdapter`.
- Produces: `WorkspaceServices.usage_stats: Arc<dyn UsageStatsPort>`; `Workspace::usage_stats_port(&self) -> Arc<dyn UsageStatsPort>`.

- [ ] **Step 1: Add the service field and accessor**

In `workspace.rs`, add `UsageStatsPort` to the `forge_ports` import, add to the struct after `browser`:

```rust
    pub usage_stats: Arc<dyn UsageStatsPort>,
```
and beside `pub fn git(&self)`:

```rust
    pub fn usage_stats_port(&self) -> Arc<dyn UsageStatsPort> {
        self.services.peek().usage_stats.clone()
    }
```

- [ ] **Step 2: Build it in the composition root**

In `crates/forge-app/src/main.rs` import `ClaudeUsageStatsAdapter` from `forge_adapter_claude_fs` (next to `ClaudeSessionAdapter`) and add to the `WorkspaceServices { ... }` literal:

```rust
        usage_stats: Arc::new(ClaudeUsageStatsAdapter::new(claude_home.clone())),
```
(`claude_home` is cloned earlier in the function; use `claude_home.clone()` here and leave the later uses as they are. If a later line already moves `claude_home`, place this line before it.)

- [ ] **Step 3: Verify it builds and nothing else broke**

Run: `cargo build -p forge-app && cargo test -p forge-features -p forge-app`
Expected: build succeeds, tests PASS. (Nothing else constructs `WorkspaceServices`: `grep -rn "WorkspaceServices {" crates` shows only `main.rs`.)

- [ ] **Step 4: Commit**

```bash
git add crates/forge-features crates/forge-app
git commit -m "feat: usage statistics service in the workspace

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: View model for the card

**Files:**
- Create: `crates/forge-entities/src/usage_card.rs`
- Modify: `crates/forge-entities/src/usage_view.rs` (`format_tokens` gets a `B` unit), `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{Day, RangeStats, StatsRange, ModelTotals}`, `format_tokens`, `KNOWN_MODELS`, `model_label`.
- Produces (exported from `forge_entities`):
  - `format_tokens(u64)` now prints `B` for billions (`2.4B`).
  - `Tile { label: &'static str, value: String }`, `overview_tiles(&RangeStats) -> Vec<Tile>` (6 tiles in the order Sessions, Messages, Total tokens, Active days, Peak hour, Favorite model)
  - `with_commas(u64) -> String`, `hour_label(u8) -> String`, `heat_level(tokens: u64, max: u64) -> u8`
  - `dashboard_model_label(&str) -> String`
  - `LegendRow { label: String, series: usize, input: String, output: String, share: String }`, `legend_rows(&RangeStats) -> Vec<LegendRow>`
  - `ChartBar { slot: usize, segments: Vec<(usize, u64)>, total: u64 }`, `ChartLayout { days: Vec<Day>, bars: Vec<ChartBar>, top: u64, ticks: Vec<u64> }`, `chart_layout(&RangeStats, StatsRange, today: Day) -> ChartLayout`
  - `day_label(Day) -> String` ("Oct 3")

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/usage_card.rs` with this test module (implementation in Step 3 goes above it). Also add to the `tests` module of `usage_view.rs` first:

```rust
    #[test]
    fn billions_get_their_own_unit() {
        assert_eq!(format_tokens(999_949_999), "999.9M");
        assert_eq!(format_tokens(999_950_000), "1.0B");
        assert_eq!(format_tokens(2_400_000_000), "2.4B");
    }
```

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{SessionId, SessionStats, StatsRow, TokenUsage, UsageStats, NO_MODEL};

    fn d(day: u8) -> Day {
        Day::new(2026, 10, day)
    }

    fn row(day: Day, hour: u8, model: &str, messages: u64, input: u64, output: u64) -> StatsRow {
        StatsRow { day, hour, model: model.to_string(), messages, tokens: TokenUsage { input, output, ..Default::default() } }
    }

    fn range_of(rows: Vec<StatsRow>, range: StatsRange, today: Day) -> RangeStats {
        UsageStats { sessions: vec![SessionStats { session: SessionId("s".to_string()), rows }] }.in_range(range, today)
    }

    #[test]
    fn the_tiles_follow_the_desktop_order_and_format() {
        let found = range_of(
            vec![row(d(3), 21, "claude-sonnet-5", 14_710, 1_000_000_000, 1_400_000_000), row(d(2), 9, NO_MODEL, 0, 0, 0)],
            StatsRange::All,
            d(3),
        );
        let tiles = overview_tiles(&found);
        let shown: Vec<_> = tiles.iter().map(|t| (t.label, t.value.as_str())).collect();
        assert_eq!(
            shown,
            vec![
                ("Sessions", "1"),
                ("Messages", "14,710"),
                ("Total tokens", "2.4B"),
                ("Active days", "2"),
                ("Peak hour", "9 PM"),
                ("Favorite model", "Sonnet 5"),
            ]
        );
    }

    #[test]
    fn empty_stats_show_dashes_for_peak_hour_and_favorite_model() {
        let tiles = overview_tiles(&RangeStats::default());
        assert_eq!(tiles[4].value, "–");
        assert_eq!(tiles[5].value, "–");
        assert_eq!(tiles[0].value, "0");
    }

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(with_commas(0), "0");
        assert_eq!(with_commas(999), "999");
        assert_eq!(with_commas(1_000), "1,000");
        assert_eq!(with_commas(1_234_567), "1,234,567");
    }

    #[test]
    fn hours_read_like_a_clock() {
        assert_eq!(hour_label(0), "12 AM");
        assert_eq!(hour_label(9), "9 AM");
        assert_eq!(hour_label(12), "12 PM");
        assert_eq!(hour_label(21), "9 PM");
    }

    #[test]
    fn heat_levels_split_the_busiest_day_in_quarters() {
        assert_eq!(heat_level(0, 100), 0);
        assert_eq!(heat_level(1, 100), 1);
        assert_eq!(heat_level(25, 100), 1);
        assert_eq!(heat_level(26, 100), 2);
        assert_eq!(heat_level(50, 100), 2);
        assert_eq!(heat_level(75, 100), 3);
        assert_eq!(heat_level(76, 100), 4);
        assert_eq!(heat_level(100, 100), 4);
        assert_eq!(heat_level(5, 0), 0, "no busiest day, nothing to shade");
    }

    #[test]
    fn model_ids_read_as_names_even_when_the_app_does_not_list_them() {
        assert_eq!(dashboard_model_label("claude-sonnet-5-5"), "Sonnet 5.5");
        assert_eq!(dashboard_model_label("claude-haiku-4-5-20251001"), "Haiku 4.5");
        assert_eq!(dashboard_model_label("claude-opus-4-8"), "Opus 4.8");
        assert_eq!(dashboard_model_label("claude-sonnet-5"), "Sonnet 5");
        assert_eq!(dashboard_model_label("something-else"), "something-else");
    }

    #[test]
    fn the_legend_has_a_row_per_model_with_input_output_and_share() {
        let found = range_of(
            vec![row(d(3), 1, "claude-sonnet-5", 1, 90_800, 5_300_000), row(d(3), 1, "claude-haiku-4-5-20251001", 1, 4_400, 291_500)],
            StatsRange::All,
            d(3),
        );
        let rows = legend_rows(&found);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].label, "Sonnet 5");
        assert_eq!((rows[0].input.as_str(), rows[0].output.as_str()), ("90.8k", "5.3M"));
        assert_eq!(rows[0].series, 0);
        assert_eq!(rows[1].label, "Haiku 4.5");
        assert_eq!(rows[1].series, 1);
        assert_eq!(rows[0].share, "94.7%");
        assert_eq!(rows[1].share, "5.3%");
    }

    #[test]
    fn the_axis_uses_round_steps_of_four_intervals() {
        let (top, ticks) = axis_ticks(658_000_000);
        assert_eq!(top, 800_000_000);
        assert_eq!(ticks, vec![0, 200_000_000, 400_000_000, 600_000_000, 800_000_000]);
        assert_eq!(axis_ticks(0).0, 4, "an empty chart still has an axis");
        assert_eq!(axis_ticks(7).1, vec![0, 2, 4, 6, 8]);
    }

    #[test]
    fn a_week_has_seven_slots_and_bars_only_where_there_is_use() {
        let found = range_of(
            vec![row(d(3), 1, "a", 1, 600, 0), row(d(3), 2, "b", 1, 100, 0), row(d(1), 1, "a", 1, 200, 0)],
            StatsRange::Last7Days,
            d(3),
        );
        let layout = chart_layout(&found, StatsRange::Last7Days, d(3));
        assert_eq!(layout.days.len(), 7);
        assert_eq!(layout.days[0], Day::new(2026, 9, 27));
        assert_eq!(layout.days[6], d(3));
        let slots: Vec<_> = layout.bars.iter().map(|b| b.slot).collect();
        assert_eq!(slots, vec![4, 6], "Oct 1 and Oct 3");
        assert_eq!(layout.bars[1].segments, vec![(0, 600), (1, 100)], "stacked in model order");
        assert_eq!(layout.bars[1].total, 700);
        assert_eq!(layout.top, 800);
    }

    #[test]
    fn all_time_runs_from_the_first_day_with_use_to_today() {
        let found = range_of(vec![row(d(1), 1, "a", 1, 5, 0), row(d(3), 1, "a", 1, 5, 0)], StatsRange::All, d(3));
        let layout = chart_layout(&found, StatsRange::All, d(3));
        assert_eq!(layout.days, vec![d(1), d(2), d(3)]);
        assert_eq!(layout.bars.len(), 2);
        let empty = chart_layout(&RangeStats::default(), StatsRange::All, d(3));
        assert_eq!(empty.days, vec![d(3)], "at least today");
        assert!(empty.bars.is_empty());
    }

    #[test]
    fn days_read_like_the_chart_axis() {
        assert_eq!(day_label(Day::new(2026, 10, 3)), "Oct 3");
        assert_eq!(day_label(Day::new(2026, 4, 25)), "Apr 25");
    }
}
```

- [ ] **Step 2: Run to verify it fails**

First declare the module in `crates/forge-entities/src/lib.rs`: add `mod usage_card;` and
```rust
pub use usage_card::{
    axis_ticks, chart_layout, dashboard_model_label, day_label, heat_level, hour_label, legend_rows, overview_tiles, with_commas, ChartBar,
    ChartLayout, LegendRow, Tile,
};
```
Run: `cargo test -p forge-entities usage_card usage_view`
Expected: compile errors / the `billions_get_their_own_unit` test FAILS (it prints `2400.0M`).

- [ ] **Step 3: Write the implementation**

In `usage_view.rs` replace the tail of `format_tokens`:

```rust
    let millions_tenths = (count as u128 * 10 + 500_000) / 1_000_000;
    if millions_tenths < 10_000 {
        return format!("{}.{}M", millions_tenths / 10, millions_tenths % 10);
    }
    let billions_tenths = (count as u128 * 10 + 500_000_000) / 1_000_000_000;
    format!("{}.{}B", billions_tenths / 10, billions_tenths % 10)
```
and update its doc comment to `…`3.4M`, `2.4B`.`.

Above the tests in `crates/forge-entities/src/usage_card.rs`:

```rust
use forge_domain::{Day, RangeStats, StatsRange};

use crate::model_catalog::{model_label, KNOWN_MODELS};
use crate::usage_view::format_tokens;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tile {
    pub label: &'static str,
    pub value: String,
}

pub fn with_commas(count: u64) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `12 AM`, `9 AM`, `12 PM`, `9 PM`.
pub fn hour_label(hour: u8) -> String {
    let clock = match hour % 12 {
        0 => 12,
        other => other,
    };
    format!("{clock} {}", if hour % 24 < 12 { "AM" } else { "PM" })
}

/// The six tiles of the overview, in the desktop app's order.
pub fn overview_tiles(stats: &RangeStats) -> Vec<Tile> {
    let dash = || "–".to_string();
    vec![
        Tile { label: "Sessions", value: with_commas(stats.session_count as u64) },
        Tile { label: "Messages", value: with_commas(stats.message_count) },
        Tile { label: "Total tokens", value: format_tokens(stats.total_tokens) },
        Tile { label: "Active days", value: with_commas(stats.active_days as u64) },
        Tile { label: "Peak hour", value: stats.peak_hour.map_or_else(dash, hour_label) },
        Tile { label: "Favorite model", value: stats.favourite_model.as_deref().map_or_else(dash, dashboard_model_label) },
    ]
}

/// 0 for a day without use, then 1 to 4 by the day's share of the busiest day (up to a quarter, a half,
/// three quarters, more).
pub fn heat_level(tokens: u64, max: u64) -> u8 {
    if tokens == 0 || max == 0 {
        return 0;
    }
    ((u128::from(tokens) * 4).div_ceil(u128::from(max))).clamp(1, 4) as u8
}

/// The name of a model id for the dashboard: the app's own label when it lists the id, otherwise the
/// id read as `claude-<family>-<major>-<minor>[-<date>]` (`claude-sonnet-5-5` is `Sonnet 5.5`).
pub fn dashboard_model_label(id: &str) -> String {
    if KNOWN_MODELS.iter().any(|(known, _)| *known == id) {
        return model_label(Some(id));
    }
    let Some(rest) = id.strip_prefix("claude-") else { return id.to_string() };
    let mut parts = rest.split('-');
    let Some(family) = parts.next().filter(|family| !family.is_empty() && family.chars().all(|c| c.is_ascii_alphabetic())) else {
        return id.to_string();
    };
    let numbers: Vec<&str> = parts.filter(|part| part.len() <= 2 && part.chars().all(|c| c.is_ascii_digit())).collect();
    if numbers.is_empty() {
        return id.to_string();
    }
    let mut name = family.to_string();
    name[..1].make_ascii_uppercase();
    format!("{name} {}", numbers.join("."))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegendRow {
    pub label: String,
    /// Which series colour the model has: its place in the legend.
    pub series: usize,
    pub input: String,
    pub output: String,
    /// `55.0%`.
    pub share: String,
}

pub fn legend_rows(stats: &RangeStats) -> Vec<LegendRow> {
    let sum: u64 = stats.models.iter().map(|line| line.total).sum();
    stats
        .models
        .iter()
        .enumerate()
        .map(|(series, line)| {
            let tenths = if sum == 0 { 0 } else { (u128::from(line.total) * 1000 + u128::from(sum) / 2) / u128::from(sum) };
            LegendRow {
                label: dashboard_model_label(&line.model),
                series,
                input: format_tokens(line.input),
                output: format_tokens(line.output),
                share: format!("{}.{}%", tenths / 10, tenths % 10),
            }
        })
        .collect()
}

/// A scale of four equal steps that covers `max`: the step is 1, 2 or 5 times a power of ten.
pub fn axis_ticks(max: u64) -> (u64, Vec<u64>) {
    let raw = max.div_ceil(4).max(1);
    let mut magnitude = 1u64;
    while magnitude.saturating_mul(10) <= raw {
        magnitude *= 10;
    }
    let step = [1, 2, 5, 10].iter().map(|factor| factor * magnitude).find(|step| *step >= raw).unwrap_or(10 * magnitude);
    (step * 4, (0..=4).map(|i| i * step).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartBar {
    /// Index into `ChartLayout::days`.
    pub slot: usize,
    /// (series, tokens) from the bottom up.
    pub segments: Vec<(usize, u64)>,
    pub total: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartLayout {
    pub days: Vec<Day>,
    pub bars: Vec<ChartBar>,
    pub top: u64,
    pub ticks: Vec<u64>,
}

/// The slots and bars of the per-day token chart. A week is always seven slots; the other ranges
/// run from the first day with use to `today` (at least `today`).
pub fn chart_layout(stats: &RangeStats, range: StatsRange, today: Day) -> ChartLayout {
    let first = match range {
        StatsRange::Last7Days => today.plus_days(-6),
        _ => stats.days.keys().next().copied().filter(|first| *first <= today).unwrap_or(today),
    };
    let count = (today.epoch_days() - first.epoch_days() + 1).max(1) as usize;
    let days: Vec<Day> = (0..count).map(|i| first.plus_days(i as i64)).collect();
    let mut bars = Vec::new();
    for (slot, day) in days.iter().enumerate() {
        let Some(per_model) = stats.days.get(day) else { continue };
        let segments: Vec<(usize, u64)> = stats
            .models
            .iter()
            .enumerate()
            .filter_map(|(series, line)| per_model.get(&line.model).map(|tokens| (series, *tokens)))
            .collect();
        let total = segments.iter().map(|(_, tokens)| tokens).sum();
        if total > 0 {
            bars.push(ChartBar { slot, segments, total });
        }
    }
    let (top, ticks) = axis_ticks(bars.iter().map(|bar| bar.total).max().unwrap_or(0));
    ChartLayout { days, bars, top, ticks }
}

/// `Oct 3`.
pub fn day_label(day: Day) -> String {
    const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
    format!("{} {}", MONTHS[usize::from(day.month.clamp(1, 12)) - 1], day.day)
}
```

`KNOWN_MODELS` and `model_label` are `pub` in `model_catalog.rs`; if `KNOWN_MODELS` is declared with a different tuple shape than `(&str, &str)`, adjust the `iter().any` pattern — `model_label` itself matches on `(known, _)`, so the shape is the same.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-entities`
Expected: PASS (the new `usage_card` tests, the billions test, and all earlier ones).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: view model for the usage card, and billions in format_tokens

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The UsageCard widget

**Files:**
- Create: `crates/forge-widgets/src/usage_card.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (class list in its test), `crates/forge-widgets/src/new_session_panel.rs`

**Interfaces:**
- Consumes: `Workspace::usage_stats_port()`, `forge_features::now_local()`, Task 5's `forge_entities` functions, `forge_domain::{Day, StatsRange, UsageStats}`.
- Produces: `UsageCard` component (no props), exported from `forge_widgets`.

This task has no unit tests of its own: the logic is in Tasks 1 and 5, and the repo has no render tests for widgets. It is checked by running the app (Task 9) and by the stylesheet class test.

- [ ] **Step 1: Write the widget**

Create `crates/forge-widgets/src/usage_card.rs`:

```rust
use std::sync::Arc;

use dioxus::prelude::*;
use forge_domain::{Day, StatsRange, UsageStats};
use forge_entities::{chart_layout, day_label, format_tokens, heat_level, legend_rows, overview_tiles};
use forge_features::{now_local, use_workspace};

#[derive(Clone, PartialEq)]
enum Load {
    Loading,
    Ready(Arc<UsageStats>),
    Failed(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Models,
}

fn today_and_offset() -> (Day, i32) {
    let now = now_local();
    (Day::new(now.year(), u8::from(now.month()), now.day()), now.offset().whole_seconds())
}

/// The usage card on the new-session screen: an overview (tiles and a heatmap) and a per-model chart,
/// for all time, the last 30 days or the last 7. It reads every transcript the first time, so it shows
/// "Loading…" until the answer comes, and hides itself when there is no use to show.
#[component]
pub fn UsageCard() -> Element {
    let workspace = use_workspace();
    let mut tab = use_signal(|| Tab::Overview);
    let mut range = use_signal(|| StatsRange::All);
    let mut load = use_signal(|| Load::Loading);

    use_effect(move || {
        let port = workspace.usage_stats_port();
        let (_, offset) = today_and_offset();
        spawn(async move {
            let next = match port.load(offset).await {
                Ok(stats) => Load::Ready(Arc::new(stats)),
                Err(error) => Load::Failed(error.to_string()),
            };
            load.set(next);
        });
    });

    let state = load.read().clone();
    let stats = match state {
        Load::Loading => return rsx! { div { class: "usage-card", div { class: "usage-note", "Loading usage…" } } },
        Load::Failed(message) => return rsx! { div { class: "usage-card", div { class: "usage-note error", "{message}" } } },
        Load::Ready(stats) if stats.is_empty() => return rsx! {},
        Load::Ready(stats) => stats,
    };
    let (today, _) = today_and_offset();
    let shown = stats.in_range(range(), today);

    rsx! {
        div { class: "usage-card",
            div { class: "usage-head",
                div { class: "usage-tabs",
                    button { class: if tab() == Tab::Overview { "usage-tab on" } else { "usage-tab" }, onclick: move |_| tab.set(Tab::Overview), "Overview" }
                    button { class: if tab() == Tab::Models { "usage-tab on" } else { "usage-tab" }, onclick: move |_| tab.set(Tab::Models), "Models" }
                }
                div { class: "usage-tabs",
                    for (value, text) in [(StatsRange::All, "All"), (StatsRange::Last30Days, "30d"), (StatsRange::Last7Days, "7d")] {
                        button {
                            key: "{text}",
                            class: if range() == value { "usage-tab on" } else { "usage-tab" },
                            onclick: move |_| range.set(value),
                            "{text}"
                        }
                    }
                }
            }
            if tab() == Tab::Overview {
                div { class: "usage-tiles",
                    for tile in overview_tiles(&shown) {
                        div { key: "{tile.label}", class: "usage-tile",
                            span { class: "usage-tile-label", "{tile.label}" }
                            span { class: "usage-tile-value", "{tile.value}" }
                        }
                    }
                }
                {heatmap(&stats, today)}
            } else {
                {chart(&shown, range(), today)}
            }
        }
    }
}

fn heatmap(stats: &UsageStats, today: Day) -> Element {
    let map = stats.heatmap(today);
    let max = map.iter().flatten().flatten().copied().max().unwrap_or(0);
    let weeks = map.first().map_or(0, Vec::len);
    rsx! {
        div { class: "heatmap", role: "img", "aria-label": "Tokens used per day",
            // Column by column: each column is one week, Sunday on top.
            for column in 0..weeks {
                for row in 0..7usize {
                    {
                        let cell = map[row][column];
                        let level = cell.map_or(0, |tokens| heat_level(tokens, max));
                        let text = cell.map_or_else(String::new, |tokens| format!("{}: {} tokens", day_label(day_at(today, column, row, weeks)), format_tokens(tokens)));
                        rsx! {
                            span {
                                key: "{column}-{row}",
                                class: if cell.is_none() { "heat-cell none" } else { "heat-cell" },
                                "data-level": "{level}",
                                title: "{text}",
                            }
                        }
                    }
                }
            }
        }
    }
}

// The day a heatmap cell stands for, the inverse of `UsageStats::heatmap`.
fn day_at(today: Day, column: usize, row: usize, weeks: usize) -> Day {
    let last_week_start = today.plus_days(-i64::from(today.weekday()));
    let weeks_back = (weeks - 1 - column) as i64;
    last_week_start.plus_days(-7 * weeks_back + row as i64)
}

fn chart(shown: &forge_domain::RangeStats, range: StatsRange, today: Day) -> Element {
    const WIDTH: f64 = 520.0;
    const HEIGHT: f64 = 190.0;
    const LEFT: f64 = 46.0;
    const TOP: f64 = 8.0;
    const BOTTOM: f64 = 26.0;
    let layout = chart_layout(shown, range, today);
    let plot_w = WIDTH - LEFT - 6.0;
    let plot_h = HEIGHT - TOP - BOTTOM;
    let slot_w = plot_w / layout.days.len() as f64;
    let bar_w = (slot_w * 0.62).max(2.0);
    let label_every = layout.days.len().div_ceil(7).max(1);
    let y_of = |tokens: u64| TOP + plot_h - plot_h * tokens as f64 / layout.top.max(1) as f64;
    let legend = legend_rows(shown);
    rsx! {
        div { class: "usage-chart",
            if legend.is_empty() {
                div { class: "usage-note", "No model use in this range." }
            } else {
                svg { class: "chart-svg", view_box: "0 0 {WIDTH} {HEIGHT}", role: "img", "aria-label": "Tokens per day by model",
                    for tick in layout.ticks.iter() {
                        g { key: "{tick}",
                            line { class: "chart-grid", x1: "{LEFT}", x2: "{WIDTH - 6.0}", y1: "{y_of(*tick)}", y2: "{y_of(*tick)}" }
                            text { class: "chart-axis", x: "{LEFT - 6.0}", y: "{y_of(*tick) + 3.0}", text_anchor: "end", "{format_tokens(*tick)}" }
                        }
                    }
                    for bar in layout.bars.iter() {
                        {
                            let x = LEFT + bar.slot as f64 * slot_w + (slot_w - bar_w) / 2.0;
                            let mut below = 0u64;
                            let pieces: Vec<(usize, f64, f64)> = bar
                                .segments
                                .iter()
                                .map(|(series, tokens)| {
                                    let top_y = y_of(below + tokens);
                                    let bottom_y = y_of(below);
                                    below += tokens;
                                    (*series, top_y, bottom_y - top_y)
                                })
                                .collect();
                            rsx! {
                                for (series, y, h) in pieces {
                                    rect { key: "{bar.slot}-{series}", class: "series-{series.min(5)}", x: "{x}", y: "{y}", width: "{bar_w}", height: "{h}" }
                                }
                            }
                        }
                    }
                    for (slot, day) in layout.days.iter().enumerate() {
                        if slot % label_every == 0 {
                            text { key: "{slot}", class: "chart-axis", x: "{LEFT + slot as f64 * slot_w + slot_w / 2.0}", y: "{HEIGHT - 8.0}", text_anchor: "middle", "{day_label(*day)}" }
                        }
                    }
                }
                div { class: "legend",
                    for row in legend {
                        div { key: "{row.label}", class: "legend-row",
                            span { class: "legend-swatch series-{row.series.min(5)}" }
                            span { class: "legend-name", "{row.label}" }
                            span { class: "legend-tokens", "{row.input} in · {row.output} out" }
                            span { class: "legend-share", "{row.share}" }
                        }
                    }
                }
            }
        }
    }
}
```

Dioxus note: SVG attributes in `rsx!` use snake_case for `view_box`, `text_anchor`; if the compiler reports an unknown attribute, use the string form `"viewBox": "..."` / `"text-anchor": "..."`, as `usage_ring.rs` does for its SVG.

In `crates/forge-widgets/Cargo.toml` make sure `forge-domain` is a dependency (`forge-widgets` already uses `forge_domain` types through other crates; if `cargo build` says `unresolved import forge_domain`, add `forge-domain = { path = "../forge-domain" }`).

- [ ] **Step 2: Export it and place it on the screen**

In `crates/forge-widgets/src/lib.rs` add `mod usage_card;` and `pub use usage_card::UsageCard;`.

In `new_session_panel.rs` the panel becomes (greeting and card above the composer; the composer itself is reshaped in Task 8):

```rust
use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::plus_menu::PlusMenu;
use crate::usage_card::UsageCard;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    rsx! {
        div { class: "main-pane",
            div { class: "home-top",
                h1 { class: "greeting", "What's next?" }
                UsageCard {}
            }
            NewSessionComposer {
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
            }
        }
    }
}
```
(Replaces the `pane-header` "New session" line.)

- [ ] **Step 3: Styles**

Append to `crates/forge-shared/src/theme.css`:

```css
/* Home: greeting and the usage card. */
:root {
  --series-0: #3a63d4;
  --series-1: #6d8fe0;
  --series-2: #94abe8;
  --series-3: #b7c7f0;
  --series-4: #d3dcf6;
  --series-5: #e5eafa;
  --heat-0: var(--bg-hover);
  --heat-1: #c9d6f4;
  --heat-2: #9cb3ea;
  --heat-3: #6f8fe0;
  --heat-4: #3a63d4;
}
@media (prefers-color-scheme: dark) {
  :root {
    --series-0: #5b82ea;
    --series-1: #7d9aee;
    --series-2: #9bb0f1;
    --series-3: #b7c5f5;
    --series-4: #cdd6f8;
    --series-5: #dde3fa;
    --heat-1: #2b3a63;
    --heat-2: #3b5299;
    --heat-3: #4f6dcc;
    --heat-4: #7d9aee;
  }
}
.home-top { flex: none; width: 100%; max-width: 720px; margin: 0 auto; padding: 32px 24px 0; box-sizing: border-box; }
.greeting { margin: 0 0 20px; font-size: 24px; font-weight: 600; }
.usage-card { max-width: 560px; padding: 14px; border-radius: var(--radius-lg); background: var(--bg-sidebar); }
.usage-note { padding: 24px 8px; color: var(--fg-muted); font-size: 13px; }
.usage-note.error { color: var(--danger); }
.usage-head { display: flex; justify-content: space-between; margin-bottom: 12px; }
.usage-tabs { display: flex; gap: 2px; }
.usage-tab { appearance: none; border: none; background: none; padding: 3px 10px; border-radius: 6px; color: var(--fg-muted); font: inherit; font-size: 12px; cursor: pointer; }
.usage-tab:hover { color: var(--fg); }
.usage-tab.on { background: var(--bg-hover); color: var(--fg); font-weight: 600; }
.usage-tiles { display: grid; grid-template-columns: repeat(3, 1fr); gap: 6px; margin-bottom: 10px; }
.usage-tile { display: flex; flex-direction: column; gap: 2px; padding: 8px 10px; border-radius: 8px; background: var(--bg-hover); }
.usage-tile-label { color: var(--fg-muted); font-size: 11px; }
.usage-tile-value { font-size: 15px; font-weight: 600; }
.heatmap { display: grid; grid-auto-flow: column; grid-template-rows: repeat(7, 1fr); gap: 3px; }
.heat-cell { aspect-ratio: 1; border-radius: 3px; background: var(--heat-0); }
.heat-cell.none { background: transparent; }
.heat-cell[data-level="1"] { background: var(--heat-1); }
.heat-cell[data-level="2"] { background: var(--heat-2); }
.heat-cell[data-level="3"] { background: var(--heat-3); }
.heat-cell[data-level="4"] { background: var(--heat-4); }
.chart-svg { width: 100%; height: auto; }
.chart-grid { stroke: var(--border); stroke-width: 1; }
.chart-axis { fill: var(--fg-muted); font-size: 10px; }
.series-0 { fill: var(--series-0); background: var(--series-0); }
.series-1 { fill: var(--series-1); background: var(--series-1); }
.series-2 { fill: var(--series-2); background: var(--series-2); }
.series-3 { fill: var(--series-3); background: var(--series-3); }
.series-4 { fill: var(--series-4); background: var(--series-4); }
.series-5 { fill: var(--series-5); background: var(--series-5); }
.legend { display: flex; flex-direction: column; gap: 4px; margin-top: 8px; font-size: 12px; }
.legend-row { display: flex; align-items: center; gap: 8px; }
.legend-swatch { flex: none; width: 10px; height: 10px; border-radius: 3px; }
.legend-name { flex: 1; }
.legend-tokens { color: var(--fg-muted); }
.legend-share { width: 48px; text-align: right; font-weight: 600; }
```

In `theme.rs`'s test that lists required classes, add `".usage-card"`, `".heatmap"`, `".legend"` to the array.

- [ ] **Step 4: Build and run the stylesheet test**

Run: `cargo build -p forge-app && cargo test -p forge-shared`
Expected: builds; test PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: usage card on the new-session screen

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: New-session draft, send state, folder menu rows

**Files:**
- Create: `crates/forge-features/src/folder_menu.rs`
- Modify: `crates/forge-features/src/new_session.rs`, `crates/forge-features/src/workspace.rs` (`start_new`), `crates/forge-features/src/lib.rs`

**Interfaces:**
- Produces (exported from `forge_features`):
  - `NewSessionDraft { mode: Signal<PermissionMode>, model: Signal<ModelChoice> }` (`Copy`), `use_new_session_draft() -> NewSessionDraft`, `type ModelChoice = Option<Option<String>>` (`None`: follow the saved default; `Some(None)`: the SDK's default; `Some(Some(id))`: that model)
  - `send_ready(text: &str, attachments: usize) -> bool`
  - `FolderMenuRow { label: String, path: PathBuf, checked: bool }`, `folder_menu_rows(projects: &[ProjectSummary], current: Option<&Path>) -> Vec<FolderMenuRow>`, `MAX_RECENT_FOLDERS: usize = 8`
  - `NewSessionComposer` takes a new prop `chips: Element` (next to `children`)
  - `Workspace::start_new(project, mode, model: ModelChoice, text, attachments, folders)`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/folder_menu.rs` (implementation after the tests, Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn project(path: &str) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: String::new(), session_count: 1, exists: true }
    }

    #[test]
    fn rows_keep_the_projects_order_name_the_folder_and_check_the_current_one() {
        let projects = vec![project("/work/mywork"), project("/work/forge")];
        let rows = folder_menu_rows(&projects, Some(Path::new("/work/forge")));
        let shown: Vec<_> = rows.iter().map(|r| (r.label.as_str(), r.checked)).collect();
        assert_eq!(shown, vec![("mywork", false), ("forge", true)]);
        assert_eq!(rows[1].path, PathBuf::from("/work/forge"));
    }

    #[test]
    fn at_most_eight_rows_are_offered_but_the_current_folder_is_always_among_them() {
        let projects: Vec<_> = (0..12).map(|i| project(&format!("/work/p{i}"))).collect();
        let rows = folder_menu_rows(&projects, Some(Path::new("/work/p10")));
        assert_eq!(rows.len(), MAX_RECENT_FOLDERS);
        assert!(rows.iter().any(|r| r.checked && r.path == Path::new("/work/p10")));
        let first = folder_menu_rows(&projects, Some(Path::new("/work/p0")));
        assert_eq!(first.len(), MAX_RECENT_FOLDERS);
        assert_eq!(first[0].path, PathBuf::from("/work/p0"));
    }

    #[test]
    fn no_projects_gives_no_rows() {
        assert!(folder_menu_rows(&[], None).is_empty());
    }
}
```

Add to `new_session.rs` a test module (the file has none yet):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_send_button_is_ready_with_text_or_an_attachment() {
        assert!(!send_ready("", 0));
        assert!(!send_ready("  \n ", 0));
        assert!(send_ready("hi", 0));
        assert!(send_ready("", 1));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Add `mod folder_menu;` and `pub use folder_menu::{folder_menu_rows, FolderMenuRow, MAX_RECENT_FOLDERS};` to `lib.rs` (and extend the `new_session` export in Step 3).
Run: `cargo test -p forge-features folder_menu new_session`
Expected: compile errors (functions not found).

- [ ] **Step 3: Implement**

Above the tests in `folder_menu.rs`:

```rust
use std::path::{Path, PathBuf};

use forge_domain::ProjectSummary;

use crate::attachments::folder_label;

/// How many folders the new-session folder menu lists (the desktop app lists eight).
pub const MAX_RECENT_FOLDERS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderMenuRow {
    pub label: String,
    pub path: PathBuf,
    pub checked: bool,
}

/// The rows of the folder menu: the projects in the order given, the first `MAX_RECENT_FOLDERS`; the
/// current folder takes the last place when it would not fit, so it is always listed and checked.
pub fn folder_menu_rows(projects: &[ProjectSummary], current: Option<&Path>) -> Vec<FolderMenuRow> {
    let row = |project: &ProjectSummary| FolderMenuRow {
        label: folder_label(&project.path),
        path: project.path.clone(),
        checked: current == Some(project.path.as_path()),
    };
    let mut rows: Vec<FolderMenuRow> = projects.iter().take(MAX_RECENT_FOLDERS).map(row).collect();
    if let Some(current_project) = projects.iter().skip(MAX_RECENT_FOLDERS).find(|project| current == Some(project.path.as_path())) {
        rows.pop();
        rows.push(row(current_project));
    }
    rows
}
```

In `new_session.rs`, replace the top of the file through the end of the component with the following (the pieces that stay the same: the probe effect, `submit`'s structure, `ComposerChips`, `CommandTextArea`). Imports:

```rust
use dioxus::prelude::*;
use forge_domain::PermissionMode;

use crate::command_box::CommandTextArea;
use crate::composer_state::{use_composer_state_provider, ComposerChips};
use crate::workspace::use_workspace;

/// What was chosen for the new session before it exists: `None` follows the saved default model,
/// `Some(None)` is the SDK's default, `Some(Some(id))` is that model.
pub type ModelChoice = Option<Option<String>>;

/// The mode and model picked on the new-session screen. Nothing is saved to settings.
#[derive(Clone, Copy)]
pub struct NewSessionDraft {
    pub mode: Signal<PermissionMode>,
    pub model: Signal<ModelChoice>,
}

pub fn use_new_session_draft() -> NewSessionDraft {
    use_context::<NewSessionDraft>()
}

/// Whether the send button is ready: there is text or something attached.
pub fn send_ready(text: &str, attachments: usize) -> bool {
    !text.trim().is_empty() || attachments > 0
}
```

The component:

```rust
/// The new-session panel: the folder and other chips (`chips`), the first message, and a row of controls
/// (`children`). The session is only created when that message is sent.
#[component]
pub fn NewSessionComposer(chips: Element, children: Element) -> Element {
    let workspace = use_workspace();
    let draft = use_context_provider(|| NewSessionDraft { mode: Signal::new(PermissionMode::Default), model: Signal::new(None) });
    let text = use_signal(String::new);
    let composer = use_composer_state_provider(text);
    let mut text = text;

    // The probe follows the chosen folder: it is replaced when the folder changes, kept when it does
    // not, and ended when this panel goes away (send, another session, the settings).
    use_effect(move || {
        let folder = workspace.chosen_new_folder();
        spawn(async move { workspace.probe_new_folder(folder) });
    });
    use_drop(move || workspace.probe_new_folder(None));

    let has_folder = !workspace.new_session_projects().is_empty();

    // `submit` is used by two handlers, so it may capture only `Copy` values (signals): it looks the
    // folder up again instead of capturing a string.
    let mut submit = move || {
        if !send_ready(&text.read(), composer.attachments().len()) {
            return;
        }
        let projects = workspace.new_session_projects();
        let project = match workspace.new_folder() {
            Some(path) => projects.iter().find(|project| project.path == path).cloned().or_else(|| projects.first().cloned()),
            None => projects.first().cloned(),
        };
        let Some(project) = project else {
            return;
        };
        let Some(outgoing) = composer.take_for_new_session(&text.read()) else {
            return;
        };
        workspace.start_new(
            project,
            *draft.mode.peek(),
            draft.model.peek().clone(),
            outgoing.text,
            outgoing.attachments,
            composer.take_folders(),
        );
        text.set(String::new());
    };
    let ready = send_ready(&text.read(), composer.attachments().len());

    rsx! {
        div { class: "new-session",
            if !has_folder {
                div { class: "banner info", "No existing project folder was found in your Claude Code history. Use “Open folder…” to choose one." }
            }
            div { class: "env-chips", {chips} }
            if let Some(message) = composer.error() {
                div { class: "banner error", onclick: move |_| composer.dismiss_error(), "{message}" }
            }
            ComposerChips {}
            div { class: "composer-box",
                CommandTextArea {
                    value: text(),
                    open_token: composer.popup_token(),
                    commands: workspace.new_commands(),
                    placeholder: "Describe a task or ask a question",
                    disabled: !has_folder,
                    oninput: move |value| text.set(value),
                    onsubmit: move |_| submit(),
                }
                button {
                    class: if ready { "send-icon ready" } else { "send-icon" },
                    r#type: "button",
                    title: "Send",
                    disabled: !has_folder || !ready,
                    onclick: move |_| submit(),
                    "↵"
                }
            }
            div { class: "composer-row", {children} }
        }
    }
}
```
(Before pasting, check the old component's `placeholder` string and keep it if tests or the harness script match on it: `grep -rn "What should Claude do" crates scripts`. If something matches, keep the old text.)

The unused imports to remove at the top of the old file: `forge_entities::model_label`, `forge_shared::{Button, ButtonKind, Select}`, `crate::attachments::folder_label`, `crate::permission_mode::{mode_options, parse_mode}`, `crate::settings::use_settings`.

In `lib.rs` change the `new_session` export to:
```rust
pub use new_session::{send_ready, use_new_session_draft, ModelChoice, NewSessionComposer, NewSessionDraft};
```

In `workspace.rs` change `start_new` (line 640): the parameter list becomes `(&self, project, mode, model: ModelChoice, text, attachments, folders)` (import `crate::new_session::ModelChoice`) and the model lookup inside the spawned future becomes:

```rust
            let model = match model {
                Some(chosen) => chosen,
                None => match services.settings.saved_model().await {
                    Ok(model) => model,
                    Err(error) => {
                        this.report(format!("failed to load the Forge config: {error}"));
                        return;
                    }
                },
            };
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p forge-features`
Expected: PASS. (`forge-widgets` does not build yet because `NewSessionPanel` still passes no `chips`; that is fixed in Task 8. Do not run the workspace build here.)

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: new-session draft for mode and model, send state, folder menu rows

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Pickers, folder chip and the new-session layout

**Files:**
- Modify: `crates/forge-widgets/src/composer_controls.rs`, `crates/forge-widgets/src/new_session_panel.rs`, `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 7's `NewSessionDraft`, `use_new_session_draft`, `folder_menu_rows`, `ModelChoice`; `mode_rows`, `mode_choice`, `model_menu_rows`, `use_settings`.
- Produces: `ModePickerView`, `ModelPickerView` (shared by the session and the new-session screens); `NewModePicker`, `NewModelPicker`, `FolderChip`, `AddFolderButton` in a new file `new_session_controls.rs`.

Checked by running the app (Task 9); no render tests exist for widgets.

- [ ] **Step 1: Let the pickers be shared, and give the picker a heading**

In `composer_controls.rs`:

- Make `PickerItem` and its fields `pub(crate)`, and `Picker` `pub(crate)`, and give `Picker` an optional heading. Replace the struct and the component signature:

```rust
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PickerItem {
    pub(crate) label: String,
    pub(crate) description: Option<String>,
    pub(crate) key: Option<usize>,
    pub(crate) checked: bool,
    pub(crate) divider_before: bool,
}
```
```rust
#[component]
pub(crate) fn Picker(
    title: String,
    current: String,
    items: Vec<PickerItem>,
    onpick: EventHandler<usize>,
    #[props(default)] heading: Option<String>,
) -> Element {
```
and in the menu body, replace `div { class: "picker-title", "{title}" }` with:

```rust
                    div { class: "picker-title", "{heading.clone().unwrap_or_else(|| title.clone())}" }
```

- Replace `ModePicker` and `ModelPicker` with a view each plus a thin wrapper. `ModePickerView` holds the whole body that `ModePicker` has now, with `current` and the apply call as inputs:

```rust
/// The permission mode picker. Bypass asks first. `onchange` gets the mode once it is confirmed.
#[component]
pub(crate) fn ModePickerView(current: PermissionMode, onchange: EventHandler<PermissionMode>) -> Element {
    // The row waiting for its confirmation.
    let mut confirming = use_signal(|| None::<usize>);
    let rows = mode_rows();
    let items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem {
            label: row.label.to_string(),
            description: Some(row.description.to_string()),
            key: row.key,
            checked: row.mode == current,
            divider_before: row.divider_before,
        })
        .collect();
    let label = rows.iter().find(|row| row.mode == current).map_or("Mode", |row| row.label).to_string();
    let rows_for_pick = rows.clone();
    let rows_for_enable = rows.clone();
    rsx! {
        Picker {
            title: "Mode",
            current: label,
            items,
            onpick: move |index: usize| {
                let mode = rows_for_pick[index].mode;
                match mode_choice(mode) {
                    ModeChoice::Apply => {
                        onchange.call(mode);
                        refocus_composer();
                    }
                    ModeChoice::AskFirst => confirming.set(Some(index)),
                }
            },
        }
        if let Some(index) = confirming() {
            div { class: "picker-confirm", role: "alertdialog",
                span { "Bypass permissions skips every permission check in this session." }
                Button {
                    label: "Cancel",
                    onclick: move |_| {
                        confirming.set(None);
                        refocus_composer();
                    },
                }
                Button {
                    label: "Enable",
                    kind: ButtonKind::Danger,
                    onclick: move |_| {
                        confirming.set(None);
                        onchange.call(rows_for_enable[index].mode);
                        refocus_composer();
                    },
                }
            }
        }
    }
}

/// The permission mode of the open session, changed from the composer.
#[component]
pub fn ModePicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().session.permission_mode;
    rsx! { ModePickerView { current, onchange: move |mode| workspace.set_permission_mode(id, mode) } }
}

/// The model picker. `current` is the model in force (`None` is the SDK's default); `onchange` gets
/// the row's value.
#[component]
pub(crate) fn ModelPickerView(current: Option<String>, onchange: EventHandler<Option<String>>) -> Element {
    let rows = model_menu_rows(current.as_deref());
    let items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem {
            label: row.label.clone(),
            description: None,
            key: row.key,
            checked: row.value == current,
            divider_before: row.divider_before,
        })
        .collect();
    let label = rows.iter().find(|row| row.value == current).map_or_else(|| "Model".to_string(), |row| row.label.clone());
    let values: Vec<Option<String>> = rows.iter().map(|row| row.value.clone()).collect();
    rsx! {
        Picker {
            title: "Model",
            current: label,
            items,
            onpick: move |index: usize| {
                onchange.call(values[index].clone());
                refocus_composer();
            },
        }
    }
}

/// The model of the open session, changed from the composer.
#[component]
pub fn ModelPicker(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let Some(view) = workspace.live_view(id) else { return rsx! {} };
    let current = view.read().model.clone();
    rsx! { ModelPickerView { current, onchange: move |model| workspace.set_model(id, model) } }
}
```
Add `use forge_domain::PermissionMode;` to the imports if the file does not have it.

- [ ] **Step 2: Check the session pickers still compile and behave**

Run: `cargo build -p forge-widgets 2>&1 | head -40`
Expected: the only errors left are in `new_session_panel.rs` (`chips` missing), which Step 3 fixes. Fix any error in `composer_controls.rs` now.

- [ ] **Step 3: The new-session controls**

Create `crates/forge-widgets/src/new_session_controls.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{folder_menu_rows, use_new_session_draft, use_settings, use_workspace};

use crate::composer_controls::{ModePickerView, ModelPickerView, Picker, PickerItem};

/// The folder chip: the folder's name, and a menu of the recent folders with "Open folder…" below.
#[component]
pub fn FolderChip() -> Element {
    let workspace = use_workspace();
    let projects = workspace.new_session_projects();
    let current = workspace.chosen_new_folder();
    let rows = folder_menu_rows(&projects, current.as_deref());
    let mut items: Vec<PickerItem> = rows
        .iter()
        .map(|row| PickerItem { label: row.label.clone(), description: None, key: None, checked: row.checked, divider_before: false })
        .collect();
    items.push(PickerItem { label: "Open folder…".to_string(), description: None, key: None, checked: false, divider_before: true });
    let current_label = rows.iter().find(|row| row.checked).map_or_else(|| "Choose a folder".to_string(), |row| row.label.clone());
    let paths: Vec<_> = rows.iter().map(|row| row.path.clone()).collect();
    rsx! {
        Picker {
            title: "Folder",
            heading: "Recent",
            current: current_label,
            items,
            onpick: move |index: usize| match paths.get(index) {
                Some(path) => workspace.set_new_folder(Some(path.clone())),
                None => workspace.open_folder(),
            },
        }
    }
}

/// A button that opens the native folder dialog, like "Open folder…" in the menu.
#[component]
pub fn AddFolderButton() -> Element {
    let workspace = use_workspace();
    rsx! {
        button {
            class: "chip-button",
            r#type: "button",
            title: "Open folder",
            disabled: workspace.picking_folder(),
            onclick: move |_| workspace.open_folder(),
            "📁+"
        }
    }
}

/// The mode for the session about to start.
#[component]
pub fn NewModePicker() -> Element {
    let mut draft = use_new_session_draft();
    let current = *draft.mode.read();
    rsx! { ModePickerView { current, onchange: move |mode| draft.mode.set(mode) } }
}

/// The model for the session about to start; until one is picked it is the saved default.
#[component]
pub fn NewModelPicker() -> Element {
    let mut draft = use_new_session_draft();
    let settings = use_settings();
    let current = match draft.model.read().clone() {
        Some(chosen) => chosen,
        None => settings.model(),
    };
    rsx! {
        div { class: "picker-end",
            ModelPickerView { current, onchange: move |model| draft.model.set(Some(model)) }
        }
    }
}
```
(`use_settings().model()` returns `Option<String>`, as `new_session.rs` used before.)

Wire the module in `lib.rs`: `mod new_session_controls;` and `pub use new_session_controls::{AddFolderButton, FolderChip, NewModePicker, NewModelPicker};` (the last four are used only by `new_session_panel.rs`, so `pub use` is optional; keep them `pub(crate)` if you prefer, then drop the `pub use`).

`new_session_panel.rs` becomes:

```rust
use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::new_session_controls::{AddFolderButton, FolderChip, NewModePicker, NewModelPicker};
use crate::plus_menu::PlusMenu;
use crate::usage_card::UsageCard;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    rsx! {
        div { class: "main-pane",
            div { class: "home-top",
                h1 { class: "greeting", "What's next?" }
                UsageCard {}
            }
            NewSessionComposer {
                chips: rsx! {
                    span { class: "chip", "Local" }
                    FolderChip {}
                    AddFolderButton {}
                },
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
                NewModePicker {}
                span { class: "spacer" }
                NewModelPicker {}
            }
        }
    }
}
```

Styles, appended to `theme.css`:

```css
/* New-session composer: chips above the box, the send icon inside it. */
.new-session { justify-content: flex-end; }
.env-chips .chip { padding: 4px 10px; font-size: 13px; }
.env-chips .picker-button { padding: 4px 10px; border: 1px solid var(--border); border-radius: 999px; background: var(--bg-hover); color: var(--fg); font-size: 13px; }
.chip-button { appearance: none; padding: 4px 8px; border: 1px solid var(--border); border-radius: 8px; background: transparent; color: var(--fg-muted); font: inherit; font-size: 12px; cursor: pointer; }
.chip-button:hover { background: var(--bg-hover); color: var(--fg); }
.composer-box { position: relative; }
.composer-box textarea { padding-right: 44px; }
.send-icon { position: absolute; right: 10px; bottom: 10px; width: 28px; height: 28px; border: none; border-radius: 50%; background: transparent; color: var(--fg-muted); font-size: 16px; cursor: default; }
.send-icon.ready { color: var(--fg); cursor: pointer; }
.send-icon.ready:hover { background: var(--bg-hover); }
.picker-end .picker-menu { left: auto; right: 0; }
.picker-heading { padding: 6px 8px 4px; font-size: 11px; color: var(--fg-muted); }
```
`.new-session { justify-content: flex-end; }` overrides the earlier `center` (it appears later in the file, so it wins) so the composer sits at the bottom under the card. Delete the now unused `.new-session .field` rules only if `grep -rn 'class: "field"' crates` shows nothing else uses them (Settings may).

- [ ] **Step 4: Build and test everything**

Run: `cargo build --workspace && cargo test --workspace`
Expected: builds; all tests PASS. Fix compile errors by following the compiler (typical: a missing import of `PermissionMode`, `Button`/`ButtonKind` in `composer_controls.rs`, or `forge_domain` in `forge-widgets/Cargo.toml`).

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: new-session screen with folder chip, mode and model pickers, send icon

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Check in the running app, time the first scan, and write it down

**Files:**
- Modify: `docs/superpowers/specs/2026-10-03-home-dashboard-design.md` (append "As built"), `docs/superpowers/roadmap/backlog.md` (PG-05, CMP-06, plan column), `docs/superpowers/roadmap/README.md` (plan table row 24)

- [ ] **Step 1: Launch with a fixture and look**

Read `/Users/james/.claude/projects/-Users-james-Developer-forge/memory/driving-the-forge-app-visually.md` and `scripts/harness/launch.sh`. Make a fixture `FORGE_CLAUDE_HOME` that has transcripts on at least five different days in two models (copy a few real transcripts, or write lines like the adapter tests do), then:

Run: `scripts/harness/launch.sh` (it rebuilds the fixture; if it wipes the one you made, put the transcripts into the fixture the script builds, or point `FORGE_CLAUDE_HOME` at yours).
Expected, checked with `screencapture`: greeting, the card with six tiles and the heatmap; the "Models" tab shows bars and a legend; "7d" shows seven columns; the folder chip opens a menu with "Recent", the folders and "Open folder…"; the mode and model pickers open upward; the send icon is grey when the box is empty and dark after typing. Save captures in the scratchpad, not in the repo.

- [ ] **Step 2: Choose a model and a mode, start a session, and check they apply**

Pick "Plan mode" and "Haiku 4.5" on the new-session screen, type a message in a throwaway folder (ask the person first if this would use their real account), send, and check the session header shows Plan mode and Haiku 4.5. Expected: yes. If the real SDK is not available, say so in the "As built" note instead of claiming it; `open_new` already keeps the mode it is given (`open_new_returns_an_empty_session_that_can_send` asserts `AcceptEdits`), so only the model path is unchecked by tests.

- [ ] **Step 3: Time the first scan on the real home**

Run the app without `FORGE_CLAUDE_HOME` (the real `~/.claude`), open the new-session screen, and time how long "Loading usage…" shows (a stopwatch is enough). Record the number. If it is over about 2 seconds, record that as a follow-up (a cache file on disk) and do not build it here. Also check the numbers against what the desktop app showed on 2026-10-03 (158 sessions, 14,710 messages, 2.4B tokens, peak 9 PM): they will not match exactly, but should be the same order of magnitude; if tokens are far off (for example 10x), re-check that cache tokens are counted once per message id.

- [ ] **Step 4: Write "As built" and update the roadmap**

Append to the spec an `## As built (2026-10-03)` section that says: what was built, what differs from the spec (the folder menu uses the generic `Picker`, with the "Recent" heading added to it; the model draft is `Option<Option<String>>` so "Default model" means the SDK default; render tests do not exist so widgets were checked by eye), the measured first-scan time, and what is left (a cache on disk if slow; hover tooltips on bars; "More models" submenu; mic; other environments).

In `backlog.md`: set PG-05's `Plan` to `Plan 24 (built)` and fill its Notes with one sentence on what exists and what was left; set CMP-06's `Plan` to `Plan 18 / 24 (built: folder menu, Local chip)`. In `README.md`'s order table add a row `| 24 | Home dashboard and new-session composer (built) | PG-05, CMP-06 | ... |` in place of the current row 24.

- [ ] **Step 5: Commit**

```bash
git add docs
git commit -m "docs: Plan 24 as built, roadmap updated

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-review notes

- **Spec coverage:** A (domain, port, adapter with cache, local time zone, total tokens with cache, user-message rows) is Tasks 1 to 4; B (card, tabs, ranges, tiles, heatmap independent of range, chart, legend, Loading/Failed/hidden states) is Tasks 5 and 6; C (greeting, chips, folder menu with Recent and Open folder, mode and model drafts into `open_new`, send icon state) is Tasks 7 and 8; the spec's "no render tests" wording is replaced by pure-function tests plus a by-eye check (Task 9), because the repo has no widget render tests. The 30 s timing question from "Open points" is Task 9 Step 3.
- **Types:** `StatsRange` (not `Range`) is used everywhere; `ModelChoice = Option<Option<String>>` is defined in Task 7 and used in Task 7's `start_new`, `NewSessionDraft` and Task 8's pickers; `chart_layout`/`legend_rows`/`overview_tiles` signatures match between Tasks 5 and 6; `Picker`'s new `heading` prop is defined in Task 8 Step 1 and used in Step 3.
- **Known loose ends the executor must resolve with the compiler:** SVG attribute spelling in Dioxus 0.7 (Task 6 note), `Option::is_none_or` availability (Task 1 note), `forge-domain` as a dependency of `forge-widgets` (Task 6 note), and the old placeholder text (Task 7 note).
