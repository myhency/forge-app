# Environment card, start line, and model/effort in an environment (Plan 53 stage B)

Date: 2026-10-22. Follows `2026-10-19-environments-design.md` (stage A, A2) and `2026-10-20-no-folder-session-design.md`. What the desktop app does is in `docs/superpowers/roadmap/observations.md`, Round 21 (parts 1 and 2) and Round 23 (parts 1 and 2). Local only: no cloud, SSH, Slack or Remote Control anything, not even disabled.

## Why

The user is copying the Claude desktop app's Environment feature. Stage A has the chip on the new-session screen and the `새 환경` / `환경 편집` dialog. Once a session runs, nothing shows which environment it is in, and there is no way to save the folder you chose as an environment. The desktop app answers both with a side panel card and a grey line above the first message. It also saves the session's model and effort into the environment.

## Goals

1. A new dock panel (the desktop app's first header toolbar icon) holding one card, `환경`.
2. A grey `시작 위치` line above the first message of a session.
3. Saving the model and effort in an environment, and a new session that follows them.
4. Open sessions keep their environment's variables and extra folders (see "Open sessions").

## Non-goals

- The panel's other cards: `컨텍스트`, `커넥터`, `변경 사항`. They overlap features Forge already has (the context circle, the Changes panel); the user decided on 2026-10-22 to leave them. Put a note in `backlog.md`.
- `Remote Control` row, switch, settings, any cloud or SSH tab.
- `폴더 추가…` and `폴더 변경…` in the card's folder menu (see "The panel").
- Seeding environments from past sessions, encrypted variable storage, cleanup of extra-folder worktrees on archive, Windows checking.
- A stored link from a session to its environment (the desktop app does not store one either; see below).

Anti-goal: a second place that edits environments. Every edit goes through the stage A dialog.

## What the desktop app does (observed)

- **The line:** `시작 위치: 이 컴퓨터 · <folder name> · 환경으로 저장` (the last part an underlined link) when the session's folder matches no saved environment. After saving: `시작 위치: 이 컴퓨터 · <folder name> · 저장된 환경: "<name>"` (plain text). It sits above the first message and scrolls with the chat.
- **The card** (`환경 ⌄`, the chevron folds it): a first row with a laptop icon and `이 컴퓨터` plus a `저장` link at the right (hover tooltip `로컬 세션`), or, for a saved environment, the environment's name plus a gear at the right (always visible). Then one row per folder (`<name> · <parent path>`), each with a menu: `Finder에서 보기`, line, `경로 복사`, `폴더 변경…` (saved-environment sessions only), `폴더 추가…`, line, `터미널에서 열기`. For a worktree session there is also a branch row with a `워크트리` tag (already seen; not new here).
- **Both `저장` entry points** open the stage A `새 환경` dialog: name = the folder's name, selected; the folder; `환경 변수`; `취소` / `환경 만들기`.
- **Saving from a session** stores the session's model and effort in the preset (`settings.modelId`, `settings.effort`), not the mode.
- **The link is worked out, not stored:** the session file keeps no environment id. Deleting the environment turns the card and the line back to `이 컴퓨터` / `환경으로 저장` at once.
- **The gear on the card** opens `환경 편집` without the `환경 삭제` button and with the folder label `폴더` (not `폴더 (선택 사항)`). The chip menu's gear opens it with `환경 삭제`.

## The design

### Matching a session to an environment

A session is "in" environment E when its starting folder equals `E.folders[0]`. The starting folder is the session's original folder. **call:** for a worktree session it is the repository folder, not the worktree; if the session record keeps only the worktree path, map it back through the worktree records Forge already keeps in the config (`worktrees`, Plan 47); a worktree the record does not know (one the desktop app made) stays unmatched. The line's `<folder name>` is the basename of that same folder. If several environments share a first folder, use the most recently used one (`last_used_at`). This is a computed lookup, nothing is added to the session record. (Why: it is what the desktop app does, and it makes deleting an environment self-cleaning.)

A session started in a no-folder scratch folder never matches (its folder is under the scratch root) and gets neither the card's `저장` link nor the start line's `환경으로 저장`: it shows `이 컴퓨터` and the existing `폴더 선택…` row only if that already exists; otherwise nothing is added for it (builder's call, keep it small).

### The panel

- `Panel::Details` (name is the builder's call) joins the dock, opened by a first icon in the chat header toolbar, before the terminal icon, as in the desktop app. It follows the same open / close / column / per-session state rules as the other panels (`crates/forge-features/src/dock.rs`, `crates/forge-widgets/src/dock.rs`, the header in `chat_window.rs`).
- The card's fold state is per session and kept like the Files panel keeps its state (builder's call how).
- Rows as in "What the desktop app does", with these calls (the builder-check questions, settled by the author, marked **call**):
  - **call: the folder rows come from the session**, not from the environment: the session's starting folder, then the added directories it was started with (stage A already passes an environment's extra folders as added directories; use what the session or its spawn request keeps; if nothing keeps them after a reopen, show only the starting folder). So editing an environment later does not change an old session's card.
  - **call: `폴더 추가…` and `폴더 변경…` are not built in this stage.** Whether the SDK can take a folder mid-session is unverified, and a folder added mid-session would need new per-session state (the session record stays as it is). Record both in `backlog.md` as left. The folder menu has `Finder에서 보기`, `경로 복사`, `터미널에서 열기`, reusing what the Files and terminal panels already offer.
  - The card's gear opens the stage A dialog in a variant: no `환경 삭제`, folder label `폴더` (a flag on the stage A dialog is fine). It keeps the stored model and effort when saved.
- A session whose folder is gone shows the folder row as usual (no extra error in this stage).

### The line

A small component above the first message of a session, same style as other grey notes in the chat. The link opens the stage A dialog with the name preset (see below); when the dialog saves, the line updates by itself because it is the same lookup.

### Saving from a session

Opening the dialog from the line or the card's `저장`: the draft is `EnvironmentDraft::blank(folder_name, Some(folder))`, plus this session's model and effort. On `환경 만들기` the new environment's `branch` and `worktree` follow stage A defaults (for a git folder: `worktree` on, as stage A's blank draft does; the desktop app showed the box checked). No trust question is asked here beyond what stage A's dialog already asks. Saving does not move the session, does not restart it, does not start a turn.

