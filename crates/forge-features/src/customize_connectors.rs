use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{AddedConnector, ConnectorCatalogUseCase, ConnectorView, ConnectorsError};
use forge_domain::{Connection, ConnectorDraft, ConnectorEntry, ConnectorSource};

use crate::customize::{CustomizeMode, CustomizeTab, Latest};
use crate::customize_state::{Customize, Dialog};
use crate::{SignInAction, SignInOutcome, SignInRun, SignInStop, TapHandle};

/// One connector in a list: the entry and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorRow {
    pub entry: ConnectorEntry,
    pub enabled: bool,
}

impl ConnectorRow {
    /// A plugin connector has no switch of its own: it is on when its plugin is.
    pub fn switchable(&self) -> bool {
        !self.entry.source.is_plugin()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorGroups {
    pub mine: Vec<ConnectorRow>,
    pub from_plugins: Vec<ConnectorRow>,
}

fn row(view: &ConnectorView, entry: &ConnectorEntry) -> ConnectorRow {
    let enabled = match entry.source.plugin_id() {
        Some(id) => view.enabled_plugins.iter().any(|on| on == id),
        None => view.enabled.contains(&entry.name),
    };
    ConnectorRow { entry: entry.clone(), enabled }
}

fn matches(entry: &ConnectorEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [entry.name.as_str(), entry.target.as_str(), entry.source.plugin_id().unwrap_or_default()]
            .iter()
            .any(|text| text.to_lowercase().contains(&query))
}

/// The Mine list: installed connectors, the person's own and the ones plugins bring.
pub fn connector_groups(view: &ConnectorView, query: &str) -> ConnectorGroups {
    let mut groups = ConnectorGroups::default();
    for entry in view.snapshot.connectors.iter().filter(|entry| entry.installed && matches(entry, query)) {
        let row = row(view, entry);
        if entry.source.is_plugin() {
            groups.from_plugins.push(row);
        } else {
            groups.mine.push(row);
        }
    }
    groups
}

/// The Explore list: connectors of plugins that a marketplace offers and are not installed.
pub fn explore_connectors(view: &ConnectorView, query: &str) -> Vec<ConnectorRow> {
    view.snapshot.connectors.iter().filter(|entry| !entry.installed && matches(entry, query)).map(|entry| row(view, entry)).collect()
}

pub fn find_connector(view: &ConnectorView, id: &str) -> Option<ConnectorRow> {
    view.snapshot.connectors.iter().find(|entry| entry.id == id).map(|entry| row(view, entry))
}

pub fn source_label(source: &ConnectorSource) -> String {
    match source {
        ConnectorSource::User => "Added by you".to_string(),
        ConnectorSource::Project => "This project (.mcp.json)".to_string(),
        ConnectorSource::Local => "This project, only for you".to_string(),
        ConnectorSource::Plugin { plugin_id } => format!("Plugin {plugin_id}"),
    }
}

/// The "Check connection" result for the open connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,
    Checking,
    Done(Connection),
    /// The check itself could not run (no `claude`, took too long).
    Failed(String),
}

pub fn connection_text(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Idle => String::new(),
        ConnectionState::Checking => "Checking…".to_string(),
        ConnectionState::Done(Connection::Connected) => "Connected".to_string(),
        ConnectionState::Done(Connection::NeedsSignIn) => "Needs sign-in. Use the Sign in button, or run /mcp in the terminal's Claude Code.".to_string(),
        ConnectionState::Done(Connection::Failed(issue)) => format!("Could not connect: {issue}"),
        ConnectionState::Done(Connection::Unknown(text)) => text.clone(),
        ConnectionState::Failed(message) => message.clone(),
    }
}

/// How far the sign-in dialog's run is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignInState {
    Idle,
    Running,
    /// The CLI says it is signed in; the connection is being checked (see `ConnectionState`).
    Signed,
    /// Forge stopped typing: the reason. The terminal is the person's now.
    NeedsHand(String),
    Failed,
}

