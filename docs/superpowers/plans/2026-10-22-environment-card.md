# Environment card, start line, model and effort in an environment (Plan 53 stage B) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A session shows which saved environment it is in (a new dock panel with one `환경` card, and a grey `시작 위치` line above its first message), can be saved as an environment from there, and an environment keeps the model and effort it was saved with.

**Architecture:** A pure lookup in `forge-domain` matches a session's origin folder to an environment (nothing stored on the session). `Environment` gains `model` and `effort`. The existing stage A dialog is reused with new request variants. A new `Panel::Details` joins the dock with a header icon and one card. The new-session environment pick copies the environment's model and effort into the composer pickers.

**Tech Stack:** Rust, Dioxus 0.7.10, crates `forge-*`.

**Spec:** `docs/superpowers/specs/2026-10-22-environment-card-design.md` (binding). Evidence: `docs/superpowers/roadmap/observations.md` Round 21 and Round 23.

## Global Constraints

- Local only: no cloud, SSH, Slack or Remote Control row, switch, tab or setting.
- No stored link from a session to its environment; the session record format does not change.
- Every user-visible string goes through `tr!` with a Korean entry in `crates/forge-shared/src/ko.rs` (use `python3 scripts/i18n_add.py`; run `cargo test -p forge-shared`). Korean texts: `환경`, `저장`, `로컬 세션`, `시작 위치: {computer} · {folder} · `, `환경으로 저장`, `저장된 환경: "{name}"`, `Finder에서 보기`, `경로 복사`, `터미널에서 열기`.
- Not built: `폴더 추가…`, `폴더 변경…`, the panel's `컨텍스트`, `커넥터`, `변경 사항` cards, `Remote Control` row.
- A session in a no-folder scratch folder never matches an environment.
- Rust style of the repo: comments only where the reason is not obvious; tests next to the code with sentence names; commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Existing `config.json` files without the new fields must load unchanged.

## Review Focus

