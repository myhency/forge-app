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
