# Card folder items: `폴더 추가…` and `폴더 변경…` (Plan 53 stage C) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The `환경` card's first folder row gets `폴더 변경…` and `폴더 추가…` like the desktop app: add a folder to a running session, and move a session to another folder while the card keeps the environment's name.

**Architecture:** A session's environment stays computed from its folder (stage B) except for sessions moved by `폴더 변경…`: that action pins the environment it showed in a new config list `session_environments`, and the lookup prefers the pin. The live session actor learns two things: a folder added after the first send restarts the sidecar on the next send (resume with the longer list), and a `MoveTo` command that changes the working folder and drops the sidecar. A new `SessionRepositoryPort::move_session` moves the transcript file to the new folder's project directory.

**Tech Stack:** Rust, Dioxus 0.7.10, crates `forge-*`.

**Spec:** `docs/superpowers/specs/2026-10-22-card-folder-items-design.md` (binding). Evidence: `docs/superpowers/roadmap/observations.md` Rounds 23, 24, 25.

## Global Constraints

- Local only: no cloud, SSH, Slack or Remote Control anywhere.
- Every user-visible string goes through `tr!` with a Korean entry in `crates/forge-shared/src/ko.rs` (use `python3 scripts/i18n_add.py`, which does NOT escape double quotes: check the entry by hand; run `cargo test -p forge-shared`). Korean: `폴더 변경…`, `폴더 추가…`, `이 워크스페이스를 신뢰하시겠습니까?`, `작업 공간 신뢰`, `세션이 {name}(으)로 이동되었습니다`. English keys: `Change folder…`, `Add folder…`, `Do you trust this workspace?`, `Trust workspace`, `Session moved to {name}`. Reuse the existing `Cancel`.
- The session record format (transcript) does not change; the only new stored data is `AppConfig.session_environments`. Configs without it must load unchanged; a bad entry is dropped (`lenient_entries`).
- No trust question for `폴더 추가…`; a trust question for `폴더 변경…` when `Workspace::needs_trust` says so.
- `폴더 变更…`/`폴더 추가…` are disabled while a turn runs (`Phase::Running`) and `폴더 변경…` is hidden for a worktree session and absent on added rows and scratch sessions.
- A failed move leaves the session exactly where it was (folder, transcript, sidecar, pin) and shows a notice.
- Not built: the composer `폴더` chip, persisting added folders across a restart, a hidden note about the new directory in the transcript, moving a worktree session, Windows (never run).
- Per-session state keyed by project path that does not move with the session (Changes mode `changes_modes`) is left behind on a move; accepted.
- Rust style of the repo: comments only where the reason is not obvious; tests next to the code with sentence names; commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`. Test binaries hang without `</dev/null`; the known unrelated failure is forge-adapter-claude-plugins `round_trip`.

## Review Focus

- Moving onto a target that already holds `<id>.jsonl` refuses and changes nothing; a source that does not exist (a session with no transcript yet) refuses with a notice. (Task 2, 5.)
- Choosing the folder the session already has is a no-op (no move, no toast, no pin). (Task 5.)
- A pin whose environment was deleted falls back to the folder match; a pin for another session id does not apply. (Task 1.)
- A restart of the sidecar after `add_directories` must keep the already added folders, `allowed_tools` and `env`, and must not happen while a turn runs. (Task 3.)
- After a move the project-scoped extensions are resolved again for the new folder (the actor caches `active` and `defaults`). (Task 3.)
- The first folder moved onto a folder that is already an added row: the card shows it once. (Task 4/5, `card_folders`.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/environment.rs`, `config.rs` | `SessionEnvironmentRef`, `AppConfig.session_environments`, `environment_of_session` |
| `crates/forge-ports/src/session_port.rs` | `SessionRepositoryPort::move_session`, `MoveFailed` |
| `crates/forge-adapter-claude-fs/src/sessions.rs` | the file move |
| `crates/forge-application/src/registry.rs` | `move_session` use (also the hand-written test fake) |
| `crates/forge-application/src/live_session.rs`, `live_session_actor.rs` | `LiveCommand::MoveTo`, handle methods, restart rule for `add_directories` |
| `crates/forge-features/src/environment.rs` | `folder_menu` pure helper |
| `crates/forge-features/src/workspace.rs` | `session_environment(project, session)`, `add_session_folder`, `change_session_folder`, pin store, trust request state |
| `crates/forge-widgets/src/details_panel.rs`, `lib.rs`, a new `workspace_trust_dialog.rs` | menu items, trust dialog |
| `crates/forge-shared/src/ko.rs` | Korean entries |
| spec, `roadmap/backlog.md` | "As built", backlog |

