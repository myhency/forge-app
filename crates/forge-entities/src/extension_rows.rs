use std::collections::HashSet;
use std::path::Path;

use forge_domain::{McpServerDefinition, PluginDefinition, SkillDefinition};
use serde_json::Value;
use forge_shared::tr;

/// One line of a Skills / Plugins / MCP servers tab. `key` is what the `enabled_*` list stores.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtensionRow {
    pub key: String,
    pub title: String,
    pub detail: String,
    pub enabled: bool,
}

/// One row per skill name (a user and a project skill with one name are one entry, as they are for
/// the SDK); the first definition's description is shown.
pub fn skill_rows(definitions: &[SkillDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|skill| seen.insert(skill.name.clone()))
        .map(|skill| ExtensionRow {
            key: skill.name.clone(),
            title: skill.name.clone(),
            detail: skill.description.clone().unwrap_or_default(),
            enabled: enabled.contains(&skill.name),
        })
        .collect()
}

/// One row per `name@marketplace` (a plugin installed under two scopes is one plugin).
pub fn plugin_rows(definitions: &[PluginDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|plugin| seen.insert(plugin.key()))
        .map(|plugin| ExtensionRow {
            key: plugin.key(),
            title: plugin.name.clone(),
            detail: plugin.marketplace.clone().unwrap_or_default(),
            enabled: enabled.contains(&plugin.key()),
        })
        .collect()
}

pub fn mcp_rows(definitions: &[McpServerDefinition], enabled: &[String]) -> Vec<ExtensionRow> {
    let mut seen = HashSet::new();
    definitions
        .iter()
        .filter(|server| seen.insert(server.name.clone()))
        .map(|server| ExtensionRow {
            key: server.name.clone(),
            title: server.name.clone(),
            detail: mcp_summary(&server.config),
            enabled: enabled.contains(&server.name),
        })
        .collect()
}

/// `command arg…` for a local server, the URL for a remote one, empty when the config has neither
/// (the config is deliberately opaque, spec §12.2).
pub fn mcp_summary(config: &Value) -> String {
    if let Some(command) = config.get("command").and_then(Value::as_str) {
        let args = config
            .get("args")
            .and_then(Value::as_array)
            .map(|args| args.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        return std::iter::once(command).chain(args).collect::<Vec<_>>().join(" ");
    }
    config.get("url").and_then(Value::as_str).unwrap_or_default().to_string()
}

/// What the tabs name as the scope they are showing.
pub fn project_label(project: Option<&Path>) -> String {
    match project.and_then(Path::file_name) {
        Some(name) => name.to_string_lossy().to_string(),
        None => tr!("your global definitions").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn skill(name: &str, description: Option<&str>) -> SkillDefinition {
        SkillDefinition {
            name: name.to_string(),
            path: PathBuf::from(format!("/skills/{name}")),
            description: description.map(str::to_string),
        }
    }

    fn plugin(name: &str, marketplace: Option<&str>, path: &str) -> PluginDefinition {
        PluginDefinition {
            name: name.to_string(),
            marketplace: marketplace.map(str::to_string),
            path: PathBuf::from(path),
        }
    }

    fn server(name: &str, config: serde_json::Value) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config }
    }

    #[test]
    fn a_skill_row_shows_its_description_and_whether_it_is_enabled() {
        let rows = skill_rows(
            &[skill("pdf", Some("Read PDFs")), skill("docx", None)],
            &["pdf".to_string()],
        );
        assert_eq!(
            rows,
            vec![
                ExtensionRow { key: "pdf".into(), title: "pdf".into(), detail: "Read PDFs".into(), enabled: true },
                ExtensionRow { key: "docx".into(), title: "docx".into(), detail: String::new(), enabled: false },
            ]
        );
    }

    #[test]
    fn two_skills_with_one_name_make_one_row() {
        let rows = skill_rows(&[skill("pdf", Some("user")), skill("pdf", Some("project"))], &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].detail, "user", "the first one wins");
    }

    #[test]
    fn a_plugin_row_is_keyed_by_name_at_marketplace_and_shows_the_marketplace() {
        let rows = plugin_rows(
            &[plugin("hud", Some("official"), "/a"), plugin("hud", Some("official"), "/b"), plugin("solo", None, "/c")],
            &["hud@official".to_string()],
        );
        assert_eq!(rows.len(), 2, "the second install of hud is the same plugin");
        assert_eq!(rows[0].key, "hud@official");
        assert_eq!(rows[0].title, "hud");
        assert_eq!(rows[0].detail, "official");
        assert!(rows[0].enabled);
        assert_eq!(rows[1].key, "solo");
        assert_eq!(rows[1].detail, "");
        assert!(!rows[1].enabled);
    }

    #[test]
    fn an_mcp_row_summarises_a_command_or_a_url() {
        let rows = mcp_rows(
            &[
                server("files", serde_json::json!({"command": "npx", "args": ["-y", "files-mcp"]})),
                server("web", serde_json::json!({"type": "http", "url": "https://example.test/mcp"})),
            ],
            &["web".to_string()],
        );
        assert_eq!(rows[0].detail, "npx -y files-mcp");
        assert!(!rows[0].enabled);
        assert_eq!(rows[1].detail, "https://example.test/mcp");
        assert!(rows[1].enabled);
    }

    #[test]
    fn an_mcp_server_with_neither_command_nor_url_still_gets_a_row() {
        let rows = mcp_rows(&[server("odd", serde_json::json!({"type": "sse"}))], &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, "odd");
        assert_eq!(rows[0].detail, "");
    }

    #[test]
    fn the_project_label_is_the_folder_name_or_the_global_wording() {
        assert_eq!(project_label(Some(Path::new("/work/forge"))), "forge");
        assert_eq!(project_label(None), "your global definitions");
    }
}
