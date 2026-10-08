use std::sync::Arc;

use forge_domain::{split_plugin_id, AppConfig, PluginSnapshot};
use forge_ports::{ConfigError, ConfigRepositoryPort, PluginManagerError, PluginManagerPort};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginCatalogError {
    #[error("{0}")]
    Manager(#[from] PluginManagerError),
    #[error("Could not save the Forge settings: {0}")]
    Config(#[from] ConfigError),
}

/// What the Customize page draws: the plugins and marketplaces, and which plugins Forge has switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogView {
    pub snapshot: PluginSnapshot,
    pub enabled: Vec<String>,
}

/// Installing, uninstalling and finding plugins. The install itself belongs to the bundled Claude
/// Code (shared with the terminal); whether Forge's sessions use a plugin is Forge's own
/// `enabled_plugins` list, which this keeps in step.
pub struct PluginCatalogUseCase {
    manager: Arc<dyn PluginManagerPort>,
    config: Arc<dyn ConfigRepositoryPort>,
}

impl PluginCatalogUseCase {
    pub fn new(manager: Arc<dyn PluginManagerPort>, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { manager, config }
    }

    pub async fn catalog(&self) -> Result<CatalogView, PluginCatalogError> {
        let snapshot = self.manager.snapshot().await?;
        let enabled = self.config.load().await?.enabled_plugins;
        Ok(CatalogView { snapshot, enabled })
    }

    /// Installs, then switches the plugin on in Forge. If the install fails nothing else changes.
    pub async fn install(&self, id: &str) -> Result<(), PluginCatalogError> {
        self.manager.install(id).await?;
        self.edit_enabled(|list| {
            if !list.iter().any(|key| key == id) {
                list.push(id.to_string());
            }
        })
        .await
    }

    pub async fn uninstall(&self, id: &str) -> Result<(), PluginCatalogError> {
        self.manager.uninstall(id).await?;
        self.edit_enabled(|list| list.retain(|key| key != id)).await
    }

    /// Switches a plugin on or off in Forge only (Claude Code's own `enabledPlugins` is not touched).
    pub async fn set_enabled(&self, id: &str, on: bool) -> Result<(), PluginCatalogError> {
        self.edit_enabled(|list| {
            let present = list.iter().any(|key| key == id);
            if on && !present {
                list.push(id.to_string());
            } else if !on {
                list.retain(|key| key != id);
            }
        })
        .await
    }

    pub async fn add_marketplace(&self, source: &str) -> Result<(), PluginCatalogError> {
        Ok(self.manager.add_marketplace(source).await?)
    }

    /// Removing a marketplace removes its plugins too, so their keys leave the enabled list.
    pub async fn remove_marketplace(&self, name: &str) -> Result<(), PluginCatalogError> {
        self.manager.remove_marketplace(name).await?;
        self.edit_enabled(|list| list.retain(|key| split_plugin_id(key).1 != Some(name))).await
    }

    pub async fn update_marketplace(&self, name: &str) -> Result<(), PluginCatalogError> {
        Ok(self.manager.update_marketplace(name).await?)
    }

    // Loads, edits and saves the list; saves only when it changed.
    async fn edit_enabled(&self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), PluginCatalogError> {
        let mut config: AppConfig = self.config.load().await?;
        let before = config.enabled_plugins.clone();
        edit(&mut config.enabled_plugins);
        if config.enabled_plugins != before {
            self.config.save(&config).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{AppConfig, PluginSnapshot};
    use forge_ports::{MockConfigRepositoryPort, MockPluginManagerPort};
    use std::sync::Mutex as StdMutex;

    // A config repository that keeps one AppConfig in memory and counts saves.
    fn memory_config(enabled: &[&str]) -> (Arc<MockConfigRepositoryPort>, Arc<StdMutex<AppConfig>>, Arc<StdMutex<u32>>) {
        let state = Arc::new(StdMutex::new(AppConfig {
            enabled_plugins: enabled.iter().map(|s| s.to_string()).collect(),
            ..AppConfig::default()
        }));
        let saves = Arc::new(StdMutex::new(0));
        let mut mock = MockConfigRepositoryPort::new();
        let loaded = state.clone();
        mock.expect_load().returning(move || Ok(loaded.lock().unwrap().clone()));
        let stored = state.clone();
        let counted = saves.clone();
        mock.expect_save().returning(move |config| {
            *stored.lock().unwrap() = config.clone();
            *counted.lock().unwrap() += 1;
            Ok(())
        });
        (Arc::new(mock), state, saves)
    }

    fn use_case(manager: MockPluginManagerPort, config: Arc<MockConfigRepositoryPort>) -> PluginCatalogUseCase {
        PluginCatalogUseCase::new(Arc::new(manager), config)
    }

    #[tokio::test]
    async fn the_catalog_is_the_snapshot_and_the_enabled_list() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_snapshot().returning(|| Ok(PluginSnapshot::default()));
        let (config, _, _) = memory_config(&["a@m"]);
        let view = use_case(manager, config).catalog().await.unwrap();
        assert_eq!(view.enabled, vec!["a@m".to_string()]);
        assert_eq!(view.snapshot, PluginSnapshot::default());
    }

    #[tokio::test]
    async fn an_install_switches_the_plugin_on_in_forge() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().withf(|id| id == "hello@m").times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["other@m"]);
        use_case(manager, config).install("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m", "hello@m"]);
    }

    #[tokio::test]
    async fn an_install_that_fails_leaves_the_enabled_list_alone() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().returning(|_| Err(PluginManagerError::Cli("clone failed".into())));
        let (config, state, saves) = memory_config(&["other@m"]);
        let error = use_case(manager, config).install("hello@m").await.unwrap_err();
        assert_eq!(error.to_string(), "clone failed");
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m"]);
        assert_eq!(*saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn an_install_of_a_plugin_that_is_already_switched_on_does_not_duplicate_it() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["hello@m"]);
        use_case(manager, config).install("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["hello@m"]);
    }

    #[tokio::test]
    async fn an_uninstall_takes_the_plugin_out_of_the_enabled_list() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_uninstall().times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["hello@m", "other@m"]);
        use_case(manager, config).uninstall("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m"]);
    }

    #[tokio::test]
    async fn an_uninstall_that_fails_leaves_the_enabled_list_alone() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_uninstall().returning(|_| Err(PluginManagerError::TimedOut));
        let (config, state, saves) = memory_config(&["hello@m"]);
        assert!(use_case(manager, config).uninstall("hello@m").await.is_err());
        assert_eq!(state.lock().unwrap().enabled_plugins, ["hello@m"]);
        assert_eq!(*saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn switching_on_and_off_is_explicit_and_never_touches_the_manager() {
        let manager = MockPluginManagerPort::new(); // any call to it would panic
        let (config, state, _) = memory_config(&[]);
        let use_case = use_case(manager, config);
        use_case.set_enabled("a@m", true).await.unwrap();
        use_case.set_enabled("a@m", true).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["a@m"]);
        use_case.set_enabled("a@m", false).await.unwrap();
        use_case.set_enabled("a@m", false).await.unwrap();
        assert!(state.lock().unwrap().enabled_plugins.is_empty());
    }

    #[tokio::test]
    async fn removing_a_marketplace_takes_every_enabled_key_of_its_plugins_out() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_remove_marketplace().withf(|name| name == "m").times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["a@m", "b@m", "c@other", "m@other"]);
        use_case(manager, config).remove_marketplace("m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["c@other", "m@other"]);
    }

    #[tokio::test]
    async fn adding_and_updating_a_marketplace_change_nothing_in_forge() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_add_marketplace().returning(|_| Ok(()));
        manager.expect_update_marketplace().returning(|_| Ok(()));
        let (config, _, saves) = memory_config(&["a@m"]);
        let use_case = use_case(manager, config);
        use_case.add_marketplace("owner/repo").await.unwrap();
        use_case.update_marketplace("m").await.unwrap();
        assert_eq!(*saves.lock().unwrap(), 0);
    }
}
