use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::config::AppConfig;

#[derive(Debug, Clone, PartialEq)]
pub struct SkillDefinition {
    pub name: String,
    pub path: PathBuf,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PluginDefinition {
    pub name: String,
    pub marketplace: Option<String>,
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpServerDefinition {
    pub name: String,
    pub config: serde_json::Value,
}

impl PluginDefinition {
    /// The string `enabled_plugins` stores: `name@marketplace`, or just `name` without a marketplace.
    pub fn key(&self) -> String {
        match &self.marketplace {
            Some(marketplace) => format!("{}@{}", self.name, marketplace),
            None => self.name.clone(),
        }
    }
}

/// What a session is started with: the enabled skills (name to directory), the enabled plugins (by
/// install path) and the enabled MCP servers (name to the server's config, as read).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ActiveExtensions {
    pub skills: BTreeMap<String, PathBuf>,
    pub plugin_paths: Vec<PathBuf>,
    pub mcp_servers: BTreeMap<String, serde_json::Value>,
}

impl ActiveExtensions {
    /// The definitions `config` has switched on. An enabled name with no definition is skipped, and a
    /// definition that appears twice (same skill name, same plugin installed under two scopes, same
    /// server name) is taken once — the first one, in the order given.
    pub fn select(
        config: &AppConfig,
        skills: &[SkillDefinition],
        plugins: &[PluginDefinition],
        servers: &[McpServerDefinition],
    ) -> Self {
        let mut active = ActiveExtensions::default();
        for skill in skills {
            if config.enabled_skills.contains(&skill.name) {
                active.skills.entry(skill.name.clone()).or_insert_with(|| skill.path.clone());
            }
        }
        let mut seen_plugins: Vec<String> = Vec::new();
        for plugin in plugins {
            let key = plugin.key();
            if config.enabled_plugins.contains(&key) && !seen_plugins.contains(&key) {
                active.plugin_paths.push(plugin.path.clone());
                seen_plugins.push(key);
            }
        }
        for server in servers {
            if config.enabled_mcp_servers.contains(&server.name) {
                active.mcp_servers.entry(server.name.clone()).or_insert_with(|| server.config.clone());
            }
        }
        active
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_definition_holds_its_fields() {
        let skill = SkillDefinition {
            name: "brainstorming".to_string(),
            path: PathBuf::from("/skills/brainstorming"),
            description: Some("Explore ideas".to_string()),
        };
        assert_eq!(skill.name, "brainstorming");
        assert_eq!(skill.description.as_deref(), Some("Explore ideas"));
    }

    #[test]
    fn mcp_server_definition_config_is_opaque_json() {
        let server = McpServerDefinition {
            name: "playwright".to_string(),
            config: serde_json::json!({"command": "npx", "args": ["playwright-mcp"]}),
        };
        assert_eq!(server.config["command"], "npx");
    }

    fn skill(name: &str) -> SkillDefinition {
        SkillDefinition { name: name.to_string(), path: PathBuf::from(format!("/skills/{name}")), description: None }
    }

    fn plugin(name: &str, marketplace: Option<&str>, path: &str) -> PluginDefinition {
        PluginDefinition {
            name: name.to_string(),
            marketplace: marketplace.map(str::to_string),
            path: PathBuf::from(path),
        }
    }

    fn server(name: &str) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config: serde_json::json!({"command": name}) }
    }

    #[test]
    fn a_plugin_key_is_name_at_marketplace_or_just_the_name() {
        assert_eq!(plugin("hud", Some("official"), "/p").key(), "hud@official");
        assert_eq!(plugin("hud", None, "/p").key(), "hud");
    }

