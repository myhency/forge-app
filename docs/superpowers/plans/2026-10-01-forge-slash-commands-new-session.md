# Forge Slash Commands on the New-Session Screen Implementation Plan (Plan 11, part 2)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On the new-session screen, once a folder is chosen, typing `/` in the first-message box lists that folder's slash commands, before any message is sent.

**Architecture:** A *probe* (a sidecar started only to learn the command list) runs for the chosen folder; a `ProbeSlot` keeps one probe at a time and republishes its list. The workspace shows that list in the new-session text box and, at the first send, hands it to the new session as its initial `commands` and ends the probe. The popup and its keys move into one shared component used by both text boxes.

**Tech Stack:** Rust workspace (tokio, Dioxus 0.7.10). No sidecar change: the sidecar already sends `commands_available` right after `start_session` (verified 2026-10-01).

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §22 (builds on §21).

## Global Constraints

- Commit messages end with the line `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`; subject style `feat: ...`, `fix: ...`, `docs: ...`, lower case.
- The probe starts with the folder, the extensions resolved for it (`ExtensionsUseCase::resolve_active`), permission mode `PermissionMode::Default`, `model: None`, `resume_session_id: None`, and sends no message (spec §22.1).
- One probe at a time; the same folder chosen again keeps it; another folder, leaving the screen, the first send and app close end it (spec §22.1).
- A probe that cannot resolve, spawn or start, or whose sidecar errors or exits, is silent: no banner, no state change, the list stays as it was (spec §22.1).
- No probe for a folder that does not exist (spec §22.3).
- `forge-entities` stays free of Dioxus; the popup rules stay pure there (spec §21.4).
- Baseline before this plan: 472 Rust workspace tests, 68 sidecar tests. Run commands from the worktree root `/Users/james/orca/workspaces/forge/plan-11`.

## Review Focus

1. A probe sidecar that outlives its use (leaving the screen, choosing another folder, the first send, app close): ended in every case (Task 2 tests; live check counts `node` processes).
2. A fast folder change: the old folder's list must not show under the new folder (Task 2 test).
3. A folder that does not exist, and a probe that fails to spawn or whose sidecar errors: no popup, no banner, no panic (Tasks 1 and 2 tests).
4. Start with the popup open or with a `/command` as the first message: the message is still sent and the session's own box shows the carried-over list (Task 5 live check).
5. Text put back after a failed send must not reopen the popup; Shift+Enter must add a line with the popup open; a shrinking list must not make Enter do nothing (Task 3 tests).

---

### Task 1: The probe and the seeded `open_new`

**Files:**
- Create: `crates/forge-application/src/command_probe.rs`
- Modify: `crates/forge-application/src/lib.rs`
- Modify: `crates/forge-application/src/registry.rs` (`probe_commands`, `open_new`, `insert`, tests)
- Modify: `crates/forge-application/src/live_session.rs` (`LiveSessionInit.commands`, `spawn_live_session`, test `new_init`)
- Modify: `crates/forge-features/src/workspace.rs` (the one production `open_new` call: pass `Vec::new()` for now)

**Interfaces:**
- Produces:
  - `forge_application::CommandProbe` with `commands(&self) -> Vec<SlashCommand>` and `subscribe(&self) -> watch::Receiver<Vec<SlashCommand>>`; dropping it ends its sidecar.
  - `SessionRegistry::probe_commands(&self, project: &ProjectSummary) -> Option<CommandProbe>` (`None` when `!project.exists`; must be called inside a tokio runtime).
  - `SessionRegistry::open_new(&self, project, permission_mode, model, commands: Vec<SlashCommand>)`; the session's first view has `commands` set.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-application/src/command_probe.rs` with only the test module (the code comes in Step 3):

