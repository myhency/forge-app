use std::collections::HashMap;

use forge_domain::{SessionUsage, TokenUsage};
use serde_json::Value;

/// Sums the `message.usage` of a session file. Claude Code writes one line per content block, so
/// one API message can appear several times; usage is kept once per `message.id`, the last line
/// winning.
#[derive(Default)]
pub(crate) struct UsageCollector {
    by_message: HashMap<String, (String, TokenUsage)>,
    unnamed: Vec<(String, TokenUsage)>,
}

impl UsageCollector {
    pub(crate) fn see(&mut self, line: &Value) {
        if line.get("type").and_then(Value::as_str) != Some("assistant") {
            return;
        }
        let flagged = |key: &str| line.get(key).and_then(Value::as_bool) == Some(true);
        if flagged("isSidechain") || flagged("isMeta") {
            return;
        }
        let Some(message) = line.get("message") else { return };
        let Some(model) = message.get("model").and_then(Value::as_str) else { return };
        // "<synthetic>" marks messages Claude Code wrote itself; they cost nothing.
        if model.is_empty() || model.starts_with('<') {
            return;
        }
        let Some(usage) = message.get("usage").and_then(token_usage) else { return };
        match message.get("id").and_then(Value::as_str) {
            Some(id) => {
                self.by_message.insert(id.to_string(), (model.to_string(), usage));
            }
            None => self.unnamed.push((model.to_string(), usage)),
        }
    }

    pub(crate) fn finish(self) -> SessionUsage {
        let mut total = SessionUsage::default();
        for (model, usage) in self.by_message.into_values().chain(self.unnamed) {
            total.add(&model, usage);
        }
        total
    }
}

fn number(value: Option<&Value>) -> u64 {
    value.and_then(Value::as_u64).unwrap_or(0)
}

