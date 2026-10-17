use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{ExtensionsUseCase, SettingsUseCase};
use forge_domain::{AppConfig, Appearance, ChatWidth, FontChoice, Language, McpServerDefinition, MotionPref, NotificationPrefs, OutputStyle, PluginDefinition, SkillDefinition, TextSize, Theme, TranscriptView, WorktreeLocation};
use forge_ports::FolderPickerPort;
use forge_entities::{mcp_rows, plugin_rows, project_label, skill_rows, ExtensionRow};

use crate::section_status::status_for;
use crate::settings_draft::ConnectionDraft;
use crate::workspace::{shortcut_from_event, WorkspaceServices};
use crate::shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    General,
    Connection,
    Model,
    Skills,
    Plugins,
    McpServers,
    Code,
    Developer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionKind {
    Skill,
    Plugin,
    McpServer,
}

// What the last listing found; an error is shown in its own tab only.
struct Catalog {
    skills: Result<Vec<SkillDefinition>, String>,
    plugins: Result<Vec<PluginDefinition>, String>,
    mcp: Result<Vec<McpServerDefinition>, String>,
}

impl Default for Catalog {
    fn default() -> Self {
        Self { skills: Ok(Vec::new()), plugins: Ok(Vec::new()), mcp: Ok(Vec::new()) }
    }
}

/// What a Code tab control does to the stored config. One function per control keeps the setters
/// below one line each and the effect testable without a runtime.
pub(crate) fn apply_code_change(config: &mut AppConfig, change: CodeChange) {
    match change {
        CodeChange::DefaultView(view) => config.default_transcript_view = view,
        CodeChange::OutputStyle(style) => config.output_style = style,
        CodeChange::PromptSuggestions(on) => config.prompt_suggestions = on,
        CodeChange::AllowBypass(on) => config.allow_bypass_mode = on,
        CodeChange::DynamicWorkflows(on) => config.dynamic_workflows = on,
        CodeChange::WorktreeLocation(value) => config.worktree_location = value,
        CodeChange::Theme(value) => config.appearance.theme = value,
        CodeChange::Font(value) => config.appearance.font = value,
        CodeChange::TextSize(value) => config.appearance.text_size = value,
        CodeChange::ChatWidth(value) => config.appearance.chat_width = value,
        CodeChange::Motion(value) => config.appearance.motion = value,
        CodeChange::Notification(kind, on) => match kind {
            NotificationKind::ReplyDone => config.notifications.reply_done = on,
            NotificationKind::ScheduledTask => config.notifications.scheduled_task = on,
            NotificationKind::CodeNotice => config.notifications.code_notice = on,
            NotificationKind::PermissionRequest => config.notifications.permission_request = on,
        },
        CodeChange::AttentionBounce(on) => config.attention_bounce = on,
        CodeChange::Language(value) => config.language = Some(value),
        CodeChange::TrustFolder(path) => {
            forge_domain::add_trusted(&mut config.trusted_folders, path);
        }
        CodeChange::UntrustFolder(path) => config.trusted_folders.retain(|entry| *entry != path),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CodeChange {
    DefaultView(TranscriptView),
    OutputStyle(Option<OutputStyle>),
    PromptSuggestions(bool),
    AllowBypass(bool),
    DynamicWorkflows(bool),
    WorktreeLocation(WorktreeLocation),
    Theme(Theme),
    Font(FontChoice),
    TextSize(TextSize),
    ChatWidth(ChatWidth),
    Motion(MotionPref),
    Notification(NotificationKind, bool),
    AttentionBounce(bool),
    Language(Language),
    TrustFolder(PathBuf),
    UntrustFolder(PathBuf),
}

/// One of the four notification switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationKind {
    ReplyDone,
    ScheduledTask,
    CodeNotice,
    PermissionRequest,
}

/// The outcome of the last save, shown under the fields until the next edit or save.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsStatus {
    Saved,
    Error(String),
}

