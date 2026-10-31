# Details panel cards: `컨텍스트`, `커넥터`, `변경 사항` (Plan 55) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The details panel gets three more cards under `환경`, like the desktop app: `컨텍스트` (usage and breakdown), `커넥터` (the session's MCP servers and a table modal) and `변경 사항` (changed line count that opens the Changes panel).

**Architecture:** The sidecar sends two new events after each turn, `context_detail` and `mcp_status`, built from the SDK's `getContextUsage()` and `mcpServerStatus()`. The live session view keeps the last of each. Pure view functions turn them into card rows; widgets draw the cards. The changes count is read from git with the call the Changes panel already uses. Each card folds on its own, per session.

**Tech Stack:** Rust, Dioxus 0.7.10, TypeScript sidecar (`sidecar/`, vitest), crates `forge-*`.

**Spec:** `docs/superpowers/specs/2026-10-23-details-cards-design.md` (binding). Evidence: `docs/superpowers/roadmap/observations.md` Round 26.

## Global Constraints

- Local only: no cloud, SSH, Slack or Remote Control anywhere.
- Every user-visible string goes through `tr!` (widgets) or `msg!` (domain) with a Korean entry in `crates/forge-shared/src/ko.rs` (use `python3 scripts/i18n_add.py`, which does NOT escape double quotes: check each entry by hand; run `cargo test -p forge-shared`). Keys and Korean: `Context` / `컨텍스트`, `Connectors` / `커넥터`, `Changes` already exists (reuse its Korean `변경 사항`; check), `{percent}% used` / `{percent}% 사용됨`, `Send a message to see the context usage.` / `컨텍스트 사용량을 확인하려면 메시지를 보내세요.`, `Send a message to see the connectors.` / `메시지를 보내면 커넥터를 확인할 수 있습니다.`, `Show more` / `더 보기`, `Show less` / `간략히 보기`, `Sign in` / `인증`, `Reconnect` / `다시 연결`, `Connecting` / `연결 중`, `Off` / `꺼짐`, `MCP servers` / `MCP 서버`, `Server` / `서버`, `Type` / `유형`, `Status` / `상태`, `Web` / `웹`, `Desktop` / `데스크톱`, `Plugin` / `플러그인`, `User` / `사용자`, `App` / `앱`, `Manage connectors` / `커넥터 관리`, `Done` / `완료`, `{name} plugin provides this` / `{name} 플러그인에서 제공됨`, `MCP tools` / `MCP 도구`, `Custom agents` / `커스텀 에이전트`, `Manage Skills` / `Skills 관리`.
- The ring keeps reading `context_usage`; the new events must not change what an existing session shows, and an old sidecar (no new events) leaves the new cards in their "send a message" state.
- A failed or slow SDK call sends nothing and never touches the turn (same rule as `emitContextUsage`).
- Not built: `결과물` card, commit button and prompt, sign-in / reconnect / connect buttons, reordering cards by hand, a link from `Custom agents`, Windows.
- Rust style of the repo: comments only where the reason is not obvious; tests next to the code with sentence names; commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Test binaries hang without `</dev/null`; the known unrelated failure is forge-adapter-claude-plugins `round_trip`. Sidecar tests: `cd sidecar && npx vitest run`.

## Review Focus