### Model and effort in an environment

- `Environment` gains `model: Option<String>` and `effort: Option<Effort>` (use the type the new-session draft already uses), both `#[serde(default)]`, read leniently like `extra_choices` (a bad value reads as `None`, never costs the environment).
- Saving from a session fills them from the model and effort the session is using now (what its composer pickers show, so a mid-chat change counts; the builder finds where that lives). The dialog opened from the chip (`새 환경`, `환경 편집`) does not show them (the desktop app's dialog has no field for them); editing keeps whatever is stored. A new environment made from the chip has `None` for both.
- Picking an environment on the new-session screen sets the composer's model and effort pickers to the environment's values when they are `Some` (the user can still change them before sending; picking `이 컴퓨터` again leaves the pickers as they are). If the stored model is no longer in the model list, ignore it and keep the current picker (the same fallback the pickers already have; reuse it).
- `touch_environment` and the existing start path are otherwise unchanged.

### Open sessions

Stage A noted: a session reopened from the sidebar or with ⇧⌘T runs without its environment's variables and extra folders, because nothing records the environment on the session. With the computed lookup the card can now find the environment, so the builder should use the same lookup when an open or reopened session's process is started, to pass the variables and extra folders. **Check first** how a reopened session is spawned in `crates/forge-app/src/runtime.rs` and `workspace.rs`; if the fix is more than a small, local change, do not do it: record it as left in the spec's "As built" and in the backlog. Do not change the session record format.

### Strings

Every user-visible string is a new `tr!` with a Korean entry in `crates/forge-shared/src/ko.rs` (the translation is otherwise complete; keep it that way). Use the desktop app's Korean text from the observations: `환경`, `저장`, `로컬 세션`, `시작 위치: {computer} · {folder} · `, `환경으로 저장`, `저장된 환경: "{name}"`, `Finder에서 보기`, `경로 복사`, `터미널에서 열기`, `폴더 추가…`.

## Checks

- Unit tests: the match lookup (no match, one match, two environments with one first folder, a worktree session, a scratch session); the draft from a session carries model and effort; lenient reading of `model` and `effort`; an old `config.json` without them loads.
- Real window, with the harness in `scripts/harness/` (fixture, `FORGE_CLAUDE_HOME`, `FORGE_CONFIG_PATH`): the panel opens from the first icon, the line and card before and after saving, the card after deleting the environment, picking an environment sets the pickers, the fold state, light and dark. A real message costs the user's account; the user allowed it once on 2026-10-22 for observation only, so ask again before the first real turn of this plan's checks.
- Keep the stage A spec's "As built" convention: add an "As built" section to this file at the end.

## What is left to the builder

Names, file split, the panel's icon, how fold state is kept, the exact style of the line, whether `폴더 추가…` is possible mid-session, and whether the open-session fix is small enough.

## As built

Plan 53 stage B, branch `plan-53b`, tasks 1 to 7 plus the real-window check.

**Built.** A fifth dock panel, `Panel::Details` (title `환경`, no partner), opened from a new header icon before the terminal icon. Its body is the Environment card: a fold header, a `이 컴퓨터` row with `저장` (session in a folder no environment starts with) or the environment's name with a gear (the environment is found by the computed lookup, not stored on the session), and one row per folder (name and parent path). A folder row opens a menu: `Finder에서 보기`, `경로 복사`, and `터미널에서 열기` on the first row only (no helper opens a terminal in an arbitrary folder; it opens the Terminal panel in the session's own folder). Scratch sessions show `이 컴퓨터` only and no folder rows. Above the first message a start line reads `시작 위치: {computer} · {folder} · 환경으로 저장` or `저장된 환경: "{name}"`; it is computed on render, so saving or deleting an environment changes the card and the line with no restart. `저장` and the line's link open `새 환경` with the folder name selected; the gear opens `환경 편집` without `환경 삭제` and with the label `폴더` (not `(선택 사항)`); delete stays in the new-session chip menu. An environment's `model` and `effort` (read leniently) set the two composer pickers when it is picked (`picker_choices`); a model that the known list does not hold leaves the model picker alone, `이 컴퓨터` touches nothing. A session reopened from the sidebar runs with its environment's variables and extra folders (`reopen_setup`, applied only before the session is attached). Card fold state is kept per session. Icons are inline SVG.

