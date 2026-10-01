# Forge Session Usage Display Implementation Plan (Plan 10)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each open session shows a context-usage ring next to the message box. Clicking it opens a popover with the session's token totals, an estimated cost and a per-model list. Totals include usage from before the session was opened (restored from the session file).

**Architecture:** `forge-domain` gets pure usage types, a price table and the cost function (one cost path for live and restored usage). `forge-adapter-claude-fs` sums `message.usage` per `message.id` when it loads a session. The sidecar's `turn_complete` carries per-model token deltas and a new `context_usage` event reports `Query.getContextUsage()`. `forge-application` adds the deltas to `Session.usage` and keeps the last context value in `LiveSessionView`. `forge-entities` formats everything into a `UsageView`; `forge-widgets` draws the ring and popover.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, Node 22 + TypeScript 7 + vitest 5 (sidecar), `@anthropic-ai/claude-agent-sdk` 0.3.283, existing `serde`, `serde_json`, `tokio`, `mockall`.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §20 (§5 for the sidecar protocol, §18/§19 for the as-built style).

## Global Constraints

- Crate graph: `forge-domain` has no I/O and no dependency on other Forge crates. `forge-entities` depends on `forge-domain` and must **not** depend on `forge-application` (spec §6/§15.6). No UI crate imports an adapter. Only `forge-app` names concrete adapters.
- Cost is computed in Rust from token counts and one price table, for live turns and restored history alike. The SDK's `total_cost_usd` is **not** used (spec §20.1).
- A model that is not in the price table has no cost (`None`, shown as `—`); its tokens are still shown (spec §20.1).
- Cost is integer micro-dollars (`Cost(u64)`); no floats in sums (spec §20.2).
- Restore keeps usage **once per `message.id`, the last line wins**; lines without an id count once each; unreadable usage is skipped, never an error; `<synthetic>`/empty models and sidechain lines are ignored (spec §20.3).
- A cache write without a 5m/1h split counts as 5m (spec §20.2).
- `turn_complete` carries only what the turn **added**; the old `input_tokens`/`output_tokens` fields are removed (spec §20.4).
- `getContextUsage()` that throws or takes over 3 s sends nothing; the turn is not affected (spec §20.4).
- Ring colour levels: normal below 70 %, warn from 70, danger from 90; the percentage is also tooltip text (spec §20.6).
- UI text is English. The cost line says it is a list-price estimate that may differ from the bill (spec §20.6).
- Colors only through CSS variables from `forge-shared/src/theme.css`; nothing hard-coded outside `:root` and the dark `@media` block (spec §16.6).
- Out of scope: plan usage, usage in the sidebar, sub-agent files in the restored sum, user price overrides, budgets, contracted rates, history charts, slash commands (spec §20.1).
- Pinned: Dioxus `=0.7.10`; Agent SDK `0.3.283`. New Rust dependencies: none.
- Every commit message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (pass it as a second `-m`).
- Test commands: `cargo test -p <crate>`; sidecar `cd sidecar && npm test`; full check `cargo test --workspace` (the `block v0.1.6` future-incompat notice is expected).

## Review Focus

Most likely first. Each line is pinned by a test in the task named in brackets.

