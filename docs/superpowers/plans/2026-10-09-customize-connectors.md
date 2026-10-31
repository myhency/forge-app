# Plan 30C: Customize Connectors Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Connectors tab of the Customize page: list MCP servers (Mine), open one (detail, connection check), find connectors bundled in plugins of added marketplaces (Explore), add a remote or local connector from a form, and remove a user-scope connector.

**Architecture:** Same layers as 30A and 30B. Reads come from files (a new `ConnectorCatalogPort` in `forge-adapter-claude-fs`); writes and the connection check go through the bundled `claude mcp` (a new `McpManagerPort` in `forge-adapter-claude-plugins`, reusing its `run`). A `ConnectorCatalogUseCase` joins them with Forge's `enabled_mcp_servers`; Dioxus state lives in a new `customize_connectors.rs` in `forge-features`, the screens in `forge-widgets`.

**Tech Stack:** Rust workspace (Dioxus desktop), `async_trait`, `mockall` (feature `mocks`), `serde_json`, `tempfile` for tests, the `claude` binary shipped with the SDK.

**Spec:** `docs/superpowers/specs/2026-10-09-customize-connectors-design.md` (read it first; the plan follows it section by section).

## Global Constraints

- Add and remove touch **user scope only**: every write is `claude mcp add|remove -s user`. Servers of project, local, `.mcp.json` and plugin scope are read-only in the UI.
- Never go through a shell: arguments are built as an array. Forge never writes `~/.claude.json` itself.
- Header and env **values** never enter `ConnectorEntry`, the page state, a log line, or an error text. They exist only in `ConnectorDraft` / `NewConnector` until the CLI call, and `Debug` of both prints counts, not values.
- A connector name is 1–64 characters of letters, digits, `-` and `_`, and does not start with `-`.
- Connection check: `claude mcp get <name>`, 15 s timeout, result is one line; the full output of `get` (which prints header and env values in clear) is never stored or shown.
- No default catalogue, no allow list (in-house, as 30A and 30B). OAuth sign-in in the app is out of scope (30D): "needs sign-in" points to `/mcp` in the terminal.
- Windows and OAuth are not checked; say so in "As built".
- `CLAUDE_CONFIG_DIR` is set for the child only when `FORGE_CLAUDE_HOME` is set (already done by `run`'s `config_dir`).
- Never run bare `git stash`. Commit messages end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

1. A header or env value with spaces, quotes, a colon, Korean text or a leading dash reaches the CLI as exactly one argument, unchanged (Task 5 test).
2. A CLI failure whose stderr echoes a header or env value does not show it (Task 5 and Task 6 tests).
3. A double click on Add (or a second add while one runs) makes one connector, not two, and a name that exists in any non-plugin scope is refused before the CLI runs (Task 2, 6, 7 tests).
4. Remove, then add the same name again: the old Forge switch does not come back (Task 6 test).
5. A missing `~/.claude.json`, `"mcpServers": null`, an entry that is a string, a plugin with a broken `.mcp.json`, and a plugin manifest whose `mcpServers` path leaves the plugin folder: the list still shows the rest (Task 4 tests).

Also covered by tests: leaving the tab while an add runs does not jump to the new connector (Task 7).

---

## File Structure

| File | Create/Modify | Responsibility |
| --- | --- | --- |
| `crates/forge-domain/src/connector.rs` | Create | `ConnectorKind`, `ConnectorSource`, `ConnectorEntry`, `ConnectorSnapshot`, `Connection`, `connector_id`, `entry_from_config` |
| `crates/forge-domain/src/connector_form.rs` | Create | `NewConnector`, `NewTransport`, `ConnectorDraft`, `validate_new`, `split_args` |
| `crates/forge-domain/src/lib.rs` | Modify | modules and exports |
| `crates/forge-ports/src/connector_catalog_port.rs` | Create | `ConnectorCatalogPort`, `ConnectorCatalogError` |
| `crates/forge-ports/src/mcp_manager_port.rs` | Create | `McpManagerPort`, `McpManagerError` |
| `crates/forge-ports/src/lib.rs` | Modify | exports and mocks |
| `crates/forge-adapter-claude-fs/src/connector_catalog.rs` | Create | `ClaudeConnectorCatalogAdapter` |
| `crates/forge-adapter-claude-fs/src/mcp.rs`, `skill_catalog.rs`, `lib.rs` | Modify | `pub(crate)` on shared helpers; export |
| `crates/forge-adapter-claude-plugins/src/mcp.rs` | Create | `ClaudeMcpCli`, `add_args`, `parse_connection` |
| `crates/forge-adapter-claude-plugins/src/lib.rs` | Modify | `mod mcp;` export |
| `crates/forge-application/src/use_cases/connector_catalog.rs` | Create | `ConnectorCatalogUseCase`, `ConnectorView`, `ConnectorsError`, `AddedConnector` |
| `crates/forge-application/src/use_cases/mod.rs`, `lib.rs` | Modify | exports |
| `crates/forge-features/src/customize_connectors.rs` | Create | rows, groups, Explore cards, `ConnectionState`, the `Connectors` signals, `impl Customize` methods |
| `crates/forge-features/src/customize_state.rs`, `lib.rs`, `workspace.rs` | Modify | field, `read_all`, `close_detail`, `Dialog` variants, services |
| `crates/forge-widgets/src/customize_connectors.rs` | Create | `ConnectorsTab`, rows, cards, detail |
| `crates/forge-widgets/src/customize_connector_dialogs.rs` | Create | Add connector and Confirm remove dialogs |
| `crates/forge-widgets/src/customize_page.rs`, `customize_dialogs.rs`, `lib.rs` | Modify | tab, add menu, dialog dispatch |
| `crates/forge-shared/src/theme.css` | Modify | a few classes |
| `crates/forge-app/src/main.rs` | Modify | wiring |
| `docs/superpowers/specs/2026-10-09-customize-connectors-design.md`, `docs/superpowers/roadmap/README.md`, `backlog.md` | Modify | "As built", status |

Facts checked against the real bundled `claude` (2026-10-09), used by the tests below:

- `claude mcp add -s user -t http <name> <url> -H "X: y"` and `... add -s user <name> -e K=V -- <cmd> <args...>` work; a header or env value with spaces, quotes or Korean arrives as one value. The CLI also accepts a name starting with `-` after `--`, which is why Forge refuses it.
- `claude mcp get <name>` exits 0 and prints `  Status: ✘ Failed to connect` + `  Issue: ...`, or `  Status: ! Needs authentication`; header and env values are printed in clear. A name that does not exist exits 1: `No MCP server named "x". Configured servers: a, b`.
- `claude mcp remove -s user <name>` exits 1 with `No MCP server named "x" in user scope` when it is not there.
- The CLI writes the stdio entry as `{"type":"stdio","command":..,"args":[],"env":{}}`.

---

### Task 1: Domain, connector entries

**Files:**
- Create: `crates/forge-domain/src/connector.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Produces: `ConnectorKind {Stdio, Http, Sse, Unknown}` (`Copy`, `label()`), `ConnectorSource {User, Project, Local, Plugin{plugin_id}}` (`plugin_id() -> Option<&str>`, `is_plugin()`, `removable()`), `ConnectorEntry {id, name, kind, source, installed, target, header_names, env_keys}`, `ConnectorSnapshot {connectors}`, `Connection {Connected, NeedsSignIn, Failed(String), Unknown(String)}`, `connector_id(&ConnectorSource, &str) -> String`, `entry_from_config(name, source, installed, &serde_json::Value) -> ConnectorEntry`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-domain/src/connector.rs` with only the tests first:

```rust
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
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-domain connector 2>&1 | tail -15`
Expected: compile errors (`ConnectorEntry` not found).

- [ ] **Step 3: Write the implementation** (above the tests in the same file)

```rust
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
```

In `crates/forge-domain/src/lib.rs` add `mod connector;` beside the other modules and, beside the other `pub use` lines:
`pub use connector::{connector_id, entry_from_config, Connection, ConnectorEntry, ConnectorKind, ConnectorSnapshot, ConnectorSource};`

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-domain connector 2>&1 | tail -15`
Expected: 6 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain/src/connector.rs crates/forge-domain/src/lib.rs
git commit -m "feat: connector entries in the domain (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Domain, the add form and its validation

**Files:**
- Create: `crates/forge-domain/src/connector_form.rs`
- Modify: `crates/forge-domain/src/lib.rs`

**Interfaces:**
- Consumes: `ConnectorKind` (Task 1).
- Produces: `ConnectorDraft {name, kind, url, command, args, headers: Vec<(String,String)>, env: Vec<(String,String)>}` (`Default`, `Clone`, `PartialEq`, redacting `Debug`), `NewTransport {Http{url,headers}, Sse{url,headers}, Stdio{command,args,env}}`, `NewConnector {name, transport}` with `secrets() -> Vec<String>`, `validate_new(&ConnectorDraft, taken: &[String]) -> Result<NewConnector, String>`, `split_args(&str) -> Result<Vec<String>, String>`.

- [ ] **Step 1: Write the failing tests** (new file, tests only)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn http() -> ConnectorDraft {
        ConnectorDraft { name: "sentry".into(), kind: ConnectorKind::Http, url: "https://mcp.sentry.dev/mcp".into(), ..ConnectorDraft::default() }
    }

    fn local() -> ConnectorDraft {
        ConnectorDraft { name: "files".into(), kind: ConnectorKind::Stdio, command: "npx".into(), args: "-y pkg \"two words\"".into(), ..ConnectorDraft::default() }
    }

    #[test]
    fn a_good_remote_draft_becomes_a_new_connector() {
        let mut draft = http();
        draft.headers = vec![("Authorization".into(), "Bearer abc".into()), (" ".into(), " ".into())];
        let made = validate_new(&draft, &[]).unwrap();
        assert_eq!(made.name, "sentry");
        assert_eq!(
            made.transport,
            NewTransport::Http { url: "https://mcp.sentry.dev/mcp".into(), headers: vec![("Authorization".into(), "Bearer abc".into())] }
        );
        assert_eq!(made.secrets(), vec!["Bearer abc".to_string()]);
    }

    #[test]
    fn a_good_local_draft_splits_the_args_and_keeps_quoted_words_together() {
        let mut draft = local();
        draft.env = vec![("API_KEY".into(), "k 1".into())];
        let made = validate_new(&draft, &[]).unwrap();
        assert_eq!(
            made.transport,
            NewTransport::Stdio { command: "npx".into(), args: vec!["-y".into(), "pkg".into(), "two words".into()], env: vec![("API_KEY".into(), "k 1".into())] }
        );
    }

    #[test]
    fn the_name_rules() {
        for bad in ["", "  ", "-x", "a b", "a.b", "한글", &"x".repeat(65)] {
            let mut draft = http();
            draft.name = bad.to_string();
            assert!(validate_new(&draft, &[]).is_err(), "{bad:?}");
        }
        for good in ["a", "my-server_2", &"x".repeat(64)] {
            let mut draft = http();
            draft.name = good.to_string();
            assert!(validate_new(&draft, &[]).is_ok(), "{good:?}");
        }
    }

    #[test]
    fn a_name_that_is_taken_is_refused_with_its_name_in_the_message() {
        let error = validate_new(&http(), &["sentry".to_string()]).unwrap_err();
        assert!(error.contains("sentry") && error.contains("already"), "{error}");
    }

    #[test]
    fn the_address_must_be_http_or_https_without_spaces() {
        for bad in ["", "mcp.sentry.dev", "ftp://x/y", "https://", "https://a b", "javascript:alert(1)"] {
            let mut draft = http();
            draft.url = bad.to_string();
            assert!(validate_new(&draft, &[]).is_err(), "{bad:?}");
        }
        let mut draft = http();
        draft.url = "  http://localhost:3000/sse  ".into();
        assert!(validate_new(&draft, &[]).is_ok());
    }

    #[test]
    fn header_and_env_names_are_checked_and_blank_rows_are_dropped() {
        let mut draft = http();
        draft.headers = vec![("Bad:Name".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.headers = vec![("Has Space".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.headers = vec![("X-Ok".into(), "line\nbreak".into())];
        assert!(validate_new(&draft, &[]).is_err());
        let mut draft = local();
        draft.env = vec![("1BAD".into(), "v".into())];
        assert!(validate_new(&draft, &[]).is_err());
        draft.env = vec![("".into(), "".into())];
        assert!(validate_new(&draft, &[]).is_ok());
    }

    #[test]
    fn a_local_connector_needs_a_command_and_balanced_quotes() {
        let mut draft = local();
        draft.command = "  ".into();
        assert!(validate_new(&draft, &[]).is_err());
        let mut draft = local();
        draft.args = "-y \"open".into();
        assert!(validate_new(&draft, &[]).is_err());
    }

    #[test]
    fn unknown_kind_is_refused() {
        let mut draft = http();
        draft.kind = ConnectorKind::Unknown;
        assert!(validate_new(&draft, &[]).is_err());
    }

    #[test]
    fn split_args_handles_quotes_and_empty_strings() {
        assert_eq!(split_args("").unwrap(), Vec::<String>::new());
        assert_eq!(split_args("  a   b ").unwrap(), vec!["a", "b"]);
        assert_eq!(split_args("a 'b c' \"d e\" \"\"").unwrap(), vec!["a", "b c", "d e", ""]);
        assert_eq!(split_args("--flag=\"x y\"").unwrap(), vec!["--flag=x y"]);
        assert!(split_args("'open").is_err());
    }

    #[test]
    fn debug_never_prints_a_value() {
        let mut draft = http();
        draft.headers = vec![("Authorization".into(), "TOP-SECRET".into())];
        assert!(!format!("{draft:?}").contains("TOP-SECRET"));
        let made = validate_new(&draft, &[]).unwrap();
        assert!(!format!("{made:?}").contains("TOP-SECRET"));
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-domain connector_form 2>&1 | tail -15`
Expected: compile errors.

- [ ] **Step 3: Write the implementation** (top of the same file)

```rust
use std::fmt;

use crate::connector::ConnectorKind;

/// What the Add dialog holds. Values (headers, env) live here only until the CLI is called.
#[derive(Clone, PartialEq, Eq)]
pub struct ConnectorDraft {
    pub name: String,
    /// `Http`, `Sse` or `Stdio`.
    pub kind: ConnectorKind,
    pub url: String,
    pub command: String,
    /// One text field; split like a shell would split quotes (see `split_args`).
    pub args: String,
    pub headers: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
}

impl Default for ConnectorDraft {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: ConnectorKind::Http,
            url: String::new(),
            command: String::new(),
            args: String::new(),
            headers: Vec::new(),
            env: Vec::new(),
        }
    }
}

impl fmt::Debug for ConnectorDraft {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConnectorDraft")
            .field("name", &self.name)
            .field("kind", &self.kind)
            .field("headers", &self.headers.len())
            .field("env", &self.env.len())
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum NewTransport {
    Http { url: String, headers: Vec<(String, String)> },
    Sse { url: String, headers: Vec<(String, String)> },
    Stdio { command: String, args: Vec<String>, env: Vec<(String, String)> },
}

impl fmt::Debug for NewTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NewTransport::Http { url, headers } => write!(f, "Http({url}, {} headers)", headers.len()),
            NewTransport::Sse { url, headers } => write!(f, "Sse({url}, {} headers)", headers.len()),
            NewTransport::Stdio { command, args, env } => write!(f, "Stdio({command}, {} args, {} env)", args.len(), env.len()),
        }
    }
}

/// A connector that passed `validate_new`: what the CLI is asked to add.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct NewConnector {
    pub name: String,
    pub transport: NewTransport,
}

impl NewConnector {
    /// The header and env values, longest first, so an error text can have them taken out.
    pub fn secrets(&self) -> Vec<String> {
        let rows = match &self.transport {
            NewTransport::Http { headers, .. } | NewTransport::Sse { headers, .. } => headers,
            NewTransport::Stdio { env, .. } => env,
        };
        let mut values: Vec<String> = rows.iter().map(|(_, value)| value.clone()).filter(|value| !value.is_empty()).collect();
        values.sort_by_key(|value| std::cmp::Reverse(value.len()));
        values
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 64
        && !name.starts_with('-')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn valid_url(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    rest.is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !url.chars().any(char::is_whitespace))
}

fn valid_header_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_graphic() && c != ':')
}

fn valid_env_key(key: &str) -> bool {
    let mut chars = key.chars();
    chars.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_') && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn blank(row: &(String, String)) -> bool {
    row.0.trim().is_empty() && row.1.trim().is_empty()
}

fn clean_headers(rows: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for row in rows.iter().filter(|row| !blank(row)) {
        let name = row.0.trim();
        if !valid_header_name(name) {
            return Err(format!("\"{name}\" is not a header name: no spaces or colons, and it cannot be empty."));
        }
        if row.1.contains(['\n', '\r', '\0']) {
            return Err(format!("The value of {name} cannot have a line break."));
        }
        out.push((name.to_string(), row.1.trim().to_string()));
    }
    Ok(out)
}

fn clean_env(rows: &[(String, String)]) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for row in rows.iter().filter(|row| !blank(row)) {
        let key = row.0.trim();
        if !valid_env_key(key) {
            return Err(format!("\"{key}\" is not a variable name: use letters, digits and _, not starting with a digit."));
        }
        if row.1.contains('\0') {
            return Err(format!("The value of {key} has a character that cannot be used."));
        }
        out.push((key.to_string(), row.1.clone()));
    }
    Ok(out)
}

/// Splits on spaces; text inside '…' or "…" stays one argument (an empty pair makes an empty argument).
pub fn split_args(text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in text.chars() {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                started = true;
            }
            None if c.is_whitespace() => {
                if started || !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            None => current.push(c),
        }
    }
    if quote.is_some() {
        return Err("Close the quote in the arguments.".to_string());
    }
    if started || !current.is_empty() {
        out.push(current);
    }
    Ok(out)
}

/// Checks the form. `taken` is every connector name already in use (any scope but plugins).
pub fn validate_new(draft: &ConnectorDraft, taken: &[String]) -> Result<NewConnector, String> {
    let name = draft.name.trim().to_string();
    if name.is_empty() {
        return Err("Give the connector a name.".to_string());
    }
    if !valid_name(&name) {
        return Err("A name can use letters, numbers, - and _ only (up to 64), and cannot start with -.".to_string());
    }
    if taken.iter().any(|existing| *existing == name) {
        return Err(format!("A connector named {name} already exists."));
    }
    let transport = match draft.kind {
        ConnectorKind::Http | ConnectorKind::Sse => {
            let url = draft.url.trim().to_string();
            if !valid_url(&url) {
                return Err("The address must start with https:// or http:// and have no spaces.".to_string());
            }
            let headers = clean_headers(&draft.headers)?;
            if draft.kind == ConnectorKind::Http {
                NewTransport::Http { url, headers }
            } else {
                NewTransport::Sse { url, headers }
            }
        }
        ConnectorKind::Stdio => {
            let command = draft.command.trim().to_string();
            if command.is_empty() {
                return Err("Enter the command to run.".to_string());
            }
            NewTransport::Stdio { command, args: split_args(&draft.args)?, env: clean_env(&draft.env)? }
        }
        ConnectorKind::Unknown => return Err("Choose a type.".to_string()),
    };
    Ok(NewConnector { name, transport })
}
```

In `lib.rs` add `mod connector_form;` and
`pub use connector_form::{split_args, validate_new, ConnectorDraft, NewConnector, NewTransport};`

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-domain connector 2>&1 | tail -15`
Expected: all connector and connector_form tests pass (16).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-domain/src/connector_form.rs crates/forge-domain/src/lib.rs
git commit -m "feat: connector form validation, values kept out of Debug (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Ports

**Files:**
- Create: `crates/forge-ports/src/connector_catalog_port.rs`, `crates/forge-ports/src/mcp_manager_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `ConnectorSnapshot`, `Connection`, `NewConnector`, `PluginEntry` (domain), `PluginManagerError` (for `From`).
- Produces:
  - `trait ConnectorCatalogPort { async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError> }`, `ConnectorCatalogError::Io(String)`.
  - `trait McpManagerPort { async fn add(&self, connector: &NewConnector) -> Result<(), McpManagerError>; async fn remove(&self, name: &str) -> Result<(), McpManagerError>; async fn check(&self, name: &str) -> Result<Connection, McpManagerError> }`, `McpManagerError {Cli(String), BinaryMissing(String), TimedOut, BadOutput(String)}` with `From<PluginManagerError>`.
  - Mocks `MockConnectorCatalogPort`, `MockMcpManagerPort`.

- [ ] **Step 1: Write the files with their tests**

`connector_catalog_port.rs`:

```rust
use std::path::PathBuf;

use async_trait::async_trait;
use forge_domain::{ConnectorSnapshot, PluginEntry};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConnectorCatalogError {
    /// A file could not be read or is not JSON: the message names it.
    #[error("{0}")]
    Io(String),
}

/// The connectors the Customize page shows. Read-only and without side effects.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ConnectorCatalogPort: Send + Sync {
    /// Servers of the user's `~/.claude.json`, the project's `.mcp.json` and local scope (when there is a
    /// project), the installed plugins in `plugins`, and the plugins that marketplaces offer and are not installed.
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<ConnectorSnapshot, ConnectorCatalogError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_a_snapshot_call() {
        let mut mock = MockConnectorCatalogPort::new();
        mock.expect_snapshot().times(1).returning(|_, _| Ok(ConnectorSnapshot::default()));
        assert!(mock.snapshot(None, vec![]).await.unwrap().connectors.is_empty());
    }
}
```

`mcp_manager_port.rs`:

```rust
use async_trait::async_trait;
use forge_domain::{Connection, NewConnector};
use thiserror::Error;

use crate::PluginManagerError;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum McpManagerError {
    /// The CLI ran and failed: its own message, with the values of the connector taken out.
    #[error("{0}")]
    Cli(String),
    #[error("The Claude program that comes with Forge was not found ({0}).")]
    BinaryMissing(String),
    #[error("The command took too long and was stopped.")]
    TimedOut,
    #[error("Could not read the output of claude: {0}")]
    BadOutput(String),
}

impl From<PluginManagerError> for McpManagerError {
    fn from(error: PluginManagerError) -> Self {
        match error {
            PluginManagerError::Cli(text) => McpManagerError::Cli(text),
            PluginManagerError::BinaryMissing(path) => McpManagerError::BinaryMissing(path),
            PluginManagerError::TimedOut => McpManagerError::TimedOut,
            PluginManagerError::BadOutput(text) => McpManagerError::BadOutput(text),
        }
    }
}

/// Adds, removes and checks MCP servers through the bundled Claude Code (the state the terminal's
/// Claude Code uses). Scope is always the user's.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait McpManagerPort: Send + Sync {
    async fn add(&self, connector: &NewConnector) -> Result<(), McpManagerError>;
    async fn remove(&self, name: &str) -> Result<(), McpManagerError>;
    /// One line about the connection. The output of the CLI is not kept: it prints header and env values.
    async fn check(&self, name: &str) -> Result<Connection, McpManagerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_a_remove_call() {
        let mut mock = MockMcpManagerPort::new();
        mock.expect_remove().withf(|name| name == "sentry").times(1).returning(|_| Ok(()));
        assert!(mock.remove("sentry").await.is_ok());
    }

    #[test]
    fn a_plugin_manager_error_keeps_its_meaning() {
        assert_eq!(McpManagerError::from(PluginManagerError::TimedOut), McpManagerError::TimedOut);
        assert_eq!(McpManagerError::from(PluginManagerError::Cli("x".into())).to_string(), "x");
    }
}
```

In `crates/forge-ports/src/lib.rs` mirror exactly what is there for `skill_catalog_port` (the `mod` line, the `pub use` line, and the `Mock…` line in the `mocks` cfg block near line 71): add `connector_catalog_port` (`ConnectorCatalogError`, `ConnectorCatalogPort`, `MockConnectorCatalogPort`) and `mcp_manager_port` (`McpManagerError`, `McpManagerPort`, `MockMcpManagerPort`). Make sure `PluginManagerError` is reachable as `crate::PluginManagerError`.

- [ ] **Step 2: Run**

Run: `cargo test -p forge-ports 2>&1 | tail -15`
Expected: pass, including the three new tests.

- [ ] **Step 3: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: connector catalog and MCP manager ports (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: File adapter, the connector catalog

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/connector_catalog.rs`
- Modify: `crates/forge-adapter-claude-fs/src/mcp.rs`, `skill_catalog.rs`, `lib.rs`

**Interfaces:**
- Consumes: `entry_from_config`, `ConnectorSource`, `ConnectorSnapshot`, `PluginEntry`, `plugin_id` (domain); `ConnectorCatalogPort` (Task 3).
- Produces: `ClaudeConnectorCatalogAdapter::new(claude_json_path: PathBuf, claude_home: PathBuf)`.
- Makes `pub(crate)`: in `mcp.rs` the functions `read_optional`, `normalize_project_servers`, `local_servers`, `project_keys` (the last is used by `local_servers`; only the first three are needed from outside); in `skill_catalog.rs` `read_json`, `local_plugin_dir`, and the structs `KnownMarketplace`, `MarketplaceFile`, `OfferedPlugin` **with their fields** (`install_location`, `plugins`, `name`, `source`).

- [ ] **Step 1: Make the helpers reachable**

Add `pub(crate)` to the items above (no behaviour change). In `lib.rs` add `mod connector_catalog;` and `pub use connector_catalog::ClaudeConnectorCatalogAdapter;`.

Run: `cargo build -p forge-adapter-claude-fs 2>&1 | tail -5` → compiles (warnings about unused are fine).

- [ ] **Step 2: Write the failing tests**

Create `connector_catalog.rs` with the tests (the `snapshot` helper is used by every test):

```rust
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
        // null servers, an entry that is a string, an entry of unknown shape.
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
```

- [ ] **Step 3: Run to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs connector_catalog 2>&1 | tail -15`
Expected: compile errors (adapter missing).

- [ ] **Step 4: Write the implementation** (top of the file)

```rust
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
```

Fix-ups the compiler may ask for: `normalize_project_servers` returns `HashMap<String, Value>` (use `.into_iter()` as above); `read_optional`'s error implements `Display`; if `local_servers` is declared `async fn local_servers(contents: &str, project_path: &Path)` the call above matches.

- [ ] **Step 5: Run to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs 2>&1 | tail -15`
Expected: all pass (the existing tests too).

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: read connectors of user, project, local and plugin scope (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: CLI adapter, add / remove / check

**Files:**
- Create: `crates/forge-adapter-claude-plugins/src/mcp.rs`
- Modify: `crates/forge-adapter-claude-plugins/src/lib.rs`

**Interfaces:**
- Consumes: `run`, `Invocation` (`run.rs`), `McpManagerPort`, `McpManagerError`, `NewConnector`, `NewTransport`, `Connection`.
- Produces: `ClaudeMcpCli::new(binary: PathBuf, config_dir: Option<PathBuf>)`; private `add_args(&NewConnector) -> Vec<String>`, `parse_connection(&str) -> Connection`, `redact(String, &[String]) -> String`.

Exact argument shapes (checked against the real CLI):
- Http: `mcp add -s user -t http <name> <url> [-H "Name: value"]...`
- Sse: same with `-t sse`.
- Stdio: `mcp add -s user <name> [-e KEY=VALUE]... -- <command> [args...]`

- [ ] **Step 1: Write the failing tests** (bottom of new `mcp.rs`)

```rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // A stand-in `claude` that writes each argument on its own line, so a test sees exactly what was passed.
    fn recording_cli(dir: &Path, stderr: &str, exit: i32, stdout: &str) -> (ClaudeMcpCli, PathBuf) {
        let log = dir.join("calls.log");
        let program = dir.join("fake-claude");
        let body = format!(
            "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> {log}; done\nprintf '%s' '{stdout}'\nprintf '%s' '{stderr}' >&2\nexit {exit}\n",
            log = log.display()
        );
        std::fs::write(&program, body).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        (ClaudeMcpCli::new(program, None), log)
    }

    fn lines(log: &Path) -> Vec<String> {
        std::fs::read_to_string(log).unwrap().lines().map(String::from).collect()
    }

    fn remote(sse: bool) -> NewConnector {
        let headers = vec![("X-One".to_string(), "a b:c".to_string()), ("X-Two".to_string(), "한글 'q' \"d\" -x".to_string())];
        let url = "https://a.example/mcp".to_string();
        NewConnector { name: "web".into(), transport: if sse { NewTransport::Sse { url, headers } } else { NewTransport::Http { url, headers } } }
    }

    #[test]
    fn a_remote_connector_is_added_with_name_and_url_first_and_each_header_as_one_argument() {
        assert_eq!(
            add_args(&remote(false)),
            ["mcp", "add", "-s", "user", "-t", "http", "web", "https://a.example/mcp", "-H", "X-One: a b:c", "-H", "X-Two: 한글 'q' \"d\" -x"]
        );
        assert_eq!(add_args(&remote(true))[5], "sse");
    }

    #[test]
    fn a_local_connector_puts_env_before_the_double_dash_and_the_command_after_it() {
        let connector = NewConnector {
            name: "files".into(),
            transport: NewTransport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "two words".into(), "--flag".into()],
                env: vec![("API_KEY".into(), "k 1".into()), ("B".into(), String::new())],
            },
        };
        assert_eq!(
            add_args(&connector),
            ["mcp", "add", "-s", "user", "files", "-e", "API_KEY=k 1", "-e", "B=", "--", "npx", "-y", "two words", "--flag"]
        );
    }

    #[tokio::test]
    async fn add_passes_every_value_as_exactly_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path(), "", 0, "");
        cli.add(&remote(false)).await.unwrap();
        assert_eq!(lines(&log), add_args(&remote(false)));
    }

    #[tokio::test]
    async fn remove_runs_mcp_remove_for_the_user_scope() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path(), "", 0, "");
        cli.remove("web").await.unwrap();
        assert_eq!(lines(&log), ["mcp", "remove", "-s", "user", "web"]);
    }

    #[tokio::test]
    async fn a_name_that_starts_with_a_dash_is_refused_and_nothing_runs() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path(), "", 0, "");
        assert!(matches!(cli.remove("--help").await, Err(McpManagerError::Cli(_))));
        assert!(matches!(cli.check(" -x").await, Err(McpManagerError::Cli(_))));
        let mut connector = remote(false);
        connector.name = "-x".into();
        assert!(matches!(cli.add(&connector).await, Err(McpManagerError::Cli(_))));
        assert!(!log.exists());
    }

    #[tokio::test]
    async fn a_failure_that_echoes_a_value_does_not_show_it() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, _) = recording_cli(dir.path(), "failed: header X-Two was 한글 'q' \"d\" -x and X-One was a b:c", 1, "");
        // Single quotes would end the shell string above, so use a connector with plain secrets for this test.
        let connector = NewConnector {
            name: "web".into(),
            transport: NewTransport::Http { url: "https://a".into(), headers: vec![("Authorization".into(), "Bearer TOPSECRET".into()), ("X".into(), "SECRET2".into())] },
        };
        let (cli2, _) = recording_cli(dir.path(), "bad header Bearer TOPSECRET and SECRET2 rejected", 1, "");
        let _ = cli;
        let McpManagerError::Cli(message) = cli2.add(&connector).await.unwrap_err() else { panic!("expected Cli") };
        assert!(!message.contains("TOPSECRET") && !message.contains("SECRET2"), "{message}");
        assert!(message.contains("rejected"), "{message}");
    }

    #[test]
    fn redact_takes_out_the_longest_value_first() {
        assert_eq!(redact("a abc b".to_string(), &["abc".to_string(), "a".to_string()]), "[hidden] [hidden] b");
    }

    #[tokio::test]
    async fn check_reads_only_the_status_line_of_the_output() {
        let dir = tempfile::tempdir().unwrap();
        let out = "web:\n  Scope: User config\n  Status: ! Needs authentication\n  Type: http\n  Headers:\n    Authorization: Bearer SECRET\n";
        let (cli, log) = recording_cli(dir.path(), "", 0, &out.replace('\n', "\\n"));
        // printf '%s' does not expand \n, so give the script real newlines instead.
        let _ = (cli, log);
        let script = dir.path().join("fake2");
        std::fs::write(&script, format!("#!/bin/sh\ncat <<'EOF'\n{out}EOF\n")).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let cli = ClaudeMcpCli::new(script, None);
        assert_eq!(cli.check("web").await.unwrap(), Connection::NeedsSignIn);
    }

    #[test]
    fn parse_connection_knows_the_real_outputs() {
        let failed = "web:\n  Scope: User config (available in all your projects)\n  Status: ✘ Failed to connect\n  Issue: ECONNREFUSED: Unable to connect.\n  Type: http\n";
        assert_eq!(parse_connection(failed), Connection::Failed("ECONNREFUSED: Unable to connect.".into()));
        let auth = "auth:\n  Status: ! Needs authentication\n  Type: http\n";
        assert_eq!(parse_connection(auth), Connection::NeedsSignIn);
        assert_eq!(parse_connection("x:\n  Status: ✓ Connected\n"), Connection::Connected);
        assert_eq!(parse_connection("x:\n  Status: ⏸ Pending approval\n"), Connection::Unknown("Pending approval".into()));
        assert_eq!(parse_connection("no status here"), Connection::Unknown("No status line in the answer of claude.".into()));
        // A failure without an Issue line keeps the status text.
        assert_eq!(parse_connection("x:\n  Status: ✘ Failed to connect\n"), Connection::Failed("Failed to connect".into()));
    }

    #[test]
    fn a_long_issue_is_cut() {
        let text = format!("x:\n  Status: ✘ Failed to connect\n  Issue: {}\n", "e".repeat(500));
        let Connection::Failed(issue) = parse_connection(&text) else { panic!() };
        assert!(issue.chars().count() <= 201, "{}", issue.chars().count());
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-adapter-claude-plugins mcp 2>&1 | tail -15`
Expected: compile errors.

- [ ] **Step 3: Write the implementation** (top of `mcp.rs`)

```rust
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::{Connection, NewConnector, NewTransport};
use forge_ports::{McpManagerError, McpManagerPort};
use tokio::sync::Mutex;

use crate::run::{run, Invocation};

const QUICK: Duration = Duration::from_secs(20);
/// The spec's limit for "Check connection".
const CHECK: Duration = Duration::from_secs(15);

/// Adds, removes and checks MCP servers by running the `claude mcp ...` commands of the Claude Code that
/// ships with the SDK, so the state is the one the terminal's Claude Code uses. User scope only.
pub struct ClaudeMcpCli {
    binary: PathBuf,
    /// Passed as `CLAUDE_CONFIG_DIR`; `None` for the real home.
    config_dir: Option<PathBuf>,
    /// Writes run one at a time: two of them would write `.claude.json` together.
    write_lock: Mutex<()>,
}

impl ClaudeMcpCli {
    pub fn new(binary: PathBuf, config_dir: Option<PathBuf>) -> Self {
        Self { binary, config_dir, write_lock: Mutex::new(()) }
    }

    async fn exec(&self, args: Vec<String>, timeout: Duration) -> Result<String, McpManagerError> {
        let binary = self.binary.clone();
        let config_dir = self.config_dir.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run(&Invocation { program: &binary, config_dir: config_dir.as_deref(), args: &refs, timeout })
        })
        .await
        .map_err(|error| McpManagerError::Cli(error.to_string()))?
        .map_err(McpManagerError::from)
    }
}