**Checked in the real window** (harness, fixture sessions only, Korean UI, nothing sent to Claude): 1 the header icon, the card and the line for a session in an unsaved folder; 2 `저장` opened `새 환경` with `my-app` selected, saving turned the card to name + gear and the line to `저장된 환경: "my-app"` with no restart; 3 the gear's `환경 편집` had no `환경 삭제` and the label `폴더`; 4 deleting the environment from the chip menu (hover, gear, `환경 삭제`, confirm) turned the card and the line back after returning to the session; 5 the folder row menu with all three items; 6 an environment seeded in `config.json` with `claude-sonnet-5-5` and `high` set the pickers to that model and `높음`; one with `no-such-model` and `low` set `낮음` and left the model picker on `claude-sonnet-5-5`; 7 light and dark (dark by setting `appearance.theme` to `dark` in the harness `config.json`, which works), and the fold state survived closing and reopening the panel. Screenshots were taken to `/private/tmp/shot-check*.png` and are not kept in the repository. One defect found and fixed: the folder menu reused `.picker-menu`, whose `bottom` offset collapsed the box to a bar below the card (`.env-card-menu` now sets `bottom: auto`).

**Left.** `폴더 추가…` and `폴더 변경…` in the folder menu; the panel's other cards (`컨텍스트`, `커넥터`, `변경 사항`); Windows (never run); a real turn: no message was sent, so that a reopened session's process really receives the variables and extra folders (Task 7) is covered by unit tests only; the unlisted-model case was checked with the picker's known list holding only the model chosen before (the list is empty until a real session probes it, and with an empty list an environment's model is applied unvalidated); the folder menu item `터미널에서 열기` was seen but not clicked; `Finder에서 보기` and `경로 복사` were seen but not clicked; the card's sizing in a narrow window and the floating dock; `scripts/i18n_add.py` does not escape quotes (the `저장된 환경: "{name}"` entry was fixed by hand).
