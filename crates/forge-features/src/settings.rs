use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{ExtensionsUseCase, SettingsUseCase};
use forge_domain::{AppConfig, Appearance, ChatWidth, FontChoice, Language, McpServerDefinition, MotionPref, NotificationPrefs, OutputStyle, PluginDefinition, SkillDefinition, TextSize, Theme, TranscriptView};
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
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CodeChange {
    DefaultView(TranscriptView),
    OutputStyle(Option<OutputStyle>),
    PromptSuggestions(bool),
    AllowBypass(bool),
    Theme(Theme),
    Font(FontChoice),
    TextSize(TextSize),
    ChatWidth(ChatWidth),
    Motion(MotionPref),
    Notification(NotificationKind, bool),
    AttentionBounce(bool),
    Language(Language),
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
        open.set(false);
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
            self.close();
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
            current.appearance = config.appearance;
            current.notifications = config.notifications;
            current.attention_bounce = config.attention_bounce;
            current.language = config.language;
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
    fn each_control_changes_its_own_field_only() {
        let mut config = AppConfig::default();
        apply_code_change(&mut config, CodeChange::DefaultView(TranscriptView::Thinking));
        apply_code_change(&mut config, CodeChange::OutputStyle(Some(OutputStyle::Learning)));
        apply_code_change(&mut config, CodeChange::PromptSuggestions(false));
        apply_code_change(&mut config, CodeChange::AllowBypass(true));

        assert_eq!(config.default_transcript_view, TranscriptView::Thinking);
        assert_eq!(config.output_style, Some(OutputStyle::Learning));
        assert!(!config.prompt_suggestions);
        assert!(config.allow_bypass_mode);
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