- A stored `model` that is not a string, or an `effort` that is not a known level, reads as `None` and keeps the rest of the environment (folders, variables). (Task 1.)
- Two environments with the same first folder: the most recently used one is the match; an environment with no folders never matches. (Task 2.)
- A worktree session (recorded or at `<repo>/.claude/worktrees/<name>`) matches the environment of its repository; a worktree the records do not know and the path rule does not name stays unmatched. (Task 2.)
- Picking an environment whose stored model is not in the model list keeps the composer's current model instead of showing a blank or wrong picker. (Task 6.)
- Deleting the environment a session matched turns the card and the line back to `저장` / `환경으로 저장` without a restart. (Task 5, 6.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/environment.rs` | `Environment.model`, `.effort`; `environment_of` lookup |
| `crates/forge-features/src/environment.rs` | `EnvironmentDraft` model/effort; `EnvironmentRequest::{NewFromSession, EditInSession}`; `card_folders`; `picker_choices` |
| `crates/forge-features/src/dock.rs` | `Panel::Details` |
| `crates/forge-features/src/workspace.rs` | `session_environment`, `session_added_folders`, request open helpers; reopened-session environment (Task 7) |
| `crates/forge-widgets/src/environment_dialog.rs` | the two new request variants (title, delete button, folder label) |
| `crates/forge-widgets/src/details_panel.rs` (new) | `DetailsPanel`, `EnvironmentCard` |
| `crates/forge-widgets/src/start_line.rs` (new) | `StartLine` |
| `crates/forge-widgets/src/session_tools.rs`, `dock.rs`, `chat_window.rs`, `environment_chip.rs`, `lib.rs` | icon, panel arm, line placement, picker copy, exports |
| `crates/forge-shared/src/ko.rs` | Korean entries |
| `docs/superpowers/specs/2026-10-22-environment-card-design.md`, `roadmap/backlog.md` | "As built", backlog |

---

### Task 1: `model` and `effort` in `Environment`

**Files:**
- Modify: `crates/forge-domain/src/environment.rs` (struct, `Debug`, the three test literals at ~245, 268, 286), `crates/forge-domain/src/config.rs` (test literals at ~889, 924), `crates/forge-features/src/environment.rs:162` and `:334`.

**Interfaces:**
- Produces: `Environment { …, pub model: Option<String>, pub effort: Option<EffortLevel> }`, both `#[serde(default)]` and lenient. `EffortLevel` is `forge_domain::EffortLevel` (`crates/forge-domain/src/effort.rs`).

- [ ] **Step 1: Write the failing tests** in `crates/forge-domain/src/environment.rs`:
  - `an_environment_without_model_and_effort_loads_with_none`: JSON `{"id":"e","name":"n"}` → `model == None && effort == None`.
  - `model_and_effort_round_trip`: `model: Some("claude-sonnet-5-5")`, `effort: Some(EffortLevel::Medium)` serialize and read back equal (the JSON has `"effort":"medium"`).
  - `a_bad_model_or_effort_reads_as_none_and_keeps_the_folders`: JSON `{"id":"e","name":"n","folders":["/a"],"model":5,"effort":"huge"}` → `model == None`, `effort == None`, `folders == ["/a"]`.
- [ ] **Step 2: Run** `cargo test -p forge-domain environment` — Expected: FAIL (fields missing).
- [ ] **Step 3: Implement** the two fields with `deserialize_with = "lenient_model"` / `"lenient_effort"` (private fns next to `lenient_choices`, same Value-based style); add them to the manual `Debug`; `normalized()` leaves them alone; add `model: None, effort: None` to every `Environment { … }` literal the compiler flags (including `EnvironmentDraft::build`, which sets them in Task 3).
- [ ] **Step 4: Run** `cargo test -p forge-domain && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: environments keep a model and an effort`.

### Task 2: Match a session to an environment

**Files:**
- Modify: `crates/forge-domain/src/environment.rs`
- Modify: `crates/forge-features/src/workspace.rs` (next to `worktree_tag`, ~line 681)

**Interfaces:**
- Consumes: `origin_of(records: &[WorktreeRecord], path: &Path) -> PathBuf` and `scratch::is_scratch(root: &Path, path: &Path) -> bool` (both in `forge-domain`), `Environment` from Task 1.
- Produces:
  - `pub fn environment_of<'a>(environments: &'a [Environment], records: &[WorktreeRecord], scratch_root: Option<&Path>, project: &Path) -> Option<&'a Environment>` in `forge-domain` (re-exported like its neighbours).
  - `Workspace::session_environment(&self, project: &Path) -> Option<forge_domain::Environment>` (reads `self.environments()`, `self.worktrees`, `self.scratch_root()`).

- [ ] **Step 1: Write the failing tests** (in `forge-domain`, sentence names, helper `env(id, folders, last_used_at)`):
  - `no_environment_matches_a_folder_that_is_nobody_s_first_folder`
  - `the_first_folder_matches_and_a_later_folder_does_not`
  - `two_environments_with_one_first_folder_pick_the_most_recently_used`
  - `an_environment_without_folders_never_matches`
  - `a_recorded_worktree_matches_the_environment_of_its_repository` (record `path: /r/.claude/worktrees/x`, `repo: /r`; environment first folder `/r`)
  - `a_path_named_worktree_matches_without_a_record`
  - `a_scratch_folder_never_matches` (root `/s`, project `/s/scratch-2026-10-22-abc123`, an environment whose first folder is that same path still returns `None`)
- [ ] **Step 2: Run** `cargo test -p forge-domain environment_of` — Expected: FAIL.
- [ ] **Step 3: Implement** `environment_of`: return `None` when `scratch_root` is `Some` and `is_scratch` says yes; origin = `origin_of(records, project)`; keep environments whose `folders.first() == Some(&origin)`; pick max `last_used_at` (first one wins a tie). Then `Workspace::session_environment` as a thin wrapper.
- [ ] **Step 4: Run** `cargo test -p forge-domain && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: find the environment a session is in`.

### Task 3: Draft and dialog variants

**Files:**
- Modify: `crates/forge-features/src/environment.rs` (`EnvironmentDraft`, `EnvironmentRequest`, tests), `crates/forge-features/src/workspace.rs` (`open_environment_dialog` callers keep working), `crates/forge-widgets/src/environment_dialog.rs`, `crates/forge-widgets/src/environment_chip.rs` (the two `EnvironmentRequest::New { folder }` sites).

**Interfaces:**
- Produces:
  - `EnvironmentDraft { …, pub model: Option<String>, pub effort: Option<EffortLevel> }`. `blank(..)` gives `None`/`None`; `of(&Environment)` copies them; `build(..)` writes them into the `Environment`; `changed_from` compares them too.
  - `EnvironmentDraft::from_session(folder: PathBuf, model: Option<String>, effort: Option<EffortLevel>) -> EnvironmentDraft`: `blank(<folder's file name or its display string>, Some(folder))` with the two fields set.
  - `EnvironmentRequest::NewFromSession { folder: PathBuf, model: Option<String>, effort: Option<EffortLevel> }` and `EnvironmentRequest::EditInSession(String)` (an environment id). `New` and `Edit` are unchanged.

- [ ] **Step 1: Write the failing tests** in `crates/forge-features/src/environment.rs`:
  - `a_draft_from_a_session_carries_the_folder_name_model_and_effort` (`from_session("/p/forge-observe-git".into(), Some("m".into()), Some(EffortLevel::High))` → `name == "forge-observe-git"`, `folders == ["/p/forge-observe-git"]`, `model == Some("m")`, `effort == Some(High)`, first choice `worktree == true`).
  - `building_a_draft_keeps_model_and_effort`
  - `editing_keeps_the_stored_model_and_effort` (`of(&env)` then `build(Some(&env), …)` equals the model/effort of `env`).
  - `a_changed_model_counts_as_a_change`
- [ ] **Step 2: Run** `cargo test -p forge-features environment` — Expected: FAIL.
- [ ] **Step 3: Implement** the draft changes and the two request variants. In `EnvironmentDialogBody`: `NewFromSession` builds `initial` with `from_session`; `EditInSession(id)` behaves as `Edit(id)` except (a) the `환경 삭제` button and the `Ask::Delete` path are not rendered, (b) the folder label is `tr!("Folders")` with no `(optional)` hint. `request` is already the `key`, so each variant rebuilds the body. Nothing else in the dialog changes.
- [ ] **Step 4: Run** `cargo test -p forge-features && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: the environment dialog can start from a session`.

### Task 4: `Panel::Details`, header icon, empty panel

**Files:**
- Modify: `crates/forge-features/src/dock.rs`, `crates/forge-widgets/src/dock.rs:~72`, `crates/forge-widgets/src/session_tools.rs`, `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/ko.rs`
- Create: `crates/forge-widgets/src/details_panel.rs`

**Interfaces:**
- Produces: `Panel::Details` (title `tr!("Environment")`; no partner, like `Files`); `DetailsPanel { id: LiveSessionId }` component inside `PanelFrame`; a header icon button before the terminal button in `SessionTools`, shown under the same rule as the terminal button (`terminal` is true, so not for an archived session).

- [ ] **Step 1: Write the failing tests** in `dock.rs`: `details_opens_in_its_own_column_and_closes` (toggle opens one column holding only `Details`, a second toggle closes it) and `details_has_the_environment_title`.
- [ ] **Step 2: Run** `cargo test -p forge-features dock` — Expected: FAIL.
- [ ] **Step 3: Implement** the variant (add it to `title`, and to the `partner` arm that returns `None`), the match arm in the widget dock, the icon button (`aria-pressed`, `title: tr!("Environment")`, glyph `☰`), and `DetailsPanel` rendering `PanelFrame { id, panel: Panel::Details, tools: rsx!{}, … }` with an empty body for now. Add `("Environment", "환경")` only if not already present (it is: do not duplicate).
- [ ] **Step 4: Run** `cargo test -p forge-features -p forge-shared && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: a Details panel in the dock`.

### Task 5: The `환경` card

**Files:**
- Modify: `crates/forge-widgets/src/details_panel.rs`, `crates/forge-features/src/environment.rs` (`card_folders`), `crates/forge-features/src/workspace.rs` (`session_added_folders`, filled in `start_new`, forgotten in `close`), `crates/forge-shared/src/ko.rs`, `crates/forge-widgets/src/lib.rs`

**Interfaces:**
- Consumes: `Workspace::session_environment` (Task 2), `EnvironmentRequest::{NewFromSession, EditInSession}` (Task 3), `workspace.reveal(&Path)`, `LiveSessionView { project_path, model, effort }`.
- Produces:
  - `pub fn card_folders(origin: &Path, added: &[PathBuf]) -> Vec<PathBuf>`: `origin` first, then each added folder once, skipping `origin` and repeats.
  - `Workspace::session_added_folders(&self, id: LiveSessionId) -> Vec<PathBuf>`: the folders handed to `handle.add_directories` in `start_new` (the merged `folders` list), kept in memory per live session and dropped in `close`. A session opened from the sidebar has none (the card then shows only its origin folder).

- [ ] **Step 1: Write the failing tests** (`crates/forge-features/src/environment.rs`): `the_card_lists_the_origin_first_then_the_added_folders`, `the_card_skips_the_origin_and_repeats`, `the_card_with_nothing_added_lists_only_the_origin`.
- [ ] **Step 2: Run** `cargo test -p forge-features card_folders` — Expected: FAIL.
- [ ] **Step 3: Implement** `card_folders` and the workspace map (same signal pattern as `docks`/`files_stores`). In `EnvironmentCard { id }`: origin = `forge_domain::origin_of(&worktrees, &view.project_path)`; header row `환경 ⌄` (the chevron folds the card; fold state is a `Signal<bool>` kept per session in the workspace map you added, so it survives closing and reopening the panel); row 1 is a laptop icon, then either `이 컴퓨터` plus a `저장` link (`tr!("Save")`, tooltip `tr!("Local session")`, opens `NewFromSession { folder: origin, model: view.model, effort: view.effort }`) or, when `session_environment(&view.project_path)` is `Some`, the environment's name plus an always-visible gear that opens `EditInSession(id)`. A scratch session has no `저장` link and no matching row: show `이 컴퓨터` only. Then one row per `card_folders(..)` entry: folder icon, name, ` · ` and the parent path (muted); a click opens a small menu: `Finder에서 보기` (`workspace.reveal`), line, `경로 복사` (reuse the clipboard helper the Files panel's `Copy path` uses; find it with `grep -rn "clipboard" crates/forge-widgets`), `터미널에서 열기` (opens the Terminal panel for the origin row only; if no helper opens a terminal in a given folder, show the item on the origin row only and say so in "As built"). Style classes follow `panel_frame`/`files_panel`; keep CSS in the same stylesheet those use.
- [ ] **Step 4: Run** `cargo test -p forge-features -p forge-shared && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: the Environment card`.

