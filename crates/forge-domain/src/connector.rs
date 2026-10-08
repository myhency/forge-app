use serde_json::Value;

/// How a connector talks to its server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectorKind {
    Stdio,
    Http,
    Sse,
    /// A config nobody here knows how to read: the row is listed, with this kind.
    Unknown,
}

impl ConnectorKind {
    pub fn label(self) -> &'static str {
        match self {
            ConnectorKind::Stdio => "Local (stdio)",
            ConnectorKind::Http => "Remote (http)",
            ConnectorKind::Sse => "Remote (sse)",
            ConnectorKind::Unknown => "Unknown",
        }
    }
}

/// Where a connector is defined. Only `User` can be added or removed from the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectorSource {
    User,
    Project,
    Local,
    Plugin { plugin_id: String },
}

impl ConnectorSource {
    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            ConnectorSource::Plugin { plugin_id } => Some(plugin_id),
            _ => None,
        }
    }

    pub fn is_plugin(&self) -> bool {
        self.plugin_id().is_some()
    }

    /// Whether `claude mcp remove -s user` reaches it.
    pub fn removable(&self) -> bool {
        matches!(self, ConnectorSource::User)
    }
}

/// One connector as the Customize page lists it. Header and env values are not kept: only their names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorEntry {
    /// `connector_id(source, name)`.
    pub id: String,
    pub name: String,
    pub kind: ConnectorKind,
    pub source: ConnectorSource,
    /// False only for a connector of a plugin that a marketplace offers and is not installed.
    pub installed: bool,
    /// The URL, or the command with its args.
    pub target: String,
    pub header_names: Vec<String>,
    pub env_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorSnapshot {
    pub connectors: Vec<ConnectorEntry>,
}

/// What `claude mcp get` says about a server, in one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    Connected,
    NeedsSignIn,
    Failed(String),
    /// A status line this app does not know; the text is shown as it is.
    Unknown(String),
}

pub fn connector_id(source: &ConnectorSource, name: &str) -> String {
    match source {
        ConnectorSource::User => format!("user:{name}"),
        ConnectorSource::Project => format!("project:{name}"),
        ConnectorSource::Local => format!("local:{name}"),
        ConnectorSource::Plugin { plugin_id } => format!("plugin:{plugin_id}:{name}"),
    }
}

fn sorted_keys(config: &Value, key: &str) -> Vec<String> {
    let mut keys: Vec<String> = config.get(key).and_then(Value::as_object).map(|map| map.keys().cloned().collect()).unwrap_or_default();
    keys.sort();
    keys
}

/// Reads one server's config as the files keep it. Never fails: an odd shape is `Unknown`.
pub fn entry_from_config(name: &str, source: ConnectorSource, installed: bool, config: &Value) -> ConnectorEntry {
    let text = |key: &str| config.get(key).and_then(Value::as_str);
    let kind = match text("type") {
        Some("stdio") => ConnectorKind::Stdio,
        Some("http") => ConnectorKind::Http,
        Some("sse") => ConnectorKind::Sse,
        Some(_) => ConnectorKind::Unknown,
        None if text("command").is_some() => ConnectorKind::Stdio,
        None if text("url").is_some() => ConnectorKind::Http,
        None => ConnectorKind::Unknown,
    };
    let target = if let Some(command) = text("command") {
        let args = config
            .get("args")
            .and_then(Value::as_array)
            .map(|args| args.iter().filter_map(Value::as_str).collect::<Vec<_>>())
            .unwrap_or_default();
        std::iter::once(command).chain(args).collect::<Vec<_>>().join(" ")
    } else {
        text("url").unwrap_or_default().to_string()
    };
    ConnectorEntry {
        id: connector_id(&source, name),
        name: name.to_string(),
        kind,
        source,
        installed,
        target,
        header_names: sorted_keys(config, "headers"),
        env_keys: sorted_keys(config, "env"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn entry(config: serde_json::Value) -> ConnectorEntry {
        entry_from_config("x", ConnectorSource::User, true, &config)
    }

    #[test]
    fn a_stdio_server_shows_its_command_and_args() {
        let e = entry(json!({"type": "stdio", "command": "npx", "args": ["-y", "pkg"], "env": {"TOKEN": "secret"}}));
        assert_eq!(e.kind, ConnectorKind::Stdio);
        assert_eq!(e.target, "npx -y pkg");
        assert_eq!(e.env_keys, vec!["TOKEN".to_string()]);
    }

    #[test]
    fn a_server_without_a_type_is_stdio_with_a_command_and_http_with_a_url() {
        assert_eq!(entry(json!({"command": "x"})).kind, ConnectorKind::Stdio);
        assert_eq!(entry(json!({"url": "https://a/b"})).kind, ConnectorKind::Http);
        assert_eq!(entry(json!({"url": "https://a/b"})).target, "https://a/b");
    }

    #[test]
    fn http_and_sse_are_told_apart_and_header_names_are_sorted() {
        let e = entry(json!({"type": "sse", "url": "https://a/sse", "headers": {"B": "2", "A": "1"}}));
        assert_eq!(e.kind, ConnectorKind::Sse);
        assert_eq!(e.header_names, vec!["A".to_string(), "B".to_string()]);
    }

    #[test]
    fn a_shape_nobody_knows_is_unknown_not_dropped() {
        assert_eq!(entry(json!({"hello": 1})).kind, ConnectorKind::Unknown);
        assert_eq!(entry(json!({"type": "carrier-pigeon"})).kind, ConnectorKind::Unknown);
        assert_eq!(entry(json!("a string")).kind, ConnectorKind::Unknown);
    }

    #[test]
    fn header_and_env_values_are_not_in_the_entry() {
        let e = entry(json!({"type": "http", "url": "https://a", "headers": {"Authorization": "Bearer SECRET-VALUE"}, "env": {"K": "SECRET-ENV"}}));
        let text = format!("{e:?}");
        assert!(!text.contains("SECRET-VALUE") && !text.contains("SECRET-ENV"), "{text}");
    }

    #[test]
    fn ids_tell_the_scopes_apart() {
        let plugin = ConnectorSource::Plugin { plugin_id: "p@m".into() };
        assert_eq!(connector_id(&ConnectorSource::User, "a"), "user:a");
        assert_eq!(connector_id(&ConnectorSource::Project, "a"), "project:a");
        assert_eq!(connector_id(&ConnectorSource::Local, "a"), "local:a");
        assert_eq!(connector_id(&plugin, "a"), "plugin:p@m:a");
        assert!(ConnectorSource::User.removable());
        assert!(!ConnectorSource::Project.removable() && !plugin.removable());
        assert_eq!(plugin.plugin_id(), Some("p@m"));
    }
}
