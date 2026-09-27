use std::path::PathBuf;

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
}