### Task 6: `시작 위치` line, and pickers follow the picked environment

**Files:**
- Create: `crates/forge-widgets/src/start_line.rs`
- Modify: `crates/forge-widgets/src/chat_window.rs` (above `Turns`, inside `div.messages`, ~line 78), `crates/forge-widgets/src/lib.rs`, `crates/forge-features/src/environment.rs` (`picker_choices`), `crates/forge-widgets/src/environment_chip.rs` (the pick handler near line 42), `crates/forge-shared/src/ko.rs`

**Interfaces:**
- Consumes: Task 2 and 3 items; `NewSessionDraft { model: Signal<ModelChoice>, effort: Signal<Option<EffortLevel>> }` (`ModelChoice = Option<Option<String>>`).
- Produces:
  - `StartLine { id: LiveSessionId }`: `시작 위치: 이 컴퓨터 · {origin folder name} · ` then a link `환경으로 저장` (opens `NewFromSession`) or plain text `저장된 환경: "{name}"` when matched. Renders nothing for a scratch session.
  - `pub fn picker_choices(environment: &Environment, current_model: &ModelChoice, current_effort: Option<EffortLevel>, known_models: &[String]) -> (ModelChoice, Option<EffortLevel>)`: the environment's model when it is `Some` and listed in `known_models`, else `current_model`; the environment's effort when `Some`, else `current_effort`.

