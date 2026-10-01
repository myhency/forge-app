use std::collections::BTreeMap;
use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};

/// Tokens of one kind of work, split the way the API prices them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
}

impl TokenUsage {
    pub fn is_empty(&self) -> bool {
        *self == TokenUsage::default()
    }

    pub fn cache_write(&self) -> u64 {
        self.cache_write_5m + self.cache_write_1h
    }
}

impl Add for TokenUsage {
    type Output = TokenUsage;

    fn add(self, other: TokenUsage) -> TokenUsage {
        TokenUsage {
            input: self.input + other.input,
            output: self.output + other.output,
            cache_read: self.cache_read + other.cache_read,
            cache_write_5m: self.cache_write_5m + other.cache_write_5m,
            cache_write_1h: self.cache_write_1h + other.cache_write_1h,
        }
    }
}

impl AddAssign for TokenUsage {
    fn add_assign(&mut self, other: TokenUsage) {
        *self = *self + other;
    }
}

/// An amount of money in micro-dollars (1 000 000 = $1), so sums stay exact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cost(pub u64);

impl Cost {
    pub fn micro_dollars(self) -> u64 {
        self.0
    }
}

/// What a session has used so far, per model.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SessionUsage {
    by_model: BTreeMap<String, TokenUsage>,
}

impl SessionUsage {
    pub fn add(&mut self, model: &str, usage: TokenUsage) {
        if usage.is_empty() {
            return;
        }
        *self.by_model.entry(model.to_string()).or_default() += usage;
    }

    pub fn total(&self) -> TokenUsage {
        self.by_model.values().fold(TokenUsage::default(), |sum, usage| sum + *usage)
    }

    pub fn models(&self) -> impl Iterator<Item = (&str, &TokenUsage)> {
        self.by_model.iter().map(|(model, usage)| (model.as_str(), usage))
    }

    pub fn is_empty(&self) -> bool {
        self.by_model.is_empty()
    }

    /// The cost of every model the table prices, and the names of the ones it does not.
    pub fn cost(&self, prices: &PriceTable) -> CostSummary {
        let mut summary = CostSummary { total: Cost(0), priced_models: 0, unpriced_models: Vec::new() };
        for (model, usage) in &self.by_model {
            match prices.estimate_cost(model, usage) {
                Some(cost) => {
                    summary.total = Cost(summary.total.0 + cost.0);
                    summary.priced_models += 1;
                }
                None => summary.unpriced_models.push(model.clone()),
            }
        }
        summary
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CostSummary {
    pub total: Cost,
    pub priced_models: usize,
    pub unpriced_models: Vec<String>,
}

/// How much of the context window the conversation fills right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextUsage {
    pub total_tokens: u64,
    pub max_tokens: u64,
}

impl ContextUsage {
    /// Rounded to a whole percent and capped at 100; 0 when the window size is unknown.
    pub fn percent(&self) -> u8 {
        if self.max_tokens == 0 {
            return 0;
        }
        let percent = (self.total_tokens as u128 * 100 + self.max_tokens as u128 / 2) / self.max_tokens as u128;
        percent.min(100) as u8
    }
}

/// Prices in micro-dollars per million tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    input: u64,
    output: u64,
    cache_read: u64,
    cache_write_5m: u64,
    cache_write_1h: u64,
}

impl Rates {
    /// From the usual way prices are written: dollars per million tokens.
    pub fn per_mtok(input: f64, output: f64, cache_read: f64, cache_write_5m: f64, cache_write_1h: f64) -> Rates {
        let micro = |dollars: f64| (dollars * 1_000_000.0).round() as u64;
        Rates {
            input: micro(input),
            output: micro(output),
            cache_read: micro(cache_read),
            cache_write_5m: micro(cache_write_5m),
            cache_write_1h: micro(cache_write_1h),
        }
    }

