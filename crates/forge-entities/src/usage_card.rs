use forge_domain::{Day, RangeStats, StatsRange};

use crate::model_catalog::{model_label, KNOWN_MODELS};
use crate::usage_view::format_tokens;
use forge_shared::tr;

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
    if hour % 24 < 12 { tr!("{hour} AM", hour = clock) } else { tr!("{hour} PM", hour = clock) }
}

/// The six tiles of the overview, in the desktop app's order.
pub fn overview_tiles(stats: &RangeStats) -> Vec<Tile> {
    let dash = || "–".to_string();
    vec![
        Tile { label: tr!("Sessions"), value: with_commas(stats.session_count as u64) },
        Tile { label: tr!("Messages"), value: with_commas(stats.message_count) },
        Tile { label: tr!("Total tokens"), value: format_tokens(stats.total_tokens) },
        Tile { label: tr!("Active days"), value: with_commas(stats.active_days as u64) },
        Tile { label: tr!("Peak hour"), value: stats.peak_hour.map_or_else(dash, hour_label) },
        Tile { label: tr!("Favorite model"), value: stats.favourite_model.as_deref().map_or_else(dash, dashboard_model_label) },
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

/// The longest span the chart draws: a year. An older day (or a wrong timestamp) is left off rather
/// than shrinking every bar to nothing.
pub const MAX_CHART_DAYS: usize = 365;

/// The slots and bars of the per-day token chart. A week is always seven slots; the other ranges
/// run from the first day with use to `today` (at least `today`, at most `MAX_CHART_DAYS`).
pub fn chart_layout(stats: &RangeStats, range: StatsRange, today: Day) -> ChartLayout {
    let earliest = today.plus_days(1 - MAX_CHART_DAYS as i64);
    let first = match range {
        StatsRange::Last7Days => today.plus_days(-6),
        _ => stats.days.keys().next().copied().filter(|first| *first <= today).unwrap_or(today).max(earliest),
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
    tr!("{month} {day}", month = crate::month_short(day.month.clamp(1, 12)), day = day.day)
}

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
        assert_eq!(rows[0].share, "94.8%");
        assert_eq!(rows[1].share, "5.2%");
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

    #[test]
    fn one_very_old_day_does_not_stretch_the_all_time_chart_past_a_year() {
        let found = range_of(vec![row(Day::new(1970, 1, 1), 1, "a", 1, 5, 0), row(d(3), 1, "a", 1, 5, 0)], StatsRange::All, d(3));
        let layout = chart_layout(&found, StatsRange::All, d(3));
        assert_eq!(layout.days.len(), MAX_CHART_DAYS);
        assert_eq!(*layout.days.last().unwrap(), d(3));
        assert_eq!(layout.bars.len(), 1, "the day outside the span has no bar");
    }
}