/// "Sign in" is offered for a connector whose check said it needs sign-in.
pub fn shows_sign_in(state: &ConnectionState) -> bool {
    matches!(state, ConnectionState::Done(Connection::NeedsSignIn))
}

pub fn stop_text(reason: &SignInStop) -> String {
    match reason {
        SignInStop::StepTimedOut(what) => format!("Claude did not show {what}."),
        SignInStop::NameNotFound => "The connector was not found on Claude's list.".to_string(),
        SignInStop::NameTwice => "The name is on Claude's list twice.".to_string(),
        SignInStop::TimedOut => "The sign-in took too long.".to_string(),
    }
}

pub fn sign_in_status(state: &SignInState, connection: &ConnectionState) -> String {
    match state {
        SignInState::Idle => String::new(),
        SignInState::Running => "Signing in… finish it in your browser. If this does not change when you are done, press Close, then Check connection.".to_string(),
        SignInState::Signed => match connection {
            ConnectionState::Idle | ConnectionState::Checking => "Signed in. Checking…".to_string(),
            done => connection_text(done),
        },
        SignInState::NeedsHand(reason) => format!("{reason} Go on in the terminal above."),
        SignInState::Failed => "Sign-in did not finish. See the terminal above.".to_string(),
    }
}

/// Where the sign-in shell starts: project connectors depend on the folder, so the project's, else the home folder.
pub fn sign_in_cwd(project: Option<&Path>, home: Option<PathBuf>) -> PathBuf {
    project.map(Path::to_path_buf).or(home).unwrap_or_else(|| PathBuf::from("."))
}

fn home_folder() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// The names the Add form must not reuse. Without a list there is nothing to check against, and a name used by a
/// project or local connector would slip through: the add waits for the list.
fn taken_names_for_add(view: Option<&ConnectorView>) -> Result<Vec<String>, String> {
    view.map(ConnectorView::taken_names).ok_or_else(|| "The connectors are still loading. Try again in a moment.".to_string())
}

/// Whether a connector that was just added is brought to the person: not when they have gone to another tab,
/// and not when they have left the page and come back since the add began (`then` is the visit it began in).
fn opens_new_connector(tab: CustomizeTab, then: u64, now: u64) -> bool {
    tab == CustomizeTab::Connectors && then == now
}

/// The tag a list row carries for a connector that was checked earlier in this visit and is not fine.
pub fn connection_tag(connection: &Connection) -> Option<&'static str> {
    match connection {
        Connection::NeedsSignIn => Some("needs sign-in"),
        Connection::Failed(_) => Some("not connected"),
        Connection::Connected | Connection::Unknown(_) => None,
    }
}

/// Whether a running connector job holds this dialog open (its backdrop and Cancel do nothing until it ends).
/// Other dialogs are not held by it. The sign-in dialog is always held: its terminal is the person's.
pub fn connector_dialog_busy(dialog: &Dialog, connectors_busy: bool) -> bool {
    matches!(dialog, Dialog::SignInConnector(_)) || (connectors_busy && matches!(dialog, Dialog::AddConnector | Dialog::ConfirmRemoveConnector(_)))
}

/// The Connectors tab's own signals. Owned by the root scope like the rest of the page state.
#[derive(Clone, Copy)]
pub(crate) struct Connectors {
    usecase: Signal<Arc<ConnectorCatalogUseCase>>,
    pub(crate) view: Signal<Option<ConnectorView>>,
    busy: Signal<bool>,
    connection: Signal<ConnectionState>,
    checks: Signal<Latest>,
    /// What checks of this visit found, by name: the list shows a tag for one that is not fine.
    checked: Signal<HashMap<String, Connection>>,
    sign_in: Signal<SignInState>,
    sign_in_tap: Signal<Option<TapHandle>>,
}

