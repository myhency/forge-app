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

    /// Whether `claude mcp get <name>` finds it from where Forge runs it: project and local servers need the
    /// project as the working directory, and a plugin's server is registered as `plugin:<plugin>:<name>`.
    pub fn checkable(&self) -> bool {
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
    /// Things the person should know that did not stop the list (a project file that could not be read).
    pub notes: Vec<String>,
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

const HIDDEN: &str = "•••";

fn looks_secret(name: &str) -> bool {
    let name = name.to_lowercase();
    ["key", "token", "secret", "password", "passwd", "auth", "sig"].iter().any(|word| name.contains(word))
}

/// The address with what looks secret taken out: `user:password@` and the value of a query parameter whose name
/// says key, token, secret, password, auth or sig. The rest is shown as it is.
pub fn mask_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://").map(|at| at + 3) else { return url.to_string() };
    let (head, rest) = url.split_at(scheme_end);
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let (authority, tail) = rest.split_at(authority_end);
    let authority = match authority.rfind('@') {
        Some(at) => format!("{HIDDEN}@{}", &authority[at + 1..]),
        None => authority.to_string(),
    };
    let tail = match tail.find('?') {
        Some(question) => {
            let (path, query) = tail.split_at(question);
            let (query, fragment) = match query.find('#') {
                Some(hash) => query.split_at(hash),
                None => (query, ""),
            };
            let masked: Vec<String> = query[1..]
                .split('&')
                .map(|pair| match pair.split_once('=') {
                    Some((name, _)) if looks_secret(name) => format!("{name}={HIDDEN}"),
                    _ => pair.to_string(),
                })
                .collect();
            format!("{path}?{}{fragment}", masked.join("&"))
        }
        None => tail.to_string(),
    };
    format!("{head}{authority}{tail}")
}

/// Command arguments with what looks secret taken out: the word after a flag such as `--api-key`, the value of
/// `--token=…`, and the secret parts of an address.
pub fn mask_args(args: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(args.len());
    let mut hide_next = false;
    for arg in args {
        if hide_next {
            out.push(HIDDEN.to_string());
            hide_next = false;
            continue;
        }
        if let Some((name, _)) = arg.split_once('=') {
            if name.starts_with('-') && looks_secret(name) {
                out.push(format!("{name}={HIDDEN}"));
                continue;
            }
        }
        if arg.starts_with('-') && looks_secret(arg) {
            hide_next = true;
        }
        out.push(if arg.contains("://") { mask_url(arg) } else { arg.to_string() });
    }
    out
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
        std::iter::once(command.to_string()).chain(mask_args(&args)).collect::<Vec<_>>().join(" ")
    } else {
        mask_url(text("url").unwrap_or_default())
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
    fn a_secret_in_the_address_is_hidden_and_the_rest_stays() {
        assert_eq!(mask_url("https://user:pw@host.example/p?token=abc&x=1#frag"), "https://•••@host.example/p?token=•••&x=1#frag");
        assert_eq!(mask_url("https://mcp.sentry.dev/mcp"), "https://mcp.sentry.dev/mcp");
        assert_eq!(mask_url("https://h/p?api_key=1&Sig=2&workspace=7"), "https://h/p?api_key=•••&Sig=•••&workspace=7");
        assert_eq!(mask_url("not a url"), "not a url");
    }

    #[test]
    fn a_secret_in_the_arguments_is_hidden_and_the_rest_stays() {
        let args = ["-y", "pkg", "--api-key", "sk-123", "--token=abc", "--port", "80", "https://h/p?token=zzz"];
        assert_eq!(mask_args(&args), vec!["-y", "pkg", "--api-key", "•••", "--token=•••", "--port", "80", "https://h/p?token=•••"]);
    }

    #[test]
    fn the_entry_shows_the_masked_address_and_arguments() {
        let http = entry(json!({"type": "http", "url": "https://u:pw@h/p?token=SECRETQ"}));
        assert!(!http.target.contains("SECRETQ") && !http.target.contains("pw@"), "{}", http.target);
        let local = entry(json!({"command": "npx", "args": ["--api-key", "SECRETA"]}));
        assert_eq!(local.target, "npx --api-key •••");
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

    #[test]
    fn only_a_user_connector_can_be_checked_because_get_finds_no_other_scope_from_here() {
        // `claude mcp get <name>` runs outside the project and does not know plugin servers by their bare name.
        assert!(ConnectorSource::User.checkable());
        assert!(!ConnectorSource::Project.checkable());
        assert!(!ConnectorSource::Local.checkable());
        assert!(!ConnectorSource::Plugin { plugin_id: "p@m".into() }.checkable());
    }
}