/// A name that starts with `-` would be read as an option (the CLI itself would take it after `--`, but a
/// connector like that cannot be addressed by later commands).
fn refuse_flag_like(name: &str) -> Result<(), McpManagerError> {
    if name.trim_start().starts_with('-') {
        return Err(McpManagerError::Cli(format!("\"{name}\" cannot start with a dash.")));
    }
    Ok(())
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// The arguments of `claude mcp add`, one array element per argument, never joined into a shell line.
fn add_args(connector: &NewConnector) -> Vec<String> {
    let mut args = strings(&["mcp", "add", "-s", "user"]);
    match &connector.transport {
        NewTransport::Http { url, headers } | NewTransport::Sse { url, headers } => {
            let kind = if matches!(connector.transport, NewTransport::Sse { .. }) { "sse" } else { "http" };
            args.extend(strings(&["-t", kind]));
            args.push(connector.name.clone());
            args.push(url.clone());
            for (name, value) in headers {
                args.push("-H".to_string());
                args.push(format!("{name}: {value}"));
            }
        }
        NewTransport::Stdio { command, args: command_args, env } => {
            args.push(connector.name.clone());
            for (key, value) in env {
                args.push("-e".to_string());
                args.push(format!("{key}={value}"));
            }
            args.push("--".to_string());
            args.push(command.clone());
            args.extend(command_args.iter().cloned());
        }
    }
    args
}

/// Takes the connector's secret values out of a CLI message. Longest first, so a short value does not cut a long one.
fn redact(message: String, secrets: &[String]) -> String {
    let mut sorted: Vec<&String> = secrets.iter().filter(|secret| !secret.is_empty()).collect();
    sorted.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    sorted.into_iter().fold(message, |text, secret| text.replace(secret.as_str(), "[hidden]"))
}

/// Reads the `Status:` (and `Issue:`) line of `claude mcp get`. Nothing else of the output is used or kept:
/// the rest has header and env values in clear.
fn parse_connection(output: &str) -> Connection {
    let field = |label: &str| {
        output
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix(label))
            .map(|rest| rest.trim().trim_start_matches(|c: char| !c.is_alphanumeric()).trim().to_string())
    };
    let Some(status) = field("Status:") else {
        return Connection::Unknown("No status line in the answer of claude.".to_string());
    };
    let lower = status.to_lowercase();
    if lower.contains("needs auth") {
        Connection::NeedsSignIn
    } else if lower.contains("failed") {
        let issue = field("Issue:").filter(|text| !text.is_empty()).unwrap_or(status);
        Connection::Failed(issue.chars().take(200).collect())
    } else if lower.starts_with("connected") {
        Connection::Connected
    } else {
        Connection::Unknown(status.chars().take(200).collect())
    }
}