1. **A session file that repeats one `message.id` on several lines** must not double the totals. [Task 4]
2. **A session with no assistant lines, or lines without usage** shows `$0.00` and an empty ring, not garbage or a panic. [Task 4, Task 7]
3. **A model that is not priced** (custom `base_url`, a model id the table does not know) keeps its tokens, shows `—`, and the total says what it excludes. [Task 2, Task 7]
4. **`getContextUsage()` that fails or hangs** leaves the turn and the totals alone. [Task 5]
5. **Percentages above 100 or a `max_tokens` of 0** are clamped and never divide by zero. [Task 2, Task 7]
6. **A resumed session's first turn** must not add the session's history a second time (depends on the spike's finding 1). [Task 1, Task 6]
7. **Very large counts** (billions of cache-read tokens) neither overflow nor render as `1000.0k`. [Task 2, Task 7]

---

## File structure

| File | Responsibility |
|---|---|
| `crates/forge-domain/src/usage.rs` (new) | `TokenUsage`, `SessionUsage`, `ContextUsage`, `Cost`, `CostSummary`, `Rates`, `PriceTable` and its list prices |
| `crates/forge-domain/src/session.rs` | `Session.usage` |
| `crates/forge-adapter-claude-fs/src/usage.rs` (new) | `UsageCollector`: per-`message.id` summing of a session file |
| `crates/forge-adapter-claude-fs/src/sessions.rs` | calls the collector from `load_session` |
| `sidecar/src/usage.ts` (new) | `toWireUsage`, `UsageTracker` |
| `sidecar/src/protocol.ts`, `session.ts` | new `turn_complete` shape, `context_usage` event |
| `crates/forge-ports/src/sidecar_port.rs` | `SidecarEvent::TurnComplete { usage }`, `SidecarEvent::ContextUsage` |
| `crates/forge-application/src/{event,live_session,live_session_actor}.rs` | totals, `LiveSessionView.context` |
| `crates/forge-entities/src/usage_view.rs` (new) | `UsageView`, `format_tokens`, `format_cost`, `ring_level`, `ring_dash` |
| `crates/forge-features/src/send_message.rs` | a `children` slot at the start of the composer row |
| `crates/forge-widgets/src/usage_ring.rs` (new) | `UsageRing` (ring + popover) |
| `crates/forge-widgets/src/chat_window.rs` | places the ring |
| `crates/forge-shared/src/theme.css`, `theme.rs` | `.usage-*` styles and their tests |

---

### Task 1: Spike — what the SDK and the session file really report

**Files:**
- Create (not committed): `sidecar/spike-usage.mjs`
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (new §20.10 "Spike findings")
- Modify (only if the findings say so): this plan's Task 5 and Task 6

This task answers spec §20.7. It needs a working Claude login or `ANTHROPIC_API_KEY` in the shell. Nothing built here is kept except the written findings.

- [ ] **Step 1: Install the sidecar's pinned dependencies and check the types**

Run:
```bash
cd sidecar && npm ci && npm run typecheck && npm test
```
Expected: install succeeds, typecheck and all vitest tests pass (this is the baseline).

Then read the real types:
```bash
grep -n "getContextUsage\|SDKControlGetContextUsageResponse = \|modelUsage\|total_cost_usd\|type ModelUsage" node_modules/@anthropic-ai/claude-agent-sdk/sdk.d.ts | head -30
```
Write down the exact field names of `ModelUsage` and the `getContextUsage` response. Task 5 assumes `modelUsage[model].{inputTokens,outputTokens,cacheReadInputTokens,cacheCreationInputTokens}` and `getContextUsage() → {totalTokens,maxTokens}`; if the real names differ, fix them in Task 5's code before starting it.

- [ ] **Step 2: Write the throwaway driver**

Create `sidecar/spike-usage.mjs` (do not commit it; Step 7 deletes it). It drives the real `SidecarSession` against the real SDK and prints the raw `result` of every turn:

```js
import { query } from "@anthropic-ai/claude-agent-sdk";
import { SidecarSession } from "./dist/session.js";

// Logs the raw `result` of every turn, then hands the message on unchanged.
function spyQuery(args) {
  const q = query(args);
  const spied = (async function* () {
    for await (const message of q) {
      if (message.type === "result") {
        console.log("RESULT", JSON.stringify({
          usage: message.usage,
          modelUsage: message.modelUsage,
          total_cost_usd: message.total_cost_usd,
        }));
      }
      yield message;
    }
  })();
  return Object.assign(spied, {
    interrupt: () => q.interrupt(),
    getContextUsage: async (...a) => {
      const started = Date.now();
      const usage = await q.getContextUsage(...a);
      console.log("CONTEXT", JSON.stringify({ ms: Date.now() - started, totalTokens: usage.totalTokens, maxTokens: usage.maxTokens, percentage: usage.percentage, model: usage.model }));
      return usage;
    },
  });
}

// usage: node spike-usage.mjs <cwd> <resume-session-id or ""> [prompt ...]
const cwd = process.argv[2];
const resume = process.argv[3] || null;
const prompts = process.argv.slice(4);
let sessionId = null;
let turns = 0;
let finish;
const finished = new Promise((resolve) => (finish = resolve));

const session = new SidecarSession(spyQuery, (event) => {
  if (event.type === "session_started") sessionId = event.session_id;
  if (event.type === "turn_complete") {
    turns += 1;
    void session.activeQuery?.getContextUsage?.().catch((e) => console.log("CONTEXT_ERROR", String(e)));
    if (turns < prompts.length) {
      void session.handle({ type: "send_message", id: String(turns), text: prompts[turns] });
    } else {
      setTimeout(() => finish(), 3000); // leave time for the context call
    }
  }
  if (event.type === "error") { console.log("ERROR", event.message); finish(); }
});

// `handle(start_session)` runs until the query ends, so it is not awaited.
void session.handle({
  type: "start_session", id: "0", cwd, resume_session_id: resume, permission_mode: "default",
  model: null, skills: {}, plugins: [], mcp_servers: {},
});

if (prompts.length > 0) {
  setTimeout(() => void session.handle({ type: "send_message", id: "0", text: prompts[0] }), 1000);
} else {
  // No prompt: ask for the context of a resumed session that has had no turn yet.
  setTimeout(() => {
    const started = Date.now();
    Promise.race([
      session.activeQuery?.getContextUsage?.(),
      new Promise((r) => setTimeout(() => r("TIMEOUT after 5 s"), 5000)),
    ]).then((r) => { console.log("CONTEXT_BEFORE_FIRST_TURN", typeof r === "string" ? r : JSON.stringify(r), Date.now() - started, "ms"); finish(); })
      .catch((e) => { console.log("CONTEXT_BEFORE_FIRST_TURN_ERROR", String(e)); finish(); });
  }, 2000);
}

await Promise.race([finished, new Promise((r) => setTimeout(r, 120000))]);
console.log("SESSION_ID", sessionId);
process.exit(0);
```

- [ ] **Step 3: Question 1 — per turn or running totals? (and Question 4 — model ids)**

Run, with an empty scratch folder as the project:
```bash
mkdir -p /tmp/forge-spike && cd /Users/james/orca/workspaces/forge/plan-10/sidecar && npm run build \
  && node spike-usage.mjs /tmp/forge-spike "" "Write about 200 words about cats." "Reply with the single word: ok."
```
Read the two `RESULT` lines. Decide:
- **Per turn:** the second `outputTokens` is far smaller than the first (a one-word reply versus 200 words).
- **Running total:** the second `outputTokens` is greater than or equal to the first.

Also record the model ids that are the keys of `modelUsage`, and whether the cache-write field has any 5m/1h split. Note the `SESSION_ID` line.

- [ ] **Step 4: Question 2 — does `getContextUsage()` work, how fast, and before any turn?**

From the Step 3 run, read the `CONTEXT` lines (one per finished turn): record the latency in ms and the values (`totalTokens`, `maxTokens`, `percentage`). A `CONTEXT_ERROR` line means the call fails on a live streaming query; record its text.

Then resume that session without sending anything:
```bash
node spike-usage.mjs /tmp/forge-spike <SESSION_ID>
```
Record the `CONTEXT_BEFORE_FIRST_TURN` line: a value, a timeout or an error. The plan assumes **no value** (the sidecar only starts on the first send, so the ring shows `—` until the first turn finishes); a value does not change the plan but is worth a line in §20.10.

Finally resume and send one message, and compare the first `RESULT` of the resumed run with the earlier run:
```bash
node spike-usage.mjs /tmp/forge-spike <SESSION_ID> "Reply with the single word: again."
```
- If its `inputTokens` / `outputTokens` are about one turn's worth, resume does **not** carry history totals (good).
- If they include the earlier turns, running totals restart from the restored history. **Stop here:** Task 5 and Task 6 need a baseline step (Review Focus 6). Revise them, and tell the user, before continuing.

- [ ] **Step 5: Question 3 — does summing the session file match the SDK?**

```bash
python3 - <<'EOF'
import json, glob, os
path = glob.glob(os.path.expanduser("~/.claude/projects/*forge-spike*/<SESSION_ID>.jsonl"))[0]
by_id = {}
for line in open(path):
    d = json.loads(line)
    m = d.get("message")
    if d.get("type") == "assistant" and isinstance(m, dict) and m.get("usage") and m.get("id"):
        u = m["usage"]; cc = u.get("cache_creation") or {}
        by_id[m["id"]] = (m["model"], u.get("input_tokens", 0), u.get("output_tokens", 0),
                          u.get("cache_read_input_tokens", 0), cc.get("ephemeral_5m_input_tokens"), cc.get("ephemeral_1h_input_tokens"), u.get("cache_creation_input_tokens", 0))
for k, v in by_id.items(): print(k, v)
print("input", sum(v[1] for v in by_id.values()), "output", sum(v[2] for v in by_id.values()),
      "cache_read", sum(v[3] for v in by_id.values()), "cache_creation", sum(v[6] for v in by_id.values()))
EOF
```
(Replace `<SESSION_ID>` with the real id.) Compare the four sums with the sum of the SDK's per-turn `RESULT` values across all three runs (the earlier two turns, the resumed turn). They should match exactly for `output_tokens` and `input_tokens`. Record any difference and its likely cause. Also record whether `ephemeral_1h_input_tokens` is non-zero in your files (a subscription login uses 1 h cache writes).

- [ ] **Step 6: Write the findings and settle the two switches**

Append a section `### 20.10 Spike findings` to the spec, answering the four questions with the real numbers (one short paragraph each, plus the exact `ModelUsage` / `getContextUsage` field names and the observed latency).

Then fix the two values Task 5 uses:
- `RESULT_USAGE_IS_CUMULATIVE` (`true` if finding 1 says running totals, else `false`).
- If the SDK's `modelUsage` has no 5m/1h split but your session files show non-zero `ephemeral_1h_input_tokens`, note it in §20.10 as a known gap: live cache writes are priced as 5m, restored ones are split, so a resume can move the cost slightly. Do not change the design for it.

- [ ] **Step 7: Clean up and commit the findings**

```bash
rm sidecar/spike-usage.mjs && rm -rf sidecar/dist /tmp/forge-spike
git status --short
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record the Plan 10 spike findings (usage and context reporting)" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
Expected: `git status` shows only the spec change before the commit (`sidecar/node_modules` and `dist` are ignored; if `git status` lists them, stop and check `.gitignore`).

---

### Task 2: `forge-domain` usage types, cost and price table

**Files:**
- Create: `crates/forge-domain/src/usage.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces (later tasks rely on these exact names):
  - `TokenUsage { input, output, cache_read, cache_write_5m, cache_write_1h: u64 }` — `Copy + Default + Serialize + Deserialize` (`#[serde(default)]`), `+`, `+=`, `is_empty()`, `cache_write() -> u64`.
  - `SessionUsage` — `Default + Clone + PartialEq + Eq + Debug`; `add(&mut self, model: &str, usage: TokenUsage)`, `total() -> TokenUsage`, `models() -> impl Iterator<Item = (&str, &TokenUsage)>`, `is_empty() -> bool`, `cost(&self, &PriceTable) -> CostSummary`.
  - `Cost(pub u64)` micro-dollars (`Copy + Default + Ord`), `Cost::micro_dollars(self) -> u64`.
  - `CostSummary { total: Cost, priced_models: usize, unpriced_models: Vec<String> }`.
  - `Rates::per_mtok(input, output, cache_read, cache_write_5m, cache_write_1h: f64) -> Rates` (dollars per million tokens).
  - `PriceTable::new(Vec<(&'static str, Rates)>)`, `rates_for(&self, &str) -> Option<&Rates>`, `estimate_cost(&self, &str, &TokenUsage) -> Option<Cost>`, `PriceTable::list_prices() -> PriceTable`.
  - `ContextUsage { total_tokens: u64, max_tokens: u64 }` (`Copy`), `percent(&self) -> u8` (rounded, capped at 100, 0 when `max_tokens` is 0).

- [ ] **Step 1: Record the baseline**

Run: `cargo test --workspace 2>&1 | grep "test result" | awk '{s+=$4} END {print s}'`
Note the number as N0 (used again in Task 9).

- [ ] **Step 2: Write the failing tests**

Create `crates/forge-domain/src/usage.rs` with only the test module first:

```rust
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
```

Add to `crates/forge-domain/src/lib.rs`:
```rust
mod usage;
```
and extend the exports:
```rust
pub use usage::{ContextUsage, Cost, CostSummary, PriceTable, Rates, SessionUsage, TokenUsage};
```

- [ ] **Step 3: Run the tests to verify they fail**

Run: `cargo test -p forge-domain usage`
Expected: FAIL to compile — `cannot find type PriceTable` (and the others).

- [ ] **Step 4: Write the implementation**

Put this **above** the `#[cfg(test)]` module in `usage.rs`:

```rust
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
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p forge-domain`
Expected: PASS (all `usage::tests` plus the crate's existing tests).

- [ ] **Step 6: Commit**

```bash
git add crates/forge-domain/src/usage.rs crates/forge-domain/src/lib.rs
git commit -m "feat: usage types, cost in micro-dollars and a list-price table in forge-domain" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: `Session` carries its usage

**Files:**
- Modify: `crates/forge-domain/src/session.rs`
- Modify (add `usage: SessionUsage::default(),` to each `Session { … }` literal): `crates/forge-adapter-claude-fs/src/sessions.rs:404`, `crates/forge-application/src/event.rs:100`, `crates/forge-application/src/use_cases/session_lifecycle.rs:103,151,178`, `crates/forge-application/src/use_cases/sidecar_commands.rs:61`, `crates/forge-application/src/live_session.rs:167`, `crates/forge-application/src/registry.rs:90,233`, `crates/forge-features/src/sidebar.rs:370`

**Interfaces:**
- Consumes: `SessionUsage` (Task 2).
- Produces: `Session.usage: SessionUsage`.

- [ ] **Step 1: Write the failing test**

In `crates/forge-domain/src/session.rs`, add at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_session_has_used_nothing() {
        let session = Session {
            id: SessionId("s".to_string()),
            project_path: PathBuf::from("/p"),
            title: None,
            messages: Vec::new(),
            permission_mode: PermissionMode::Default,
            usage: SessionUsage::default(),
        };
        assert!(session.usage.is_empty());
    }
}
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p forge-domain a_new_session_has_used_nothing`
Expected: FAIL to compile — `struct Session has no field named usage`.

- [ ] **Step 3: Add the field**

In `session.rs`, change the imports and struct:

```rust
use crate::{PermissionMode, SessionUsage};
```
```rust
pub struct Session {
    pub id: SessionId,
    pub project_path: PathBuf,
    pub title: Option<String>,
    pub messages: Vec<Message>,
    pub permission_mode: PermissionMode,
    /// What the session has used so far: restored from the session file, then grown by each turn.
    pub usage: SessionUsage,
}
```

- [ ] **Step 4: Fix every literal**

Run: `cargo check --workspace --all-targets 2>&1 | grep -A3 "missing field"`
Expected: one error per site listed under **Files**. In each, add `usage: SessionUsage::default(),` after the `permission_mode: …,` line and add `SessionUsage` to that file's `use forge_domain::{…}` import (in test modules, to the test module's imports). Repeat the `cargo check` until it prints nothing.

- [ ] **Step 5: Run the whole suite**

Run: `cargo test --workspace`
Expected: PASS, with N0 + 13 + 1 tests (Task 2's 13 and this task's 1).

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: Session carries its usage" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Restore usage from the session file

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/usage.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (add `mod usage;`)
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (call the collector in `load_session`; add one integration test)

**Interfaces:**
- Consumes: `SessionUsage`, `TokenUsage` (Task 2); `Session.usage` (Task 3).
- Produces: `UsageCollector::default()`, `.see(&serde_json::Value)`, `.finish() -> SessionUsage` (crate-private); `load_session` returns `Session.usage` filled.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-adapter-claude-fs/src/usage.rs` with the tests only:

```rust
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
```

Add `mod usage;` to `crates/forge-adapter-claude-fs/src/lib.rs` (next to `mod trash;`).

In `sessions.rs`, inside `mod tests`, add one integration test:

```rust
    #[tokio::test]
    async fn load_session_sums_usage_once_per_message_id() {
        let home = tempfile::tempdir().unwrap();
        let line = |id: &str, output: u64| {
            json!({"type": "assistant", "isSidechain": false, "message": {
                "id": id, "role": "assistant", "model": "claude-sonnet-4-6", "content": [{"type": "text", "text": "hi"}],
                "usage": {"input_tokens": 3, "output_tokens": output, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 0}
            }})
        };
        write_session(
            home.path(),
            &project(),
            "s1",
            &[user_text("hello"), line("m1", 10), line("m1", 10), line("m2", 20)],
        );
        let adapter = ClaudeSessionAdapter::new(home.path().to_path_buf());

        let loaded = adapter.load_session(&project(), &SessionId("s1".to_string())).await.unwrap();

        let total = loaded.usage.total();
        assert_eq!((total.input, total.output, total.cache_read), (6, 30, 200));
        assert_eq!(loaded.usage.models().count(), 1);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs usage`
Expected: FAIL to compile — `cannot find type UsageCollector`.

- [ ] **Step 3: Write the collector**

Put this **above** the test module in `usage.rs`:

```rust
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
```

- [ ] **Step 4: Call it from `load_session`**

In `sessions.rs` add the import `use crate::usage::UsageCollector;` next to `use crate::trash::…`. In `load_session`, after `let mut titles = TitleTracker::default();` add:
```rust
        let mut usage = UsageCollector::default();
```
In the `Some(value) => {` arm, after the `title_from_value` block and before `apply_line(&mut session, &value)`, add:
```rust
                    usage.see(&value);
```
After `session.title = titles.title();` add:
```rust
        session.usage = usage.finish();
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: PASS (the 10 new unit tests, the integration test and the existing ones).

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: restore a session's usage from its file, once per message id" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Sidecar — per-turn usage deltas and `context_usage`

> **Revised after the Task 1 spike (spec §20.4, §20.10):** the sidecar reports `result.usage` (main loop, per turn, with the 5m/1h split) under the main-loop assistant model, **not** `modelUsage`. So there is no `UsageTracker`, no `cumulative` switch and no `RESULT_USAGE_IS_CUMULATIVE`; `sidecar/src/usage.ts` has `toWireUsage(usage)` and `turnUsage(model, usage)`. The code and tests below this note that mention `UsageTracker`/`modelUsage` are superseded by what was committed.

**Files:**
- Create: `sidecar/src/usage.ts`, `sidecar/test/usage.test.ts`
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `sidecar/test/session.test.ts`

**Interfaces:**
- Consumes: the spike's finding 1 (`RESULT_USAGE_IS_CUMULATIVE`) and the real SDK field names from Task 1 Step 1 (assumed below: `modelUsage[model].inputTokens|outputTokens|cacheReadInputTokens|cacheCreationInputTokens`, `getContextUsage() → { totalTokens, maxTokens }`).
- Produces: wire events `{ type: "turn_complete", usage: Record<model, WireTokenUsage> }` and `{ type: "context_usage", total_tokens, max_tokens }`; `WireTokenUsage { input, output, cache_read, cache_write_5m, cache_write_1h }`; `UsageTracker(cumulative: boolean).turn(modelUsage: unknown)`; `SidecarSession` third constructor argument `{ contextUsageTimeoutMs?: number }`.

- [ ] **Step 1: Write the failing tests**

Create `sidecar/test/usage.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { toWireUsage, UsageTracker } from "../src/usage.js";

const entry = (inputTokens: number, outputTokens: number, cacheReadInputTokens = 0, cacheCreationInputTokens = 0) => ({
  inputTokens, outputTokens, cacheReadInputTokens, cacheCreationInputTokens, costUSD: 1, contextWindow: 200000,
});

describe("toWireUsage", () => {
  it("maps the SDK's per-model entry to the wire shape, counting cache writes as 5m", () => {
    expect(toWireUsage(entry(1, 2, 3, 4))).toEqual({ input: 1, output: 2, cache_read: 3, cache_write_5m: 4, cache_write_1h: 0 });
  });

  it("treats missing, negative and non-numeric fields as zero", () => {
    expect(toWireUsage({ inputTokens: -5, outputTokens: "x" })).toEqual({ input: 0, output: 0, cache_read: 0, cache_write_5m: 0, cache_write_1h: 0 });
    expect(toWireUsage(undefined)).toEqual({ input: 0, output: 0, cache_read: 0, cache_write_5m: 0, cache_write_1h: 0 });
  });
});

describe("UsageTracker", () => {
  it("per-turn mode passes each turn's usage through unchanged", () => {
    const tracker = new UsageTracker(false);
    expect(tracker.turn({ "m-a": entry(10, 20) })).toEqual({ "m-a": toWireUsage(entry(10, 20)) });
    expect(tracker.turn({ "m-a": entry(1, 2) })).toEqual({ "m-a": toWireUsage(entry(1, 2)) });
  });

  it("running-total mode sends only what the turn added", () => {
    const tracker = new UsageTracker(true);
    tracker.turn({ "m-a": entry(10, 20, 100, 5) });
    expect(tracker.turn({ "m-a": entry(12, 50, 400, 5) })).toEqual({
      "m-a": { input: 2, output: 30, cache_read: 300, cache_write_5m: 0, cache_write_1h: 0 },
    });
  });

  it("running-total mode treats a total that went down as a restart", () => {
    const tracker = new UsageTracker(true);
    tracker.turn({ "m-a": entry(100, 100) });
    expect(tracker.turn({ "m-a": entry(7, 3) })).toEqual({ "m-a": toWireUsage(entry(7, 3)) });
  });

  it("a model seen for the first time in running-total mode counts in full", () => {
    const tracker = new UsageTracker(true);
    tracker.turn({ "m-a": entry(10, 10) });
    expect(tracker.turn({ "m-a": entry(10, 10), "m-b": entry(4, 5) })).toEqual({ "m-b": toWireUsage(entry(4, 5)) });
  });

  it("leaves out models that added nothing", () => {
    const tracker = new UsageTracker(true);
    tracker.turn({ "m-a": entry(10, 10) });
    expect(tracker.turn({ "m-a": entry(10, 10) })).toEqual({});
  });

  it("returns nothing when the result has no model usage", () => {
    expect(new UsageTracker(false).turn(undefined)).toEqual({});
    expect(new UsageTracker(true).turn(null)).toEqual({});
  });
});
```

In `sidecar/test/session.test.ts`: change the helper at the top to let a test supply the extra query methods:

```ts
function toQueryHandle(gen: AsyncGenerator<unknown>, extra: Record<string, unknown> = {}): ReturnType<QueryFn> {
  return Object.assign(gen, { interrupt: async () => undefined }, extra);
}
```
Replace the existing test `forwards a result message as turn_complete` (line ~277) with these:

```ts
  it("forwards a result message as turn_complete with the turn's per-model usage", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle((async function* () {
        yield {
          type: "result",
          subtype: "success",
          usage: { input_tokens: 12, output_tokens: 34 },
          modelUsage: {
            "claude-test": { inputTokens: 12, outputTokens: 34, cacheReadInputTokens: 5, cacheCreationInputTokens: 6, costUSD: 0.1, contextWindow: 200000 },
          },
        };
      })());

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events).toContainEqual({
      type: "turn_complete",
      usage: { "claude-test": { input: 12, output: 34, cache_read: 5, cache_write_5m: 6, cache_write_1h: 0 } },
    });
  });

  it("sends context_usage after turn_complete", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", modelUsage: {} };
          await new Promise((resolve) => setTimeout(resolve, 20));
        })(),
        { getContextUsage: async () => ({ totalTokens: 76200, maxTokens: 200000, percentage: 38 }) },
      );

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    const types = events.map((e) => e.type);
    expect(types.indexOf("context_usage")).toBeGreaterThan(types.indexOf("turn_complete"));
    expect(events).toContainEqual({ type: "context_usage", total_tokens: 76200, max_tokens: 200000 });
  });

  it("sends no context_usage when getContextUsage throws, and the turn still completes", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", modelUsage: {} };
          await new Promise((resolve) => setTimeout(resolve, 20));
        })(),
        { getContextUsage: async () => { throw new Error("nope"); } },
      );

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.map((e) => e.type)).toContain("turn_complete");
    expect(events.map((e) => e.type)).not.toContain("context_usage");
    expect(events.map((e) => e.type)).not.toContain("error");
  });

  it("gives up on a getContextUsage that never answers", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", modelUsage: {} };
          await new Promise((resolve) => setTimeout(resolve, 60));
        })(),
        { getContextUsage: () => new Promise(() => {}) },
      );

    const session = new SidecarSession(queryFn, (e) => events.push(e), { contextUsageTimeoutMs: 10 });
    await session.handle(startCommand);

    expect(events.map((e) => e.type)).not.toContain("context_usage");
  });

  it("ignores a context answer with an unusable window", async () => {
    const events: SidecarEvent[] = [];
    const queryFn: QueryFn = () =>
      toQueryHandle(
        (async function* () {
          yield { type: "result", subtype: "success", modelUsage: {} };
          await new Promise((resolve) => setTimeout(resolve, 20));
        })(),
        { getContextUsage: async () => ({ totalTokens: 5, maxTokens: 0 }) },
      );

    const session = new SidecarSession(queryFn, (e) => events.push(e));
    await session.handle(startCommand);

    expect(events.map((e) => e.type)).not.toContain("context_usage");
  });
```
(The test near line 428 that yields `usage` only keeps working: no `modelUsage` gives `usage: {}`.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd sidecar && npx vitest run test/usage.test.ts test/session.test.ts`
Expected: FAIL — `Cannot find module '../src/usage.js'` and the changed session tests fail.

- [ ] **Step 3: Implement**

`sidecar/src/protocol.ts` — add the wire type and replace the `turn_complete` event:

```ts
export interface WireTokenUsage {
  input: number;
  output: number;
  cache_read: number;
  cache_write_5m: number;
  cache_write_1h: number;
}
```
```ts
  | { type: "turn_complete"; usage: Record<string, WireTokenUsage> }
  | { type: "context_usage"; total_tokens: number; max_tokens: number }
```

Create `sidecar/src/usage.ts`:

```ts
import type { WireTokenUsage } from "./protocol.js";

const count = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? Math.floor(value) : 0;

// The SDK reports one cache-write number with no 5m/1h split, so it is sent as 5m (spec §20.2).
export function toWireUsage(entry: any): WireTokenUsage {
  return {
    input: count(entry?.inputTokens),
    output: count(entry?.outputTokens),
    cache_read: count(entry?.cacheReadInputTokens),
    cache_write_5m: count(entry?.cacheCreationInputTokens),
    cache_write_1h: 0,
  };
}

const isZero = (u: WireTokenUsage): boolean =>
  u.input + u.output + u.cache_read + u.cache_write_5m + u.cache_write_1h === 0;

function difference(now: WireTokenUsage, before: WireTokenUsage): WireTokenUsage {
  const fields = Object.keys(now) as Array<keyof WireTokenUsage>;
  // A total that went down means the SDK started counting again: take it as it stands.
  if (fields.some((field) => now[field] < before[field])) return now;
  return {
    input: now.input - before.input,
    output: now.output - before.output,
    cache_read: now.cache_read - before.cache_read,
    cache_write_5m: now.cache_write_5m - before.cache_write_5m,
    cache_write_1h: now.cache_write_1h - before.cache_write_1h,
  };
}

/**
 * Turns the `modelUsage` of each `result` message into what that turn added. With `cumulative`
 * the SDK's numbers are running totals and the tracker subtracts the previous ones.
 */
export class UsageTracker {
  private readonly last = new Map<string, WireTokenUsage>();

  constructor(private readonly cumulative: boolean) {}

  turn(modelUsage: unknown): Record<string, WireTokenUsage> {
    const added: Record<string, WireTokenUsage> = {};
    if (typeof modelUsage !== "object" || modelUsage === null) return added;
    for (const [model, entry] of Object.entries(modelUsage)) {
      const now = toWireUsage(entry);
      const before = this.cumulative ? this.last.get(model) : undefined;
      const delta = before ? difference(now, before) : now;
      if (this.cumulative) this.last.set(model, now);
      if (!isZero(delta)) added[model] = delta;
    }
    return added;
  }
}
```

`sidecar/src/session.ts`:

1. Imports: `import { UsageTracker } from "./usage.js";`
2. Extend the handle type:
```ts
export type QueryHandle = AsyncIterable<unknown> & {
  interrupt: () => Promise<unknown>;
  getContextUsage?: () => Promise<{ totalTokens: number; maxTokens: number }>;
};
```
3. Above the class add (set the value from the spike, §20.10):
```ts
// Whether `result.modelUsage` holds running totals for the session (true) or just the turn (false).
// Settled by the Plan 10 spike, spec §20.10.
const RESULT_USAGE_IS_CUMULATIVE = false;
const CONTEXT_USAGE_TIMEOUT_MS = 3000;

function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T | undefined> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => resolve(undefined), ms);
    promise.then(
      (value) => { clearTimeout(timer); resolve(value); },
      (error) => { clearTimeout(timer); reject(error); },
    );
  });
}
```
4. In the class add the fields and constructor argument:
```ts
  private readonly usage = new UsageTracker(RESULT_USAGE_IS_CUMULATIVE);
  private readonly contextUsageTimeoutMs: number;

  constructor(
    private readonly queryFn: QueryFn,
    private readonly emit: (event: SidecarEvent) => void,
    options: { contextUsageTimeoutMs?: number } = {},
  ) {
    this.contextUsageTimeoutMs = options.contextUsageTimeoutMs ?? CONTEXT_USAGE_TIMEOUT_MS;
  }
