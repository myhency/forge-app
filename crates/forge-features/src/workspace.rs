use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::{WorktreesUseCase, ScratchUseCase, Phase, ConnectorCatalogUseCase, ExtensionsUseCase, PluginCatalogUseCase, SkillCatalogUseCase, LiveSessionHandle, LiveSessionId, LiveSessionView, ProbeSlot, SessionRegistry, SettingsUseCase};
use forge_entities::TranscriptView;
use forge_domain::{ChangeModeRef, ChangesViewPrefs, WorktreeRecord, ArchivedRef, Attachment, AvailableModel, Card, LineRange, EditorPrefs, EffortLevel, PermissionDecision, PermissionMode, ProjectSummary, SearchHit, SessionId, SessionSummary, SidebarPrefs, SlashCommand};
use forge_ports::{BrowserPort, FilePickerPort, FileVersion, FilesPort, FolderPickerPort, GitPort, McpLogPort, ProjectRepositoryPort, SessionRepositoryPort, TerminalPort, UsageStatsPort, WebViewEvent, WebViewPort, WebViewTabId};
use forge_ports::Surface;
use time::OffsetDateTime;

use crate::change_rows::{ChangeView, ChangesSession};
use crate::cards::{clamp_cards, Cards};
use crate::closed_sessions::ClosedSessions;
use crate::editor_tabs::EditorTabs;
use crate::files_store::{FilesStore, Restore};
use crate::dock::{DockState, DockStates, Panel};
use crate::folder_choice::{choose_folder, FolderChoice};
use crate::nav_history::NavHistory;
use crate::pane_tree::{Dir, Divider, PaneTree, Rect};
use crate::preview_host::PreviewHost;
use crate::preview_tabs::{normalize_url, PreviewTabId, PreviewTabs};
use crate::new_session::ModelChoice;
use crate::shortcut::{match_code, shortcut_allowed, shortcut_for, swallowed_behind_overlay, types_text, KeyPress, PickerKind, PreviewKey, Shortcut};
use crate::tool_activity::finished_tool_calls;
use crate::task_list_views::{TaskListView, TaskListViews};
use crate::transcript_views::TranscriptViews;
use crate::arrange::{arrange_with_origins, open_session_order_with_origins, ArrangedGroup};
use crate::search::{merge_search, recent_rows, SearchRow, RECENT_LIMIT};
use crate::sidebar::{archive_key_plan, cycle_target, ArchiveKey, SidebarEntry, nth_session, mark_archived, merge_sidebar, project_for_open, replace_project_sessions, EntryKey, LiveSummary, SidebarGroup};
use crate::worktree_view::{folder_choices, NewWorktree, RepoInfo, WorktreeTag};
use forge_shared::tr;

const AUTO_ARCHIVE_EVERY: std::time::Duration = std::time::Duration::from_secs(60 * 60);

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

/// How a past session opens: in the focused pane (the search palette, notices), as a click on its
/// sidebar row (adds a pane while split), or beside (⌥+click).
#[derive(Clone, Copy)]
enum PastOpen {
    Replace,
    Click,
    Beside,
}

/// What the page draws of the panes: one rectangle per pane in reading order, the dividers, and
/// whether the area holds the layout at the minimum sizes.
#[derive(Debug, Clone, PartialEq)]
pub struct PaneGeometry {
    pub rects: Vec<Rect>,
    pub dividers: Vec<Divider>,
    pub fits: bool,
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
    pub worktrees: Arc<WorktreesUseCase>,
    pub scratch: Arc<ScratchUseCase>,
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
    panes: Signal<PaneTree>,
    // What the rest of the app reads of `panes`, kept apart so a divider drag (which only changes the
    // ratio) does not rerun everything that watches the focused screen.
    active: Signal<Active>,
    shown: Signal<Vec<Active>>,
    focus: Signal<usize>,
    /// The size of the area right of the docked sidebar; the page reports it.
    main_size: Signal<(u32, u32)>,
    history: Signal<NavHistory<Active>>,
    notice: Signal<Option<String>>,
    new_folder: Signal<Option<PathBuf>>,
    new_no_folder: Signal<bool>,
    extra_folders: Signal<Vec<PathBuf>>,
    picking: Signal<bool>,
    probe: Signal<Arc<ProbeSlot>>,
    new_commands: Signal<Vec<SlashCommand>>,
    new_models: Signal<Vec<AvailableModel>>,
    transcript_views: Signal<TranscriptViews>,
    task_list_views: Signal<TaskListViews>,
    prefs: Signal<SidebarPrefs>,
    editor: Signal<EditorPrefs>,
    changes_view: Signal<ChangesViewPrefs>,
    changes_sessions: Signal<HashMap<LiveSessionId, ChangesSession>>,
    changes_modes: Signal<Vec<ChangeModeRef>>,
    /// False until the startup `reload_prefs` has set `changes_modes`: saving before that would
    /// write a list that holds only the new entry and drop the others.
    changes_modes_loaded: Signal<bool>,
    archived: Signal<Vec<ArchivedRef>>,
    worktrees: Signal<Vec<WorktreeRecord>>,
    environments: Signal<Vec<forge_domain::Environment>>,
    environment_key: Signal<Option<crate::EnvironmentKey>>,
    environment_request: Signal<Option<crate::EnvironmentRequest>>,
    new_repo: Signal<Option<RepoInfo>>,
    new_unsent: Signal<Option<String>>,
    docks: Signal<DockStates>,
    file_requests: Signal<HashMap<LiveSessionId, String>>,
    previews: Signal<PreviewHost>,
    preview_surface: Signal<Option<Surface>>,
    overlay_open: Signal<bool>,
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
    picker_request: Signal<Option<PickerKind>>,
    changes_list_request: Signal<bool>,
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
            panes: Signal::new(PaneTree::new(Active::NewSession)),
            active: Signal::new(Active::NewSession),
            shown: Signal::new(vec![Active::NewSession]),
            focus: Signal::new(0),
            main_size: Signal::new((1200, 800)),
            history: Signal::new(NavHistory::new(Active::NewSession)),
            notice: Signal::new(None),
            new_folder: Signal::new(None),
            new_no_folder: Signal::new(false),
            extra_folders: Signal::new(Vec::new()),
            picking: Signal::new(false),
            probe: Signal::new(slot),
            new_commands,
            new_models,
            transcript_views: Signal::new(TranscriptViews::default()),
            task_list_views: Signal::new(TaskListViews::default()),
            prefs: Signal::new(SidebarPrefs::default()),
            editor: Signal::new(EditorPrefs::default()),
            changes_view: Signal::new(ChangesViewPrefs::default()),
            changes_sessions: Signal::new(HashMap::new()),
            changes_modes: Signal::new(Vec::new()),
            changes_modes_loaded: Signal::new(false),
            archived: Signal::new(Vec::new()),
            worktrees: Signal::new(Vec::new()),
            environments: Signal::new(Vec::new()),
            environment_key: Signal::new(None),
            environment_request: Signal::new(None),
            new_repo: Signal::new(None),
            new_unsent: Signal::new(None),
            docks: Signal::new(DockStates::default()),
            file_requests: Signal::new(HashMap::new()),
            previews: Signal::new(PreviewHost::default()),
            preview_surface: Signal::new(None),
            overlay_open: Signal::new(false),
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
            picker_request: Signal::new(None),
            changes_list_request: Signal::new(false),
        }
    });
    use_context_provider(move || workspace);
    use_hook(move || workspace.reload_past());
    use_hook(move || workspace.reload_prefs());
    use_hook(move || workspace.start_auto_archive());
    workspace
}

