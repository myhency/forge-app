# Plan 32A: the Code tab of Settings (SET-04, SET-05, SET-06, SET-09)

Date: 2026-10-09. Roadmap rows: SET-04, SET-05, SET-06, SET-09 (`docs/superpowers/roadmap/backlog.md`). Seen in observation round 15 (`observations.md`), the Claude Code page of the desktop app's Settings window.

## Goal

Four defaults that the desktop app keeps on its Claude Code page get a home in Forge's Settings overlay: the transcript view new sessions open in, the output style new sessions use, whether Claude predicts the next prompt, and whether the bypass-permissions mode may be chosen at all.

Success: each control changes what the next session does, the change survives a restart, and an old config file without the new fields still loads with today's behaviour (except bypass, which is now off by default).

## Decisions made with the user

- Output style is a default for new sessions only. A per-session override in the session menu (SES-04) is not part of this plan.
- The style names are a fixed list of five. Custom styles on disk are not read.
- Controls save when changed. The existing tabs keep their Save button.
- A session already in bypass mode keeps that mode when the switch is turned off.

## Stored settings

`AppConfig` (`forge-domain`) gets four fields, each `#[serde(default)]` so an older file loads:

| Field | Values | Default |
| --- | --- | --- |
| `default_transcript_view` | normal, thinking, detailed | normal |
| `output_style` | none, Concise, Explanatory, Learning, Proactive | none (the SDK's default style) |
| `prompt_suggestions` | bool | true (today's behaviour) |
| `allow_bypass_mode` | bool | false |

`prompt_suggestions` needs a serde default function that returns true, since `bool::default()` is false.

## Screen

A "Code" tab in the Settings overlay (`SettingsSection::Code`, `settings_overlay.rs`). Two groups, named as in the real app:

- `Display`: "Default transcript view", a three-way choice. Hint: "The view sessions open in. The transcript view menu of a session changes that session only."
- `Sessions`: "Output style" (menu), "Prompt suggestions" (switch), "Allow bypass permissions mode" (switch, off). Hints from the real app: the output style "applies to new sessions"; prompt suggestions "use a little more of your usage"; bypass carries a risk text.

Each control calls a `Settings` method that updates `AppConfig` and saves it through `SettingsUseCase` at once. A failed save shows an error in the tab, like the other tabs. The Code tab does not use `ConnectionDraft`.

## Behaviour

- **Default view (SET-04).** `TranscriptViews::get` returns a session's own choice, else the configured default. `set` stores a session's choice, including Normal when the default is not Normal (today Normal is "no entry"; with a default, Normal must be stored). Sessions open already have no entry, so they follow the default until the person picks a view for them.
- **Output style (SET-05).** The start command carries `output_style: Option<String>`. The sidecar passes `settings: { outputStyle }` in the query options when it is set (`Settings.outputStyle` in the SDK's `sdk.d.ts`). A session already running keeps its style.
- **Prompt suggestions (SET-06).** The start command carries `prompt_suggestions: bool`. The sidecar sets `promptSuggestions` from it instead of `true`. When off, the SDK sends no suggestion, and the composer shows its normal placeholder without any other change.
- **Bypass (SET-09).**
  - Off: `permission_mode_rows` leaves out "Bypass permissions". The start command carries `allow_bypass: false` and the sidecar sets `allowDangerouslySkipPermissions` from it, so the SDK refuses the mode too.
  - On: the row shows and the existing confirmation (CMP-03) stays.
  - A session that is already in bypass mode is not changed. Its picker still shows the current mode as selected, but no other session can reach bypass.
  - Resumed and forked sessions follow the same flag as new ones.

## Where it changes

- `forge-domain`: `AppConfig` fields, `OutputStyle` enum.
- `forge-ports` (`sidecar_port.rs`) and `forge-application` (`live_session*.rs`, registry host): three options on the start command; the sidecar protocol in `forge-adapter-sidecar`.
- `sidecar/src/session.ts`: the three options; `sidecar/test/session.test.ts`.
- `forge-features`: `Settings` methods and `SettingsSection::Code`; `TranscriptViews`; `permission_mode_rows`.
- `forge-widgets`: the Code tab; the mode picker takes the filtered rows.

## Tests

- Unit: an old config file loads with the defaults; the four fields round-trip; `TranscriptViews` follows the default and keeps an explicit Normal; `permission_mode_rows(false)` has no bypass row; the sidecar protocol carries the three options.
- Sidecar (`session.test.ts`): `outputStyle` reaches the query options only when set; `promptSuggestions` and `allowDangerouslySkipPermissions` follow the command.
- Real SDK: start a session with Concise and one with the default and see that the answer differs in style; with suggestions off no `PromptSuggestion` event arrives; with bypass off the SDK refuses a switch to bypass.
- Real window (Quartz harness, `FORGE_CLAUDE_HOME` fixture): open Settings, use the Code tab, start a session, check the transcript view, the mode picker and a restart.

## Not in this plan

Per-session output style (SES-04), custom output styles, the rest of the Claude Code page (SET-03, 07, 08, 10..14), and a search field in Settings (SET-01).
