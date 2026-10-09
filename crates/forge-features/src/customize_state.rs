use std::path::PathBuf;
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{CatalogView, PluginCatalogUseCase, SkillCatalogUseCase, SkillView, SkillsError};
use forge_domain::{SkillFile, SkillFileList};
use forge_ports::{FilePickerPort, FolderPickerPort};

use crate::customize_connectors::Connectors;
use crate::customize_skills::{set_skill_enabled_in, SkillPane, SkillText};
use crate::customize::{check_source, closes_detail, error_in_dialog, CustomizeMode, CustomizeTab, Job, JobQueue, Latest};
use crate::workspace::WorkspaceServices;

/// What the last read of the catalogue gave.
#[derive(Debug, Clone, PartialEq)]
pub enum Loaded {
    Loading,
    Ready(CatalogView),
    Failed(String),
}

/// Whether a skill that was just added is brought to the person: not when they have gone to another
/// tab while the picker or the copy was running.
fn shows_new_skill(tab: CustomizeTab) -> bool {
    tab == CustomizeTab::Skills
}

/// What the skills are read against: the plugin catalogue when it was read, else an empty one, so a
/// broken or missing bundled `claude` still leaves the person's own skills on the page.
fn plugins_for_skills(loaded: &Loaded) -> CatalogView {
    match loaded {
        Loaded::Ready(view) => view.clone(),
        _ => CatalogView { snapshot: forge_domain::PluginSnapshot::default(), enabled: Vec::new() },
    }
}

/// A dialog over the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    AddMarketplace,
    ManageMarketplaces,
    ConfirmUninstall(String),
    ConfirmRemoveMarketplace(String),
    CreateSkill,
    AddConnector,
    ConfirmRemoveConnector(String),
    SignInConnector(String),
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
    reads: Signal<Latest>,
    skills_usecase: Signal<Arc<SkillCatalogUseCase>>,
    folder_picker: Signal<Arc<dyn FolderPickerPort>>,
    file_picker: Signal<Arc<dyn FilePickerPort>>,
    pub(crate) project: Signal<Option<PathBuf>>,
    skills: Signal<Option<SkillView>>,
    skills_busy: Signal<bool>,
    skill_dir: Signal<Option<PathBuf>>,
    skill_pane: Signal<SkillPane>,
    skill_files: Signal<Vec<SkillFile>>,
    skill_files_cut: Signal<bool>,
    skill_file: Signal<Option<String>>,
    skill_text: Signal<SkillText>,
    skill_source: Signal<bool>,
    file_reads: Signal<Latest>,
    /// Counts page visits, so an open request made on one visit never lands on the next.
    visit: Signal<u64>,
    pub(crate) connectors: Connectors,
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
        reads: Signal::new_in_scope(Latest::default(), ScopeId::ROOT),
        skills_usecase: Signal::new_in_scope(services.skill_catalog.clone(), ScopeId::ROOT),
        folder_picker: Signal::new_in_scope(services.folder_picker.clone(), ScopeId::ROOT),
        file_picker: Signal::new_in_scope(services.file_picker.clone(), ScopeId::ROOT),
        project: Signal::new_in_scope(None, ScopeId::ROOT),
        skills: Signal::new_in_scope(None, ScopeId::ROOT),
        skills_busy: Signal::new_in_scope(false, ScopeId::ROOT),
        skill_dir: Signal::new_in_scope(None, ScopeId::ROOT),
        skill_pane: Signal::new_in_scope(SkillPane::Overview, ScopeId::ROOT),
        skill_files: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
        skill_files_cut: Signal::new_in_scope(false, ScopeId::ROOT),
        skill_file: Signal::new_in_scope(None, ScopeId::ROOT),
        skill_text: Signal::new_in_scope(SkillText::Empty, ScopeId::ROOT),
        skill_source: Signal::new_in_scope(false, ScopeId::ROOT),
        file_reads: Signal::new_in_scope(Latest::default(), ScopeId::ROOT),
        visit: Signal::new_in_scope(0, ScopeId::ROOT),
        connectors: Connectors::new(services.connector_catalog.clone()),
    });
    use_context_provider(move || customize);
    customize
}

pub fn use_customize() -> Customize {
    use_context::<Customize>()
}

