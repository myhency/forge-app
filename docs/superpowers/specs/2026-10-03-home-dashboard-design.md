# Plan 24: Home dashboard and the new-session composer (PG-05, CMP-06)

Plan 24 adds a usage dashboard to the new-session screen and reshapes that screen to look and work like the desktop app's. It follows observation round 6 ([observations.md](../roadmap/observations.md), parts 1 to 3), not guesses.

## What the desktop app does (seen)

- **Dashboard card** under the greeting. Tabs "개요" (overview) and "모델" (models); ranges "전체" (all), "30일", "7d". Defaults: overview, all.
- **Overview**: six tiles (sessions, messages, total tokens, active days, peak hour, favourite model) and a heatmap of 7 rows (weekdays, Sunday first) by 27 columns (weeks, newest right, ending today). The ranges change the tiles; the heatmap does **not** change with the range. A caption compares the tokens to a book (a joke; left out).
- **Models**: a stacked bar chart of tokens per day, one colour per model, y axis 0 to the largest day in steps; below it one legend row per model: name, "<in> 입력 · <out> 출력", share in percent. Rows ordered by share. In "7d" there are exactly seven bars, one per day including empty days; in "전체" the axis is the date span.
- **Composer area**: chips "로컬" and the folder name and a folder-with-plus button; the box with a return-arrow icon (grey when empty, black when there is text); under it `+`, mode, and at the right model, effort, a circle. The folder chip's menu: "폴더 없음", the heading "최근" with recent folders (current one checked), "폴더 열기…".

## What Forge has now