```rust
#[cfg(test)]
pub(crate) mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::AtomicUsize;
    use std::sync::Arc;
    use std::time::Duration;

    use forge_domain::{ActiveExtensions, PermissionMode, ProjectSummary, SlashCommand};
    use forge_ports::{MockSessionRepositoryPort, SidecarError, SidecarEvent, StartSessionParams};
    use tokio::sync::watch;

    use crate::registry::SessionRegistry;
    use crate::test_support::{factory_returning, fake_sidecar, no_extensions, Call};

    pub(crate) fn project(path: &str, exists: bool) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: "2026-10-01T00:00:00Z".to_string(), session_count: 1, exists }
    }

    pub(crate) fn command(name: &str) -> SlashCommand {
        SlashCommand { name: name.to_string(), description: None, argument_hint: None }
    }

    pub(crate) fn registry_with(
        sidecars: Vec<Result<Arc<dyn forge_ports::SidecarPort>, SidecarError>>,
    ) -> (Arc<SessionRegistry>, Arc<AtomicUsize>) {
        let (factory, spawns) = factory_returning(sidecars);
        let registry = SessionRegistry::new(factory, Arc::new(MockSessionRepositoryPort::new()), no_extensions());
        (Arc::new(registry), spawns)
    }

    pub(crate) async fn until(rx: &mut watch::Receiver<Vec<SlashCommand>>, what: &str, ok: impl Fn(&[SlashCommand]) -> bool) {
        let waiting = async {
            loop {
                if ok(&rx.borrow_and_update()) {
                    return;
                }
                if rx.changed().await.is_err() {
                    panic!("the probe ended while waiting for {what}");
                }
            }
        };
        tokio::time::timeout(Duration::from_secs(2), waiting).await.unwrap_or_else(|_| panic!("timed out waiting for {what}"));
    }

    pub(crate) async fn eventually(what: &str, condition: impl Fn() -> bool) {
        for _ in 0..200 {
            if condition() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("timed out waiting for {what}");
    }

    #[tokio::test]
    async fn a_probe_starts_a_default_sidecar_for_the_folder_and_publishes_its_commands() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(sidecar)]);

        let probe = registry.probe_commands(&project("/work/a", true)).unwrap();
        let mut list = probe.subscribe();
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("compact")] });
        until(&mut list, "the commands", |c| !c.is_empty()).await;

        assert_eq!(probe.commands(), vec![command("compact")]);
        assert_eq!(
            control.calls(),
            vec![Call::Start(StartSessionParams {
                cwd: "/work/a".to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                extensions: ActiveExtensions::default(),
            })]
        );
    }

    #[tokio::test]
    async fn a_later_list_replaces_the_earlier_one_and_other_events_are_ignored() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(sidecar)]);

        let probe = registry.probe_commands(&project("/work/a", true)).unwrap();
        let mut list = probe.subscribe();
        control.push(SidecarEvent::AssistantDelta { text: "x".to_string() });
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("a"), command("b")] });
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("c")] });
        until(&mut list, "the second list", |c| c == [command("c")]).await;

        assert_eq!(probe.commands(), vec![command("c")]);
    }

    #[tokio::test]
    async fn a_folder_that_does_not_exist_has_no_probe() {
        let (registry, spawns) = registry_with(vec![]);

        assert!(registry.probe_commands(&project("/gone", false)).is_none());
        assert_eq!(spawns.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn dropping_the_probe_shuts_its_sidecar_down() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(sidecar)]);

        let probe = registry.probe_commands(&project("/work/a", true)).unwrap();
        let mut list = probe.subscribe();
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("a")] });
        until(&mut list, "the commands", |c| !c.is_empty()).await;
        drop(probe);

        eventually("the shutdown", || control.lifecycle().contains(&"shutdown")).await;
    }

    #[tokio::test]
    async fn a_failed_spawn_leaves_the_list_empty_without_a_panic() {
        let (registry, spawns) = registry_with(vec![Err(SidecarError::SpawnFailed("no node".to_string()))]);

        let probe = registry.probe_commands(&project("/work/a", true)).unwrap();
        eventually("the spawn attempt", || spawns.load(std::sync::atomic::Ordering::SeqCst) == 1).await;
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert!(probe.commands().is_empty());
    }

    #[tokio::test]
    async fn an_error_event_ends_the_probe_quietly_and_keeps_the_list() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(sidecar)]);

        let probe = registry.probe_commands(&project("/work/a", true)).unwrap();
        let mut list = probe.subscribe();
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("a")] });
        until(&mut list, "the commands", |c| !c.is_empty()).await;
        control.push(SidecarEvent::Error { message: "boom".to_string() });

        eventually("the shutdown", || control.lifecycle().contains(&"shutdown")).await;
        assert_eq!(probe.commands(), vec![command("a")]);
    }
}
```

In `crates/forge-application/src/lib.rs` add `mod command_probe;` (alphabetical, first) so the tests compile once Step 3 adds the code.

In `crates/forge-application/src/registry.rs` tests, add:

```rust
    #[tokio::test]
    async fn open_new_starts_the_session_with_the_given_commands() {
        let (registry, _spawns) = registry(vec![], repository_returning("unused", 0));
        let commands = vec![forge_domain::SlashCommand { name: "compact".to_string(), description: None, argument_hint: None }];

        let handle = registry
            .open_new(&project("/work/a", true), PermissionMode::AcceptEdits, None, commands.clone())
            .unwrap();

        assert_eq!(handle.snapshot().commands, commands);
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-application command_probe 2>&1 | grep -E "^error" | head -5`
Expected: compile errors: `probe_commands` and the fourth `open_new` argument do not exist.

- [ ] **Step 3: Implement**

Put above the test module in `command_probe.rs`:

