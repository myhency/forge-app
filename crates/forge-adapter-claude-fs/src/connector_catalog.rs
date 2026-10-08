use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{entry_from_config, plugin_id, ConnectorEntry, ConnectorSnapshot, ConnectorSource, PluginEntry};
use forge_ports::{ConnectorCatalogError, ConnectorCatalogPort};
use serde_json::Value;

use crate::mcp::{local_servers, normalize_project_servers, read_optional};
use crate::skill_catalog::{local_plugin_dir, read_json, KnownMarketplace, MarketplaceFile, OfferedPlugin};

/// Reads the connectors the Customize page lists from the files Claude Code keeps. Writes nothing.
pub struct ClaudeConnectorCatalogAdapter {
    claude_json_path: PathBuf,
    claude_home: PathBuf,
}

impl ClaudeConnectorCatalogAdapter {
    pub fn new(claude_json_path: PathBuf, claude_home: PathBuf) -> Self {
        Self { claude_json_path, claude_home }
    }
}

fn io(path: &Path, reason: impl std::fmt::Display) -> ConnectorCatalogError {
    ConnectorCatalogError::Io(format!("{}: {}", path.display(), reason))
}

// `file` must lie inside `dir` once links are followed: a plugin is somebody else's folder.
async fn inside(dir: &Path, file: &Path) -> bool {
    let (Ok(dir), Ok(file)) = (tokio::fs::canonicalize(dir).await, tokio::fs::canonicalize(file).await) else {
        return false;
    };
    file.starts_with(dir)
}

async fn read_servers_file(dir: &Path, file: &Path) -> Option<BTreeMap<String, Value>> {
    if !inside(dir, file).await {
        return None;
    }
    let text = tokio::fs::read_to_string(file).await.ok()?;
    let parsed: Value = serde_json::from_str(&text).ok()?;
    Some(normalize_project_servers(parsed).ok()?.into_iter().collect())
}

/// The servers a plugin folder bundles: its `.mcp.json`, and `mcpServers` of its manifest (an object, or a
/// path inside the plugin). Anything unreadable is skipped.
async fn bundled_servers(dir: &Path) -> Vec<(String, Value)> {
    let mut out: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(servers) = read_servers_file(dir, &dir.join(".mcp.json")).await {
        out.extend(servers);
    }
    if let Some(manifest) = read_json::<Value>(&dir.join(".claude-plugin").join("plugin.json")).await {
        match manifest.get("mcpServers") {
            Some(Value::Object(servers)) => out.extend(servers.iter().map(|(name, config)| (name.clone(), config.clone()))),
            Some(Value::String(relative)) => {
                if let Some(path) = local_plugin_dir(dir, &Value::String(relative.clone())) {
                    if let Some(servers) = read_servers_file(dir, &path).await {
                        out.extend(servers);
                    }
                }
            }
            _ => {}
        }
    }
    out.into_iter().filter(|(_, config)| config.is_object()).collect()
}

impl ClaudeConnectorCatalogAdapter {
    async fn read_snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError> {
        // Later insertions win by name: user, then the project's .mcp.json, then local scope (as the SDK does).
        let mut mine: BTreeMap<String, (ConnectorSource, Value)> = BTreeMap::new();
        let claude_json = read_optional(&self.claude_json_path).await.map_err(|e| io(&self.claude_json_path, e))?;
        if let Some(text) = &claude_json {
            let parsed: Value = serde_json::from_str(text).map_err(|e| io(&self.claude_json_path, e))?;
            if let Some(Value::Object(servers)) = parsed.get("mcpServers") {
                for (name, config) in servers {
                    mine.insert(name.clone(), (ConnectorSource::User, config.clone()));
                }
            }
        }
        if let Some(project) = &project_path {
            let file = project.join(".mcp.json");
            if let Some(text) = read_optional(&file).await.map_err(|e| io(&file, e))? {
                let parsed: Value = serde_json::from_str(&text).map_err(|e| io(&file, e))?;
                for (name, config) in normalize_project_servers(parsed).map_err(|reason| io(&file, reason))? {
                    mine.insert(name, (ConnectorSource::Project, config));
                }
            }
            if let Some(text) = &claude_json {
                let local: HashMap<String, Value> = local_servers(text, project).await;
                for (name, config) in local {
                    mine.insert(name, (ConnectorSource::Local, config));
                }
            }
        }
        let mut connectors: Vec<ConnectorEntry> = mine
            .into_iter()
            .filter(|(_, (_, config))| config.is_object())
            .map(|(name, (source, config))| entry_from_config(&name, source, true, &config))
            .collect();

        let mut seen: HashSet<String> = HashSet::new();
        for plugin in plugins.iter().filter(|plugin| plugin.installed) {
            if let Some(path) = &plugin.install_path {
                for (name, config) in bundled_servers(path).await {
                    let source = ConnectorSource::Plugin { plugin_id: plugin.id.clone() };
                    connectors.push(entry_from_config(&name, source, true, &config));
                }
                seen.insert(plugin.id.clone());
            }
        }
        let known: HashMap<String, KnownMarketplace> =
            read_json(&self.claude_home.join("plugins").join("known_marketplaces.json")).await.unwrap_or_default();
        for (market, info) in known {
            let Some(dir) = info.install_location else { continue };
            let Some(file) = read_json::<MarketplaceFile>(&dir.join(".claude-plugin").join("marketplace.json")).await else {
                continue;
            };
            for offered in file.plugins.into_iter().filter_map(|entry| serde_json::from_value::<OfferedPlugin>(entry).ok()) {
                let id = plugin_id(&offered.name, &market);
                if seen.contains(&id) || plugins.iter().any(|plugin| plugin.installed && plugin.id == id) {
                    continue;
                }
                let Some(plugin_dir) = local_plugin_dir(&dir, &offered.source) else { continue };
                if !inside(&dir, &plugin_dir).await {
                    continue;
                }
                for (name, config) in bundled_servers(&plugin_dir).await {
                    connectors.push(entry_from_config(&name, ConnectorSource::Plugin { plugin_id: id.clone() }, false, &config));
                }
            }
        }
        connectors.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        Ok(ConnectorSnapshot { connectors })
    }
}

