# Details panel cards: `컨텍스트`, `커넥터`, `변경 사항` (Plan 55)

Date: 2026-10-23. Follows `2026-10-22-environment-card-design.md` (the panel and the `환경` card) and `2026-10-22-card-folder-items-design.md`. What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 26. Local only: no cloud, SSH, Slack or Remote Control anywhere, not even disabled.

## Why

The desktop app's details panel is a stack of cards. Forge has the first, `환경`. The user asked on 2026-10-23 to add the three that follow it: `컨텍스트`, `커넥터` and `변경 사항`. Stage B had left them out because Forge already has a context ring, a Connectors page and a Changes panel; the user has now decided to add them, so each card is a summary that opens the existing feature.

## Goals

1. `컨텍스트` card: the percentage and tokens used, a bar, and the breakdown by category.
2. `커넥터` card: the session's MCP servers with their state; a click opens a table of them.
3. `변경 사항` card: the count of changed lines and a click that opens the Changes panel.
4. Every card folds like `환경` does, per session.

## Non-goals and anti-goals

- `결과물` card (files a session made). Not asked for. Put it in `backlog.md`.
- The card's `변경 사항 커밋` button and the chat's `Commit b.txt?` question: Forge has no commit prompt yet (Plan 49 was never built). Not built.
- Sign-in, reconnect or connect buttons in the connector rows and table (`인증`, `다시 연결`, `연결`): the state is shown as text only. Sign-in stays on the Connectors page (Plan 30D). `커넥터 관리` opens that page.
- Reordering cards by their drag handle (the desktop app shows six dots on an expanded card and lets you drag it; whether the order is kept was not checked). Cards keep the fixed order.
- A link from `Custom agents` (Forge has no agents page) and from `MCP server instructions` (its target was not read).
- No new place that edits connectors or environments. Anti-goal: a second connector list that disagrees with the SDK's; the card shows what the running session reports, not the config file.

## What the desktop app does (observed, Round 26)

- Order: `환경`, `컨텍스트`, `커넥터`, (`결과물`), `변경 사항`. Each header is `<name> ▼`, one line when folded.
- `컨텍스트` before any message: `컨텍스트 사용량을 확인하려면 메시지를 보내세요.` After a turn: `8% 사용됨` and `78.1k / 1M`, a thin stacked bar, then one row per category: colour square, name, tokens, percent (`Messages 20.1k 2.0%`, `System tools`, `MCP tools`, `System prompt`, `Skills`, `MCP server instructions`, `Custom agents`, `Autocompact buffer 33k 3.3%`, `Free space 888.9k 88.9%`), a gap, the deferred rows (`MCP tools (deferred) 123.9k —`, tokens and a dash for the percent), then `MCP 도구 248` and `커스텀 에이전트 3`, each a fold row that opens a list of `<server> · <tool>` (agents: `<name>`) with its tokens. Folded, the card is `컨텍스트 ▶`, `8%` and a small usage bar (not a switch). The underlined row names are links with hover help: `MCP tools` → `커넥터 관리`, `Skills` → `Skills 관리` (the Settings window on its Skills page), `Custom agents` → `사용자 지정 에이전트 보기`.
- `커넥터` lists letter-icon rows with the state at the right as brown text for the ones that need action (`인증`, `다시 연결`), three rows then `더 보기` (`간략히 보기` when open). A click on a row opens the modal `MCP 서버` (folder path, the table `서버` / `유형` / `상태`, footer `커넥터 관리` and `완료`).
- `변경 사항`: one row with a diff icon, green `+N`, red `−N` for all uncommitted changes together (not a row per file); a click opens the Changes panel.
- `결과물` (seen, not built): a row per file the session made or edited, newest first; hover shows the full path; a click opens the Files panel.

## The design

### Data

All three come from the running session, not from files.

- **Context detail:** the sidecar already asks the SDK for `getContextUsage()` after each turn and sends only the total and the window. It now also sends the categories (`name`, `tokens`, `kind` of `used`/`free`/`buffer`/`deferred`), the MCP tools (`server`, `name`, `tokens`) and the custom agents (`name`, `tokens`), in a new event `context_detail`. Classify rows by `kind`, never by the English name. The ring keeps using `context_usage`.
- **MCP state:** the sidecar calls `mcpServerStatus()` after each turn (same place, same timeout rule) and sends `mcp_status`: per server `name`, `status` (`connected`, `failed`, `needs-auth`, `pending`, `disabled`), `scope`, `source`, `tool_count`, `error`. A failure or timeout sends nothing; the card then keeps the last value, or its loading row if there never was one. (A session that has had no turn has no value: the card says so instead of loading forever. Observed: the desktop app loads forever in a dead session; do not copy that.)
- **Changes count:** from git, not from the model: the uncommitted changes of the session's folder, counted with the call the Changes panel uses, refreshed when a tool call finishes (`finished_tool_calls`), as the panel does. No count (card hidden) when the folder is not a git repository, has no changes, or the read fails.