#[async_trait]
impl McpManagerPort for ClaudeMcpCli {
    async fn add(&self, connector: &NewConnector) -> Result<(), McpManagerError> {
        refuse_flag_like(&connector.name)?;
        let _one_at_a_time = self.write_lock.lock().await;
        match self.exec(add_args(connector), QUICK).await {
            Ok(_) => Ok(()),
            Err(McpManagerError::Cli(message)) => Err(McpManagerError::Cli(redact(message, &connector.secrets()))),
            Err(other) => Err(other),
        }
    }

    async fn remove(&self, name: &str) -> Result<(), McpManagerError> {
        refuse_flag_like(name)?;
        let _one_at_a_time = self.write_lock.lock().await;
        self.exec(strings(&["mcp", "remove", "-s", "user"]).into_iter().chain([name.to_string()]).collect(), QUICK).await.map(|_| ())
    }

    async fn check(&self, name: &str) -> Result<Connection, McpManagerError> {
        refuse_flag_like(name)?;
        let output = self.exec(vec!["mcp".to_string(), "get".to_string(), name.to_string()], CHECK).await?;
        Ok(parse_connection(&output))
    }
}
```

Note: `Path` is used by the tests (`use super::*`) so keep the import; remove it if the compiler warns in non-test builds and import it in the tests instead.

In `lib.rs`: add `mod mcp;` and `pub use mcp::ClaudeMcpCli;`.

The two recording-CLI helpers in the tests above are awkward on purpose-less lines: if a test body has an unused `cli` or `log`, delete those lines; what matters are the assertions. (`check_reads_only_the_status_line_of_the_output` and `a_failure_that_echoes_a_value_does_not_show_it` each need a script whose output has real newlines or a given stderr; simplify them to one `recording_cli`-style script each when implementing.)

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-adapter-claude-plugins 2>&1 | tail -15`
Expected: pass (existing tests too).

