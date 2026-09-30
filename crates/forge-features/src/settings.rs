use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::SettingsUseCase;
use forge_domain::AppConfig;

use crate::settings_draft::ConnectionDraft;
use crate::workspace::{shortcut_from_event, WorkspaceServices};
use crate::shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsSection {
    Connection,
    Model,
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
    saved: Signal<AppConfig>,
    base_url: Signal<String>,
    api_key: Signal<String>,
    key_visible: Signal<bool>,
    status: Signal<Option<SettingsStatus>>,
}

/// Creates the settings state, provides it to every descendant and loads the saved config once.
/// Call it in the page before `use_workspace_provider` (the new-session panel reads the model).
pub fn use_settings_provider() -> Settings {
    let services = use_context::<WorkspaceServices>();
    let settings = use_hook(move || Settings {
        usecase: Signal::new(services.settings.clone()),
        open: Signal::new(false),
        section: Signal::new(SettingsSection::Connection),
        saved: Signal::new(AppConfig::default()),
        base_url: Signal::new(String::new()),
        api_key: Signal::new(String::new()),
        key_visible: Signal::new(false),
        status: Signal::new(None),
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
        status.set(None);
        key_visible.set(false);
        self.reload();
    }

    pub fn close(&self) {
        let mut open = self.open;
        open.set(false);
    }

    pub fn section(&self) -> SettingsSection {
        *self.section.read()
    }

    pub fn set_section(&self, section: SettingsSection) {
        let mut current = self.section;
        current.set(section);
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

    pub fn status(&self) -> Option<SettingsStatus> {
        self.status.read().clone()
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
                Err(error) => this.fail(error.to_string()),
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
                Err(error) => this.fail(error.to_string()),
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

    // Loads the stored config into `saved` and the two fields.
    fn reload(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        spawn(async move {
            match usecase.load().await {
                Ok(config) => this.show(config),
                Err(error) => this.fail(error.to_string()),
            }
        });
    }

    fn show(&self, config: AppConfig) {
        let draft = ConnectionDraft::from_config(&config);
        let mut saved = self.saved;
        let mut base_url = self.base_url;
        let mut api_key = self.api_key;
        saved.set(config);
        base_url.set(draft.base_url);
        api_key.set(draft.api_key);
    }

    // After a Connection save: keep the fields as the person typed them, record what is stored.
    fn after_save(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        saved.set(config);
        status.set(Some(SettingsStatus::Saved));
    }

    // After a model save only the model changed; leave any unsaved Connection edits alone.
    fn after_save_model(&self, config: AppConfig) {
        let mut saved = self.saved;
        let mut status = self.status;
        let model = config.model;
        saved.write().model = model;
        status.set(Some(SettingsStatus::Saved));
    }

    fn fail(&self, message: String) {
        let mut status = self.status;
        status.set(Some(SettingsStatus::Error(message)));
    }

    fn clear_status(&self) {
        let mut status = self.status;
        status.set(None);
    }
}