```rust
use std::path::PathBuf;
use std::sync::Arc;

use forge_domain::{PermissionMode, SlashCommand};
use forge_ports::{SidecarEvent, SidecarFactoryPort, StartSessionParams};
use tokio::sync::{oneshot, watch};

use crate::use_cases::ExtensionsUseCase;

/// A sidecar started only to learn a folder's slash commands. Dropping the probe ends it.
pub struct CommandProbe {
    commands: watch::Receiver<Vec<SlashCommand>>,
    // Dropping this sender is the signal for the probe's task to shut its sidecar down.
    _end: oneshot::Sender<()>,
}

impl CommandProbe {
    pub fn commands(&self) -> Vec<SlashCommand> {
        self.commands.borrow().clone()
    }

    /// Wakes on every new list; reports `Err` once the probe's task has ended.
    pub fn subscribe(&self) -> watch::Receiver<Vec<SlashCommand>> {
        self.commands.clone()
    }
}

/// Starts the probe's task. Must be called from inside a tokio runtime. Every failure ends the task
/// without a word: nothing was asked of the probe, so there is nothing to report.
pub(crate) fn spawn_probe(
    factory: Arc<dyn SidecarFactoryPort>,
    extensions: Arc<ExtensionsUseCase>,
    project_path: PathBuf,
) -> CommandProbe {
    let (list_tx, list_rx) = watch::channel(Vec::new());
    let (end_tx, mut end_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let Ok(active) = extensions.resolve_active(&project_path).await else {
            return;
        };
        let Ok(sidecar) = factory.spawn().await else {
            return;
        };
        let started = sidecar
            .start_session(StartSessionParams {
                cwd: project_path.to_string_lossy().to_string(),
                resume_session_id: None,
                permission_mode: PermissionMode::Default,
                model: None,
                extensions: active,
            })
            .await;
        if started.is_ok() {
            loop {
                tokio::select! {
                    // Resolves when the probe is dropped; also when the end signal was already sent.
                    _ = &mut end_rx => break,
                    event = sidecar.next_event() => match event {
                        Some(SidecarEvent::CommandsAvailable { commands }) => {
                            list_tx.send_replace(commands);
                        }
                        Some(SidecarEvent::Error { .. }) | Some(SidecarEvent::SessionEnded) | None => break,
                        Some(_) => {}
                    },
                }
            }
        }
        let _ = sidecar.shutdown().await;
    });
    CommandProbe { commands: list_rx, _end: end_tx }
}
```

`lib.rs`: add `pub use command_probe::CommandProbe;`.

`registry.rs`: import `crate::command_probe::{spawn_probe, CommandProbe}` and `forge_domain::SlashCommand`; add next to `open_new`:

```rust
    /// Starts a probe for `project`'s slash commands; `None` when the folder is gone. Must be called
    /// from inside a tokio runtime.
    pub fn probe_commands(&self, project: &ProjectSummary) -> Option<CommandProbe> {
        if !project.exists {
            return None;
        }
        Some(spawn_probe(self.factory.clone(), self.extensions.clone(), project.path.clone()))
    }
```

Change `open_new` to take `commands: Vec<SlashCommand>` as its last parameter and pass it to `insert`; `insert` gets a `commands: Vec<SlashCommand>` parameter and puts it into `LiveSessionInit { ..., commands }`; the one other caller of `insert` (`open_existing`) passes `Vec::new()`.

`live_session.rs`: `LiveSessionInit` gets `pub commands: Vec<SlashCommand>,`; `spawn_live_session` sets `commands: init.commands,` in the view (replacing `commands: Vec::new()`); the test helper `new_init` gets `commands: Vec::new(),`.

`workspace.rs`: the existing `services.registry.open_new(&project, mode, model)` call becomes `open_new(&project, mode, model, Vec::new())` (Task 5 replaces the empty list).

Update every other existing `open_new(` call in the tests (the compiler lists them): add `Vec::new()` as the last argument.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|failed;|panicked|^error" | grep -v " 0 failed"; cargo test -p forge-application command_probe 2>&1 | grep -E "^test "`
Expected: no failure lines; the six `command_probe` tests and `open_new_starts_the_session_with_the_given_commands` show `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: a probe sidecar reports a folder's slash commands before any session exists" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: One probe at a time

**Files:**
- Create: `crates/forge-application/src/probe_slot.rs`
- Modify: `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `SessionRegistry::probe_commands`, `CommandProbe::subscribe` (Task 1).
- Produces: `forge_application::ProbeSlot`:
  - `ProbeSlot::new(registry: Arc<SessionRegistry>) -> ProbeSlot`
  - `set_folder(&self, project: Option<&ProjectSummary>)`: keeps the probe when the folder is the same, otherwise ends it, clears the list, and starts one for `project` (a missing folder or `None` starts none). Needs a tokio runtime when `project` is `Some`.
  - `subscribe(&self) -> watch::Receiver<Vec<SlashCommand>>`, `commands(&self) -> Vec<SlashCommand>`
  - `end(&self) -> Vec<SlashCommand>`: returns the latest list and ends the probe.

