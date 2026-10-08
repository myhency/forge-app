use std::path::Path;
use std::sync::Arc;

use super::config_lock::ConfigLock;
use super::plugin_catalog::CatalogView;

use forge_domain::{connector_id, validate_new, AppConfig, Connection, ConnectorDraft, ConnectorSnapshot, ConnectorSource};
use forge_ports::{ConfigError, ConfigRepositoryPort, ConnectorCatalogError, ConnectorCatalogPort, McpManagerError, McpManagerPort};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectorsError {
    #[error("{0}")]
    Catalog(#[from] ConnectorCatalogError),
    #[error("{0}")]
    Manager(#[from] McpManagerError),
    #[error("Could not read or save the Forge settings: {0}")]
    Config(#[from] ConfigError),
    /// The form is wrong: the message tells the person what to fix.
    #[error("{0}")]
    Invalid(String),
}

/// What the Connectors tab draws: the connectors, and what Forge has switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorView {
    pub snapshot: ConnectorSnapshot,
    /// Names of user, project and local connectors that are on.
    pub enabled: Vec<String>,
    /// Ids of plugins that are on; a plugin connector is on when its plugin is.
    pub enabled_plugins: Vec<String>,
    /// Whether any marketplace was added (Explore has something to look at).
    pub has_marketplaces: bool,
}

impl ConnectorView {
    /// Names the Add form must not reuse: every connector but the ones plugins bring.
    pub fn taken_names(&self) -> Vec<String> {
        self.snapshot.connectors.iter().filter(|entry| !entry.source.is_plugin()).map(|entry| entry.name.clone()).collect()
    }
}

/// A connector that was just added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedConnector {
    pub name: String,
    /// `connector_id` of the user connector: what the page opens.
    pub id: String,
}

/// Finding, adding, removing and switching connectors. The servers belong to the bundled Claude Code
/// (shared with the terminal); whether Forge's sessions use one is Forge's own `enabled_mcp_servers`
/// list, which this keeps in step.
pub struct ConnectorCatalogUseCase {
    catalog: Arc<dyn ConnectorCatalogPort>,
    manager: Arc<dyn McpManagerPort>,
    config: Arc<dyn ConfigRepositoryPort>,
    lock: ConfigLock,
}

impl ConnectorCatalogUseCase {
    pub fn new(catalog: Arc<dyn ConnectorCatalogPort>, manager: Arc<dyn McpManagerPort>, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { catalog, manager, config, lock: ConfigLock::default() }
    }

    /// Shares the lock the other config writers hold (see `ConfigLock`).
    pub fn with_config_lock(mut self, lock: ConfigLock) -> Self {
        self.lock = lock;
        self
    }

    /// `plugins` is the plugin catalogue the page has just read, so the bundled `claude` is not asked twice.
    pub async fn connectors(&self, project: Option<&Path>, plugins: &CatalogView) -> Result<ConnectorView, ConnectorsError> {
        let snapshot = self.catalog.snapshot(project.map(Path::to_path_buf), plugins.snapshot.plugins.clone()).await?;
        let enabled = self.config.load().await?.enabled_mcp_servers;
        Ok(ConnectorView {
            snapshot,
            enabled,
            enabled_plugins: plugins.enabled.clone(),
            has_marketplaces: !plugins.snapshot.marketplaces.is_empty(),
        })
    }

    /// Switches a user, project or local connector on or off in Forge.
    pub async fn set_enabled(&self, name: &str, on: bool) -> Result<(), ConnectorsError> {
        self.edit_enabled(|list| {
            let present = list.iter().any(|key| key == name);
            if on && !present {
                list.push(name.to_string());
            } else if !on {
                list.retain(|key| key != name);
            }
        })
        .await
    }

    /// Checks the form, adds the server in user scope, then switches it on. If the CLI fails nothing else changes.
    pub async fn add(&self, draft: &ConnectorDraft, taken: &[String]) -> Result<AddedConnector, ConnectorsError> {
        let connector = validate_new(draft, taken).map_err(ConnectorsError::Invalid)?;
        self.manager.add(&connector).await?;
        self.set_enabled(&connector.name, true).await?;
        Ok(AddedConnector { id: connector_id(&ConnectorSource::User, &connector.name), name: connector.name })
    }

    /// Removes the user-scope server, then its name leaves the enabled list, so a later connector of the same name starts off.
    pub async fn remove(&self, name: &str) -> Result<(), ConnectorsError> {
        self.manager.remove(name).await?;
        self.edit_enabled(|list| list.retain(|key| key != name)).await
    }

    pub async fn check(&self, name: &str) -> Result<Connection, ConnectorsError> {
        Ok(self.manager.check(name).await?)
    }

    // Loads, edits and saves the list; saves only when it changed.
    async fn edit_enabled(&self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), ConnectorsError> {
        let _held = self.lock.hold().await;
        let mut config: AppConfig = self.config.load().await?;
        let before = config.enabled_mcp_servers.clone();
        edit(&mut config.enabled_mcp_servers);
        if config.enabled_mcp_servers != before {
            self.config.save(&config).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{entry_from_config, ConnectorKind, ConnectorSource, Marketplace, NewTransport, PluginSnapshot};
    use forge_ports::{MockConfigRepositoryPort, MockConnectorCatalogPort, MockMcpManagerPort};
    use std::sync::Mutex as StdMutex;

    fn memory_config(enabled: &[&str]) -> (Arc<MockConfigRepositoryPort>, Arc<StdMutex<AppConfig>>) {
        let state = Arc::new(StdMutex::new(AppConfig { enabled_mcp_servers: enabled.iter().map(|s| s.to_string()).collect(), ..AppConfig::default() }));
        let mut mock = MockConfigRepositoryPort::new();
        let loaded = state.clone();
        mock.expect_load().returning(move || Ok(loaded.lock().unwrap().clone()));
        let stored = state.clone();
        mock.expect_save().returning(move |config| {
            *stored.lock().unwrap() = config.clone();
            Ok(())
        });
        (Arc::new(mock), state)
    }

    fn use_case(catalog: MockConnectorCatalogPort, manager: MockMcpManagerPort, config: Arc<MockConfigRepositoryPort>) -> ConnectorCatalogUseCase {
        ConnectorCatalogUseCase::new(Arc::new(catalog), Arc::new(manager), config)
    }

    fn plugin_view(enabled: &[&str], markets: usize) -> CatalogView {
        CatalogView {
            snapshot: PluginSnapshot {
                plugins: vec![],
                marketplaces: (0..markets).map(|i| Marketplace { name: format!("m{i}"), kind: "git".into(), location: String::new(), last_updated: None }).collect(),
            },
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn http_draft(name: &str) -> ConnectorDraft {
        ConnectorDraft { name: name.into(), kind: ConnectorKind::Http, url: "https://a.example/mcp".into(), ..ConnectorDraft::default() }
    }

    #[tokio::test]
    async fn the_view_joins_the_snapshot_with_both_enabled_lists() {
        let mut catalog = MockConnectorCatalogPort::new();
        let entry = entry_from_config("web", ConnectorSource::User, true, &serde_json::json!({"url": "https://a"}));
        let returned = entry.clone();
        catalog
            .expect_snapshot()
            .withf(|project, _| project.as_deref() == Some(Path::new("/work")))
            .returning(move |_, _| Ok(ConnectorSnapshot { connectors: vec![returned.clone()] }));
        let (config, _) = memory_config(&["web"]);
        let view = use_case(catalog, MockMcpManagerPort::new(), config)
            .connectors(Some(Path::new("/work")), &plugin_view(&["p@m"], 1))
            .await
            .unwrap();
        assert_eq!(view.snapshot.connectors, vec![entry]);
        assert_eq!(view.enabled, vec!["web".to_string()]);
        assert_eq!(view.enabled_plugins, vec!["p@m".to_string()]);
        assert!(view.has_marketplaces);
    }

    #[test]
    fn taken_names_are_every_name_but_a_plugins() {
        let at = |name: &str, source| entry_from_config(name, source, true, &serde_json::json!({"command": "x"}));
        let view = ConnectorView {
            snapshot: ConnectorSnapshot {
                connectors: vec![at("a", ConnectorSource::User), at("b", ConnectorSource::Project), at("c", ConnectorSource::Plugin { plugin_id: "p@m".into() })],
            },
            enabled: vec![],
            enabled_plugins: vec![],
            has_marketplaces: false,
        };
        assert_eq!(view.taken_names(), vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn add_checks_the_form_first_and_the_cli_is_not_asked_when_it_is_wrong() {
        let (config, _) = memory_config(&[]);
        let uc = use_case(MockConnectorCatalogPort::new(), MockMcpManagerPort::new(), config); // no expectations: any call would panic
        let error = uc.add(&http_draft("a b"), &[]).await.unwrap_err();
        assert!(matches!(error, ConnectorsError::Invalid(_)));
        let error = uc.add(&http_draft("web"), &["web".to_string()]).await.unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error}");
    }

    #[tokio::test]
    async fn add_runs_the_cli_then_switches_the_connector_on_in_forge() {
        let mut manager = MockMcpManagerPort::new();
        manager
            .expect_add()
            .withf(|connector| connector.name == "web" && matches!(&connector.transport, NewTransport::Http { url, .. } if url == "https://a.example/mcp"))
            .times(1)
            .returning(|_| Ok(()));
        let (config, state) = memory_config(&[]);
        let added = use_case(MockConnectorCatalogPort::new(), manager, config).add(&http_draft("web"), &[]).await.unwrap();
        assert_eq!(added, AddedConnector { name: "web".into(), id: "user:web".into() });
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["web".to_string()]);
    }

    #[tokio::test]
    async fn a_failed_add_leaves_the_forge_settings_alone() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_add().returning(|_| Err(McpManagerError::Cli("nope".into())));
        let (config, state) = memory_config(&[]);
        let error = use_case(MockConnectorCatalogPort::new(), manager, config).add(&http_draft("web"), &[]).await.unwrap_err();
        assert_eq!(error.to_string(), "nope");
        assert!(state.lock().unwrap().enabled_mcp_servers.is_empty());
    }

    #[tokio::test]
    async fn remove_runs_the_cli_and_drops_the_name_so_a_new_connector_of_that_name_starts_off() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_remove().withf(|name| name == "web").times(1).returning(|_| Ok(()));
        let (config, state) = memory_config(&["web", "other"]);
        use_case(MockConnectorCatalogPort::new(), manager, config).remove("web").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["other".to_string()]);
    }

    #[tokio::test]
    async fn a_failed_remove_keeps_the_switch() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_remove().returning(|_| Err(McpManagerError::Cli("No MCP server named \"web\" in user scope".into())));
        let (config, state) = memory_config(&["web"]);
        assert!(use_case(MockConnectorCatalogPort::new(), manager, config).remove("web").await.is_err());
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["web".to_string()]);
    }

    #[tokio::test]
    async fn set_enabled_saves_only_when_the_list_changes() {
        let (config, state) = memory_config(&["a"]);
        let uc = use_case(MockConnectorCatalogPort::new(), MockMcpManagerPort::new(), config);
        uc.set_enabled("a", true).await.unwrap();
        uc.set_enabled("b", true).await.unwrap();
        uc.set_enabled("a", false).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn check_passes_the_answer_on() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_check().returning(|_| Ok(Connection::NeedsSignIn));
        let (config, _) = memory_config(&[]);
        assert_eq!(use_case(MockConnectorCatalogPort::new(), manager, config).check("web").await.unwrap(), Connection::NeedsSignIn);
    }
}