```
(replacing the existing two-argument constructor).
5. Replace the `case "result":` block:
```ts
      case "result":
        this.emit({ type: "turn_complete", usage: this.usage.turn(message.modelUsage) });
        void this.emitContextUsage();
        return;
```
6. Add the method (next to `forwardMessage`):
```ts
  // A failure or a slow answer only means no ring value this turn; it never touches the turn.
  private async emitContextUsage(): Promise<void> {
    const query = this.activeQuery;
    if (!query?.getContextUsage) return;
    try {
      const context = await withTimeout(query.getContextUsage(), this.contextUsageTimeoutMs);
      if (this.shuttingDown || !context) return;
      const { totalTokens, maxTokens } = context;
      if (Number.isFinite(totalTokens) && Number.isFinite(maxTokens) && maxTokens > 0 && totalTokens >= 0) {
        this.emit({ type: "context_usage", total_tokens: totalTokens, max_tokens: maxTokens });
      }
    } catch {
      // No context value this turn.
    }
  }
```

- [ ] **Step 4: Run the tests and the typecheck**

Run: `cd sidecar && npm run typecheck && npm test`
Expected: PASS (all of vitest, including the new tests).

- [ ] **Step 5: Commit**

```bash
git add sidecar/src sidecar/test
git commit -m "feat: sidecar sends per-turn usage deltas and the context window use" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Ports and application — totals and the context value

