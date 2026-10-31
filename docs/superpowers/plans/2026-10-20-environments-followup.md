# Environments follow-up (Plan 53, stage A2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every git folder of an environment gets its own branch choice and `워크트리` choice (not only the first), and two layout defects of stage A are fixed: the folder popover of the dialog is clipped at the bottom, and a long environment name runs under the gear icon of the chip menu.

**Architecture:** `Environment` keeps its first-folder fields (`branch`, `worktree`) so every stored config still loads, and gains `extra_choices: Vec<FolderChoice>` aligned with `folders[1..]`. The dialog's draft holds one `FolderChoice` per folder. `start_plan` returns one optional worktree request per extra folder; `Workspace::start_new` creates those worktrees after the first folder's and passes their paths as the added directories. Each folder row of the dialog probes its own branches.

**Tech Stack:** Rust (Dioxus 0.7.10, crates `forge-*`).

**Spec:** `docs/superpowers/specs/2026-10-19-environments-design.md` (binding) and `docs/superpowers/plans/2026-10-19-environments.md` (what was built). Evidence: observations Round 22 (`docs/superpowers/roadmap/observations.md`). Decided with the user on 2026-10-20: option A below.

## Global Constraints

- Local only; no cloud, SSH or Remote Control anywhere.
- Variable values never appear in Debug/log/error output (carry them only as `EnvVars`; `Environment` and `EnvironmentDraft` already redact `env_vars` in `Debug`; any new `Debug` impl must too).
- Every config written by stage A (`folders`, `branch`, `worktree` only) must load unchanged and mean the same: the first folder's choice stays where it is; a missing `extra_choices` means "no branch chosen, no worktree" for every extra folder.
- Option A (user's choice): every git folder shows a branch picker (the repository's current branch while none is chosen) and a `Worktree` checkbox; the first folder's checkbox is on by default as before, the extra folders' checkboxes are off by default (as in the desktop app). At send time each folder whose box is on gets its own worktree (from its chosen branch, or its current branch), the first folder's as the working folder, the others' paths as the added directories. A folder that is not a git repository starts in itself, as before.
- Known limit, to be written in the spec's "As built": archiving a session removes only the first folder's worktree (when clean); worktrees made for extra folders stay on disk and in the `worktrees` records until removed by hand.
- Every new user-visible string goes through `tr!`/`msg!` with a Korean entry (`python3 scripts/i18n_add.py`; `cargo test -p forge-shared` shows what is missing).
- Rust style of the repo: comments only where the reason is not obvious; tests next to the code, sentence names.
- Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (use your own model name if your attribution instructions say so).

## Review Focus

- A stage-A config (no `extra_choices`) loads and a plan from it is identical to before (one worktree request for the first folder at most). (Task 1.)
- Removing or de-duplicating a folder in the draft or in `normalized` keeps `extra_choices` aligned with the folders that remain (a choice must never move to another folder). (Task 1.)
- One extra folder's worktree fails to be made (not a repository, git error): nothing starts, the typed text comes back, the notice names the problem; worktrees already made for that send are not left half-recorded without telling the person. (Task 2.)
- Two folders of the same repository (same repo path twice cannot happen: folders are deduplicated) or two repositories with equal names get distinct worktree names. (Task 2.)
- The popover and the menu stay usable in a short window and with long names. (Task 3.)

---

## File Structure

| File | Responsibility |
| --- | --- |
| `crates/forge-domain/src/environment.rs` | `FolderChoice`, `Environment.extra_choices`, alignment in `normalized` |
| `crates/forge-features/src/environment.rs` | draft with one choice per folder, `build`, `start_plan` with per-extra worktree requests |
| `crates/forge-features/src/workspace.rs`, `new_session.rs` | `start_new` creates the extra worktrees |
| `crates/forge-widgets/src/environment_dialog.rs` | one component per folder row (own branch probe), popover placement |
| `crates/forge-widgets/src/environment_chip.rs`, `crates/forge-shared/src/theme.css` | long names vs. the gear icon, popover styles |
| `crates/forge-shared/src/ko.rs`, spec | Korean text, "As built" |