- [ ] **Step 5: Check against the real bundled `claude`**

```bash
cd /Users/james/orca/workspaces/forge/plan-30
CLI=$PWD/$(ls sidecar/node_modules/@anthropic-ai/claude-agent-sdk-*/claude | head -1)
export CLAUDE_CONFIG_DIR=$(mktemp -d); NO_COLOR=1
$CLI mcp add -s user -t http web https://127.0.0.1:9/mcp -H "X-One: a b:c" -H "X-Two: 한글 'q' \"d\" -x" >/dev/null
$CLI mcp add -s user files -e API_KEY="k 1" -- /bin/echo -y "two words" --flag >/dev/null
python3 -c "import json,os;print(json.dumps(json.load(open(os.environ['CLAUDE_CONFIG_DIR']+'/.claude.json'))['mcpServers'],ensure_ascii=False,indent=1))"
```
Expected: `web` has both headers with the exact text; `files` has `args: ["/bin/echo"...]`? No: `command: "/bin/echo"`, `args: ["-y","two words","--flag"]`, `env: {"API_KEY":"k 1"}`.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-plugins
git commit -m "feat: add, remove and check MCP servers through the bundled claude (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Use case

**Files:**
- Create: `crates/forge-application/src/use_cases/connector_catalog.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`, and `crates/forge-application/src/lib.rs` (add the new names where `SkillCatalogUseCase`, `SkillView`, `SkillsError`, `AddedSkill` are re-exported)

**Interfaces:**
- Consumes: Tasks 1–3, `CatalogView` (plugin catalogue), `ConfigLock`, `ConfigRepositoryPort`, `AppConfig::enabled_mcp_servers`.
- Produces:
  - `ConnectorsError {Catalog, Manager, Config, Invalid(String)}`
  - `ConnectorView {snapshot: ConnectorSnapshot, enabled: Vec<String>, enabled_plugins: Vec<String>, has_marketplaces: bool}` with `taken_names(&self) -> Vec<String>`
  - `AddedConnector {name, id}`
  - `ConnectorCatalogUseCase::new(catalog: Arc<dyn ConnectorCatalogPort>, manager: Arc<dyn McpManagerPort>, config: Arc<dyn ConfigRepositoryPort>)`, `.with_config_lock(ConfigLock)`, `connectors(project: Option<&Path>, plugins: &CatalogView)`, `set_enabled(name, on)`, `add(&ConnectorDraft, taken: &[String]) -> AddedConnector`, `remove(name)`, `check(name) -> Connection`.

- [ ] **Step 1: Write the failing tests**