---

### Task 1: Pinned environment lookup

**Files:**
- Modify: `crates/forge-domain/src/environment.rs`, `crates/forge-domain/src/config.rs` (field, `Default`, a test beside the `environments` ones), `crates/forge-domain/src/lib.rs` (exports)
- Test: same files

**Interfaces:**
- Produces: `pub struct SessionEnvironmentRef { pub project: PathBuf, pub session_id: String, pub environment_id: String }` (`Serialize, Deserialize, Clone, Debug, PartialEq, Eq`); `AppConfig.session_environments: Vec<SessionEnvironmentRef>` with `#[serde(default, deserialize_with = "lenient_entries")]`; `pub fn environment_of_session<'a>(environments: &'a [Environment], pins: &[SessionEnvironmentRef], records: &[WorktreeRecord], scratch_root: Option<&Path>, project: &Path, session_id: Option<&str>) -> Option<&'a Environment>`.

- [ ] **Step 1: Write the failing tests** (`environment_of_session_tests` module):
  - `a_pin_for_this_session_wins_over_the_folder_match`: environments `A` (first folder `/a`) and `B`; project `/a`; pin `{project:/a, session_id:"s", environment_id:"B"}` → `B`.
  - `a_pin_for_another_session_or_project_is_ignored`: same pin, `session_id: Some("t")` → `A`; project `/c` with pin project `/a` → `None`.
  - `a_pin_whose_environment_is_gone_falls_back_to_the_folder_match`: pin names `"zzz"` → `A`.
  - `without_a_session_id_only_the_folder_match_applies`.
  - `a_scratch_session_ignores_pins`.
  - In `config.rs`: `old_configs_without_session_environments_load_with_none` and `a_bad_session_environments_entry_is_dropped_and_the_rest_kept`.
- [ ] **Step 2:** `cargo test -p forge-domain </dev/null` → FAIL (not defined).
- [ ] **Step 3: Implement** `environment_of_session`: scratch → `None`; a pin with equal `project` and `session_id` whose environment id exists wins; else `environment_of(...)`. Add the struct, the config field with its `Default` entry, and the `lib.rs` exports.
- [ ] **Step 4:** run the domain tests → PASS.
- [ ] **Step 5: Commit** `feat: environment lookup that honours a pin set by moving a session`.

---

### Task 2: Move a transcript to another project directory

**Files:**
- Modify: `crates/forge-ports/src/session_port.rs`, `crates/forge-adapter-claude-fs/src/sessions.rs`, `crates/forge-application/src/registry.rs` (use case plus the hand-written fake at ~560)
- Test: `crates/forge-adapter-claude-fs/src/sessions.rs` (tests module, use its `write_session` helper), `registry.rs`

**Interfaces:**
- Produces: `SessionRepositoryError::MoveFailed(String)` (`"failed to move session: {0}"`); `async fn move_session(&self, from_project: &Path, to_project: &Path, session_id: &SessionId) -> Result<(), SessionRepositoryError>` on `SessionRepositoryPort`; on the registry/manage-sessions type next to `fork` (line ~216): `pub async fn move_to(&self, from_project: &Path, to_project: &Path, session_id: &SessionId) -> Result<(), ManageSessionError>`.

