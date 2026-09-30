use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{LiveSessionHandle, LiveSessionId, LiveSessionView, SessionRegistry, SettingsUseCase};
use forge_domain::{PermissionDecision, PermissionMode, ProjectSummary, SessionId, SessionSummary};
use forge_ports::{ProjectRepositoryPort, SessionRepositoryPort};

use crate::shortcut::{shortcut_for, Shortcut};
use crate::sidebar::{cycle_target, merge_sidebar, open_order, LiveSummary, SidebarGroup};

/// Everything the workspace needs from the outside, injected once by the composition root.
#[derive(Clone)]
pub struct WorkspaceServices {
    pub registry: Arc<SessionRegistry>,
    pub projects: Arc<dyn ProjectRepositoryPort>,
    pub sessions: Arc<dyn SessionRepositoryPort>,
    pub settings: Arc<SettingsUseCase>,
}

/// What the main pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Active {
    NewSession,
    Live(LiveSessionId),
}

#[derive(Clone)]
struct LiveEntry {
    handle: LiveSessionHandle,
    view: Signal<LiveSessionView>,
}

/// The UI-side session store: the sidebar's past sessions, the open sessions and the active one.
/// It is `Copy` (every field is a signal), so event handlers can capture it freely.
#[derive(Clone, Copy)]
pub struct Workspace {
    services: Signal<WorkspaceServices>,
    past: Signal<Vec<(ProjectSummary, Vec<SessionSummary>)>>,
    live: Signal<Vec<LiveEntry>>,
    active: Signal<Active>,
    notice: Signal<Option<String>>,
    new_folder: Signal<Option<PathBuf>>,
}

/// Creates the workspace, provides it to every descendant and loads the sidebar once. Call it in
/// the page at the top of the tree; descendants call `use_workspace`.
pub fn use_workspace_provider() -> Workspace {
    let services = use_context::<WorkspaceServices>();
    let workspace = use_hook(move || Workspace {
        services: Signal::new(services),
        past: Signal::new(Vec::new()),
        live: Signal::new(Vec::new()),
        active: Signal::new(Active::NewSession),
        notice: Signal::new(None),
        new_folder: Signal::new(None),
    });
    use_context_provider(move || workspace);
    use_hook(move || workspace.reload_past());
    workspace
}

pub fn use_workspace() -> Workspace {
    use_context::<Workspace>()
}

impl Workspace {
    pub fn active(&self) -> Active {
        *self.active.read()
    }

    /// A message for the user (a failed open, a failed listing), shown until dismissed.
    pub fn notice(&self) -> Option<String> {
        self.notice.read().clone()
    }

    pub fn dismiss_notice(&self) {
        let mut notice = self.notice;
        notice.set(None);
    }

    /// The sidebar as drawn: past sessions with the open ones merged in.
    pub fn sidebar(&self) -> Vec<SidebarGroup> {
        let open: Vec<LiveSummary> = self
            .live
            .read()
            .iter()
            .map(|entry| LiveSummary::from_view(&entry.view.read()))
            .collect();
        merge_sidebar(&self.past.read(), &open)
    }

    /// The project folders a new session can start in: known projects that still exist.
    pub fn new_session_projects(&self) -> Vec<ProjectSummary> {
        self.past
            .read()
            .iter()
            .filter(|(project, _)| project.exists)
            .map(|(project, _)| project.clone())
            .collect()
    }

    pub fn live_view(&self, id: LiveSessionId) -> Option<Signal<LiveSessionView>> {
        self.live
            .read()
            .iter()
            .find(|entry| entry.handle.id() == id)
            .map(|entry| entry.view)
    }

    pub fn handle(&self, id: LiveSessionId) -> Option<LiveSessionHandle> {
        self.live
            .read()
            .iter()
            .find(|entry| entry.handle.id() == id)
            .map(|entry| entry.handle.clone())
    }

    pub fn show_new_session(&self) {
        let mut active = self.active;
        active.set(Active::NewSession);
    }

    /// The folder chosen in the new-session panel (`None`: the first one offered).
    pub fn new_folder(&self) -> Option<PathBuf> {
        self.new_folder.read().clone()
    }

    pub fn set_new_folder(&self, folder: Option<PathBuf>) {
        let mut current = self.new_folder;
        current.set(folder);
    }

    /// Shows the new-session panel with `folder` preselected (the `+` beside a project).
    pub fn show_new_session_in(&self, folder: PathBuf) {
        self.set_new_folder(Some(folder));
        self.show_new_session();
    }

    pub fn select(&self, id: LiveSessionId) {
        let mut active = self.active;
        active.set(Active::Live(id));
    }

    pub fn answer_permission(&self, id: LiveSessionId, request_id: String, decision: PermissionDecision) {
        if let Some(handle) = self.handle(id) {
            handle.approve(request_id, decision);
        }
    }

    /// Reloads the sidebar's past sessions from disk.
    pub fn reload_past(&self) {
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        let mut notice = self.notice;
        spawn(async move {
            let (loaded, problem) = load_past(&services).await;
            past.set(loaded);
            if let Some(problem) = problem {
                notice.set(Some(problem));
            }
        });
    }