impl Customize {
    /// Called when the page appears: back to the list, then read the catalogue. `project` is the
    /// folder whose `.claude/skills` the Skills tab lists as well.
    pub fn open_page(&self, project: Option<PathBuf>) {
        let mut project_signal = self.project;
        project_signal.set(project);
        let mut visit = self.visit;
        *visit.write() += 1;
        // A file read still on its way belongs to the earlier visit.
        let mut file_reads = self.file_reads;
        file_reads.write().issue();
        let mut detail = self.detail;
        detail.set(None);
        let mut dialog = self.dialog;
        dialog.set(None);
        // A message from an earlier visit is not news any more.
        self.dismiss_error();
        let mut view = self.view;
        view.set(Loaded::Loading);
        // The connectors of the project before are not this project's.
        self.connectors.forget();
        self.reload();
    }

    /// The number of the current visit to the page.
    pub(crate) fn visit_now(&self) -> u64 {
        *self.visit.peek()
    }

    /// Reads the catalogue (plugins, then the skills that depend on it) again. Pinned to the root
    /// scope so leaving the page does not cancel it.
    pub fn reload(&self) {
        self.read_all(None, None);
    }

    // `open` is a skill to show once the lists are fresh (one that was just added).
    pub(crate) fn read_all(&self, open: Option<(String, PathBuf)>, open_connector: Option<String>) {
        let usecase = (*self.usecase.peek()).clone();
        let skills_usecase = (*self.skills_usecase.peek()).clone();
        let connectors_usecase = self.connectors.usecase();
        let project = self.project.peek().clone();
        let this = *self;
        let mut reads = self.reads;
        let token = reads.write().issue();
        let visit = *self.visit.peek();
        dioxus::core::spawn_forever(async move {
            // The connectors come from files: draw them before the plugin catalogue (a call to `claude`) is back.
            // Only when there is nothing drawn yet; a list that is there is not swapped for one without plugins.
            if this.connectors.view.peek().is_none() {
                if let Ok(early) = connectors_usecase.connectors(project.as_deref(), &plugins_for_skills(&Loaded::Loading)).await {
                    if this.reads.peek().is_latest(token) && this.connectors.view.peek().is_none() {
                        let mut slot = this.connectors.view;
                        slot.set(Some(early));
                    }
                }
            }
            let loaded = match usecase.catalog().await {
                Ok(view) => Loaded::Ready(view),
                Err(error) => Loaded::Failed(error.to_string()),
            };
            let skills = skills_usecase.skills(project.as_deref(), &plugins_for_skills(&loaded)).await;
            let connectors = connectors_usecase.connectors(project.as_deref(), &plugins_for_skills(&loaded)).await;
            // A read that was overtaken by a newer one is dropped, so a stale list never replaces a fresh one.
            if this.reads.peek().is_latest(token) {
                let mut view = this.view;
                view.set(loaded);
                match skills {
                    Ok(skills) => {
                        let mut slot = this.skills;
                        slot.set(Some(skills));
                    }
                    Err(error) => {
                        // The old list would pass for current: show the failure instead.
                        let mut list = this.skills;
                        list.set(None);
                        let mut slot = this.error;
                        slot.set(Some(format!("Could not read the skills: {error}")));
                    }
                }
                match connectors {
                    Ok(view) => {
                        let mut slot = this.connectors.view;
                        slot.set(Some(view));
                    }
                    Err(error) => {
                        // The old list would pass for current: show the failure instead.
                        let mut slot = this.connectors.view;
                        slot.set(None);
                        this.set_error(Some(format!("Could not read the connectors: {error}")));
                    }
                }
            }
            // A newer read may have taken the token, but the skill was added all the same: show it,
            // unless the person has left the page and come back since.
            if let Some((id, path)) = open {
                if *this.visit.peek() == visit {
                    this.open_skill(id, path);
                    this.set_skill_pane(SkillPane::Files);
                }
            }
            // A connector that was just added opens once the lists are fresh, unless the person has left and come back.
            if let Some(id) = open_connector {
                if *this.visit.peek() == visit {
                    this.open_connector(id);
                }
            }
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
        self.reset_connection();
        // A file read that is still on its way has no screen to land on.
        let mut reads = self.file_reads;
        reads.write().issue();
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

    pub(crate) fn set_error(&self, message: Option<String>) {
        let mut error = self.error;
        error.set(message);
    }

    pub(crate) fn set_dialog_error(&self, message: Option<String>) {
        let mut error = self.dialog_error;
        error.set(message);
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
        if job.marketplace_name().is_some() {
            let mut error = self.dialog_error;
            error.set(None);
        }
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

    /// Whether an Add marketplace job is running or waiting (the dialog shows a spinner and takes no second one).
    pub fn adding_marketplace(&self) -> bool {
        self.queue.read().adding_marketplace()
    }

    /// Whether an update or remove of this marketplace is running or waiting (the Manage dialog's spinner).
    pub fn busy_marketplace(&self, name: &str) -> Busy {
        let queue = self.queue.read();
        if queue.running_marketplace(name) {
            Busy::Running
        } else if queue.waits_for_marketplace(name) {
            Busy::Waiting
        } else {
            Busy::No
        }
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
        let add_open = self.dialog() == Some(Dialog::AddMarketplace);
        let manage_open = self.dialog() == Some(Dialog::ManageMarketplaces);
        let ok = result.is_ok();
        match result {
            // A success closes the Add dialog, but only if it is still the one that is open.
            Ok(()) => {
                if matches!(job, Job::AddMarketplace(_)) && add_open {
                    self.close_dialog();
                }
            }
            // An error goes where the person is looking: the open dialog's own line, else the page bar.
            Err(message) => {
                let place = if error_in_dialog(&job, add_open, manage_open) { self.dialog_error } else { self.error };
                let mut place = place;
                place.set(Some(message));
            }
        }
        // An uninstalled plugin has no detail to show, and neither has a plugin of a removed marketplace.
        if closes_detail(&job, ok, self.detail().as_deref()) {
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

    pub fn skills(&self) -> Option<SkillView> {
        self.skills.read().clone()
    }

    pub fn skills_busy(&self) -> bool {
        *self.skills_busy.read()
    }

    pub fn skill_pane(&self) -> SkillPane {
        *self.skill_pane.read()
    }

    pub fn set_skill_pane(&self, pane: SkillPane) {
        let mut current = self.skill_pane;
        current.set(pane);
    }

    pub fn skill_files(&self) -> Vec<SkillFile> {
        self.skill_files.read().clone()
    }

    /// Whether the skill has more files than the list shows.
    pub fn skill_files_cut(&self) -> bool {
        *self.skill_files_cut.read()
    }

    pub fn skill_file(&self) -> Option<String> {
        self.skill_file.read().clone()
    }

    pub fn skill_text(&self) -> SkillText {
        self.skill_text.read().clone()
    }

    pub fn skill_source(&self) -> bool {
        *self.skill_source.read()
    }

    pub fn set_skill_source(&self, source: bool) {
        let mut current = self.skill_source;
        current.set(source);
    }

    /// Opens a skill's detail (Overview) and starts reading its files; SKILL.md is selected first.
    pub fn open_skill(&self, id: String, path: PathBuf) {
        let mut detail = self.detail;
        detail.set(Some(id));
        self.set_skill_pane(SkillPane::Overview);
        self.set_skill_source(false);
        let mut dir = self.skill_dir;
        dir.set(Some(path.clone()));
        let mut files = self.skill_files;
        files.set(Vec::new());
        let mut cut = self.skill_files_cut;
        cut.set(false);
        let mut file = self.skill_file;
        file.set(None);
        let mut text = self.skill_text;
        text.set(SkillText::Empty);
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        let mut reads = self.file_reads;
        let token = reads.write().issue();
        dioxus::core::spawn_forever(async move {
            let listed = usecase.files(&path).await;
            if !this.file_reads.peek().is_latest(token) {
                return;
            }
            match listed {
                Ok(SkillFileList { files: listed, truncated }) => {
                    let has_skill_md = listed.iter().any(|file| file.relative == "SKILL.md");
                    let mut cut = this.skill_files_cut;
                    cut.set(truncated);
                    let mut files = this.skill_files;
                    files.set(listed);
                    if has_skill_md {
                        this.select_skill_file("SKILL.md".to_string());
                    }
                }
                Err(error) => {
                    let mut text = this.skill_text;
                    text.set(SkillText::Failed(error.to_string()));
                }
            }
        });
    }

    /// Shows one file of the open skill.
    pub fn select_skill_file(&self, relative: String) {
        let Some(path) = self.skill_dir.peek().clone() else { return };
        let mut file = self.skill_file;
        file.set(Some(relative.clone()));
        let mut text = self.skill_text;
        text.set(SkillText::Loading);
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        let mut reads = self.file_reads;
        let token = reads.write().issue();
        dioxus::core::spawn_forever(async move {
            let result = usecase.read(&path, &relative).await;
            if this.file_reads.peek().is_latest(token) {
                let mut text = this.skill_text;
                text.set(match result {
                    Ok(content) => SkillText::Ready(content),
                    Err(error) => SkillText::Failed(error.to_string()),
                });
            }
        });
    }

    /// Switches a user or project skill on or off. The list is read again afterwards.
    pub fn set_skill_enabled(&self, name: String, on: bool) {
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            match usecase.set_enabled(&name, on).await {
                Ok(()) => {
                    // Show the new state now; the read after it only confirms.
                    let mut slot = this.skills;
                    let mut held = slot.write();
                    if let Some(view) = held.as_mut() {
                        set_skill_enabled_in(view, &name, on);
                    }
                }
                Err(error) => {
                    let mut slot = this.error;
                    slot.set(Some(error.to_string()));
                }
            }
            this.reload();
        });
    }

    /// The Upload skill items: the system picker, then the copy. A cancelled picker does nothing.
    pub fn upload_skill_folder(&self) {
        self.upload(false);
    }

    pub fn upload_skill_zip(&self) {
        self.upload(true);
    }

    fn upload(&self, zip: bool) {
        if self.skills_busy() {
            return;
        }
        let mut busy = self.skills_busy;
        busy.set(true);
        let usecase = (*self.skills_usecase.peek()).clone();
        let folder_picker = (*self.folder_picker.peek()).clone();
        let file_picker = (*self.file_picker.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let outcome = if zip {
                match file_picker.pick_files().await.into_iter().next() {
                    None => None,
                    Some(path) if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) => Some(usecase.add_zip(&path).await),
                    Some(_) => Some(Err(SkillsError::Catalog(forge_ports::SkillCatalogError::Invalid("Choose a .zip file.".to_string())))),
                }
            } else {
                match folder_picker.pick_folder().await {
                    None => None,
                    Some(path) => Some(usecase.add_folder(&path).await),
                }
            };
            this.skill_added(outcome, false);
        });
    }

    /// The Create dialog's button: writes the template, then opens the new skill on its Files tab.
    pub fn create_skill(&self, name: &str, description: &str) {
        if self.skills_busy() {
            return;
        }
        let mut busy = self.skills_busy;
        busy.set(true);
        let mut dialog_error = self.dialog_error;
        dialog_error.set(None);
        let usecase = (*self.skills_usecase.peek()).clone();
        let (name, description) = (name.to_string(), description.to_string());
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let outcome = Some(usecase.create(&name, &description).await);
            this.skill_added(outcome, true);
        });
    }