- [ ] **Step 1: Write the failing adapter tests** (temp claude home):
  - `move_puts_the_transcript_in_the_new_project_dir_and_removes_the_old_one`: after the move `load_session(to, id)` works and `load_session(from, id)` is `NotFound`.
  - `move_takes_the_same_named_folder_along`: a `<id>/` directory with a file beside the transcript ends up under the new project dir.
  - `move_refuses_when_the_target_already_has_the_session_and_changes_nothing`.
  - `move_of_a_missing_session_is_not_found`.
  - `move_refuses_ids_that_could_leave_the_project_directory` (like the fork test).
  - `moving_within_the_same_project_dir_is_a_no_op_ok` (from and to escape to the same dir).
- [ ] **Step 2:** run `cargo test -p forge-adapter-claude-fs </dev/null` → FAIL.
- [ ] **Step 3: Implement** `move_session` in `ClaudeSessionAdapter`: validate the id (`valid_session_id`), compute both dirs with `project_dir`, refuse (`MoveFailed`) if the target file exists, `NotFound` if the source is missing, create the target dir, `tokio::fs::rename` the file; when rename fails with a cross-device error copy then delete; then move the same-named directory the same way if it exists. If moving the directory fails after the file moved, move the file back and return `MoveFailed`. Add the error variant and the trait method (the `automock` mock follows).
- [ ] **Step 4:** add `move_to` to the registry use case (maps the error as `fork` does) and a registry test `move_to_asks_the_repository_once`; update the hand-written fake. Run `cargo test -p forge-application -p forge-ports </dev/null` → PASS.
- [ ] **Step 5: Commit** `feat: move a session's transcript to another project directory`.

---

### Task 3: The live session can take a folder later and move

**Files:**
- Modify: `crates/forge-application/src/live_session.rs` (`LiveCommand::MoveTo(PathBuf)`, handle method), `crates/forge-application/src/live_session_actor.rs`
- Test: `crates/forge-application/src/live_session.rs` tests (fake sidecar; the existing `add_directories` tests at ~536, ~1722, ~1742 show the harness)

**Interfaces:**
- Produces: `LiveSessionHandle::move_to(&self, folder: PathBuf)`; changed behavior of `add_directories` (below). Consumes nothing new.

- [ ] **Step 1: Write the failing tests**:
  - `a_folder_added_after_the_first_send_restarts_the_sidecar_on_the_next_send_with_the_longer_list`: send a message, wait idle, `add_directories([/x])`, send again → the fake records a second `start_session` with `resume_session_id` set and `additional_directories == [/x]` (and the earlier folders kept).
  - `a_folder_added_while_a_turn_runs_is_ignored`.
  - `a_folder_added_twice_is_listed_once`.
  - `moving_changes_the_working_folder_and_the_next_send_resumes_there`: `move_to(/new)` while idle → view `project_path == /new` and `session.project_path == /new`; next send starts a sidecar with `cwd == /new` and the old Claude session id as `resume_session_id`.
  - `moving_while_a_turn_runs_is_ignored`.
  - `moving_drops_the_cached_extensions_so_they_are_resolved_again_for_the_new_folder` (the extensions resolver mock is called with the new path on the next start).
- [ ] **Step 2:** `cargo test -p forge-application live_session </dev/null` → FAIL.
- [ ] **Step 3: Implement.** In the actor's `add_directories`: if `self.sidecar.is_some() && !self.warm_only`, then when `self.view.phase != Phase::Running` push the new (de-duplicated) folders and `discard_sidecar(true)`, otherwise return. (`allow_tools` and `set_env` keep their rule; they are not used by this stage.) Add `LiveCommand::MoveTo`, handler `move_to(folder)`: ignore while `Phase::Running`; set `view.project_path` and `view.session.project_path`; `self.active = None; self.defaults = None;` discard the sidecar with shutdown; `dirty = true`. Update the doc comment of `LiveSessionHandle::add_directories`.
- [ ] **Step 4:** run the crate's tests → PASS.
- [ ] **Step 5: Commit** `feat: a running session takes a folder at its next start and can move to another folder`.

