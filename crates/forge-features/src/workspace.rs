use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::{Phase, ConnectorCatalogUseCase, ExtensionsUseCase, PluginCatalogUseCase, SkillCatalogUseCase, LiveSessionHandle, LiveSessionId, LiveSessionView, ProbeSlot, SessionRegistry, SettingsUseCase};
use forge_entities::TranscriptView;
use forge_domain::{ArchivedRef, Attachment, AvailableModel, Card, LineRange, EditorPrefs, EffortLevel, PermissionDecision, PermissionMode, ProjectSummary, SearchHit, SessionId, SessionSummary, SidebarPrefs, SlashCommand};
use forge_ports::{BrowserPort, FilePickerPort, FileVersion, FilesPort, FolderPickerPort, GitPort, McpLogPort, ProjectRepositoryPort, SessionRepositoryPort, TerminalPort, UsageStatsPort, WebViewEvent, WebViewEventKind, WebViewPort, WebViewTabId};
use forge_ports::{KeyForward, Surface};
use time::OffsetDateTime;

use crate::cards::{clamp_cards, Cards};
use crate::closed_sessions::ClosedSessions;
use crate::editor_tabs::EditorTabs;
use crate::files_store::{FilesStore, Restore};
use crate::dock::{DockState, DockStates, Panel};
use crate::folder_choice::{choose_folder, FolderChoice};
use crate::nav_history::NavHistory;
use crate::preview_host::PreviewHost;
use crate::preview_tabs::{normalize_url, PreviewTabId, PreviewTabs};
use crate::new_session::ModelChoice;
use crate::shortcut::{match_code, shortcut_allowed, shortcut_for, swallowed_behind_overlay, types_text, KeyPress, Shortcut};
use crate::tool_activity::finished_tool_calls;
use crate::transcript_views::TranscriptViews;
use crate::arrange::{arrange, open_session_order, ArrangedGroup};
use crate::search::{merge_search, recent_rows, SearchRow, RECENT_LIMIT};
use crate::sidebar::{archive_key_plan, cycle_target, ArchiveKey, SidebarEntry, nth_session, mark_archived, merge_sidebar, project_for_open, replace_project_sessions, EntryKey, LiveSummary, SidebarGroup};
use forge_shared::tr;

// Typing waits this long before the transcripts are scanned.
const SEARCH_DEBOUNCE: Duration = Duration::from_millis(200);
const SEARCH_LIMIT: usize = 50;

/// Everything the Routines page and its timer need.
#[derive(Clone)]
pub struct RoutineServices {
    pub routines: Arc<forge_application::RoutinesUseCase>,
    pub runner: Arc<forge_application::RoutineRunner>,
    pub host: Arc<forge_application::RegistryHost>,
    /// Sessions the host started, waiting to be shown. The UI loop drains it.
    pub starts: Arc<std::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<LiveSessionHandle>>>,
    /// Tells the person when a routine waits for approval.
    pub notify: Arc<dyn forge_ports::NotifyPort>,
}

/// Everything the workspace needs from the outside, injected once by the composition root.
#[derive(Clone)]
pub struct WorkspaceServices {
    pub registry: Arc<SessionRegistry>,
    pub projects: Arc<dyn ProjectRepositoryPort>,
    pub sessions: Arc<dyn SessionRepositoryPort>,
    pub settings: Arc<SettingsUseCase>,
    pub extensions: Arc<ExtensionsUseCase>,
    pub plugin_catalog: Arc<PluginCatalogUseCase>,
    pub skill_catalog: Arc<SkillCatalogUseCase>,
    pub connector_catalog: Arc<ConnectorCatalogUseCase>,
    pub folder_picker: Arc<dyn FolderPickerPort>,
    pub file_picker: Arc<dyn FilePickerPort>,
    pub terminal: Arc<dyn TerminalPort>,
    pub git: Arc<dyn GitPort>,
    pub files: Arc<dyn FilesPort>,
    pub browser: Arc<dyn BrowserPort>,
    pub webviews: Arc<dyn WebViewPort>,
    pub usage_stats: Arc<dyn UsageStatsPort>,
    pub mcp_logs: Arc<dyn McpLogPort>,
    pub routines: RoutineServices,
}

/// What the main pane shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Active {
    NewSession,
    Customize,
    Routines,
    Live(LiveSessionId),
}

#[derive(Clone)]
struct LiveEntry {
    handle: LiveSessionHandle,
    view: Signal<LiveSessionView>,
}

/// A picture shown over the window by a click on a picture path in the chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The session folder, which `FilesPort::read` takes as its root.
    pub folder: PathBuf,
    /// The picture's path inside the folder.
    pub relative: String,
}