pub fn use_workspace() -> Workspace {
    use_context::<Workspace>()
}

/// Where the new-session slash commands are probed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbePlace {
    ChosenFolder,
    ScratchRoot,
}

/// Whether archiving a session should try to remove its folder: only a scratch folder, and only when no
/// other open or un-archived session lives in it.
pub(crate) fn should_release_scratch(is_scratch: bool, in_use: bool) -> bool {
    is_scratch && !in_use
}

/// The scratch root is probed only while `폴더 없음` is chosen, no folder is, and a root exists.
pub(crate) fn probe_in_scratch_root(chosen: Option<&Path>, no_folder: bool, scratch_available: bool) -> ProbePlace {
    if chosen.is_none() && no_folder && scratch_available {
        ProbePlace::ScratchRoot
    } else {
        ProbePlace::ChosenFolder
    }
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

    /// Archives the sessions that have been idle longer than the setting says; nothing when it is off.
    /// Each one goes through `archive_session`, so a worktree follows the same rule as a hand archive.
    pub fn auto_archive_now(&self, now_ms: i64) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            let Ok(config) = services.settings.load().await else { return };
            let Some(days) = config.auto_archive_days else { return };
            // Every open session is busy: its listed time is only re-read on close, so it is not idle.
            let busy: Vec<SessionId> = this.live.peek().iter().filter_map(|entry| entry.view.peek().claude_session_id.clone()).collect();
            let rows = crate::worktree_view::candidates(&this.past.peek(), &this.archived.peek(), &busy);
            let due: Vec<(PathBuf, SessionId)> = forge_domain::sessions_to_archive(now_ms, days, &rows)
                .into_iter()
                .map(|row| (row.project.clone(), SessionId(row.session_id.clone())))
                .collect();
            if due.is_empty() {
                return;
            }
            let count = due.len();
            for (project, session) in due {
                this.archive_session(project, session);
            }
            this.report(tr!("Inactive sessions archived: {count}", count = count));
        });
    }

    // Once the lists have loaded, and then every hour while Forge is open.
    fn start_auto_archive(&self) {
        let this = *self;
        dioxus::core::spawn_forever(async move {
            // The sidebar lists load first (`reload_past`); a first pass over an empty list does nothing.
            tokio::time::sleep(std::time::Duration::from_secs(20)).await;
            loop {
                let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
                this.auto_archive_now(now);
                tokio::time::sleep(AUTO_ARCHIVE_EVERY).await;
            }
        });
    }

    /// How the Changes panel draws a diff (its `⋮` menu).
    pub fn changes_view_prefs(&self) -> ChangesViewPrefs {
        self.changes_view.read().clone()
    }

    /// Changes the Changes panel options and saves them.
    pub fn update_changes_view_prefs(&self, change: impl FnOnce(&mut ChangesViewPrefs)) {
        let mut view = self.changes_view;
        change(&mut view.write());
        let this = *self;
        let services = (*self.services.peek()).clone();
        let prefs = self.changes_view_prefs();
        // Root scope: the save must finish even if the menu that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_code_settings(move |config| config.changes_view = prefs).await {
                this.report(tr!("failed to save the changes view settings: {error}", error = error));
            }
        });
    }

    /// What the person chose in one session's Changes panel.
    pub fn changes_session(&self, id: LiveSessionId) -> ChangesSession {
        self.changes_sessions.read().get(&id).cloned().unwrap_or_default()
    }

    pub fn update_changes_session(&self, id: LiveSessionId, change: impl FnOnce(&mut ChangesSession)) {
        let mut sessions = self.changes_sessions;
        change(sessions.write().entry(id).or_default());
    }

    /// The project folder and Claude session id of an open session; `None` before its first
    /// message. Reads without subscribing, so a streamed chunk does not rerun the caller.
    fn session_ref(&self, id: LiveSessionId) -> Option<(PathBuf, String)> {
        self.live.peek().iter().find(|entry| entry.handle.id() == id).and_then(|entry| {
            let view = entry.view.peek();
            view.claude_session_id.as_ref().map(|session| (view.project_path.clone(), session.0.clone()))
        })
    }

    /// The Claude session id of an open session, subscribing to it: the Changes panel watches this
    /// so a mode chosen before the first message is saved once the id exists.
    pub fn claude_session_key(&self, id: LiveSessionId) -> Option<String> {
        self.live.read().iter().find(|entry| entry.handle.id() == id).and_then(|entry| entry.view.read().claude_session_id.as_ref().map(|session| session.0.clone()))
    }

    /// Whether the saved views have been read from the config, subscribing to it: the Changes
    /// panel restores a session's view when this turns true.
    pub fn change_modes_loaded(&self) -> bool {
        *self.changes_modes_loaded.read()
    }

    /// Puts back the view and base this session had when Forge last saved them, unless the person
    /// has already changed them in this run. Does nothing until the saved modes have loaded.
    pub fn restore_change_mode(&self, id: LiveSessionId) {
        if !*self.changes_modes_loaded.peek() {
            return;
        }
        let Some((project, session)) = self.session_ref(id) else { return };
        let Some((view, base)) = crate::change_rows::mode_of(&self.changes_modes.peek(), &project, &session) else { return };
        self.update_changes_session(id, |state| {
            if state.view == ChangeView::All && state.base.is_none() {
                state.view = view;
                state.base = base;
            }
        });
    }

    /// Saves this session's view and base (a no-op when they are what is already saved, and until
    /// the saved modes have loaded).
    pub fn remember_change_mode(&self, id: LiveSessionId) {
        if !*self.changes_modes_loaded.peek() {
            return;
        }
        let Some((project, session)) = self.session_ref(id) else { return };
        let state = self.changes_sessions.peek().get(&id).cloned().unwrap_or_default();
        let mut list = self.changes_modes.peek().clone();
        if crate::change_rows::upsert_mode(&mut list, &project, &session, &state.view, state.base.as_deref()) {
            let mut modes = self.changes_modes;
            modes.set(list);
            self.save_change_modes();
        }
    }

    fn save_change_modes(&self) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let list = self.changes_modes.peek().clone();
        // Root scope: the save must finish even if the panel that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            if let Err(error) = services.settings.save_code_settings(move |config| config.changes_modes = list).await {
                this.report(tr!("failed to save the changes view: {error}", error = error));
            }
        });
    }

    /// The branch a worktree of Forge's record started from; `None` for any other folder.
    pub fn worktree_base(&self, path: &Path) -> Option<String> {
        self.worktrees.read().iter().find(|record| record.path == path).map(|record| record.base_branch.clone())
    }

    // A config that cannot be read keeps the defaults; the settings screen reports that file's
    // problems, so the sidebar does not repeat them.
    fn reload_prefs(&self) {
        let services = (*self.services.peek()).clone();
        let mut prefs = self.prefs;
        let mut archived = self.archived;
        let mut worktrees = self.worktrees;
        let mut environments = self.environments;
        let mut editor = self.editor;
        let mut changes_view = self.changes_view;
        let mut changes_modes = self.changes_modes;
        let mut changes_modes_loaded = self.changes_modes_loaded;
        spawn(async move {
            if let Ok(config) = services.settings.load().await {
                prefs.set(config.sidebar);
                editor.set(config.editor);
                changes_view.set(config.changes_view);
                archived.set(config.archived);
                changes_modes.set(config.changes_modes);
                changes_modes_loaded.set(true);
                worktrees.set(config.worktrees);
                environments.set(config.environments);
            }
        });
    }

    /// The sidebar as drawn: sorted, grouped and collapsed as the preferences say.
    pub fn arranged_sidebar(&self, now: OffsetDateTime) -> Vec<ArrangedGroup> {
        let active = match self.active() {
            Active::Live(id) => Some(id),
            Active::NewSession | Active::Customize | Active::Routines => None,
        };
        let records = self.worktrees.read().clone();
        let scratch_root = self.scratch_root();
        arrange_with_origins(&self.sidebar(), &self.prefs.read(), now, active, &|path| forge_domain::origin_for_grouping(&records, scratch_root.as_deref(), path))
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
        let records = self.worktrees.read().clone();
        let scratch_root = self.scratch_root();
        let past = self.past.read();
        let listed: Vec<ProjectSummary> = past
            .iter()
            .filter(|(project, _)| project.exists && !self.is_scratch_project(&project.path))
            .map(|(project, _)| project.clone())
            .collect();
        let mut projects = folder_choices(listed, &|path| forge_domain::origin_for_grouping(&records, scratch_root.as_deref(), path));
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

    /// The folder a session's project belongs to in the sidebar and the header: the repository of
    /// a worktree, else the project itself.
    pub fn origin_of(&self, path: &Path) -> PathBuf {
        forge_domain::origin_of(&self.worktrees.read(), path)
    }

    /// `Some` for a session in a worktree (one Forge made, or one at `<repo>/.claude/worktrees/<name>`).
    pub fn worktree_tag(&self, path: &Path) -> Option<WorktreeTag> {
        if let Some(record) = self.worktrees.read().iter().find(|record| record.path == path) {
            return Some(WorktreeTag { branch: Some(record.branch.clone()) });
        }
        forge_domain::main_repo_of(path).map(|_| WorktreeTag { branch: None })
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
                    let mut modes = this.changes_modes;
                    if crate::change_rows::forget_mode(&mut modes.write(), &project, &session.0) {
                        this.save_change_modes();
                    }
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

    /// Hides a session from the default list. Nothing of the transcript changes; when it was the last
    /// session of a worktree Forge made, the worktree is removed if it holds no work.
    pub fn archive_session(&self, project: PathBuf, session: SessionId) {
        self.set_archived(project.clone(), session.clone(), true);
        self.release_worktree(project.clone(), session.clone());
        self.release_scratch(project, session);
    }

    /// When the archived session was the last one in a scratch folder Forge made, the folder is removed
    /// if it is empty (the use case keeps one that holds anything).
    fn release_scratch(&self, path: PathBuf, session: SessionId) {
        if !self.is_scratch_project(&path) {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: the removal must finish even if the row that asked for it unmounts.
        dioxus::core::spawn_forever(async move {
            let live: Vec<(PathBuf, Option<SessionId>)> = this
                .live
                .peek()
                .iter()
                .map(|entry| {
                    let view = entry.view.peek();
                    (view.project_path.clone(), view.claude_session_id.clone())
                })
                .collect();
            let in_use = crate::worktree_view::worktree_in_use(&path, &session, &this.past.peek(), &this.archived.peek(), &live);
            if !should_release_scratch(true, in_use) {
                return;
            }
            let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
            match services.scratch.release(&path).await {
                Ok(forge_application::ScratchRelease::Removed) => this.reload_past(),
                Ok(_) => {}
                Err(error) => this.report(tr!("Could not remove the folder {name}: {error}", name = name, error = error)),
            }
        });
    }

    fn release_worktree(&self, path: PathBuf, session: SessionId) {
        if !self.worktrees.peek().iter().any(|record| record.path == path) {
            return;
        }
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: the removal must finish even if the row that asked for it unmounts. It starts
        // after the caller returns, so a session the caller closes right after archiving is closed by then.
        dioxus::core::spawn_forever(async move {
            let live: Vec<(PathBuf, Option<SessionId>)> = this
                .live
                .peek()
                .iter()
                .map(|entry| {
                    let view = entry.view.peek();
                    (view.project_path.clone(), view.claude_session_id.clone())
                })
                .collect();
            if crate::worktree_view::worktree_in_use(&path, &session, &this.past.peek(), &this.archived.peek(), &live) {
                return;
            }
            let name = path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default();
            match services.worktrees.release(&path).await {
                Ok(forge_application::Release::Removed) => {
                    let mut records = this.worktrees;
                    records.write().retain(|record| record.path != path);
                    // A custom-location worktree was a folder of its own in the list.
                    this.reload_past();
                }
                Ok(_) => {}
                Err(error) => this.report(tr!("Could not remove the worktree {name}: {error}", name = name, error = error)),
            }
        });
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

    /// The Background tasks panel's state for the open session.
    pub fn task_list_view(&self, id: LiveSessionId) -> TaskListView {
        self.task_list_views.read().get(id)
    }

    pub fn toggle_finished_tasks(&self, id: LiveSessionId) {
        let mut views = self.task_list_views;
        views.write().toggle_finished(id);
    }

    /// Hides the finished cards with these ids (the trash icon).
    pub fn clear_finished_tasks(&self, id: LiveSessionId, task_ids: Vec<String>) {
        let mut views = self.task_list_views;
        views.write().clear(id, task_ids);
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

    /// What the menu under the active session's title offers (`None` when no session is active).
    pub fn active_title_menu(&self) -> Option<crate::sidebar::RowActions> {
        self.active_entry().map(|(_, entry)| entry.title_menu_actions())
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
        if self.new_no_folder() {
            return None;
        }
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

    /// Probes the new-session screen's slash commands and models: in the chosen folder, or in the
    /// scratch root while `폴더 없음` is chosen. Call it inside the Dioxus runtime, from a `spawn`.
    pub async fn probe_new_commands(&self, no_folder: bool) {
        let services = (*self.services.peek()).clone();
        let chosen = self.chosen_new_folder();
        if probe_in_scratch_root(chosen.as_deref(), no_folder, services.scratch.available()) == ProbePlace::ChosenFolder {
            self.probe_new_folder(chosen);
            return;
        }
        let root = match services.scratch.ensure_root().await {
            Ok(root) => root,
            Err(_) => {
                self.probe_new_folder(None);
                return;
            }
        };
        // The person may have chosen a folder while the root was being made.
        if !self.new_no_folder() {
            return;
        }
        let project = ProjectSummary { path: root, last_active: String::new(), session_count: 0, exists: true };
        let slot = (*self.probe.peek()).clone();
        slot.set_folder(Some(&project));
    }

    /// The branches of the chosen folder when it is a git repository (and the probe has answered).
    pub fn new_repo(&self) -> Option<RepoInfo> {
        let chosen = self.chosen_new_folder()?;
        self.new_repo.read().clone().filter(|info| info.folder == chosen)
    }

    /// Reads the branches of `folder` for the new-session panel; `None` forgets them. The same
    /// folder keeps its answer.
    pub fn probe_new_repo(&self, folder: Option<PathBuf>) {
        let mut repo = self.new_repo;
        let Some(folder) = folder else {
            repo.set(None);
            return;
        };
        if repo.peek().as_ref().is_some_and(|info| info.folder == folder) {
            return;
        }
        repo.set(None);
        let this = *self;
        let services = (*self.services.peek()).clone();
        spawn(async move {
            let found = services.worktrees.branches(folder.clone()).await.ok();
            // The person may have chosen another folder while git ran.
            if this.chosen_new_folder().as_deref() != Some(folder.as_path()) {
                return;
            }
            repo.set(found.map(|branches| RepoInfo { folder, branches }));
        });
    }

    /// The message of a send whose worktree could not be made, until the panel takes it back.
    pub fn new_unsent(&self) -> Option<String> {
        self.new_unsent.read().clone()
    }

    pub fn clear_new_unsent(&self) {
        let mut unsent = self.new_unsent;
        unsent.set(None);
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
            Active::NewSession | Active::Customize | Active::Routines if self.new_no_folder() => None,
            Active::NewSession | Active::Customize | Active::Routines => self
                .new_folder()
                .or_else(|| self.new_session_projects().into_iter().next().map(|project| project.path)),
        }
    }

    pub fn set_new_folder(&self, folder: Option<PathBuf>) {
        if folder.is_some() {
            self.set_new_no_folder(false);
        }
        let mut current = self.new_folder;
        current.set(folder);
    }

    /// Whether `폴더 없음` is chosen on the new-session screen.
    pub fn new_no_folder(&self) -> bool {
        *self.new_no_folder.read()
    }

    pub fn set_new_no_folder(&self, on: bool) {
        let mut current = self.new_no_folder;
        current.set(on);
    }

    /// Whether a no-folder session can be made (a scratch folder root is known).
    pub fn scratch_available(&self) -> bool {
        self.services.peek().scratch.available()
    }

    /// The scratch folder root, when one is known.
    pub fn scratch_root(&self) -> Option<PathBuf> {
        self.services.peek().scratch.root().map(Path::to_path_buf)
    }

    /// Whether `path` is one of Forge's scratch folders (a session started without a folder).
    pub fn is_scratch_project(&self, path: &Path) -> bool {
        self.services.peek().scratch.root().is_some_and(|root| forge_domain::is_scratch(root, path))
    }

    /// Whether `path` is the scratch root or a scratch folder: the one `This computer` group.
    pub fn is_scratch_group(&self, path: &Path) -> bool {
        self.services.peek().scratch.root().is_some_and(|root| forge_domain::is_scratch_group(root, path))
    }

    /// Shows the new-session panel with `폴더 없음` chosen (the `+` beside `이 컴퓨터`).
    pub fn show_new_no_folder_session(&self) {
        self.set_new_folder(None);
        self.set_new_no_folder(true);
        self.show_new_session();
    }

    /// Shows the new-session panel with `folder` preselected (the `+` beside a project).
    pub fn show_new_session_in(&self, folder: PathBuf) {
        self.set_new_folder(Some(folder));
        self.show_new_session();
    }

    /// The screens of the panes in reading order (one to four).
    pub fn pane_targets(&self) -> Vec<Active> {
        self.shown.read().clone()
    }

    /// The screen of pane `index` without subscribing (for tasks that answer a click).
    pub fn pane_target(&self, index: usize) -> Option<Active> {
        self.panes.peek().target(index)
    }

    pub fn focused_pane(&self) -> usize {
        *self.focus.read()
    }

    // Changes the panes and brings the derived signals up to date, writing only those that changed.
    fn edit_panes<R>(&self, change: impl FnOnce(&mut PaneTree) -> R) -> R {
        let mut panes = self.panes;
        let result = change(&mut panes.write());
        let (target, targets, focused) = {
            let now = self.panes.peek();
            (now.focused_target(), now.targets(), now.focused())
        };
        set_if_changed(self.active, target);
        set_if_changed(self.shown, targets);
        set_if_changed(self.focus, focused);
        result
    }

    fn visit_active(&self) {
        let mut history = self.history;
        history.write().visit(self.active());
    }

    /// A click or focus inside a pane. Writes only when the focus moves, so a click in the focused
    /// pane redraws nothing. The move is a stop of the history, so Back returns to the other pane.
    pub fn focus_pane(&self, index: usize) {
        if *self.focus.peek() == index {
            return;
        }
        if self.edit_panes(|panes| panes.focus(index)) {
            self.clear_rename_request();
            self.visit_active();
        }
    }

    /// A plain click on a session row: with one pane it replaces that pane (as always); while split it
    /// adds a pane, like the desktop app.
    pub fn open_session(&self, id: LiveSessionId) {
        if self.panes.peek().is_split() {
            self.add_session(id);
        } else {
            self.select(id);
        }
    }

    /// ⌥+click on a session row: it gets a pane of its own (or takes focus if it is shown).
    pub fn add_session(&self, id: LiveSessionId) {
        self.clear_rename_request();
        let (w, h) = self.main_size();
        let target = Active::Live(id);
        self.edit_panes(|panes| panes.add(target, w, h));
        self.after_pane_change(target);
    }

    /// `⌃⌘\` and `⌃⌘-`: a New-session pane beside or below the focused one.
    pub fn split_new_session(&self, dir: Dir) {
        self.clear_rename_request();
        let (w, h) = self.main_size();
        if self.edit_panes(|panes| panes.split_focused(dir, Active::NewSession, w, h)) {
            self.after_pane_change(Active::NewSession);
        }
    }

    // What `display` does for a screen that now has the focus: the lightbox goes, a routine's
    // session stays open, and the history records the screen (so Back returns to it).
    fn after_pane_change(&self, target: Active) {
        self.close_lightbox();
        if let Active::Live(id) = target {
            self.services.peek().routines.host.keep_open(id);
        }
        let mut history = self.history;
        history.write().visit(target);
    }

    pub fn close_pane(&self, index: usize) {
        self.edit_panes(|panes| panes.close(index));
        self.visit_active();
    }

    /// The `⋮` menu: move pane `index` along or across its split (see `PaneTree::move_pane`).
    pub fn move_pane(&self, index: usize, dir: Dir) {
        let (w, h) = self.main_size();
        self.edit_panes(|panes| panes.move_pane(index, dir, w, h));
    }

    /// Whether the move item is offered: a move that would fold the layout is left out.
    pub fn can_move_pane(&self, index: usize, dir: Dir) -> bool {
        let (w, h) = self.main_size();
        self.panes.read().can_move(index, dir, w, h)
    }

    /// The direction of the split pane `index` sits in and whether it is the second child.
    pub fn pane_slot(&self, index: usize) -> Option<(Dir, bool)> {
        self.panes.read().pane_slot(index)
    }

    /// The rectangles of the panes and the dividers in a main area `w` by `h`. Reads the tree, so a
    /// divider drag redraws.
    pub fn pane_geometry(&self, w: u32, h: u32) -> PaneGeometry {
        let panes = self.panes.read();
        PaneGeometry { rects: panes.rects(w, h), dividers: panes.dividers(w, h), fits: panes.fits(w, h) }
    }

    /// The rectangle the split at `path` cuts; what a drag of its divider measures the pointer in.
    pub fn pane_split_rect(&self, path: &[bool], w: u32, h: u32) -> Option<Rect> {
        self.panes.peek().split_rect(path, w, h)
    }

    /// The direction of the split at `path` now; none when the tree no longer has a split there.
    pub fn pane_split_dir(&self, path: &[bool]) -> Option<Dir> {
        self.panes.peek().split_dir(path)
    }

    /// A divider drag: `ratio` is the first child's share of the split at `path`.
    pub fn set_split_ratio(&self, path: &[bool], ratio: f32) {
        let (w, h) = self.main_size();
        self.edit_panes(|panes| panes.set_ratio(path, ratio, w, h));
    }

    /// The size of the area right of the docked sidebar; the page reports it.
    pub fn set_main_size(&self, width: u32, height: u32) {
        set_if_changed(self.main_size, (width, height));
    }

    pub fn main_size(&self) -> (u32, u32) {
        *self.main_size.peek()
    }

    // The first message of a New-session pane turns that very pane into the session, even if the
    // person has clicked into the other pane since.
    fn show_started(&self, id: LiveSessionId) {
        let replaced = self.edit_panes(|panes| panes.replace(Active::NewSession, Active::Live(id)));
        if !replaced {
            self.select(id);
        } else if self.active() == Active::Live(id) {
            self.after_pane_change(Active::Live(id));
        }
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
        self.edit_panes(|panes| panes.show(to));
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
        self.open_past_in(project, session_id, PastOpen::Replace);
    }

    /// A plain click on a past session's row: like `open_session`, it adds a pane while split.
    pub fn open_past_clicked(&self, project: ProjectSummary, session_id: SessionId) {
        self.open_past_in(project, session_id, PastOpen::Click);
    }

    /// The same, in a pane of its own (⌥+click).
    pub fn add_past(&self, project: ProjectSummary, session_id: SessionId) {
        self.open_past_in(project, session_id, PastOpen::Beside);
    }

    fn open_past_in(&self, project: ProjectSummary, session_id: SessionId, how: PastOpen) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        // Root scope: the search palette closes the moment it opens a session, and a task of the
        // palette's own scope would be cancelled with it before the session ever opened.
        dioxus::core::spawn_forever(async move {
            match services.registry.open_existing(&project, session_id).await {
                Ok(outcome) => {
                    let id = this.ensure_attached(outcome.handle);
                    match how {
                        PastOpen::Replace => this.select(id),
                        PastOpen::Click => this.open_session(id),
                        PastOpen::Beside => this.add_session(id),
                    }
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
        worktree: Option<NewWorktree>,
        environment: Option<crate::EnvironmentStart>,
        scratch: bool,
    ) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        let slot = (*self.probe.peek()).clone();
        // Pinned to the root scope: `NewSessionComposer` unmounts if the person clicks elsewhere
        // while the saved model is being read, which would cancel a plain `spawn`.
        dioxus::core::spawn_forever(async move {
            // An environment names its worktree choice, not a probed repository: ask git now, and start
            // in the folder itself when it is not a repository.
            let worktree = match (&environment, worktree) {
                (Some(_), Some(request)) => services.worktrees.branches(project.path.clone()).await.ok().map(|_| request),
                (_, other) => other,
            };
            let mut first_made: Option<PathBuf> = None;
            let project = match worktree {
                None => project,
                Some(request) => {
                    let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
                    let created_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
                    match services.worktrees.create(&project.path, request.base, &text, seed, created_at).await {
                        Ok(record) => {
                            let mut records = this.worktrees;
                            records.write().push(record.clone());
                            first_made = Some(record.path.clone());
                            ProjectSummary { path: record.path, last_active: String::new(), session_count: 0, exists: true }
                        }
                        Err(error) => {
                            this.report(tr!("Could not create the worktree: {error}", error = crate::worktree_view::creation_error_text(&error)));
                            let mut unsent = this.new_unsent;
                            unsent.set(Some(text));
                            return;
                        }
                    }
                }
            };
            // The extra folders that asked for a worktree get one; a folder that is not a repository stays as it is.
            let mut made: Vec<Option<PathBuf>> = Vec::new();
            if let Some(environment) = &environment {
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
                let created_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
                for (index, folder) in environment.extra.iter().enumerate() {
                    let Some(Some(request)) = environment.extra_worktrees.get(index) else {
                        made.push(None);
                        continue;
                    };
                    if services.worktrees.branches(folder.clone()).await.is_err() {
                        made.push(None);
                        continue;
                    }
                    match services.worktrees.create(folder, request.base.clone(), &text, seed.wrapping_add(index as u64 + 1), created_at).await {
                        Ok(record) => {
                            let mut records = this.worktrees;
                            records.write().push(record.clone());
                            made.push(Some(record.path));
                        }
                        Err(error) => {
                            let notice = tr!("Could not create the worktree: {error}", error = crate::worktree_view::creation_error_text(&error));
                            this.abandon_start(&services, crate::environment::worktrees_made(first_made.as_ref(), &made), None, notice, Some(text)).await;
                            return;
                        }
                    }
                }
            }
            let model = match model {
                Some(chosen) => chosen,
                None => match services.settings.saved_model().await {
                    Ok(model) => model,
                    Err(error) => {
                        let notice = tr!("failed to load the Forge config: {error}", error = error);
                        this.abandon_start(&services, crate::environment::worktrees_made(first_made.as_ref(), &made), None, notice, None).await;
                        return;
                    }
                },
            };
            // Made as late as possible so an earlier failure leaves nothing behind; the folder is empty,
            // so any failure after this releases it again.
            let mut scratch_made: Option<PathBuf> = None;
            let project = if scratch && environment.is_none() {
                // UTC date: the local offset is not at hand here.
                let now = time::OffsetDateTime::now_utc();
                let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_nanos() as u64).unwrap_or(0);
                match services.scratch.create(now.year(), now.month() as u8, now.day(), seed).await {
                    Ok(path) => {
                        scratch_made = Some(path.clone());
                        ProjectSummary { path, last_active: String::new(), session_count: 0, exists: true }
                    }
                    Err(error) => {
                        let notice = tr!("Could not make a folder for this session: {error}", error = error);
                        this.abandon_start(&services, crate::environment::worktrees_made(first_made.as_ref(), &made), None, notice, Some(text)).await;
                        return;
                    }
                }
            } else {
                project
            };
            let commands = slot.end();
            match services.registry.open_new_with_effort(&project, mode, model, effort, commands) {
                Ok(handle) => {
                    let id = this.attach(handle.clone());
                    let mut folders = folders;
                    if let Some(environment) = &environment {
                        folders = crate::environment::merge_added_dirs(folders, &environment.extra, &made);
                        if !environment.env.0.is_empty() {
                            handle.set_env(environment.env.clone());
                        }
                    }
                    if !folders.is_empty() {
                        handle.add_directories(folders);
                    }
                    handle.send_with(text, attachments);
                    this.show_started(id);
                }
                Err(error) => {
                    this.abandon_start(&services, crate::environment::worktrees_made(first_made.as_ref(), &made), scratch_made, error.to_string(), None).await;
                }
            }
        });
    }

    /// A send that failed after it made worktrees: removes them (they are new and clean), drops their
    /// records, and shows the notice, naming any worktree that could not be removed. `unsent` is the
    /// text to give back to the composer.
    async fn abandon_start(&self, services: &WorkspaceServices, made: Vec<PathBuf>, scratch: Option<PathBuf>, notice: String, unsent: Option<String>) {
        if let Some(path) = scratch {
            // Empty and made for this send only; a folder that cannot be removed is harmless.
            let _ = services.scratch.release(&path).await;
        }
        let mut left: Vec<PathBuf> = Vec::new();
        for path in made {
            match services.worktrees.release(&path).await {
                Ok(forge_application::Release::Removed | forge_application::Release::NotTracked) => {
                    let mut records = self.worktrees;
                    records.write().retain(|record| record.path != path);
                }
                Ok(forge_application::Release::Kept(_)) | Err(_) => left.push(path),
            }
        }
        self.reload_past();
        self.report(crate::environment::failed_send_notice(notice, &left));
        if let Some(text) = unsent {
            let mut slot = self.new_unsent;
            slot.set(Some(text));
        }
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
        let mut task_lists = self.task_list_views;
        task_lists.write().forget(id);
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
        let mut changes = self.changes_sessions;
        crate::change_rows::forget_changes_session(&mut changes.write(), &id);
        // While split the pane that shows it closes; with one pane, the next open session (or the
        // New-session page) takes its place.
        if self.edit_panes(|panes| panes.remove_target(Active::Live(id))) {
            self.visit_active();
        } else if self.active() == Active::Live(id) {
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
        let records = self.worktrees.read().clone();
        let scratch_root = self.scratch_root();
        let order = open_session_order_with_origins(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local(), &|path| forge_domain::origin_for_grouping(&records, scratch_root.as_deref(), path));
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
        let records = self.worktrees.read().clone();
        let scratch_root = self.scratch_root();
        let order = open_session_order_with_origins(&self.sidebar(), &self.prefs.read(), crate::arrange::now_local(), &|path| forge_domain::origin_for_grouping(&records, scratch_root.as_deref(), path));
        if let Some(id) = nth_session(&order, index) {
            self.select(id);
        }
    }

    /// The new-session page with the active session's folder chosen (just the page when no session is active).
    pub fn new_session_here(&self) {
        if let Active::Live(id) = self.active() {
            if let Some(view) = self.live_view(id) {
                let folder = view.read().project_path.clone();
                if self.is_scratch_group(&folder) {
                    return self.show_new_no_folder_session();
                }
                return self.show_new_session_in(self.origin_of(&folder));
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

    /// Asks the composer menu of `kind` on screen to open. It opens only if a menu of that kind is
    /// there (an open session, or the new-session screen).
    pub fn request_picker(&self, kind: PickerKind) {
        if matches!(self.active(), Active::Live(_) | Active::NewSession) {
            let mut request = self.picker_request;
            request.set(Some(kind));
        }
    }

    pub fn picker_request(&self) -> Option<PickerKind> {
        *self.picker_request.read()
    }

    pub fn clear_picker_request(&self) {
        let mut request = self.picker_request;
        if request.peek().is_some() {
            request.set(None);
        }
    }

    pub fn environments(&self) -> Vec<forge_domain::Environment> {
        self.environments.read().clone()
    }

    /// Adds the environment, or replaces the one with its id. Pinned to the root scope: the save must
    /// finish even if the dialog goes away meanwhile.
    pub fn save_environment(&self, environment: forge_domain::Environment) {
        self.change_environments(move |list| match list.iter_mut().find(|existing| existing.id == environment.id) {
            Some(existing) => *existing = environment,
            None => list.push(environment),
        });
    }

    pub fn delete_environment(&self, id: String) {
        self.change_environments(move |list| list.retain(|environment| environment.id != id));
    }

    pub fn set_environment_pinned(&self, id: String, pinned: bool) {
        self.change_environments(move |list| {
            if let Some(environment) = list.iter_mut().find(|environment| environment.id == id) {
                environment.pinned = pinned;
            }
        });
    }

    fn change_environments(&self, change: impl FnOnce(&mut Vec<forge_domain::Environment>) + 'static) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            match services.settings.save_code_settings(|config| change(&mut config.environments)).await {
                Ok(config) => {
                    let mut environments = this.environments;
                    environments.set(config.environments);
                }
                Err(error) => this.report(tr!("Could not save the environment: {error}", error = error)),
            }
        });
    }

    /// Asks the new-session screen's environment chip to act on a key (⌥⌘0…9). Nothing else shows one.
    pub fn request_environment_key(&self, key: crate::EnvironmentKey) {
        if self.active() == Active::NewSession && self.environment_request.peek().is_none() {
            let mut request = self.environment_key;
            request.set(Some(key));
        }
    }

    pub fn environment_key(&self) -> Option<crate::EnvironmentKey> {
        *self.environment_key.read()
    }

    pub fn clear_environment_key(&self) {
        let mut request = self.environment_key;
        if request.peek().is_some() {
            request.set(None);
        }
    }

    pub fn environment_request(&self) -> Option<crate::EnvironmentRequest> {
        self.environment_request.read().clone()
    }

    pub fn open_environment_dialog(&self, request: crate::EnvironmentRequest) {
        let mut current = self.environment_request;
        current.set(Some(request));
    }

    pub fn close_environment_dialog(&self) {
        let mut current = self.environment_request;
        current.set(None);
    }

    /// The local branches of `folder`; `None` when it is not a git repository.
    pub async fn folder_branches(&self, folder: PathBuf) -> Option<forge_ports::BranchList> {
        let services = (*self.services.peek()).clone();
        services.worktrees.branches(folder).await.ok()
    }

    /// The system folder dialog; `None` when cancelled.
    pub async fn pick_folder(&self) -> Option<PathBuf> {
        let services = (*self.services.peek()).clone();
        services.folder_picker.pick_folder().await
    }

    /// Whether `folder` is off the trusted list, so the person must be asked. A list that cannot be
    /// read counts as empty, like the composer's add-folder.
    pub async fn needs_trust(&self, folder: PathBuf) -> bool {
        let services = (*self.services.peek()).clone();
        let trusted = services.settings.load().await.map(|config| config.trusted_folders).unwrap_or_default();
        crate::run_blocking(move || crate::trust::classify_added_folder(&trusted, &[], &folder, &crate::trust::resolved(&folder)) == crate::trust::FolderAdd::NeedsAsking).await.unwrap_or(true)
    }

    /// Puts `folder` on the trusted list.
    pub fn trust_folder(&self, folder: PathBuf) {
        let this = *self;
        let services = (*self.services.peek()).clone();
        dioxus::core::spawn_forever(async move {
            let Some(resolved) = crate::run_blocking(move || crate::trust::resolved(&folder)).await else { return };
            let saved = services.settings.save_code_settings(|config| {
                forge_domain::add_trusted(&mut config.trusted_folders, resolved);
            });
            if let Err(error) = saved.await {
                this.report(tr!("Could not save the trusted folder: {error}", error = error));
            }
        });
    }

    /// Marks the environment as used now, which is what orders the menu.
    pub fn touch_environment(&self, id: String) {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0);
        self.change_environments(move |list| {
            if let Some(environment) = list.iter_mut().find(|environment| environment.id == id) {
                environment.last_used_at = now;
            }
        });
    }


    /// Steps the active session's transcript view (Normal, Thinking, Detailed, Normal…).
    pub fn cycle_transcript_view(&self) {
        if let Active::Live(id) = self.active() {
            // The page provides the settings to every handler; without them the plain default holds.
            let default = try_consume_context::<crate::Settings>().map(|settings| settings.default_transcript_view()).unwrap_or_default();
            let next = self.transcript_view(id, default).next();
            self.set_transcript_view(id, next);
        }
    }

    /// Shows or hides the file list of the Files panel, or of Changes when Files is shut.
    pub fn toggle_file_list(&self) {
        let Active::Live(id) = self.active() else { return };
        match self.dock(id).file_list_panel() {
            Some(Panel::Files) => self.update_editor_prefs(|prefs| prefs.tree = !prefs.tree),
            Some(Panel::Changes) => {
                let mut request = self.changes_list_request;
                request.set(true);
            }
            _ => {}
        }
    }

    pub fn changes_list_request(&self) -> bool {
        *self.changes_list_request.read()
    }

    pub fn clear_changes_list_request(&self) {
        let mut request = self.changes_list_request;
        if *request.peek() {
            request.set(false);
        }
    }

    /// Expands the open panel of the active session, or puts an expanded one back.
    pub fn toggle_panel_expanded(&self) {
        if let Active::Live(id) = self.active() {
            self.change_dock(id, |dock| dock.toggle_expanded());
        }
    }

    /// A key of the Preview panel of session `id`: its tabs, address bar and reload.
    pub fn preview_key(&self, id: LiveSessionId, key: PreviewKey) {
        match key {
            PreviewKey::NewTab => self.change_previews(id, |tabs| {
                tabs.open();
            }),
            PreviewKey::NextTab => self.change_previews(id, |tabs| tabs.step(true)),
            PreviewKey::PreviousTab => self.change_previews(id, |tabs| tabs.step(false)),
            PreviewKey::GoToTab(index) => self.change_previews(id, |tabs| tabs.select_index(index)),
            PreviewKey::Reload => {
                if let Some(tab) = self.previews(id).active() {
                    // A page that cannot be reloaded says so in its own notice; a key stays quiet.
                    let _ = self.preview_reload(id, tab);
                }
            }
            // The address field belongs to the panel, which watches for this (`PreviewPanel`).
            PreviewKey::FocusAddress => {}
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
            Shortcut::NewSessionBeside => self.split_new_session(Dir::Beside),
            Shortcut::NewSessionBelow => self.split_new_session(Dir::Below),
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
            Shortcut::OpenPicker(kind) => self.request_picker(kind),
            Shortcut::PickEnvironment(index) => self.request_environment_key(crate::EnvironmentKey::Pick(index)),
            Shortcut::NewEnvironment => self.request_environment_key(crate::EnvironmentKey::New),
            Shortcut::CycleTranscript => self.cycle_transcript_view(),
            Shortcut::ToggleFileList => self.toggle_file_list(),
            Shortcut::ExpandPanel => self.toggle_panel_expanded(),
            // The composer handles ⌘U itself, and the Preview panel its own keys.
            Shortcut::AddFiles | Shortcut::Preview(_) => return false,
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

/// The Preview panel key a key press is, if any. Only the panel's own handler asks.
pub fn preview_key_from_event(event: &KeyboardEvent) -> Option<PreviewKey> {
    let modifiers = event.modifiers();
    let key = event.key().to_string();
    let code = match_code(&key, &event.code().to_string(), modifiers.alt());
    crate::shortcut::preview_shortcut(
        &KeyPress { code: &code, ctrl: modifiers.ctrl(), meta: modifiers.meta(), alt: modifiers.alt(), shift: modifiers.shift() },
        cfg!(target_os = "macos"),
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

fn set_if_changed<T: PartialEq + 'static>(mut slot: Signal<T>, value: T) {
    if *slot.peek() != value {
        slot.set(value);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{probe_in_scratch_root, should_release_scratch, ProbePlace};

    #[test]
    fn a_scratch_folder_is_released_only_when_no_other_session_uses_it() {
        assert!(should_release_scratch(true, false));
        assert!(!should_release_scratch(true, true));
        assert!(!should_release_scratch(false, false));
        assert!(!should_release_scratch(false, true));
    }

    #[test]
    fn the_scratch_root_is_probed_only_with_no_folder_chosen_and_a_root() {
        let folder = Path::new("/p");
        assert_eq!(probe_in_scratch_root(None, true, true), ProbePlace::ScratchRoot);
        assert_eq!(probe_in_scratch_root(Some(folder), true, true), ProbePlace::ChosenFolder);
        assert_eq!(probe_in_scratch_root(None, false, true), ProbePlace::ChosenFolder);
        assert_eq!(probe_in_scratch_root(None, true, false), ProbePlace::ChosenFolder);
    }

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
