# Plan mode: the plan panel and the plan card (Plan 56A)

Date: 2026-10-24. What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 27 (task 2) and Round 29 (the checks of 2026-10-24 that this spec adds). Backlog row `NEW-01`.

## Why

In plan mode Claude writes a plan and then asks to leave plan mode with the tool `ExitPlanMode`. The desktop app shows the plan, rendered, in a side panel and asks `수락 / 수정… / 거부`. Forge shows the generic permission card `Claude가 ExitPlanMode을(를) 사용하려고 합니다` with `거부 / 항상 허용 / 한 번 허용`. The plan text is nowhere in the window, so a person has to accept or refuse a plan they cannot read. The plan text is already in the request: the sidecar's `permission_request` carries `tool_name: "ExitPlanMode"` and `input.plan` (the whole markdown plan; seen in a real transcript).

## Goals

1. When Claude proposes a plan, the plan is shown rendered in a `계획` panel of the right dock, opened by itself.
2. A plan card replaces the permission card for `ExitPlanMode`, with the desktop app's four choices: `거부`, `수정…`, `수락`, `수락 및 자동 모드`, and a `⌄` menu on the last with `수락 및 편집 허용` and `수락 및 자동 모드`.
3. Each accept choice also sets the session's permission mode (below), so the next step runs in the mode the person chose.
4. `수정…` opens an input in the card, "무엇을 변경해야 하나요? (선택사항)", with `뒤로` and `수정`; `수정` refuses the plan and sends the text to Claude as the reason, so Claude revises the plan.
5. After a decision the card folds to one row `계획 제안됨 ▸` (the desktop app does the same); the panel stays open.

## Non-goals and anti-goals

