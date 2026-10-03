# Plan 19: fork a session and effort (SES-01, CMP-05, OTH-07)

Plan 14's roadmap had a terminal panel at Plan 19. The user chose on 2026-10-03 to take the small backlog items first: fork (SES-01), effort (CMP-05) and the model id check (OTH-07). Archive (SES-02) was put off by the user. CMP-07 (the context circle) is left out: the desktop popover adds plan limits that come from the account, and Forge's `UsageRing` already shows context, tokens and cost.

Evidence: `docs/superpowers/roadmap/observations.md`, round 4.

## What the desktop app does (seen)

- **Fork** (title menu "포크", key F): at once, with no dialog, a new session "<title> (fork)" appears at the top of the same project group, opened and selected. The whole transcript is copied; folder, model, effort and mode are kept; the composer is empty with the cursor in it. The original stays.
- **Effort** (the label "중간" at the right of the model name): a popover titled "노력 <level>" with a `?` tooltip, end labels "더 빠르게" and "더 스마트하게", a slider of six stops and "추천" under the model's recommended stop.
- Not seen: a fork of a fork, of a running or an archived session, the names of the slider stops.

## What Forge has now

The session menu has rename and delete; the F key is tested to do nothing (`menu_key_action`). The composer has a mode and a model picker (Plan 16) that follow the SDK's answer. Nothing handles effort. Rename and delete are repository operations (`SessionRepositoryPort`, the `claude-fs` adapter edits the transcript files); no sidecar is needed for them.

## SDK facts (read from `sdk.d.ts` of the pinned SDK 0.3.283, and one run)

- `forkSession(sessionId, {dir, upToMessageId?, title?})` copies a transcript into a new session. Run on a probe transcript (user, assistant, `custom-title`): the new file has a new session id; every message gets a new `uuid`, its `parentUuid` points at the new uuid of its parent, `sessionId` is the new one, and each message carries `forkedFrom: {sessionId, messageUuid}`. The old `custom-title` line is not copied; a new one is written with the given title, a `uuid` and a `timestamp`. The original is not touched.
- `Query.applyFlagSettings({effortLevel})` changes effort in a running session (streaming input only, as `setModel`). Levels: `low`, `medium`, `high`, `xhigh`, `max`. `max` runs as `high` on a model without it. The query option `effort` sets it at the start.
- `Query.supportedModels()` returns `ModelInfo[]` with `value`, `displayName`, `supportsEffort`, `supportedEffortLevels`, `supportsAutoMode`. There is no "recommended level" field.
- The sidecar does not call `supportedModels()` today; it already emits `commands_available` once after the start.

## Design

### Fork

A repository operation, like rename and delete, because a past session has no sidecar and Forge starts one sidecar per open session.

**Port.** `SessionRepositoryPort::fork_session(project_path, session_id, title) -> Result<SessionId, SessionRepositoryError>` (new error `ForkFailed`). The `claude-fs` adapter reads the transcript line by line and writes the copy next to it:

- a new session id (UUID v4) names the file;
- every line that has a `uuid` gets a new one (a map old to new); `parentUuid` is mapped through it (an unknown or null parent stays as it is); `sessionId` becomes the new id; each such message gets `forkedFrom: {sessionId, messageUuid}`;
- `custom-title` lines are dropped and one new line is appended with the title;
- every other line is copied with only `sessionId` replaced; a line that is not valid JSON is copied as it is;
- the file is written to a temporary name and renamed, so a half-written copy is never listed. Data kept beside the transcript (file history) is not copied, as in the SDK.

The id check of rename and delete (nothing that could leave the project directory) applies to both ids.

**Title.** "<title> (fork)", where `<title>` is the title the sidebar shows for the source (custom title, else the first prompt). A fork of a fork becomes "<title> (fork) (fork)"; this is not seen in the real app and is left as it falls out.

**Application and UI.** A use case `fork_session` calls the port, then refreshes the project's list. The session menu gets "Fork" under "Rename" with the key F (physical key, like R and D; Plan 15), enabled for every row that has a Claude session id. After the copy the new session is opened as a past session (it starts a sidecar with `resume` on the first message, as any past session does), and is selected. The desktop app also puts the cursor in the composer; Forge does so only if the opened session's composer does it by itself (no extra code in this plan; checked in the window and noted in "As built"). An open session can be forked too; the copy holds what is on disk at that moment. A failure shows the error as the existing menu errors do, and nothing is opened. The fork starts with the mode and model the loader reads from the copied transcript, as any past session does. Effort is not recorded in a transcript, so the fork, like every resumed session, starts on the default effort; the desktop app keeps it, Forge does not (see Effort).

