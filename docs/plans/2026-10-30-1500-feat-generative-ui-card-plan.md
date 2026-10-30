---
title: "feat: generative-UI card in Forge chat"
type: feat
date: 2026-10-30
artifact_contract: ce-unified-plan/v1
product_contract_source: ce-brainstorm
execution: code
---

# feat: generative-UI card in Forge chat

## Goal Capsule

- **Objective:** In a Forge session, Claude can answer with a small interactive page (calculator, chart, 3D explainer, map) that appears in the conversation, works there, and can hand a chosen value back to Claude as the next message.
- **Means:** A sidecar tool `generate_sandboxed_ui` (in-process MCP, same server as the turn tools) whose call is read from the transcript into a chat card holding a script-enabled sandboxed iframe (KTD1, KTD2).
- **Authority:** User decisions (below) > this plan > OpenIntelligentUI's behaviour as inspiration. OpenIntelligentUI is a pattern source, not code to run.
- **Stop conditions:** Stop and report if U1 shows a model-authored script can still reach Forge's IPC after the guard (KTD2) on macOS; do not ship scripts-on frames without the guard.
- **Execution profile:** one PR, macOS verified; Windows unverified and must be said so.

## Product Contract

### Summary
Port the idea of OpenIntelligentUI's `generateSandboxedUi` into Forge. The web app itself, its Python agent, OpenAI/Jev keys, its MCP server and its A2UI table component are out of scope.

### Settled decisions (user-directed)
- SD1: A card inside the Forge chat using the existing Claude sidecar. Rejected: embedding the whole web app in a panel; MCP-only connection. Reason: no new server or provider keys.
- SD2: Render plus a follow-up bridge. Rejected: render-only; render+bridge+native A2UI tables. Reason: the user wants interactivity without table-component scope.

### Requirements
- R1. Claude has a tool `generate_sandboxed_ui` with parameters in this order: `initialHeight`, `placeholderMessages`, `css`, `html`, `jsFunctions`, `jsExpressions`. (Order matters while streaming, so the card can draw early.)
- R2. A call becomes a card in the chat at the place of the call. While the parameters stream, the card shows the placeholder messages at the initial height; when `html` is complete it draws the page.
- R3. Each call is its own card; there is no patching of earlier cards.
- R4. The page cannot reach the network, the parent page, Forge's IPC, local files or the clipboard, and cannot navigate the window.
- R5. Inside the page, a call `forge.send(values)` (host bridge) shows a send control; on the person's click, validated values become a user message and start a new turn. The page cannot send without a real click on the card.
- R6. The card is restored when a session is reopened (drawn from the recorded call, no extra storage).
- R7. A setting `Generative UI` (default on) turns the tool on and off; Korean text for every new string.
- R8. A page that fails (script error, bad parameters) shows a short error in the card, never breaks the chat.
- R9. Claude is told when to use the tool: only when text would serve worse (explore, compare, make a tool).

### Scope boundaries
Out: A2UI-style native tables, the OpenIntelligentUI MCP server, a patch API, saving a page to a file, running pages in the Preview pane, Windows verification.

### Acceptance examples
- AE1 (R2,R6): Asking "bill splitter" produces a card with working total, tip and group-size controls; closing and reopening the session shows the same card.
- AE2 (R4): A page that runs `fetch('https://example.com')`, `window.parent.document`, `window.webkit.messageHandlers.ipc.postMessage('{}')` does nothing visible and Forge state is unchanged.
- AE3 (R5): Clicking the page's send control adds a user message with the chosen values and a turn starts; a script calling `forge.send` without a click does nothing.

## Planning Contract