**Files:**
- Modify: `crates/forge-ports/src/sidecar_port.rs`
- Modify: `crates/forge-application/src/event.rs`
- Modify: `crates/forge-application/src/live_session.rs`
- Modify: `crates/forge-application/src/live_session_actor.rs`
- Modify: `crates/forge-features/src/sidebar.rs` (the test `LiveSessionView` literal at line ~367)

**Interfaces:**
- Consumes: `TokenUsage`, `ContextUsage`, `SessionUsage` (Task 2); `Session.usage` (Task 3); the wire shapes from Task 5.
- Produces: `SidecarEvent::TurnComplete { usage: BTreeMap<String, TokenUsage> }`, `SidecarEvent::ContextUsage { total_tokens: u64, max_tokens: u64 }`; `SessionUpdate::TurnCompleted` (no fields), `SessionUpdate::ContextUsage(ContextUsage)`; `LiveSessionView.context: Option<ContextUsage>`.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-ports/src/sidecar_port.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn turn_complete_carries_per_model_usage() {
        let event: SidecarEvent = serde_json::from_str(
            r#"{"type":"turn_complete","usage":{"claude-test":{"input":12,"output":34,"cache_read":5,"cache_write_5m":6,"cache_write_1h":0}}}"#,
        )
        .unwrap();
        let SidecarEvent::TurnComplete { usage } = event else { panic!("not a turn_complete") };
        assert_eq!(
            usage["claude-test"],
            forge_domain::TokenUsage { input: 12, output: 34, cache_read: 5, cache_write_5m: 6, cache_write_1h: 0 }
        );
    }

    #[test]
    fn turn_complete_with_no_usage_is_still_a_turn_complete() {
        let event: SidecarEvent = serde_json::from_str(r#"{"type":"turn_complete","usage":{}}"#).unwrap();
        assert_eq!(event, SidecarEvent::TurnComplete { usage: Default::default() });
    }

    #[test]
    fn context_usage_event_parses() {
        let event: SidecarEvent =
            serde_json::from_str(r#"{"type":"context_usage","total_tokens":76200,"max_tokens":200000}"#).unwrap();
        assert_eq!(event, SidecarEvent::ContextUsage { total_tokens: 76_200, max_tokens: 200_000 });
    }
```

In `crates/forge-application/src/event.rs`, replace the test `turn_complete_reports_usage` with:

```rust
    #[test]
    fn turn_complete_adds_the_turns_usage_to_the_session() {
        let mut session = empty_session();
        session.usage.add("claude-test", TokenUsage { input: 100, output: 1, ..Default::default() });
        let event = SidecarEvent::TurnComplete {
            usage: BTreeMap::from([
                ("claude-test".to_string(), TokenUsage { input: 10, output: 20, ..Default::default() }),
                ("claude-other".to_string(), TokenUsage { output: 5, ..Default::default() }),
            ]),
        };

        let update = apply_sidecar_event(&mut session, &event);

        assert_eq!(update, SessionUpdate::TurnCompleted);
        assert_eq!(session.usage.total(), TokenUsage { input: 110, output: 26, ..Default::default() });
        assert_eq!(session.usage.models().count(), 2);
    }

    #[test]
    fn context_usage_is_reported_and_leaves_the_session_alone() {
        let mut session = empty_session();
        let update = apply_sidecar_event(&mut session, &SidecarEvent::ContextUsage { total_tokens: 5, max_tokens: 10 });
        assert_eq!(update, SessionUpdate::ContextUsage(ContextUsage { total_tokens: 5, max_tokens: 10 }));
        assert!(session.usage.is_empty());
    }
```
and add to that test module's imports `use std::collections::BTreeMap; use forge_domain::{ContextUsage, TokenUsage};`.

In `live_session.rs` tests: change the helper
```rust
    fn turn_complete() -> SidecarEvent {
        SidecarEvent::TurnComplete { usage: BTreeMap::new() }
    }
```
and add:

```rust
    #[tokio::test]
    async fn a_finished_turn_adds_its_usage_to_the_totals_that_came_with_the_session() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let mut init = resumed_init(factory);
        init.session.usage.add("claude-test", TokenUsage { input: 100, ..Default::default() });
        let handle = spawn_live_session(init);
        let mut view = handle.subscribe();

        handle.send("go".to_string());
        control.push(SidecarEvent::TurnComplete {
            usage: BTreeMap::from([("claude-test".to_string(), TokenUsage { input: 10, output: 5, ..Default::default() })]),
        });
        let seen = wait_for(&mut view, "the usage to grow", |v| v.session.usage.total().output == 5).await;

        assert_eq!(seen.session.usage.total().input, 110);
        assert_eq!(seen.phase, Phase::Idle);
    }

    #[tokio::test]
    async fn a_context_update_replaces_the_last_one() {
        let (sidecar, control) = fake_sidecar();
        let (factory, _spawns) = factory_returning(vec![Ok(sidecar)]);
        let handle = spawn_live_session(new_init(factory));
        let mut view = handle.subscribe();
        assert_eq!(handle.snapshot().context, None, "nothing is measured before the first turn");

        handle.send("go".to_string());
        control.push(SidecarEvent::ContextUsage { total_tokens: 10, max_tokens: 100 });
        control.push(SidecarEvent::ContextUsage { total_tokens: 20, max_tokens: 100 });
        let seen = wait_for(&mut view, "the second context value", |v| {
            v.context == Some(ContextUsage { total_tokens: 20, max_tokens: 100 })
        })
        .await;

        assert_eq!(seen.context.map(|c| c.percent()), Some(20));
    }
```
Add `ContextUsage, TokenUsage` to that test module's `use forge_domain::{…}` import.

In `crates/forge-features/src/sidebar.rs` test helper `view(...)` add `context: None,` after `failed_sends: 0,`.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-ports -p forge-application`
Expected: FAIL to compile — the variants and the `context` field do not exist yet.

- [ ] **Step 3: Implement**

`crates/forge-ports/src/sidecar_port.rs`: add `use std::collections::BTreeMap;`, change the import to `use forge_domain::{ActiveExtensions, PermissionDecision, PermissionMode, TokenUsage};` and replace the `TurnComplete` variant:

```rust
    /// What the turn added, per model.
    TurnComplete {
        #[serde(default)]
        usage: BTreeMap<String, TokenUsage>,
    },
    /// How full the context window is, sent after each `TurnComplete` when the SDK could say.
    ContextUsage { total_tokens: u64, max_tokens: u64 },
```

`crates/forge-application/src/event.rs`: import `ContextUsage` from `forge_domain`; replace the `TurnCompleted` variant and add the new one:

```rust
    TurnCompleted,
    ContextUsage(ContextUsage),
```
and the two match arms:

```rust
        SidecarEvent::TurnComplete { usage } => {
            for (model, tokens) in usage {
                session.usage.add(model, *tokens);
            }
            SessionUpdate::TurnCompleted
        }
        SidecarEvent::ContextUsage { total_tokens, max_tokens } => SessionUpdate::ContextUsage(ContextUsage {
            total_tokens: *total_tokens,
            max_tokens: *max_tokens,
        }),
```

`crates/forge-application/src/live_session.rs`: import `ContextUsage` (`use forge_domain::{ContextUsage, PermissionDecision, PermissionRequest, Session, SessionId};`), add to `LiveSessionView` after `failed_sends`:

```rust
    /// How full the context window was after the last finished turn. `None` until a turn has
    /// finished in this run: the sidecar starts on the first send, so an opened-but-idle session
    /// has no value yet.
    pub context: Option<ContextUsage>,
```
and in `spawn_live_session`'s literal add `context: None,` after `failed_sends: 0,`.

`crates/forge-application/src/live_session_actor.rs`: in `apply_event` change the arm `SessionUpdate::TurnCompleted { .. } => {` to `SessionUpdate::TurnCompleted => {` and add, before `SessionUpdate::Errored`:

```rust
            SessionUpdate::ContextUsage(context) => self.view.context = Some(context),
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --workspace`
Expected: PASS. (If `cargo check --workspace --all-targets` shows other users of the old `TurnComplete { input_tokens, output_tokens }`, e.g. in `crates/forge-adapter-sidecar/examples/` or `tests/`, change them to the new shape.)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: sessions keep running usage totals and the last context window value" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: `forge-entities` — `UsageView` and formatting

**Files:**
- Create: `crates/forge-entities/src/usage_view.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `SessionUsage`, `ContextUsage`, `PriceTable`, `Cost`, `TokenUsage` (Task 2).
- Produces (the widget in Task 8 uses exactly these):
  - `format_tokens(u64) -> String`, `format_cost(Cost) -> String`
  - `RingLevel { Normal, Warn, Danger }` with `class(self) -> &'static str`; `ring_level(percent: u8) -> RingLevel`
  - `ring_dash(percent: u8) -> String` (the SVG `stroke-dasharray`)
  - `ContextView { percent: u8, level: RingLevel, label: String }`
  - `ModelLine { model: String, tokens: String, cost: String }`
  - `UsageView { context: Option<ContextView>, input, output, cache_read, cache_write: String, cost: Option<String>, cost_note: Option<String>, models: Vec<ModelLine>, has_usage: bool }`
  - `UsageView::new(&SessionUsage, Option<ContextUsage>, &PriceTable) -> UsageView`, `UsageView::for_session(&SessionUsage, Option<ContextUsage>) -> UsageView` (uses `PriceTable::list_prices()`).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/usage_view.rs` with the tests only:

```rust
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
            TokenUsage { input: 1_000_000, output: 100_000, cache_read: 2_000, cache_write_5m: 3_000, cache_write_1h: 4_000 },
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
}
```

Add to `crates/forge-entities/src/lib.rs`: `mod usage_view;` and
```rust
pub use usage_view::{format_cost, format_tokens, ring_dash, ring_level, ContextView, ModelLine, RingLevel, UsageView};
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-entities usage_view`
Expected: FAIL to compile — `cannot find function format_tokens`.

- [ ] **Step 3: Implement**

Put this above the test module in `usage_view.rs`:

```rust
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-entities`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: UsageView formats tokens, cost and the context ring" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The ring and the popover

**Files:**
- Create: `crates/forge-widgets/src/usage_ring.rs`
- Modify: `crates/forge-widgets/src/lib.rs`, `crates/forge-widgets/src/chat_window.rs`
- Modify: `crates/forge-features/src/send_message.rs` (a `children` slot)
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs` (tests)

