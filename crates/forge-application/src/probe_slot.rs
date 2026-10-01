use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use forge_domain::{ProjectSummary, SlashCommand};
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::registry::SessionRegistry;

struct Running {
    folder: PathBuf,
    generation: u64,
    // Owns the probe: aborting this task drops it, which ends the probe's sidecar.
    forwarder: JoinHandle<()>,
}

struct State {
    running: Option<Running>,
    /// The generation of the probe started last; each probe gets the next number.
    generation: u64,
}

struct Inner {
    registry: Arc<SessionRegistry>,
    commands: watch::Sender<Vec<SlashCommand>>,
    state: Mutex<State>,
}

impl Inner {
    // `abort` cannot stop a poll that is already running on another thread, so a forwarder can
    // deliver a list after its probe was replaced. Only the running probe's generation gets through,
    // checked under the same lock `set_folder` clears the list under (never held across an await).
    fn publish(&self, generation: u64, list: Vec<SlashCommand>) {
        let state = self.state.lock().unwrap();
        if state.running.as_ref().is_some_and(|running| running.generation == generation) {
            self.commands.send_replace(list);
        }
    }
}

/// Keeps at most one probe, for the folder the new-session screen shows, and republishes its list.
pub struct ProbeSlot {
    inner: Arc<Inner>,
}

impl ProbeSlot {
    pub fn new(registry: Arc<SessionRegistry>) -> Self {
        let (commands, _) = watch::channel(Vec::new());
        Self {
            inner: Arc::new(Inner { registry, commands, state: Mutex::new(State { running: None, generation: 0 }) }),
        }
    }

    /// Wakes on every change of the list, including its clearing.
    pub fn subscribe(&self) -> watch::Receiver<Vec<SlashCommand>> {
        self.inner.commands.subscribe()
    }

    pub fn commands(&self) -> Vec<SlashCommand> {
        self.inner.commands.borrow().clone()
    }

    #[cfg(test)]
    fn generation(&self) -> u64 {
        self.inner.state.lock().unwrap().generation
    }

    #[cfg(test)]
    fn publish(&self, generation: u64, list: Vec<SlashCommand>) {
        self.inner.publish(generation, list);
    }

    /// Makes `project` the folder being probed. The same folder keeps its probe; anything else ends it,
    /// clears the list and starts a probe for `project` (none for `None` or a folder that is gone).
    /// Starting needs a tokio runtime.
    pub fn set_folder(&self, project: Option<&ProjectSummary>) {
        let mut state = self.inner.state.lock().unwrap();
        if let (Some(current), Some(project)) = (state.running.as_ref(), project) {
            if current.folder == project.path {
                return;
            }
        }
        if let Some(old) = state.running.take() {
            old.forwarder.abort();
        }
        self.inner.commands.send_replace(Vec::new());
        let Some(project) = project else {
            return;
        };
        let Some(probe) = self.inner.registry.probe_commands(project) else {
            return;
        };
        state.generation += 1;
        let generation = state.generation;
        let inner = self.inner.clone();
        let forwarder = tokio::spawn(async move {
            let mut updates = probe.subscribe();
            while updates.changed().await.is_ok() {
                let list = updates.borrow_and_update().clone();
                inner.publish(generation, list);
            }
        });
        state.running = Some(Running { folder: project.path.clone(), generation, forwarder });
    }

    /// The latest list; the probe is ended and the list cleared.
    pub fn end(&self) -> Vec<SlashCommand> {
        let latest = self.commands();
        self.set_folder(None);
        latest
    }
}

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

    #[tokio::test]
    async fn a_list_from_an_ended_probe_is_not_published_even_if_it_arrives_after_the_clear() {
        let (a, _control_a) = fake_sidecar();
        let (b, _control_b) = fake_sidecar();
        let (registry, _spawns) = registry_with(vec![Ok(a), Ok(b)]);
        let slot = ProbeSlot::new(registry);

        slot.set_folder(Some(&project("/work/a", true)));
        let old = slot.generation();
        slot.set_folder(Some(&project("/work/b", true)));
        // The old forwarder was mid-poll when it was aborted and now delivers its list.
        slot.publish(old, vec![command("late-from-a")]);

        assert!(slot.commands().is_empty());
        slot.publish(slot.generation(), vec![command("from-b")]);
        assert_eq!(slot.commands(), vec![command("from-b")]);
        slot.end();
        slot.publish(slot.generation(), vec![command("after-end")]);
        assert!(slot.commands().is_empty(), "nothing is published once the probe has ended");
    }
}
