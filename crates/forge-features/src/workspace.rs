use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{ExtensionsUseCase, LiveSessionHandle, LiveSessionId, LiveSessionView, ProbeSlot, SessionRegistry, SettingsUseCase};
use forge_entities::TranscriptView;
use forge_domain::{PermissionDecision, PermissionMode, ProjectSummary, SessionId, SessionSummary, SlashCommand};
use forge_ports::{FolderPickerPort, ProjectRepositoryPort, SessionRepositoryPort};

use crate::folder_choice::{choose_folder, FolderChoice};
use crate::shortcut::{shortcut_allowed, shortcut_for, Shortcut};
use crate::transcript_views::TranscriptViews;
use crate::sidebar::{cycle_target, merge_sidebar, open_order, replace_project_sessions, LiveSummary, SidebarGroup};

/// Everything the workspace needs from the outside, injected once by the composition root.
#[derive(Clone)]
pub struct WorkspaceServices {
    pub registry: Arc<SessionRegistry>,
    pub projects: Arc<dyn ProjectRepositoryPort>,
    pub sessions: Arc<dyn SessionRepositoryPort>,
    pub settings: Arc<SettingsUseCase>,
    pub extensions: Arc<ExtensionsUseCase>,
    pub folder_picker: Arc<dyn FolderPickerPort>,
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
    extra_folders: Signal<Vec<PathBuf>>,
    picking: Signal<bool>,
    probe: Signal<Arc<ProbeSlot>>,
    new_commands: Signal<Vec<SlashCommand>>,
    transcript_views: Signal<TranscriptViews>,
}

/// Creates the workspace, provides it to every descendant and loads the sidebar once. Call it in
/// the page at the top of the tree; descendants call `use_workspace`.
pub fn use_workspace_provider() -> Workspace {
    let services = use_context::<WorkspaceServices>();
    let workspace = use_hook(move || {
        let slot = Arc::new(ProbeSlot::new(services.registry.clone()));
        let mut list = slot.subscribe();
        let mut new_commands = Signal::new_in_scope(Vec::new(), ScopeId::ROOT);
        dioxus::core::spawn_forever(async move {
            while list.changed().await.is_ok() {
                new_commands.set(list.borrow_and_update().clone());
            }
        });
        Workspace {
            services: Signal::new(services),
            past: Signal::new(Vec::new()),
            live: Signal::new(Vec::new()),
            active: Signal::new(Active::NewSession),
            notice: Signal::new(None),
            new_folder: Signal::new(None),
            extra_folders: Signal::new(Vec::new()),
            picking: Signal::new(false),
            probe: Signal::new(slot),
            new_commands,
            transcript_views: Signal::new(TranscriptViews::default()),
        }
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

    /// The project folders a new session can start in: known projects that still exist, plus any
    /// folders added through the dialog.
    pub fn new_session_projects(&self) -> Vec<ProjectSummary> {
        let past = self.past.read();
        let mut projects: Vec<ProjectSummary> = past
            .iter()
            .filter(|(project, _)| project.exists)
            .map(|(project, _)| project.clone())
            .collect();
        for path in self.extra_folders.read().iter() {
            if !projects.iter().any(|project| project.path == *path) {
                projects.push(ProjectSummary {
                    path: path.clone(),
                    last_active: String::new(),
                    session_count: 0,
                    exists: true,
                });
            }
        }
        projects
    }

    /// Reloads one project's session list, leaving every other project as drawn.
    pub fn reload_project(&self, project: PathBuf) {
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        let mut notice = self.notice;
        // Root scope: a disk mutation must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            match services.sessions.list_sessions(&project).await {
                Ok(sessions) => replace_project_sessions(&mut past.write(), &project, sessions),
                Err(error) => notice.set(Some(format!("{}: {error}", project.display()))),
            }
        });
    }

    /// Renames a session (open or not). The list is re-read for that project only.
    pub fn rename_session(&self, project: PathBuf, session: SessionId, title: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: a disk mutation must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            match services.registry.rename(&project, &session, &title).await {
                Ok(_) => this.reload_project(project),
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Moves a session that is not open to the trash. The list is re-read for that project only.
    pub fn delete_session(&self, project: PathBuf, session: SessionId) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: a disk mutation must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            match services.registry.delete(&project, &session).await {
                Ok(()) => this.reload_project(project),
                Err(error) => {
                    this.report(error.to_string());
                    // A delete can fail half way (transcript trashed, its folder not): reload so
                    // what is gone from disk is gone from the list.
                    this.reload_project(project.clone());
                }
            }
        });
    }

    /// Whether a folder dialog is open; the button is disabled meanwhile so only one can be.
    pub fn picking_folder(&self) -> bool {
        *self.picking.read()
    }

    /// Opens the native folder dialog and applies the choice to the new-session panel.
    pub fn open_folder(&self) {
        if self.picking_folder() {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut picking = self.picking;
        picking.set(true);
        // Root scope: the dialog outlives a re-render of the composer that asked for it.
        dioxus::core::spawn_forever(async move {
            let picked = services.folder_picker.pick_folder().await;
            picking.set(false);
            let Some(picked) = picked else {
                return;
            };
            let known: Vec<PathBuf> = this.new_session_projects().into_iter().map(|project| project.path).collect();
            let is_dir = std::fs::metadata(&picked).map(|m| m.is_dir()).unwrap_or(false);
            match choose_folder(&known, picked, is_dir) {
                FolderChoice::Known(path) => this.set_new_folder(Some(path)),
                FolderChoice::Added(path) => {
                    let mut extra = this.extra_folders;
                    extra.write().push(path.clone());
                    this.set_new_folder(Some(path));
                }
                FolderChoice::Rejected(message) => this.report(message),
            }
        });
    }

    /// How much of the open session the chat shows. Normal until the person changes it.
    pub fn transcript_view(&self, id: LiveSessionId) -> TranscriptView {
        self.transcript_views.read().get(id)
    }

    pub fn set_transcript_view(&self, id: LiveSessionId, view: TranscriptView) {
        let mut views = self.transcript_views;
        views.write().set(id, view);
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

    /// The project the settings tabs describe: the active session's, or the one the new-session
    /// panel would start in, or none.
    pub fn context_project(&self) -> Option<PathBuf> {
        match self.active() {
            Active::Live(id) => self.live_view(id).map(|view| view.read().project_path.clone()),
            Active::NewSession => self
                .new_folder()
                .or_else(|| self.new_session_projects().into_iter().next().map(|project| project.path)),
        }
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
        let slot = (*self.probe.peek()).clone();
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
            let commands = slot.end();
            match services.registry.open_new(&project, mode, model, commands) {
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
        let mut views = self.transcript_views;
        views.write().forget(id);
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

    /// Handles the session shortcuts; returns whether the key press was one of them. While the
    /// settings overlay is open they are swallowed (spec §17.5).
    pub fn on_key(&self, event: &KeyboardEvent, overlay_open: bool) -> bool {
        let Some(shortcut) = shortcut_from_event(event) else {
            return false;
        };
        if !shortcut_allowed(shortcut, overlay_open) {
            event.prevent_default();
            return true;
        }
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