/// The UI-side session store: the sidebar's past sessions, the open sessions and the active one.
/// It is `Copy` (every field is a signal), so event handlers can capture it freely.
#[derive(Clone, Copy)]
pub struct Workspace {
    services: Signal<WorkspaceServices>,
    past: Signal<Vec<(ProjectSummary, Vec<SessionSummary>)>>,
    live: Signal<Vec<LiveEntry>>,
    active: Signal<Active>,
    history: Signal<NavHistory<Active>>,
    notice: Signal<Option<String>>,
    new_folder: Signal<Option<PathBuf>>,
    extra_folders: Signal<Vec<PathBuf>>,
    picking: Signal<bool>,
    probe: Signal<Arc<ProbeSlot>>,
    new_commands: Signal<Vec<SlashCommand>>,
    new_models: Signal<Vec<AvailableModel>>,
    transcript_views: Signal<TranscriptViews>,
    prefs: Signal<SidebarPrefs>,
    editor: Signal<EditorPrefs>,
    archived: Signal<Vec<ArchivedRef>>,
    docks: Signal<DockStates>,
    file_requests: Signal<HashMap<LiveSessionId, String>>,
    previews: Signal<PreviewHost>,
    preview_surface: Signal<Option<Surface>>,
    overlay_open: Signal<bool>,
    key_forward: Signal<Option<KeyForward>>,
    lightbox: Signal<Option<Picture>>,
    cards: Signal<Cards>,
    files_stores: Signal<HashMap<LiveSessionId, FilesStore>>,
    queued_files: Signal<HashMap<LiveSessionId, Vec<PathBuf>>>,
    comment_focus: Signal<HashMap<LiveSessionId, u64>>,
    search_open: Signal<bool>,
    shortcuts_open: Signal<bool>,
    search_query: Signal<String>,
    search_hits: Signal<Vec<SearchHit>>,
    search_task: Signal<Option<dioxus::core::Task>>,
    closed: Signal<ClosedSessions>,
    rename_request: Signal<Option<EntryKey>>,
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
        let mut models = slot.subscribe_models();
        let mut new_models = Signal::new_in_scope(Vec::new(), ScopeId::ROOT);
        dioxus::core::spawn_forever(async move {
            while models.changed().await.is_ok() {
                new_models.set(models.borrow_and_update().clone());
            }
        });
        Workspace {
            services: Signal::new(services),
            past: Signal::new(Vec::new()),
            live: Signal::new(Vec::new()),
            active: Signal::new(Active::NewSession),
            history: Signal::new(NavHistory::new(Active::NewSession)),
            notice: Signal::new(None),
            new_folder: Signal::new(None),
            extra_folders: Signal::new(Vec::new()),
            picking: Signal::new(false),
            probe: Signal::new(slot),
            new_commands,
            new_models,
            transcript_views: Signal::new(TranscriptViews::default()),
            prefs: Signal::new(SidebarPrefs::default()),
            editor: Signal::new(EditorPrefs::default()),
            archived: Signal::new(Vec::new()),
            docks: Signal::new(DockStates::default()),
            file_requests: Signal::new(HashMap::new()),
            previews: Signal::new(PreviewHost::default()),
            preview_surface: Signal::new(None),
            overlay_open: Signal::new(false),
            key_forward: Signal::new(None),
            lightbox: Signal::new(None),
            cards: Signal::new(Cards::default()),
            files_stores: Signal::new(HashMap::new()),
            queued_files: Signal::new(HashMap::new()),
            comment_focus: Signal::new(HashMap::new()),
            search_open: Signal::new(false),
            shortcuts_open: Signal::new(false),
            search_query: Signal::new(String::new()),
            search_hits: Signal::new(Vec::new()),
            search_task: Signal::new(None),
            closed: Signal::new(ClosedSessions::default()),
            rename_request: Signal::new(None),
        }
    });
    use_context_provider(move || workspace);
    use_hook(move || workspace.reload_past());
    use_hook(move || workspace.reload_prefs());
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
        let mut groups = merge_sidebar(&self.past.read(), &open);
        mark_archived(&mut groups, &self.archived.read());
        groups
    }

    /// How the sidebar is sorted, grouped and sized.
    pub fn prefs(&self) -> SidebarPrefs {
        self.prefs.read().clone()
    }

    /// Changes the preferences and saves them.
    pub fn update_prefs(&self, change: impl FnOnce(&mut SidebarPrefs)) {
        self.change_prefs_unsaved(change);
        self.save_prefs();
    }

    /// Changes the preferences on screen only (a drag changes the width many times); call
    /// `save_prefs` when it ends.
    pub fn change_prefs_unsaved(&self, change: impl FnOnce(&mut SidebarPrefs)) {
        let mut prefs = self.prefs;
        change(&mut prefs.write());
    }

    pub fn save_prefs(&self) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.prefs();
        // Root scope: the save must finish even if the control that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_sidebar(&prefs).await {
                this.report(tr!("failed to save the sidebar settings: {error}", error = error));
            }
        });
    }

    /// How the Files panel's editor behaves (its `⋮` menu).
    pub fn editor_prefs(&self) -> EditorPrefs {
        self.editor.read().clone()
    }

    /// Changes the editor preferences and saves them.
    pub fn update_editor_prefs(&self, change: impl FnOnce(&mut EditorPrefs)) {
        let mut editor = self.editor;
        change(&mut editor.write());
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.editor_prefs();
        // Root scope: the save must finish even if the menu that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_editor(&prefs).await {
                this.report(tr!("failed to save the editor settings: {error}", error = error));
            }
        });
    }

    // A config that cannot be read keeps the defaults; the settings screen reports that file's
    // problems, so the sidebar does not repeat them.
    fn reload_prefs(&self) {
        let services = (*self.services.peek()).clone();
        let mut prefs = self.prefs;
        let mut archived = self.archived;
        let mut editor = self.editor;
        spawn(async move {
            if let Ok(config) = services.settings.load().await {
                prefs.set(config.sidebar);
                editor.set(config.editor);
                archived.set(config.archived);
            }
        });
    }

    /// The sidebar as drawn: sorted, grouped and collapsed as the preferences say.
    pub fn arranged_sidebar(&self, now: OffsetDateTime) -> Vec<ArrangedGroup> {
        let active = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession | Active::Customize | Active::Routines => None,
        };
        arrange(&self.sidebar(), &self.prefs.read(), now, active)
    }

    pub fn shortcuts_open(&self) -> bool {
        *self.shortcuts_open.read()
    }

    pub fn open_shortcuts(&self) {
        let mut open = self.shortcuts_open;
        open.set(true);
    }

    pub fn close_shortcuts(&self) {
        let mut open = self.shortcuts_open;
        open.set(false);
    }

    pub fn toggle_shortcuts(&self) {
        if self.shortcuts_open() {
            self.close_shortcuts();
        } else {
            self.open_shortcuts();
        }
    }

    pub fn search_open(&self) -> bool {
        *self.search_open.read()
    }

    /// Opens the palette empty, listing the newest sessions.
    pub fn open_search(&self) {
        let mut open = self.search_open;
        let mut query = self.search_query;
        let mut hits = self.search_hits;
        query.set(String::new());
        hits.set(Vec::new());
        open.set(true);
    }

    pub fn close_search(&self) {
        self.cancel_search();
        let mut open = self.search_open;
        open.set(false);
    }

    pub fn search_query(&self) -> String {
        self.search_query.read().clone()
    }

    /// Sets the query. The transcripts are scanned after a short pause, and typing again cancels a
    /// scan that is still going, so only the answer to the latest query is ever shown.
    pub fn set_search_query(&self, text: String) {
        self.cancel_search();
        let mut query = self.search_query;
        let mut hits = self.search_hits;
        query.set(text.clone());
        // Quotes of an older query would be wrong for this one.
        hits.set(Vec::new());
        if text.trim().is_empty() {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        let task = spawn(async move {
            tokio::time::sleep(SEARCH_DEBOUNCE).await;
            match services.sessions.search_sessions(&text, SEARCH_LIMIT).await {
                Ok(found) => hits.set(found),
                Err(error) => this.report(error.to_string()),
            }
        });
        let mut slot = self.search_task;
        slot.set(Some(task));
    }

    fn cancel_search(&self) {
        let mut slot = self.search_task;
        let task = slot.write().take();
        if let Some(task) = task {
            task.cancel();
        }
    }

    /// The palette's rows: the newest sessions for an empty query, else the matches.
    pub fn search_rows(&self) -> Vec<SearchRow> {
        let query = self.search_query();
        if query.trim().is_empty() {
            recent_rows(&self.sidebar(), RECENT_LIMIT)
        } else {
            merge_search(&self.sidebar(), &query, &self.search_hits.read())
        }
    }

    /// Opens the session a row stands for and closes the palette.
    pub fn open_search_row(&self, row: &SearchRow) {
        match &row.key {
            EntryKey::Live(id) => self.select(*id),
            EntryKey::Past { session, .. } => self.open_past(row.project.clone(), session.clone()),
        }
        self.close_search();
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
                Ok(()) => {
                    this.set_archived(project.clone(), session.clone(), false);
                    this.reload_project(project);
                }
                Err(error) => {
                    this.report(error.to_string());
                    // A delete can fail half way (transcript trashed, its folder not): reload so
                    // what is gone from disk is gone from the list.
                    this.reload_project(project.clone());
                }
            }
        });
    }

    /// Whether the person archived this session.
    pub fn is_archived(&self, project: &Path, session: &SessionId) -> bool {
        self.archived
            .read()
            .iter()
            .any(|item| item.project == project && item.session_id == session.0)
    }

    /// Hides a session from the default list. Nothing on disk changes.
    pub fn archive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project, session, true);
    }

    /// Puts an archived session back in the default list.
    pub fn unarchive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project, session, false);
    }

    fn set_archived(&self, project: PathBuf, session: SessionId, archive: bool) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut archived = self.archived;
        toggle_archived(&mut archived.write(), &project, &session.0, archive);
        let list = self.archived.peek().clone();
        // Root scope: the save must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_archived(&list).await {
                this.report(tr!("failed to save the archived sessions: {error}", error = error));
            }
        });
    }

    /// Forks a session (open or not): the copy is made, the project's list is re-read and the copy is
    /// opened. A failure shows as a notice and nothing is opened.
    pub fn fork_session(&self, project: PathBuf, session: SessionId, title: String) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let mut past = self.past;
        // Root scope: a disk mutation must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            let new_id = match services.registry.fork(&project, &session, &title).await {
                Ok(id) => id,
                Err(error) => return this.report(error.to_string()),
            };
            match services.sessions.list_sessions(&project).await {
                Ok(sessions) => replace_project_sessions(&mut past.write(), &project, sessions),
                Err(error) => return this.report(format!("{}: {error}", project.display())),
            }
            let summary = project_for_open(&past.peek(), &project);
            this.open_past(summary, new_id);
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

    /// How much of the open session the chat shows: the person's choice for it, else `default`.
    pub fn transcript_view(&self, id: LiveSessionId, default: TranscriptView) -> TranscriptView {
        self.transcript_views.read().get(id, default)
    }

    /// The dock of one session: which panels are open and how wide.
    pub fn dock(&self, id: LiveSessionId) -> DockState {
        self.docks.read().get(id)
    }

    pub fn change_dock(&self, id: LiveSessionId, change: impl FnOnce(&mut DockState)) {
        let mut docks = self.docks;
        docks.write().change(id, change);
    }

    /// The Preview tabs of a session.
    pub fn previews(&self, id: LiveSessionId) -> PreviewTabs {
        self.previews.read().tabs(id)
    }

    pub fn change_previews(&self, id: LiveSessionId, change: impl FnOnce(&mut PreviewTabs)) {
        let mut previews = self.previews;
        previews.write().change(id, change);
        // Another tab may be on top now.
        self.sync_web_views();
    }

    /// The page's own back, forward and reload. A tab whose address could not be reached tries it again.
    pub fn preview_back(&self, tab: PreviewTabId) {
        self.webviews().back(WebViewTabId(tab.0));
    }

    pub fn preview_forward(&self, tab: PreviewTabId) {
        self.webviews().forward(WebViewTabId(tab.0));
    }

    pub fn preview_reload(&self, id: LiveSessionId, tab: PreviewTabId) -> Result<(), String> {
        match self.previews(id).failure(tab) {
            Some((url, _)) => self.preview_load(id, tab, &url),
            None => {
                self.webviews().reload(WebViewTabId(tab.0));
                Ok(())
            }
        }
    }

    /// An address typed in the Preview bar: the tab shows it. The error is a message for the person.
    pub fn preview_go(&self, id: LiveSessionId, tab: PreviewTabId, typed: &str) -> Result<(), String> {
        let url = normalize_url(typed).ok_or_else(|| tr!("Enter a web address, such as localhost:3000").to_string())?;
        self.preview_load(id, tab, &url)
    }

    fn preview_load(&self, id: LiveSessionId, tab: PreviewTabId, url: &str) -> Result<(), String> {
        let (webviews, browser, surface) = (self.webviews(), self.browser(), self.surface());
        let mut previews = self.previews;
        let result = previews.write().go(id, tab, url, surface, webviews.as_ref(), browser.as_ref());
        self.sync_web_views();
        result
    }

    /// Opens a file of the session in a new Preview tab; the panel opens if it is closed.
    pub fn open_in_preview(&self, id: LiveSessionId, absolute_path: &str) {
        self.open_preview_tab(id, &forge_entities::file_address(absolute_path));
    }

    fn open_preview_tab(&self, id: LiveSessionId, url: &str) {
        self.change_dock(id, |dock| {
            if !dock.is_open(Panel::Preview) {
                dock.toggle(Panel::Preview);
            }
        });
        let mut tab = None;
        self.change_previews(id, |tabs| tab = Some(tabs.open()));
        if let Some(tab) = tab {
            if let Err(message) = self.preview_load(id, tab, url) {
                self.report(message);
            }
        }
    }

    /// Closes a Preview tab and its web view; the panel closes with its last tab.
    pub fn close_preview_tab(&self, id: LiveSessionId, tab: PreviewTabId) {
        let webviews = self.webviews();
        let mut previews = self.previews;
        previews.write().close_tab(id, tab, webviews.as_ref());
        if self.previews(id).is_empty() {
            self.change_dock(id, |dock| dock.close(Panel::Preview));
        }
        self.sync_web_views();
    }

    /// Takes in what a web view reported.
    pub fn apply_web_view_event(&self, event: WebViewEvent) {
        if let WebViewEventKind::Key(key) = event.kind {
            let mut forward = self.key_forward;
            forward.set(Some(key));
            return;
        }
        let mut previews = self.previews;
        let asked = previews.write().apply(event);
        if let Some((id, url)) = asked {
            self.open_preview_tab(id, &url);
        }
        self.sync_web_views();
    }

    pub fn set_surface(&self, surface: Option<Surface>) {
        let mut slot = self.preview_surface;
        if *slot.peek() != surface {
            slot.set(surface);
        }
    }

    pub fn surface(&self) -> Option<Surface> {
        *self.preview_surface.peek()
    }

    pub fn set_overlay_open(&self, open: bool) {
        let mut slot = self.overlay_open;
        if *slot.peek() != open {
            slot.set(open);
        }
    }

    /// A key a page sent on, once: reading it clears it.
    pub fn take_key_forward(&self) -> Option<KeyForward> {
        let mut slot = self.key_forward;
        let key = slot.write().take();
        key
    }

    /// Whether the Preview panel of the session on screen is open (reads, so an effect follows it).
    pub fn active_dock_open_preview(&self) -> bool {
        match self.active() {
            Active::Live(id) => self.dock(id).is_open(Panel::Preview),
            _ => false,
        }
    }

    /// Puts every web view where it belongs: shown over the surface, or hidden.
    pub fn sync_web_views(&self) {
        let active = match self.active() {
            Active::Live(id) => Some(id),
            _ => None,
        };
        let (overlay, surface) = (*self.overlay_open.peek(), self.surface());
        let plan = self.previews.peek().visibility(active, &|id| self.docks.peek().get(id).is_open(Panel::Preview), overlay, surface);
        let webviews = self.webviews();
        for (tab, show) in plan {
            if show {
                if let Some(surface) = surface {
                    webviews.set_surface(tab, surface);
                }
            }
            webviews.set_visible(tab, show);
        }
    }

    /// Opens a file in the session's Files panel. The path must be absolute and inside the session
    /// folder; anything else only sets a notice. The panel opens if it is closed and shows the file
    /// once it is mounted. A picture opens in the lightbox instead.
    pub fn open_file(&self, id: LiveSessionId, absolute_path: String) {
        let Some(view) = self.live_view(id) else { return };
        let folder = view.read().project_path.clone();
        let Some(relative) = forge_entities::relative_to_folder(&folder, &absolute_path) else {
            let mut notice = self.notice;
            notice.set(Some(tr!("Only files inside the session folder can be opened here.").to_string()));
            return;
        };
        match forge_entities::open_target(&relative) {
            // A picture opens in the lightbox over the window, not in the Files panel.
            forge_entities::OpenTarget::Lightbox => {
                let mut lightbox = self.lightbox;
                lightbox.set(Some(Picture { folder, relative }));
                return;
            }
            // A page, a pdf or an svg opens in a new tab of the Preview panel.
            forge_entities::OpenTarget::Preview => {
                self.open_in_preview(id, &absolute_path);
                return;
            }
            forge_entities::OpenTarget::Files => {}
        }
        self.change_dock(id, |dock| {
            if !dock.is_open(Panel::Files) {
                dock.toggle(Panel::Files);
            }
        });
        let mut requests = self.file_requests;
        requests.write().insert(id, relative);
    }

    /// The same for the session on screen (a click in the page does not know its session).
    pub fn open_file_in_active(&self, absolute_path: String) {
        if let Active::Live(id) = self.active() {
            self.open_file(id, absolute_path);
        }
    }

    /// The file a click asked the Files panel of this session to show, if any.
    pub fn file_request(&self, id: LiveSessionId) -> Option<String> {
        self.file_requests.read().get(&id).cloned()
    }

    pub fn clear_file_request(&self, id: LiveSessionId) {
        let mut requests = self.file_requests;
        requests.write().remove(&id);
    }

    /// The picture shown over the window, if any.
    pub fn lightbox(&self) -> Option<Picture> {
        self.lightbox.read().clone()
    }

    pub fn close_lightbox(&self) {
        let mut lightbox = self.lightbox;
        lightbox.set(None);
    }

    /// The cards (excerpts and comments) of this session that are not sent yet.
    pub fn cards(&self, id: LiveSessionId) -> Vec<Card> {
        self.cards.read().list(id)
    }

    pub fn add_excerpt(&self, id: LiveSessionId, path: String, range: LineRange, text: String) {
        let mut cards = self.cards;
        cards.write().add_excerpt(id, path, range, text);
    }

    pub fn add_comment(&self, id: LiveSessionId, path: String, range: LineRange, text: String) -> u64 {
        let mut cards = self.cards;
        let card_id = cards.write().add_comment(id, path, range, text);
        card_id
    }

    pub fn update_comment(&self, id: LiveSessionId, card_id: u64, text: String) {
        let mut cards = self.cards;
        cards.write().update_comment(id, card_id, text);
    }

    pub fn remove_card(&self, id: LiveSessionId, card_id: u64) {
        let mut cards = self.cards;
        cards.write().remove(id, card_id);
    }

    /// Takes the cards for a send, each range kept inside its file (`lines_of` is a file's line count).
    pub fn take_cards(&self, id: LiveSessionId, lines_of: impl Fn(&str) -> Option<u32>) -> Vec<Card> {
        let mut cards = self.cards;
        let mut taken = cards.write().take(id);
        clamp_cards(&mut taken, lines_of);
        taken
    }

    /// Gives cards back (a send that did not happen).
    pub fn restore_cards(&self, id: LiveSessionId, list: Vec<Card>) {
        let mut cards = self.cards;
        cards.write().restore(id, list);
    }

    /// The Files panel's tabs and markdown source set of this session, as kept when the panel left.
    pub fn files_view(&self, id: LiveSessionId) -> (EditorTabs, HashSet<String>) {
        let stores = self.files_stores.peek();
        stores.get(&id).map(|store| (store.tabs.clone(), store.source.clone())).unwrap_or_default()
    }

    pub fn save_files_view(&self, id: LiveSessionId, tabs: EditorTabs, source: HashSet<String>) {
        let mut stores = self.files_stores;
        let mut write = stores.write();
        let store = write.entry(id).or_default();
        store.tabs = tabs;
        store.source = source;
    }

    /// The editor reported `text` for a tab: kept as a draft when it is not what is on disk. A session
    /// that was closed has no store any more and gets none back.
    pub fn note_draft(&self, id: LiveSessionId, path: &str, text: &str, disk_hash: u64, base: FileVersion, conflict: bool) {
        let mut stores = self.files_stores;
        let mut write = stores.write();
        if let Some(store) = write.get_mut(&id) {
            store.note_text(path, text, disk_hash, base, conflict);
        }
    }

    /// A write of `text` succeeded and left the file at `version`.
    pub fn draft_written(&self, id: LiveSessionId, path: &str, text: &str, version: FileVersion) {
        let mut stores = self.files_stores;
        let mut write = stores.write();
        if let Some(store) = write.get_mut(&id) {
            store.written(path, text, version);
        }
    }

    pub fn draft_text(&self, id: LiveSessionId, path: &str) -> Option<String> {
        self.files_stores.peek().get(&id).and_then(|store| store.draft_text(path))
    }

    pub fn clear_draft(&self, id: LiveSessionId, path: &str) {
        let mut stores = self.files_stores;
        let mut write = stores.write();
        if let Some(store) = write.get_mut(&id) {
            store.clear_draft(path);
        }
    }

    pub fn restore_draft(&self, id: LiveSessionId, path: &str, fresh: FileVersion, fresh_hash: u64) -> Restore {
        self.files_stores.peek().get(&id).map_or(Restore::FromDisk, |store| store.restore(path, fresh, fresh_hash))
    }

    /// A comment card in the message box was clicked: the Files panel shows its box.
    pub fn request_comment_focus(&self, id: LiveSessionId, card_id: u64) {
        let mut focus = self.comment_focus;
        focus.write().insert(id, card_id);
    }

    pub fn comment_focus(&self, id: LiveSessionId) -> Option<u64> {
        self.comment_focus.read().get(&id).copied()
    }

    pub fn clear_comment_focus(&self, id: LiveSessionId) {
        let mut focus = self.comment_focus;
        focus.write().remove(&id);
    }

    /// A file the Files panel wants attached to the next message of this session.
    pub fn queue_file(&self, id: LiveSessionId, path: PathBuf) {
        let mut queued = self.queued_files;
        queued.write().entry(id).or_default().push(path);
    }

    pub fn queued_files(&self, id: LiveSessionId) -> Vec<PathBuf> {
        self.queued_files.read().get(&id).cloned().unwrap_or_default()
    }

    pub fn clear_queued_files(&self, id: LiveSessionId) {
        let mut queued = self.queued_files;
        queued.write().remove(&id);
    }

    pub fn terminal(&self) -> Arc<dyn TerminalPort> {
        self.services.peek().terminal.clone()
    }

    pub fn git(&self) -> Arc<dyn GitPort> {
        self.services.peek().git.clone()
    }

    pub fn usage_stats_port(&self) -> Arc<dyn UsageStatsPort> {
        self.services.peek().usage_stats.clone()
    }

    pub fn files(&self) -> Arc<dyn FilesPort> {
        self.services.peek().files.clone()
    }

    pub fn browser(&self) -> Arc<dyn BrowserPort> {
        self.services.peek().browser.clone()
    }

    pub fn webviews(&self) -> Arc<dyn WebViewPort> {
        self.services.peek().webviews.clone()
    }

    /// Shows a file in Finder; a failure becomes a notice.
    pub fn reveal(&self, path: &Path) {
        if let Err(message) = self.browser().reveal(path) {
            self.report(message);
        }
    }

    /// Shows the file that defines the person's own MCP servers in Finder.
    pub fn reveal_mcp_config(&self) {
        let path = self.services.peek().mcp_logs.config_file();
        self.reveal(&path);
    }

    /// Opens the newest log of a local MCP server with the default app. With none, the message says so: the
    /// page that asked shows it, because a notice would sit behind the Settings window.
    pub fn open_mcp_log(&self, server: &str) -> Result<(), String> {
        let path = crate::developer::log_to_open(self.services.peek().mcp_logs.as_ref(), server)?;
        crate::developer::open_log(self.browser().as_ref(), &path)
    }

    /// Opens a file with the default app or a named one; a failure becomes a notice.
    pub fn open_with_app(&self, path: &Path, app: Option<&'static str>) {
        if let Err(message) = self.browser().open_with(path, app) {
            self.report(message);
        }
    }

    /// How many tool calls of an open session have finished. A panel that shows files on disk
    /// reads this inside its loader, so it reloads when a tool call finishes. 0 for a session
    /// that is not open.
    pub fn finished_tool_calls(&self, id: LiveSessionId) -> usize {
        self.live_view(id).map(|view| finished_tool_calls(&view.read().session)).unwrap_or(0)
    }

    pub fn set_transcript_view(&self, id: LiveSessionId, view: TranscriptView) {
        let mut views = self.transcript_views;
        views.write().set(id, view);
    }

    /// Changes the permission mode of the open session, from its next turn on.
    pub fn set_permission_mode(&self, id: LiveSessionId, mode: PermissionMode) {
        if let Some(handle) = self.handle(id) {
            handle.set_permission_mode(mode);
        }
    }

    /// Changes the model of the open session (`None`: the SDK's default), from its next turn on.
    pub fn set_model(&self, id: LiveSessionId, model: Option<String>) {
        if let Some(handle) = self.handle(id) {
            handle.set_model(model);
        }
    }

    /// Changes the effort of the open session (`None`: the SDK's default), from its next turn on.
    pub fn set_effort(&self, id: LiveSessionId, effort: Option<EffortLevel>) {
        if let Some(handle) = self.handle(id) {
            handle.set_effort(effort);
        }
    }

    /// Every open session with its view.
    pub fn live_views(&self) -> Vec<(LiveSessionId, Signal<LiveSessionView>)> {
        self.live.read().iter().map(|entry| (entry.handle.id(), entry.view)).collect()
    }

    /// Whether a routine run started this session (the routines timer tells about those).
    pub fn is_routine_session(&self, id: LiveSessionId) -> bool {
        self.services.peek().routines.host.is_routine_session(id)
    }

    /// Where notifications go.
    pub fn notify_port(&self) -> Arc<dyn forge_ports::NotifyPort> {
        self.services.peek().routines.notify.clone()
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

    /// Shows the Customize page in the main pane.
    pub fn show_customize(&self) {
        self.navigate(Active::Customize);
    }

    /// Shows the Routines page in the main pane.
    pub fn show_routines(&self) {
        self.navigate(Active::Routines);
    }

    /// Shows a session that something other than the person started (a routine). It is not selected:
    /// the person keeps what they were doing.
    pub fn adopt(&self, handle: LiveSessionHandle) {
        self.ensure_attached(handle);
    }

    /// What a routine's run is now, read from its session when it is open: "Running", "Waiting for
    /// approval" or "Done".
    pub fn run_state(&self, claude_session_id: &str) -> &'static str {
        let open = self.live.read().iter().find_map(|entry| {
            let view = entry.view.read();
            (view.claude_session_id.as_ref().is_some_and(|id| id.0 == claude_session_id)).then(|| (view.phase, view.pending_permissions.len()))
        });
        crate::routines::run_state_text(open)
    }

    /// Shows the session with this Claude session id: the open one, or the past one from the sidebar's list.
    pub fn open_session_by_claude_id(&self, claude_session_id: &str) {
        let open = self.live.read().iter().find_map(|entry| {
            let view = entry.view.read();
            view.claude_session_id.as_ref().is_some_and(|id| id.0 == claude_session_id).then(|| entry.handle.id())
        });
        if let Some(id) = open {
            self.select(id);
            return;
        }
        let found = self
            .past
            .read()
            .iter()
            .find(|(_, sessions)| sessions.iter().any(|session| session.id.0 == claude_session_id))
            .map(|(project, _)| project.clone());
        match found {
            Some(project) => self.open_past(project, SessionId(claude_session_id.to_string())),
            None => self.report(tr!("That session is not in the sidebar any more.").to_string()),
        }
    }

    // The active session's folder and sidebar entry (its title, its actions).
    fn active_entry(&self) -> Option<(PathBuf, SidebarEntry)> {
        let Active::Live(id) = self.active() else {
            return None;
        };
        self.sidebar().into_iter().find_map(|group| {
            let path = group.project.path.clone();
            group.entries.into_iter().find(|entry| entry.key == EntryKey::Live(id)).map(|entry| (path, entry))
        })
    }

    /// Forks the active session, as the row menu's Fork does.
    pub fn fork_active(&self) {
        if let Some((project, entry)) = self.active_entry() {
            if let (true, Some(session)) = (entry.actions().can_fork, entry.session_id.clone()) {
                self.fork_session(project, session, entry.title);
            }
        }
    }

    /// Archives the active session and closes it, or unarchives it when it is archived (see
    /// `ArchiveKey`).
    pub fn archive_active(&self) {
        let (Active::Live(id), Some((project, entry))) = (self.active(), self.active_entry()) else {
            return;
        };
        let running = self.live_view(id).is_some_and(|view| view.read().phase == Phase::Running);
        match (archive_key_plan(entry.archived, entry.session_id.is_some(), running), entry.session_id) {
            (ArchiveKey::Unarchive, Some(session)) => self.unarchive_session(project, session),
            (ArchiveKey::ArchiveAndClose, Some(session)) => {
                self.archive_session(project, session);
                self.close_remembering(id);
            }
            (ArchiveKey::StopFirst, _) => self.report(tr!("Stop the session before archiving it.").to_string()),
            _ => {}
        }
    }

    /// Puts the active session's sidebar row into rename mode. Nothing when the session cannot be
    /// renamed or its row is not in the list (the status filter hides it). A collapsed group still
    /// lists the active session's row, so only membership counts, not `collapsed`.
    pub fn rename_active(&self) {
        let Some((_, entry)) = self.active_entry() else {
            return;
        };
        let shown = self
            .arranged_sidebar(crate::arrange::now_local())
            .iter()
            .any(|group| group.entries.iter().any(|item| item.entry.key == entry.key));
        if entry.actions().can_rename && shown {
            let mut request = self.rename_request;
            request.set(Some(entry.key));
        }
    }

    /// The row that is asked to start renaming, until it takes the request.
    pub fn rename_request(&self) -> Option<EntryKey> {
        self.rename_request.read().clone()
    }

    pub fn clear_rename_request(&self) {
        let mut request = self.rename_request;
        request.set(None);
    }

    pub fn show_new_session(&self) {
        self.clear_rename_request();
        self.navigate(Active::NewSession);
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

    /// The models the probed folder's SDK lists, with their effort levels; empty until the probe answers.
    pub fn new_models(&self) -> Vec<AvailableModel> {
        self.new_models.read().clone()
    }

    /// The project the settings tabs describe: the active session's, or the one the new-session
    /// panel would start in, or none.
    pub fn context_project(&self) -> Option<PathBuf> {
        match self.active() {
            Active::Live(id) => self.live_view(id).map(|view| view.read().project_path.clone()),
            Active::NewSession | Active::Customize | Active::Routines => self
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
        self.clear_rename_request();
        self.navigate(Active::Live(id));
    }

    // Shows `to` and records it in the history the back and forward buttons walk.
    fn navigate(&self, to: Active) {
        self.display(to);
        let mut history = self.history;
        history.write().visit(to);
    }

    // Shows `to` without touching the history.
    fn display(&self, to: Active) {
        self.close_lightbox();
        if let Active::Live(id) = to {
            // A routine's session the person opens stays open: what they type or edit in it must not be
            // closed away by the routine timer.
            self.services.peek().routines.host.keep_open(id);
        }
        let mut active = self.active;
        active.set(to);
    }

    // A screen the history can return to: every page, and a session that is still open.
    fn reachable(&self, screen: Active) -> bool {
        match screen {
            Active::Live(id) => self.handle(id).is_some(),
            Active::NewSession | Active::Customize | Active::Routines => true,
        }
    }

    pub fn can_go_back(&self) -> bool {
        self.history.read().can_back(|screen| self.reachable(screen))
    }

    pub fn can_go_forward(&self) -> bool {
        self.history.read().can_forward(|screen| self.reachable(screen))
    }

    /// The back button: the screen the person was on before this one.
    pub fn go_back(&self) {
        let mut history = self.history;
        let target = history.write().back(|screen| self.reachable(screen));
        if let Some(target) = target {
            self.clear_rename_request();
            self.display(target);
        }
    }

    /// The forward button.
    pub fn go_forward(&self) {
        let mut history = self.history;
        let target = history.write().forward(|screen| self.reachable(screen));
        if let Some(target) = target {
            self.clear_rename_request();
            self.display(target);
        }
    }

    pub fn answer_permission(&self, id: LiveSessionId, request_id: String, decision: PermissionDecision) {
        if let Some(handle) = self.handle(id) {
            handle.approve(request_id, decision);
        }
    }

    pub fn stop_task(&self, id: LiveSessionId, task_id: String) {
        if let Some(handle) = self.handle(id) {
            handle.stop_task(task_id);
        }
    }

    pub fn answer_question(&self, id: LiveSessionId, request_id: String, answers: std::collections::BTreeMap<String, String>) {
        if let Some(handle) = self.handle(id) {
            handle.answer_question(request_id, answers);
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
        // Root scope: the search palette closes the moment it opens a session, and a task of the
        // palette's own scope would be cancelled with it before the session ever opened.
        dioxus::core::spawn_forever(async move {
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
    pub fn start_new(
        &self,
        project: ProjectSummary,
        mode: PermissionMode,
        model: ModelChoice,
        effort: Option<EffortLevel>,
        text: String,
        attachments: Vec<Attachment>,
        folders: Vec<PathBuf>,
    ) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let slot = (*self.probe.peek()).clone();
        // Pinned to the root scope: `NewSessionComposer` unmounts if the person clicks elsewhere
        // while the saved model is being read, which would cancel a plain `spawn`.
        dioxus::core::spawn_forever(async move {
            let model = match model {
                Some(chosen) => chosen,
                None => match services.settings.saved_model().await {
                    Ok(model) => model,
                    Err(error) => {
                        this.report(tr!("failed to load the Forge config: {error}", error = error));
                        return;
                    }
                },
            };
            let commands = slot.end();
            match services.registry.open_new_with_effort(&project, mode, model, effort, commands) {
                Ok(handle) => {
                    let id = this.attach(handle.clone());
                    if !folders.is_empty() {
                        handle.add_directories(folders);
                    }
                    handle.send_with(text, attachments);
                    this.select(id);
                }
                Err(error) => this.report(error.to_string()),
            }
        });
    }

    /// Opens the session closed last again (or focuses it when it is open once more). Does
    /// nothing when none was closed; a failed open shows as a notice, like any open.
    pub fn reopen_closed(&self) {
        let mut closed = self.closed;
        let Some((project, session)) = closed.write().pop() else {
            return;
        };
        // Archiving closed it; the default list hides archived rows, so bring it back to the list too.
        if self.is_archived(&project, &session) {
            self.unarchive_session(project.clone(), session.clone());
        }
        let summary = project_for_open(&self.past.peek(), &project);
        self.open_past(summary, session);
    }

    pub fn close(&self, id: LiveSessionId) {
        let services = (*self.services.peek()).clone();
        services.registry.close(id);
        let mut live = self.live;
        live.write().retain(|entry| entry.handle.id() != id);
        let mut views = self.transcript_views;
        views.write().forget(id);
        let mut docks = self.docks;
        docks.write().forget(id);
        let webviews = self.webviews();
        let mut previews = self.previews;
        previews.write().forget_session(id, webviews.as_ref());
        // What the session left behind: the Files panel's tabs and drafts, and unsent cards.
        let mut stores = self.files_stores;
        stores.write().remove(&id);
        let mut cards = self.cards;
        cards.write().take(id);
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
            self.close_remembering(id);
        }
    }

    /// Closes a session the person asked to close (Cmd+W, the archive key) and remembers it for
    /// "reopen the last closed session". Closes the app makes itself (a finished routine) use `close`.
    pub fn close_remembering(&self, id: LiveSessionId) {
        // A session that never got an id has no transcript to come back to.
        if let Some(handle) = self.handle(id) {
            let view = handle.snapshot();
            if let Some(session) = view.claude_session_id {
                let mut closed = self.closed;
                closed.write().push(view.project_path, session);
            }
        }
        self.close(id);
    }

    /// Focuses the next (or previous) open session in the order the sidebar draws them; `false` when
    /// no session is open, so the key press can be left to the page.
    pub fn cycle(&self, forward: bool) -> bool {
        let order = open_session_order(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local());
        let current = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession | Active::Customize | Active::Routines => None,
        };
        match cycle_target(&order, current, forward) {
            // The only open session: nothing to switch to, so the key stays the caret's.
            Some(target) if Some(target) == current => false,
            Some(target) => {
                self.select(target);
                true
            }
            None => false,
        }
    }

    /// Focuses the n-th open session (from 0), in the order the sidebar draws them. Nothing when
    /// fewer are open.
    pub fn go_to_session(&self, index: usize) {
        let order = open_session_order(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local());
        if let Some(id) = nth_session(&order, index) {
            self.select(id);
        }
    }

    /// The new-session page with the active session's folder chosen (just the page when no session is active).
    pub fn new_session_here(&self) {
        if let Active::Live(id) = self.active() {
            if let Some(view) = self.live_view(id) {
                let folder = view.read().project_path.clone();
                return self.show_new_session_in(folder);
            }
        }
        self.show_new_session();
    }

    /// Opens or closes `panel` in the active session's dock; nothing on the other pages.
    pub fn toggle_panel(&self, panel: Panel) {
        if let Active::Live(id) = self.active() {
            self.change_dock(id, |dock| dock.toggle(panel));
        }
    }

    /// Handles the session shortcuts; returns whether the key press was one of them. While the
    /// settings overlay is open they are swallowed (spec §17.5).
    pub fn on_key(&self, event: &KeyboardEvent, overlay_open: bool) -> bool {
        // Escape puts the shortcut list away.
        if self.shortcuts_open() && event.key() == Key::Escape {
            self.close_shortcuts();
            event.prevent_default();
            return true;
        }
        let Some(shortcut) = shortcut_from_event(event) else {
            return false;
        };
        if !shortcut_allowed(shortcut, overlay_open) {
            if !swallowed_behind_overlay(shortcut) {
                return false;
            }
            event.prevent_default();
            return true;
        }
        match shortcut {
            // Opening settings belongs to `Settings::on_key`; the page offers it the event next.
            Shortcut::OpenSettings => return false,
            Shortcut::NewSession => self.show_new_session(),
            Shortcut::OpenSearch => self.open_search(),
            Shortcut::NextSession => {
                if !self.cycle(true) {
                    return false;
                }
            }
            Shortcut::PreviousSession => {
                if !self.cycle(false) {
                    return false;
                }
            }
            Shortcut::NewSessionHere => self.new_session_here(),
            Shortcut::GoToSession(index) => self.go_to_session(index),
            Shortcut::TogglePanel(panel) => self.toggle_panel(panel),
            // The sidebar belongs to the page (it knows the layout); it offers the event next.
            Shortcut::ToggleSidebar => return false,
            Shortcut::CloseSession => self.close_active(),
            Shortcut::ReopenClosed => self.reopen_closed(),
            Shortcut::ArchiveActive => self.archive_active(),
            Shortcut::ForkActive => self.fork_active(),
            Shortcut::RenameActive => self.rename_active(),
            Shortcut::ShowShortcuts => self.toggle_shortcuts(),
        }
        event.prevent_default();
        true
    }

    pub fn report(&self, message: String) {
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
pub fn shortcut_from_event(event: &KeyboardEvent) -> Option<Shortcut> {
    let modifiers = event.modifiers();
    let mac = cfg!(target_os = "macos");
    let key = event.key().to_string();
    if types_text(&key, modifiers.ctrl(), modifiers.alt(), mac) {
        return None;
    }
    let code = match_code(&key, &event.code().to_string(), modifiers.alt());
    shortcut_for(
        &KeyPress { code: &code, ctrl: modifiers.ctrl(), meta: modifiers.meta(), alt: modifiers.alt(), shift: modifiers.shift() },
        mac,
    )
}

// Every known project with its sessions. A project whose listing fails is kept (empty) and the
// first failure is reported, so one bad directory never blanks the sidebar.
/// Adds or removes one session in the archived list; adding twice keeps one entry.
fn toggle_archived(list: &mut Vec<ArchivedRef>, project: &Path, session: &str, archive: bool) {
    let same = |item: &ArchivedRef| item.project == project && item.session_id == session;
    if archive {
        if !list.iter().any(same) {
            list.push(ArchivedRef { project: project.to_path_buf(), session_id: session.to_string() });
        }
    } else {
        list.retain(|item| !same(item));
    }
}

async fn load_past(
    services: &WorkspaceServices,
) -> (Vec<(ProjectSummary, Vec<SessionSummary>)>, Option<String>) {
    match services.projects.list_projects().await {
        Ok(projects) => list_all(projects, services.sessions.clone()).await,
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}

// How many projects are read at the same time: reading a transcript is mostly waiting for the disk.
const PROJECTS_AT_ONCE: usize = 8;

/// The sessions of every project, read a few projects at a time. The projects keep their order; one that
/// cannot be read shows no sessions and the first such problem is returned for a notice.
async fn list_all(
    projects: Vec<ProjectSummary>,
    sessions: Arc<dyn SessionRepositoryPort>,
) -> (Vec<(ProjectSummary, Vec<SessionSummary>)>, Option<String>) {
    let permits = Arc::new(tokio::sync::Semaphore::new(PROJECTS_AT_ONCE));
    let mut tasks = tokio::task::JoinSet::new();
    for (index, project) in projects.iter().enumerate() {
        let (path, sessions, permits) = (project.path.clone(), sessions.clone(), permits.clone());
        tasks.spawn(async move {
            let _permit = permits.acquire_owned().await;
            (index, sessions.list_sessions(&path).await)
        });
    }
    let mut results: Vec<Option<Result<Vec<SessionSummary>, String>>> = (0..projects.len()).map(|_| None).collect();
    while let Some(done) = tasks.join_next().await {
        if let Ok((index, result)) = done {
            results[index] = Some(result.map_err(|error| error.to_string()));
        }
    }
    let mut problem = None;
    let mut loaded = Vec::with_capacity(projects.len());
    for (project, result) in projects.into_iter().zip(results) {
        match result {
            Some(Ok(found)) => loaded.push((project, found)),
            Some(Err(error)) => {
                problem.get_or_insert_with(|| format!("{}: {error}", project.path.display()));
                loaded.push((project, Vec::new()));
            }
            None => loaded.push((project, Vec::new())),
        }
    }
    (loaded, problem)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use forge_domain::ArchivedRef;

    use super::*;

    fn project(path: &str) -> ProjectSummary {
        ProjectSummary { path: path.into(), last_active: String::new(), session_count: 1, exists: true }
    }

    fn summary(id: &str) -> SessionSummary {
        SessionSummary { id: SessionId(id.to_string()), title: None, updated_at: String::new(), created_at: String::new() }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn the_sessions_of_every_project_load_together_and_keep_the_projects_order() {
        let mut mock = forge_ports::MockSessionRepositoryPort::new();
        mock.expect_list_sessions().returning(|path| {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            // The first project answers last: the result must still list it first.
            if name == "a" {
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            if name == "c" {
                return Err(forge_ports::SessionRepositoryError::ListFailed("no access".to_string()));
            }
            Ok(vec![summary(&name)])
        });
        let projects = vec![project("/w/a"), project("/w/b"), project("/w/c"), project("/w/d")];

        let (loaded, problem) = list_all(projects, Arc::new(mock)).await;

        let names: Vec<_> = loaded.iter().map(|(project, _)| project.path.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, ["a", "b", "c", "d"]);
        assert_eq!(loaded[0].1[0].id.0, "a");
        assert!(loaded[2].1.is_empty(), "a project that cannot be listed shows no sessions");
        assert!(problem.unwrap().contains("/w/c"), "and the notice names it");
    }

    #[test]
    fn archiving_adds_once_and_unarchiving_removes() {
        let one = ArchivedRef { project: "/a".into(), session_id: "s1".to_string() };
        let mut list = vec![];

        toggle_archived(&mut list, Path::new("/a"), "s1", true);
        toggle_archived(&mut list, Path::new("/a"), "s1", true);
        assert_eq!(list, vec![one]);

        toggle_archived(&mut list, Path::new("/b"), "s1", false);
        assert_eq!(list.len(), 1, "another project's s1 is a different session");
        toggle_archived(&mut list, Path::new("/a"), "s1", false);
        assert!(list.is_empty());
    }
}
