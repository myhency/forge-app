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
- The expandable `MCP 도구` / `커스텀 에이전트` fold rows of the context card: shown as plain count rows (not observed to expand; builder's call stays plain).
- Hover help on the underlined context rows; the folded `컨텍스트` row's switch; the card drag handle.
- No new place that edits connectors or environments. Anti-goal: a second connector list that disagrees with the SDK's; the card shows what the running session reports, not the config file.

## What the desktop app does (observed, Round 26)

- Order: `환경`, `컨텍스트`, `커넥터`, (`결과물`), `변경 사항`. Each header is `<name> ▼`, one line when folded.
- `컨텍스트` before any message: `컨텍스트 사용량을 확인하려면 메시지를 보내세요.` After a turn: `8% 사용됨` and `78.1k / 1M`, a thin stacked bar, then one row per category: colour square, name, tokens, percent (`Messages 20.1k 2.0%`, `System tools`, `MCP tools`, `System prompt`, `Skills`, `MCP server instructions`, `Custom agents`, `Autocompact buffer 33k 3.3%`, `Free space 888.9k 88.9%`), a gap, the deferred rows (`MCP tools (deferred) 123.9k —`, tokens and a dash for the percent), then `MCP 도구 248` and `커스텀 에이전트 3`.
- `커넥터` lists letter-icon rows with the state at the right as brown text for the ones that need action (`인증`, `다시 연결`), three rows then `더 보기` (`간략히 보기` when open). A click on a row opens the modal `MCP 서버` (folder path, the table `서버` / `유형` / `상태`, footer `커넥터 관리` and `완료`).
- `변경 사항`: a diff icon, green `+N`, red `−N`; a click opens the Changes panel.

## The design

### Data

All three come from the running session, not from files.

- **Context detail:** the sidecar already asks the SDK for `getContextUsage()` after each turn and sends only the total and the window. It now also sends the categories (`name`, `tokens`, `kind` of `used`/`free`/`buffer`/`deferred`) and the counts of MCP tools and custom agents, in a new event `context_detail`. Classify rows by `kind`, never by the English name. The ring keeps using `context_usage`.
- **MCP state:** the sidecar calls `mcpServerStatus()` after each turn (same place, same timeout rule) and sends `mcp_status`: per server `name`, `status` (`connected`, `failed`, `needs-auth`, `pending`, `disabled`), `scope`, `source`, `tool_count`, `error`. A failure or timeout sends nothing; the card then keeps the last value, or its loading row if there never was one. (A session that has had no turn has no value: the card says so instead of loading forever. Observed: the desktop app loads forever in a dead session; do not copy that.)
- **Changes count:** from git, not from the model: the uncommitted changes of the session's folder, counted with the call the Changes panel uses, refreshed when a tool call finishes (`finished_tool_calls`), as the panel does. No count (card hidden) when the folder is not a git repository, has no changes, or the read fails.

### The cards

- Order: `환경`, `컨텍스트`, `커넥터`, `변경 사항`. Each folds on its own; fold state is per session (replace the single `card_folded` flag by a flag per card).
- **Context card:** header `컨텍스트`; before a value, the grey sentence above; with a value, `{percent}% used` and `{used} / {max}`, the bar (segments by `used` rows, widths by tokens), the used rows, free, buffer, a gap, the deferred rows with `—`, then the two count rows when the counts are above zero. Colours from the usage ring's palette (`theme.css`), light and dark. The bar and row maths live in one pure function so the percent column adds up (rounded; the free row may differ by a tenth).
- **Connector card:** rows in this order: servers that need action first (`needs-auth`, `failed`), then the rest in the SDK's order, `claudeai`-scope servers last; first three, then `더 보기` / `간략히 보기`. Row: a letter icon (first letter, uppercase), the name (first letter uppercase as the desktop app shows it), and at the right the state text for `needs-auth` (`Sign in`), `failed` (`Reconnect`), `pending` (`Connecting`), `disabled` (`Off`); nothing for `connected`. A click opens the modal.
- **Modal:** title `MCP servers`, the session's folder, the table `Server` / `Type` / `Status`. Type: `Web` for a remote server, `Desktop` for a local one (decide from `config.type`: `stdio` is local; `http`, `sse` and `claudeai-proxy` are remote); a tag after it: `Plugin` when `source` is `plugin`, `User` when `scope` is `user`, `App` otherwise only if `source` says so (an unknown source shows no tag). Status: a check for `connected`, the state text otherwise. Footer: `Manage connectors` (opens the Customize page on its Connectors tab, closes the modal) and `Done`. The row under the server's name shows `{plugin} plugin` text only when `source` is `plugin`, as the desktop app does (builder's call: skip it if the plugin's name is not in the status).
- **Changes card:** hidden when there is no count. Row: the diff icon, `+{added}` in green and `−{removed}` in red. A click opens the Changes panel in the dock (reusing the toolbar's toggle; if it is already open, it stays open).

### Where the code goes

- `forge-ports` / sidecar TypeScript: the two events. `forge-domain`: `ContextBreakdown`, `ContextRow`, `ContextKind`, `McpServerState`, `McpStatus`. `forge-application`: `LiveSessionView` keeps the last of each. `forge-entities`: pure view functions for the context card and connector rows. `forge-features`: the changes count and the per-card fold. `forge-widgets`: the cards and the modal.

## What is left

`결과물` card, the commit button and prompt, sign-in or reconnect from the card, expanding `MCP 도구`, hover help, Windows, the card's order with several changed files.