- [ ] **Step 1: Write the failing tests** (`environment.rs`): `an_environment_model_replaces_the_picker_when_it_is_listed`, `an_unlisted_environment_model_keeps_the_current_picker`, `an_environment_without_model_or_effort_changes_nothing`, `an_environment_effort_replaces_the_current_effort`.
- [ ] **Step 2: Run** `cargo test -p forge-features picker_choices` — Expected: FAIL.
- [ ] **Step 3: Implement** `picker_choices`. In the chip's pick handler, after setting `draft.environment`, write the returned pair into `draft.model` and `draft.effort` (find the list of known models where the model picker already reads it, `composer_controls.rs`/`new_session_controls.rs`; picking `이 컴퓨터` does not touch the pickers). Build `StartLine` and place it above `Turns`.
- [ ] **Step 4: Run** `cargo test -p forge-features -p forge-shared && cargo build --workspace` — Expected: PASS.
- [ ] **Step 5: Commit** `feat: the start line, and environments set the model and effort pickers`.

### Task 7: Reopened sessions keep the environment (conditional)

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (`open_past_in`, ~line 1902)

**Interfaces:**
- Consumes: `Workspace::session_environment`, `crates/forge-features/src/environment.rs::start_plan`, `handle.set_env(EnvVars)`, `handle.add_directories(Vec<PathBuf>)`.