impl Connectors {
    pub(crate) fn new(usecase: Arc<ConnectorCatalogUseCase>) -> Self {
        Self {
            usecase: Signal::new_in_scope(usecase, ScopeId::ROOT),
            view: Signal::new_in_scope(None, ScopeId::ROOT),
            busy: Signal::new_in_scope(false, ScopeId::ROOT),
            connection: Signal::new_in_scope(ConnectionState::Idle, ScopeId::ROOT),
            checks: Signal::new_in_scope(Latest::default(), ScopeId::ROOT),
            checked: Signal::new_in_scope(HashMap::new(), ScopeId::ROOT),
            sign_in: Signal::new_in_scope(SignInState::Idle, ScopeId::ROOT),
            sign_in_tap: Signal::new_in_scope(None, ScopeId::ROOT),
        }
    }

    /// A new visit starts with nothing known about earlier checks, and no list of the project before.
    pub(crate) fn forget(&self) {
        let mut checked = self.checked;
        checked.write().clear();
        let mut view = self.view;
        view.set(None);
    }

    pub(crate) fn usecase(&self) -> Arc<ConnectorCatalogUseCase> {
        (*self.usecase.peek()).clone()
    }
}

impl Customize {
    pub fn connectors(&self) -> Option<ConnectorView> {
        self.connectors.view.read().clone()
    }

    pub fn connectors_busy(&self) -> bool {
        *self.connectors.busy.read()
    }

    pub fn connection(&self) -> ConnectionState {
        self.connectors.connection.read().clone()
    }

