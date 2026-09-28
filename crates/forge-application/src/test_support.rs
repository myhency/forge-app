//! Test doubles for the session tasks. `MockSidecarPort` cannot script "alive but quiet" (a
//! `next_event` that stays pending), so the sidecar here is a small fake driven through a control.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::PermissionDecision;
use forge_ports::{
    MockSidecarFactoryPort, SidecarError, SidecarEvent, SidecarFactoryPort, SidecarPort,
    StartSessionParams,
};
use tokio::sync::{mpsc, watch};

use crate::live_session::LiveSessionView;

/// Every command the fake sidecar received, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Call {
    Start(StartSessionParams),
    Send(String),
    Permission(String, PermissionDecision),
    Interrupt,
    Shutdown,
}

#[derive(Clone)]
pub(crate) struct FakeControl {
    events: Arc<Mutex<Option<mpsc::UnboundedSender<SidecarEvent>>>>,
    calls: Arc<Mutex<Vec<Call>>>,
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
        fail_start: Arc::new(AtomicBool::new(false)),
        fail_send: Arc::new(AtomicBool::new(false)),
    };
    let sidecar = FakeSidecar {
        events: tokio::sync::Mutex::new(events_rx),
        control: control.clone(),
    };
    (Arc::new(sidecar), control)
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

    async fn send_message(&self, text: String) -> Result<(), SidecarError> {
        self.control.record(Call::Send(text));
        if self.control.fail_send.load(Ordering::SeqCst) {
            return Err(SidecarError::ProcessExited);
        }
        Ok(())
    }

    async fn respond_to_permission(
        &self,
        permission_request_id: String,
        decision: PermissionDecision,
    ) -> Result<(), SidecarError> {
        self.control.record(Call::Permission(permission_request_id, decision));
        Ok(())
    }

    async fn interrupt(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Interrupt);
        Ok(())
    }

    async fn shutdown(&self) -> Result<(), SidecarError> {
        self.control.record(Call::Shutdown);
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