- `UsageCollector` (`forge-adapter-claude-fs/src/usage.rs`) sums one transcript per model, once per `message.id`, skipping sidechain and meta lines. It keeps no dates.
- `UsageView`, `format_tokens` (`forge-entities/src/usage_view.rs`) format one session's usage. The usage ring uses them.
- The new-session screen (`NewSessionComposer`) is a form: a folder `Select`, the model as text ("Change it in Settings"), a mode `Select`, the box and a "Start" button. `start_new` reads the saved model from settings.
- `Picker` (a generic menu used by the session's mode and model controls) exists in `forge-widgets`. `ModePicker`, `ModelPicker` and `EffortPicker` read the open session (`live_view(id)`), so they cannot be used before a session exists.

## Design

### A. Statistics (data)

**Domain** (`forge-domain/src/usage_stats.rs`, pure, no I/O):

- `Day { year: i32, month: u8, day: u8 }` (ordered, with `days_since` and weekday), `StatsRow { day: Day, hour: u8, model: String, messages: u64, tokens: TokenUsage }` and, per session, `SessionStats { session: SessionId, rows: Vec<StatsRow> }`. The adapter folds each transcript into these rows; nothing per message is kept.
- `UsageStats { sessions: Vec<SessionStats> }` with methods, all pure and unit tested:
  - `in_range(range, today) -> RangeStats` where `Range` is `All | Last30Days | Last7Days` (a day counts when `today - 29 <= day <= today`, `today - 6 <= day <= today`).
  - `RangeStats` gives: `session_count` (sessions with at least one row in range), `message_count`, `total_tokens` (input + output + cache read + cache write; web searches are not tokens), `active_days`, `peak_hour` (hour with most messages, ties go to the earlier hour; `None` when empty), `favourite_model` (most tokens; `None` when empty), `models()` (per model: input, output, total, share of the total, ordered by share then name), `days()` (tokens per day per model, for the chart).
  - `heatmap(today) -> [[Option<u64>; 27]; 7]`: tokens per day for the 27 weeks ending with the week of `today`, Sunday first; days after `today` and days before the span are `None`. Always uses all sessions (the range does not apply).
- Days and hours are in the **local** time zone. The domain takes a `Day` value and an `hour`, never a time zone; the adapter converts.

**Port** (`forge-ports/src/usage_stats_port.rs`): `UsageStatsPort::load(&self) -> Result<UsageStats, UsageStatsError>` (`#[automock]` like the others).

**Adapter** (`forge-adapter-claude-fs`):

- Reads every `projects/*/*.jsonl` under the Claude home, one line at a time (no whole-file strings).
- Per file it counts what `UsageCollector` counts, with the same rules (one usage per `message.id`, the last line winning; no sidechain, meta or `<synthetic>` lines), but also records each assistant message's `timestamp` (the first line carrying that id wins for the time) so its tokens land in a (day, hour, model) row. A message's **count** is one per `message.id` for assistant lines and one per user line that carries text (not tool results, not meta). Web-search requests are added to the row of the model before them, as `UsageCollector` does.
- A cache keyed by path holds each file's result with its (modified time, size). A file whose key is unchanged is not read again; a deleted file drops out. The cache lives in the adapter for the life of the app. The first call reads everything; later calls read only what changed.
- A line that is not JSON, or a file that cannot be read, is skipped (and the file counted as read as far as it went); the call fails only when the projects folder cannot be listed for a reason other than "does not exist" (that is an empty `UsageStats`).
- It runs in `spawn_blocking` / on the tokio file API; nothing blocks the UI thread.

**Application**: `Services` gets the port; a `Workspace::usage_stats()` returns a resource-like state `Loading | Ready(Arc<UsageStats>) | Failed(String)`, started when the new-session screen mounts and refreshed each time it mounts again (the cache makes that cheap).

### B. The dashboard card (`forge-widgets/src/usage_card.rs`)

- Local state: tab (overview/models) and range, both defaulting as in the app and reset when the screen is shown again.
- States: Loading shows the card frame with "Loading…"; Failed shows the message; no sessions at all (`session_count` over all is 0) hides the card.
- Overview: the six tiles, then the heatmap. Tokens are shortened with the existing `format_tokens` (k, M, B). Peak hour as "9 PM" (Forge's UI is English). Heatmap cells are 4 levels by the day's share of the busiest day (`< 25 %`, `< 50 %`, `< 75 %`, rest) plus an empty cell colour; a `title` on each cell reads "Oct 3: 658M tokens".
- Models: an inline SVG stacked bar chart. Bars: one per day in range that has tokens; for "7d" always seven columns; for "30 days" and "All", the span from the first day with tokens to today, bars only where there is data, x labels about every fifth day. Model colours: shades of one hue ordered by share (the app's look), taken from CSS variables so light and dark themes both work. The legend has one row per model with a colour square, name (`model_label`), "<in> in · <out> out", and the share to one decimal.
- All colours and spacing from `forge-shared` theme tokens; no new global styles outside the card's own classes.

### C. The new-session composer

- **Layout** (top to bottom): greeting "What's next?" (the app's "다음 일정은 무엇인가요?"), the card, then the composer block pinned toward the bottom: chips row, box, controls row.
- **Chips row**: `Local` (static; the app's other environments are cloud features), the **folder chip**, and the add-folder button (opens the same native folder picker as "Open folder…" does today).
- **Folder chip menu** (replaces the `Select`): heading "Recent", the project folders from `new_session_projects()` in the order Forge already lists them, the current one checked, a divider, "Open folder…". It opens upward. "No folder" is not offered: a Forge session needs a folder.
- **Box**: the existing `CommandTextArea` and its send path. The "Start" button becomes a return-arrow icon button, grey when the box and attachments are empty, dark otherwise; it still calls `submit`.
- **Controls row**: the `+` menu (`PlusMenu`, unchanged), a **mode picker** and, at the right, a **model picker**. They use the generic `Picker` with draft state held by `NewSessionComposer`: `mode` (as today) and `model: Option<String>` (None means "the saved default"). The mode picker has the same five rows and the bypass confirmation as `ModePicker`; the model picker has the same rows as `ModelPicker` (`model_menu_rows`). `start_new` takes the chosen model; when it is `None` it reads the saved model as it does now. Nothing is saved to settings by choosing here.
- The Settings page keeps the default model setting.

## Left out

- Microphone and "hold to record" (CMP-02 stays `-`); the book caption; the pixel creature.
- "No folder", other environments (cloud, remote control, SSH) and their submenus; the chip stays "Local".
- The effort picker on the new-session screen: its stops come from the running SDK, which does not exist yet. It stays on the open session.
- The circle with plan limits and cloud credit (CMP-07 stays won't: the numbers come from the account).
- Hover tooltips on bars (cells use `title` only), a dashboard on the empty-session screen, per-project dashboards.
- "More models" submenu (seen: Sonnet 5, Opus 5, Fable 5, Opus 4.8, 4.7, 4.6, Sonnet 4.6) is not added; Forge's model rows are unchanged. This stays in CMP-04.

## Open points checked before building

- Whether total tokens in the app include cache tokens could not be seen. Plan 24 includes them (the user chose this). The models legend shows input and output only, as the app does.
- The app's heatmap shading rule was not seen; the 4 levels above are Forge's own.
- The cost of the first full scan on the user's real `~/.claude` (the app reports 14k messages) must be measured; if it is over about 2 s, the card still shows "Loading…" without blocking the rest of the screen, and a persisted cache becomes a follow-up.

## Testing

- **Domain** (unit): range edges (day 29 and 30 ago, 6 and 7 ago), empty stats, ties for the peak hour and the favourite model, token sums with cache, shares that add to 100 within rounding, heatmap shape for a `today` that is a Sunday and one that is a Saturday, days after today.
- **Adapter** (with temp `.claude` folders): duplicate `message.id` lines count once; sidechain, meta, synthetic and tool-result lines are skipped; time zone conversion puts a 23:30 UTC message on the next local day (test with an injected offset); a broken line is skipped; the cache does not re-read an unchanged file (a counter) and does re-read a changed one; a missing projects folder gives empty stats.
- **Features / widgets**: the folder menu rows, the draft mode and model reaching `open_new`, the send icon state; the card's states (Loading, Failed, hidden, Ready) through the existing render approach in `forge-widgets`.
- **By eye**: run the app through `scripts/harness/launch.sh` with a fixture `FORGE_CLAUDE_HOME` that has a few days of transcripts, and compare with the observations; also with the real `~/.claude` to time the first scan.