### Evidence
- `sidecar/src/turn_tools.ts`: in-process MCP server `ccd_turn`, tools need `alwaysLoad: true` or the SDK defers them and the model never calls them (Plan 70). Haiku did not call those tools; the new tool needs the same real-SDK check on Opus and Haiku.
- `sidecar/src/session.ts:460` adds the server and allowed tool names when `command.turn_tools === true`; the flag travels `AppConfig` (`forge-domain/src/config.rs`) -> `SessionDefaults` (`session_defaults.rs`) -> `forge-adapter-sidecar/src/protocol.rs` -> `sidecar/src/protocol.ts`.
- `forge-entities/src/turn_cards.rs` and `turn_card_views.rs` read tool calls into cards (`Item::PlanList`, `Item::EndCard` in `turn_view.rs`, drawn in `turn.rs:178`); the calls are hidden from the folded activity line.
- `forge-widgets/src/files_kinds.rs:110-119`: the HTML file preview uses `sandbox=""` because scripts in any frame can call the app's IPC handler (Plan 28).
- `dioxus-desktop-0.7.10/src/webview.rs:308` accepts every IPC message and ignores the request URL. wry 0.53.5 on macOS (`wkwebview/class/wry_web_view_delegate.rs`) builds the request URL from the sending frame, so a frame check is possible at that one point.
- `forge-adapter-webview` (Preview) uses separate wry child views; they sit above the page and cannot be clipped by a scrolling chat, so they do not suit inline cards.

### Key technical decisions
- KTD1 (SD1): the tool lives next to the turn tools in the same in-process server, behind its own setting and start flag `generative_ui`, `alwaysLoad: true`. Its handler only validates the parameters and returns a short text; the card is drawn from the recorded call like `wrap_up`.
- KTD2 (R4): render in `<iframe sandbox="allow-scripts" srcdoc=...>` (opaque origin: no parent access, no storage, no same-origin requests) plus a Content-Security-Policy meta as the first element of the srcdoc (`default-src 'none'; script-src 'unsafe-inline'; style-src 'unsafe-inline'; img-src data:; font-src data:`, so no network, no navigation by form or link). The remaining hole is IPC: vendor `dioxus-desktop` through `[patch.crates-io]` with one change in the IPC handler, dropping a message whose request URL is not Forge's own (`dioxus://`, `http(s)://dioxus.`). Rejected: a separate child web view per card (cannot scroll/clip inside chat); scripts off (kills the feature); trusting `sandbox` alone (Plan 28 shows it does not stop IPC).
- KTD3 (R5): the bridge is a `postMessage` from the frame to its parent (`forge.send`, injected by the host as a small script that also tags the message with a per-card nonce held only by the host script). Because the frame has an opaque origin and the IPC patch drops frame messages, the parent receives it through a `message` listener in the Forge page; the listener accepts only `event.source` equal to that card's frame, a nonce match, `event.isTrusted` plus a recent real click inside the frame (the host script requires a `click` with `isTrusted` before posting), a values object of at most 20 string/number/boolean entries with keys and strings bounded, and then fills the composer with a text built by Forge (`"<label>: key=value, ..."`), never raw html. Values do not start with `/` or `!` (same rule as `wrap_up` buttons).
- KTD4 (R6): a pure function `generated_ui_of(call) -> Option<GeneratedUi>` in `forge-entities` reads the recorded call (like `end_card_of`); parameters are size-capped (html+css+js at most 200 KB) and the card is restored from the transcript.
- KTD5 (R2): streaming draw uses the partial tool input the SDK already streams (`includePartialMessages: true`); if the app does not expose partial inputs for tool calls, the card shows only the placeholder until the call is complete (deferred question for U3).
- KTD6 (R9): tool description in Forge's own words carries the rules; no copy of OpenIntelligentUI prompt text.

### Assumptions
- A1. wry on macOS reports the sending frame's URL for srcdoc frames as `about:srcdoc` (to verify in U1). If it reports the parent URL, KTD2 cannot discriminate and this plan stops (see Goal Capsule).
- A2. Windows (WebView2) reports the frame source differently; unverified, so the feature stays macOS-verified and the PR says so.

### Deferred to implementation
- Exact message shape of the nonce handshake; card chrome (header, expand, copy source); dark-mode styling of the page (inject `color-scheme` and Forge colour variables).

## Implementation Units