    // The end of an upload or a create. A success shows the new skill; an error goes where the
    // person is looking (the Create dialog's line, else the page bar).
    fn skill_added(&self, outcome: Option<Result<forge_application::AddedSkill, SkillsError>>, from_dialog: bool) {
        let mut busy = self.skills_busy;
        busy.set(false);
        match outcome {
            None => {}
            Some(Ok(added)) => {
                if self.dialog() == Some(Dialog::CreateSkill) {
                    self.close_dialog();
                }
                if shows_new_skill(self.tab()) {
                    let mut mode = self.mode;
                    mode.set(CustomizeMode::Mine);
                    // The new skill opens on its Files tab, once the lists are fresh.
                    self.read_all(Some((added.id, added.path)), None);
                } else {
                    self.reload();
                }
            }
            Some(Err(error)) => {
                let place = if from_dialog && self.dialog() == Some(Dialog::CreateSkill) { self.dialog_error } else { self.error };
                let mut place = place;
                place.set(Some(error.to_string()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{Marketplace, PluginSnapshot};

    #[test]
    fn a_failed_plugin_read_still_lets_the_skills_be_read_with_no_plugins() {
        let failed = plugins_for_skills(&Loaded::Failed("claude is missing".into()));
        assert_eq!(failed, CatalogView { snapshot: PluginSnapshot::default(), enabled: vec![] });
        assert_eq!(plugins_for_skills(&Loaded::Loading), failed);
    }

    #[test]
    fn a_good_plugin_read_is_passed_on_as_it_is() {
        let view = CatalogView {
            snapshot: PluginSnapshot {
                plugins: vec![],
                marketplaces: vec![Marketplace { name: "m".into(), kind: "git".into(), location: String::new(), last_updated: None }],
            },
            enabled: vec!["a@m".into()],
        };
        assert_eq!(plugins_for_skills(&Loaded::Ready(view.clone())), view);
    }
    #[test]
    fn a_new_skill_is_shown_only_to_a_person_who_is_still_on_the_skills_tab() {
        assert!(shows_new_skill(CustomizeTab::Skills));
        assert!(!shows_new_skill(CustomizeTab::Plugins));
        assert!(!shows_new_skill(CustomizeTab::Connectors));
    }
}