- [ ] **Step 1: Write the failing tests**

`probe_slot.rs` starts with the test module. It reuses the helpers `project`, `command`, `registry_with`, `until` and `eventually` from `command_probe::tests` (Task 1).

```rust
#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use forge_ports::SidecarEvent;

    use super::*;
    use crate::command_probe::tests::{command, eventually, project, registry_with, until};
    use crate::test_support::fake_sidecar;

    #[tokio::test]
    async fn choosing_a_folder_starts_a_probe_and_its_list_reaches_subscribers() {
        let (sidecar, control) = fake_sidecar();
        let (registry, spawns) = registry_with(vec![Ok(sidecar)]);
        let slot = ProbeSlot::new(registry);
        let mut list = slot.subscribe();

        slot.set_folder(Some(&project("/work/a", true)));
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("compact")] });
        until(&mut list, "the commands", |c| !c.is_empty()).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
        assert_eq!(slot.commands(), vec![command("compact")]);
    }

    #[tokio::test]
    async fn choosing_the_same_folder_again_keeps_the_probe() {
        let (sidecar, _control) = fake_sidecar();
        let (registry, spawns) = registry_with(vec![Ok(sidecar)]);
        let slot = ProbeSlot::new(registry);

        slot.set_folder(Some(&project("/work/a", true)));
        slot.set_folder(Some(&project("/work/a", true)));
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn choosing_another_folder_ends_the_old_probe_and_the_old_list_does_not_show_under_the_new_folder() {
        let (first, first_control) = fake_sidecar();
        let (second, second_control) = fake_sidecar();
        let (registry, spawns) = registry_with(vec![Ok(first), Ok(second)]);
        let slot = ProbeSlot::new(registry);
        let mut list = slot.subscribe();

        slot.set_folder(Some(&project("/work/a", true)));
        first_control.push(SidecarEvent::CommandsAvailable { commands: vec![command("from-a")] });
        until(&mut list, "the first list", |c| c == [command("from-a")]).await;

        slot.set_folder(Some(&project("/work/b", true)));
        assert!(slot.commands().is_empty(), "the list is cleared at once");
        // A late event from the old folder's sidecar must not reach the new folder.
        first_control.push(SidecarEvent::CommandsAvailable { commands: vec![command("late-from-a")] });
        second_control.push(SidecarEvent::CommandsAvailable { commands: vec![command("from-b")] });
        until(&mut list, "the second list", |c| c == [command("from-b")]).await;

        eventually("the old probe's shutdown", || first_control.lifecycle().contains(&"shutdown")).await;
        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        assert_eq!(slot.commands(), vec![command("from-b")]);
    }

    #[tokio::test]
    async fn ending_returns_the_latest_list_and_stops_the_probe() {
        let (sidecar, control) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(sidecar)]);
        let slot = ProbeSlot::new(registry);
        let mut list = slot.subscribe();
        slot.set_folder(Some(&project("/work/a", true)));
        control.push(SidecarEvent::CommandsAvailable { commands: vec![command("compact")] });
        until(&mut list, "the commands", |c| !c.is_empty()).await;

        let latest = slot.end();

        assert_eq!(latest, vec![command("compact")]);
        assert!(slot.commands().is_empty());
        eventually("the shutdown", || control.lifecycle().contains(&"shutdown")).await;
    }

    #[tokio::test]
    async fn no_folder_and_a_missing_folder_start_nothing() {
        let (registry, spawns) = registry_with(vec![]);
        let slot = ProbeSlot::new(registry);

        slot.set_folder(None);
        slot.set_folder(Some(&project("/gone", false)));
        tokio::time::sleep(Duration::from_millis(30)).await;

        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert!(slot.commands().is_empty());
    }
}
```

Add `mod probe_slot;` to `lib.rs` (after `mod live_session_actor;`).

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-application probe_slot 2>&1 | grep -E "^error" | head -4`
Expected: compile error, `ProbeSlot` not found.

- [ ] **Step 3: Implement**

Above the tests in `probe_slot.rs`:

```rust
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use forge_domain::{ProjectSummary, SlashCommand};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::registry::SessionRegistry;

struct Running {
    folder: PathBuf,
    // Owns the probe: aborting this task drops it, which ends the probe's sidecar.
    forwarder: JoinHandle<()>,
}

/// Keeps at most one probe, for the folder the new-session screen shows, and republishes its list.
pub struct ProbeSlot {
    registry: Arc<SessionRegistry>,
    commands: watch::Sender<Vec<SlashCommand>>,
    running: Mutex<Option<Running>>,
}