(Reuse the `memory_config` helper pattern of `skill_catalog.rs`, with `enabled_mcp_servers`.)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{entry_from_config, ConnectorKind, ConnectorSource, Marketplace, NewTransport, PluginSnapshot};
    use forge_ports::{MockConfigRepositoryPort, MockConnectorCatalogPort, MockMcpManagerPort};
    use std::sync::Mutex as StdMutex;

    fn memory_config(enabled: &[&str]) -> (Arc<MockConfigRepositoryPort>, Arc<StdMutex<AppConfig>>) {
        let state = Arc::new(StdMutex::new(AppConfig { enabled_mcp_servers: enabled.iter().map(|s| s.to_string()).collect(), ..AppConfig::default() }));
        let mut mock = MockConfigRepositoryPort::new();
        let loaded = state.clone();
        mock.expect_load().returning(move || Ok(loaded.lock().unwrap().clone()));
        let stored = state.clone();
        mock.expect_save().returning(move |config| {
            *stored.lock().unwrap() = config.clone();
            Ok(())
        });
        (Arc::new(mock), state)
    }

    fn use_case(catalog: MockConnectorCatalogPort, manager: MockMcpManagerPort, config: Arc<MockConfigRepositoryPort>) -> ConnectorCatalogUseCase {
        ConnectorCatalogUseCase::new(Arc::new(catalog), Arc::new(manager), config)
    }

    fn plugin_view(enabled: &[&str], markets: usize) -> CatalogView {
        CatalogView {
            snapshot: PluginSnapshot {
                plugins: vec![],
                marketplaces: (0..markets).map(|i| Marketplace { name: format!("m{i}"), kind: "git".into(), location: String::new(), last_updated: None }).collect(),
            },
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn http_draft(name: &str) -> ConnectorDraft {
        ConnectorDraft { name: name.into(), kind: ConnectorKind::Http, url: "https://a.example/mcp".into(), ..ConnectorDraft::default() }
    }

    #[tokio::test]
    async fn the_view_joins_the_snapshot_with_both_enabled_lists() {
        let mut catalog = MockConnectorCatalogPort::new();
        let entry = entry_from_config("web", ConnectorSource::User, true, &serde_json::json!({"url": "https://a"}));
        let returned = entry.clone();
        catalog
            .expect_snapshot()
            .withf(|project, _| project.as_deref() == Some(Path::new("/work")))
            .returning(move |_, _| Ok(ConnectorSnapshot { connectors: vec![returned.clone()] }));
        let (config, _) = memory_config(&["web"]);
        let view = use_case(catalog, MockMcpManagerPort::new(), config)
            .connectors(Some(Path::new("/work")), &plugin_view(&["p@m"], 1))
            .await
            .unwrap();
        assert_eq!(view.snapshot.connectors, vec![entry]);
        assert_eq!(view.enabled, vec!["web".to_string()]);
        assert_eq!(view.enabled_plugins, vec!["p@m".to_string()]);
        assert!(view.has_marketplaces);
    }

    #[test]
    fn taken_names_are_every_name_but_a_plugins() {
        let at = |name: &str, source| entry_from_config(name, source, true, &serde_json::json!({"command": "x"}));
        let view = ConnectorView {
            snapshot: ConnectorSnapshot {
                connectors: vec![at("a", ConnectorSource::User), at("b", ConnectorSource::Project), at("c", ConnectorSource::Plugin { plugin_id: "p@m".into() })],
            },
            enabled: vec![],
            enabled_plugins: vec![],
            has_marketplaces: false,
        };
        assert_eq!(view.taken_names(), vec!["a".to_string(), "b".to_string()]);
    }

    #[tokio::test]
    async fn add_checks_the_form_first_and_the_cli_is_not_asked_when_it_is_wrong() {
        let (config, _) = memory_config(&[]);
        let uc = use_case(MockConnectorCatalogPort::new(), MockMcpManagerPort::new(), config); // no expectations: any call would panic
        let error = uc.add(&http_draft("a b"), &[]).await.unwrap_err();
        assert!(matches!(error, ConnectorsError::Invalid(_)));
        let error = uc.add(&http_draft("web"), &["web".to_string()]).await.unwrap_err();
        assert!(error.to_string().contains("already exists"), "{error}");
    }

    #[tokio::test]
    async fn add_runs_the_cli_then_switches_the_connector_on_in_forge() {
        let mut manager = MockMcpManagerPort::new();
        manager
            .expect_add()
            .withf(|connector| connector.name == "web" && matches!(&connector.transport, NewTransport::Http { url, .. } if url == "https://a.example/mcp"))
            .times(1)
            .returning(|_| Ok(()));
        let (config, state) = memory_config(&[]);
        let added = use_case(MockConnectorCatalogPort::new(), manager, config).add(&http_draft("web"), &[]).await.unwrap();
        assert_eq!(added, AddedConnector { name: "web".into(), id: "user:web".into() });
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["web".to_string()]);
    }

    #[tokio::test]
    async fn a_failed_add_leaves_the_forge_settings_alone() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_add().returning(|_| Err(McpManagerError::Cli("nope".into())));
        let (config, state) = memory_config(&[]);
        let error = use_case(MockConnectorCatalogPort::new(), manager, config).add(&http_draft("web"), &[]).await.unwrap_err();
        assert_eq!(error.to_string(), "nope");
        assert!(state.lock().unwrap().enabled_mcp_servers.is_empty());
    }

    #[tokio::test]
    async fn remove_runs_the_cli_and_drops_the_name_so_a_new_connector_of_that_name_starts_off() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_remove().withf(|name| name == "web").times(1).returning(|_| Ok(()));
        let (config, state) = memory_config(&["web", "other"]);
        use_case(MockConnectorCatalogPort::new(), manager, config).remove("web").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["other".to_string()]);
    }

    #[tokio::test]
    async fn a_failed_remove_keeps_the_switch() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_remove().returning(|_| Err(McpManagerError::Cli("No MCP server named \"web\" in user scope".into())));
        let (config, state) = memory_config(&["web"]);
        assert!(use_case(MockConnectorCatalogPort::new(), manager, config).remove("web").await.is_err());
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["web".to_string()]);
    }

    #[tokio::test]
    async fn set_enabled_saves_only_when_the_list_changes() {
        let (config, state) = memory_config(&["a"]);
        let uc = use_case(MockConnectorCatalogPort::new(), MockMcpManagerPort::new(), config);
        uc.set_enabled("a", true).await.unwrap();
        uc.set_enabled("b", true).await.unwrap();
        uc.set_enabled("a", false).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_mcp_servers, vec!["b".to_string()]);
    }

    #[tokio::test]
    async fn check_passes_the_answer_on() {
        let mut manager = MockMcpManagerPort::new();
        manager.expect_check().returning(|_| Ok(Connection::NeedsSignIn));
        let (config, _) = memory_config(&[]);
        assert_eq!(use_case(MockConnectorCatalogPort::new(), manager, config).check("web").await.unwrap(), Connection::NeedsSignIn);
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-application connector 2>&1 | tail -15`
Expected: compile errors.

- [ ] **Step 3: Write the implementation** (top of the file)

```rust
use std::path::Path;
use std::sync::Arc;

use super::config_lock::ConfigLock;
use super::plugin_catalog::CatalogView;

use forge_domain::{connector_id, validate_new, AppConfig, Connection, ConnectorDraft, ConnectorSnapshot, ConnectorSource};
use forge_ports::{ConfigError, ConfigRepositoryPort, ConnectorCatalogError, ConnectorCatalogPort, McpManagerError, McpManagerPort};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConnectorsError {
    #[error("{0}")]
    Catalog(#[from] ConnectorCatalogError),
    #[error("{0}")]
    Manager(#[from] McpManagerError),
    #[error("Could not read or save the Forge settings: {0}")]
    Config(#[from] ConfigError),
    /// The form is wrong: the message tells the person what to fix.
    #[error("{0}")]
    Invalid(String),
}

/// What the Connectors tab draws: the connectors, and what Forge has switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct ConnectorView {
    pub snapshot: ConnectorSnapshot,
    /// Names of user, project and local connectors that are on.
    pub enabled: Vec<String>,
    /// Ids of plugins that are on; a plugin connector is on when its plugin is.
    pub enabled_plugins: Vec<String>,
    /// Whether any marketplace was added (Explore has something to look at).
    pub has_marketplaces: bool,
}

impl ConnectorView {
    /// Names the Add form must not reuse: every connector but the ones plugins bring.
    pub fn taken_names(&self) -> Vec<String> {
        self.snapshot.connectors.iter().filter(|entry| !entry.source.is_plugin()).map(|entry| entry.name.clone()).collect()
    }
}

/// A connector that was just added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedConnector {
    pub name: String,
    /// `connector_id` of the user connector: what the page opens.
    pub id: String,
}

/// Finding, adding, removing and switching connectors. The servers belong to the bundled Claude Code
/// (shared with the terminal); whether Forge's sessions use one is Forge's own `enabled_mcp_servers`
/// list, which this keeps in step.
pub struct ConnectorCatalogUseCase {
    catalog: Arc<dyn ConnectorCatalogPort>,
    manager: Arc<dyn McpManagerPort>,
    config: Arc<dyn ConfigRepositoryPort>,
    lock: ConfigLock,
}

impl ConnectorCatalogUseCase {
    pub fn new(catalog: Arc<dyn ConnectorCatalogPort>, manager: Arc<dyn McpManagerPort>, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { catalog, manager, config, lock: ConfigLock::default() }
    }

    /// Shares the lock the other config writers hold (see `ConfigLock`).
    pub fn with_config_lock(mut self, lock: ConfigLock) -> Self {
        self.lock = lock;
        self
    }

    /// `plugins` is the plugin catalogue the page has just read, so the bundled `claude` is not asked twice.
    pub async fn connectors(&self, project: Option<&Path>, plugins: &CatalogView) -> Result<ConnectorView, ConnectorsError> {
        let snapshot = self.catalog.snapshot(project.map(Path::to_path_buf), plugins.snapshot.plugins.clone()).await?;
        let enabled = self.config.load().await?.enabled_mcp_servers;
        Ok(ConnectorView {
            snapshot,
            enabled,
            enabled_plugins: plugins.enabled.clone(),
            has_marketplaces: !plugins.snapshot.marketplaces.is_empty(),
        })
    }

    /// Switches a user, project or local connector on or off in Forge.
    pub async fn set_enabled(&self, name: &str, on: bool) -> Result<(), ConnectorsError> {
        self.edit_enabled(|list| {
            let present = list.iter().any(|key| key == name);
            if on && !present {
                list.push(name.to_string());
            } else if !on {
                list.retain(|key| key != name);
            }
        })
        .await
    }

    /// Checks the form, adds the server in user scope, then switches it on. If the CLI fails nothing else changes.
    pub async fn add(&self, draft: &ConnectorDraft, taken: &[String]) -> Result<AddedConnector, ConnectorsError> {
        let connector = validate_new(draft, taken).map_err(ConnectorsError::Invalid)?;
        self.manager.add(&connector).await?;
        self.set_enabled(&connector.name, true).await?;
        Ok(AddedConnector { id: connector_id(&ConnectorSource::User, &connector.name), name: connector.name })
    }

    /// Removes the user-scope server, then its name leaves the enabled list, so a later connector of the same name starts off.
    pub async fn remove(&self, name: &str) -> Result<(), ConnectorsError> {
        self.manager.remove(name).await?;
        self.edit_enabled(|list| list.retain(|key| key != name)).await
    }

    pub async fn check(&self, name: &str) -> Result<Connection, ConnectorsError> {
        Ok(self.manager.check(name).await?)
    }

    // Loads, edits and saves the list; saves only when it changed.
    async fn edit_enabled(&self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), ConnectorsError> {
        let _held = self.lock.hold().await;
        let mut config: AppConfig = self.config.load().await?;
        let before = config.enabled_mcp_servers.clone();
        edit(&mut config.enabled_mcp_servers);
        if config.enabled_mcp_servers != before {
            self.config.save(&config).await?;
        }
        Ok(())
    }
}
```

Add to `use_cases/mod.rs`: `mod connector_catalog;` and `pub use connector_catalog::{AddedConnector, ConnectorCatalogUseCase, ConnectorView, ConnectorsError};`; mirror in the crate's `lib.rs` re-export.

- [ ] **Step 4: Run to verify they pass**

Run: `cargo test -p forge-application 2>&1 | tail -15`
Expected: pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-application
git commit -m "feat: connector catalog use case (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Page state (`forge-features`)

**Files:**
- Create: `crates/forge-features/src/customize_connectors.rs`
- Modify: `crates/forge-features/src/customize_state.rs`, `lib.rs`, `workspace.rs`

**Interfaces:**
- Consumes: Task 6 (`ConnectorCatalogUseCase`, `ConnectorView`, `AddedConnector`, `ConnectorsError`), `Latest`, `CustomizeMode`, `CustomizeTab` (`customize.rs`).
- Produces (pure): `ConnectorRow {entry, enabled}` (`switchable()`), `ConnectorGroups {mine, from_plugins}`, `connector_groups(&ConnectorView, query) -> ConnectorGroups`, `explore_connectors(&ConnectorView, query) -> Vec<ConnectorRow>`, `find_connector(&ConnectorView, id) -> Option<ConnectorRow>`, `source_label(&ConnectorSource) -> String`, `ConnectionState {Idle, Checking, Done(Connection), Failed(String)}`, `connection_text(&ConnectionState) -> String`.
- Produces (methods on `Customize`): `connectors() -> Option<ConnectorView>`, `connectors_busy() -> bool`, `connection() -> ConnectionState`, `open_connector(id)`, `check_connector(name)`, `set_connector_enabled(name, on)`, `add_connector(ConnectorDraft)`, `remove_connector(name)`.
- `Dialog` gains `AddConnector` and `ConfirmRemoveConnector(String)`. `WorkspaceServices` gains `connector_catalog: Arc<ConnectorCatalogUseCase>`.

- [ ] **Step 1: Write the failing tests for the pure part** (bottom of `customize_connectors.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{entry_from_config, ConnectorSnapshot};
    use serde_json::json;

    fn entry(name: &str, source: ConnectorSource, installed: bool, config: serde_json::Value) -> forge_domain::ConnectorEntry {
        entry_from_config(name, source, installed, &config)
    }

    fn plugin(id: &str) -> ConnectorSource {
        ConnectorSource::Plugin { plugin_id: id.into() }
    }

    fn view() -> ConnectorView {
        ConnectorView {
            snapshot: ConnectorSnapshot {
                connectors: vec![
                    entry("alpha", ConnectorSource::User, true, json!({"type": "http", "url": "https://sentry.example/mcp"})),
                    entry("beta", ConnectorSource::Project, true, json!({"command": "npx"})),
                    entry("gamma", plugin("p@m"), true, json!({"command": "bun"})),
                    entry("delta", plugin("q@m"), false, json!({"command": "bun"})),
                ],
            },
            enabled: vec!["alpha".into()],
            enabled_plugins: vec!["p@m".into()],
            has_marketplaces: true,
        }
    }

    #[test]
    fn mine_has_the_installed_ones_in_two_groups() {
        let groups = connector_groups(&view(), "");
        let names = |rows: &[ConnectorRow]| rows.iter().map(|r| r.entry.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&groups.mine), vec!["alpha", "beta"]);
        assert_eq!(names(&groups.from_plugins), vec!["gamma"]);
    }

    #[test]
    fn a_plugin_connector_is_on_when_its_plugin_is_and_has_no_switch_of_its_own() {
        let groups = connector_groups(&view(), "");
        assert!(groups.mine[0].enabled && groups.mine[0].switchable());
        assert!(!groups.mine[1].enabled);
        assert!(groups.from_plugins[0].enabled && !groups.from_plugins[0].switchable());
    }

    #[test]
    fn explore_has_only_connectors_of_plugins_that_are_not_installed() {
        let cards = explore_connectors(&view(), "");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].entry.name, "delta");
    }

    #[test]
    fn the_search_looks_at_name_address_and_plugin() {
        assert_eq!(connector_groups(&view(), "SENTRY").mine.len(), 1);
        assert_eq!(connector_groups(&view(), "p@m").from_plugins.len(), 1);
        assert!(connector_groups(&view(), "zzz").mine.is_empty());
        assert_eq!(explore_connectors(&view(), "q@m").len(), 1);
    }

    #[test]
    fn find_connector_looks_by_id_in_both_lists() {
        assert_eq!(find_connector(&view(), "user:alpha").unwrap().entry.name, "alpha");
        assert_eq!(find_connector(&view(), "plugin:q@m:delta").unwrap().entry.name, "delta");
        assert!(find_connector(&view(), "user:nope").is_none());
    }

    #[test]
    fn the_source_reads_like_a_sentence_fragment() {
        assert_eq!(source_label(&ConnectorSource::User), "Added by you");
        assert_eq!(source_label(&ConnectorSource::Project), "This project (.mcp.json)");
        assert_eq!(source_label(&ConnectorSource::Local), "This project, only for you");
        assert_eq!(source_label(&plugin("p@m")), "Plugin p@m");
    }

    #[test]
    fn the_connection_line() {
        assert_eq!(connection_text(&ConnectionState::Idle), "");
        assert_eq!(connection_text(&ConnectionState::Checking), "Checking…");
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Connected)), "Connected");
        assert!(connection_text(&ConnectionState::Done(Connection::NeedsSignIn)).contains("/mcp"));
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Failed("refused".into()))), "Could not connect: refused");
        assert_eq!(connection_text(&ConnectionState::Done(Connection::Unknown("odd".into()))), "odd");
        assert_eq!(connection_text(&ConnectionState::Failed("took too long".into())), "took too long");
    }
}
```

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p forge-features customize_connectors 2>&1 | tail -15`
Expected: compile errors.

