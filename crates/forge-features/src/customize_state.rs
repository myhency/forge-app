use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{CatalogView, PluginCatalogUseCase};

use crate::customize::{check_source, CustomizeMode, CustomizeTab, Job, JobQueue};
use crate::workspace::WorkspaceServices;

/// What the last read of the catalogue gave.
#[derive(Debug, Clone, PartialEq)]
pub enum Loaded {
    Loading,
    Ready(CatalogView),
    Failed(String),
}

/// A dialog over the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    AddMarketplace,
    ManageMarketplaces,
    ConfirmUninstall(String),
    ConfirmRemoveMarketplace(String),
}

/// Whether a job about one plugin is running, waiting, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    No,
    Waiting,
    Running,
}

/// The Customize page's state. `Copy` (every field is a signal), like `Workspace`.
#[derive(Clone, Copy)]
pub struct Customize {
    usecase: Signal<Arc<PluginCatalogUseCase>>,
    tab: Signal<CustomizeTab>,
    mode: Signal<CustomizeMode>,
    query: Signal<String>,
    detail: Signal<Option<String>>,
    dialog: Signal<Option<Dialog>>,
    queue: Signal<JobQueue>,
    view: Signal<Loaded>,
    error: Signal<Option<String>>,
    dialog_error: Signal<Option<String>>,
}

/// Creates the page state and provides it to every descendant. Call it in the page at the top of the tree.
pub fn use_customize_provider() -> Customize {
    let services = use_context::<WorkspaceServices>();
    // Owned by the root scope: `spawn_forever` tasks write to these after the page that made them has re-rendered.
    let customize = use_hook(move || Customize {
        usecase: Signal::new_in_scope(services.plugin_catalog.clone(), ScopeId::ROOT),
        tab: Signal::new_in_scope(CustomizeTab::Plugins, ScopeId::ROOT),
        mode: Signal::new_in_scope(CustomizeMode::Mine, ScopeId::ROOT),
        query: Signal::new_in_scope(String::new(), ScopeId::ROOT),
        detail: Signal::new_in_scope(None, ScopeId::ROOT),
        dialog: Signal::new_in_scope(None, ScopeId::ROOT),
        queue: Signal::new_in_scope(JobQueue::default(), ScopeId::ROOT),
        view: Signal::new_in_scope(Loaded::Loading, ScopeId::ROOT),
        error: Signal::new_in_scope(None, ScopeId::ROOT),
        dialog_error: Signal::new_in_scope(None, ScopeId::ROOT),
    });
    use_context_provider(move || customize);
    customize
}

pub fn use_customize() -> Customize {
    use_context::<Customize>()
}

impl Customize {
    /// Called when the page appears: back to the list, then read the catalogue.
    pub fn open_page(&self) {
        let mut detail = self.detail;
        detail.set(None);
        let mut dialog = self.dialog;
        dialog.set(None);
        let mut view = self.view;
        view.set(Loaded::Loading);
        self.reload();
    }

