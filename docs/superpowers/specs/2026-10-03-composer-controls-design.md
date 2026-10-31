# Plan 16: composer controls, permission mode and model (CMP-03, CMP-04)

Plan 14's roadmap put the composer controls at Plan 16. The user chose on 2026-10-03: mode and model only. Effort (CMP-05) and the "더 많은 모델" submenu were never seen in the real app and stay out; they come after an observation.

Evidence: `docs/superpowers/roadmap/observations.md`, round 1 ("Composer menus").

## What the desktop app does (seen)

- Mode menu titled "모드", five rows: 자동 "Claude가 권한 결정을 처리합니다" (checked), 수동 "변경하기 전에 항상 확인", 편집 자동 수락 "모든 파일 편집을 자동으로 수락", 계획 "변경하기 전에 계획 세우기", and 권한 무시 with a separate "활성화" button. Keys 1 to 4 pick the first four.
- Model menu: Opus 5.5 (1), Sonnet 5.5 (2, badge 기본값), Fable 5.1 (3, badge 사용 크레딧 필요), Haiku 4.5 (4), "더 많은 모델".
- Not seen: whether a change applies to a running session, what happens on a failed change.

## What Forge has now

Four modes, picked on the new-session screen only; the model comes from Settings and applies to the next new session. Nothing changes either in a running session. The sidecar protocol has no command for it.

## SDK facts (read from `sdk.d.ts` of the pinned SDK)

- `PermissionMode` has `auto` ("use a model classifier to approve/deny permission prompts") and `dontAsk`. Forge adds `auto`; `dontAsk` is not in the desktop menu.
- `Query.setPermissionMode(mode)` and `Query.setModel(model?)` exist, streaming input only (Forge uses streaming input).
- `bypassPermissions` "requires `allowDangerouslySkipPermissions`". The sidecar does not pass it today.
- A model may not support `auto` (`ModelInfo`); the SDK may reject the change.

## Design

Both settings are changed in the session at once, and take effect on the next turn.

**Domain.** `PermissionMode::Auto` (wire and config value `auto`); the transcript loader reads `permission-mode: auto`. The view gets `model: Option<String>` (the model of this session; `None` is the SDK default).

**Protocol.** Commands `set_permission_mode { id, mode }` and `set_model { id, model | null }`. Events `permission_mode_set { mode }`, `model_set { model | null }` and `setting_failed { message }`. The sidecar answers each command with exactly one of them. `setting_failed` is not fatal: it shows the message as a banner and the session goes on (every `error` event ends the sidecar, so it cannot be reused for this).

**Sidecar.** `set_*` calls the active query. With no active query it answers `setting_failed`. The sidecar now passes `allowDangerouslySkipPermissions: true` so that bypass can be chosen later; it only ever enters bypass when told to, and Forge asks for a confirmation first (below).

**Application.** `LiveCommand::SetMode` and `SetModel`. With a sidecar running, the actor forwards the command and the view changes only when the answer comes (`permission_mode_set`, `model_set`): the control never shows what the SDK refused. With no sidecar yet (a new or resumed session before its first message), the actor changes the view and its start parameters directly, and the next start uses them. A failed write to the sidecar fails the session, like the other commands.

**UI.** Two pickers at the left of the composer's bottom row, before the usage ring: the mode and the model, each a button with the current name that opens a list, a check on the current row, a second line of description for modes, and the number keys 1 to 4 while the list is open. The keys use the physical key (`Digit1`, `Numpad1`), because of the Korean input method (Plan 15). Modes, in the desktop order: Auto, Ask before edits, Accept edits, Plan mode, and Bypass permissions below a divider. Choosing Bypass does not change the mode; it shows "Skips every permission check for this session." with Cancel and Enable, and only Enable sends the command. Models, in the desktop order: Opus 5.5, Sonnet 5.5, Fable 5.1, Haiku 4.5, a divider, then "Default model" (no key). The desktop badges are not copied: Forge does not know the account's plan or credits. The new-session screen keeps its select and gains "Auto".

**Left out.** Effort; "더 많은 모델"; `dontAsk`; remembering the choice for the next session (the model default stays in Settings); the model's `auto` support flag (a refusal comes back as `setting_failed`).

## Testing

Unit tests: wire round trip of the new modes, loader `auto`, `apply_sidecar_event` for the three events, actor with the fake sidecar (forwarded when running, direct when not, view unchanged on `setting_failed`), the menu rows and keys, the bypass confirmation state, vitest for the sidecar commands. Then the harness: open the pickers, change the mode with the number keys, and send a message to see that the sidecar accepted a mode and a model change.

## As built (2026-10-03)

Built as designed: `PermissionMode::Auto`, the two commands and three events, `allowDangerouslySkipPermissions`, `LiveSessionView.model`, the actor's two paths, the pickers with number keys and the bypass confirmation. `Workspace` carries `set_permission_mode` and `set_model` so the widgets do not depend on the domain crate.

Checked in the real window (harness, no message sent): both pickers open with the desktop order, descriptions and keys; key 4 (`key code`, Korean input method on) switched to Plan mode and the cursor went back to the message box; key 1 switched the model to Opus 5.5; choosing Bypass showed the confirmation and left the mode as it was; Cancel closed it.

**Checked with the real SDK** (harness app, one real message "hello", then changes in the idle session; the transcript the harness left in the real `~/.claude` was deleted afterwards). The view followed the SDK's answer every time, and a refusal showed the SDK's own text as the banner, left the picker on the old value and did not end the session.

| Change | Answer |
| --- | --- |
| Mode to Accept edits | accepted |
| Mode to Bypass permissions (after Enable) | accepted (so `allowDangerouslySkipPermissions` works) |
| Mode to Auto, on Haiku 4.5 | refused: "Cannot set permission mode to auto: auto mode unavailable for this model" |
| Model to Opus 5.5 | accepted |
| Model to Haiku 4.5 | accepted |
| Model to Sonnet 5.5 (`claude-sonnet-5-5`) | refused: "isn't described by this version's model catalog; update Claude Code, or map it with behavesAs on a modelPicker row" |
| Model to Fable 5.1 | refused: "API error: 429 Usage credits are required for this model · model not changed" |

Findings:

- **A refusal banner stayed after a later change was accepted.** Fixed: the actor remembers that `last_error` is a refusal (`refusal_shown`) and clears it when the SDK accepts a mode or model change; a failure message is never cleared this way.
- **`claude-sonnet-5-5` is not known to the bundled Claude Code (2.1.283).** The model ids in `KNOWN_MODELS` also feed Settings' default model and the model of a new session, so those may fail the same way. Not checked; see the backlog note.
- Not tried: Auto on a model that supports it (Opus 5.5 may), a change while a turn is running, a second turn to see the new model answer.
- The two paths are both seen now: no sidecar (view changes at once) and a running sidecar (view follows the answer).

- Left as designed: effort, "더 많은 모델", badges, `dontAsk`.
- The new-session select got "Auto" through the shared mode table.
- A session opened without a message has no sidecar until its first send (or warm-up), so the first change in it takes the direct path.