- [ ] **Step 3: Write the pure part** (top of the file)

```rust
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{AddedConnector, ConnectorCatalogUseCase, ConnectorView, ConnectorsError};
use forge_domain::{Connection, ConnectorDraft, ConnectorEntry, ConnectorSource};

use crate::customize::{CustomizeMode, CustomizeTab, Latest};
use crate::customize_state::{Customize, Dialog};

/// One connector in a list: the entry and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectorRow {
    pub entry: ConnectorEntry,
    pub enabled: bool,
}

impl ConnectorRow {
    /// A plugin connector has no switch of its own: it is on when its plugin is.
    pub fn switchable(&self) -> bool {
        !self.entry.source.is_plugin()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ConnectorGroups {
    pub mine: Vec<ConnectorRow>,
    pub from_plugins: Vec<ConnectorRow>,
}

fn row(view: &ConnectorView, entry: &ConnectorEntry) -> ConnectorRow {
    let enabled = match entry.source.plugin_id() {
        Some(id) => view.enabled_plugins.iter().any(|on| on == id),
        None => view.enabled.contains(&entry.name),
    };
    ConnectorRow { entry: entry.clone(), enabled }
}

fn matches(entry: &ConnectorEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [entry.name.as_str(), entry.target.as_str(), entry.source.plugin_id().unwrap_or_default()]
            .iter()
            .any(|text| text.to_lowercase().contains(&query))
}

/// The Mine list: installed connectors, the person's own and the ones plugins bring.
pub fn connector_groups(view: &ConnectorView, query: &str) -> ConnectorGroups {
    let mut groups = ConnectorGroups::default();
    for entry in view.snapshot.connectors.iter().filter(|entry| entry.installed && matches(entry, query)) {
        let row = row(view, entry);
        if entry.source.is_plugin() {
            groups.from_plugins.push(row);
        } else {
            groups.mine.push(row);
        }
    }
    groups
}

/// The Explore list: connectors of plugins that a marketplace offers and are not installed.
pub fn explore_connectors(view: &ConnectorView, query: &str) -> Vec<ConnectorRow> {
    view.snapshot.connectors.iter().filter(|entry| !entry.installed && matches(entry, query)).map(|entry| row(view, entry)).collect()
}

pub fn find_connector(view: &ConnectorView, id: &str) -> Option<ConnectorRow> {
    view.snapshot.connectors.iter().find(|entry| entry.id == id).map(|entry| row(view, entry))
}

pub fn source_label(source: &ConnectorSource) -> String {
    match source {
        ConnectorSource::User => "Added by you".to_string(),
        ConnectorSource::Project => "This project (.mcp.json)".to_string(),
        ConnectorSource::Local => "This project, only for you".to_string(),
        ConnectorSource::Plugin { plugin_id } => format!("Plugin {plugin_id}"),
    }
}

/// The "Check connection" result for the open connector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Idle,
    Checking,
    Done(Connection),
    /// The check itself could not run (no `claude`, took too long).
    Failed(String),
}

pub fn connection_text(state: &ConnectionState) -> String {
    match state {
        ConnectionState::Idle => String::new(),
        ConnectionState::Checking => "Checking…".to_string(),
        ConnectionState::Done(Connection::Connected) => "Connected".to_string(),
        ConnectionState::Done(Connection::NeedsSignIn) => "Needs sign-in. Run /mcp in the terminal's Claude Code to sign in.".to_string(),
        ConnectionState::Done(Connection::Failed(issue)) => format!("Could not connect: {issue}"),
        ConnectionState::Done(Connection::Unknown(text)) => text.clone(),
        ConnectionState::Failed(message) => message.clone(),
    }
}
```

- [ ] **Step 4: Run the pure tests**

Run: `cargo test -p forge-features customize_connectors 2>&1 | tail -15`
Expected: the 7 tests pass (the unused imports are used by the next step).

- [ ] **Step 5: Add the signals and the `Customize` methods** (same file, below the pure part)

```rust
/// The Connectors tab's own signals. Owned by the root scope like the rest of the page state.
#[derive(Clone, Copy)]
pub(crate) struct Connectors {
    usecase: Signal<Arc<ConnectorCatalogUseCase>>,
    pub(crate) view: Signal<Option<ConnectorView>>,
    busy: Signal<bool>,
    connection: Signal<ConnectionState>,
    checks: Signal<Latest>,
}

impl Connectors {
    pub(crate) fn new(usecase: Arc<ConnectorCatalogUseCase>) -> Self {
        Self {
            usecase: Signal::new_in_scope(usecase, ScopeId::ROOT),
            view: Signal::new_in_scope(None, ScopeId::ROOT),
            busy: Signal::new_in_scope(false, ScopeId::ROOT),
            connection: Signal::new_in_scope(ConnectionState::Idle, ScopeId::ROOT),
            checks: Signal::new_in_scope(Latest::default(), ScopeId::ROOT),
        }
    }

    pub(crate) fn usecase(&self) -> Arc<ConnectorCatalogUseCase> {
        (*self.usecase.peek()).clone()
    }
}

impl Customize {
    pub fn connectors(&self) -> Option<ConnectorView> {
        self.connectors.view.read().clone()
    }

    pub fn connectors_busy(&self) -> bool {
        *self.connectors.busy.read()
    }

    pub fn connection(&self) -> ConnectionState {
        self.connectors.connection.read().clone()
    }

    /// A late answer of a check belongs to the connector that was open; leaving it drops the answer.
    pub(crate) fn reset_connection(&self) {
        let mut checks = self.connectors.checks;
        checks.write().issue();
        let mut connection = self.connectors.connection;
        connection.set(ConnectionState::Idle);
    }

    pub fn open_connector(&self, id: String) {
        self.reset_connection();
        self.open_detail(id);
    }

    pub fn check_connector(&self, name: String) {
        let mut connection = self.connectors.connection;
        connection.set(ConnectionState::Checking);
        let mut checks = self.connectors.checks;
        let token = checks.write().issue();
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = usecase.check(&name).await;
            if this.connectors.checks.peek().is_latest(token) {
                let mut connection = this.connectors.connection;
                connection.set(match result {
                    Ok(answer) => ConnectionState::Done(answer),
                    Err(error) => ConnectionState::Failed(error.to_string()),
                });
            }
        });
    }

    /// Switches a user, project or local connector on or off. The list is read again afterwards.
    pub fn set_connector_enabled(&self, name: String, on: bool) {
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            if let Err(error) = usecase.set_enabled(&name, on).await {
                this.set_error(Some(error.to_string()));
            }
            this.reload();
        });
    }

    /// The Add dialog's button. A second press while one add runs does nothing.
    pub fn add_connector(&self, draft: ConnectorDraft) {
        if self.connectors_busy() {
            return;
        }
        let mut busy = self.connectors.busy;
        busy.set(true);
        self.set_dialog_error(None);
        let taken = self.connectors().map(|view| view.taken_names()).unwrap_or_default();
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = usecase.add(&draft, &taken).await;
            this.connector_added(result);
        });
    }

    // A success shows the new connector, but only to a person who is still on this tab. An error goes
    // where the person is looking: the Add dialog's line, else the page bar.
    fn connector_added(&self, result: Result<AddedConnector, ConnectorsError>) {
        let mut busy = self.connectors.busy;
        busy.set(false);
        match result {
            Ok(added) => {
                if self.dialog() == Some(Dialog::AddConnector) {
                    self.close_dialog();
                }
                if shows_new_connector(self.tab()) {
                    self.set_mode(CustomizeMode::Mine);
                    self.read_all(None, Some(added.id));
                } else {
                    self.reload();
                }
            }
            Err(error) => {
                if self.dialog() == Some(Dialog::AddConnector) {
                    self.set_dialog_error(Some(error.to_string()));
                } else {
                    self.set_error(Some(error.to_string()));
                }
            }
        }
    }

    /// The Remove dialog's button.
    pub fn remove_connector(&self, name: String) {
        if self.connectors_busy() {
            return;
        }
        let mut busy = self.connectors.busy;
        busy.set(true);
        self.set_dialog_error(None);
        let usecase = self.connectors.usecase();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let result = usecase.remove(&name).await;
            let mut busy = this.connectors.busy;
            busy.set(false);
            match result {
                Ok(()) => {
                    if matches!(this.dialog(), Some(Dialog::ConfirmRemoveConnector(_))) {
                        this.close_dialog();
                    }
                    this.close_detail();
                }
                Err(error) => {
                    if matches!(this.dialog(), Some(Dialog::ConfirmRemoveConnector(_))) {
                        this.set_dialog_error(Some(error.to_string()));
                    } else {
                        this.set_error(Some(error.to_string()));
                    }
                }
            }
            this.reload();
        });
    }
}

/// Whether a connector that was just added is brought to the person: not when they have gone to another tab.
fn shows_new_connector(tab: CustomizeTab) -> bool {
    tab == CustomizeTab::Connectors
}
```

Add one more test in the `tests` module:

```rust
    #[test]
    fn a_new_connector_is_shown_only_to_a_person_who_is_still_on_the_connectors_tab() {
        assert!(shows_new_connector(CustomizeTab::Connectors));
        assert!(!shows_new_connector(CustomizeTab::Plugins));
        assert!(!shows_new_connector(CustomizeTab::Skills));
    }
```

- [ ] **Step 6: Wire the state into `customize_state.rs` and the crate**

In `customize_state.rs`:
1. `use crate::customize_connectors::Connectors;` and a field `pub(crate) connectors: Connectors,` on `Customize`; in `use_customize_provider` add `connectors: Connectors::new(services.connector_catalog.clone()),`.
2. `Dialog` gains `AddConnector` and `ConfirmRemoveConnector(String)`.
3. Add two small helpers (the other module needs them; the fields stay private): 
```rust
    pub(crate) fn set_error(&self, message: Option<String>) {
        let mut error = self.error;
        error.set(message);
    }

    pub(crate) fn set_dialog_error(&self, message: Option<String>) {
        let mut error = self.dialog_error;
        error.set(message);
    }
```
4. `fn read_all(&self, open: Option<(String, PathBuf)>)` becomes `pub(crate) fn read_all(&self, open: Option<(String, PathBuf)>, open_connector: Option<String>)`. Update its callers: `reload` → `self.read_all(None, None)`, `skill_added` → `self.read_all(Some((added.id, added.path)), None)`. Inside, next to the skills read:
```rust
        let connectors_usecase = self.connectors.usecase();
        ...
            let connectors = connectors_usecase.connectors(project.as_deref(), &plugins_for_skills(&loaded)).await;
        ...inside `if this.reads.peek().is_latest(token)` after the skills match:
                match connectors {
                    Ok(view) => {
                        let mut slot = this.connectors.view;
                        slot.set(Some(view));
                    }
                    Err(error) => {
                        let mut slot = this.connectors.view;
                        slot.set(None);
                        this.set_error(Some(format!("Could not read the connectors: {error}")));
                    }
                }
        ...after the skills `open` block:
            if let Some(id) = open_connector {
                if *this.visit.peek() == visit {
                    this.open_connector(id);
                }
            }
```
(clone `connectors_usecase` before `spawn_forever` exactly like `skills_usecase`; keep `project` cloned for both.)
5. `close_detail` also calls `self.reset_connection();`.
6. In `workspace.rs` add `pub connector_catalog: Arc<ConnectorCatalogUseCase>,` to `WorkspaceServices` (and the `use forge_application::{...}` list).
7. In `lib.rs`: `mod customize_connectors;` and
`pub use customize_connectors::{connection_text, connector_groups, explore_connectors, find_connector, source_label, ConnectionState, ConnectorGroups, ConnectorRow};`

