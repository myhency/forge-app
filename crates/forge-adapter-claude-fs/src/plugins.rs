use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::PluginDefinition;
use forge_ports::{PluginRepositoryError, PluginRepositoryPort};
use serde::Deserialize;

pub struct ClaudePluginAdapter {
    claude_home: PathBuf,
}

impl ClaudePluginAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[derive(Deserialize)]
struct InstalledPlugins {
    #[serde(default)]
    plugins: HashMap<String, Vec<InstalledPlugin>>,
}

#[derive(Deserialize)]
struct InstalledPlugin {
    #[serde(default, rename = "installPath")]
    install_path: Option<PathBuf>,
}

#[async_trait]
impl PluginRepositoryPort for ClaudePluginAdapter {
    async fn list_plugins(&self) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        let path = self
            .claude_home
            .join("plugins")
            .join("installed_plugins.json");
        let contents = match tokio::fs::read_to_string(&path).await {
            Ok(contents) => contents,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(PluginRepositoryError::ListFailed(format!(
                    "{}: {}",
                    path.display(),
                    e
                )))
            }
        };
        let installed: InstalledPlugins = serde_json::from_str(&contents).map_err(|e| {
            PluginRepositoryError::ListFailed(format!("{}: {}", path.display(), e))
        })?;

        let mut plugins = Vec::new();
        for (key, installs) in installed.plugins {
            let (name, marketplace) = match key.split_once('@') {
                Some((name, marketplace)) => (name.to_string(), Some(marketplace.to_string())),
                None => (key.clone(), None),
            };
            for install in installs {
                if let Some(install_path) = install.install_path {
                    plugins.push(PluginDefinition {
                        name: name.clone(),
                        marketplace: marketplace.clone(),
                        path: install_path,
                    });
                }
            }
        }
        plugins.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then_with(|| a.marketplace.cmp(&b.marketplace))
                .then_with(|| a.path.cmp(&b.path))
        });
        Ok(plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_registry(home: &Path, json: &str) {
        let dir = home.join("plugins");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("installed_plugins.json"), json).unwrap();
    }

    async fn list(home: &Path) -> Result<Vec<PluginDefinition>, PluginRepositoryError> {
        ClaudePluginAdapter::new(home.to_path_buf()).list_plugins().await
    }

    #[tokio::test]
    async fn list_plugins_returns_empty_when_the_registry_file_is_missing() {
        let home = tempfile::tempdir().unwrap();

        let plugins = list(home.path()).await.unwrap();

        assert!(plugins.is_empty());
    }

    #[tokio::test]
    async fn list_plugins_splits_name_and_marketplace_ignores_extra_fields_and_sorts_by_name() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{
                "version": 2,
                "plugins": {
                    "frontend-design@claude-plugins-official": [{
                        "scope": "user",
                        "installPath": "/plugins/frontend-design/fa59",
                        "version": "fa59",
                        "installedAt": "2026-02-20T18:35:42.344Z",
                        "lastUpdated": "2026-09-27T07:32:47.339Z",
                        "gitCommitSha": "fa59bc9037741ecfa131aa27938272605710d7b2"
                    }],
                    "andrej-karpathy-skills@karpathy-skills": [{
                        "scope": "user",
                        "installPath": "/plugins/karpathy/1.0.0",
                        "version": "1.0.0",
                        "installedAt": "2026-02-17T04:26:33.277Z",
                        "lastUpdated": "2026-02-17T04:26:33.277Z",
                        "gitCommitSha": "aa4467f0b33e1e80d11c7c043d4b27e7c79a73a3"
                    }]
                }
            }"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(
            plugins,
            vec![
                PluginDefinition {
                    name: "andrej-karpathy-skills".to_string(),
                    marketplace: Some("karpathy-skills".to_string()),
                    path: PathBuf::from("/plugins/karpathy/1.0.0"),
                },
                PluginDefinition {
                    name: "frontend-design".to_string(),
                    marketplace: Some("claude-plugins-official".to_string()),
                    path: PathBuf::from("/plugins/frontend-design/fa59"),
                },
            ]
        );
    }

    #[tokio::test]
    async fn list_plugins_emits_one_definition_per_install_entry() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"tool@market": [
                {"scope": "project", "installPath": "/b"},
                {"scope": "user", "installPath": "/a"}
            ]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        let paths: Vec<&PathBuf> = plugins.iter().map(|p| &p.path).collect();
        assert_eq!(paths, vec![&PathBuf::from("/a"), &PathBuf::from("/b")]);
        assert!(plugins.iter().all(|p| p.name == "tool" && p.marketplace.as_deref() == Some("market")));
    }

    #[tokio::test]
    async fn list_plugins_treats_a_key_without_an_at_sign_as_a_name_with_no_marketplace() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"local-plugin": [{"installPath": "/local"}]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(
            plugins,
            vec![PluginDefinition {
                name: "local-plugin".to_string(),
                marketplace: None,
                path: PathBuf::from("/local"),
            }]
        );
    }

    #[tokio::test]
    async fn list_plugins_skips_an_install_entry_without_an_install_path() {
        let home = tempfile::tempdir().unwrap();
        write_registry(
            home.path(),
            r#"{"version": 2, "plugins": {"tool@market": [{"scope": "user"}, {"installPath": "/ok"}]}}"#,
        );

        let plugins = list(home.path()).await.unwrap();

        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].path, PathBuf::from("/ok"));
    }

    #[tokio::test]
    async fn list_plugins_fails_on_malformed_json_and_names_the_file() {
        let home = tempfile::tempdir().unwrap();
        write_registry(home.path(), "{not json");

        let result = list(home.path()).await;

        match result {
            Err(PluginRepositoryError::ListFailed(message)) => {
                assert!(message.contains("installed_plugins.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_plugins_returns_empty_when_the_plugins_map_is_missing() {
        let home = tempfile::tempdir().unwrap();
        write_registry(home.path(), r#"{"version": 2}"#);

        let plugins = list(home.path()).await.unwrap();

        assert!(plugins.is_empty());
    }
}