    /// The tag for a connector that an earlier check of this visit found not fine.
    pub fn checked_tag(&self, name: &str) -> Option<&'static str> {
        self.connectors.checked.read().get(name).and_then(connection_tag)
    }

    /// A late answer of a check belongs to the connector that was open; leaving it drops the answer.
    pub(crate) fn reset_connection(&self) {
        let mut checks = self.connectors.checks;
        checks.write().issue();
        let mut connection = self.connectors.connection;
        connection.set(ConnectionState::Idle);
    }

    pub fn sign_in_state(&self) -> SignInState {
        self.connectors.sign_in.read().clone()
    }

    pub fn sign_in_tap(&self) -> Option<TapHandle> {
        self.connectors.sign_in_tap.read().clone()
    }

    pub fn sign_in_cwd(&self) -> PathBuf {
        sign_in_cwd(self.project.peek().as_deref(), home_folder())
    }

    fn set_sign_in(&self, state: SignInState) {
        let mut sign_in = self.connectors.sign_in;
        sign_in.set(state);
    }

    /// The "Sign in" button: opens the dialog whose terminal Forge types into. A press while the dialog is open does nothing.
    pub fn sign_in_connector(&self, name: String) {
        if matches!(self.dialog(), Some(Dialog::SignInConnector(_))) {
            return;
        }
        let launch = match self.connectors.usecase().launch_line() {
            Ok(line) => line,
            Err(error) => {
                self.set_error(Some(error.to_string()));
                return;
            }
        };
        let this = *self;
        let checked_name = name.clone();
        let run = SignInRun::new(&name, &launch, move |verdict| match verdict {
            SignInAction::Done(SignInOutcome::Signed) => {
                this.set_sign_in(SignInState::Signed);
                this.check_connector(checked_name.clone());
            }
            SignInAction::Done(SignInOutcome::Failed) => this.set_sign_in(SignInState::Failed),
            SignInAction::Stop(reason) => this.set_sign_in(SignInState::NeedsHand(stop_text(&reason))),
            SignInAction::Send(_) => {}
        });
        let mut tap = self.connectors.sign_in_tap;
        tap.set(Some(TapHandle(std::rc::Rc::new(run))));
        self.set_sign_in(SignInState::Running);
        self.open_dialog(Dialog::SignInConnector(name));
    }

    /// Close: ends the shell (the dialog's terminal goes away with it) and forgets the run.
    pub fn close_sign_in(&self) {
        let mut tap = self.connectors.sign_in_tap;
        tap.set(None);
        self.set_sign_in(SignInState::Idle);
        self.close_dialog();
    }

    pub fn open_connector(&self, id: String) {
        self.reset_connection();
        self.open_detail(id);
    }

    pub fn check_connector(&self, name: String) {
        let mut connection = self.connectors.connection;
        connection.set(ConnectionState::Checking);
        let mut checks = self.connectors.checks;
        let token = checks.write().issue();
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = usecase.check(&name).await;
            if let Ok(answer) = &result {
                let mut checked = this.connectors.checked;
                checked.write().insert(name.clone(), answer.clone());
            }
            if this.connectors.checks.peek().is_latest(token) {
                let mut connection = this.connectors.connection;
                connection.set(match result {
                    Ok(answer) => ConnectionState::Done(answer),
                    Err(error) => ConnectionState::Failed(error.to_string()),
                });
            }
        });
    }

    /// Switches a user, project or local connector on or off. The list is read again afterwards.
    pub fn set_connector_enabled(&self, name: String, on: bool) {
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            if let Err(error) = usecase.set_enabled(&name, on).await {
                this.set_error(Some(error.to_string()));
            }
            this.reload();
        });
    }

    /// The Add dialog's button. A second press while one add runs does nothing.
    pub fn add_connector(&self, draft: ConnectorDraft) {
        if self.connectors_busy() {
            return;
        }
        let taken = match taken_names_for_add(self.connectors().as_ref()) {
            Ok(names) => names,
            Err(message) => {
                self.set_dialog_error(Some(message));
                return;
            }
        };
        let mut busy = self.connectors.busy;
        busy.set(true);
        self.set_dialog_error(None);
        let usecase = self.connectors.usecase();
        let this = *self;
        let visit = self.visit_now();
        dioxus::core::spawn_forever(async move {
            let result = usecase.add(&draft, &taken).await;
            this.connector_added(result, visit);
        });
    }

    // A success shows the new connector, but only to a person who is still on this tab. An error goes
    // where the person is looking: the Add dialog's line, else the page bar.
    fn connector_added(&self, result: Result<AddedConnector, ConnectorsError>, visit: u64) {
        let mut busy = self.connectors.busy;
        busy.set(false);
        match result {
            Ok(added) => {
                if self.dialog() == Some(Dialog::AddConnector) {
                    self.close_dialog();
                }
                // A later check of a connector that was removed and added again must not carry the old answer.
                let mut checked = self.connectors.checked;
                checked.write().remove(&added.name);
                if opens_new_connector(self.tab(), visit, self.visit_now()) {
                    self.set_mode(CustomizeMode::Mine);
                    self.read_all(None, Some(added.id));
                } else {
                    self.reload();
                }
            }
            Err(error) => {
                if self.dialog() == Some(Dialog::AddConnector) {
                    self.set_dialog_error(Some(error.to_string()));
                } else {
                    self.set_error(Some(error.to_string()));
                }
            }
        }
    }

    /// The Remove dialog's button.
    pub fn remove_connector(&self, name: String) {
        if self.connectors_busy() {
            self.set_dialog_error(Some("Another change to the connectors is still running. Try again in a moment.".to_string()));
            return;
        }
        let mut busy = self.connectors.busy;
        busy.set(true);
        self.set_dialog_error(None);
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = usecase.remove(&name).await;
            let mut busy = this.connectors.busy;
            busy.set(false);
            let in_dialog = matches!(this.dialog(), Some(Dialog::ConfirmRemoveConnector(_)));
            match result {
                Ok(()) => {
                    let mut checked = this.connectors.checked;
                    checked.write().remove(&name);
                    if in_dialog {
                        this.close_dialog();
                    }
                    this.close_detail();
                }
                Err(error) => {
                    if in_dialog {
                        this.set_dialog_error(Some(error.to_string()));
                    } else {
                        this.set_error(Some(error.to_string()));
                    }
                }
            }
            this.reload();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{entry_from_config, ConnectorSnapshot};
    use serde_json::json;

    fn entry(name: &str, source: ConnectorSource, installed: bool, config: serde_json::Value) -> forge_domain::ConnectorEntry {
        entry_from_config(name, source, installed, &config)
    }

    fn plugin(id: &str) -> ConnectorSource {
        ConnectorSource::Plugin { plugin_id: id.into() }
    }

    fn view() -> ConnectorView {
        ConnectorView {
            snapshot: ConnectorSnapshot {
                notes: vec![],
                connectors: vec![
                    entry("alpha", ConnectorSource::User, true, json!({"type": "http", "url": "https://sentry.example/mcp"})),
                    entry("beta", ConnectorSource::Project, true, json!({"command": "npx"})),
                    entry("gamma", plugin("p@m"), true, json!({"command": "bun"})),
                    entry("delta", plugin("q@m"), false, json!({"command": "bun"})),
                ],
            },
            enabled: vec!["alpha".into()],
            enabled_plugins: vec!["p@m".into()],
            has_marketplaces: true,
        }
    }

    #[test]
    fn mine_has_the_installed_ones_in_two_groups() {
        let groups = connector_groups(&view(), "");
        let names = |rows: &[ConnectorRow]| rows.iter().map(|r| r.entry.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&groups.mine), vec!["alpha", "beta"]);
        assert_eq!(names(&groups.from_plugins), vec!["gamma"]);
    }

    #[test]
    fn a_plugin_connector_is_on_when_its_plugin_is_and_has_no_switch_of_its_own() {
        let groups = connector_groups(&view(), "");
        assert!(groups.mine[0].enabled && groups.mine[0].switchable());
        assert!(!groups.mine[1].enabled);
        assert!(groups.from_plugins[0].enabled && !groups.from_plugins[0].switchable());
    }

    #[test]
    fn explore_has_only_connectors_of_plugins_that_are_not_installed() {
        let cards = explore_connectors(&view(), "");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].entry.name, "delta");
    }

    #[test]
    fn the_search_looks_at_name_address_and_plugin() {
        assert_eq!(connector_groups(&view(), "SENTRY").mine.len(), 1);
        assert_eq!(connector_groups(&view(), "p@m").from_plugins.len(), 1);
        assert!(connector_groups(&view(), "zzz").mine.is_empty());
        assert_eq!(explore_connectors(&view(), "q@m").len(), 1);
    }

    #[test]
    fn find_connector_looks_by_id_in_both_lists() {
        assert_eq!(find_connector(&view(), "user:alpha").unwrap().entry.name, "alpha");
        assert_eq!(find_connector(&view(), "plugin:q@m:delta").unwrap().entry.name, "delta");
        assert!(find_connector(&view(), "user:nope").is_none());
    }

    #[test]
    fn the_source_reads_like_a_sentence_fragment() {
        assert_eq!(source_label(&ConnectorSource::User), "Added by you");
        assert_eq!(source_label(&ConnectorSource::Project), "This project (.mcp.json)");
        assert_eq!(source_label(&ConnectorSource::Local), "This project, only for you");
        assert_eq!(source_label(&plugin("p@m")), "Plugin p@m");
    }

    #[test]
    fn the_connection_line() {
        assert_eq!(connection_text(&ConnectionState::Idle), "");
        assert_eq!(connection_text(&ConnectionState::Checking), "Checking…");
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Connected)), "Connected");
        assert!(connection_text(&ConnectionState::Done(Connection::NeedsSignIn)).contains("/mcp"));
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Failed("refused".into()))), "Could not connect: refused");
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Unknown("odd".into()))), "odd");
        assert_eq!(connection_text(&ConnectionState::Failed("took too long".into())), "took too long");
    }

    #[test]
    fn an_add_waits_for_the_list_so_a_taken_name_cannot_slip_through() {
        assert!(taken_names_for_add(None).unwrap_err().contains("loading"));
        let names = taken_names_for_add(Some(&view())).unwrap();
        assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn a_new_connector_opens_only_for_a_person_who_is_still_on_the_connectors_tab_of_the_same_visit() {
        assert!(opens_new_connector(CustomizeTab::Connectors, 3, 3));
        assert!(!opens_new_connector(CustomizeTab::Connectors, 3, 4), "left the page and came back");
        assert!(!opens_new_connector(CustomizeTab::Plugins, 3, 3));
        assert!(!opens_new_connector(CustomizeTab::Skills, 3, 3));
    }

    #[test]
    fn a_connector_checked_earlier_shows_a_tag_in_the_list_only_when_it_is_not_fine() {
        assert_eq!(connection_tag(&Connection::NeedsSignIn), Some("needs sign-in"));
        assert_eq!(connection_tag(&Connection::Failed("x".into())), Some("not connected"));
        assert_eq!(connection_tag(&Connection::Connected), None);
        assert_eq!(connection_tag(&Connection::Unknown("x".into())), None);
    }

    #[test]
    fn only_the_dialogs_of_a_running_connector_job_hold_the_backdrop() {
        assert!(connector_dialog_busy(&Dialog::AddConnector, true));
        assert!(connector_dialog_busy(&Dialog::ConfirmRemoveConnector("a".into()), true));
        assert!(!connector_dialog_busy(&Dialog::AddConnector, false));
        assert!(!connector_dialog_busy(&Dialog::ManageMarketplaces, true));
        assert!(!connector_dialog_busy(&Dialog::ConfirmUninstall("a".into()), true));
    }

    #[test]
    fn the_sign_in_button_shows_only_for_a_connector_whose_check_said_needs_sign_in() {
        assert!(shows_sign_in(&ConnectionState::Done(Connection::NeedsSignIn)));
        assert!(!shows_sign_in(&ConnectionState::Done(Connection::Connected)));
        assert!(!shows_sign_in(&ConnectionState::Idle));
        assert!(!shows_sign_in(&ConnectionState::Checking));
        assert!(!shows_sign_in(&ConnectionState::Failed("x".into())));
    }

    #[test]
    fn each_stop_reason_says_what_happened_in_a_few_words() {
        assert_eq!(stop_text(&SignInStop::StepTimedOut("the input box")), "Claude did not show the input box.");
        assert_eq!(stop_text(&SignInStop::NameNotFound), "The connector was not found on Claude's list.");
        assert_eq!(stop_text(&SignInStop::NameTwice), "The name is on Claude's list twice.");
        assert_eq!(stop_text(&SignInStop::TimedOut), "The sign-in took too long.");
    }

    #[test]
    fn the_sign_in_status_line_follows_the_run_and_then_the_check() {
        let idle = ConnectionState::Idle;
        assert_eq!(sign_in_status(&SignInState::Idle, &idle), "");
        let running = sign_in_status(&SignInState::Running, &idle);
        assert!(running.contains("browser"));
        assert!(running.contains("Check connection"), "a way out when Forge does not see the end of the sign-in");
        assert_eq!(sign_in_status(&SignInState::Signed, &idle), "Signed in. Checking…");
        assert_eq!(sign_in_status(&SignInState::Signed, &ConnectionState::Checking), "Signed in. Checking…");
        assert_eq!(sign_in_status(&SignInState::Signed, &ConnectionState::Done(Connection::Connected)), "Connected");
        assert_eq!(
            sign_in_status(&SignInState::NeedsHand("Claude did not show the input box.".into()), &idle),
            "Claude did not show the input box. Go on in the terminal above."
        );
        assert_eq!(sign_in_status(&SignInState::Failed, &idle), "Sign-in did not finish. See the terminal above.");
    }

    #[test]
    fn the_shell_starts_in_the_project_folder_else_the_home_folder_else_here() {
        assert_eq!(sign_in_cwd(Some(Path::new("/p")), Some(PathBuf::from("/h"))), PathBuf::from("/p"));
        assert_eq!(sign_in_cwd(None, Some(PathBuf::from("/h"))), PathBuf::from("/h"));
        assert_eq!(sign_in_cwd(None, None), PathBuf::from("."));
    }

    #[test]
    fn the_sign_in_dialog_is_never_closed_by_its_backdrop() {
        assert!(connector_dialog_busy(&Dialog::SignInConnector("web".into()), false));
        assert!(!connector_dialog_busy(&Dialog::AddConnector, false));
    }
}