fn token_usage(usage: &Value) -> Option<TokenUsage> {
    if !usage.is_object() {
        return None;
    }
    let plain_write = number(usage.get("cache_creation_input_tokens"));
    let (write_5m, write_1h) = match usage.get("cache_creation").filter(|split| split.is_object()) {
        Some(split) => {
            let (five, hour) = (
                number(split.get("ephemeral_5m_input_tokens")),
                number(split.get("ephemeral_1h_input_tokens")),
            );
            // A split that adds up to nothing while the plain total is not zero is not trusted.
            if five + hour == 0 { (plain_write, 0) } else { (five, hour) }
        }
        None => (plain_write, 0),
    };
    Some(TokenUsage {
        input: number(usage.get("input_tokens")),
        output: number(usage.get("output_tokens")),
        cache_read: number(usage.get("cache_read_input_tokens")),
        cache_write_5m: write_5m,
        cache_write_1h: write_1h,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn assistant(id: Option<&str>, model: &str, usage: Value) -> Value {
        let mut message = json!({"role": "assistant", "model": model, "content": [], "usage": usage});
        if let Some(id) = id {
            message["id"] = json!(id);
        }
        json!({"type": "assistant", "isSidechain": false, "message": message})
    }

    fn collect(lines: &[Value]) -> SessionUsage {
        let mut collector = UsageCollector::default();
        for line in lines {
            collector.see(line);
        }
        collector.finish()
    }

    fn total(usage: &SessionUsage) -> TokenUsage {
        usage.total()
    }

    #[test]
    fn one_message_repeated_on_several_lines_is_counted_once() {
        let u = json!({"input_tokens": 3, "output_tokens": 10, "cache_read_input_tokens": 5, "cache_creation_input_tokens": 0});
        let usage = collect(&[assistant(Some("m1"), "claude-test", u.clone()), assistant(Some("m1"), "claude-test", u)]);
        assert_eq!(total(&usage), TokenUsage { input: 3, output: 10, cache_read: 5, ..Default::default() });
    }

    #[test]
    fn the_last_line_of_a_repeated_message_wins() {
        let early = json!({"input_tokens": 3, "output_tokens": 1});
        let late = json!({"input_tokens": 3, "output_tokens": 40});
        let usage = collect(&[assistant(Some("m1"), "claude-test", early), assistant(Some("m1"), "claude-test", late)]);
        assert_eq!(total(&usage).output, 40);
    }

    #[test]
    fn different_messages_add_up_and_models_stay_apart() {
        let usage = collect(&[
            assistant(Some("m1"), "claude-a", json!({"input_tokens": 1, "output_tokens": 2})),
            assistant(Some("m2"), "claude-a", json!({"input_tokens": 10, "output_tokens": 20})),
            assistant(Some("m3"), "claude-b", json!({"input_tokens": 100, "output_tokens": 200})),
        ]);
        let models: Vec<_> = usage.models().map(|(m, u)| (m.to_string(), u.input)).collect();
        assert_eq!(models, vec![("claude-a".to_string(), 11), ("claude-b".to_string(), 100)]);
    }

    #[test]
    fn lines_without_a_message_id_count_once_each() {
        let u = json!({"input_tokens": 4, "output_tokens": 1});
        let usage = collect(&[assistant(None, "claude-test", u.clone()), assistant(None, "claude-test", u)]);
        assert_eq!(total(&usage).input, 8);
    }

    #[test]
    fn the_cache_write_split_is_read_when_present() {
        let u = json!({
            "input_tokens": 1, "output_tokens": 1, "cache_creation_input_tokens": 300,
            "cache_creation": {"ephemeral_5m_input_tokens": 100, "ephemeral_1h_input_tokens": 200}
        });
        let usage = collect(&[assistant(Some("m1"), "claude-test", u)]);
        assert_eq!((total(&usage).cache_write_5m, total(&usage).cache_write_1h), (100, 200));
    }

    #[test]
    fn a_cache_write_without_a_split_counts_as_5m() {
        let u = json!({"input_tokens": 1, "output_tokens": 1, "cache_creation_input_tokens": 300});
        let usage = collect(&[assistant(Some("m1"), "claude-test", u)]);
        assert_eq!((total(&usage).cache_write_5m, total(&usage).cache_write_1h), (300, 0));
    }

    #[test]
    fn a_split_that_sums_to_zero_falls_back_to_the_plain_total() {
        let u = json!({
            "cache_creation_input_tokens": 300,
            "cache_creation": {"ephemeral_5m_input_tokens": 0, "ephemeral_1h_input_tokens": 0}
        });
        let usage = collect(&[assistant(Some("m1"), "claude-test", u)]);
        assert_eq!(total(&usage).cache_write_5m, 300);
    }

    #[test]
    fn synthetic_empty_and_sidechain_lines_are_ignored() {
        let u = json!({"input_tokens": 9, "output_tokens": 9});
        let mut sidechain = assistant(Some("m3"), "claude-test", u.clone());
        sidechain["isSidechain"] = json!(true);
        let usage = collect(&[
            assistant(Some("m1"), "<synthetic>", u.clone()),
            assistant(Some("m2"), "", u.clone()),
            sidechain,
        ]);
        assert!(usage.is_empty());
    }

    #[test]
    fn lines_without_usable_usage_are_skipped_without_failing() {
        let usage = collect(&[
            json!({"type": "assistant", "message": {"id": "m1", "model": "claude-test"}}),
            json!({"type": "assistant", "message": {"id": "m2", "model": "claude-test", "usage": "lots"}}),
            json!({"type": "assistant", "message": {"id": "m3", "model": "claude-test", "usage": {"input_tokens": -4, "output_tokens": "x"}}}),
            json!({"type": "user", "message": {"role": "user", "content": "hi"}}),
            json!("not even an object"),
        ]);
        assert!(usage.is_empty());
    }

    #[test]
    fn a_file_with_no_assistant_lines_has_no_usage() {
        assert!(collect(&[json!({"type": "user", "message": {"role": "user", "content": "hi"}})]).is_empty());
    }
}