    /// Reads the catalogue again. Pinned to the root scope so leaving the page does not cancel it.
    pub fn reload(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let loaded = match usecase.catalog().await {
                Ok(view) => Loaded::Ready(view),
                Err(error) => Loaded::Failed(error.to_string()),
            };
            let mut view = this.view;
            view.set(loaded);
        });
    }

    pub fn view(&self) -> Loaded {
        self.view.read().clone()
    }

    pub fn tab(&self) -> CustomizeTab {
        *self.tab.read()
    }

    pub fn set_tab(&self, tab: CustomizeTab) {
        let mut current = self.tab;
        current.set(tab);
        self.close_detail();
    }

    pub fn mode(&self) -> CustomizeMode {
        *self.mode.read()
    }

    pub fn set_mode(&self, mode: CustomizeMode) {
        let mut current = self.mode;
        current.set(mode);
        self.close_detail();
    }

    pub fn query(&self) -> String {
        self.query.read().clone()
    }

    pub fn set_query(&self, text: String) {
        let mut query = self.query;
        query.set(text);
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.read().clone()
    }

    pub fn open_detail(&self, id: String) {
        let mut detail = self.detail;
        detail.set(Some(id));
    }

    pub fn close_detail(&self) {
        let mut detail = self.detail;
        detail.set(None);
    }

    pub fn dialog(&self) -> Option<Dialog> {
        self.dialog.read().clone()
    }

    pub fn open_dialog(&self, dialog: Dialog) {
        let mut error = self.dialog_error;
        error.set(None);
        let mut current = self.dialog;
        current.set(Some(dialog));
    }

    pub fn close_dialog(&self) {
        let mut error = self.dialog_error;
        error.set(None);
        let mut current = self.dialog;
        current.set(None);
    }

    pub fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    pub fn dismiss_error(&self) {
        let mut error = self.error;
        error.set(None);
    }

    pub fn dialog_error(&self) -> Option<String> {
        self.dialog_error.read().clone()
    }

    /// Asks for a job. It starts at once when nothing is running, otherwise it waits its turn.
    pub fn run(&self, job: Job) {
        let mut queue = self.queue;
        let start = queue.write().push(job);
        if let Some(job) = start {
            self.start(job);
        }
    }

    /// Whether a job about this plugin is running or waiting (drives the spinner).
    pub fn busy(&self, id: &str) -> Busy {
        let queue = self.queue.read();
        if queue.running().and_then(Job::plugin_id) == Some(id) {
            Busy::Running
        } else if queue.waits_for(id) {
            Busy::Waiting
        } else {
            Busy::No
        }
    }

    /// Whether an Add marketplace job is running (the dialog shows a spinner).
    pub fn adding_marketplace(&self) -> bool {
        matches!(self.queue.read().running(), Some(Job::AddMarketplace(_)))
    }

    /// The Add marketplace button: checks the text, then queues the job.
    pub fn add_marketplace(&self, text: &str) {
        match check_source(text) {
            Ok(source) => {
                let mut error = self.dialog_error;
                error.set(None);
                self.run(Job::AddMarketplace(source));
            }
            Err(message) => {
                let mut error = self.dialog_error;
                error.set(Some(message.to_string()));
            }
        }
    }

    fn start(&self, job: Job) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        // Pinned to the root scope: a plain `spawn` is cancelled when the page unmounts, which
        // could drop an install half-way.
        dioxus::core::spawn_forever(async move {
            let result = match &job {
                Job::Install(id) => usecase.install(id).await,
                Job::Uninstall(id) => usecase.uninstall(id).await,
                Job::SetEnabled(id, on) => usecase.set_enabled(id, *on).await,
                Job::AddMarketplace(source) => usecase.add_marketplace(source).await,
                Job::RemoveMarketplace(name) => usecase.remove_marketplace(name).await,
                Job::UpdateMarketplace(name) => usecase.update_marketplace(name).await,
            }
            .map_err(|error| error.to_string());
            this.finished(job, result);
        });
    }

    fn finished(&self, job: Job, result: Result<(), String>) {
        match (&job, result) {
            (Job::AddMarketplace(_), Ok(())) => self.close_dialog(),
            (Job::AddMarketplace(_), Err(message)) => {
                let mut error = self.dialog_error;
                error.set(Some(message));
            }
            (_, Err(message)) => {
                let mut error = self.error;
                error.set(Some(message));
            }
            (_, Ok(())) => {}
        }
        // An uninstalled plugin has no detail to show, and a removed marketplace takes its plugins.
        if matches!(job, Job::Uninstall(_) | Job::RemoveMarketplace(_)) {
            self.close_detail();
        }
        let mut queue = self.queue;
        let next = queue.write().finish();
        // The list is read again, never patched by hand.
        self.reload();
        if let Some(next) = next {
            self.start(next);
        }
    }
}