---

### Task 4: `폴더 추가…` in the card

**Files:**
- Modify: `crates/forge-features/src/environment.rs` (helper), `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/details_panel.rs`, `crates/forge-shared/src/ko.rs`
- Test: `crates/forge-features/src/environment.rs`

**Interfaces:**
- Produces: `pub struct FolderMenu { pub change: bool, pub add: bool, pub terminal: bool }`; `pub fn folder_menu(first: bool, busy: bool, worktree_session: bool) -> FolderMenu` (change and add only on `first`, and only when `!busy`; `change` also needs `!worktree_session`; `terminal` is `first`); `Workspace::add_session_folder(&self, id: LiveSessionId)`; `Workspace::session_is_busy(&self, id: LiveSessionId) -> bool`.

- [ ] **Step 1: Write the failing tests** in `environment.rs`: `an_added_row_has_no_change_or_add_item`; `the_first_row_has_both_items_when_idle`; `a_running_turn_disables_both_items` (both false, terminal still true); `a_worktree_session_has_no_change_item_but_keeps_add`.
- [ ] **Step 2:** `cargo test -p forge-features folder_menu </dev/null` → FAIL.
- [ ] **Step 3: Implement** `folder_menu`. `Workspace::add_session_folder(id)`: spawn; `pick_folder().await`; on `Some(folder)` that is neither the session's origin nor already in `session_added_folders(id)`, call `handle.add_directories(vec![folder.clone()])` and append to `added_folders[id]`. `session_is_busy` reads the live view's phase. In `FolderRow` render the items in the order `Finder에서 보기`, divider, `경로 복사`, `폴더 변경…` (Task 5 wires it; render it now disabled-free only when `menu.change`), `폴더 추가…`, divider, `터미널에서 열기`, per `folder_menu(first, busy, worktree)`; `폴더 추가…` calls `add_session_folder`. A disabled item is not shown (hidden, simplest). Pass `busy` and `worktree_session` (via `workspace.worktree_tag(&view.project_path).is_some()`) from `EnvironmentCard`. Add the two Korean entries for the labels now.
- [ ] **Step 4:** run the feature tests and `cargo build -p forge-widgets` → PASS.
- [ ] **Step 5: Commit** `feat: add a folder to a running session from the environment card`.

---

