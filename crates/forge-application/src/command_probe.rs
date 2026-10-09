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
                effort: None,
                extensions: active,
                additional_directories: Vec::new(),
                allowed_tools: Vec::new(),
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
                effort: None,
                extensions: ActiveExtensions::default(),
                additional_directories: Vec::new(),
                allowed_tools: Vec::new(),
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
