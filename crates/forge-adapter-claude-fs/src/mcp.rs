use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::McpServerDefinition;
use forge_ports::{McpRegistryError, McpRegistryPort};
use serde::Deserialize;
use serde_json::Value;

pub struct ClaudeMcpAdapter {
    claude_json_path: PathBuf,
}

impl ClaudeMcpAdapter {
    pub fn new(claude_json_path: PathBuf) -> Self {
        Self { claude_json_path }
    }
}

#[derive(Deserialize)]
struct GlobalClaudeJson {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: HashMap<String, Value>,
}

fn failed(path: &Path, reason: impl std::fmt::Display) -> McpRegistryError {
    McpRegistryError::ListFailed(format!("{}: {}", path.display(), reason))
}

async fn read_optional(path: &Path) -> Result<Option<String>, McpRegistryError> {
    match tokio::fs::read_to_string(path).await {
        Ok(contents) => Ok(Some(contents)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(failed(path, e)),
    }
}

fn normalize_project_servers(parsed: Value) -> Result<HashMap<String, Value>, &'static str> {
    let Value::Object(mut top) = parsed else {
        return Err("expected a JSON object at the top level");
    };
    match top.remove("mcpServers") {
        Some(Value::Object(wrapped)) => Ok(wrapped.into_iter().collect()),
        Some(_) => Err("\"mcpServers\" is not an object"),
        None => Ok(top.into_iter().collect()),
    }
}

#[async_trait]
impl McpRegistryPort for ClaudeMcpAdapter {
    async fn list_servers(
        &self,
        project_path: &Path,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        let mut servers: HashMap<String, Value> = HashMap::new();

        if let Some(contents) = read_optional(&self.claude_json_path).await? {
            let parsed: GlobalClaudeJson = serde_json::from_str(&contents)
                .map_err(|e| failed(&self.claude_json_path, e))?;
            servers.extend(parsed.mcp_servers);
        }

        let project_file = project_path.join(".mcp.json");
        if let Some(contents) = read_optional(&project_file).await? {
            let parsed: Value =
                serde_json::from_str(&contents).map_err(|e| failed(&project_file, e))?;
            let project_servers =
                normalize_project_servers(parsed).map_err(|reason| failed(&project_file, reason))?;
            servers.extend(project_servers);
        }

        let mut servers: Vec<McpServerDefinition> = servers
            .into_iter()
            .filter(|(_, config)| config.is_object())
            .map(|(name, config)| McpServerDefinition { name, config })
            .collect();
        servers.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(servers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture {
        _dir: tempfile::TempDir,
        claude_json: PathBuf,
        project: PathBuf,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let claude_json = dir.path().join(".claude.json");
        let project = dir.path().join("project");
        std::fs::create_dir_all(&project).unwrap();
        Fixture { _dir: dir, claude_json, project }
    }

    async fn list(fixture: &Fixture) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        ClaudeMcpAdapter::new(fixture.claude_json.clone())
            .list_servers(&fixture.project)
            .await
    }

    fn server(name: &str, config: serde_json::Value) -> McpServerDefinition {
        McpServerDefinition { name: name.to_string(), config }
    }

    #[tokio::test]
    async fn list_servers_returns_empty_when_neither_file_exists() {
        let fixture = fixture();

        assert!(list(&fixture).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_servers_reads_the_global_mcp_servers_and_ignores_every_other_key() {
        let fixture = fixture();
        std::fs::write(
            &fixture.claude_json,
            json!({
                "numStartups": 3,
                "oauthAccount": {"emailAddress": "someone@example.com"},
                "projects": {},
                "mcpServers": {"playwright": {"command": "npx", "args": ["playwright-mcp"]}}
            })
            .to_string(),
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server("playwright", json!({"command": "npx", "args": ["playwright-mcp"]}))]
        );
    }

    #[tokio::test]
    async fn list_servers_reads_a_bare_project_mcp_json() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"atlassian": {"command": "uvx", "args": ["mcp-atlassian"]}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server("atlassian", json!({"command": "uvx", "args": ["mcp-atlassian"]}))]
        );
    }

    #[tokio::test]
    async fn list_servers_reads_a_wrapped_project_mcp_json() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"mcpServers": {"docs": {"type": "http", "url": "https://example.com/mcp", "headers": {"Authorization": "Bearer t"}}}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![server(
                "docs",
                json!({"type": "http", "url": "https://example.com/mcp", "headers": {"Authorization": "Bearer t"}})
            )]
        );
    }

    #[tokio::test]
    async fn list_servers_lets_project_entries_override_global_entries_by_name_and_sorts_by_name() {
        let fixture = fixture();
        std::fs::write(
            &fixture.claude_json,
            r#"{"mcpServers": {"a": {"from": "global"}, "b": {"from": "global"}}}"#,
        )
        .unwrap();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"b": {"from": "project"}, "c": {"from": "project"}}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(
            servers,
            vec![
                server("a", json!({"from": "global"})),
                server("b", json!({"from": "project"})),
                server("c", json!({"from": "project"})),
            ]
        );
    }

    #[tokio::test]
    async fn list_servers_drops_bare_entries_that_are_not_objects() {
        let fixture = fixture();
        std::fs::write(
            fixture.project.join(".mcp.json"),
            r#"{"real": {"command": "x"}, "note": "hello", "version": 2}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(servers, vec![server("real", json!({"command": "x"}))]);
    }

    #[tokio::test]
    async fn list_servers_fails_on_malformed_global_json_and_names_the_file() {
        let fixture = fixture();
        std::fs::write(&fixture.claude_json, "{oops").unwrap();

        match list(&fixture).await {
            Err(McpRegistryError::ListFailed(message)) => {
                assert!(message.contains(".claude.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_servers_fails_on_malformed_project_json_and_names_the_file() {
        let fixture = fixture();
        std::fs::write(fixture.project.join(".mcp.json"), "{oops").unwrap();

        match list(&fixture).await {
            Err(McpRegistryError::ListFailed(message)) => {
                assert!(message.contains(".mcp.json"), "unexpected message: {message}");
            }
            other => panic!("expected ListFailed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_servers_fails_when_project_mcp_json_is_not_an_object() {
        let fixture = fixture();
        std::fs::write(fixture.project.join(".mcp.json"), "[1, 2]").unwrap();

        assert!(matches!(
            list(&fixture).await,
            Err(McpRegistryError::ListFailed(_))
        ));
    }

    #[tokio::test]
    async fn list_servers_returns_empty_when_the_global_file_has_no_mcp_servers_key() {
        let fixture = fixture();
        std::fs::write(&fixture.claude_json, r#"{"numStartups": 1}"#).unwrap();

        assert!(list(&fixture).await.unwrap().is_empty());
    }
}
