use std::sync::Arc;

use dioxus::prelude::*;
use forge_domain::{Day, RangeStats, StatsRange, UsageStats};
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

fn chart(shown: &RangeStats, range: StatsRange, today: Day) -> Element {
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