/// The settings overlay's state. `Copy` (every field is a signal), like `Workspace`.
#[derive(Clone, Copy)]
pub struct Settings {
    usecase: Signal<Arc<SettingsUseCase>>,
    open: Signal<bool>,
    section: Signal<SettingsSection>,
    search: Signal<String>,
    saved: Signal<AppConfig>,
    base_url: Signal<String>,
    api_key: Signal<String>,
    key_visible: Signal<bool>,
    status: Signal<Option<(SettingsSection, SettingsStatus)>>,
    extensions: Signal<Arc<ExtensionsUseCase>>,
    project: Signal<Option<PathBuf>>,
    catalog: Signal<Catalog>,
    /// The local MCP server the Developer page shows in detail.
    server: Signal<Option<String>>,
    folder_picker: Signal<Arc<dyn FolderPickerPort>>,
    trusted_dialog: Signal<bool>,
    trusted_picking: Signal<bool>,
    location_nonce: Signal<u32>,
}

/// Creates the settings state, provides it to every descendant and loads the saved config once.
/// Call it in the page before `use_workspace_provider` (the new-session panel reads the model).
pub fn use_settings_provider() -> Settings {
    let services = use_context::<WorkspaceServices>();
    let settings = use_hook(move || Settings {
        usecase: Signal::new(services.settings.clone()),
        open: Signal::new(false),
        section: Signal::new(SettingsSection::General),
        search: Signal::new(String::new()),
        saved: Signal::new(AppConfig::default()),
        base_url: Signal::new(String::new()),
        api_key: Signal::new(String::new()),
        key_visible: Signal::new(false),
        status: Signal::new(None),
        extensions: Signal::new(services.extensions.clone()),
        project: Signal::new(None),
        catalog: Signal::new(Catalog::default()),
        server: Signal::new(None),
        folder_picker: Signal::new(services.folder_picker.clone()),
        trusted_dialog: Signal::new(false),
        trusted_picking: Signal::new(false),
        location_nonce: Signal::new(0),
    });
    use_context_provider(move || settings);
    use_hook(move || settings.reload());
    settings
}

pub fn use_settings() -> Settings {
    use_context::<Settings>()
}

impl Settings {
    pub fn is_open(&self) -> bool {
        *self.open.read()
    }

    /// Opens the overlay with the fields showing what is stored (discarding earlier edits).
    pub fn open(&self) {
        let mut open = self.open;
        let mut status = self.status;
        let mut key_visible = self.key_visible;
        open.set(true);
        let mut search = self.search;
        search.set(String::new());
        status.set(None);
        key_visible.set(false);
        self.reload();
        self.reload_catalog();
    }

    pub fn close(&self) {
        let mut open = self.open;
        let mut dialog = self.trusted_dialog;
        open.set(false);
        dialog.set(false);
    }

    pub fn search(&self) -> String {
        self.search.read().clone()
    }

    pub fn set_search(&self, query: String) {
        let mut search = self.search;
        search.set(query);
    }

    pub fn section(&self) -> SettingsSection {
        *self.section.read()
    }

    pub fn set_section(&self, section: SettingsSection) {
        let mut current = self.section;
        current.set(section);
        self.clear_status();
    }

    pub fn base_url(&self) -> String {
        self.base_url.read().clone()
    }

    pub fn set_base_url(&self, value: String) {
        let mut field = self.base_url;
        field.set(value);
        self.clear_status();
    }

    pub fn api_key(&self) -> String {
        self.api_key.read().clone()
    }

    pub fn set_api_key(&self, value: String) {
        let mut field = self.api_key;
        field.set(value);
        self.clear_status();
    }

    pub fn key_visible(&self) -> bool {
        *self.key_visible.read()
    }

    pub fn toggle_key_visible(&self) {
        let mut visible = self.key_visible;
        let now = *visible.peek();
        visible.set(!now);
    }