**Why not the SDK's `forkSession`.** It would need a one-shot node process for each fork (a sidecar is bound to one session) and a new entry point. The adapter code is about the size of rename and needs no node. The SDK stays the reference: a test runs the real `forkSession` on the same input once (as in the probe) and compares the structure.

### Effort

**Domain.** `EffortLevel` (`Low`, `Medium`, `High`, `XHigh`, `Max`; wire `low`, `medium`, `high`, `xhigh`, `max`). `LiveSessionView.effort: Option<EffortLevel>` (`None` is the SDK default). The transcript does not record effort, so a resumed session starts with `None` and the picker says "Default" until the user chooses.

**Protocol.** Command `set_effort { id, effort | null }`; event `effort_set { effort | null }` (a refusal is the existing `setting_failed`). `start_session` gains `effort: string | null`, passed as the query option `effort` when set. New event `models_available { models: [{value, display_name, effort_levels: []}] }`, sent once after the start, from `supportedModels()`; a failure of that call sends nothing.

**Sidecar.** `set_effort` calls `applyFlagSettings({effortLevel})` (`null` clears it); with no active query it answers `setting_failed`, as `set_model` does.

**Application.** `LiveCommand::SetEffort`, with the same two paths as mode and model: a running sidecar, so the view changes when `effort_set` arrives; none yet, so the view and the start parameters change at once. `models_available` is stored in the view (`available_models`). The refusal handling of Plan 16 (`refusal_shown`) applies unchanged.

**UI.** A third picker in the composer's bottom row, after the model picker: a button with the level name ("Default" when none). It opens a popover titled "Effort <level>" with a `?` tooltip ("Higher effort means a more thorough answer, but it takes longer and uses your limit faster."), the end labels "Faster" and "Smarter", and a stepped slider (`input type=range`, one stop per level the current model supports, arrow keys move it, a change sends `set_effort`). The levels come from the `available_models` entry whose `value` matches the session's model. When the session uses the SDK's default model (no id), the plan must first check which entry that is (the first row, or the `resolvedModel` match) against the real SDK; until then the picker is not shown for it. No matching entry, or `supportsEffort` false, means the picker is not shown. The desktop's "추천" mark is not copied: the SDK has no recommended level.

### Model ids (OTH-07)

A check, then a fix. With the real SDK: start a session, read `supportedModels()` and compare every `KNOWN_MODELS` id and the Plan 16 picker rows with the `value` and `resolvedModel` it returns. The desktop app's toast ("switched to Sonnet 5.5, the latest Sonnet 5") and Plan 16's refusal of `claude-sonnet-5-5` both say the id or alias may differ. Whatever the SDK accepts replaces the refused id in `KNOWN_MODELS` and in the picker; ids the SDK refuses are listed in the "As built" section. This part cannot be designed further before the check.

## Left out

Archive and restore (SES-02), CMP-07's plan limits, the effort stop names of the desktop (six stops), "추천", reading effort from a transcript, forking from a chosen message (`upToMessageId`), a fork button elsewhere than the session menu.

## Testing

Unit tests: the fork copy (uuid map, `parentUuid`, `sessionId`, `forkedFrom`, title line replaced, odd lines kept, ids that escape the directory refused, nothing left behind on failure, a missing source is `NotFound`), the title rule, `menu_key_action` for F, the use case with the mock port, wire round trips for `set_effort`, `effort_set`, `models_available`, the start parameter, `apply_sidecar_event`, the actor with the fake sidecar (both paths, refusal), the effort levels per model, vitest for the sidecar commands. A one-time check that compares the adapter's output with the SDK's `forkSession` on the same transcript.

Then the harness: fork from the menu and with F (Korean input method on), the new row, the opened copy, and the original unchanged; the effort popover and slider; and with the real SDK, one real message, then an effort change in the idle session and the `models_available` rows. OTH-07 is run against the real SDK too.

## As built (2026-10-03)

Built as designed, with the changes below. Fork is a `SessionRepositoryPort::fork_session` (the `claude-fs` adapter, `fork.rs`), the menu has "Fork" and the F key, effort follows the SDK's answer through the same two paths as mode and model, and the sidecar reports `models_available`.

**Changes from the design**

- **`resolved_model`.** The real SDK lists models by alias (`default`, `opus`, `haiku`, `sonnet`) and says the full id each stands for in `resolvedModel`. A session's model is a full id (`claude-opus-5-5`), so matching on the listed name alone never found it. `AvailableModel` and the wire event carry `resolved_model`, and `effort_levels` matches the session's model against the name or the full id.
- **Sonnet's id.** `claude-sonnet-5-5` is not in the bundled Claude Code's catalog (2.1.283); the SDK lists `sonnet` as `claude-sonnet-5` ("Sonnet 5"). `KNOWN_MODELS` and the composer picker now use `claude-sonnet-5` with the label "Sonnet 5", the SDK's own name. The desktop app calls it "Sonnet 5.5" because its Claude Code is newer; Forge shows the same label once the bundled SDK knows the model.
- A fork keeps the original message timestamps (the SDK probe rewrote one assistant timestamp to the fork time).

