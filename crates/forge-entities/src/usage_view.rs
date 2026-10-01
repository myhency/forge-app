use forge_domain::{ContextUsage, Cost, PriceTable, SessionUsage, TokenUsage};

/// `999`, `1.2k`, `76.2k`, `3.4M`. Rounds to one decimal and moves up a unit instead of printing
/// `1000.0k`.
pub fn format_tokens(count: u64) -> String {
    if count < 1_000 {
        return count.to_string();
    }
    let thousands_tenths = (count as u128 * 10 + 500) / 1_000;
    if thousands_tenths < 10_000 {
        return format!("{}.{}k", thousands_tenths / 10, thousands_tenths % 10);
    }
    let millions_tenths = (count as u128 * 10 + 500_000) / 1_000_000;
    format!("{}.{}M", millions_tenths / 10, millions_tenths % 10)
}

/// `$0.00`, `<$0.01` for anything under a cent, otherwise dollars and cents.
pub fn format_cost(cost: Cost) -> String {
    let micro = cost.micro_dollars();
    if micro == 0 {
        return "$0.00".to_string();
    }
    if micro < 10_000 {
        return "<$0.01".to_string();
    }
    let cents = (micro + 5_000) / 10_000;
    format!("${}.{:02}", cents / 100, cents % 100)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RingLevel {
    Normal,
    Warn,
    Danger,
}

impl RingLevel {
    /// The CSS class the stylesheet colours.
    pub fn class(self) -> &'static str {
        match self {
            RingLevel::Normal => "normal",
            RingLevel::Warn => "warn",
            RingLevel::Danger => "danger",
        }
    }
}

pub fn ring_level(percent: u8) -> RingLevel {
    match percent {
        90.. => RingLevel::Danger,
        70..=89 => RingLevel::Warn,
        _ => RingLevel::Normal,
    }
}

// 2 * pi * 8, the circumference of the ring's circle (r = 8 in a 20 x 20 box), to two decimals.
const RING_CIRCUMFERENCE: f32 = 50.27;