    /// Whether the two Connection fields differ from what is stored (enables Save).
    pub fn connection_dirty(&self) -> bool {
        let draft = ConnectionDraft { base_url: self.base_url(), api_key: self.api_key() };
        draft.is_dirty(&self.saved.read())
    }

    /// The stored default model (`None`: the SDK chooses).
    pub fn model(&self) -> Option<String> {
        self.saved.read().model.clone()
    }

    pub fn default_transcript_view(&self) -> TranscriptView {
        self.saved.read().default_transcript_view
    }

    pub fn output_style(&self) -> Option<OutputStyle> {
        self.saved.read().output_style
    }

    pub fn prompt_suggestions(&self) -> bool {
        self.saved.read().prompt_suggestions
    }

    pub fn allow_bypass_mode(&self) -> bool {
        self.saved.read().allow_bypass_mode
    }

    pub fn dynamic_workflows(&self) -> bool {
        self.saved.read().dynamic_workflows
    }

    pub fn set_default_transcript_view(&self, view: TranscriptView) {
        self.change_code(CodeChange::DefaultView(view));
    }

    pub fn set_output_style(&self, style: Option<OutputStyle>) {
        self.change_code(CodeChange::OutputStyle(style));
    }

    pub fn set_prompt_suggestions(&self, on: bool) {
        self.change_code(CodeChange::PromptSuggestions(on));
    }

    pub fn set_allow_bypass_mode(&self, on: bool) {
        self.change_code(CodeChange::AllowBypass(on));
    }

    pub fn set_dynamic_workflows(&self, on: bool) {
        self.change_code(CodeChange::DynamicWorkflows(on));
    }

    pub fn worktree_location(&self) -> WorktreeLocation {
        self.saved.read().worktree_location.clone()
    }

    pub fn set_worktree_location(&self, location: WorktreeLocation) {
        self.change_code(CodeChange::WorktreeLocation(location));
    }

    /// Changes whenever the folder dialog closes without a choice, so the menu that opened it is drawn again.
    pub fn location_nonce(&self) -> u32 {
        *self.location_nonce.read()
    }

    /// Asks for a folder with the system dialog and keeps worktrees there from now on. A cancelled
    /// dialog changes nothing.
    pub fn choose_worktree_folder(&self) {
        if *self.trusted_picking.peek() {
            return;
        }
        let picker = (*self.folder_picker.peek()).clone();
        let this = *self;
        let mut picking = self.trusted_picking;
        picking.set(true);
        // Root scope, like the other saves: the dialog may outlive the page.
        dioxus::core::spawn_forever(async move {
            let picked = picker.pick_folder().await;
            picking.set(false);
            match picked {
                Some(folder) => this.set_worktree_location(WorktreeLocation::Folder(folder)),
                None => {
                    let mut nonce = this.location_nonce;
                    let next = *nonce.peek() + 1;
                    nonce.set(next);
                }
            }
        });
    }

    /// The language on screen.
    pub fn language(&self) -> Language {
        forge_shared::i18n::current()
    }

    /// Switches the interface language at once and remembers the choice. The screen follows the
    /// choice even if the config cannot be written (the error shows as for any other setting).
    pub fn set_language(&self, value: Language) {
        forge_shared::i18n::set_current(value);
        self.change_code(CodeChange::Language(value));
    }

    pub fn appearance(&self) -> Appearance {
        self.saved.read().appearance
    }

    pub fn notifications(&self) -> NotificationPrefs {
        self.saved.read().notifications
    }

    pub fn attention_bounce(&self) -> bool {
        self.saved.read().attention_bounce
    }

    /// The config as last loaded or saved.
    pub fn saved_config(&self) -> AppConfig {
        self.saved.read().clone()
    }

    pub fn set_theme(&self, value: Theme) {
        self.change_code(CodeChange::Theme(value));
    }

    pub fn set_font(&self, value: FontChoice) {
        self.change_code(CodeChange::Font(value));
    }

