use std::sync::Arc;
use std::time::Duration;

use dioxus::prelude::*;
use forge_application::{RoutineError, RoutineRunner, RoutinesUseCase};
use forge_domain::{Routine, RoutineRun};

use crate::routines::{FormDraft, RoutineFilter, RoutineSort};
use crate::workspace::{Active, Workspace, WorkspaceServices};

/// How often the timer looks at the routines.
const TICK: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutineScreen {
    List,
    Detail(String),
    /// `None`: a new routine.
    Form(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutineDialog {
    ConfirmDelete(String),
    ConfirmDiscard,
}

/// The Routines page's state. `Copy` (every field is a signal), like `Workspace`.
#[derive(Clone, Copy)]
pub struct Routines {
    services: Signal<WorkspaceServices>,
    screen: Signal<RoutineScreen>,
    routines: Signal<Vec<Routine>>,
    runs: Signal<Vec<RoutineRun>>,
    query: Signal<String>,
    sort: Signal<RoutineSort>,
    filter: Signal<RoutineFilter>,
    form: Signal<FormDraft>,
    form_start: Signal<FormDraft>,
    dialog: Signal<Option<RoutineDialog>>,
    error: Signal<Option<String>>,
    busy: Signal<bool>,
    /// Routine ids that have a session waiting for approval right now.
    waiting: Signal<Vec<String>>,
}

/// Creates the Routines state, starts the timer and provides the state to every descendant. Call it
/// in the page at the top of the tree, after the workspace provider.
pub fn use_routines_provider(workspace: Workspace) -> Routines {
    let services = use_context::<WorkspaceServices>();
    let routines = use_hook(move || {
        let routines = Routines {
            services: Signal::new_in_scope(services.clone(), ScopeId::ROOT),
            screen: Signal::new_in_scope(RoutineScreen::List, ScopeId::ROOT),
            routines: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
            runs: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
            query: Signal::new_in_scope(String::new(), ScopeId::ROOT),
            sort: Signal::new_in_scope(RoutineSort::default(), ScopeId::ROOT),
            filter: Signal::new_in_scope(RoutineFilter::default(), ScopeId::ROOT),
            form: Signal::new_in_scope(FormDraft::new(), ScopeId::ROOT),
            form_start: Signal::new_in_scope(FormDraft::new(), ScopeId::ROOT),
            dialog: Signal::new_in_scope(None, ScopeId::ROOT),
            error: Signal::new_in_scope(None, ScopeId::ROOT),
            busy: Signal::new_in_scope(false, ScopeId::ROOT),
            waiting: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
        };
        routines.start_timer(workspace);
        routines
    });
    use_context_provider(move || routines);
    routines
}

pub fn use_routines() -> Routines {
    use_context::<Routines>()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|elapsed| elapsed.as_millis() as i64).unwrap_or(0)
}

impl Routines {
    pub fn screen(&self) -> RoutineScreen {
        self.screen.read().clone()
    }
    pub fn routines(&self) -> Vec<Routine> {
        self.routines.read().clone()
    }
    pub fn runs(&self) -> Vec<RoutineRun> {
        self.runs.read().clone()
    }
    pub fn query(&self) -> String {
        self.query.read().clone()
    }
    pub fn sort(&self) -> RoutineSort {
        *self.sort.read()
    }
    pub fn filter(&self) -> RoutineFilter {
        *self.filter.read()
    }
    pub fn form(&self) -> FormDraft {
        self.form.read().clone()
    }
    pub fn dialog(&self) -> Option<RoutineDialog> {
        self.dialog.read().clone()
    }
    pub fn error(&self) -> Option<String> {
        self.error.read().clone()
    }
    pub fn busy(&self) -> bool {
        *self.busy.read()
    }
    pub fn waiting(&self) -> Vec<String> {
        self.waiting.read().clone()
    }
    pub fn form_is_dirty(&self) -> bool {
        self.form.read().is_dirty_against(&self.form_start.read())
    }

    pub fn set_query(&self, text: String) {
        let mut query = self.query;
        query.set(text);
    }
    pub fn set_sort(&self, sort: RoutineSort) {
        let mut slot = self.sort;
        slot.set(sort);
    }
    pub fn set_filter(&self, filter: RoutineFilter) {
        let mut slot = self.filter;
        slot.set(filter);
    }
    pub fn edit_form(&self, change: impl FnOnce(&mut FormDraft)) {
        let mut form = self.form;
        change(&mut form.write());
    }
    pub fn open_dialog(&self, dialog: RoutineDialog) {
        let mut slot = self.dialog;
        slot.set(Some(dialog));
    }
    pub fn close_dialog(&self) {
        let mut slot = self.dialog;
        slot.set(None);
    }
    pub fn clear_error(&self) {
        let mut slot = self.error;
        slot.set(None);
    }

    /// Called when the page appears: back to the list, then read what is saved.
    pub fn open_page(&self) {
        let mut screen = self.screen;
        screen.set(RoutineScreen::List);
        self.reload();
    }

    pub fn show_detail(&self, id: String) {
        let mut screen = self.screen;
        screen.set(RoutineScreen::Detail(id));
    }

    pub fn show_list(&self) {
        let mut screen = self.screen;
        screen.set(RoutineScreen::List);
    }

    /// Opens the form: empty for a new routine, filled for `Some(id)`.
    pub fn open_form(&self, id: Option<String>) {
        let start = match id.as_ref().and_then(|id| self.routines.read().iter().find(|routine| &routine.id == id).cloned()) {
            Some(routine) => FormDraft::from_routine(&routine),
            None => FormDraft::new(),
        };
        let (mut form, mut form_start, mut screen) = (self.form, self.form_start, self.screen);
        form.set(start.clone());
        form_start.set(start);
        screen.set(RoutineScreen::Form(id));
    }

    /// Leaves the form; a changed form asks first.
    pub fn leave_form(&self) {
        if self.form_is_dirty() {
            self.open_dialog(RoutineDialog::ConfirmDiscard);
        } else {
            self.discard_form();
        }
    }

    pub fn discard_form(&self) {
        self.close_dialog();
        let back = match self.screen() {
            RoutineScreen::Form(Some(id)) => RoutineScreen::Detail(id),
            _ => RoutineScreen::List,
        };
        let mut screen = self.screen;
        screen.set(back);
    }

    /// Re-reads the routines and their history from the config.
    pub fn reload(&self) {
        let this = *self;
        let usecase = self.services.peek().routines.routines.clone();
        dioxus::core::spawn_forever(async move {
            match usecase.snapshot().await {
                Ok((routines, runs)) => {
                    let (mut routines_slot, mut runs_slot) = (this.routines, this.runs);
                    routines_slot.set(routines);
                    runs_slot.set(runs);
                }
                Err(error) => this.fail(error),
            }
        });
    }

    fn fail(&self, error: RoutineError) {
        let mut slot = self.error;
        slot.set(Some(error.to_string()));
    }

    /// Runs `job` against the use cases, then re-reads. One job at a time (`busy`). The job may name the
    /// screen to show afterwards.
    fn run_job<F, Fut>(&self, job: F)
    where
        F: FnOnce(Arc<RoutinesUseCase>, Arc<RoutineRunner>) -> Fut + 'static,
        Fut: std::future::Future<Output = Result<Option<RoutineScreen>, RoutineError>> + 'static,
    {
        if self.busy() {
            return;
        }
        let this = *self;
        let services = self.services.peek().routines.clone();
        let mut busy = self.busy;
        busy.set(true);
        dioxus::core::spawn_forever(async move {
            let result = job(services.routines.clone(), services.runner.clone()).await;
            let mut busy = this.busy;
            busy.set(false);
            match result {
                Ok(next) => {
                    this.clear_error();
                    if let Some(screen) = next {
                        let mut slot = this.screen;
                        slot.set(screen);
                    }
                    this.reload();
                }
                Err(error) => this.fail(error),
            }
        });
    }

    /// Saves the form: creates the routine or updates the one being edited.
    pub fn submit_form(&self) {
        let form = self.form();
        let editing = match self.screen() {
            RoutineScreen::Form(id) => id,
            _ => return,
        };
        self.run_job(move |routines, _| async move {
            let saved = match editing {
                Some(id) => routines.update(&id, form.to_draft(), now_ms()).await?,
                None => routines.create(form.to_draft(), now_ms()).await?,
            };
            Ok(Some(RoutineScreen::Detail(saved.id)))
        });
    }

    pub fn set_enabled(&self, id: String, enabled: bool) {
        self.run_job(move |routines, _| async move {
            routines.set_enabled(&id, enabled, now_ms()).await?;
            Ok(None)
        });
    }

    pub fn delete(&self, id: String) {
        self.close_dialog();
        self.run_job(move |routines, _| async move {
            routines.delete(&id).await?;
            Ok(Some(RoutineScreen::List))
        });
    }

    pub fn remove_allowed(&self, id: String, rule: String) {
        self.run_job(move |routines, _| async move {
            routines.remove_allowed(&id, &rule).await?;
            Ok(None)
        });
    }

    /// Starts the routine at once; its session is shown a moment later (when the host has handed it out).
    pub fn run_now(&self, id: String, workspace: Workspace) {
        let this = *self;
        self.run_job(move |_, runner| async move {
            runner.run_now(&id, now_ms(), &chrono::Local).await?;
            Ok(None)
        });
        dioxus::core::spawn_forever(async move {
            tokio::time::sleep(Duration::from_millis(400)).await;
            this.adopt_started(workspace);
        });
    }

    /// Picks the folder for the form with the system picker.
    pub fn choose_folder(&self) {
        let picker = self.services.peek().folder_picker.clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            if let Some(folder) = picker.pick_folder().await {
                this.edit_form(|form| form.folder = Some(folder));
            }
        });
    }

    /// Shows every session the host started since the last look, and closes the finished ones that are
    /// not on screen (their transcripts stay; the sidebar lists them as past sessions).
    fn adopt_started(&self, workspace: Workspace) {
        let services = self.services.peek().routines.clone();
        let handles: Vec<_> = {
            let mut receiver = services.starts.lock().unwrap();
            std::iter::from_fn(|| receiver.try_recv().ok()).collect()
        };
        for handle in handles {
            workspace.adopt(handle);
        }
        for id in services.host.finished() {
            if workspace.active() != Active::Live(id) {
                workspace.close(id);
                services.host.forget(id);
            }
        }
    }

    fn start_timer(&self, workspace: Workspace) {
        let this = *self;
        dioxus::core::spawn_forever(async move {
            loop {
                let services = this.services.peek().routines.clone();
                if let Err(error) = services.runner.tick(now_ms(), &chrono::Local).await {
                    this.fail(error);
                }
                this.adopt_started(workspace);
                let mut waiting = this.waiting;
                waiting.set(services.host.waiting_now());
                for run in services.host.waiting() {
                    let name = this.routines.peek().iter().find(|routine| routine.id == run.routine_id).map(|routine| routine.name.clone()).unwrap_or(run.routine_id.clone());
                    let (title, body) = crate::routines::waiting_notice(&name, &run.tool_name);
                    // A notification that cannot be sent is not worth a banner: the page and the sidebar show the wait.
                    let _ = services.notify.notify(&title, &body);
                }
                this.reload();
                tokio::time::sleep(TICK).await;
            }
        });
    }
}
