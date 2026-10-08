use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::McpServerDefinition;
use forge_ports::{McpRegistryError, McpRegistryPort};
use serde::Deserialize;
use serde_json::Value;
use unicode_normalization::UnicodeNormalization;

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

// Only the per-project `mcpServers` (Claude Code's default "local" scope) is captured; kept as a raw
// `Value` so an odd shape in one entry cannot fail the parse.
#[derive(Deserialize)]
struct ProjectEntry {
    #[serde(default, rename = "mcpServers")]
    mcp_servers: Option<Value>,
}

#[derive(Deserialize)]
struct ProjectsClaudeJson {
    #[serde(default)]
    projects: HashMap<String, ProjectEntry>,
}

// Claude Code keys a project by its NFC-normalised, symlink-resolved working directory (for a git
// worktree it uses the main repository root, which is not resolved here).
async fn project_keys(project_path: &Path) -> Vec<String> {
    let resolved = tokio::fs::canonicalize(project_path)
        .await
        .map(|path| dunce::simplified(&path).to_path_buf())
        .unwrap_or_else(|_| project_path.to_path_buf());
    let mut keys: Vec<String> = Vec::new();
    for path in [resolved.as_path(), project_path] {
        let key: String = path.to_string_lossy().nfc().collect();
        if !keys.contains(&key) {
            keys.push(key);
        }
    }
    keys
}

// Best-effort by design: a malformed `projects` value must never break the global listing, so a
// parse failure here simply means "no local-scope servers".
async fn local_servers(contents: &str, project_path: &Path) -> HashMap<String, Value> {
    let Ok(parsed) = serde_json::from_str::<ProjectsClaudeJson>(contents) else {
        return HashMap::new();
    };
    let mut projects = parsed.projects;
    for key in project_keys(project_path).await {
        if let Some(entry) = projects.remove(&key) {
            return match entry.mcp_servers {
                Some(Value::Object(servers)) => servers.into_iter().collect(),
                _ => HashMap::new(),
            };
        }
    }
    HashMap::new()
}

#[async_trait]
impl McpRegistryPort for ClaudeMcpAdapter {
    async fn list_servers(
        &self,
        project_path: &Path,
    ) -> Result<Vec<McpServerDefinition>, McpRegistryError> {
        let mut servers: HashMap<String, Value> = HashMap::new();
        let mut local: HashMap<String, Value> = HashMap::new();

        if let Some(contents) = read_optional(&self.claude_json_path).await? {
            let parsed: GlobalClaudeJson = serde_json::from_str(&contents)
                .map_err(|e| failed(&self.claude_json_path, e))?;
            servers.extend(parsed.mcp_servers);
            local = local_servers(&contents, project_path).await;
        }

        let project_file = project_path.join(".mcp.json");
        if let Some(contents) = read_optional(&project_file).await? {
            let parsed: Value =
                serde_json::from_str(&contents).map_err(|e| failed(&project_file, e))?;
            let project_servers =
                normalize_project_servers(parsed).map_err(|reason| failed(&project_file, reason))?;
            servers.extend(project_servers);
        }
        servers.extend(local);

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

    fn write_claude_json(fixture: &Fixture, global: serde_json::Value, projects: serde_json::Map<String, serde_json::Value>) {
        std::fs::write(
            &fixture.claude_json,
            json!({"mcpServers": global, "projects": projects}).to_string(),
        )
        .unwrap();
    }

    #[tokio::test]
    async fn list_servers_reads_local_scope_servers_for_the_project_path() {
        let fixture = fixture();
        let mut projects = serde_json::Map::new();
        projects.insert(
            fixture.project.to_string_lossy().into_owned(),
            json!({"allowedTools": [], "mcpServers": {"mine": {"command": "npx"}}}),
        );
        write_claude_json(&fixture, json!({}), projects);

        let servers = list(&fixture).await.unwrap();

        assert_eq!(servers, vec![server("mine", json!({"command": "npx"}))]);
    }

    #[tokio::test]
    async fn local_scope_overrides_project_mcp_json_which_overrides_global_by_name() {
        let fixture = fixture();
        let mut projects = serde_json::Map::new();
        projects.insert(
            fixture.project.to_string_lossy().into_owned(),
            json!({"mcpServers": {"c": {"from": "local"}, "d": {"from": "local"}}}),
        );
        write_claude_json(
            &fixture,
            json!({"a": {"from": "global"}, "b": {"from": "global"}, "c": {"from": "global"}}),
            projects,
        );
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
                server("c", json!({"from": "local"})),
                server("d", json!({"from": "local"})),
            ]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn local_scope_is_found_under_the_resolved_path_of_a_symlinked_project() {
        let fixture = fixture();
        let link = fixture.project.parent().unwrap().join("linked-project");
        std::os::unix::fs::symlink(&fixture.project, &link).unwrap();
        // Claude Code keys the entry by the resolved working directory.
        let resolved = std::fs::canonicalize(&fixture.project).unwrap();
        let mut projects = serde_json::Map::new();
        projects.insert(
            resolved.to_string_lossy().into_owned(),
            json!({"mcpServers": {"resolved": {"command": "x"}}}),
        );
        write_claude_json(&fixture, json!({}), projects);

        let servers = ClaudeMcpAdapter::new(fixture.claude_json.clone())
            .list_servers(&link)
            .await
            .unwrap();

        assert_eq!(servers, vec![server("resolved", json!({"command": "x"}))]);
    }

    #[tokio::test]
    async fn project_keys_are_nfc_normalised_because_claude_code_keys_projects_that_way() {
        // Claude Code stores the project key NFC-normalised, while a folder created by Finder is
        // on disk in decomposed (NFD) form and a directory listing hands that form back.
        let decomposed = "\u{1111}\u{1173}\u{1105}\u{1169}\u{110C}\u{1166}\u{11A8}\u{1110}\u{1173}";
        let composed = "프로젝트";
        assert_ne!(decomposed, composed);

        let keys = project_keys(Path::new(&format!("/nonexistent/{decomposed}"))).await;

        assert_eq!(keys, vec![format!("/nonexistent/{composed}")]);
    }

    #[tokio::test]
    async fn local_scope_ignores_other_projects_and_odd_entries() {
        let fixture = fixture();
        let mut projects = serde_json::Map::new();
        projects.insert(
            fixture.project.to_string_lossy().into_owned(),
            json!({"mcpServers": {"mine": {"command": "npx"}, "junk": 5}}),
        );
        projects.insert("/some/other/project".to_string(), json!({"mcpServers": {"leak": {"command": "x"}}, "secret": "s3cr3t"}));
        projects.insert("/broken/project".to_string(), json!({"mcpServers": [1, 2]}));
        projects.insert("/null/project".to_string(), json!({"mcpServers": null}));
        write_claude_json(&fixture, json!({}), projects);

        let servers = list(&fixture).await.unwrap();

        assert_eq!(servers, vec![server("mine", json!({"command": "npx"}))]);
    }

    #[tokio::test]
    async fn a_malformed_projects_map_does_not_break_the_global_listing() {
        let fixture = fixture();
        std::fs::write(
            &fixture.claude_json,
            r#"{"mcpServers": {"global": {"command": "g"}}, "projects": [1, 2]}"#,
        )
        .unwrap();

        let servers = list(&fixture).await.unwrap();

        assert_eq!(servers, vec![server("global", json!({"command": "g"}))]);
    }
}