**Interfaces:**
- Consumes: `UsageView`, `ring_dash`, `RingLevel` (Task 7); `use_workspace().live_view(id)` giving `LiveSessionView { session.usage, context }` (Task 6).
- Produces: `UsageRing { id: LiveSessionId }`; `SendMessageBox` accepts children rendered first in `.composer-row`.

- [ ] **Step 1: Write the failing stylesheet tests**

In `crates/forge-shared/src/theme.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn the_usage_ring_styles_exist() {
        for selector in [".usage-ring", ".usage-ring.warn", ".usage-ring.danger", ".usage-popover", ".usage-backdrop"] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
    }

    #[test]
    fn the_usage_styles_take_every_color_from_the_theme_variables() {
        let rules: Vec<&str> = THEME_CSS.lines().filter(|line| line.starts_with(".usage")).collect();
        assert!(rules.len() >= 5, "found only {} usage rules", rules.len());
        for rule in rules {
            assert!(!rule.contains('#'), "a hard-coded color in: {rule}");
            assert!(!rule.contains("rgb"), "a hard-coded color in: {rule}");
        }
    }
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p forge-shared usage`
Expected: FAIL — `theme.css is missing .usage-ring` (and `found only 0 usage rules`).

- [ ] **Step 3: Add the styles**

Append to `crates/forge-shared/src/theme.css`, after the `.composer-row .spacer` line (one rule per line, as the tests expect):