- [ ] **Step 1: Check first.** Read `open_past_in` and `LiveSessionHandle::set_env` / `add_directories` (`crates/forge-application/src/live_session.rs`): both only take effect before the sidecar starts (the doc comments say "the next start"), and `open_existing` does not start it. If so, continue; if `set_env` after `open_existing` does not reach the first start, **stop this task**, write the finding under "Left" in the spec's "As built" and in `backlog.md`, and go to Task 8.
- [ ] **Step 2: Write the failing test** for a pure helper `reopen_setup(environment: &Environment, made: &[Option<PathBuf>]) -> (EnvVars, Vec<PathBuf>)` in `environment.rs`: returns the parsed variables (an unparsable `env_vars` gives empty variables, never an error, so a reopen is not blocked) and the extra folders. Names: `a_reopened_session_gets_the_environment_variables_and_extra_folders`, `a_reopened_session_with_bad_variables_still_opens`.
- [ ] **Step 3: Run** `cargo test -p forge-features reopen_setup` — Expected: FAIL.
- [ ] **Step 4: Implement** the helper and call it in `open_past_in` after `ensure_attached`, only when the session was not already attached and `session_environment(&project.path)` is `Some`. Extra worktrees are not remade on a reopen (pass `&[]` for `made`).
- [ ] **Step 5: Run** `cargo test --workspace` — Expected: PASS except the known `forge-adapter-claude-plugins round_trip` failure (it reads the real plugin folder).
- [ ] **Step 6: Commit** `fix: a reopened session runs with its environment's variables and folders`.

### Task 8: Real-window check, docs, memory

**Files:**
- Modify: `docs/superpowers/specs/2026-10-22-environment-card-design.md` (add "As built"), `docs/superpowers/roadmap/backlog.md` (OTH-11 row, PNL-08 note), memory `plan-53-environments.md` (update, no new file).

- [ ] **Step 1: Run the harness** (`scripts/harness/launch.sh`, then `capture.py`, `click.py`; seed environments by editing `/private/tmp/forge-harness-home/forge-config/config.json`; paste text with pbcopy then key code 9 with command, because typing drops letters with the Korean IME). Check, with a screenshot each:
  1. The header icon opens the panel; the card shows `이 컴퓨터` + `저장` for a session in an unsaved folder, and the line `시작 위치: … · 환경으로 저장`.
  2. `저장` opens `새 환경` with the folder name selected; saving turns the card to name + gear and the line to `저장된 환경: "…"` with no restart.
  3. The card's gear opens `환경 편집` with no `환경 삭제` and the label `폴더`.
  4. Deleting that environment from the chip menu turns the card and line back.
  5. The folder row menu: `Finder에서 보기`, `경로 복사`.
  6. Picking an environment saved with a model and effort sets the two pickers; one with an unlisted model leaves the model picker alone.
  7. Light and dark mode; the fold state survives closing and reopening the panel.
  Ask the user before sending any real message.
- [ ] **Step 2: Fix what the check shows**, one commit per fix.
- [ ] **Step 3: Write "As built"** in the spec (what was built, what was checked, what is left: `폴더 추가…`, `폴더 변경…`, the other cards, Windows, Task 7 outcome), update the backlog rows, update the memory file.
- [ ] **Step 4: Run** `cargo test --workspace` and `cargo build --workspace`; commit `docs: Plan 53 stage B as built`.