/// The `stroke-dasharray` that draws `percent` of the ring: the filled part, then the gap.
pub fn ring_dash(percent: u8) -> String {
    let filled = RING_CIRCUMFERENCE * f32::from(percent.min(100)) / 100.0;
    format!("{:.2} {:.2}", filled, RING_CIRCUMFERENCE - filled)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextView {
    pub percent: u8,
    pub level: RingLevel,
    /// `38% · 76.2k / 200.0k tokens`
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelLine {
    pub model: String,
    /// `↑ 12.3k ↓ 4.1k`
    pub tokens: String,
    /// `$0.55`, or `—` for a model without a price.
    pub cost: String,
}

/// Everything the usage popover and ring show, already formatted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageView {
    /// `None` until the context window has been measured.
    pub context: Option<ContextView>,
    pub input: String,
    pub output: String,
    pub cache_read: String,
    pub cache_write: String,
    /// The number of web searches, shown only when there were any.
    pub web_searches: Option<String>,
    /// `None` when something was used but no model has a price.
    pub cost: Option<String>,
    /// `excludes <model>, …` when some models have no price.
    pub cost_note: Option<String>,
    pub models: Vec<ModelLine>,
    pub has_usage: bool,
}

impl UsageView {
    pub fn new(usage: &SessionUsage, context: Option<ContextUsage>, prices: &PriceTable) -> UsageView {
        let total = usage.total();
        let summary = usage.cost(prices);
        let cost = if usage.is_empty() {
            Some(format_cost(Cost(0)))
        } else if summary.priced_models == 0 {
            None
        } else {
            Some(format_cost(summary.total))
        };
        let cost_note = if summary.unpriced_models.is_empty() {
            None
        } else {
            Some(format!("excludes {}", summary.unpriced_models.join(", ")))
        };
        UsageView {
            context: context.map(|context| {
                let percent = context.percent();
                ContextView {
                    percent,
                    level: ring_level(percent),
                    label: format!(
                        "{percent}% · {} / {} tokens",
                        format_tokens(context.total_tokens),
                        format_tokens(context.max_tokens)
                    ),
                }
            }),
            input: format_tokens(total.input),
            output: format_tokens(total.output),
            cache_read: format_tokens(total.cache_read),
            cache_write: format_tokens(total.cache_write()),
            web_searches: (total.web_search_requests > 0).then(|| format_tokens(total.web_search_requests)),
            cost,
            cost_note,
            models: usage
                .models()
                .map(|(model, tokens)| model_line(model, tokens, prices))
                .collect(),
            has_usage: !usage.is_empty(),
        }
    }

    /// Prices from the built-in list (`PriceTable::list_prices`).
    pub fn for_session(usage: &SessionUsage, context: Option<ContextUsage>) -> UsageView {
        UsageView::new(usage, context, &PriceTable::list_prices())
    }
}

fn model_line(model: &str, tokens: &TokenUsage, prices: &PriceTable) -> ModelLine {
    ModelLine {
        model: model.to_string(),
        tokens: format!("↑ {} ↓ {}", format_tokens(tokens.input), format_tokens(tokens.output)),
        cost: prices
            .estimate_cost(model, tokens)
            .map(format_cost)
            .unwrap_or_else(|| "—".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::Rates;

    fn prices() -> PriceTable {
        PriceTable::new(vec![("claude-test", Rates::per_mtok(3.0, 15.0, 0.30, 3.75, 6.0))])
    }

    #[test]
    fn tokens_are_written_short() {
        assert_eq!(format_tokens(0), "0");
        assert_eq!(format_tokens(999), "999");
        assert_eq!(format_tokens(1_000), "1.0k");
        assert_eq!(format_tokens(1_234), "1.2k");
        assert_eq!(format_tokens(76_200), "76.2k");
        assert_eq!(format_tokens(999_949), "999.9k");
        assert_eq!(format_tokens(999_950), "1.0M", "never 1000.0k");
        assert_eq!(format_tokens(3_400_000), "3.4M");
        assert_eq!(format_tokens(4_000_000_000_000), "4000000.0M");
    }

    #[test]
    fn cost_is_written_in_dollars_and_cents() {
        assert_eq!(format_cost(Cost(0)), "$0.00");
        assert_eq!(format_cost(Cost(1)), "<$0.01");
        assert_eq!(format_cost(Cost(9_999)), "<$0.01");
        assert_eq!(format_cost(Cost(10_000)), "$0.01");
        assert_eq!(format_cost(Cost(550_000)), "$0.55");
        assert_eq!(format_cost(Cost(12_345_678)), "$12.35");
    }

    #[test]
    fn the_ring_changes_level_at_70_and_90() {
        assert_eq!(ring_level(0), RingLevel::Normal);
        assert_eq!(ring_level(69), RingLevel::Normal);
        assert_eq!(ring_level(70), RingLevel::Warn);
        assert_eq!(ring_level(89), RingLevel::Warn);
        assert_eq!(ring_level(90), RingLevel::Danger);
        assert_eq!(ring_level(100), RingLevel::Danger);
        assert_eq!((RingLevel::Normal.class(), RingLevel::Warn.class(), RingLevel::Danger.class()), ("normal", "warn", "danger"));
    }

    #[test]
    fn the_ring_arc_is_empty_at_0_full_at_100_and_grows_between() {
        assert_eq!(ring_dash(0), "0.00 50.27");
        assert_eq!(ring_dash(100), "50.27 0.00");
        let filled = |dash: String| dash.split(' ').next().unwrap().parse::<f32>().unwrap();
        assert!(filled(ring_dash(25)) < filled(ring_dash(26)));
    }

    #[test]
    fn a_session_with_nothing_used_shows_zero_cost_and_no_ring_value() {
        let view = UsageView::new(&SessionUsage::default(), None, &prices());
        assert!(!view.has_usage);
        assert_eq!(view.context, None);
        assert_eq!(view.cost.as_deref(), Some("$0.00"));
        assert_eq!(view.cost_note, None);
        assert!(view.models.is_empty());
        assert_eq!((view.input.as_str(), view.output.as_str()), ("0", "0"));
    }

    #[test]
    fn totals_cost_and_models_are_filled_in() {
        let mut usage = SessionUsage::default();
        usage.add(
            "claude-test",
            TokenUsage { input: 1_000_000, output: 100_000, cache_read: 2_000, cache_write_5m: 3_000, cache_write_1h: 4_000, web_search_requests: 0 },
        );
        let view = UsageView::new(&usage, Some(ContextUsage { total_tokens: 76_200, max_tokens: 200_000 }), &prices());

        assert!(view.has_usage);
        assert_eq!((view.input.as_str(), view.output.as_str()), ("1.0M", "100.0k"));
        assert_eq!((view.cache_read.as_str(), view.cache_write.as_str()), ("2.0k", "7.0k"));
        assert_eq!(view.cost.as_deref(), Some("$4.54"), "$3 + $1.50 + $0.0006 + $0.01125 + $0.024 = $4.53585");
        assert_eq!(view.cost_note, None);
        let context = view.context.unwrap();
        assert_eq!((context.percent, context.level), (38, RingLevel::Normal));
        assert_eq!(context.label, "38% · 76.2k / 200.0k tokens");
        assert_eq!(view.models, vec![ModelLine { model: "claude-test".to_string(), tokens: "↑ 1.0M ↓ 100.0k".to_string(), cost: "$4.54".to_string() }]);
    }

    #[test]
    fn an_unpriced_model_shows_a_dash_and_the_total_says_what_it_leaves_out() {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { input: 1_000_000, ..Default::default() });
        usage.add("mystery-model", TokenUsage { input: 500, output: 500, ..Default::default() });
        let view = UsageView::new(&usage, None, &prices());

        assert_eq!(view.cost.as_deref(), Some("$3.00"));
        assert_eq!(view.cost_note.as_deref(), Some("excludes mystery-model"));
        let unpriced = view.models.iter().find(|m| m.model == "mystery-model").unwrap();
        assert_eq!(unpriced.cost, "—");
    }

    #[test]
    fn when_no_model_is_priced_there_is_no_cost_at_all() {
        let mut usage = SessionUsage::default();
        usage.add("mystery-model", TokenUsage { input: 500, ..Default::default() });
        let view = UsageView::new(&usage, None, &prices());
        assert_eq!(view.cost, None);
        assert_eq!(view.cost_note.as_deref(), Some("excludes mystery-model"));
    }

    #[test]
    fn a_context_over_the_window_is_capped_and_a_missing_window_is_zero() {
        let over = UsageView::new(&SessionUsage::default(), Some(ContextUsage { total_tokens: 300, max_tokens: 200 }), &prices());
        assert_eq!(over.context.as_ref().map(|c| (c.percent, c.level)), Some((100, RingLevel::Danger)));
        let none = UsageView::new(&SessionUsage::default(), Some(ContextUsage { total_tokens: 5, max_tokens: 0 }), &prices());
        assert_eq!(none.context.map(|c| c.percent), Some(0));
    }

    #[test]
    fn for_session_uses_the_list_prices() {
        let mut usage = SessionUsage::default();
        usage.add("claude-haiku-4-5-20251001", TokenUsage { input: 1_000_000, ..Default::default() });
        assert_eq!(UsageView::for_session(&usage, None).cost.as_deref(), Some("$1.00"));
    }

    #[test]
    fn web_searches_are_listed_and_priced_at_a_cent_each() {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { input: 1_000_000, web_search_requests: 120, ..Default::default() });
        let view = UsageView::new(&usage, None, &prices());

        assert_eq!(view.web_searches.as_deref(), Some("120"));
        assert_eq!(view.cost.as_deref(), Some("$4.20"), "$3.00 of input + 120 searches at $0.01");
    }

    #[test]
    fn no_web_searches_means_no_web_search_row() {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { input: 5, ..Default::default() });
        assert_eq!(UsageView::new(&usage, None, &prices()).web_searches, None);
        assert_eq!(UsageView::new(&SessionUsage::default(), None, &prices()).web_searches, None);
    }

    #[test]
    fn a_session_that_only_searched_the_web_has_usage_and_a_cost() {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { web_search_requests: 50, ..Default::default() });
        let view = UsageView::new(&usage, None, &prices());
        assert!(view.has_usage);
        assert_eq!(view.web_searches.as_deref(), Some("50"));
        assert_eq!(view.cost.as_deref(), Some("$0.50"));
    }

    #[test]
    fn large_search_counts_are_written_short() {
        let mut usage = SessionUsage::default();
        usage.add("claude-test", TokenUsage { web_search_requests: 12_300, ..Default::default() });
        assert_eq!(UsageView::new(&usage, None, &prices()).web_searches.as_deref(), Some("12.3k"));
    }
}