```css
.usage { position: relative; flex: none; }
.usage-ring { display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; padding: 0; border: none; border-radius: 50%; background: transparent; cursor: pointer; }
.usage-ring:hover { background: var(--bg-hover); }
.usage-ring svg { width: 20px; height: 20px; transform: rotate(-90deg); }
.usage-ring .track { fill: none; stroke: var(--border); stroke-width: 3; }
.usage-ring .arc { fill: none; stroke: var(--accent); stroke-width: 3; stroke-linecap: round; }
.usage-ring.warn .arc { stroke: var(--warning); }
.usage-ring.danger .arc { stroke: var(--danger); }
.usage-ring.empty .arc { stroke: none; }
.usage-backdrop { position: fixed; inset: 0; z-index: 9; background: transparent; }
.usage-popover { position: absolute; left: 0; bottom: calc(100% + 8px); z-index: 10; width: 300px; padding: 12px; box-sizing: border-box; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); box-shadow: var(--shadow); font-size: 12px; }
.usage-heading { margin: 0 0 6px; font-size: 11px; font-weight: 600; color: var(--fg-muted); text-transform: uppercase; letter-spacing: 0.04em; }
.usage-row { display: flex; justify-content: space-between; gap: 12px; padding: 1px 0; }
.usage-row .value { font-variant-numeric: tabular-nums; }
.usage-section + .usage-section { margin-top: 10px; padding-top: 10px; border-top: 1px solid var(--border); }
.usage-note { margin-top: 4px; font-size: 11px; color: var(--fg-muted); }
```