    /// Opens a past session read-only (or focuses it if it is already open).
    pub fn open_past(&self, project: ProjectSummary, session_id: SessionId) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            match services.registry.open_existing(&project, session_id).await {
                Ok(outcome) => {
                    let id = this.ensure_attached(outcome.handle);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Opens a new session in `project`, on the saved default model, and sends its first message.
    pub fn start_new(&self, project: ProjectSummary, mode: PermissionMode, text: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Pinned to the root scope: `NewSessionComposer` unmounts if the person clicks elsewhere
        // while the saved model is being read, which would cancel a plain `spawn`.
        dioxus::core::spawn_forever(async move {
            let model = match services.settings.saved_model().await {
                Ok(model) => model,
                Err(error) => {
                    this.report(format!("failed to load the Forge config: {error}"));
                    return;
                }
            };
            match services.registry.open_new(&project, mode, model) {
                Ok(handle) => {
                    let id = this.attach(handle.clone());
                    handle.send(text);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    pub fn close(&self, id: LiveSessionId) {
        let services = (*self.services.peek()).clone();
        services.registry.close(id);
        let mut live = self.live;
        live.write().retain(|entry| entry.handle.id() != id);
        if self.active() == Active::Live(id) {
            let next = self.live.peek().last().map(|entry| entry.handle.id());
            match next {
                Some(next) => self.select(next),
                None => self.show_new_session(),
            }
        }
        self.reload_past();
    }

    pub fn close_active(&self) {
        if let Active::Live(id) = self.active() {
            self.close(id);
        }
    }

    /// Focuses the next (or previous) open session in the order the sidebar draws them.
    pub fn cycle(&self, forward: bool) {
        let order = open_order(&self.sidebar());
        let current = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession => None,
        };
        if let Some(target) = cycle_target(&order, current, forward) {
            self.select(target);
        }
    }

    /// Handles the session shortcuts; returns whether the key press was one of them.
    pub fn on_key(&self, event: &KeyboardEvent) -> bool {
        let Some(shortcut) = shortcut_from_event(event) else {
            return false;
        };
        match shortcut {
            // Opening settings belongs to `Settings::on_key`; the page offers it the event next.
            Shortcut::OpenSettings => return false,
            Shortcut::NewSession => self.show_new_session(),
            Shortcut::NextSession => self.cycle(true),
            Shortcut::PreviousSession => self.cycle(false),
            Shortcut::CloseSession => self.close_active(),
        }
        event.prevent_default();
        true
    }

    fn report(&self, message: String) {
        let mut notice = self.notice;
        notice.set(Some(message));
    }

    fn ensure_attached(&self, handle: LiveSessionHandle) -> LiveSessionId {
        let id = handle.id();
        if self.handle(id).is_some() {
            id
        } else {
            self.attach(handle)
        }
    }

    // Adds the session to the store and starts the task that copies its published state into a
    // signal. The session runs under tokio; this task runs on the UI thread, which is why the
    // state crosses through a `watch` channel instead of the session touching a signal itself.
    //
    // Both the signal and this bridging task are pinned to the app's root scope
    // (`dioxus::prelude::spawn` cancels its task when the *calling* component unmounts, and
    // `start_new`'s caller, `NewSessionComposer`, unmounts the instant `select()` below switches
    // the active view — killing the bridging task before it is ever polled once, so no update is
    // ever seen). `open_past`'s caller, `Sidebar`, never unmounts, which is why that path worked
    // without this fix. `dioxus::core::spawn_forever` runs the task in `ScopeId::ROOT` instead, so
    // it survives whichever component happened to call `attach`.
    fn attach(&self, handle: LiveSessionHandle) -> LiveSessionId {
        let id = handle.id();
        let mut view = Signal::new_in_scope(handle.snapshot(), ScopeId::APP);
        let mut live = self.live;
        live.write().push(LiveEntry { handle: handle.clone(), view });
        let mut updates = handle.subscribe();
        dioxus::core::spawn_forever(async move {
            while updates.changed().await.is_ok() {
                let latest = updates.borrow_and_update().clone();
                view.set(latest);
            }
        });
        id
    }
}

/// The shortcut a key press is, if any (platform-aware: Cmd on macOS, Ctrl elsewhere).
pub(crate) fn shortcut_from_event(event: &KeyboardEvent) -> Option<Shortcut> {
    let key = match event.key() {
        Key::Tab => "Tab".to_string(),
        Key::Character(text) => text,
        _ => return None,
    };
    let modifiers = event.modifiers();
    shortcut_for(&key, modifiers.ctrl(), modifiers.meta(), modifiers.shift(), cfg!(target_os = "macos"))
}

// Every known project with its sessions. A project whose listing fails is kept (empty) and the
// first failure is reported, so one bad directory never blanks the sidebar.
async fn load_past(
    services: &WorkspaceServices,
) -> (Vec<(ProjectSummary, Vec<SessionSummary>)>, Option<String>) {
    let projects = match services.projects.list_projects().await {
        Ok(projects) => projects,
        Err(error) => return (Vec::new(), Some(error.to_string())),
    };
    let mut loaded = Vec::with_capacity(projects.len());
    let mut problem = None;
    for project in projects {
        match services.sessions.list_sessions(&project.path).await {
            Ok(sessions) => loaded.push((project, sessions)),
            Err(error) => {
                problem.get_or_insert_with(|| format!("{}: {error}", project.path.display()));
                loaded.push((project, Vec::new()));
            }
        }
    }
    (loaded, problem)
}