impl ProbeSlot {
    pub fn new(registry: Arc<SessionRegistry>) -> Self {
        let (commands, _) = watch::channel(Vec::new());
        Self { registry, commands, running: Mutex::new(None) }
    }

    /// Wakes on every change of the list, including its clearing.
    pub fn subscribe(&self) -> watch::Receiver<Vec<SlashCommand>> {
        self.commands.subscribe()
    }

    pub fn commands(&self) -> Vec<SlashCommand> {
        self.commands.borrow().clone()
    }

    /// Makes `project` the folder being probed. The same folder keeps its probe; anything else ends it,
    /// clears the list and starts a probe for `project` (none for `None` or a folder that is gone).
    /// Starting needs a tokio runtime.
    pub fn set_folder(&self, project: Option<&ProjectSummary>) {
        let mut running = self.running.lock().unwrap();
        if let (Some(current), Some(project)) = (running.as_ref(), project) {
            if current.folder == project.path {
                return;
            }
        }
        if let Some(old) = running.take() {
            old.forwarder.abort();
        }
        self.commands.send_replace(Vec::new());
        let Some(project) = project else {
            return;
        };
        let Some(probe) = self.registry.probe_commands(project) else {
            return;
        };
        let sender = self.commands.clone();
        let forwarder = tokio::spawn(async move {
            let mut updates = probe.subscribe();
            while updates.changed().await.is_ok() {
                sender.send_replace(updates.borrow_and_update().clone());
            }
        });
        *running = Some(Running { folder: project.path.clone(), forwarder });
    }

    /// The latest list; the probe is ended and the list cleared.
    pub fn end(&self) -> Vec<SlashCommand> {
        let latest = self.commands();
        self.set_folder(None);
        latest
    }
}
```

`lib.rs`: `pub use probe_slot::ProbeSlot;`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|failed;|panicked|^error" | grep -v " 0 failed"; cargo test -p forge-application probe_slot 2>&1 | grep -E "^test "`
Expected: no failure lines; five `probe_slot` tests `ok`. If `choosing_another_folder...` is flaky because the old forwarder published once more after the clear, report it: do not add sleeps to hide it.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: one command probe at a time, for the folder the new-session screen shows" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Popup rules: Shift, selection range, typed-only

**Files:**
- Modify: `crates/forge-entities/src/command_menu.rs`
- Modify: `crates/forge-entities/src/lib.rs`
- Modify: `crates/forge-features/src/send_message.rs` (the one call of `menu_action`, so the workspace keeps compiling)