    pub fn set_text_size(&self, value: TextSize) {
        self.change_code(CodeChange::TextSize(value));
    }

    pub fn set_chat_width(&self, value: ChatWidth) {
        self.change_code(CodeChange::ChatWidth(value));
    }

    pub fn set_motion(&self, value: MotionPref) {
        self.change_code(CodeChange::Motion(value));
    }

    pub fn set_notification(&self, kind: NotificationKind, on: bool) {
        self.change_code(CodeChange::Notification(kind, on));
    }

    pub fn set_attention_bounce(&self, on: bool) {
        self.change_code(CodeChange::AttentionBounce(on));
    }

    /// The trusted folders, each once (a hand-edited config can repeat one).
    pub fn trusted_folders(&self) -> Vec<PathBuf> {
        crate::trust::dedup_paths(&self.saved.read().trusted_folders)
    }

    pub fn trusted_dialog_open(&self) -> bool {
        *self.trusted_dialog.read()
    }

    pub fn open_trusted_dialog(&self) {
        let mut dialog = self.trusted_dialog;
        dialog.set(true);
    }

    pub fn close_trusted_dialog(&self) {
        let mut dialog = self.trusted_dialog;
        dialog.set(false);
    }

    /// Asks for a folder with the system dialog and puts it on the list (a folder below a listed
    /// one is not added again).
    pub fn add_trusted_folder(&self) {
        if *self.trusted_picking.peek() {
            return;
        }
        let picker = (*self.folder_picker.peek()).clone();
        let this = *self;
        let mut picking = self.trusted_picking;
        picking.set(true);
        // Pinned to the root scope, like the other saves: the dialog may be closed while the system dialog is open.
        dioxus::core::spawn_forever(async move {
            let picked = picker.pick_folder().await;
            picking.set(false);
            if let Some(picked) = picked {
                // `canonicalize` can stall on a slow share: off the UI thread.
                if let Some(folder) = crate::blocking::run_blocking(move || crate::trust::resolved(&picked)).await {
                    this.change_code(CodeChange::TrustFolder(folder));
                }
            }
        });
    }

    pub fn remove_trusted_folder(&self, path: PathBuf) {
        self.change_code(CodeChange::UntrustFolder(path));
    }

    pub fn status(&self) -> Option<SettingsStatus> {
        status_for(&self.status.read(), self.section())
    }

    /// Saves the Connection fields. On failure the fields keep what was typed and only the
    /// status changes.
    pub fn save_connection(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let base_url = self.base_url.peek().clone();
        let api_key = self.api_key.peek().clone();
        let this = *self;
        // Pinned to the root scope: a plain `spawn` is cancelled when the calling component
        // unmounts (switching tab, Esc), which could drop the save half-way (spec §15.11).
        dioxus::core::spawn_forever(async move {
            match usecase.save_connection(&base_url, &api_key).await {
                Ok(config) => this.after_save(config),
                Err(error) => this.fail(SettingsSection::Connection, error.to_string()),
            }
        });
    }