- [ ] **Step 4: Run the stylesheet tests to verify they pass**

Run: `cargo test -p forge-shared`
Expected: PASS.

- [ ] **Step 5: Give `SendMessageBox` a children slot**

In `crates/forge-features/src/send_message.rs` change the signature and the composer row:

```rust
pub fn SendMessageBox(id: LiveSessionId, running: bool, can_send: bool, children: Element) -> Element {
```
```rust
            div { class: "composer-row",
                {children}
                span { class: "spacer" }
```
Run `cargo check --workspace --all-targets` and, for any other caller of `SendMessageBox` it reports (there should be none besides `chat_window.rs`), pass no children.

- [ ] **Step 6: Write the widget**

Create `crates/forge-widgets/src/usage_ring.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{ring_dash, UsageView};
use forge_features::use_workspace;

/// The context ring beside the message box. Clicking it opens a popover with the session's token
/// totals, an estimated cost and a line per model.
#[component]
pub fn UsageRing(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut open = use_signal(|| false);
    let Some(view_signal) = workspace.live_view(id) else {
        return rsx! {};
    };
    let usage = {
        let view = view_signal.read();
        UsageView::for_session(&view.session.usage, view.context)
    };

    let context_text = usage
        .context
        .as_ref()
        .map(|context| context.label.clone())
        .unwrap_or_else(|| "Not measured yet — it appears after the first reply.".to_string());
    let cost_text = usage.cost.clone().unwrap_or_else(|| "—".to_string());
    let (level, dash, tooltip) = match &usage.context {
        Some(context) => (context.level.class(), ring_dash(context.percent), context.label.clone()),
        None => ("empty", ring_dash(0), "Context window not measured yet".to_string()),
    };

    rsx! {
        div { class: "usage",
            button {
                class: "usage-ring {level}",
                title: "{tooltip}",
                aria_label: "Session usage",
                onclick: move |_| open.set(!open()),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        open.set(false);
                    }
                },
                svg { view_box: "0 0 20 20",
                    circle { class: "track", cx: "10", cy: "10", r: "8" }
                    circle { class: "arc", cx: "10", cy: "10", r: "8", stroke_dasharray: "{dash}" }
                }
            }
            if open() {
                div { class: "usage-backdrop", onclick: move |_| open.set(false) }
                div { class: "usage-popover",
                    div { class: "usage-section",
                        div { class: "usage-heading", "Context window" }
                        div { class: "usage-row", span { "{context_text}" } }
                    }
                    div { class: "usage-section",
                        div { class: "usage-heading", "This session" }
                        div { class: "usage-row", span { "Input" } span { class: "value", "{usage.input}" } }
                        div { class: "usage-row", span { "Output" } span { class: "value", "{usage.output}" } }
                        div { class: "usage-row", span { "Cache read" } span { class: "value", "{usage.cache_read}" } }
                        div { class: "usage-row", span { "Cache write" } span { class: "value", "{usage.cache_write}" } }
                    }
                    div { class: "usage-section",
                        div { class: "usage-heading", "Estimated cost" }
                        div { class: "usage-row",
                            span { "Total" }
                            span { class: "value", "{cost_text}" }
                        }
                        if let Some(note) = usage.cost_note.as_ref() {
                            div { class: "usage-note", "{note}" }
                        }
                        div { class: "usage-note", "List-price estimate; it may differ from your bill." }
                    }
                    if !usage.models.is_empty() {
                        div { class: "usage-section",
                            div { class: "usage-heading", "By model" }
                            for line in usage.models.iter() {
                                div { class: "usage-row", key: "{line.model}",
                                    span { "{line.model}" }
                                    span { class: "value", "{line.tokens} · {line.cost}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod usage_ring;` and `pub use usage_ring::UsageRing;`.