### Task 5: `폴더 변경…`, the trust question and the pinned name

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`, `crates/forge-widgets/src/details_panel.rs`, `crates/forge-widgets/src/lib.rs` and `chat_window.rs` or the root where `EnvironmentDialog` is mounted (mount the new dialog the same way), `crates/forge-widgets/src/start_line.rs`, `crates/forge-shared/src/ko.rs`
- Create: `crates/forge-widgets/src/workspace_trust_dialog.rs`
- Test: `crates/forge-features/src/workspace.rs` (pure helpers only), `crates/forge-shared`

**Interfaces:**
- Consumes: `environment_of_session`, `SessionEnvironmentRef` (Task 1); `registry.move_to` (Task 2); `LiveSessionHandle::move_to` (Task 3).
- Produces: `Workspace::session_environment(&self, project: &Path, session: Option<&SessionId>) -> Option<Environment>` (replaces the one-argument form; every caller is updated: `open_past_in`, `details_panel.rs`, `start_line.rs`); `Workspace::change_session_folder(&self, id: LiveSessionId)`; `Workspace::trust_request(&self) -> Option<TrustRequest>` with `pub struct TrustRequest { pub id: LiveSessionId, pub folder: PathBuf }` and `Workspace::answer_trust_request(&self, trust: bool)`; a signal-mirrored `session_environments` list loaded with the config next to `environments` (line ~525-539) and saved by `pin_environment(project, session_id, environment_id)` through `save_code_settings` (replace an existing ref for the same session id).

- [ ] **Step 1: Write the failing tests** for the pure parts: `should_move(current: &Path, picked: &Path) -> bool` (false for the same folder, also when only the trailing slash differs); `moved_message(folder: &Path) -> String` uses the folder's last component; `pins_after(list, project_old, new_project, session_id, environment_id) -> Vec<SessionEnvironmentRef>` replaces any ref for that session id and keeps the others. Put them in `environment.rs` of forge-features next to `folder_menu`.
- [ ] **Step 2:** run → FAIL.
- [ ] **Step 3: Implement the flow** in `change_session_folder(id)` (spawn): pick a folder; stop when `!should_move`; if `needs_trust(folder)` set the trust request signal and return (the dialog's answer continues with the same function body factored as `finish_change(id, folder)`); `Cancel` clears the request and does nothing; `작업 공간 신뢰` calls `trust_folder` then `finish_change`. `finish_change`: read the live view (project, Claude session id; none → notice, stop); compute the environment shown now with `session_environment`; call `registry.move_to(old, new, session_id)`; on `Err` report the error and stop (nothing else changed); on `Ok` call `handle.move_to(new)`, `pin_environment` when an environment was shown (pins re-keyed to the new project path), reload the sidebar projects for both folders (use the existing project reload helper `reload_project`), and `report(moved_message)`. Mount `WorkspaceTrustDialog` (title `이 워크스페이스를 신뢰하시겠습니까?`, a line with the folder path, buttons `Cancel` and `Trust workspace`; Esc cancels) like the other modal dialogs. Wire `폴더 변경…` in `FolderRow`. Update all `session_environment` callers to pass `view.claude_session_id.as_ref()` or the opened session id; `open_past_in` passes `Some(&session_id)`. A pinned session reopened in the new folder gets its environment's variables and extra folders through the existing `reopen_setup` path because the lookup now returns the pin.
- [ ] **Step 4:** add the Korean entries, run `cargo test -p forge-features -p forge-shared -p forge-domain </dev/null` and `cargo build --workspace` → PASS.
- [ ] **Step 5: Commit** `feat: change a session's folder from the environment card and keep the environment's name`.

---

### Task 6: Real-window check, docs

**Files:**
- Modify: the spec (new "As built" and "Checked" sections), `docs/superpowers/roadmap/backlog.md` (OTH-11, PNL-08 rows), `docs/superpowers/roadmap/observations.md` only if something new is seen.

- [ ] **Step 1:** Harness check with the fake Claude home (see `scripts/harness/README.md`; seed environments by editing `/private/tmp/forge-harness-home/forge-config/config.json`, paste with pbcopy and key code 9, throwaway folders under `/private/tmp/forge-c-*`): the menu items on the first and an added row and their absence while a turn runs (a fixture session cannot run a turn: check the idle state only); `폴더 추가…` adds a row; `폴더 변경…` to an untrusted folder shows the trust dialog (`취소` changes nothing, `작업 공간 신뢰` moves), the card keeps the environment name and gear, the sidebar group, header tag and a notice follow; deleting the environment turns the card back to `이 컴퓨터` + `저장`; moving onto the same folder does nothing; a worktree session has no `폴더 변경…`.
- [ ] **Step 2:** Real turns (ask the user for permission before any real message; at most two): create a throwaway real session in a folder, `폴더 변경…` to a second folder, send a probe message asking Claude to run `pwd` and see the new folder and the earlier conversation remembered (resume worked); `폴더 추가…` a folder with `probe.txt` and ask Claude to `cat` it. Delete everything created in the real `~/.claude/projects` by the run (list before and after), never the real Forge config.
- [ ] **Step 3:** Write "As built", what was seen, what was not checked (Windows, a pinned session after an app restart if not run), and the rulings; update the backlog rows; run `cargo test -p forge-domain -p forge-features -p forge-shared -p forge-application -p forge-adapter-claude-fs </dev/null` and `cargo build --workspace`.
- [ ] **Step 4: Commit** `docs: Plan 53 stage C as built`.