    /// Saves the chosen model at once. `value` is what to store: empty means the SDK default.
    pub fn choose_model(&self, value: String) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        // Pinned to the root scope: a plain `spawn` is cancelled when the calling component
        // unmounts (switching tab, Esc), which could drop the save half-way (spec §15.11).
        dioxus::core::spawn_forever(async move {
            match usecase.save_model(&value).await {
                Ok(config) => this.after_save_model(config),
                Err(error) => this.fail(SettingsSection::Model, error.to_string()),
            }
        });
    }

    /// Opens on ⌘, and closes on Esc; returns whether the key press was one of them.
    pub fn on_key(&self, event: &KeyboardEvent) -> bool {
        if self.is_open() && event.key() == Key::Escape {
            if self.trusted_dialog_open() {
                self.close_trusted_dialog();
            } else {
                self.close();
            }
            event.prevent_default();
            return true;
        }
        if shortcut_from_event(event) == Some(Shortcut::OpenSettings) {
            self.open();
            event.prevent_default();
            return true;
        }
        false
    }

    /// The project whose skills and MCP servers the tabs list (`None`: global definitions only).
    pub fn set_project(&self, project: Option<PathBuf>) {
        let mut current = self.project;
        if *current.peek() != project {
            current.set(project);
        }
    }

    pub fn project_label(&self) -> String {
        project_label(self.project.read().as_deref())
    }

    /// The servers that run on this computer, for the Developer page.
    pub fn local_servers(&self) -> Result<Vec<forge_domain::LocalMcpServer>, String> {
        self.catalog.read().mcp.clone().map(|definitions| forge_domain::local_mcp_servers(&definitions))
    }

    /// The server shown in the Developer page's detail: the chosen one while it is still listed, else the first.
    pub fn selected_server(&self) -> Option<forge_domain::LocalMcpServer> {
        let servers = self.local_servers().ok()?;
        let chosen = self.server.read().clone();
        chosen.and_then(|name| servers.iter().find(|server| server.name == name).cloned()).or_else(|| servers.into_iter().next())
    }

    pub fn select_server(&self, name: String) {
        let mut server = self.server;
        server.set(Some(name));
    }

    pub fn skill_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_skills.clone();
        self.catalog.read().skills.clone().map(|definitions| skill_rows(&definitions, &enabled))
    }

    pub fn plugin_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_plugins.clone();
        self.catalog.read().plugins.clone().map(|definitions| plugin_rows(&definitions, &enabled))
    }

    pub fn mcp_tab(&self) -> Result<Vec<ExtensionRow>, String> {
        let enabled = self.saved.read().enabled_mcp_servers.clone();
        self.catalog.read().mcp.clone().map(|definitions| mcp_rows(&definitions, &enabled))
    }

    /// The MCP servers, as the composer's `+` menu lists them: the same rows as the Settings tab.
    pub fn connector_rows(&self) -> Result<Vec<ExtensionRow>, String> {
        self.mcp_tab()
    }

    /// Points the lists at `project` and reads them again, so the `+` menu shows what Settings would.
    pub fn prepare_menu(&self, project: PathBuf) {
        self.set_project(Some(project));
        self.reload_catalog();
    }

    /// Switches one skill, plugin or MCP server on or off and saves at once. `key` is the row's key.
    pub fn toggle(&self, kind: ExtensionKind, key: String) {
        let extensions = (*self.extensions.peek()).clone();
        let this = *self;
        let section = match kind {
            ExtensionKind::Skill => SettingsSection::Skills,
            ExtensionKind::Plugin => SettingsSection::Plugins,
            ExtensionKind::McpServer => SettingsSection::McpServers,
        };
        // Pinned to the root scope, like the other saves: switching tab must not cancel it.
        dioxus::core::spawn_forever(async move {
            let result = match kind {
                ExtensionKind::Skill => extensions.toggle_skill(&key).await,
                ExtensionKind::Plugin => extensions.toggle_plugin(&key).await,
                ExtensionKind::McpServer => extensions.toggle_mcp_server(&key).await,
            };
            match result {
                Ok(config) => this.after_toggle(section, config),
                Err(error) => this.fail(section, error.to_string()),
            }
        });
    }

    // Lists the definitions for the current project; each kind fails on its own.
    fn reload_catalog(&self) {
        let extensions = (*self.extensions.peek()).clone();
        let project = self.project.peek().clone();
        let mut catalog = self.catalog;
        spawn(async move {
            let scope = project.as_deref();
            catalog.set(Catalog {
                skills: extensions.list_skills(scope).await.map_err(|e| e.to_string()),
                plugins: extensions.list_plugins().await.map_err(|e| e.to_string()),
                mcp: extensions.list_mcp_servers(scope).await.map_err(|e| e.to_string()),
            });
        });
    }

    // After a toggle only the enabled lists changed; leave unsaved Connection edits alone.
    fn after_toggle(&self, section: SettingsSection, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        {
            let mut current = saved.write();
            current.enabled_skills = config.enabled_skills;
            current.enabled_plugins = config.enabled_plugins;
            current.enabled_mcp_servers = config.enabled_mcp_servers;
        }
        status.set(Some((section, SettingsStatus::Saved)));
    }

    // Loads the stored config into `saved` and the two fields.
    fn reload(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        spawn(async move {
            match usecase.load().await {
                Ok(config) => this.show(config),
                Err(error) => this.fail(*this.section.peek(), error.to_string()),
            }
        });
    }

    fn show(&self, config: AppConfig) {
        let draft = ConnectionDraft::from_config(&config);
        let mut saved = self.saved;
        let mut base_url = self.base_url;
        let mut api_key = self.api_key;
        crate::appearance::apply_appearance(&config.appearance);
        forge_shared::i18n::set_current(forge_shared::i18n::effective_language(config.language));
        saved.set(config);
        base_url.set(draft.base_url);
        api_key.set(draft.api_key);
    }

    // After a Connection save: keep the fields as the person typed them, record what is stored.
    fn after_save(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        saved.set(config);
        status.set(Some((SettingsSection::Connection, SettingsStatus::Saved)));
    }

    // Saves one Code tab control at once. The shown value follows the save, so a failed save leaves
    // the control where it was and shows the error.
    fn change_code(&self, change: CodeChange) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        // Pinned to the root scope, like the other saves: switching tab must not cancel it.
        dioxus::core::spawn_forever(async move {
            match usecase.save_code_settings(|config| apply_code_change(config, change)).await {
                Ok(config) => this.after_save_code(config),
                Err(error) => this.fail(*this.section.peek(), error.to_string()),
            }
        });
    }

    // After a Code save only its four fields changed; leave unsaved Connection edits alone.
    fn after_save_code(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        {
            let mut current = saved.write();
            current.default_transcript_view = config.default_transcript_view;
            current.output_style = config.output_style;
            current.prompt_suggestions = config.prompt_suggestions;
            current.allow_bypass_mode = config.allow_bypass_mode;
            current.dynamic_workflows = config.dynamic_workflows;
            current.worktree_location = config.worktree_location;
            current.auto_archive_days = config.auto_archive_days;
            current.appearance = config.appearance;
            current.notifications = config.notifications;
            current.attention_bounce = config.attention_bounce;
            current.language = config.language;
            current.trusted_folders = config.trusted_folders;
        }
        crate::appearance::apply_appearance(&config.appearance);
        status.set(Some((*self.section.peek(), SettingsStatus::Saved)));
    }

    // After a model save only the model changed; leave any unsaved Connection edits alone.
    fn after_save_model(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        let model = config.model;
        saved.write().model = model;
        status.set(Some((SettingsSection::Model, SettingsStatus::Saved)));
    }

    fn fail(&self, section: SettingsSection, message: String) {
        let mut status = self.status;
        status.set(Some((section, SettingsStatus::Error(message))));
    }

    fn clear_status(&self) {
        let mut status = self.status;
        status.set(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trusting_and_untrusting_a_folder_changes_only_the_list() {
        let mut config = AppConfig { model: Some("m".into()), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::TrustFolder("/work/app".into()));
        apply_code_change(&mut config, CodeChange::TrustFolder("/other".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/work/app"), PathBuf::from("/other")]);
        apply_code_change(&mut config, CodeChange::TrustFolder("/work/app/src".into()));
        assert_eq!(config.trusted_folders.len(), 2, "below a trusted folder: no new entry");
        apply_code_change(&mut config, CodeChange::UntrustFolder("/work/app".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/other")]);
        apply_code_change(&mut config, CodeChange::UntrustFolder("/never/listed".into()));
        assert_eq!(config.trusted_folders, vec![PathBuf::from("/other")]);
        assert_eq!(config.model.as_deref(), Some("m"), "nothing else is touched");
    }

    #[test]
    fn choosing_a_language_stores_it() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::Language(forge_domain::Language::Korean));
        assert_eq!(config.language, Some(forge_domain::Language::Korean));
        apply_code_change(&mut config, CodeChange::Language(forge_domain::Language::English));
        assert_eq!(config.language, Some(forge_domain::Language::English));
    }

    #[test]
    fn choosing_the_current_language_again_changes_nothing_else() {
        let mut config = AppConfig { language: Some(forge_domain::Language::Korean), model: Some("m".into()), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::Language(forge_domain::Language::Korean));
        assert_eq!(config.language, Some(forge_domain::Language::Korean));
        assert_eq!(config.model.as_deref(), Some("m"));
    }

    #[test]
    fn the_worktree_location_change_sets_only_the_location() {
        let mut config = AppConfig { auto_archive_days: Some(7), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::WorktreeLocation(forge_domain::WorktreeLocation::Folder("/trees".into())));
        assert_eq!(config.worktree_location, forge_domain::WorktreeLocation::Folder("/trees".into()));
        assert_eq!(config.auto_archive_days, Some(7));
        apply_code_change(&mut config, CodeChange::WorktreeLocation(forge_domain::WorktreeLocation::Project));
        assert_eq!(config.worktree_location, forge_domain::WorktreeLocation::Project);
    }

    #[test]
    fn the_search_finds_the_worktree_location_row_on_the_code_page() {
        let pages = crate::settings_pages::filter_pages("worktree");
        assert!(pages.contains(&SettingsSection::Code), "{pages:?}");
    }

    #[test]
    fn each_control_changes_its_own_field_only() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::DefaultView(TranscriptView::Thinking));
        apply_code_change(&mut config, CodeChange::OutputStyle(Some(OutputStyle::Learning)));
        apply_code_change(&mut config, CodeChange::PromptSuggestions(false));
        apply_code_change(&mut config, CodeChange::AllowBypass(true));
        apply_code_change(&mut config, CodeChange::DynamicWorkflows(true));

        assert_eq!(config.default_transcript_view, TranscriptView::Thinking);
        assert_eq!(config.output_style, Some(OutputStyle::Learning));
        assert!(!config.prompt_suggestions);
        assert!(config.allow_bypass_mode);
        assert!(config.dynamic_workflows);
        assert_eq!(config.model, None, "nothing else is touched");
    }

    #[test]
    fn choosing_the_default_style_clears_it() {
        let mut config = AppConfig { output_style: Some(OutputStyle::Concise), ..AppConfig::default() };
        apply_code_change(&mut config, CodeChange::OutputStyle(None));
        assert_eq!(config.output_style, None);
    }

    #[test]
    fn the_new_controls_change_their_own_field_only() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::Theme(Theme::Dark));
        apply_code_change(&mut config, CodeChange::Font(FontChoice::OpenDyslexic));
        apply_code_change(&mut config, CodeChange::TextSize(TextSize::Large));
        apply_code_change(&mut config, CodeChange::ChatWidth(ChatWidth::Narrow));
        apply_code_change(&mut config, CodeChange::Motion(MotionPref::Reduced));
        apply_code_change(&mut config, CodeChange::Notification(NotificationKind::ReplyDone, false));
        apply_code_change(&mut config, CodeChange::AttentionBounce(true));
        assert_eq!(config.appearance.theme, Theme::Dark);
        assert_eq!(config.appearance.font, FontChoice::OpenDyslexic);
        assert_eq!(config.appearance.text_size, TextSize::Large);
        assert_eq!(config.appearance.chat_width, ChatWidth::Narrow);
        assert_eq!(config.appearance.motion, MotionPref::Reduced);
        assert!(!config.notifications.reply_done);
        assert!(config.notifications.permission_request, "the other switches stay on");
        assert!(config.attention_bounce);
        assert!(config.prompt_suggestions, "nothing else is touched");
    }
}