---

### Task 1: Domain and draft — one choice per folder

**Files:**
- Modify: `crates/forge-domain/src/environment.rs`
- Modify: `crates/forge-features/src/environment.rs`
- Modify: `crates/forge-features/src/lib.rs` (exports)

**Interfaces:**
- Produces:
  - `forge_domain::FolderChoice { pub branch: Option<String>, pub worktree: bool }` (`Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize`, every field `#[serde(default)]`) — exported from `forge-domain`.
  - `Environment.extra_choices: Vec<FolderChoice>` (`#[serde(default)]`), aligned with `folders[1..]` (entry `i` belongs to `folders[i + 1]`).
  - `Environment::normalized` keeps the alignment: it pads `extra_choices` with defaults or truncates it to `folders.len().saturating_sub(1)` entries after de-duplicating, and a folder dropped as a duplicate takes its choice with it.
  - `EnvironmentDraft.choices: Vec<FolderChoice>` — one entry per folder, aligned with `folders` (entry 0 is the first folder's choice; it replaces the draft's old `branch` and `worktree` fields, which are removed).
  - `EnvironmentDraft::push_folder(&mut self, folder: PathBuf)` (adds the folder and a choice: `worktree: true` for the very first folder, `false` for later ones, `branch: None`; ignores a folder already present) and `EnvironmentDraft::remove_folder(&mut self, index: usize)` (removes folder and choice together).
  - `EnvironmentDraft::blank(name, folder)` uses `push_folder`; `EnvironmentDraft::of(&Environment)` builds `choices` from `branch`/`worktree` and `extra_choices` (padded to the folder count); `build` writes `choices[0]` back to `branch`/`worktree` and `choices[1..]` to `extra_choices`.
  - `EnvironmentStart.extra_worktrees: Vec<Option<NewWorktree>>` aligned with `EnvironmentStart.extra` (entry `i` is for `extra[i]`; `Some` when that folder's `worktree` is on).

- [ ] **Step 1: Write the failing tests**

In `crates/forge-domain/src/environment.rs` tests:

```rust
    fn two_folder_env() -> Environment {
        Environment {
            id: "env-1".into(),
            name: "w".into(),
            folders: vec!["/a".into(), "/b".into(), "/c".into()],
            branch: Some("main".into()),
            worktree: true,
            env_vars: String::new(),
            pinned: false,
            created_at: 0,
            last_used_at: 0,
            extra_choices: vec![
                FolderChoice { branch: Some("dev".into()), worktree: true },
                FolderChoice { branch: None, worktree: false },
            ],
        }
    }

    #[test]
    fn a_stage_a_environment_without_extra_choices_loads_with_default_choices() {
        let text = r#"{"id":"e","name":"n","folders":["/a","/b"],"branch":"main","worktree":true}"#;
        let environment: Environment = serde_json::from_str(text).unwrap();
        assert!(environment.extra_choices.is_empty());
        let normalized = environment.normalized();
        assert_eq!(normalized.extra_choices, vec![FolderChoice::default()]);
        assert_eq!(normalized.branch.as_deref(), Some("main"));
    }

    #[test]
    fn normalizing_keeps_each_choice_with_its_folder_when_a_duplicate_is_dropped() {
        let mut environment = two_folder_env();
        environment.folders = vec!["/a".into(), "/b".into(), "/b".into(), "/c".into()];
        environment.extra_choices = vec![
            FolderChoice { branch: Some("dev".into()), worktree: true },
            FolderChoice { branch: Some("dup".into()), worktree: true },
            FolderChoice { branch: None, worktree: false },
        ];
        let normalized = environment.normalized();
        assert_eq!(normalized.folders, vec![std::path::PathBuf::from("/a"), "/b".into(), "/c".into()]);
        assert_eq!(normalized.extra_choices, vec![FolderChoice { branch: Some("dev".into()), worktree: true }, FolderChoice { branch: None, worktree: false }]);
    }

    #[test]
    fn normalizing_pads_or_truncates_the_choices_to_the_extra_folders() {
        let mut environment = two_folder_env();
        environment.extra_choices.push(FolderChoice { branch: Some("x".into()), worktree: true });
        assert_eq!(environment.clone().normalized().extra_choices.len(), 2);
        environment.folders.truncate(1);
        assert!(environment.normalized().extra_choices.is_empty());
    }
```

(Existing tests that build an `Environment` literal need `extra_choices: Vec::new(),` added.)

In `crates/forge-features/src/environment.rs` tests (the helper `env(..)` literal also gets `extra_choices: Vec::new(),`):

```rust
    #[test]
    fn a_draft_keeps_one_choice_per_folder_and_the_first_folder_defaults_to_a_worktree() {
        let mut draft = EnvironmentDraft::blank("app".to_string(), Some("/a".into()));
        draft.push_folder("/b".into());
        draft.push_folder("/a".into());
        assert_eq!(draft.folders.len(), 2);
        assert_eq!(draft.choices, vec![FolderChoice { branch: None, worktree: true }, FolderChoice { branch: None, worktree: false }]);
        draft.remove_folder(0);
        assert_eq!(draft.folders, vec![std::path::PathBuf::from("/b")]);
        assert_eq!(draft.choices.len(), 1);
    }

    #[test]
    fn building_and_reading_a_draft_round_trips_every_folders_choice() {
        let mut draft = EnvironmentDraft::blank("app".to_string(), Some("/a".into()));
        draft.push_folder("/b".into());
        draft.choices[0].branch = Some("main".into());
        draft.choices[1] = FolderChoice { branch: Some("dev".into()), worktree: true };
        let environment = draft.build(None, 1, 5);
        assert_eq!((environment.branch.as_deref(), environment.worktree), (Some("main"), true));
        assert_eq!(environment.extra_choices, vec![FolderChoice { branch: Some("dev".into()), worktree: true }]);
        assert_eq!(EnvironmentDraft::of(&environment).choices, draft.choices);
    }

    #[test]
    fn the_start_plan_asks_a_worktree_for_each_extra_folder_whose_box_is_on() {
        let mut environment = env("a", "app", false, 0);
        environment.folders = vec!["/a".into(), "/b".into(), "/c".into()];
        environment.worktree = false;
        environment.extra_choices = vec![FolderChoice { branch: Some("dev".into()), worktree: true }, FolderChoice::default()];
        let plan = start_plan(&environment).unwrap();
        assert_eq!(plan.worktree, None);
        assert_eq!(plan.extra, vec![std::path::PathBuf::from("/b"), "/c".into()]);
        assert_eq!(plan.extra_worktrees, vec![Some(NewWorktree { base: Some("dev".into()) }), None]);
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p forge-domain -p forge-features environment 2>&1 | tail -15` — Expected: FAIL to compile (`FolderChoice`, `choices`, `push_folder`, `extra_worktrees`).

- [ ] **Step 3: Implement**

`crates/forge-domain/src/environment.rs`:

```rust
/// The branch and worktree choice of one folder of an environment.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct FolderChoice {
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub worktree: bool,
}
```

Add to `Environment` (after `worktree`): `/// Choices of folders[1..], entry i for folders[i + 1]; missing entries read as the default.` `#[serde(default)] pub extra_choices: Vec<FolderChoice>,`; add it to the manual `Debug` impl (`.field("extra_choices", &self.extra_choices)`); rewrite `normalized`:

```rust
    pub fn normalized(mut self) -> Environment {
        self.name = self.name.trim().to_string();
        let folders = std::mem::take(&mut self.folders);
        let mut choices = std::mem::take(&mut self.extra_choices).into_iter();
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut kept: Vec<FolderChoice> = Vec::new();
        for (index, folder) in folders.into_iter().enumerate() {
            // The choice of folders[index] (index 0 has none here: it lives in `branch` and `worktree`).
            let choice = if index == 0 { None } else { Some(choices.next().unwrap_or_default()) };
            if seen.contains(&folder) {
                continue;
            }
            seen.push(folder);
            if let Some(choice) = choice {
                kept.push(choice);
            }
        }
        self.folders = seen;
        kept.resize(self.folders.len().saturating_sub(1), FolderChoice::default());
        self.extra_choices = kept;
        if self.folders.is_empty() {
            self.branch = None;
            self.worktree = false;
        }
        self
    }
```

Export `FolderChoice` from `forge-domain`'s `lib.rs` next to `Environment`.

`crates/forge-features/src/environment.rs`: replace the draft's `branch` and `worktree` fields by `pub choices: Vec<FolderChoice>`; implement `blank` (`let mut draft = Self { id: None, name, folders: Vec::new(), choices: Vec::new(), env_vars: String::new() }; if let Some(folder) = folder { draft.push_folder(folder); } draft`), `push_folder` (`if self.folders.contains(&folder) { return; } let first = self.folders.is_empty(); self.folders.push(folder); self.choices.push(FolderChoice { branch: None, worktree: first });`), `remove_folder` (`if index < self.folders.len() { self.folders.remove(index); self.choices.remove(index); }`), `of` (`choices`: first = `FolderChoice { branch: environment.branch.clone(), worktree: environment.worktree }` when there is a first folder, then `extra_choices` cloned and padded with defaults to `folders.len() - 1`), `build` (first choice → `branch`/`worktree` (`None`/`false` when no folder), the rest → `extra_choices`, then `.normalized()` as before). Replace the hand-written `Debug` of `EnvironmentDraft` to print `choices` instead of `branch`/`worktree`. `EnvironmentStart` gets `pub extra_worktrees: Vec<Option<NewWorktree>>`; `start_plan` fills it from `normalized.extra_choices` (`choice.worktree.then(|| NewWorktree { base: choice.branch.clone() })`). Export `FolderChoice` use: the features crate imports it from `forge_domain` where needed. Update every other use of `draft.branch` / `draft.worktree` in the workspace (compiler-driven; `environment_dialog.rs` is rebuilt in Task 3, so keep it compiling with minimal edits: read/write `draft.choices[0]` where it used `draft.branch`/`draft.worktree`).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p forge-domain -p forge-features 2>&1 | tail -8`; `cargo build --workspace 2>&1 | tail -5` — Expected: PASS and builds.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: an environment keeps a branch and worktree choice for every folder

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Start path — a worktree for each chosen extra folder

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (`start_new`)
- Modify: `crates/forge-features/src/new_session.rs` (nothing but the call, if the signature changes; none planned)

**Interfaces:**
- Consumes: `EnvironmentStart.extra_worktrees` (Task 1); `WorktreesUseCase::{branches, create}` (existing; `create(&repo, base: Option<String>, message: &str, seed: u64, created_at: i64) -> Result<WorktreeRecord, _>`).
- Produces: `start_new` makes the worktrees of the extra folders and passes their paths in place of the folders as added directories.

Behaviour of `start_new` with `Some(environment)`:
1. The first folder's worktree is made as today (checked with `branches` first; a non-repository starts in the folder itself).
2. Then, for each `i` with `environment.extra_worktrees[i] == Some(request)`: if `services.worktrees.branches(extra[i])` is `Err` (not a repository) the folder stays as it is (no worktree); otherwise `services.worktrees.create(&extra[i], request.base, &text, seed.wrapping_add(i as u64 + 1), created_at)`; on `Ok(record)` push the record to `this.worktrees` (as the first folder's code does) and use `record.path` instead of `extra[i]`. On `Err(error)`: `this.report(tr!("Could not create the worktree: {error}", error = creation_error_text(&error)))`, put the text back with `new_unsent`, and `return` before anything is opened. (Worktrees already made for this send stay recorded; the notice says so: use the existing message and append nothing — the records are visible in the sidebar's worktree grouping and are removed by hand; see the spec's "As built".)
3. The added directories are the composer's folders plus the resulting extra paths, without duplicates (as now).

- [ ] **Step 1: Write the failing test**

`start_new` is UI-bound. Move the pure part into `crates/forge-features/src/environment.rs` and test it there:

```rust
/// The folder an extra folder becomes: its worktree when one was made, else itself.
pub fn extra_paths(extra: &[PathBuf], made: &[Option<PathBuf>]) -> Vec<PathBuf> {
    extra.iter().enumerate().map(|(index, folder)| made.get(index).cloned().flatten().unwrap_or_else(|| folder.clone())).collect()
}
```

```rust
    #[test]
    fn an_extra_folder_becomes_its_worktree_when_one_was_made_and_stays_itself_otherwise() {
        let extra: Vec<PathBuf> = vec!["/b".into(), "/c".into(), "/d".into()];
        let made = vec![Some(PathBuf::from("/b/.claude/worktrees/x")), None];
        assert_eq!(extra_paths(&extra, &made), vec![PathBuf::from("/b/.claude/worktrees/x"), "/c".into(), "/d".into()]);
    }
```

Run: `cargo test -p forge-features an_extra_folder 2>&1 | tail -6` — Expected: FAIL (function missing); then add `extra_paths` (and export it from `lib.rs` if `workspace.rs` needs the path `crate::environment::extra_paths`, no export needed inside the crate) — Expected: PASS.

- [ ] **Step 2: Implement `start_new`**

In the `spawn_forever` block of `Workspace::start_new` (crates/forge-features/src/workspace.rs), after the first folder's worktree is resolved into `project` and before `open_new_with_effort`, add the extra worktrees as described above (a `Vec<Option<PathBuf>> made`), then in the `Ok(handle)` arm use `crate::environment::extra_paths(&environment.extra, &made)` instead of `environment.extra` when adding directories. Keep the existing duplicate guard.

- [ ] **Step 3: Verify**

Run: `cargo build --workspace`; `cargo test -p forge-features`. Then a window check with the harness (scripts/harness/README.md; stop it at the end; send NO message to Claude from the app): make two temporary git repositories under /private/tmp (`git init`, one commit each), seed an environment into the harness config (see `.superpowers/sdd/2026-10-19-environments/` reports for how the config was seeded: stop the app, edit `/private/tmp/forge-harness-home/forge-config/config.json`, start `target/debug/forge-app` with `FORGE_CLAUDE_HOME`/`FORGE_CONFIG_PATH` pointing into `/private/tmp/forge-harness-home`) with `folders: [repoA, repoB]`, `worktree: true`, `extra_choices: [{"branch": null, "worktree": true}]`; verify with `git worktree list` in both repos what exists after the session would start — a real session needs a message, so do NOT send one; instead check the pure parts by tests and report that the end-to-end check is left for the controller. Remove the temporary repos.

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: sessions start in a worktree of each extra folder that asks for one

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Dialog rows, popover and menu layout

**Files:**
- Modify: `crates/forge-widgets/src/environment_dialog.rs`
- Modify: `crates/forge-widgets/src/environment_chip.rs` (only if the markup needs a class)
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `EnvironmentDraft.choices`, `push_folder`, `remove_folder` (Task 1); `Workspace::folder_branches`.
- Produces: no new public items.

Required behaviour:
1. **One component per folder row** (`FolderRow { draft: Signal<EnvironmentDraft>, index: usize }`): it reads the branches of its own folder with its own `use_resource` (keyed on that folder via a `use_memo`, as the first-folder code does today) and shows, for a git repository, a branch `<select>` (current branch shown while `choices[index].branch` is `None`, `selected` set on the matching option because the web view ignores `value`) and a `Worktree` checkbox bound to `choices[index].worktree`; for a folder that is not a repository, the existing `Not a Git repository` text; the `×` removes the row with `draft.write().remove_folder(index)`. The old first-folder-only code is removed. Add folders only through `push_folder` (the trust flow already adds with `draft.write().folders.push(..)`: change those to `push_folder`).
2. **Popover clipped at the bottom:** the folder popover must always be fully visible: render it so it flips above the button when it would not fit below, or give it `max-height` with its own scroll (`overflow: auto`, `max-height: min(320px, 40vh)`) and `position: absolute` below the button inside a dialog that does not clip it (`overflow: visible` on the folder header, with the dialog's own scroll area unchanged). Check it with an empty folder list AND with many known folders in a short window (use `python3 scripts/harness/resize.py 900 560`).
3. **Long names vs. the gear icon (chip menu):** in `EnvironmentRow` the row label gets `overflow: hidden; text-overflow: ellipsis; white-space: nowrap; min-width: 0`, and the hover icons never sit on the label: reserve their width as right padding of the row's button while hovered (`.env-row:hover .picker-item { padding-right: <icons width + shortcut width> }`) or lay the icons out as flex siblings of the label and the shortcut text instead of absolutely positioned. The shortcut text and the check stay visible. Keep `title` on the row so the full name shows on hover.

- [ ] **Step 1: Implement** the three items above; keep the stage-A behaviours (select the name on open, hook order, Escape handling, Variables page).
- [ ] **Step 2: Build and test:** `cargo build --workspace`; `cargo test -p forge-features -p forge-widgets`.
- [ ] **Step 3: Window checks (harness, send no message):** seed two environments into the harness config (one with a 70-character name and two git repos under /private/tmp as folders) — see Task 2 Step 3 for how to seed. Check and describe: (a) the chip menu with the long name: no overlap with the gear/star/⌥⌘ text, ellipsis; (b) the edit dialog: each git folder row shows branch select + `Worktree` box (first on, extra off by default for a new folder), a non-git folder shows `Not a Git repository`; change the second folder's branch and box, save, reopen: both persisted; (c) the folder popover is fully visible with the dialog at 900x560 and with the list of known folders long. Remove the temporary repos; stop the harness.
- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: branch and worktree for every git folder in the dialog; popover and long names fit

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Korean text and the spec's "As built"

**Files:** `crates/forge-shared/src/ko.rs` (through `scripts/i18n_add.py`), `docs/superpowers/specs/2026-10-19-environments-design.md`, `docs/superpowers/roadmap/backlog.md` (OTH-11 row text).

- [ ] **Step 1:** run `cargo test -p forge-shared`; add Korean entries for any new string (none are planned: the dialog reuses `Worktree`, `Not a Git repository`, `Could not create the worktree: {error}`); if the test is green, say so.
- [ ] **Step 2:** In the spec's "As built" add a section "A2 (2026-10-20)": every git folder has its own branch and worktree choice (option A, decided with the user on 2026-10-20; the desktop app shows an unchecked box on extra git folders but stores only the first folder's flag); `extra_choices` alignment and compatibility; the known limit (archive removes only the first folder's worktree); the two layout fixes; what was checked in the window; what is left: `폴더 없음` (observed in Round 22: the desktop app runs such a session in an empty app-made scratch folder; Forge needs a design for it).
- [ ] **Step 3:** In backlog row OTH-11 replace "Not built: …" so it names: `폴더 없음`, the side-panel card (stage B), model/effort in an environment, seeding from past sessions, encrypted storage, cleanup of extra worktrees on archive.
- [ ] **Step 4:** `cargo test --workspace` once (the `round_trip` plugin test fails on this machine because it reads the real plugin directory; report it, do not change it). Commit: `docs: environments A2 built (per-folder worktree), Korean text` + the Co-Authored-By line.

---

## Self-review (against the approved decision)

- Option A covered: model (Task 1), start (Task 2), dialog (Task 3). Stage-A configs load (Task 1 tests). Alignment (Task 1 tests). Failure of an extra worktree keeps the text (Task 2). Layout fixes (Task 3). `폴더 없음` is explicitly not in this plan (needs a design; recorded in Task 4).