    #[test]
    fn select_keeps_only_what_is_enabled() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let active = ActiveExtensions::select(
            &config,
            &[skill("pdf"), skill("docx")],
            &[plugin("hud", Some("official"), "/plugins/hud"), plugin("other", Some("official"), "/plugins/other")],
            &[server("files"), server("browser")],
        );
        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert_eq!(active.skills["pdf"], PathBuf::from("/skills/pdf"));
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/plugins/hud")]);
        assert_eq!(active.mcp_servers.keys().collect::<Vec<_>>(), vec!["files"]);
        assert_eq!(active.mcp_servers["files"]["command"], "files");
    }

    #[test]
    fn select_skips_an_enabled_name_that_has_no_definition() {
        let config = AppConfig {
            enabled_skills: vec!["deleted".to_string(), "pdf".to_string()],
            enabled_plugins: vec!["gone@nowhere".to_string()],
            enabled_mcp_servers: vec!["missing".to_string()],
            ..AppConfig::default()
        };
        let active = ActiveExtensions::select(&config, &[skill("pdf")], &[], &[]);
        assert_eq!(active.skills.keys().collect::<Vec<_>>(), vec!["pdf"]);
        assert!(active.plugin_paths.is_empty());
        assert!(active.mcp_servers.is_empty());
    }

    #[test]
    fn select_lists_a_duplicated_definition_once() {
        let config = AppConfig {
            enabled_skills: vec!["pdf".to_string()],
            enabled_plugins: vec!["hud@official".to_string()],
            enabled_mcp_servers: vec!["files".to_string()],
            ..AppConfig::default()
        };
        let project_pdf =
            SkillDefinition { name: "pdf".to_string(), path: PathBuf::from("/project/pdf"), description: None };
        let active = ActiveExtensions::select(
            &config,
            &[skill("pdf"), project_pdf],
            &[plugin("hud", Some("official"), "/a"), plugin("hud", Some("official"), "/b")],
            &[server("files"), server("files")],
        );
        assert_eq!(active.skills.len(), 1);
        assert_eq!(active.skills["pdf"], PathBuf::from("/skills/pdf"), "the first definition wins");
        assert_eq!(active.plugin_paths, vec![PathBuf::from("/a")], "the first install wins");
        assert_eq!(active.mcp_servers.len(), 1);
    }

    #[test]
    fn select_with_nothing_enabled_is_empty() {
        let active = ActiveExtensions::select(&AppConfig::default(), &[skill("pdf")], &[], &[server("files")]);
        assert_eq!(active, ActiveExtensions::default());
    }
}

/// A server that runs on this computer, as the Developer page shows it: the command and its arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalMcpServer {
    pub name: String,
    pub command: String,
    /// With what looks secret taken out (`mask_args`).
    pub args: Vec<String>,
}

/// The servers that start a program (a `command`), in the order given. An address-based server is not one.
pub fn local_mcp_servers(definitions: &[McpServerDefinition]) -> Vec<LocalMcpServer> {
    definitions
        .iter()
        .filter_map(|definition| {
            let command = definition.config.get("command")?.as_str()?.to_string();
            let args: Vec<&str> = definition.config.get("args").and_then(|args| args.as_array()).map(|args| args.iter().filter_map(|arg| arg.as_str()).collect()).unwrap_or_default();
            Some(LocalMcpServer { name: definition.name.clone(), command, args: crate::mask_args(&args) })
        })
        .collect()
}

#[cfg(test)]
mod local_server_tests {
    use super::*;
    use serde_json::json;

    fn definition(name: &str, config: serde_json::Value) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config }
    }

    #[test]
    fn a_server_with_a_command_is_local_and_shows_its_arguments() {
        let servers = local_mcp_servers(&[definition("fs", json!({"command": "npx", "args": ["-y", "server-filesystem", "/tmp"]}))]);
        assert_eq!(servers, [LocalMcpServer { name: "fs".into(), command: "npx".into(), args: vec!["-y".into(), "server-filesystem".into(), "/tmp".into()] }]);
    }

    #[test]
    fn a_server_with_an_address_is_not_local() {
        assert!(local_mcp_servers(&[definition("web", json!({"type": "http", "url": "https://x.dev/mcp"}))]).is_empty());
    }

    #[test]
    fn a_missing_args_list_is_empty_and_a_secret_argument_is_hidden() {
        let servers = local_mcp_servers(&[definition("a", json!({"command": "run"})), definition("b", json!({"command": "run", "args": ["--api-key", "abc"]}))]);
        assert!(servers[0].args.is_empty());
        assert_eq!(servers[1].args[0], "--api-key");
        assert_ne!(servers[1].args[1], "abc");
    }
}