### The cards

- Order: `환경`, `컨텍스트`, `커넥터`, `변경 사항`. Each folds on its own; fold state is per session (replace the single `card_folded` flag by a flag per card).
- **Context card:** header `컨텍스트`; before a value, the grey sentence above; with a value, `{percent}% used` and `{used} / {max}`, the bar (segments by `used` rows, widths by tokens), the used rows, free, buffer, a gap, the deferred rows with `—`, then the two fold rows `MCP tools` and `Custom agents` when the lists are not empty: the count at the right, a click opens the list (a row per item: `<server> · <name>` or the agent's name, tokens), each fold local to the card. Folded, the card is the header, `{percent}%` and a small bar (a track with the used part filled); no switch.
- Link rows: `MCP tools` opens the Customize page on its Connectors tab (`Manage connectors`), `Skills` opens it on its Skills tab (`Manage Skills`); both as a hover `title`. `Custom agents` and `MCP server instructions` are plain text. Colours from the usage ring's palette (`theme.css`), light and dark. The bar and row maths live in one pure function so the percent column adds up (rounded; the free row may differ by a tenth).
- **Connector card:** rows in this order: servers that need action first (`needs-auth`, `failed`), then the rest in the SDK's order, `claudeai`-scope servers last; first three, then `더 보기` / `간략히 보기`. Row: a letter icon (first letter, uppercase), the name (first letter uppercase as the desktop app shows it), and at the right the state text for `needs-auth` (`Sign in`), `failed` (`Reconnect`), `pending` (`Connecting`), `disabled` (`Off`); nothing for `connected`. A click opens the modal.
- **Modal:** title `MCP servers`, the session's folder, the table `Server` / `Type` / `Status`. Type: `Web` for a remote server, `Desktop` for a local one (decide from `config.type`: `stdio` is local; `http`, `sse` and `claudeai-proxy` are remote); a tag after it: `Plugin` when `source` is `plugin`, `User` when `scope` is `user`, `App` otherwise only if `source` says so (an unknown source shows no tag). Status: a check for `connected`, the state text otherwise. Footer: `Manage connectors` (opens the Customize page on its Connectors tab, closes the modal) and `Done`. The row under the server's name shows `{plugin} plugin` text only when `source` is `plugin`, as the desktop app does (builder's call: skip it if the plugin's name is not in the status).
- **Changes card:** hidden when there is no count. Row: the diff icon, `+{added}` in green and `−{removed}` in red. A click opens the Changes panel in the dock (reusing the toolbar's toggle; if it is already open, it stays open).

### Where the code goes

- `forge-ports` / sidecar TypeScript: the two events. `forge-domain`: `ContextBreakdown`, `ContextRow`, `ContextKind`, `McpServerState`, `McpStatus`. `forge-application`: `LiveSessionView` keeps the last of each. `forge-entities`: pure view functions for the context card and connector rows. `forge-features`: the changes count and the per-card fold. `forge-widgets`: the cards and the modal.

## What is left

`결과물` card (observed in Round 26 part 2), the commit button and prompt, sign-in or reconnect from the card, reordering cards by hand, a link from `Custom agents`, Windows.

## As built

- **Events:** the sidecar sends `context_detail` (categories with `name`, `tokens`, `kind`; MCP tools; agents) and `mcp_status` (per server `name`, `status`, `scope`, `source`, `type`, `tool_count`, `error`) after each turn. Token counts are rounded and a row with a missing or non-string server or agent name is dropped, because one line that fails serde ends the session. The port's structs are `ContextItemReport` and friends so they do not clash with the domain `ContextItem`; the domain has `ContextBreakdown`, `ContextRow`, `ContextKind`, `McpServerState`, `McpStatus`, and `LiveSessionView` keeps the last of each. A breakdown with no rows counts as no value, and the last good breakdown is kept (the keep-last-breakdown rule); an empty server list is a real value.
- **Timeouts:** the ring keeps its 3 s wait. The breakdown and the server states wait 10 s (their own constant) and write one line to stderr when a call times out or fails.
- **Pure views** (`forge-entities/src/details_view.rs`): `context_card` (percent in tenths, bar segments that never add to more than 1000, used rows, free, buffer, deferred with a dash) and `connector_rows` / `connector_table` (servers needing action first, `claudeai` scope last). The `mcp__<server>__` prefix is dropped from an MCP tool's label, also when the SDK normalized the server name.
- **Features** (`forge-features/src/details.rs`, `workspace.rs`): a fold flag per card and per session (`DetailsCard`), the change counts from git (`session_change_counts`, refreshed when a tool call finishes, hidden when there is no count), and a one-shot Customize tab request (`OneShot<T>`) so `Manage connectors` and the `MCP tools` / `Skills` rows open Customize on the right tab once.
- **Widgets:** the cards in `details_panel.rs` and the modal in `mcp_servers_dialog.rs` (title `MCP 서버`, columns `서버` / `유형` / `상태`, a check for a connected server, `커넥터 관리` and `완료`).
- **Wording and tags:** the state text for `needs-auth` is `Authenticate` / `인증`; the `App` tag was dropped (an unknown source shows no tag). The `MCP tools` and `Skills` context rows are matched by the SDK's English name for their links (the rows themselves are classified by `kind`), kept in named constants.
- **Window-check fixes:** the MCP tool row drops the SDK prefix (7cd0155), a connected server shows a check in the table (523e9a0), and `launch.sh` rebuilds a sidecar build older than its sources (063fb02).

## Checked

In the real window on macOS, light mode, in a throwaway folder with 5 real turns (and one SDK probe) and the fake connector servers of the harness fixture:

- Before a message: the two grey sentences, no changes card; each card folds on its own.
- After a turn: `1% 사용됨`, `11.9k / 1M`, the bar, rows adding to 100.0, the deferred rows, the `MCP 도구` fold row and its list (after the prefix fix), the folded form.
- Connectors: failed servers first, `더 보기` / `간략히 보기`, the modal over the whole window (also with the floating dock), Escape closes it, `커넥터 관리` lands on Customize, tab `커넥터`.
- Changes: `+1 −0`, then `+3 −1`, equal to the chat's counts; no card for a clean worktree.
- Switching to another session and back kept the values.

Second window check (macOS, 4 more real turns in throwaway folders under /private/tmp):

- Non-git folder: after a turn the `컨텍스트` and `커넥터` cards show values (`1% 사용됨`, `11.5k / 1M`, `커넥터 없음`), there is no `변경 사항` card and no error text.
- `커스텀 에이전트` row: in a git folder with `.claude/agents/demo-agent.md` the card shows a fold row `커스텀 에이전트` with `1`, expanded `demo-agent` `18` as plain text.
- `Skills` link row: the underlined `Skills` row appeared (`17`, `0.0%`) with an enabled user skill; clicking it opens Customize on `스킬`. `커넥터 관리` from the `MCP 서버` modal switched Customize from a remembered `스킬` tab to `커넥터`. The SDK listed the enabled stdio server's tools only as the `MCP 도구` fold row (deferred), so no `MCP tools` link row came up; it is covered by unit tests. A native hover title does not show in a screenshot, so the `Manage Skills` title was read from the source only.
- Changes card: clicking `변경 사항` (`+1 −0`) opens the Changes panel with `y.txt +1 −0`; clicking it again keeps it open.
- Dark mode (Settings > 테마 > 다크): the three cards, the bar, the rows and the `MCP 서버` modal read well. One defect, fixed (09cc5f4): the grey squares and the bar track were nearly invisible; both now use a mix of the text colour.
- A relaunched app showed the cards again after the next turn without a crash (new session; the earlier sessions are not in the fixture's Claude home).

## Left

- The underlined `MCP tools` link row live (the SDK shows deferred MCP tools as a fold row; unit tests cover the rule), the hover titles, and the click while Customize is the open page (the cards are on the session page, so that state cannot be reached from a card).
- Dark mode: the brown `인증` / `다시 연결` state texts (no needs-auth server to show them).
- Windows.
- The grey-cards flake: once both cards stayed on the grey sentences after a turn and did not come back in a later identical run. It was not reproduced; the likely cause was the shared 3 s timeout, which is now 10 s for these two calls and leaves a stderr line when it fires.
- The count does not refresh after a folder change or after edits made outside tool calls; the live list changing causes extra git reads.
- No focus trap in the modal.
- A plugin subtitle under a server's name never shows (the status has no plugin name).
- The `결과물` card, the commit button, sign-in or reconnect from the card, reordering cards by hand, a link from `Custom agents` (backlog OTH-14 to OTH-18).