**Interfaces:**
- Produces:
  - `menu_action(key: MenuKey, composing: bool, shift: bool) -> Option<MenuAction>`: Enter and Tab with Shift held are `None`.
  - `menu_rows<'a>(commands: &'a [SlashCommand], value: &str, typed: &str, dismissed: bool) -> Vec<&'a SlashCommand>`: empty when `dismissed`, or when `value != typed` (text that did not come from the person's typing), or when `command_query(value)` is `None`; otherwise `filter_commands`.

- [ ] **Step 1: Write the failing tests**

In `command_menu.rs`, change the existing `menu_action` test to the three-argument form and add one test and the rows tests:

```rust
    #[test]
    fn keys_map_to_menu_actions_and_enter_during_an_ime_composition_does_nothing() {
        assert_eq!(menu_action(MenuKey::Up, false, false), Some(MenuAction::Move(-1)));
        assert_eq!(menu_action(MenuKey::Down, false, false), Some(MenuAction::Move(1)));
        assert_eq!(menu_action(MenuKey::Enter, false, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Tab, false, false), Some(MenuAction::Pick));
        assert_eq!(menu_action(MenuKey::Escape, false, false), Some(MenuAction::Close));
        assert_eq!(menu_action(MenuKey::Other, false, false), None);
        assert_eq!(menu_action(MenuKey::Enter, true, false), None);
        assert_eq!(menu_action(MenuKey::Tab, true, false), None);
    }

    #[test]
    fn enter_and_tab_with_shift_held_do_not_pick() {
        assert_eq!(menu_action(MenuKey::Enter, false, true), None, "Shift+Enter adds a line");
        assert_eq!(menu_action(MenuKey::Tab, false, true), None);
        assert_eq!(menu_action(MenuKey::Down, false, true), Some(MenuAction::Move(1)));
    }

    #[test]
    fn the_popup_rows_are_the_matches_for_text_the_person_typed() {
        let commands = vec![cmd("compact"), cmd("review")];
        let names = |rows: Vec<&SlashCommand>| rows.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(menu_rows(&commands, "/c", "/c", false)), vec!["compact"]);
        assert_eq!(names(menu_rows(&commands, "/", "/", false)), vec!["compact", "review"]);
    }

    #[test]
    fn the_popup_is_closed_after_escape_for_text_put_back_and_after_a_space() {
        let commands = vec![cmd("compact")];
        assert!(menu_rows(&commands, "/c", "/c", true).is_empty(), "dismissed with Esc");
        assert!(menu_rows(&commands, "/compact", "", false).is_empty(), "text put back after a failed send was not typed");
        assert!(menu_rows(&commands, "/compact now", "/compact now", false).is_empty());
        assert!(menu_rows(&commands, "hello", "hello", false).is_empty());
    }
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-entities command_menu 2>&1 | grep -E "^error" | head -4`
Expected: compile errors (`menu_action` takes two arguments, `menu_rows` missing).

- [ ] **Step 3: Implement**

`command_menu.rs`: replace `menu_action` and add `menu_rows`:

```rust
/// What an open menu does with a key. Enter or Tab that confirms an IME composition (Korean, Japanese,
/// Chinese) belongs to the composition, and with Shift held Enter adds a line, so neither picks.
pub fn menu_action(key: MenuKey, composing: bool, shift: bool) -> Option<MenuAction> {
    match key {
        MenuKey::Up => Some(MenuAction::Move(-1)),
        MenuKey::Down => Some(MenuAction::Move(1)),
        MenuKey::Enter | MenuKey::Tab if !composing && !shift => Some(MenuAction::Pick),
        MenuKey::Escape => Some(MenuAction::Close),
        _ => None,
    }
}

/// The popup's rows; empty means closed. It is open only for text the person typed (`value == typed`):
/// text that arrives another way, like a failed send put back in the box, must not open it.
pub fn menu_rows<'a>(commands: &'a [SlashCommand], value: &str, typed: &str, dismissed: bool) -> Vec<&'a SlashCommand> {
    if dismissed || value != typed {
        return Vec::new();
    }
    match command_query(value) {
        Some(query) => filter_commands(commands, query),
        None => Vec::new(),
    }
}
```
`lib.rs`: add `menu_rows` to the `command_menu` re-export list.
`send_message.rs`: the call `menu_action(key, event.is_composing())` becomes `menu_action(key, event.is_composing(), event.modifiers().shift())`.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test --workspace 2>&1 | grep -E "FAILED|failed;|panicked|^error" | grep -v " 0 failed"; cargo test -p forge-entities command_menu 2>&1 | grep -E "^test "`
Expected: no failure lines; the menu tests (nine) `ok`.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: the command popup ignores Shift+Enter, text that was not typed, and a stale selection rule" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: One popup component for both text boxes

**Files:**
- Create: `crates/forge-features/src/command_box.rs`
- Modify: `crates/forge-features/src/lib.rs`
- Modify: `crates/forge-features/src/send_message.rs`
- Modify: `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `menu_rows`, `menu_action`, `move_selection`, `insert_text`, `MenuKey`, `MenuAction` (Task 3).
- Produces: `forge_features::CommandTextArea` with props `value: String`, `commands: Vec<SlashCommand>`, `placeholder: String` (default empty), `disabled: bool` (default false), `oninput: EventHandler<String>`, `onsubmit: EventHandler<()>` (Enter without Shift, popup closed or composition not involved).

This is Dioxus view code; its decisions are the pure functions tested in Task 3. The check is that it builds, the workspace stays green, and the live window behaves (Task 5).

- [ ] **Step 1: Write the component**

`crates/forge-features/src/command_box.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::SlashCommand;
use forge_entities::{insert_text, menu_action, menu_rows, move_selection, MenuAction, MenuKey};
use forge_shared::TextArea;

/// A text box that lists slash commands while its text is just `/name`. The parent owns the text:
/// picking a command calls `oninput` with `/name `. Enter (without Shift) calls `onsubmit` unless the
/// popup is open, in which case it picks.
#[component]
pub fn CommandTextArea(
    value: String,
    commands: Vec<SlashCommand>,
    #[props(default)] placeholder: String,
    #[props(default)] disabled: bool,
    oninput: EventHandler<String>,
    onsubmit: EventHandler<()>,
) -> Element {
    let mut selected = use_signal(|| 0usize);
    let mut dismissed = use_signal(|| false);
    // What the person last typed. Text that arrives another way (a failed send put back, a picked
    // command) does not open the popup.
    let mut typed = use_signal(String::new);

    let matches: Vec<SlashCommand> = menu_rows(&commands, &value, &typed.read(), dismissed() || disabled)
        .into_iter()
        .cloned()
        .collect();
    let count = matches.len();
    // The stored selection can be out of range after the list shrinks; the row shown and the row
    // picked are always this one.
    let current = move_selection(selected(), count, 0);

    let pick = {
        let matches = matches.clone();
        move |index: usize| {
            if let Some(command) = matches.get(index) {
                oninput.call(insert_text(command));
                selected.set(0);
            }
        }
    };

    rsx! {
        div { class: "command-box",
            if count > 0 {
                div { class: "command-menu", role: "listbox",
                    for (index, command) in matches.iter().enumerate() {
                        div {
                            key: "{command.name}",
                            role: "option",
                            class: if index == current { "command-item selected" } else { "command-item" },
                            // mousedown, not click: a click would first take the focus from the text box.
                            onmousedown: {
                                let mut pick = pick.clone();
                                move |event: MouseEvent| {
                                    event.prevent_default();
                                    pick(index);
                                }
                            },
                            span { class: "command-name", "/{command.name}" }
                            if let Some(hint) = command.argument_hint.as_ref() {
                                span { class: "command-hint", "{hint}" }
                            }
                            if let Some(description) = command.description.as_ref() {
                                span { class: "command-desc", "{description}" }
                            }
                        }
                    }
                }
            }
            TextArea {
                value: value.clone(),
                placeholder,
                disabled,
                oninput: move |text: String| {
                    typed.set(text.clone());
                    selected.set(0);
                    dismissed.set(false);
                    oninput.call(text);
                },
                onkeydown: {
                    let mut pick = pick.clone();
                    move |event: KeyboardEvent| {
                        if count > 0 {
                            let key = match event.key() {
                                Key::ArrowUp => MenuKey::Up,
                                Key::ArrowDown => MenuKey::Down,
                                Key::Enter => MenuKey::Enter,
                                Key::Tab => MenuKey::Tab,
                                Key::Escape => MenuKey::Escape,
                                _ => MenuKey::Other,
                            };
                            if let Some(action) = menu_action(key, event.is_composing(), event.modifiers().shift()) {
                                event.prevent_default();
                                match action {
                                    MenuAction::Move(delta) => selected.set(move_selection(current, count, delta)),
                                    MenuAction::Pick => pick(current),
                                    MenuAction::Close => dismissed.set(true),
                                }
                                return;
                            }
                        }
                        // Enter that confirms an IME composition (Korean, Japanese, Chinese) must not send.
                        if event.key() == Key::Enter && !event.modifiers().shift() && !event.is_composing() {
                            event.prevent_default();
                            onsubmit.call(());
                        }
                    }
                },
            }
        }
    }
}
```

`lib.rs`: add `mod command_box;` (alphabetical, after `approve_permission`) and `pub use command_box::CommandTextArea;`.

`theme.css`, next to the `.command-menu` rules, add `.command-box { position: relative; }` (the menu is placed against it: `bottom: calc(100% + 8px)` of the text box).

- [ ] **Step 2: Use it in the message box**

In `send_message.rs`: remove the `selected` and `dismissed` signals, the `matches`, `current` and `pick` blocks, the popup `div` and the `TextArea` with its handlers, and the imports that become unused (`SlashCommand`, the `forge_entities` names, `TextArea`). In their place, inside `div { class: "composer", ... }`:

```rust
            CommandTextArea {
                value: draft(),
                commands: view.map(|view| view.read().commands.clone()).unwrap_or_default(),
                placeholder: "Message Claude…",
                disabled: !can_send,
                oninput: move |text| draft.set(text),
                onsubmit: move |_| submit(),
            }
```
with `use crate::command_box::CommandTextArea;`. The restore effect, `submit`, `stop` and the button row stay as they are.

- [ ] **Step 3: Build and test**

Run: `cargo build --workspace 2>&1 | grep -E "^(error|warning: unused)" -A8 | head -30; cargo test --workspace 2>&1 | grep -E "FAILED|failed;|panicked|^error" | grep -v " 0 failed"`
Expected: builds without error or unused-import warning; no failure lines.

- [ ] **Step 4: Commit**

```bash
git add crates
git commit -m "feat: one slash command popup component for the message boxes" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The new-session screen uses the probe

**Files:**
- Modify: `crates/forge-features/src/workspace.rs`
- Modify: `crates/forge-features/src/new_session.rs`
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (§22.6 As built)

**Interfaces:**
- Consumes: `ProbeSlot` (Task 2), `CommandTextArea` (Task 4), `open_new(..., commands)` (Task 1).
- Produces on `Workspace`: `chosen_new_folder() -> Option<PathBuf>` (the folder the panel would start in), `probe_new_folder(folder: Option<PathBuf>)`, `new_commands() -> Vec<SlashCommand>`. `start_new` seeds the new session with the probe's latest list and ends the probe.

- [ ] **Step 1: Workspace state**

In `workspace.rs`: import `forge_application::ProbeSlot` and `forge_domain::SlashCommand`. Add two fields to `Workspace`:

```rust
    probe: Signal<Arc<ProbeSlot>>,
    new_commands: Signal<Vec<SlashCommand>>,
```
In `use_workspace_provider`, build them before the `Workspace` literal and forward the slot's list into the signal (the same pattern as `attach`):

```rust
    let slot = Arc::new(ProbeSlot::new(services.registry.clone()));
    let mut list = slot.subscribe();
    let mut new_commands = Signal::new_in_scope(Vec::new(), ScopeId::APP);
    dioxus::core::spawn_forever(async move {
        while list.changed().await.is_ok() {
            new_commands.set(list.borrow_and_update().clone());
        }
    });
```
and in the literal `probe: Signal::new(slot), new_commands,` (place the `let slot...` lines before `use_hook(move || Workspace {...})` only if `services` is still usable there: clone `services.registry` before it is moved into `Signal::new(services)`). Methods:

```rust
    /// The folder the new-session panel would start in: the chosen one if it is still offered,
    /// otherwise the first.
    pub fn chosen_new_folder(&self) -> Option<PathBuf> {
        let projects = self.new_session_projects();
        self.new_folder()
            .filter(|path| projects.iter().any(|project| project.path == *path))
            .or_else(|| projects.first().map(|project| project.path.clone()))
    }

    /// Probes `folder`'s slash commands for the new-session text box; `None` ends the probe. The same
    /// folder keeps its probe. Call it inside the Dioxus runtime (a `spawn`), not from a render.
    pub fn probe_new_folder(&self, folder: Option<PathBuf>) {
        let project = folder.and_then(|path| self.new_session_projects().into_iter().find(|p| p.path == path));
        let slot = (*self.probe.peek()).clone();
        slot.set_folder(project.as_ref());
    }

    /// The slash commands of the probed folder; empty until the probe answers.
    pub fn new_commands(&self) -> Vec<SlashCommand> {
        self.new_commands.read().clone()
    }
```
In `start_new`, take the slot before the async block (`let slot = (*self.probe.peek()).clone();`) and right before `open_new`:

```rust
            let commands = slot.end();
            match services.registry.open_new(&project, mode, model, commands) {
```
(replacing the `Vec::new()` of Task 1).

- [ ] **Step 2: The composer**

In `new_session.rs`: import `crate::command_box::CommandTextArea` and drop `TextArea` from the `forge_shared` import. Replace the folder computation `selected` so it uses the shared helper where it already does the same job:

```rust
    let selected = workspace.chosen_new_folder().map(|path| path.to_string_lossy().to_string()).unwrap_or_default();
```
(`options` stays as it is: it is still needed for the `Select`.) Add, after the signals at the top of the component:

```rust
    // The probe follows the chosen folder: it is replaced when the folder changes, kept when it does
    // not, and ended when this panel goes away (Start, another session, the settings).
    use_effect(move || {
        let folder = workspace.chosen_new_folder();
        spawn(async move { workspace.probe_new_folder(folder) });
    });
    use_drop(move || workspace.probe_new_folder(None));
```
Replace the `TextArea { ... }` with:

```rust
            CommandTextArea {
                value: text(),
                commands: workspace.new_commands(),
                placeholder: "What should Claude do?",
                disabled: !has_folder,
                oninput: move |value| text.set(value),
                onsubmit: move |_| submit(),
            }
```

- [ ] **Step 3: Build and test**

Run: `cargo build --workspace 2>&1 | grep -E "^(error|warning: unused)" -A8 | head -30; cargo test --workspace 2>&1 | grep -E "FAILED|failed;|panicked|^error" | grep -v " 0 failed"; cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4} END {print "rust tests:", s}'`
Expected: builds clean; no failure lines; the Rust test count is 472 plus the 15 new tests of Tasks 1 to 3 (487).

- [ ] **Step 4: Check it in the running app**

The sidecar must be built: `cd sidecar && npm run build`. Then `cargo run -p forge-app`, and:
1. New-session screen, a folder chosen, no message sent: type `/` — within about 4 s a list appears. Narrow it by typing. Up/Down/Enter/Tab/Esc, a mouse click, Shift+Enter (adds a line with the popup open), Korean composition + Enter (no pick).
2. Choose another folder in the Folder select: the list clears and refills for that folder; choosing the same one again does nothing.
3. Press Start with a `/command ...` message: the session opens, the message is sent, and `/` in its message box lists commands at once.
4. In a terminal, `pgrep -fl "sidecar/dist/index.js"`: with the new-session screen open there is one process; after Start, one per open session and none for the probe; after switching to an open session and back to the new-session screen, one again; after quitting the app, none.
5. A folder that does not exist and a run with `node` missing from `PATH`: no popup, no banner.
Note what could not be checked.

- [ ] **Step 5: Record "As built" in the spec**

Append `### 22.6 As built` to §22: the check date, the test counts (Rust before 472, now; sidecar 68), what was checked in the live window and what was not, differences from §22, every `Ruling:` from the ledger that changed behaviour.

- [ ] **Step 6: Commit**

```bash
git add crates docs
git commit -m "feat: the new-session screen lists slash commands before the first message; record Plan 11 part 2 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