- [ ] **Step 7: Build and test**

Run: `cargo build -p forge-features 2>&1 | tail -20` — fix exhaustive `match` on `Dialog` in this crate if any (the widgets crate is Task 8).
Run: `cargo test -p forge-features 2>&1 | tail -15`
Expected: pass.

(`forge-app` will not build until Task 9 wires `connector_catalog`; build only the crates named in each task until then.)

- [ ] **Step 8: Commit**

```bash
git add crates/forge-features
git commit -m "feat: Connectors tab state, rows, search, connection line (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Screens (`forge-widgets`)

**Files:**
- Create: `crates/forge-widgets/src/customize_connectors.rs`, `crates/forge-widgets/src/customize_connector_dialogs.rs`
- Modify: `crates/forge-widgets/src/customize_page.rs`, `customize_dialogs.rs`, `lib.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 7 exports; `Button`, `ButtonKind` from `forge_shared`; `Busy`, `Job` (to install a plugin from Explore: `customize.run(Job::Install(plugin_id))`).
- Produces: `ConnectorsTab` component, dialogs `AddConnector`, `ConfirmRemoveConnector`.

There is no unit-test seam for Dioxus markup in this repo (30A and 30B checked the screens in the real window); the correctness gate here is a clean build plus Task 10.

- [ ] **Step 1: The tab** (`customize_connectors.rs`)

```rust
use dioxus::prelude::*;
use forge_domain::ConnectorKind;
use forge_features::{
    connection_text, connector_groups, explore_connectors, find_connector, source_label, use_customize, Busy, ConnectionState, ConnectorRow,
    CustomizeMode, Dialog, Job, Loaded,
};
use forge_shared::{Button, ButtonKind};

#[component]
pub fn ConnectorsTab() -> Element {
    let customize = use_customize();
    let Some(view) = customize.connectors() else {
        return match customize.view() {
            Loaded::Failed(message) => rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            },
            Loaded::Ready(_) => rsx! {
                div { class: "status-line error", "The connectors could not be read." }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            },
            Loaded::Loading => rsx! { div { class: "hint", "Loading…" } },
        };
    };
    if let Some(id) = customize.detail() {
        return match find_connector(&view, &id) {
            Some(row) => rsx! { ConnectorDetail { row } },
            None => rsx! {
                div { class: "hint",
                    "That connector is no longer there. "
                    Button { label: "Back", onclick: move |_| customize.close_detail() }
                }
            },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = connector_groups(&view, &query);
            if groups.mine.is_empty() && groups.from_plugins.is_empty() {
                let text = if query.trim().is_empty() { "No connectors yet. Use + Add to add one." } else { "Nothing matches." };
                return rsx! { div { class: "hint", "{text}" } };
            }
            rsx! {
                if !groups.mine.is_empty() { ConnectorGroup { title: "Mine", rows: groups.mine } }
                if !groups.from_plugins.is_empty() { ConnectorGroup { title: "From plugins", rows: groups.from_plugins } }
            }
        }
        CustomizeMode::Explore => {
            if !view.has_marketplaces {
                return rsx! {
                    div { class: "customize-note",
                        p { "Add a marketplace in the Plugins tab to find connectors." }
                        Button { label: "Add marketplace", kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            let cards = explore_connectors(&view, &query);
            if cards.is_empty() {
                return rsx! {
                    div { class: "hint",
                        "No connectors to find in the marketplaces you added. A plugin that lives in another Git repository shows its connectors after it is installed."
                    }
                };
            }
            rsx! {
                div { class: "customize-grid",
                    for row in cards {
                        ConnectorCard { key: "{row.entry.id}", row }
                    }
                }
            }
        }
    }
}

fn kind_tag(kind: ConnectorKind) -> &'static str {
    match kind {
        ConnectorKind::Stdio => "local",
        ConnectorKind::Http => "http",
        ConnectorKind::Sse => "sse",
        ConnectorKind::Unknown => "unreadable",
    }
}

#[component]
fn ConnectorGroup(title: String, rows: Vec<ConnectorRow>) -> Element {
    let customize = use_customize();
    rsx! {
        div { class: "customize-group",
            h2 { "{title} " span { class: "count", "{rows.len()}" } }
            for row in rows {
                {
                    let id = row.entry.id.clone();
                    let (name, on) = (row.entry.name.clone(), row.enabled);
                    rsx! {
                        div {
                            key: "{id}",
                            class: "customize-row",
                            onclick: move |_| customize.open_connector(id.clone()),
                            div { class: "customize-icon", "⌁" }
                            div { class: "customize-main",
                                div { class: "customize-name",
                                    "{row.entry.name}"
                                    span { class: "tag", "{kind_tag(row.entry.kind)}" }
                                    span { class: "tag", "{source_label(&row.entry.source)}" }
                                }
                                div { class: "customize-desc", "{row.entry.target}" }
                            }
                            button {
                                class: if on { "switch on" } else { "switch" },
                                disabled: !row.switchable(),
                                title: if row.switchable() { if on { "Turn off" } else { "Turn on" } } else { "Turned on and off with its plugin" },
                                onclick: move |event| {
                                    event.stop_propagation();
                                    customize.set_connector_enabled(name.clone(), !on);
                                },
                                span { class: "knob" }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn ConnectorCard(row: ConnectorRow) -> Element {
    let customize = use_customize();
    let plugin_id = row.entry.source.plugin_id().unwrap_or_default().to_string();
    let busy = customize.busy(&plugin_id);
    let id = row.entry.id.clone();
    let install = plugin_id.clone();
    rsx! {
        div { class: "customize-card", onclick: move |_| customize.open_connector(id.clone()),
            div { class: "customize-icon", "⌁" }
            div { class: "customize-main",
                div { class: "customize-name", "{row.entry.name}" }
                div { class: "customize-card-desc", "{row.entry.target}" }
                div { class: "customize-source", "{plugin_id}" }
            }
            if busy != Busy::No {
                span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Working…" } }
            } else {
                button {
                    class: "customize-plus",
                    title: "Install the plugin {plugin_id}",
                    onclick: move |event| {
                        event.stop_propagation();
                        customize.run(Job::Install(install.clone()));
                    },
                    "+"
                }
            }
        }
    }
}

#[component]
fn ConnectorDetail(row: ConnectorRow) -> Element {
    let customize = use_customize();
    let entry = row.entry.clone();
    let name = entry.name.clone();
    let on = row.enabled;
    let state = customize.connection();
    let checking = state == ConnectionState::Checking;
    let plugin_id = entry.source.plugin_id().map(str::to_string);
    let busy = plugin_id.as_deref().map_or(Busy::No, |id| customize.busy(id));
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), "← Connectors" }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "⌁" }
                div {
                    h2 { "{entry.name}" }
                    div { class: "hint", "{entry.kind.label()} · {source_label(&entry.source)}" }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: "Working…" }
                    } else if !entry.installed {
                        if let Some(id) = plugin_id.clone() {
                            Button { label: "Install plugin", kind: ButtonKind::Primary, onclick: move |_| customize.run(Job::Install(id.clone())) }
                        }
                    } else if row.switchable() {
                        button {
                            class: if on { "switch on" } else { "switch" },
                            title: if on { "Turn off" } else { "Turn on" },
                            onclick: {
                                let name = name.clone();
                                move |_| customize.set_connector_enabled(name.clone(), !on)
                            },
                            span { class: "knob" }
                        }
                    }
                }
            }
            div { class: "customize-section",
                div { class: "label", if entry.kind == ConnectorKind::Stdio { "Command" } else { "Address" } }
                p { class: "mono customize-wrap", "{entry.target}" }
            }
            if !entry.header_names.is_empty() {
                div { class: "customize-section",
                    div { class: "label", "Headers" }
                    for header in entry.header_names.clone() {
                        p { key: "{header}", class: "mono customize-wrap", "{header}: ••••••" }
                    }
                }
            }
            if !entry.env_keys.is_empty() {
                div { class: "customize-section",
                    div { class: "label", "Environment" }
                    for key in entry.env_keys.clone() {
                        p { key: "{key}", class: "mono customize-wrap", "{key}=••••••" }
                    }
                }
            }
            if entry.installed {
                div { class: "customize-section",
                    div { class: "label", "Connection" }
                    div { class: "customize-check",
                        Button {
                            label: "Check connection",
                            disabled: checking,
                            onclick: {
                                let name = name.clone();
                                move |_| customize.check_connector(name.clone())
                            },
                        }
                        span { class: "connection-line customize-wrap", "{connection_text(&state)}" }
                    }
                }
            }
            div { class: "customize-section",
                if entry.source.removable() {
                    Button {
                        label: "Remove",
                        onclick: {
                            let name = name.clone();
                            move |_| customize.open_dialog(Dialog::ConfirmRemoveConnector(name.clone()))
                        },
                    }
                } else if entry.source.is_plugin() {
                    p { class: "hint", "This connector comes with its plugin. Uninstall the plugin in the Plugins tab to remove it." }
                } else {
                    p { class: "hint", "This connector is defined in the project's files. Change it there." }
                }
            }
        }
    }
}
```

- [ ] **Step 2: The dialogs** (`customize_connector_dialogs.rs`)

```rust
use dioxus::prelude::*;
use forge_domain::{ConnectorDraft, ConnectorKind};
use forge_features::use_customize;
use forge_shared::{Button, ButtonKind};

fn kind_button(current: ConnectorKind, value: ConnectorKind, label: &'static str, mut kind: Signal<ConnectorKind>, disabled: bool) -> Element {
    rsx! {
        button {
            class: if current == value { "active" } else { "" },
            disabled,
            onclick: move |_| kind.set(value),
            "{label}"
        }
    }
}

#[component]
pub fn AddConnector() -> Element {
    let customize = use_customize();
    let mut name = use_signal(String::new);
    let kind = use_signal(|| ConnectorKind::Http);
    let mut url = use_signal(String::new);
    let mut command = use_signal(String::new);
    let mut args = use_signal(String::new);
    let mut headers: Signal<Vec<(String, String)>> = use_signal(|| vec![(String::new(), String::new())]);
    let mut env: Signal<Vec<(String, String)>> = use_signal(|| vec![(String::new(), String::new())]);
    let adding = customize.connectors_busy();
    let current = kind();
    let draft = move || ConnectorDraft {
        name: name(),
        kind: kind(),
        url: url(),
        command: command(),
        args: args(),
        headers: headers(),
        env: env(),
    };
    rsx! {
        h2 { "Add connector" }
        p { class: "hint", "A connector is an MCP server. It is added for you in every project." }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: "Name (letters, numbers, - and _)",
            value: "{name}",
            disabled: adding,
            oninput: move |event| name.set(event.value()),
        }
        div { class: "customize-switch",
            {kind_button(current, ConnectorKind::Http, "Remote (http)", kind, adding)}
            {kind_button(current, ConnectorKind::Sse, "Remote (sse)", kind, adding)}
            {kind_button(current, ConnectorKind::Stdio, "Local", kind, adding)}
        }
        if current == ConnectorKind::Stdio {
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: "Command, e.g. npx",
                value: "{command}",
                disabled: adding,
                oninput: move |event| command.set(event.value()),
            }
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: "Arguments, e.g. -y some-mcp-server",
                value: "{args}",
                disabled: adding,
                oninput: move |event| args.set(event.value()),
            }
            PairRows { label: "Environment variables", name_hint: "NAME", rows: env, disabled: adding }
        } else {
            input {
                class: "customize-field",
                r#type: "text",
                placeholder: "https://example.com/mcp",
                value: "{url}",
                disabled: adding,
                oninput: move |event| url.set(event.value()),
            }
            PairRows { label: "Headers", name_hint: "Header name", rows: headers, disabled: adding }
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error customize-wrap", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if adding { span { class: "spinner", title: "Adding…" } }
            Button { label: "Cancel", disabled: adding, onclick: move |_| customize.close_dialog() }
            Button { label: "Add", kind: ButtonKind::Primary, disabled: adding, onclick: move |_| customize.add_connector(draft()) }
        }
    }
}

// Rows of name and value. The value is a password field: a token typed here is not shown.
#[component]
fn PairRows(label: String, name_hint: String, rows: Signal<Vec<(String, String)>>, disabled: bool) -> Element {
    let count = rows.read().len();
    rsx! {
        div { class: "label", "{label}" }
        for index in 0..count {
            div { key: "{index}", class: "customize-pair",
                input {
                    class: "customize-field",
                    r#type: "text",
                    placeholder: "{name_hint}",
                    value: "{rows.read()[index].0}",
                    disabled,
                    oninput: move |event| rows.write()[index].0 = event.value(),
                }
                input {
                    class: "customize-field",
                    r#type: "password",
                    placeholder: "Value",
                    value: "{rows.read()[index].1}",
                    disabled,
                    oninput: move |event| rows.write()[index].1 = event.value(),
                }
            }
        }
        div {
            Button { label: "+ Row", disabled, onclick: move |_| rows.write().push((String::new(), String::new())) }
        }
    }
}

#[component]
pub fn ConfirmRemoveConnector(name: String) -> Element {
    let customize = use_customize();
    let busy = customize.connectors_busy();
    let target = name.clone();
    rsx! {
        h2 { "Remove {name}?" }
        p { class: "hint", "It is removed from Claude Code on this computer, so the terminal's Claude Code loses it too." }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error customize-wrap", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if busy { span { class: "spinner", title: "Removing…" } }
            Button { label: "Cancel", disabled: busy, onclick: move |_| customize.close_dialog() }
            Button { label: "Remove", kind: ButtonKind::Primary, disabled: busy, onclick: move |_| customize.remove_connector(target.clone()) }
        }
    }
}
```

