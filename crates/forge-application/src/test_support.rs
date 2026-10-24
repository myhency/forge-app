//! Test doubles for the session tasks. `MockSidecarPort` cannot script "alive but quiet" (a
//! `next_event` that stays pending), so the sidecar here is a small fake driven through a control.
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::{AppConfig, PermissionDecision, PermissionMode, SkillDefinition};
use forge_ports::{
    MockConfigRepositoryPort, MockMcpRegistryPort, MockPluginRepositoryPort, MockSkillRepositoryPort,
    SkillRepositoryError,
    MockSidecarFactoryPort, SidecarError, SidecarEvent, SidecarFactoryPort, SidecarPort,
    StartSessionParams,
};
use tokio::sync::{mpsc, watch};

use crate::live_session::LiveSessionView;
use crate::use_cases::ExtensionsUseCase;

/// Every command the fake sidecar received, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    Start(StartSessionParams),
    Send(String),
    /// A send that carried files or images.
    SendAttached(String, Vec<forge_domain::Attachment>),
    Permission(String, PermissionDecision, Option<String>),
    Answer(String, std::collections::BTreeMap<String, String>),
    Interrupt,
    StopTask(String),
    SetMode(PermissionMode),
    SetModel(Option<String>),
    SetEffort(Option<forge_domain::EffortLevel>),
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct FakeControl {
    events: Arc<Mutex<Option<mpsc::UnboundedSender<SidecarEvent>>>>,
    calls: Arc<Mutex<Vec<Call>>>,
    /// `shutdown` and the drop of the sidecar, in the order they happened.
    lifecycle: Arc<Mutex<Vec<&'static str>>>,
    fail_start: Arc<AtomicBool>,
    fail_send: Arc<AtomicBool>,
}

impl FakeControl {
    /// Queues an event for the session to read.
    pub(crate) fn push(&self, event: SidecarEvent) {
        if let Some(events) = self.events.lock().unwrap().as_ref() {
            let _ = events.send(event);
        }
    }

    /// Ends the event stream, like a sidecar process that has died.
    pub(crate) fn end_stream(&self) {
        self.events.lock().unwrap().take();
    }

    pub(crate) fn calls(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    pub(crate) fn lifecycle(&self) -> Vec<&'static str> {
        self.lifecycle.lock().unwrap().clone()
    }

    pub(crate) fn fail_start(&self) {
        self.fail_start.store(true, Ordering::SeqCst);
    }

    pub(crate) fn fail_send(&self) {
        self.fail_send.store(true, Ordering::SeqCst);
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

struct FakeSidecar {
    events: tokio::sync::Mutex<mpsc::UnboundedReceiver<SidecarEvent>>,
    control: FakeControl,
}

pub(crate) fn fake_sidecar() -> (Arc<dyn SidecarPort>, FakeControl) {
    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let control = FakeControl {
        events: Arc::new(Mutex::new(Some(events_tx))),
        calls: Arc::new(Mutex::new(Vec::new())),
        lifecycle: Arc::new(Mutex::new(Vec::new())),
        fail_start: Arc::new(AtomicBool::new(false)),
        fail_send: Arc::new(AtomicBool::new(false)),
    };
    let sidecar = FakeSidecar {
        events: tokio::sync::Mutex::new(events_rx),
        control: control.clone(),
    };
    (Arc::new(sidecar), control)
}

impl Drop for FakeSidecar {
    fn drop(&mut self) {
        self.control.lifecycle.lock().unwrap().push("dropped");
    }
}

#[async_trait]
impl SidecarPort for FakeSidecar {
    async fn start_session(&self, params: StartSessionParams) -> Result<(), SidecarError> {
        self.control.record(Call::Start(params));
        if self.control.fail_start.load(Ordering::SeqCst) {
            return Err(SidecarError::WriteFailed("fake start failure".to_string()));
        }
        Ok(())
    }

    async fn send_message(&self, text: String, attachments: Vec<forge_domain::Attachment>) -> Result<(), SidecarError> {
        self.control.record(if attachments.is_empty() {
            Call::Send(text)
        } else {
            Call::SendAttached(text, attachments)
        });
        if self.control.fail_send.load(Ordering::SeqCst) {
            return Err(SidecarError::ProcessExited);
        }
        Ok(())
    }

    async fn respond_to_permission(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
        reason: Option<String>,
    ) -> Result<(), SidecarError> {
        self.control.record(Call::Permission(permission_request_id, decision, reason));
        Ok(())
    }

    async fn answer_question(
        &self,
        permission_request_id: String,
        answers: std::collections::BTreeMap<String, String>,
    ) -> Result<(), SidecarError> {
        self.control.record(Call::Answer(permission_request_id, answers));
        Ok(())
    }

    async fn stop_task(&self, task_id: String) -> Result<(), SidecarError> {
        self.control.record(Call::StopTask(task_id));
        Ok(())
    }

    async fn interrupt(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Interrupt);
        Ok(())
    }

    async fn set_permission_mode(&self, mode: PermissionMode) -> Result<(), SidecarError> {
        self.control.record(Call::SetMode(mode));
        Ok(())
    }

    async fn set_model(&self, model: Option<String>) -> Result<(), SidecarError> {
        self.control.record(Call::SetModel(model));
        Ok(())
    }

    async fn set_effort(&self, effort: Option<forge_domain::EffortLevel>) -> Result<(), SidecarError> {
        self.control.record(Call::SetEffort(effort));
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Shutdown);
        self.control.lifecycle.lock().unwrap().push("shutdown");
        Ok(())
    }

