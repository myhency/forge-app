# "No folder" sessions (Plan 54) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A person can start a session without choosing a folder: Forge makes an empty scratch folder under its own data directory and starts the session there; the sidebar groups such sessions under `이 컴퓨터`; archiving removes an empty scratch folder.

**Architecture:** Pure helpers in `forge-domain` (`scratch.rs`: folder name, `is_scratch`). A `ScratchPort` (create a folder, remove it when empty) with a filesystem adapter, used by a `ScratchUseCase` that owns the scratch root; it is a new field of `WorkspaceServices`, wired in `forge-app`. The workspace gets a `new_no_folder` signal (the new-session screen's "no folder" choice); `submit` makes the scratch folder before `start_new`. The sidebar labels scratch projects `이 컴퓨터`, its `+` starts a no-folder session, and `archive_session` releases an empty scratch folder like it releases a clean worktree.

**Tech Stack:** Rust (Dioxus 0.7.10, crates `forge-*`).

**Spec:** `docs/superpowers/specs/2026-10-20-no-folder-session-design.md` (binding, decisions A/B/C recorded in it). Evidence: observations Round 22.

## Global Constraints

- Local only. Scratch root = `<parent folder of the config file>/scratch-workspaces` (config path from `FORGE_CONFIG_PATH` or `default_config_path()`); if there is no config path, no-folder sessions are unavailable (the row is not shown). Folder name `scratch-<YYYY-MM-DD>-<6 hex digits>`.
- A scratch folder is never added to the trusted-folder list, never offered in the folder menu's recent list, and never created when the first send fails earlier (make it right before `start_new` opens the session; if making it fails: notice, text comes back, nothing starts).
- Archiving removes a scratch folder only when it is empty (no entries at all) and no other open or un-archived session lives in it; a folder with files is kept and nothing is said. Never delete anything outside the scratch root: `ScratchPort::remove_if_empty` must refuse a path that is not directly inside the root (a `..` or a symlink escape must not pass), and uses `remove_dir` (never recursive).
- Sidebar: a scratch project's group heading reads `이 컴퓨터` (Korean for "This computer", through `tr!`); its `+` starts a no-folder session; the group id stays the folder path (so collapsing works).
- Every new user-visible string goes through `tr!`/`msg!` with a Korean entry (`python3 scripts/i18n_add.py`; `cargo test -p forge-shared`). Existing keys: `This computer` ("이 컴퓨터"), `Choose a folder` ("폴더 선택").
- Rust style of the repo: comments only where the reason is not obvious; tests next to the code, sentence names. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (your own model name if your attribution instructions say so).

## Review Focus

- The root check of `remove_if_empty`: a path outside the root, a path equal to the root, `<root>/../x`, a symlink inside the root pointing outside, a non-empty folder, a missing folder — none may remove anything but an empty direct child. (Task 2.)
- A person with no past projects and no extra folders can type and send (composer enabled) when `폴더 없음` is chosen. (Task 3.)
- Choosing a recent folder or `폴더 열기…` after `폴더 없음` unchecks it and sends into that folder; the chosen environment (stage A) takes precedence over `폴더 없음`. (Task 3.)
- A scratch folder shows in the sidebar only as `이 컴퓨터` and never in the new-session folder menu; opening an old scratch session from the sidebar still works. (Task 4.)
- Archiving a scratch session whose folder gained files keeps the folder; archiving one of two sessions in the same scratch folder keeps it. (Task 5.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/scratch.rs` (new) | `scratch_folder_name`, `is_scratch`, `SCRATCH_DIR` |
| `crates/forge-ports/src/scratch_port.rs` (new), `lib.rs` | `ScratchPort` |
| `crates/forge-adapter-files/src/scratch.rs` (new) or the adapter crate the repo uses for plain fs access | `ScratchFs` |
| `crates/forge-application/src/use_cases/scratch.rs` (new), `mod.rs` | `ScratchUseCase { root, port }` |
| `crates/forge-app/src/main.rs` | wiring |
| `crates/forge-features/src/workspace.rs`, `new_session.rs`, `folder_menu.rs` | `new_no_folder`, submit, menu rows, sidebar label helper, archive release |
| `crates/forge-widgets/src/new_session_controls.rs`, `sidebar.rs` | `폴더 없음` row and chip label; heading label and `+` |
| `crates/forge-shared/src/ko.rs`, spec, backlog | Korean text, As built |

---

### Task 1: Domain helpers

**Files:** Create `crates/forge-domain/src/scratch.rs`; modify `crates/forge-domain/src/lib.rs`.

**Interfaces — Produces:**
- `pub const SCRATCH_DIR: &str = "scratch-workspaces";`
- `pub fn scratch_folder_name(year: i32, month: u8, day: u8, seed: u64) -> String` → `scratch-2026-10-20-ab12cd` (zero-padded month/day, the low 24 bits of `seed` as 6 lower-case hex digits).
- `pub fn is_scratch(root: &Path, path: &Path) -> bool` — true only when `path` is a direct child of `root` (compared as components; `root` itself, deeper paths, `..` components and unrelated paths are false) and its file name starts with `scratch-`.
- `pub fn scratch_root(config_file: &Path) -> Option<PathBuf>` → `config_file.parent()?.join(SCRATCH_DIR)`.

- [ ] **Step 1: Write failing tests** (in `scratch.rs`): name format incl. zero padding and 6 hex digits for a seed with high bits set; `is_scratch` true for `<root>/scratch-2026-10-20-ab12cd`; false for the root itself, `<root>/scratch-x/deeper`, `<root>/other`, `/elsewhere/scratch-2026-10-20-ab12cd`, `<root>/../scratch-a`; `scratch_root(Path::new("/c/Forge/config.json")) == Some("/c/Forge/scratch-workspaces")`, and `None` for `Path::new("config.json")`'s parent being empty? (`Path::new("config.json").parent()` is `Some("")`: then the root is `scratch-workspaces` relative — acceptable; test only the absolute case and `Path::new("/")` → `None`? `/`.parent() is `None`: assert that.)
- [ ] **Step 2: Run** `cargo test -p forge-domain scratch` — FAIL (missing).
- [ ] **Step 3: Implement** the four items; export them from `lib.rs`.
- [ ] **Step 4: Run** `cargo test -p forge-domain` — PASS.
- [ ] **Step 5: Commit** `feat: scratch folder names and the scratch check in the domain`.

---

### Task 2: Port, filesystem adapter, use case, wiring

**Files:** Create `crates/forge-ports/src/scratch_port.rs`; create the adapter (look at `forge-adapter-files` for a plain-fs adapter and put `ScratchFs` there; keep its dependency list as small as it is); create `crates/forge-application/src/use_cases/scratch.rs`; modify `crates/forge-ports/src/lib.rs`, `crates/forge-application/src/use_cases/mod.rs` (+ crate exports), `crates/forge-features/src/workspace.rs` (`WorkspaceServices.scratch: Arc<ScratchUseCase>`), `crates/forge-app/src/main.rs`, and every test/helper that builds `WorkspaceServices` (let the compiler find them).

**Interfaces:**
- Consumes: `scratch_folder_name`, `is_scratch`, `scratch_root` (Task 1).
- Produces:
  - `#[async_trait] pub trait ScratchPort: Send + Sync { async fn create_dir(&self, path: &Path) -> Result<(), ScratchError>; async fn is_empty_dir(&self, path: &Path) -> Result<bool, ScratchError>; async fn remove_empty_dir(&self, path: &Path) -> Result<(), ScratchError>; }`, `ScratchError(String)` (`thiserror`, like the repo's other port errors). Mockall `automock` behind the `mocks` feature like the other ports.
  - `ScratchFs` implementing it: `create_dir` = `create_dir_all`; `is_empty_dir` = directory with no entries (false for a file or missing); `remove_empty_dir` = `std::fs::remove_dir` (fails on a non-empty folder).
  - `pub struct ScratchUseCase` with `new(root: Option<PathBuf>, port: Arc<dyn ScratchPort>)`; `pub fn available(&self) -> bool` (root is `Some`); `pub fn root(&self) -> Option<&Path>`; `pub async fn create(&self, year: i32, month: u8, day: u8, seed: u64) -> Result<PathBuf, ScratchError>` (makes `<root>/<name>`; tries up to 5 seeds `seed, seed+1…` if the folder already exists (is_empty_dir true or any existing path) so two sessions never share a folder; error when unavailable); `pub async fn release(&self, path: &Path) -> Result<ScratchRelease, ScratchError>` with `enum ScratchRelease { Removed, Kept, NotScratch }`: `NotScratch` when `!is_scratch(root, path)` or unavailable; `Kept` when the folder is not empty or missing; `Removed` after `remove_empty_dir`. Path checks use the root normalised by `canonicalize` of the root only when it exists; the candidate must be a direct child by `is_scratch` AND `path.canonicalize()` (when it exists) must still be a direct child of the canonical root (a symlink escape is `NotScratch`).
  - `forge-app` builds `ScratchUseCase::new(scratch_root(&config_path), Arc::new(ScratchFs))` from the same `config_path` it already resolves (`FORGE_CONFIG_PATH` override included).

- [ ] **Step 1: Write failing tests** with a temp directory (std only, like existing fs adapter tests; `std::env::temp_dir().join(unique)`): `ScratchFs` create/is_empty/remove behaviours (non-empty folder: `remove_empty_dir` errors and the folder stays); use case: `create` makes `<root>/scratch-…` and returns it; a second `create` with the same date and seed gets a different folder; `release` of an empty child → `Removed` and the folder is gone; of a child holding a file → `Kept` and the file stays; of the root itself, of `<root>/..`, of a sibling of the root, of a path outside → `NotScratch` and nothing removed; of a missing child → `Kept`; with `root = None`, `available()` is false, `create` errors, `release` → `NotScratch`; on unix a symlink `<root>/scratch-link -> <other empty dir>` → `NotScratch` and the target survives (`#[cfg(unix)]`).
- [ ] **Step 2: Run** the tests — FAIL.
- [ ] **Step 3: Implement** as specified; wire it in `forge-app`; add `scratch: Arc<ScratchUseCase>` to `WorkspaceServices`; fix every builder of `WorkspaceServices` found by `cargo build --workspace --tests`.
- [ ] **Step 4: Run** `cargo test -p forge-ports -p forge-application -p forge-adapter-files` (use the adapter's real crate name) and `cargo build --workspace --tests` — PASS.
- [ ] **Step 5: Commit** `feat: scratch folder port, adapter and use case`.

---

### Task 3: New-session screen — `폴더 없음`

**Files:** Modify `crates/forge-features/src/workspace.rs`, `new_session.rs`, `folder_menu.rs`; `crates/forge-widgets/src/new_session_controls.rs`.

**Interfaces:**
- Consumes: `ScratchUseCase` (Task 2) through `WorkspaceServices.scratch`.
- Produces:
  - `Workspace::new_no_folder() -> bool`, `Workspace::set_new_no_folder(bool)` (a `Signal<bool>` field, default false); `set_new_folder(Some(..))` sets it to false (choosing a folder unchecks `폴더 없음`); `Workspace::scratch_available() -> bool`.
  - `folder_menu_rows` unchanged; the widget adds the row.
  - In `NewSessionComposer::submit`: with no environment picked and `new_no_folder()` true: make the scratch folder (`services.scratch.create(year, month, day, seed)` from `time::OffsetDateTime::now_utc()` (local date is fine to use UTC if local is not at hand; say which in a comment) and the clock's nanoseconds) and use `ProjectSummary { path: <scratch>, last_active: String::new(), session_count: 0, exists: true }`; no worktree request; on failure `workspace.report(tr!("Could not make a folder for this session: {error}", error = error))` and return BEFORE `take_for_new_session` (text kept). Because creating is async and `submit` is sync, do it the way `start_new` already does asynchronous work: add an optional parameter to `start_new` (`scratch: bool`) so the folder is made inside its `spawn_forever` block before the session opens; on failure it sets `new_unsent` with the text and reports (like the worktree failure). Do NOT create the folder before the text is taken and then forget it on a later failure: the folder is made inside `start_new` right before `open_new_with_effort`, and on any later failure the helper that already releases worktrees also releases the scratch folder (it is empty).
  - `has_folder` in `NewSessionComposer` is true when an environment is picked OR `new_no_folder()` OR there are projects.
  - Widget: the folder chip's menu gets `폴더 없음` as its first row (checked when `new_no_folder()`), only when `scratch_available()`; picking it calls `set_new_no_folder(true)` and `set_new_folder(None)`; the chip label is `Choose a folder` while it is checked; with no past projects the menu still opens. `AddFolderButton` and `BranchChip` stay as they are (the branch chip needs a repo; none for no folder).
  - The first time with no past projects and no extra folders the banner `No existing project folder was found…` must not show when `new_no_folder()` is true.

- [ ] **Step 1: Write failing tests** for the pure parts: add `pub fn no_folder_rows_checked(no_folder: bool, ...)`? Keep it simple: test a small pure function `fn can_send_here(picked_environment: bool, no_folder: bool, projects: usize) -> bool` placed in `new_session.rs` (extract the `has_folder` expression into it): true for (true,_,0), (false,true,0), (false,false,1); false for (false,false,0). And test `Workspace`-free logic of the label choice if any.
- [ ] **Step 2: Run** `cargo test -p forge-features can_send_here` — FAIL.
- [ ] **Step 3: Implement** everything above; build and run `cargo test -p forge-features -p forge-widgets`.
- [ ] **Step 4: Window check (harness: `scripts/harness/README.md`; stop it only at the very end with `stop.sh`; send NO message to Claude)**: the folder menu shows `폴더 없음` first; picking it checks it and the chip reads `폴더 선택`/`Choose a folder` text of the app's language; picking a recent folder unchecks it; with the fixture's folders removed from the list (rename the fixture's projects away or use a fresh `FORGE_CLAUDE_HOME` with no projects: look at `scripts/harness/build_fixture.py` to see how to make an empty one) the composer is enabled when `폴더 없음` is chosen. Report what you saw.
- [ ] **Step 5: Commit** `feat: start a session without a folder in a scratch folder`.

---

### Task 4: Sidebar — `이 컴퓨터` group

**Files:** Modify `crates/forge-features/src/workspace.rs` (`new_session_projects` excludes scratch projects; a `is_scratch_project(&Path) -> bool` method), `crates/forge-features/src/sidebar.rs` or where `labels` (project labels) are made, `crates/forge-widgets/src/sidebar.rs`.

**Interfaces:** Consumes `ScratchUseCase::root()`. Produces:
- `Workspace::is_scratch_project(&self, path: &Path) -> bool` (uses `is_scratch(root, path)`).
- Project labels: a scratch project's label is `tr!("This computer")` (find where `labels.get(&project.path)` is built and give scratch paths that label instead of the folder name; the disambiguation logic for equal folder names must not see scratch folders).
- `new_session_projects()` filters scratch projects out of the list the folder menu shows.
- The sidebar group header's `+` for a scratch project calls `workspace.start_no_folder_session()` = `set_new_no_folder(true)`, `set_new_folder(None)` … careful: `set_new_folder` sets no_folder false; implement as `set_new_no_folder(true)` after clearing the folder, then `show_new_session()`; the `+` title for scratch is `New session` (existing key; check) instead of `New session in this folder`.
- Search results/rows that show `project_name` for a scratch session use the same `이 컴퓨터` label.

- [ ] **Step 1: Write failing tests** for the pure parts: a function `project_labels` (or the existing one) given a scratch path returns `This computer` and does not shift the labels of other folders; `is_scratch_project` equivalent over the pure `is_scratch`.
- [ ] **Step 2: Run** — FAIL; **Step 3: Implement**; **Step 4: Run** `cargo test -p forge-features -p forge-widgets`.
- [ ] **Step 5: Window check (harness, no message sent)**: add a scratch folder with a fake session to the fixture: create `<fixture config dir>/scratch-workspaces/scratch-2026-10-20-aaaaaa/` and a transcript for it under the fixture's `.claude/projects/` (see `scripts/harness/build_fixture.py` for the format: copy how it writes `my-app`'s project; the transcript's `cwd` must be the scratch path); the sidebar shows a group `이 컴퓨터` with that session; its `+` opens the new-session screen with `폴더 없음` checked; the folder menu does not list the scratch folder. Report what you saw.
- [ ] **Step 6: Commit** `feat: scratch sessions group under 이 컴퓨터 in the sidebar`.

---

### Task 5: Archive releases an empty scratch folder; Korean text; docs

**Files:** Modify `crates/forge-features/src/workspace.rs` (`archive_session`), `crates/forge-shared/src/ko.rs`, the spec, `docs/superpowers/roadmap/backlog.md`.

**Interfaces:** Consumes `ScratchUseCase::release`, `worktree_in_use` (existing helper: reuse it for "another open or un-archived session lives in this folder"; read its doc comment).

- `archive_session(project, session)`: after `release_worktree`, when `is_scratch_project(&project)`: in the same style (root scope `spawn_forever`; skip when `worktree_in_use(..)` says another session uses the folder; call `services.scratch.release(&project)`; on `Removed` call `this.reload_past()`; on error `this.report(tr!("Could not remove the folder {name}: {error}", ..))`; `Kept`/`NotScratch` silent).
- [ ] **Step 1: Failing test** for any pure part (for example the decision `should_release_scratch(is_scratch, in_use) -> bool`); **Step 2–3**: implement.
- [ ] **Step 4: Korean text**: run `cargo test -p forge-shared`; add entries for new strings: "Could not make a folder for this session: {error}" → "이 세션의 폴더를 만들 수 없습니다: {error}", "Could not remove the folder {name}: {error}" → "폴더 {name}을(를) 삭제하지 못했습니다: {error}" (adjust to the style of the neighbouring entries), plus any other new key.
- [ ] **Step 5: Window check (harness, no message)**: archive the fixture's scratch session from Task 4's check (keyboard ⌥⌘A or the row menu): an empty scratch folder disappears from disk and the group from the list; with a file put into the folder first, the folder stays; with two sessions in one folder, archiving one keeps the folder. Report.
- [ ] **Step 6: Docs**: spec gets an "As built" section (what was built, window checks done, NOT done: the end-to-end start with a real message — a real turn on the user's account — which the controller asks the user for; Windows path behaviour of the data directory not run); backlog row OTH-11 loses `폴더 없음` from "Not built"; add a row OTH-12 `No-folder sessions` in the OTH table's four-column shape.
- [ ] **Step 7: Run** `cargo test --workspace --no-fail-fast` once (the `forge-adapter-claude-plugins round_trip` test fails on this machine because it reads the real plugin directory; one timing test is flaky: report, do not change); commit `docs: no-folder sessions built (Plan 54), Korean text`.

---

## Self-review (against the spec)

Menu row (Task 3), scratch folder under Forge's data dir (Tasks 1–2, wiring), sidebar `이 컴퓨터` group and `+` (Task 4), no default-folder dead end (Task 3 `has_folder`, banner), cleanup on archive with the safety rules (Tasks 2, 5), environments take precedence (Task 3), Out of scope respected.