- [ ] **Step 3: Hook them into the page**

`lib.rs`: `mod customize_connectors;` and `mod customize_connector_dialogs;`.

`customize_dialogs.rs`: add `use crate::customize_connector_dialogs::{AddConnector, ConfirmRemoveConnector};` and in the `match dialog` add
```rust
                    Dialog::AddConnector => rsx! { AddConnector {} },
                    Dialog::ConfirmRemoveConnector(name) => rsx! { ConfirmRemoveConnector { name } },
```
The backdrop click must not close the Add dialog while an add runs: change its `onclick` condition to `if !customize.adding_marketplace() && !customize.connectors_busy()`.

`customize_page.rs`:
1. `use crate::customize_connectors::ConnectorsTab;` and replace the `Connectors` arm with `CustomizeTab::Connectors => rsx! { ConnectorsTab {} },`.
2. Delete the `SettingsNote` component and the now-unused `use_settings` / `SettingsSection` imports; fix the doc comment ("Plugins, Skills and Connectors are built").
3. The Mine/Explore switch, search and `+ Add` show for all three tabs: change `if tab == CustomizeTab::Plugins || tab == CustomizeTab::Skills {` to just always (remove the condition).
4. `+ Add` disabled: `disabled: customize.skills_busy() || customize.connectors_busy()`.
5. In the menu `if tab == CustomizeTab::Skills { ... } else { ... }` becomes a `match tab` with a Connectors arm:
```rust
                                if tab == CustomizeTab::Connectors {
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.open_dialog(Dialog::AddConnector);
                                        },
                                        "Add connector"
                                    }
                                } else if tab == CustomizeTab::Skills {
                                    ...existing Skills buttons...
                                } else {
                                    ...existing marketplace buttons...
                                }
```

- [ ] **Step 4: CSS** (`crates/forge-shared/src/theme.css`, beside the other `.customize-*` rules; use the same variables they use)

```css
.customize-check { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
.connection-line { color: var(--text-muted, inherit); min-width: 0; }
.customize-wrap { overflow-wrap: anywhere; word-break: break-word; min-width: 0; }
.customize-pair { display: flex; gap: 8px; margin-bottom: 6px; }
.customize-pair .customize-field { flex: 1 1 0; min-width: 0; }
```
Check the file for the actual variable names (`grep -n "customize-" crates/forge-shared/src/theme.css | head`) and reuse them; the list rows must keep `flex: none` (30B lesson) — copy the rule the Skills rows use if it is per-class.

- [ ] **Step 5: Build**

Run: `cargo build -p forge-widgets 2>&1 | tail -20`
Expected: compiles. (Dioxus `rsx!` is picky: if a closure borrows `name` after a move, clone before; the fixes are local.)

- [ ] **Step 6: Commit**

```bash
git add crates/forge-widgets crates/forge-shared
git commit -m "feat: Connectors tab screens, add and remove dialogs (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Wire into the app, full build and tests

**Files:**
- Modify: `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: `ClaudeConnectorCatalogAdapter::new(claude_json, claude_home)`, `ClaudeMcpCli::new(binary, config_dir)`, `ConnectorCatalogUseCase`.

- [ ] **Step 1: Wire**

In `main.rs` (near line 150–190):
1. Imports: `ClaudeConnectorCatalogAdapter` from the `forge_adapter_claude_fs` list; `ClaudeMcpCli` beside `ClaudePluginCli`; `ConnectorCatalogUseCase` in the `forge_application` list.
2. `plugin_binary` is moved into `ClaudePluginCli::new`: build the connector use case **before** it, cloning: 
```rust
    let connector_catalog = Arc::new(
        ConnectorCatalogUseCase::new(
            Arc::new(ClaudeConnectorCatalogAdapter::new(claude_json.clone(), claude_home.clone())),
            Arc::new(ClaudeMcpCli::new(plugin_binary.clone(), plugin_config_dir.clone())),
            config.clone(),
        )
        .with_config_lock(config_lock.clone()),
    );
```
placed right after `plugin_config_dir` is defined and before `plugin_catalog` is built. `claude_json` is moved later into `ClaudeMcpAdapter::new(claude_json)`, which comes after, so the `.clone()` here is enough.
3. Add `connector_catalog,` to the `WorkspaceServices { ... }` literal.

- [ ] **Step 2: Build everything and run the whole suite**

Run: `cargo build --workspace 2>&1 | tail -20`
Expected: builds, no new warnings from this plan's files.

Run: `cargo test --workspace 2>&1 | tail -30`
Expected: all green (1287 before this plan, plus the new ones). If a test elsewhere constructs `WorkspaceServices` or matches `Dialog` exhaustively, add the new field or arms.

- [ ] **Step 3: Commit**

```bash
git add crates/forge-app
git commit -m "feat: wire the Connectors tab into the app (Plan 30C)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Real-window check and documents

**Files:**
- Modify: `docs/superpowers/specs/2026-10-09-customize-connectors-design.md` (add "As built"), `docs/superpowers/roadmap/README.md` (row 30C), `docs/superpowers/roadmap/backlog.md` (PG-02 text)

Use the project's harness (see memory notes `driving-the-forge-app-visually` and Plan 30B lessons): a `FORGE_CLAUDE_HOME` fixture, `capture.py` (not `screencapture -x`), `click.py` after a fresh capture, type one key at a time, keep the display awake with `caffeinate -d -u`. Add a marketplace to the harness home with `CLAUDE_CONFIG_DIR=<harness home>/.claude <bundled claude> plugin marketplace add <dir>` (`scripts/harness/add_market_fixture.py` writes a two-plugin marketplace; give one plugin a `.mcp.json` such as `{"mcpServers": {"fakechat": {"command": "bun", "args": ["run", "x"]}}}`).

- [ ] **Step 1: Run the app on the fixture and walk through** — each item is a pass/fail line to write in "As built":
  1. Connectors tab opens; empty state text shows; no CLI call (open it with no `claude` available too: rename the binary path with an env override, or skip if not easy and say so).
  2. Add a remote http connector with a header → it opens in detail; header shows `Name: ••••••`; the value is in the harness `.claude.json` (check with python) and not on screen.
  3. Add a local connector with env and an argument containing a quoted space → file has the args split right.
  4. Check connection on an unreachable URL → `Could not connect: …` within 15 s; on a reachable-but-auth server (e.g. `https://mcp.sentry.dev/mcp`) → needs sign-in line.
  5. Add the same name again → refused before the CLI runs; add an invalid URL → message in the dialog.
  6. Remove → confirm dialog → gone from the list and from the file; add the same name again → switch starts On (new add turns it on) but after Remove then restart the Forge config has no stale name (check `enabled_mcp_servers` in the Forge config file).
  7. Explore shows the fixture plugin's connector; `+` installs the plugin; it moves to "From plugins" with no switch.
  8. Project `.mcp.json` server shows read-only with its source badge and no Remove.
  9. Many rows (30+ connectors) and a 300-character URL and header name: rows keep their height, text wraps, dialog scrolls.
  10. Leave the tab while an add runs (hard to time; if you cannot, say it is covered by the unit test only).
- [ ] **Step 2: Fix what the check finds**, each fix with a test where there is a seam, one commit per fix.
- [ ] **Step 3: Write "As built"** in the spec: what was built, what differs from the design, the pass/fail lines above, and "Not checked: Windows, OAuth servers, a remote server behind a proxy".
- [ ] **Step 4: Roadmap**: add row `30C` to `docs/superpowers/roadmap/README.md` after `30B` (same columns: `Customize Connectors tab: Mine, detail with connection check, Explore, add remote or local, remove (built)`, `PG-02`, notes pointing to the spec and to 30D OAuth); in `backlog.md` PG-02 change "30C Connectors later" to "30C Connectors (built: Mine, detail, check, Explore, add, remove); 30D OAuth sign-in later".
- [ ] **Step 5: Final run and commit**

Run: `cargo test --workspace 2>&1 | tail -5` → green.

```bash
git add docs
git commit -m "docs: Plan 30C as built, roadmap row

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Review** — dispatch a fresh reviewer on the whole branch diff against `master` (spec + this plan as the brief); fix Critical and Important findings; then ask the person whether to merge to master as in 30A and 30B.

---

## Self-Review

**Spec coverage:** Back end (domain kind/source/names — Tasks 1–2; `McpManagerPort` — 3; CLI adapter — 5; file adapter with source and plugin-bundled — 4; use case with `enabled_mcp_servers` cleanup and validation — 2, 6). Screen (Mine, detail, masking, Add dialog, Explore, `+` installs plugin, no-marketplace note — 8). State and flow (open reads files only; add opens detail only on the tab; remove; check on demand — 7). Secrets (Debug redaction — 2; entry has no values — 1; redaction of CLI errors — 5; `get` output reduced to one line — 5). Out of scope items are not built. Testing list: adapter argument tests (5), registry/odd shapes (4), use case and state (6, 7), secrets (2, 5), real window (10), "not checked" (10).

**Spec deviation to note in "As built":** the spec says the registry adapter (`ClaudeMcpAdapter`) "also returns kind and source and reads plugin-bundled servers". This plan leaves `ClaudeMcpAdapter` (used to start sessions) untouched and puts the richer read in a new `ClaudeConnectorCatalogAdapter`, as 30B did for skills. Sessions still get only user/project/local servers through `ActiveExtensions`; plugin servers reach sessions through the plugin itself.

**Placeholder scan:** no TBD/TODO. Task 5 Step 3 and Task 8 Step 4 tell the implementer to adjust two things the compiler/CSS file decide (unused imports; CSS variable names) and say how to find them; Task 5's two clumsy test bodies are flagged with what to simplify.

**Type consistency:** `ConnectorDraft`, `NewConnector`, `NewTransport`, `Connection`, `ConnectorEntry` (Task 1–2) are used with the same fields in Tasks 4–8; `ConnectorView::taken_names`, `AddedConnector{name,id}`, `read_all(open, open_connector)`, `Dialog::{AddConnector, ConfirmRemoveConnector(String)}`, `Customize::{connectors, connectors_busy, connection, open_connector, check_connector, set_connector_enabled, add_connector, remove_connector}` match between Tasks 6, 7 and 8.