- **Comments on selected text in the plan** (the desktop app shows a `댓글 추가` box and keeps the comment as a quote chip in the card): Plan 56B.
- The sidebar tooltip for a session with a plan ready (`플랜 준비됨 · Ready to start?`, the plan's first lines, `승인하려면 세션을 여세요`): Plan 56B.
- Editing the plan text by hand, saving it, or opening the plan file from the panel's folder icon in Forge's own editor: not built. The panel is read-only. (The desktop's panel has icons: open folder, copy, expand, close. Forge builds copy, expand and close; the folder icon is left out.)
- The `생성됨 <plan>.md +35` row in the chat: Forge already shows a created file in the turn's fold; not changed.
- Anti-goal: the plan card must never decide for the person. No key, timer or focus rule may accept a plan. `Esc` opens `수정…`; it does not refuse.
- Anti-goal: a plan accepted through `거부`/`수정` must not leave the session stuck in plan mode with no way to talk; after `수정` Claude is still in plan mode and writes a new plan.

## What the desktop app does (observed)

- Chat: the plan file shows as `생성됨 wordstat-py-top-n-hashed-walrus.md +35`, then a row `계획 제안 중 ▸` while the request is open.
- Card above the composer, title `Claude가 계획을 제안했습니다`, a link `계획 열기` at the right; buttons: `거부`, `수정… [Esc]`, a gap, `수락 [⇧ ⌘ ⏎]`, and the dark primary `수락 및 자동 모드 [⌘ ⏎] | ⌄`. The `⌄` menu: `수락 및 편집 허용`, `수락 및 자동 모드`.
- **`수락`** switches the session to `수동` (the composer's mode shows `수동` and the next file write asks for permission; seen). **`수락 및 편집 허용`** is the accept-edits mode (from its name; not clicked). **`수락 및 자동 모드`** is `자동` (seen in Round 27's mode list; the button was not clicked).
- **`수정…`**: the card's buttons are replaced by a text area (placeholder `무엇을 변경해야 하나요? (선택사항)`), `뒤로` at the left and a dark `수정` at the right. Not sent in the checks (the desktop session was left on it); what Claude receives is assumed to be the refusal with this text.
- Plan panel (right dock, title `계획`): a hint line at the top, the plan rendered as markdown (headings, bullets, inline code, numbered list), icons at the right: open folder, copy, expand, close. The panel opens on its own when the plan is proposed. After `수락` the hint changes to `계획 모드에서 계획에 대해 의견을 남기거나, Claude가 승인을 요청할 때 의견을 남길 수 있습니다` and the panel stays.
- After `수락` the card folds to `계획 제안됨 ▸`.

## The design

### Data (sidecar and wire)

- `permission_request` for `ExitPlanMode` already arrives with `input: { plan: string }`. No new event.
- A refusal can carry a reason. `permission_response` gets an optional `message` (string). The sidecar's `decisionToResult` uses it as the `deny` message when present and non-empty, otherwise today's text. Both the Rust and the TypeScript protocol types change together; an older parent that omits `message` keeps today's behaviour.
- Accepting also changes the mode. Two ways; the builder checks the real SDK (the harness drives the real one) and keeps the first that works:
  1. The sidecar answers `allow` with `updatedPermissions: [{ type: "setMode", mode, destination: "session" }]`. The wire's `permission_response` then carries the chosen mode.
  2. As the existing `Allow and switch to Auto mode` menu item does: answer `allow_once`, then send `set_permission_mode`. Forge's own mode state is set at the same time, so the composer shows the new mode.
  Either way the composer's mode shows the chosen mode after the click, and the mode the person chose is the one a restart restores (the session record already keeps `permission-mode`).
- Mapping: `수락` → `default`, `수락 및 편집 허용` → `accept_edits`, `수락 및 자동 모드` → `auto`.

### Entities (`forge-entities`)

- `permission_view` gets `plan: Option<PlanAsk>` (like `workflow`), set when `tool_name == "ExitPlanMode"` and `input.plan` is a non-empty string; `PlanAsk { text: String }`. When `input.plan` is missing or empty the card falls back to the generic permission card (a malformed request must still be answerable).
- `plan_key(code, shift, meta, other_modifier) -> Option<PlanKey>`: `Esc` and the `Digit` keys of the card do **not** refuse. `Esc` → `Revise`; `⌘↩` → `AcceptAuto`; `⇧⌘↩` → `Accept`. No digit keys (the desktop card shows none). Keys are matched by physical key (`code`), never the character (Korean input method; see the permission card's tests).

### Panel (`forge-features`, `forge-widgets`)

- `Panel::Plan` in the dock (title `Plan` / `계획`). It is per session. The plan text lives in the session's view state (`plan: Option<String>`), set when an `ExitPlanMode` request arrives and kept after the decision, cleared when a newer plan replaces it or the session closes.
- It opens by itself when the request arrives, unless the person has the window too narrow for a dock (the responsive rule from Plan 26 applies as for any dock panel). If the person closed it, a later plan opens it again (a new plan is new information).
- Body: `render_markdown(plan)` in the same look as the chat's markdown; the hint line; header icons `copy` (copies the plan text), `expand`, `close`.
- Selecting text does nothing special in 56A.

### Card (`forge-widgets/src/permission_panel.rs`)

- When `view.plan` is set the panel renders the plan card instead of the permission body: title `Claude proposes a plan` (ko `Claude가 계획을 제안했습니다`), a link `Open plan` (ko `계획 열기`) that opens the panel, and the buttons above. States: `Choose` and `Revise` (the text area). `뒤로` returns to `Choose`; `수정` with an empty text refuses with the default reason.
- The `⌄` menu follows the existing `perm-menu` look.
- Focus: as the permission card does (`take_focus_unless_typing`).
- After the answer, the card is gone (the request is answered) and the chat shows the row `Plan proposed ▸` (ko `계획 제안됨`) at the place of the request, which opens the panel.

### Texts

All new strings go through `tr!` with Korean entries in `forge-shared/src/ko.rs`: `Claude proposes a plan`, `Open plan`, `Accept`, `Accept and use Auto mode`, `Accept and allow edits`, `Revise…`, `What should change? (optional)`, `Back`, `Revise`, `Plan proposed`, `Plan` (panel title), and the panel hint.

## Testing

- Unit: `permission_view` for `ExitPlanMode` with, without and with an empty `plan`; `plan_key` for each key (Esc is `Revise`; plain digits are nothing); the mode mapping; the sidecar's `decisionToResult` with and without a reason.
- Sidecar test with the fake SDK: a `permission_response` with `message` reaches the SDK as the deny message; an accept with a mode produces the chosen mode.
- Real window (harness, real SDK, throwaway folder): start a session in `계획` mode, ask for a small plan; the panel opens with the plan; `수락` leaves the composer on `수동`; `수락 및 자동 모드` leaves it on `자동`; `수정…` with a text makes Claude propose a second plan that answers the text; `Esc` does not refuse. Dark mode check by the person (the harness cannot judge it).

## Left to the builder

The panel's width and header look (follow the Files panel), the exact CSS, the choice between the two ways of setting the mode, where the `Plan proposed` row sits in the turn view's data, and the wording of the default refusal reason when `수정` has no text (it must tell Claude to revise the plan, not to stop).

## Decisions I made (not the person's)

- Panel read-only with copy, expand and close only (the folder icon is left out).
- `Esc` opens `수정…` as in the desktop app, even though `Esc` refuses on the other permission cards.
- A malformed `ExitPlanMode` request (no plan text) falls back to the generic card.
- A later plan reopens the panel even if the person closed it.
- **After a restart or in a reopened session** the plan is not kept in a new store. The `Plan proposed ▸` row is built from the transcript's `ExitPlanMode` tool call, which holds the plan text; a click on the row opens the panel with that text. So the panel works for past sessions too, and nothing is added to the session record. The panel's own state (open or closed) is not restored.
- **A mode change that cannot be applied** does not end the session: the accept is sent, the mode stays as it was, and the composer shows the usual inline setting error. The builder first tries the SDK with a small spike (the sidecar test with the real SDK) to learn whether a plain allow of `ExitPlanMode` already leaves plan mode by itself and whether `setMode` in `updatedPermissions` is honoured; the choice between the two ways follows from that.
- **`거부` and `수정`** both answer with a refusal that does not interrupt the turn, so Claude stays in plan mode and replies. `거부` has no text: the default reason says the person declined the plan and Claude should ask what to change. `수정` sends the person's text, or the default reason when empty.
- **The `⌄` menu** repeats `수락 및 자동 모드` as the desktop app does; both entries are kept.
- **Several requests:** one open request per session, as the permission queue already works; if a second `ExitPlanMode` arrives while one is open it waits its turn. When a session is shown in several panes the Plan panel opens in the pane that shows the session that sent the plan.