#[async_trait]
impl ConnectorCatalogPort for ClaudeConnectorCatalogAdapter {
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError> {
        self.read_snapshot(project_path, plugins).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{ConnectorKind, ConnectorSource};
    use serde_json::json;

    struct Fixture {
        _dir: tempfile::TempDir,
        home: PathBuf,
        claude_json: PathBuf,
        project: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().join(".claude");
        std::fs::create_dir_all(home.join("plugins")).unwrap();
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        Fixture { claude_json: dir.path().join(".claude.json"), home, project, _dir: dir }
    }

    async fn snapshot(f: &Fixture, project: bool, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError> {
        ClaudeConnectorCatalogAdapter::new(f.claude_json.clone(), f.home.clone())
            .snapshot(project.then(|| f.project.clone()), plugins)
            .await
    }

    fn names(s: &ConnectorSnapshot) -> Vec<(String, String)> {
        s.connectors.iter().map(|c| (c.id.clone(), c.name.clone())).collect()
    }

    fn plugin(id: &str, installed: bool, path: Option<&Path>) -> PluginEntry {
        let (name, market) = id.split_once('@').unwrap();
        PluginEntry {
            id: id.into(),
            name: name.into(),
            marketplace: market.into(),
            version: None,
            description: String::new(),
            installed,
            scope: Some("user".into()),
            install_path: path.map(Path::to_path_buf),
        }
    }

    #[tokio::test]
    async fn no_files_means_no_connectors() {
        let f = fixture();
        assert!(snapshot(&f, true, vec![]).await.unwrap().connectors.is_empty());
    }

    #[tokio::test]
    async fn user_project_and_local_servers_carry_their_source() {
        let f = fixture();
        let key = f.project.to_string_lossy().into_owned();
        std::fs::write(
            &f.claude_json,
            json!({
                "mcpServers": {"u": {"type": "http", "url": "https://u"}},
                "projects": {key: {"mcpServers": {"l": {"command": "x"}}}}
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(f.project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "y"}}}"#).unwrap();

        let s = snapshot(&f, true, vec![]).await.unwrap();

        let sources: Vec<_> = s.connectors.iter().map(|c| (c.name.as_str(), c.source.clone())).collect();
        assert_eq!(sources, vec![("l", ConnectorSource::Local), ("p", ConnectorSource::Project), ("u", ConnectorSource::User)]);
    }

    #[tokio::test]
    async fn without_a_project_only_the_user_servers_show() {
        let f = fixture();
        std::fs::write(&f.claude_json, json!({"mcpServers": {"u": {"command": "x"}}}).to_string()).unwrap();
        std::fs::write(f.project.join(".mcp.json"), r#"{"mcpServers": {"p": {"command": "y"}}}"#).unwrap();
        let s = snapshot(&f, false, vec![]).await.unwrap();
        assert_eq!(names(&s), vec![("user:u".to_string(), "u".to_string())]);
    }

    #[tokio::test]
    async fn one_name_in_two_scopes_is_listed_once_by_the_scope_that_wins() {
        let f = fixture();
        std::fs::write(&f.claude_json, json!({"mcpServers": {"same": {"command": "user"}}}).to_string()).unwrap();
        std::fs::write(f.project.join(".mcp.json"), r#"{"same": {"command": "project"}}"#).unwrap();
        let s = snapshot(&f, true, vec![]).await.unwrap();
        assert_eq!(s.connectors.len(), 1);
        assert_eq!(s.connectors[0].source, ConnectorSource::Project);
        assert_eq!(s.connectors[0].target, "project");
    }

    #[tokio::test]
    async fn odd_files_do_not_hide_the_rest() {
        let f = fixture();
        std::fs::write(&f.claude_json, r#"{"mcpServers": {"str": "hello", "odd": {"hello": 1}, "ok": {"command": "x"}}}"#).unwrap();
        let s = snapshot(&f, false, vec![]).await.unwrap();
        let kinds: Vec<_> = s.connectors.iter().map(|c| (c.name.as_str(), c.kind)).collect();
        assert_eq!(kinds, vec![("odd", ConnectorKind::Unknown), ("ok", ConnectorKind::Stdio)]);

        std::fs::write(&f.claude_json, r#"{"mcpServers": null}"#).unwrap();
        assert!(snapshot(&f, false, vec![]).await.unwrap().connectors.is_empty());
    }

    #[tokio::test]
    async fn a_claude_json_that_is_not_json_is_an_error_naming_the_file() {
        let f = fixture();
        std::fs::write(&f.claude_json, "{oops").unwrap();
        let ConnectorCatalogError::Io(message) = snapshot(&f, false, vec![]).await.unwrap_err();
        assert!(message.contains(".claude.json"), "{message}");
    }

    #[tokio::test]
    async fn an_installed_plugins_servers_are_found_in_mcp_json_and_in_the_manifest() {
        let f = fixture();
        let dir = f.home.join("plugins/cache/hello");
        std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        std::fs::write(dir.join(".mcp.json"), r#"{"mcpServers": {"from-file": {"command": "bun", "args": ["run", "${CLAUDE_PLUGIN_ROOT}"]}}}"#).unwrap();
        std::fs::write(
            dir.join(".claude-plugin/plugin.json"),
            r#"{"name": "hello", "mcpServers": {"from-manifest": {"type": "http", "url": "https://m"}}}"#,
        )
        .unwrap();

        let s = snapshot(&f, false, vec![plugin("hello@m", true, Some(&dir))]).await.unwrap();

        let found: Vec<_> = s.connectors.iter().map(|c| (c.name.as_str(), c.installed, c.source.plugin_id())).collect();
        assert_eq!(found, vec![("from-file", true, Some("hello@m")), ("from-manifest", true, Some("hello@m"))]);
        assert_eq!(s.connectors[0].target, "bun run ${CLAUDE_PLUGIN_ROOT}");
    }

    #[tokio::test]
    async fn a_broken_plugin_file_or_a_manifest_path_outside_the_plugin_is_skipped() {
        let f = fixture();
        let dir = f.home.join("plugins/cache/hello");
        std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        std::fs::write(dir.join(".mcp.json"), "{oops").unwrap();
        std::fs::write(f._dir.path().join("outside.json"), r#"{"mcpServers": {"leak": {"command": "x"}}}"#).unwrap();
        std::fs::write(dir.join(".claude-plugin/plugin.json"), r#"{"mcpServers": "./../../../../../outside.json"}"#).unwrap();
        let s = snapshot(&f, false, vec![plugin("hello@m", true, Some(&dir))]).await.unwrap();
        assert!(s.connectors.is_empty(), "{:?}", s.connectors);
    }

    #[tokio::test]
    async fn a_plugin_a_marketplace_offers_shows_as_not_installed_and_an_installed_one_is_not_repeated() {
        let f = fixture();
        let market = f.home.join("plugins/marketplaces/m");
        std::fs::create_dir_all(market.join(".claude-plugin")).unwrap();
        std::fs::write(
            market.join(".claude-plugin/marketplace.json"),
            r#"{"plugins": [{"name": "offered", "source": "./plugins/offered"}, {"name": "hello", "source": "./plugins/hello"}, "broken", {"name": "far", "source": {"source": "github", "repo": "a/b"}}]}"#,
        )
        .unwrap();
        for name in ["offered", "hello"] {
            let dir = market.join("plugins").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(".mcp.json"), format!(r#"{{"{name}-server": {{"command": "x"}}}}"#)).unwrap();
        }
        std::fs::write(
            f.home.join("plugins/known_marketplaces.json"),
            json!({"m": {"installLocation": market}}).to_string(),
        )
        .unwrap();
        let installed_dir = f.home.join("plugins/cache/hello");
        std::fs::create_dir_all(&installed_dir).unwrap();
        std::fs::write(installed_dir.join(".mcp.json"), r#"{"hello-server": {"command": "x"}}"#).unwrap();

        let s = snapshot(&f, false, vec![plugin("hello@m", true, Some(&installed_dir))]).await.unwrap();

        let found: Vec<_> = s.connectors.iter().map(|c| (c.name.as_str(), c.installed)).collect();
        assert_eq!(found, vec![("hello-server", true), ("offered-server", false)]);
    }
}