**What the real SDK listed** (`sidecar/scripts/list_models.mjs`, SDK 0.3.283, no message sent; this is the OTH-07 check):

| Listed as (`value`) | Resolves to | Name | Effort levels |
| --- | --- | --- | --- |
| `default` | `claude-opus-5-5` | Default (recommended) | low, medium, high, xhigh, max |
| `opus` | `claude-opus-5-5` | Opus 5.5 | low, medium, high, xhigh, max |
| `claude-fable-5-1` | `claude-fable-5-1` | Fable 5.1 | low, medium, high, xhigh, max |
| `haiku` | `claude-haiku-4-5-20251001` | Haiku 4.5 | none (`supportsEffort` absent) |
| `sonnet` | `claude-sonnet-5` | Sonnet 5 | low, medium, high, xhigh, max |
| `claude-opus-5`, `claude-fable-5`, `claude-opus-4-8`, `claude-opus-4-7` | the same | Opus 5, Fable 5, Opus 4.8, Opus 4.7 | low, medium, high, xhigh, max |
| `claude-opus-4-6`, `claude-sonnet-4-6` | the same | Opus 4.6, Sonnet 4.6 | low, medium, high, max |

The first row is `default`, so the rule "a session on the default model uses the first listed model" holds. Haiku, Opus 5.5 (`claude-opus-5-5`), Fable 5.1 ids in `KNOWN_MODELS` were already right (Haiku and Opus as `resolvedModel`).

**Checked in the real window** (harness, the fake Claude home; Forge's own fork code on the fixture sessions):

- Session menu: "Rename R", "Fork F", "Delete D". Clicking "Fork" made "Read the config (fork)" at the top of its project (just now), opened with the whole transcript; the original stayed. The F key (physical key, `key code 3`) did the same for "Fix the login bug" (the Korean input method was not switched on for this run; the key is matched by `event.code()` and unit-tested).
- The composer of an opened fork showed its placeholder; no cursor was seen after the fork, so the desktop app's focus on the composer is **not matched** (a gap, not planned code in this plan).
- Effort with the real SDK (one real message "Hello" in a new session in the harness folder; its transcript in the real `~/.claude/projects/-private-tmp-forge-harness-my-app/` was deleted afterwards):

| Step | Result |
| --- | --- |
| Before the first message | no effort control (no model list yet) |
| After the first reply | a button "Default"; the popover shows "Effort Default", a `?`, "Faster" / "Smarter", a five-stop slider with the thumb on High, and "Use the default" |
| Slider to the fourth stop | accepted: the button and the title read "Extra high" |
| "Use the default" | accepted: back to "Default" |
| Model to Haiku 4.5 (key 4) | accepted; the effort control is gone |
| Model back to Opus 5.5 (key 1) | accepted; the effort control is back (the full id matched through `resolved_model`) |

**Not tried:** an effort refusal from the real SDK (the unit tests cover `setting_failed`), `max` on a model without it, the names of the desktop's six stops, a fork of an open running session, a fork of a session whose project folder is gone, the Korean input method for the F key.

**After the whole-branch review** (a fresh reviewer read the full diff):

- Fixed: a fork of a session whose project is not in the sidebar's list (a folder added through the dialog, or a path spelled differently from the listed one) wrote the copy and then reported "the project is no longer listed" and opened nothing. `project_for_open` builds a summary from the path when none is listed, and the fork opens (view-only if the folder is gone). Test: `a_project_that_is_not_listed_still_gets_a_summary_so_the_fork_can_open`.
- Fixed: the effort slider sent one `set_effort` per step of a drag, so answers could arrive out of order and the controlled thumb could snap back. It now sends on `onchange` (when the thumb is released, and once per arrow key). No unit test (a DOM event binding); **not re-checked in the real window**: that needs one more real message to make the effort control appear.
- Not done (minor): the thumb falls back to the first stop when the model lacks the current level (label "Extra high", thumb on Low after switching to a model without it); ids in `leafUuid` and `logicalParentUuid` are not remapped in a fork; a fork of a running session can copy a half-written last line; invalid UTF-8 bytes in a copied line become U+FFFD; holding F can make several forks; the effort control shows only after the first message (the desktop shows it on a fresh composer); a model id with a context suffix (`claude-opus-5-5[1m]`) matches no listed model; a saved config model `claude-sonnet-5-5` shows as "Custom: …" and still fails on a new session (no migration to `claude-sonnet-5`).