In `chat_window.rs` add `use crate::usage_ring::UsageRing;` and change the send box:

```rust
            if view.can_send {
                SendMessageBox { id, running: view.phase == Phase::Running, can_send: true,
                    UsageRing { id }
                }
            } else {
```

- [ ] **Step 7: Build and run everything**

Run: `cargo build --workspace && cargo test --workspace`
Expected: build succeeds; all tests pass. If `stroke_dasharray` or `aria_label` is not an accepted attribute name in Dioxus 0.7.10, the compiler says so; use the attribute name it suggests (for a missing SVG attribute, `"stroke-dasharray": "{dash}"` string-literal syntax is accepted).

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "feat: context ring and usage popover beside the message box" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Check it in the real window, final review, as-built notes

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§20.11 "As built")

- [ ] **Step 1: Count the tests**

Run: `cargo test --workspace 2>&1 | grep "test result" | awk '{s+=$4} END {print s}'` and `cd sidecar && npm test 2>&1 | tail -5`.
Expected: the Rust total is N0 (Task 2 Step 1) + the new tests; vitest passes. Note both numbers.

- [ ] **Step 2: Walk through the real window**

Build the sidecar and start the app the way earlier plans did (`cd sidecar && npm run build`, then `cargo run -p forge-app`; if the spec's §15 or the earlier plans name another launch command, use that). Check each item and write down what you saw:

1. A new session: the ring is an empty circle; the tooltip says "Context window not measured yet"; the popover shows `$0.00`, zeros and no model list.
2. Send a short message. After the reply: the ring fills a little and its tooltip reads like `3% · 6.2k / 200.0k tokens`; the popover shows non-zero input/output, a cost and one model line.
3. Close the session, open it again from the sidebar **without sending**: the popover shows the same totals as before (restored from the file) and the ring is empty again (the sidecar has not started).
4. Send another message in that resumed session: totals grow by about one turn (not by the whole history) — this is the spike's finding 1 seen end to end.
5. Switch macOS to dark mode: the ring track, arc and the popover are readable; close the popover by clicking outside and with Esc.
6. A model the table does not know (set one by hand in Settings → Model as a custom model id): tokens show, cost shows `—` and the note says `excludes <id>`.
7. Resize the window narrow: the ring stays beside the Send button and the popover stays inside the window.

If a step cannot be done (no login, no window access), say so in the notes under "Not checked".

- [ ] **Step 3: Whole-branch review**

Invoke `superpowers:requesting-code-review` on the `plan-10` branch (base: `master`). Fix every Critical and Important finding, each with a failing test first; list the rest as deferred minors in the as-built notes.

- [ ] **Step 4: Write §20.11 "As built" into the spec**

Same shape as §19.9: the branch and date, the test counts from Step 1, **Checked (passed)**, **Not checked (needs a person)**, **Differences from §20** (anything the spike or review changed), **Final review** findings and what was fixed, **Not fixed, on purpose**.

- [ ] **Step 5: Commit and hand over**

```bash
git add docs/superpowers/specs/2026-09-27-forge-design.md
git commit -m "docs: record Plan 10 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
Then invoke `superpowers:finishing-a-development-branch`; the user decides how to integrate (earlier plans were fast-forward merged to `master`).

---

## Self-review (spec coverage)

| Spec | Task |
|---|---|
| §20.1 placement, history, one cost path, out of scope | Task 8 (ring beside the box), Task 4 (history), Task 2 + 7 (cost), Global Constraints |
| §20.2 domain types, price table, `Session.usage` | Task 2, Task 3 |
| §20.3 restore, once per `message.id`, ignores | Task 4 |
| §20.4 `turn_complete` deltas, `context_usage`, 3 s limit | Task 5 (sidecar), Task 6 (port) |
| §20.5 application and `UsageView` | Task 6, Task 7 |
| §20.6 ring, popover, colours, dark test that can fail | Task 8 (`the_usage_styles_take_every_color_from_the_theme_variables` can fail: it scans the usage rules) |
| §20.7 spike | Task 1 |
| §20.8 testing | tests in each task |
| §20.9 task order | Tasks 1–9 follow it |