    async fn next_event(&self) -> Option<SidecarEvent> {
        self.events.lock().await.recv().await
    }
}

/// A factory that hands out the scripted sidecars in order, and a counter of how many times it was
/// asked. Asking for more sidecars than were scripted panics inside the session task.
pub(crate) fn factory_returning(
    sidecars: Vec<Result<Arc<dyn SidecarPort>, SidecarError>>,
) -> (Arc<dyn SidecarFactoryPort>, Arc<AtomicUsize>) {
    let queue = Mutex::new(VecDeque::from(sidecars));
    let spawns = Arc::new(AtomicUsize::new(0));
    let counter = spawns.clone();
    let mut factory = MockSidecarFactoryPort::new();
    factory.expect_spawn().returning(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        queue
            .lock()
            .unwrap()
            .pop_front()
            .expect("the test scripted fewer sidecars than were spawned")
    });
    (Arc::new(factory), spawns)
}

const WAIT: Duration = Duration::from_secs(2);

/// Waits until the published view satisfies `condition` and returns that view.
pub(crate) async fn wait_for(
    view: &mut watch::Receiver<LiveSessionView>,
    what: &str,
    condition: impl Fn(&LiveSessionView) -> bool,
) -> LiveSessionView {
    let waiting = async {
        loop {
            {
                let current = view.borrow_and_update();
                if condition(&current) {
                    return current.clone();
                }
            }
            if view.changed().await.is_err() {
                panic!("the session ended while waiting for {what}");
            }
        }
    };
    tokio::time::timeout(WAIT, waiting)
        .await
        .unwrap_or_else(|_| panic!("timed out waiting for {what}"))
}

/// Waits until the session's task has ended (its view channel closes).
pub(crate) async fn wait_closed(view: &mut watch::Receiver<LiveSessionView>) {
    let waiting = async { while view.changed().await.is_ok() {} };
    tokio::time::timeout(WAIT, waiting)
        .await
        .expect("the session should have closed");
}

/// An `ExtensionsUseCase` over mocks: `config` is what the config file holds, `skills` what the
/// skill reader finds. The plugin and MCP readers have no expectations, so a test that needs them
/// gets a panic rather than a silent empty answer.
pub(crate) fn extensions_with(config: AppConfig, skills: Vec<SkillDefinition>) -> Arc<ExtensionsUseCase> {
    let mut config_port = MockConfigRepositoryPort::new();
    config_port.expect_load().returning(move || Ok(config.clone()));
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port.expect_list_skills().returning(move |_| Ok(skills.clone()));
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}

/// The first read of the config gives `first`, every later read `later`: a config edited while a
/// session is open.
pub(crate) fn extensions_changing_config(first: AppConfig, later: AppConfig) -> Arc<ExtensionsUseCase> {
    let reads = Arc::new(AtomicUsize::new(0));
    let mut config_port = MockConfigRepositoryPort::new();
    config_port.expect_load().returning(move || {
        Ok(if reads.fetch_add(1, Ordering::SeqCst) == 0 { first.clone() } else { later.clone() })
    });
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(MockSkillRepositoryPort::new()),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}

/// A skill is enabled and found; the counter says how many times the config file was read.
pub(crate) fn counting_extensions() -> (Arc<ExtensionsUseCase>, Arc<AtomicUsize>) {
    let loads = Arc::new(AtomicUsize::new(0));
    let counter = loads.clone();
    let mut config_port = MockConfigRepositoryPort::new();
    config_port.expect_load().returning(move || {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() })
    });
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port.expect_list_skills().returning(|_| {
        Ok(vec![SkillDefinition { name: "pdf".to_string(), path: PathBuf::from("/skills/pdf"), description: None }])
    });
    let use_case = Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ));
    (use_case, loads)
}

/// A skill is enabled; the folders the skill reader was asked about are recorded.
pub(crate) fn path_recording_extensions() -> (Arc<ExtensionsUseCase>, Arc<Mutex<Vec<PathBuf>>>) {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let record = asked.clone();
    let mut config_port = MockConfigRepositoryPort::new();
    config_port
        .expect_load()
        .returning(|| Ok(AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() }));
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port.expect_list_skills().returning(move |project| {
        record.lock().unwrap().push(project.to_path_buf());
        Ok(Vec::new())
    });
    let use_case = Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ));
    (use_case, asked)
}

/// Nothing enabled, so no reader is ever called.
pub(crate) fn no_extensions() -> Arc<ExtensionsUseCase> {
    extensions_with(AppConfig::default(), Vec::new())
}

/// A skill is enabled and the skill reader fails.
pub(crate) fn failing_extensions() -> Arc<ExtensionsUseCase> {
    let mut config_port = MockConfigRepositoryPort::new();
    config_port
        .expect_load()
        .returning(|| Ok(AppConfig { enabled_skills: vec!["pdf".to_string()], ..AppConfig::default() }));
    let mut skill_port = MockSkillRepositoryPort::new();
    skill_port
        .expect_list_skills()
        .returning(|_| Err(SkillRepositoryError::ListFailed("skill reader is down".to_string())));
    Arc::new(ExtensionsUseCase::new(
        Arc::new(config_port),
        Arc::new(skill_port),
        Arc::new(MockPluginRepositoryPort::new()),
        Arc::new(MockMcpRegistryPort::new()),
        PathBuf::from("/home/me"),
    ))
}