    fn cost(&self, usage: &TokenUsage) -> Cost {
        let scaled = usage.input as u128 * self.input as u128
            + usage.output as u128 * self.output as u128
            + usage.cache_read as u128 * self.cache_read as u128
            + usage.cache_write_5m as u128 * self.cache_write_5m as u128
            + usage.cache_write_1h as u128 * self.cache_write_1h as u128;
        Cost(((scaled + 500_000) / 1_000_000) as u64)
    }
}

/// Model-id prefixes and what they cost. The longest matching prefix wins, so a dated id such as
/// `claude-haiku-4-5-20251001` finds the `claude-haiku-4-5` row.
#[derive(Debug, Clone)]
pub struct PriceTable {
    rows: Vec<(&'static str, Rates)>,
}

impl PriceTable {
    pub fn new(rows: Vec<(&'static str, Rates)>) -> PriceTable {
        PriceTable { rows }
    }

    pub fn rates_for(&self, model: &str) -> Option<&Rates> {
        self.rows
            .iter()
            .filter(|(prefix, _)| model.starts_with(prefix))
            .max_by_key(|(prefix, _)| prefix.len())
            .map(|(_, rates)| rates)
    }

    pub fn estimate_cost(&self, model: &str, usage: &TokenUsage) -> Option<Cost> {
        self.rates_for(model).map(|rates| rates.cost(usage))
    }

    /// Anthropic's published list prices, read from
    /// https://platform.claude.com/docs/en/about-claude/pricing on 2026-10-01 (dollars per million
    /// tokens: input, output, cache read, 5-minute cache write, 1-hour cache write). Data-residency
    /// (1.1x), fast-mode and batch rates are not modelled. This is the one place to edit when
    /// prices or models change.
    pub fn list_prices() -> PriceTable {
        PriceTable::new(vec![
            ("claude-fable-5-1", Rates::per_mtok(10.0, 50.0, 0.25, 12.5, 20.0)),
            ("claude-fable-5", Rates::per_mtok(10.0, 50.0, 1.0, 12.5, 20.0)),
            ("claude-opus-5-5", Rates::per_mtok(4.0, 20.0, 0.20, 5.0, 8.0)),
            ("claude-opus-5", Rates::per_mtok(5.0, 25.0, 0.50, 6.25, 10.0)),
            ("claude-opus-4-8", Rates::per_mtok(5.0, 25.0, 0.50, 6.25, 10.0)),
            ("claude-opus-4-7", Rates::per_mtok(5.0, 25.0, 0.50, 6.25, 10.0)),
            ("claude-opus-4-6", Rates::per_mtok(5.0, 25.0, 0.50, 6.25, 10.0)),
            ("claude-opus-4-5", Rates::per_mtok(5.0, 25.0, 0.50, 6.25, 10.0)),
            ("claude-sonnet-5-5", Rates::per_mtok(2.0, 10.0, 0.20, 2.5, 4.0)),
            ("claude-sonnet-5", Rates::per_mtok(2.0, 10.0, 0.20, 2.5, 4.0)),
            ("claude-sonnet-4-6", Rates::per_mtok(3.0, 15.0, 0.30, 3.75, 6.0)),
            ("claude-sonnet-4-5", Rates::per_mtok(3.0, 15.0, 0.30, 3.75, 6.0)),
            ("claude-haiku-4-5", Rates::per_mtok(1.0, 5.0, 0.10, 1.25, 2.0)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> PriceTable {
        PriceTable::new(vec![
            ("claude-test", Rates::per_mtok(3.0, 15.0, 0.30, 3.75, 6.0)),
            ("claude-test-big", Rates::per_mtok(10.0, 50.0, 1.0, 12.5, 20.0)),
        ])
    }

    fn usage(input: u64, output: u64) -> TokenUsage {
        TokenUsage { input, output, ..Default::default() }
    }

    #[test]
    fn input_and_output_are_priced_per_million_tokens() {
        let cost = table().estimate_cost("claude-test", &usage(1_000_000, 1_000_000));
        assert_eq!(cost, Some(Cost(18_000_000)), "$3 + $15");
    }

    #[test]
    fn cache_reads_and_the_two_cache_writes_have_their_own_rates() {
        let all = TokenUsage { cache_read: 1_000_000, cache_write_5m: 1_000_000, cache_write_1h: 1_000_000, ..Default::default() };
        assert_eq!(table().estimate_cost("claude-test", &all), Some(Cost(300_000 + 3_750_000 + 6_000_000)));
    }

    #[test]
    fn a_model_id_with_a_date_suffix_matches_its_prefix() {
        assert_eq!(table().estimate_cost("claude-test-20260101", &usage(1_000_000, 0)), Some(Cost(3_000_000)));
    }

    #[test]
    fn the_longest_matching_prefix_wins() {
        assert_eq!(table().estimate_cost("claude-test-big-20260101", &usage(1_000_000, 0)), Some(Cost(10_000_000)));
    }

    #[test]
    fn an_unknown_model_has_no_cost() {
        assert_eq!(table().estimate_cost("some-other-model", &usage(1_000_000, 0)), None);
    }

    #[test]
    fn small_amounts_round_to_the_nearest_micro_dollar() {
        let read = |n| TokenUsage { cache_read: n, ..Default::default() };
        assert_eq!(table().estimate_cost("claude-test", &read(1)), Some(Cost(0)), "0.3 micro-dollars");
        assert_eq!(table().estimate_cost("claude-test", &read(2)), Some(Cost(1)), "0.6 micro-dollars");
    }

    #[test]
    fn huge_counts_do_not_overflow() {
        let huge = TokenUsage { cache_read: 4_000_000_000_000, ..Default::default() };
        assert_eq!(table().estimate_cost("claude-test", &huge), Some(Cost(1_200_000_000_000)));
    }

    #[test]
    fn adding_usage_keeps_one_row_per_model_and_sums_the_total() {
        let mut session = SessionUsage::default();
        session.add("claude-test", usage(10, 1));
        session.add("claude-test", usage(5, 2));
        session.add("other", usage(100, 0));
        assert_eq!(session.models().count(), 2);
        assert_eq!(session.total(), usage(115, 3));
    }

    #[test]
    fn adding_an_empty_usage_creates_no_row() {
        let mut session = SessionUsage::default();
        session.add("claude-test", TokenUsage::default());
        assert!(session.is_empty());
        assert_eq!(session.models().count(), 0);
    }

    #[test]
    fn the_cost_adds_priced_models_and_names_the_others() {
        let mut session = SessionUsage::default();
        session.add("claude-test", usage(1_000_000, 0));
        session.add("mystery-model", usage(5, 5));
        let summary = session.cost(&table());
        assert_eq!(summary.total, Cost(3_000_000));
        assert_eq!(summary.priced_models, 1);
        assert_eq!(summary.unpriced_models, vec!["mystery-model".to_string()]);
    }

    #[test]
    fn context_percent_is_rounded_and_capped() {
        assert_eq!(ContextUsage { total_tokens: 76_200, max_tokens: 200_000 }.percent(), 38);
        assert_eq!(ContextUsage { total_tokens: 1, max_tokens: 200_000 }.percent(), 0);
        assert_eq!(ContextUsage { total_tokens: 250_000, max_tokens: 200_000 }.percent(), 100);
        assert_eq!(ContextUsage { total_tokens: 10, max_tokens: 0 }.percent(), 0, "no division by zero");
    }

    #[test]
    fn the_list_prices_cover_the_models_forge_offers() {
        let prices = PriceTable::list_prices();
        for id in ["claude-fable-5-1", "claude-opus-5-5", "claude-sonnet-5-5", "claude-haiku-4-5-20251001"] {
            assert!(prices.rates_for(id).is_some(), "no price for {id}");
        }
    }

    #[test]
    fn token_usage_reads_missing_json_fields_as_zero() {
        let parsed: TokenUsage = serde_json::from_str(r#"{"input": 7, "cache_write_1h": 2}"#).unwrap();
        assert_eq!(parsed, TokenUsage { input: 7, cache_write_1h: 2, ..Default::default() });
    }
}