### U1. IPC guard spike and patch (R4)
- **Goal:** prove or disprove KTD2 before anything else is built.
- **Files:** `Cargo.toml` (`[patch.crates-io]` dioxus-desktop), `vendor/dioxus-desktop/src/webview.rs` (one check), a throwaway harness page.
- **Approach:** vendor 0.7.10 unchanged, add the URL check to the IPC closure, run a srcdoc frame with `sandbox="allow-scripts"` that tries `window.webkit.messageHandlers.ipc.postMessage` with a valid event JSON and watch Forge state; log the URL wry reports.
- **Tests:** a unit test for the allow-list function (accepts the app origins, rejects `about:srcdoc`, `null`, `data:`, `blob:`, `https://evil`).
- **Verification:** AE2 in the real window; Forge's own clicks still work.

### U2. Sidecar tool and setting flag (R1, R7, R9)
- **Files:** `sidecar/src/generative_ui.ts` (new, or extend `turn_tools.ts`), `sidecar/src/session.ts`, `sidecar/src/protocol.ts`, `forge-domain/src/config.rs`, `session_defaults.rs`, `forge-adapter-sidecar/src/protocol.rs`, `forge-features/src/settings.rs`, `forge-widgets/src/settings_overlay.rs`, `forge-shared` `ko.rs`.
- **Approach:** zod schema in the R1 order, size caps, `alwaysLoad: true`, allowed tool name added when `generative_ui` is on; setting `Switch` row modelled on `turn_tools`.
- **Tests:** vitest (tool listed, `_meta['anthropic/alwaysLoad']`, bad parameters return a readable error, options carry the server/allowed name only when on); Rust config default-on for an old file, protocol JSON, `every_sentence_in_the_sources_has_a_korean_text`.

### U3. Card model from the transcript (R2, R3, R6, R8)
- **Files:** `forge-entities/src/generated_ui.rs` (new), `turn_view.rs` (`Item::GeneratedUi`, hide the call from the folded line and count), `lib.rs`.
- **Approach:** `generated_ui_of(call)` with caps and a `Pending`/`Ready`/`Invalid` state; partial input handling per KTD5.
- **Tests:** parameter order independence, caps, missing html -> Pending then Invalid when the turn ends, tool prefix `mcp__ccd_turn__`/own server name, hidden from folded line, reopening a transcript yields the card.

### U4. Card view, sandbox and bridge (R2, R4, R5, R8)
- **Files:** `forge-widgets` or `forge-entities/src/generated_ui_view.rs` (new), `turn.rs` draw arm, css, Dioxus page script for the `message` listener, composer fill hook (existing `on_send` path used by end card).
- **Approach:** build the srcdoc (CSP meta, theme variables, host bridge script, css, html, jsFunctions, then jsExpressions); placeholder state; error line; the listener per KTD3 with every check; `forge.send` fills the composer or sends per the end-card convention.
- **Tests:** srcdoc builder (CSP first, parameter order, `</script>` and `</style>` in model text cannot break out of their element, size caps); bridge validator (wrong source, wrong nonce, no click, too many keys, long values, leading `/` or `!`, non-object).

### U5. Real checks and samples (AE1-AE3)
- **Files:** `scripts/harness/add_generated_ui_samples.py`, spec `docs/superpowers/specs/2026-10-30-generative-ui-design.md`.
- **Approach:** sample sessions (bill splitter, hostile page for AE2); window check light and dark; one real SDK turn on Opus 5.5 and on Haiku 5.5 to see the model calls the tool (copy config with api_key null, per Plan 70 note); reopen check.
- **Verification:** screenshots, state of Forge unchanged after the hostile page, report on Haiku compliance.

## Verification Contract
- `cargo test --workspace` (known flaky plugin test `round_trip` noted), `cargo clippy --workspace`, `cargo fmt --check`.
- `cd sidecar && npm test` (vitest) and typecheck.
- Real window via the harness (see `scripts/harness`), macOS only; fresh worktree needs `sidecar/node_modules` symlinked.

## Definition of Done
- R1-R9 met; AE1-AE3 checked in the real window; U1 spike result recorded in the spec; every new string has a Korean entry; Windows and Haiku compliance reported honestly as unverified if so.