- A category row with an unknown `kind`, or tokens that are not finite, is dropped; the card never shows `NaN` or a negative number. (Task 1, 3.)
- Zero servers: the connector card says there are none (does not stay on a loading row); an unknown `status` string is dropped or shown as `pending`, never panics. (Task 1, 3.)
- A window of 0 tokens, or rows that add to more than the window: the percent is capped at 100 and the bar never overflows. (Task 3.)
- A folder that is not a git repository, or a git error: no `변경 사항` card and no error text. (Task 4.)
- A session closed while a changes read is in flight does not write into another session's card (tickets, as the Changes panel does). (Task 4.)
- Folding one card leaves the others as they were; the fold flag of a closed session is dropped (`forget_*` as the other per-session maps do). (Task 4.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `sidecar/src/protocol.ts`, `session.ts`, `sidecar/test/session.test.ts` | the two events and their emit after a turn |
| `crates/forge-ports/src/sidecar_port.rs` | `SidecarEvent::ContextDetail`, `SidecarEvent::McpStatus` and the payload structs |
| `crates/forge-domain/src/usage.rs` (or a new `context_detail.rs`), `lib.rs` | `ContextBreakdown`, `ContextRow`, `ContextKind`, `McpServerState`, `McpStatus` |
| `crates/forge-application/src/{live_session.rs,live_session_actor.rs,test_support.rs}` | the view keeps the last values |
| `crates/forge-entities/src/details_view.rs` (new), `lib.rs` | `context_card`, `connector_rows`, `connector_table`: pure |
| `crates/forge-features/src/{workspace.rs,details.rs (new),lib.rs}` | per-card fold, changes count, open-panel and open-Connectors actions |
| `crates/forge-widgets/src/{details_panel.rs,mcp_servers_dialog.rs (new),lib.rs}`, `crates/forge-shared/src/{ko.rs,theme.css}` | the cards, the modal, the strings and the styles |
| `docs/superpowers/{roadmap,specs}` | observations check, backlog, "As built" |

---

### Task 1: Sidecar sends `context_detail` and `mcp_status`

**Files:**
- Modify: `sidecar/src/protocol.ts`, `sidecar/src/session.ts`, `crates/forge-ports/src/sidecar_port.rs`
- Test: `sidecar/test/session.test.ts`, the test module of `sidecar_port.rs`

**Interfaces:**
- Produces (JSON, snake_case, like the existing events):
  - `{"type":"context_detail","categories":[{"name":"Messages","tokens":20100,"kind":"used"}],"mcp_tools":[{"server":"Claude Docs","name":"Make","tokens":152}],"agents":[{"name":"Explore","tokens":80}]}`
  - `{"type":"mcp_status","servers":[{"name":"figma","status":"connected","scope":"user","source":"user","type":"http","tool_count":12,"error":null}]}`
  - Rust: `SidecarEvent::ContextDetail { categories: Vec<ContextCategory>, mcp_tools: Vec<ContextItem>, agents: Vec<ContextItem> }` with `ContextCategory { name: String, tokens: u64, kind: String }` and `ContextItem { server: Option<String>, name: String, tokens: u64 }` (an agent has no `server`); `SidecarEvent::McpStatus { servers: Vec<McpServerReport> }` with `McpServerReport { name: String, status: String, scope: Option<String>, source: Option<String>, transport: Option<String> (serde rename `type`), tool_count: u64, error: Option<String> }`. Fields `scope`, `source`, `type`, `error` default to `None`.

- [ ] **Step 1: Write the failing sidecar tests** in `sidecar/test/session.test.ts`, next to "sends context_usage after turn_complete":
  - `sends context_detail with its categories after context_usage` (query stub's `getContextUsage` also returns `categories: [{name:"Messages",tokens:20100,kind:"used"},{name:"Free space",tokens:888900,kind:"free"}]`, `mcpTools: [{name,serverName,tokens}, …]` (2), `agents: [{agentType,tokens}]` (1): expect `mcp_tools` with `server`, `name`, `tokens`, `agents` with `name` from `agentType`, categories as given);
  - `drops categories with a bad kind or a non-finite token count`;
  - `sends mcp_status after a turn` (stub `mcpServerStatus` returns two servers, one with `tools: [a, b, c]` and `config: {type:"http"}`: expect `tool_count 3`, `type "http"`);
  - `sends no mcp_status when mcpServerStatus rejects or times out` and `sends no context_detail when the SDK sent no categories`.
- [ ] **Step 2: Run** `cd sidecar && npx vitest run test/session.test.ts` and see them fail.
- [ ] **Step 3: Implement** in `session.ts`: widen the `getContextUsage` type with `categories?`, `mcpTools?` (`name`, `serverName`, `tokens`), `agents?` (`agentType`, `tokens`); add `mcpServerStatus?: () => Promise<...>`; extend `emitContextUsage` to emit `context_detail` after `context_usage` from the same call (no second SDK call), and add `emitMcpStatus()` with the same timeout helper, called next to it after each `turn_complete`. `kind` outside `used|free|buffer|deferred` or a non-finite or negative `tokens` drops that category; an unknown `status` is sent as is (the Rust side maps it). Add the two event types to `protocol.ts`.
- [ ] **Step 4: Write the failing Rust tests** in `sidecar_port.rs`: `context_detail_event_parses`, `mcp_status_event_parses` (with the optional fields missing), using the JSON above.
- [ ] **Step 5: Implement** the two variants and structs in `sidecar_port.rs`; make every other `match` on `SidecarEvent` in the workspace compile (`cargo build --workspace`; an event the match does not use goes to its catch-all or is ignored there; Task 2 gives them meaning).
- [ ] **Step 6: Run** `cd sidecar && npx vitest run` and `cargo test -p forge-ports </dev/null`; expected PASS.
- [ ] **Step 7: Commit** `feat: the sidecar reports the context breakdown and the MCP server states`.

---

### Task 2: The live session view keeps the last breakdown and server list

**Files:**
- Modify: `crates/forge-domain/src/usage.rs` (or new `context_detail.rs`) and `lib.rs`, `crates/forge-application/src/{live_session.rs,live_session_actor.rs,event.rs if it maps events,test_support.rs}`
- Test: the test modules of `live_session.rs` / `live_session_actor.rs`, next to `a_context_update_replaces_the_last_one`

**Interfaces:**
- Consumes: Task 1's `SidecarEvent::ContextDetail`, `SidecarEvent::McpStatus`.
- Produces (domain):
  - `enum ContextKind { Used, Free, Buffer, Deferred }`; `struct ContextRow { name: String, tokens: u64, kind: ContextKind }`; `struct ContextItem { group: Option<String>, name: String, tokens: u64 }`; `struct ContextBreakdown { rows: Vec<ContextRow>, mcp_tools: Vec<ContextItem>, agents: Vec<ContextItem> }` (all `Clone, Debug, PartialEq, Eq`).
  - `enum McpStatus { Connected, NeedsAuth, Failed, Pending, Disabled }`; `struct McpServerState { name: String, status: McpStatus, scope: Option<String>, source: Option<String>, remote: Option<bool>, tool_count: u64, error: Option<String> }`.
  - `LiveSessionView.context_detail: Option<ContextBreakdown>` and `LiveSessionView.mcp_servers: Option<Vec<McpServerState>>`, both `None` until the first value.
  - mapping functions `ContextBreakdown::from_report(...)`/`McpServerState::from_report(...)`: a row with an unknown kind is dropped; status strings `connected`, `failed`, `needs-auth`, `pending`, `disabled`, anything else `Pending`; `remote` is `Some(false)` for transport `stdio`, `Some(true)` for `http`, `sse`, `claudeai-proxy`, `None` otherwise.

- [ ] **Step 1: Write the failing tests:** `a_context_detail_event_is_kept_and_replaced`, `an_mcp_status_event_replaces_the_server_list`, `an_empty_server_list_is_kept_as_empty` (Some(vec![]) is "no servers", not "not measured"), `an_unknown_kind_and_an_unknown_status_are_handled` (domain mapping tests), `remote_follows_the_transport`.
- [ ] **Step 2: Run** `cargo test -p forge-application -p forge-domain </dev/null`; see them fail.
- [ ] **Step 3: Implement** the domain types and mappings, the two view fields (default `None`, set in the constructor next to `context: None`), and the actor's handling next to `ContextUsage` (replace, publish the view). A resumed or restarted session keeps the last values until new ones come.
- [ ] **Step 4: Run** the same tests; PASS. Then `cargo build --workspace`.
- [ ] **Step 5: Commit** `feat: a live session keeps its context breakdown and MCP server states`.

---

### Task 3: Pure view functions for the cards

**Files:**
- Create: `crates/forge-entities/src/details_view.rs`; modify `crates/forge-entities/src/lib.rs`
- Test: the same file's `#[cfg(test)]` module

**Interfaces:**
- Consumes: `ContextBreakdown`, `ContextUsage`, `McpServerState`, `McpStatus` (Task 2); `format_tokens` of `usage_view.rs` (reuse; check its output for `78.1k` and `1M`: the desktop app shows `78.1k / 1M` and `20.1k`, `888.9k`; if `format_tokens` prints `1.0M` or `200.0k`, add a `format_tokens_short` that drops a trailing `.0`, and leave the existing function alone).
- Produces:
  - `struct ContextCardView { percent: u8, used_label: String, bar: Vec<BarSegment>, rows: Vec<ContextRowView>, deferred: Vec<ContextRowView>, mcp_tools: Vec<ContextItemView>, agents: Vec<ContextItemView>, folded_bar: u16 }`; `ContextItemView { label: String, tokens: String }` (`label` is `<group> · <name>` or just the name; `folded_bar` is the used part in per-mille); `BarSegment { color: usize, per_mille: u16 }`; `ContextRowView { name: String, tokens: String, percent: Option<String>, color: Option<usize> }` (`percent` `None` renders `—`; `color` an index into the ring palette, `None` for the greys).
  - `fn context_card(usage: Option<ContextUsage>, detail: Option<&ContextBreakdown>) -> Option<ContextCardView>`: `None` when there is no detail (the card shows its sentence). Percent comes from `ContextUsage::percent()` when present, else from the rows.
  - `struct ConnectorRowView { name: String, initial: char, state: Option<ConnectorState> }`, `enum ConnectorState { SignIn, Reconnect, Connecting, Off }`; `fn connector_rows(servers: &[McpServerState]) -> Vec<ConnectorRowView>` ordered as the spec says (needs action first, `claudeai` scope last).
  - `struct ConnectorTableRow { name: String, subtitle: Option<String>, kind: Option<TransportLabel>, tag: Option<ServerTag>, status: ConnectorState or Connected }`; `fn connector_table(servers: &[McpServerState], plugin_names: &dyn Fn(&str) -> Option<String>) -> Vec<ConnectorTableRow>` in the SDK's order; tag `Plugin` when `source == "plugin"`, `User` when `scope == "user"`.

- [ ] **Step 1: Write the failing tests** (names are sentences):
  - `the_context_card_has_the_percent_and_the_used_and_max_labels` (78.1k of 1M → `8`, `78.1k / 1M`);
  - `used_rows_come_before_free_and_buffer_and_deferred_rows_show_a_dash`;
  - `the_free_row_is_not_part_of_the_bar` and `bar_segments_never_add_to_more_than_a_thousand`;
  - `an_unmeasured_window_gives_zero_percent_without_dividing_by_zero` and `rows_over_the_window_cap_the_percent_at_100`;
  - `empty_mcp_and_agent_lists_give_no_fold_rows`, `an_item_label_joins_its_group_and_name`;
  - `servers_that_need_action_come_first_and_claudeai_ones_last`;
  - `a_connected_server_has_no_state_text`, `an_empty_list_gives_no_rows`;
  - `the_table_keeps_the_sdk_order_and_tags_plugin_and_user_servers`.
- [ ] **Step 2: Run** `cargo test -p forge-entities </dev/null`; see them fail.
- [ ] **Step 3: Implement** the two types of view and the functions above; keep them free of Dioxus and of strings that need translating (labels are produced by the widget with `tr!`; return enums, not text, for states, tags and transport).
- [ ] **Step 4: Run** the tests; PASS.
- [ ] **Step 5: Commit** `feat: view functions for the context and connector cards`.

---

### Task 4: Workspace state: per-card fold, changes count, open actions

**Files:**
- Create: `crates/forge-features/src/details.rs`; modify `crates/forge-features/src/{workspace.rs,lib.rs}`
- Test: `details.rs` tests and `workspace.rs` tests (where the other `card_folded` and Changes helpers are tested)

**Interfaces:**
- Consumes: the Changes panel's loader (find the git call and `ChangeSource` it uses for the uncommitted view of a session's folder; reuse them, do not write a second diff parser), `dirty_from_changes` style helpers in `change_rows.rs`.
- Produces:
  - `enum DetailsCard { Environment, Context, Connectors, Changes }`; `Workspace::details_card_folded(id, DetailsCard) -> bool` and `set_details_card_folded(id, DetailsCard, bool)`. Replace `environment_card_folded` / `set_environment_card_folded` and the `card_folded` map's key by `(LiveSessionId, DetailsCard)`; update their one caller later in Task 5; the closed-session cleanup that drops `card_folded` keeps working.
  - `struct ChangeCounts { files: usize, added: usize, removed: usize }`; pure `fn change_counts(files: &[ChangedFile]) -> Option<ChangeCounts>` (`None` for an empty list; a file with `added: None` or `removed: None` counts as 0 for the sums and still as a file).
  - `Workspace::session_change_counts(id) -> Option<ChangeCounts>` (a signal read) and `Workspace::refresh_change_counts(id)`: spawns the blocking git read with a ticket (as `ChangesPanel` does), writes only if the ticket is still current and the session is still open; called by the card on mount and whenever `finished_tool_calls(id)` changes.
  - `Workspace::open_changes_panel(id)`: opens `Panel::Changes` if not open (never closes it). `Workspace::open_skills_page()` and `Workspace::open_connectors_page()`: navigate to the Customize page on its Skills / Connectors tab (find how the sidebar or the Customize entry does it; reuse that route).

- [ ] **Step 1: Write the failing tests:** `change_counts_sums_lines_and_counts_files`, `change_counts_is_none_for_no_files`, `a_missing_line_count_counts_as_zero`, `folding_one_card_leaves_the_others`, `a_closed_sessions_card_flags_are_dropped`, `open_changes_panel_does_not_close_an_open_panel` (on `Dock`, as `dock.rs` tests do).
- [ ] **Step 2: Run** `cargo test -p forge-features </dev/null`; see them fail.
- [ ] **Step 3: Implement** the above. For a non-repository or a git error the count is `None` and nothing else changes.
- [ ] **Step 4: Run** the tests; PASS. `cargo build --workspace` will fail at the widget's old fold calls until Task 5: fix that one call site in this task (`EnvironmentCard` uses `DetailsCard::Environment`) so the build stays green.
- [ ] **Step 5: Commit** `feat: per-card fold state, the changes count and the actions the details cards open`.

---

### Task 5: The cards, the modal, the strings and the styles

**Files:**
- Modify: `crates/forge-widgets/src/{details_panel.rs,lib.rs}`, `crates/forge-shared/src/{ko.rs,theme.css}`
- Create: `crates/forge-widgets/src/mcp_servers_dialog.rs`
- Test: `cargo test -p forge-shared` (Korean entries); widget code is checked in Task 6's window check

**Interfaces:**
- Consumes: Task 3's view functions, Task 4's workspace calls, `LiveSessionView.context`, `.context_detail`, `.mcp_servers` (Task 2), `workspace.plugin_name(...)` or the equivalent lookup for a plugin's display name (if none exists, pass `|_| None` and skip the subtitle: the spec allows it).
- Produces: `ContextCard`, `ConnectorsCard`, `ChangesCard` components rendered by `DetailsPanel` after `EnvironmentCard` in that order; `McpServersDialog { id }` opened from a connector row or `더 보기`-free row click, closed by `완료` or Esc.

- [ ] **Step 1: Add the strings** from Global Constraints with `i18n_add.py`; run `cargo test -p forge-shared </dev/null` and fix an escaped-quote slip by hand.
- [ ] **Step 2: Implement `ContextCard`**: a header button like `EnvironmentCard`'s (same classes, chevron, `aria-expanded`, `DetailsCard::Context`), `{percent}% used` and the labels, the bar (flex segments with the palette colours), the rows as a grid of square, name, tokens, percent; the `MCP tools` and `Custom agents` fold rows with the count and a local open flag listing `ContextItemView`s; the `MCP tools` and `Skills` row names are buttons with a `title` (`Manage connectors`, `Manage Skills`) that call `open_connectors_page()` / `open_skills_page()` (Task 4). Folded: the header, `{percent}%` and a small bar. Without a card view: the grey sentence.
- [ ] **Step 3: Implement `ConnectorsCard`**: rows with a letter icon and state text (brown, the existing warning colour), first three then `더 보기` / `간략히 보기` (the expanded flag is local `use_signal`), a click on a row sets the modal open. With `mcp_servers == None`: the grey sentence; `Some(empty)`: a grey "No connectors" line (reuse the Customize page's string if it has one, else add `No connectors` / `커넥터 없음`).
- [ ] **Step 4: Implement `ChangesCard`**: nothing when `session_change_counts(id)` is `None`; else the diff icon and green `+N`, red `−N` as a button that calls `open_changes_panel(id)`. An effect calls `refresh_change_counts(id)` on mount and when `finished_tool_calls(id)` changes.
- [ ] **Step 5: Implement `McpServersDialog`**: a backdrop + panel using the stage C `WorkspaceTrustDialog`'s structure for focus and Esc; title, the session folder as grey text, the table, footer `Manage connectors` (calls `open_connectors_page()` and closes) and `Done`. Transport and tag labels via `tr!`.
- [ ] **Step 6: Styles** in `theme.css`: reuse `.env-card*` classes where the look is the same; add only what is new (`.ctx-bar`, `.ctx-row`, `.conn-row`, `.mcp-dialog*`), with light and dark values like the neighbouring rules.
- [ ] **Step 7: Run** `cargo build --workspace` and `cargo test -p forge-shared -p forge-features -p forge-entities </dev/null`; PASS.
- [ ] **Step 8: Commit** `feat: the context, connector and changes cards of the details panel`.

---

### Task 6: Real-window check, docs

**Files:**
- Modify: `docs/superpowers/specs/2026-10-23-details-cards-design.md` (add `## As built`, `## Checked`, `## Left`), `docs/superpowers/roadmap/backlog.md` (rows for `결과물` card, commit button, connector actions; mark the three cards built), memory `plan-55-details-cards.md` and its line in `MEMORY.md`

- [ ] **Step 1: Window check** with `scripts/harness/` on a throwaway `FORGE_CONFIG_PATH` and a folder under `/private/tmp` (never the real Forge config). Real turns on the user's account only as the user allowed in this session; ask again if more are needed. Check and write down: the three cards before a message (sentences, no card for changes), after a turn (percent and rows add up, deferred rows show `—`), the connector rows and states against `claude mcp list` on the same machine, the modal and `커넥터 관리` landing on the Connectors tab, the changes count after the model writes a file (appears) and after the folder is clean (gone), each card folding alone, a narrow window and dark mode (look, do not fix beyond CSS slips). Fix defects found; keep each fix a small commit.
- [ ] **Step 2: Final checks:** `cargo build --workspace`; `cargo test -p forge-domain -p forge-ports -p forge-application -p forge-entities -p forge-features -p forge-shared </dev/null`; `cd sidecar && npx vitest run`.
- [ ] **Step 3: Write the docs** (what was built, what was checked and how, what is left) and the memory file.
- [ ] **Step 4: Commit** `docs: Plan 55 as built`.
