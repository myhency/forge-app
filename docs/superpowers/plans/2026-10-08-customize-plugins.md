# Customize page, plugins and marketplaces (30A) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Forge gets a Customize page whose Plugins tab lists installed plugins by marketplace, lets the person add a marketplace (a Git repo, a URL or a folder), find a plugin in it, install it, switch it on or off in Forge, and uninstall it, all through the `claude` program that ships with the SDK.

**Architecture:** A new port `PluginManagerPort` (forge-ports) with a new adapter crate `forge-adapter-claude-plugins` that runs the bundled `claude plugin ...` commands and parses their `--json` output. A use case `PluginCatalogUseCase` (forge-application) adds Forge's own on/off list to every install and uninstall. Pure page logic and a signal state `Customize` live in forge-features; the page, its menus and dialogs are widgets in forge-widgets, mounted as a third main-pane state `Active::Customize`.

**Tech Stack:** Rust, Dioxus 0.7 desktop, tokio, serde_json, mockall, `theme.css`, the Python harness in `scripts/harness/`.

**Spec:** `docs/superpowers/specs/2026-10-08-customize-plugins-design.md`. Observations: `docs/superpowers/roadmap/observations.md`, Round 13. Spike: the spec's "Spike" section, plus the extra output captured while writing this plan (given verbatim in Task 2).

## Global Constraints

- Install and uninstall go through the bundled `claude` (`node_modules/@anthropic-ai/claude-agent-sdk-<platform>/claude`, 2.1.283); never through a `claude` on the PATH. Scope is always the CLI default (`user`); never pass `--scope`.
- `CLAUDE_CONFIG_DIR` is set for the child process **only** when `FORGE_CLAUDE_HOME` is set (the harness). With the real home it stays unset, because with it set the CLI keeps `.claude.json` inside the folder instead of beside it.
- Time limits: 120 s for `plugin install`, `plugin marketplace add` and `plugin marketplace update`; 20 s for everything else. A timeout is its own error.
- Write commands (install, uninstall, marketplace add, remove, update) run one at a time. A user string (plugin id, marketplace name, source) that starts with `-` is refused before it reaches the command line.
- A failed command shows the CLI's own message (stderr, trimmed, without the leading `✘ `), unchanged.
- The on/off state of a plugin is Forge's `enabled_plugins` list, keyed by the id `name@marketplace`. A new install is switched on; an uninstall or a removed marketplace takes the keys out.
- No default marketplace and no allow list (spec, goal).
- UI text is English. The uninstall confirmation reads: "Uninstall <name>? Other Claude Code installs on this computer lose it too."
- Code style: match the surrounding crates (plain English doc comments on public items, test names as sentences, signals are `Copy`, `spawn_forever` for work that must survive the component).
- Every commit ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- A source or id that starts with `-` (for example `--help`, `-x`) is never passed to `claude` as a flag (Task 3 test).
- With the real home, `CLAUDE_CONFIG_DIR` is not set on the child process; with `FORGE_CLAUDE_HOME`, it is (Task 3 tests).
- A failed install or uninstall leaves Forge's enabled list exactly as it was; a removed marketplace takes every enabled key of its plugins out (Task 4 tests).
- A second install clicked while one runs waits and runs after it, also when the first one fails (Task 5 tests).
- A plugin that is installed in two scopes shows once, and two marketplaces that offer a plugin of the same name show as two entries with separate on/off keys (Task 2 and Task 5 tests).

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/forge-domain/src/plugin_catalog.rs` | create | `PluginEntry`, `Marketplace`, `PluginSnapshot`, `plugin_id`, `split_plugin_id` |
| `crates/forge-domain/src/lib.rs` | modify | declare and export them |
| `crates/forge-ports/src/plugin_manager_port.rs` | create | `PluginManagerPort`, `PluginManagerError`, mock |
| `crates/forge-ports/src/lib.rs` | modify | declare and export them |
| `crates/forge-adapter-claude-plugins/Cargo.toml` | create | the new crate |
| `crates/forge-adapter-claude-plugins/src/parse.rs` | create | JSON output to `PluginSnapshot`, error message cleaning |
| `crates/forge-adapter-claude-plugins/src/binary.rs` | create | where the bundled `claude` is |
| `crates/forge-adapter-claude-plugins/src/run.rs` | create | run a command with a timeout |
| `crates/forge-adapter-claude-plugins/src/lib.rs` | create | `ClaudePluginCli` implementing the port |
| `crates/forge-adapter-claude-plugins/tests/round_trip.rs` | create | real `claude`, local marketplace (skipped when the binary is absent) |
| `crates/forge-application/src/use_cases/plugin_catalog.rs` | create | `PluginCatalogUseCase`, `CatalogView`, `PluginCatalogError` |
| `crates/forge-application/src/use_cases/mod.rs`, `src/lib.rs` | modify | export them |
| `crates/forge-features/src/customize.rs` | create | pure logic: groups, filter, `Job`, `JobQueue`, `check_source` |
| `crates/forge-features/src/customize_state.rs` | create | `Customize` signals, `use_customize_provider`, `use_customize` |
| `crates/forge-features/src/workspace.rs` | modify | `Active::Customize`, `show_customize`, `WorkspaceServices.plugin_catalog` |
| `crates/forge-features/src/lib.rs` | modify | declare and export |
| `crates/forge-widgets/src/customize_page.rs` | create | page frame, tabs, Skills and Connectors notes, dialogs mount |
| `crates/forge-widgets/src/customize_plugins.rs` | create | Mine list, Explore grid, plugin detail |
| `crates/forge-widgets/src/customize_dialogs.rs` | create | Add marketplace, Manage marketplaces, confirmations |
| `crates/forge-widgets/src/sidebar.rs` | modify | the "Customize" entry |
| `crates/forge-widgets/src/lib.rs` | modify | declare and export `CustomizePage` |
| `crates/forge-pages/src/workspace_page.rs` | modify | provide `Customize`, show `CustomizePage` |
| `crates/forge-app/Cargo.toml`, `src/main.rs` | modify | build the adapter and use case |
| `crates/forge-shared/src/theme.css`, `theme.rs` | modify | styles and a rule test |
| `scripts/harness/add_market_fixture.py`, `test_add_market_fixture.py` | create | a local marketplace with two plugins for the real window check |
| `docs/superpowers/specs/2026-10-08-customize-plugins-design.md`, `roadmap/backlog.md`, `roadmap/README.md` | modify | as built |

Run one crate's tests with `cargo test -p <crate>`; everything with `cargo test --workspace`.

---

### Task 1: Domain types and the port

**Files:**
- Create: `crates/forge-domain/src/plugin_catalog.rs`
- Modify: `crates/forge-domain/src/lib.rs`
- Create: `crates/forge-ports/src/plugin_manager_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Produces: `forge_domain::{PluginEntry, Marketplace, PluginSnapshot, plugin_id, split_plugin_id}`; `forge_ports::{PluginManagerPort, PluginManagerError, MockPluginManagerPort}`.

- [ ] **Step 1: Write the domain file with its tests**

Create `crates/forge-domain/src/plugin_catalog.rs`:

```rust
use std::path::PathBuf;

/// One plugin as the Customize page lists it: installed, or only offered by a marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginEntry {
    /// `name@marketplace`: what `AppConfig::enabled_plugins` stores and what `claude plugin` takes.
    pub id: String,
    pub name: String,
    /// Empty when the id has no `@`.
    pub marketplace: String,
    pub version: Option<String>,
    pub description: String,
    pub installed: bool,
    pub install_path: Option<PathBuf>,
}

/// A marketplace the person (or the terminal's Claude Code) added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marketplace {
    pub name: String,
    /// The source kind as the CLI names it: `github`, `git`, `directory`, ...
    pub kind: String,
    /// The `owner/repo`, the URL or the folder, whichever the kind has. Empty if none.
    pub location: String,
    /// An ISO time from `known_marketplaces.json`, when it has one.
    pub last_updated: Option<String>,
}

/// Everything the Customize page shows about plugins, read in one go.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PluginSnapshot {
    /// Installed plugins and the ones marketplaces offer, sorted by marketplace then name; a plugin
    /// that is installed appears once, with `installed: true`.
    pub plugins: Vec<PluginEntry>,
    pub marketplaces: Vec<Marketplace>,
}

/// `name@marketplace`, or just `name` when `marketplace` is empty.
pub fn plugin_id(name: &str, marketplace: &str) -> String {
    if marketplace.is_empty() {
        name.to_string()
    } else {
        format!("{name}@{marketplace}")
    }
}

/// The two halves of an id; the marketplace is `None` when there is no `@`.
pub fn split_plugin_id(id: &str) -> (&str, Option<&str>) {
    match id.split_once('@') {
        Some((name, marketplace)) => (name, Some(marketplace)),
        None => (id, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_joins_the_name_and_the_marketplace() {
        assert_eq!(plugin_id("hello", "spike-market"), "hello@spike-market");
        assert_eq!(plugin_id("hello", ""), "hello");
    }

    #[test]
    fn an_id_splits_at_the_first_at_sign() {
        assert_eq!(split_plugin_id("hello@spike-market"), ("hello", Some("spike-market")));
        assert_eq!(split_plugin_id("hello"), ("hello", None));
    }
}
```

- [ ] **Step 2: Export it**

In `crates/forge-domain/src/lib.rs`, next to the other `mod` and `pub use` lines for `extension`, add `mod plugin_catalog;` and `pub use plugin_catalog::{plugin_id, split_plugin_id, Marketplace, PluginEntry, PluginSnapshot};`.

- [ ] **Step 3: Run the domain tests**

Run: `cargo test -p forge-domain plugin_catalog`
Expected: 2 tests pass.

- [ ] **Step 4: Write the port with a mock test**

Create `crates/forge-ports/src/plugin_manager_port.rs`:

```rust
use async_trait::async_trait;
use forge_domain::PluginSnapshot;
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PluginManagerError {
    /// The CLI ran and failed: its own message, as it is.
    #[error("{0}")]
    Cli(String),
    /// The `claude` program that ships with the SDK is not where it should be.
    #[error("The Claude program that comes with Forge was not found ({0}).")]
    BinaryMissing(String),
    #[error("The command took too long and was stopped.")]
    TimedOut,
    /// The CLI printed something that is not the JSON this app expects.
    #[error("Could not read the output of claude: {0}")]
    BadOutput(String),
}

/// Plugins and marketplaces, managed by the bundled Claude Code (the same state the terminal's
/// Claude Code uses). `id` is `name@marketplace`. Install scope is always the user's.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait PluginManagerPort: Send + Sync {
    async fn snapshot(&self) -> Result<PluginSnapshot, PluginManagerError>;
    /// `source` is `owner/repo`, a Git URL or a folder.
    async fn add_marketplace(&self, source: &str) -> Result<(), PluginManagerError>;
    async fn remove_marketplace(&self, name: &str) -> Result<(), PluginManagerError>;
    async fn update_marketplace(&self, name: &str) -> Result<(), PluginManagerError>;
    async fn install(&self, id: &str) -> Result<(), PluginManagerError>;
    async fn uninstall(&self, id: &str) -> Result<(), PluginManagerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_an_install_call() {
        let mut mock = MockPluginManagerPort::new();
        mock.expect_install().withf(|id| id == "a@b").times(1).returning(|_| Ok(()));
        assert!(mock.install("a@b").await.is_ok());
    }

    #[test]
    fn a_cli_error_shows_its_own_text() {
        assert_eq!(PluginManagerError::Cli("Path does not exist: /x".into()).to_string(), "Path does not exist: /x");
    }
}
```

In `crates/forge-ports/src/lib.rs` add `mod plugin_manager_port;`, `pub use plugin_manager_port::{PluginManagerError, PluginManagerPort};` and, with the other mock exports, `#[cfg(any(test, feature = "mocks"))] pub use plugin_manager_port::MockPluginManagerPort;`.

- [ ] **Step 5: Run the port tests**

Run: `cargo test -p forge-ports plugin_manager`
Expected: 2 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-domain crates/forge-ports
git commit -m "feat: plugin catalog types and PluginManagerPort (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Reading the CLI's output

**Files:**
- Create: `crates/forge-adapter-claude-plugins/Cargo.toml`, `src/lib.rs` (a stub for now), `src/parse.rs`

**Interfaces:**
- Consumes: `forge_domain::{PluginEntry, Marketplace, PluginSnapshot, split_plugin_id}`, `forge_ports::PluginManagerError`.
- Produces (crate-private, used by Task 3):
  - `parse::parse_snapshot(list_json: &str, marketplaces_json: &str, last_updated: &HashMap<String, String>) -> Result<PluginSnapshot, PluginManagerError>`
  - `parse::parse_last_updated(known_marketplaces_json: &str) -> HashMap<String, String>`
  - `parse::fill_descriptions(snapshot: &mut PluginSnapshot, read: impl Fn(&Path) -> Option<String>)`
  - `parse::clean_message(stderr: &str, stdout: &str, code: Option<i32>) -> String`

The shapes below were captured from the bundled `claude` 2.1.283 (see the spec's Spike section, and the extra run made for this plan): `plugin list --available --json` is `{"installed":[{id, version, scope, enabled, installPath, installedAt, lastUpdated}], "available":[{pluginId, name, description, marketplaceName, source, version?}]}`; `plugin marketplace list --json` is `[{name, source: "github"|"directory"|..., repo? | path? | url?, installLocation}]`; errors go to stderr and start with `✘ `.

- [ ] **Step 1: Create the crate**

`crates/forge-adapter-claude-plugins/Cargo.toml`:

```toml
[package]
name = "forge-adapter-claude-plugins"
version.workspace = true
edition.workspace = true

[dependencies]
forge-domain = { path = "../forge-domain" }
forge-ports = { path = "../forge-ports" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["rt", "sync"] }

[dev-dependencies]
tokio = { version = "1", features = ["rt", "sync", "macros"] }
tempfile = "3"
```

`crates/forge-adapter-claude-plugins/src/lib.rs` (replaced in Task 3):

```rust
mod parse;
```

- [ ] **Step 2: Write the failing tests**

Create `src/parse.rs` with only the tests first (the functions come in Step 4):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::Path;

    // Captured from `claude plugin list --available --json` with `hello` installed and `second` offered.
    const LIST: &str = r#"{
      "installed": [
        {"id":"hello@spike-market","version":"1.0.0","scope":"user","enabled":true,
         "installPath":"/h/plugins/cache/spike-market/hello/1.0.0","installedAt":"2026-10-08T10:57:56.183Z","lastUpdated":"2026-10-08T10:57:56.183Z"}
      ],
      "available": [
        {"pluginId":"second@spike-market","name":"second","description":"second plugin","marketplaceName":"spike-market","source":"./plugins/second"}
      ]
    }"#;

    // Captured from `claude plugin marketplace list --json` (a folder and a GitHub marketplace).
    const MARKETS: &str = r#"[
      {"name":"spike-market","source":"directory","path":"/tmp/mp","installLocation":"/tmp/mp"},
      {"name":"claude-hud","source":"github","repo":"jarrodwatts/claude-hud","installLocation":"/h/plugins/marketplaces/claude-hud"}
    ]"#;

    fn none() -> HashMap<String, String> {
        HashMap::new()
    }

    #[test]
    fn installed_and_offered_plugins_are_one_sorted_list() {
        let snapshot = parse_snapshot(LIST, MARKETS, &none()).unwrap();
        let ids: Vec<&str> = snapshot.plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["hello@spike-market", "second@spike-market"]);
        let hello = &snapshot.plugins[0];
        assert!(hello.installed);
        assert_eq!(hello.name, "hello");
        assert_eq!(hello.marketplace, "spike-market");
        assert_eq!(hello.version.as_deref(), Some("1.0.0"));
        assert_eq!(hello.install_path.as_deref(), Some(Path::new("/h/plugins/cache/spike-market/hello/1.0.0")));
        let second = &snapshot.plugins[1];
        assert!(!second.installed);
        assert_eq!(second.description, "second plugin");
        assert_eq!(second.install_path, None);
    }

    #[test]
    fn a_marketplace_shows_its_repo_url_or_folder() {
        let snapshot = parse_snapshot(LIST, MARKETS, &none()).unwrap();
        assert_eq!(snapshot.marketplaces[0].kind, "directory");
        assert_eq!(snapshot.marketplaces[0].location, "/tmp/mp");
        assert_eq!(snapshot.marketplaces[1].kind, "github");
        assert_eq!(snapshot.marketplaces[1].location, "jarrodwatts/claude-hud");
    }

    #[test]
    fn a_marketplace_takes_its_last_update_from_the_known_marketplaces_file() {
        let known = parse_last_updated(r#"{"claude-hud":{"source":{"source":"github"},"lastUpdated":"2026-02-21T01:21:00.000Z"}}"#);
        let snapshot = parse_snapshot(LIST, MARKETS, &known).unwrap();
        assert_eq!(snapshot.marketplaces[1].last_updated.as_deref(), Some("2026-02-21T01:21:00.000Z"));
        assert_eq!(snapshot.marketplaces[0].last_updated, None);
    }

    #[test]
    fn a_plugin_installed_in_two_scopes_is_listed_once_with_the_user_copy() {
        let list = r#"{"installed":[
          {"id":"a@m","version":"2.0.0","scope":"project","installPath":"/p"},
          {"id":"a@m","version":"1.0.0","scope":"user","installPath":"/u"}],"available":[]}"#;
        let snapshot = parse_snapshot(list, "[]", &none()).unwrap();
        assert_eq!(snapshot.plugins.len(), 1);
        assert_eq!(snapshot.plugins[0].install_path.as_deref(), Some(Path::new("/u")));
    }

    #[test]
    fn two_marketplaces_with_a_plugin_of_the_same_name_stay_two_entries() {
        let list = r#"{"installed":[],"available":[
          {"pluginId":"tool@one","name":"tool","marketplaceName":"one","source":"x"},
          {"pluginId":"tool@two","name":"tool","marketplaceName":"two","source":"x"}]}"#;
        let snapshot = parse_snapshot(list, "[]", &none()).unwrap();
        let ids: Vec<&str> = snapshot.plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["tool@one", "tool@two"]);
    }

    #[test]
    fn output_that_is_not_json_is_a_bad_output_error() {
        let error = parse_snapshot("not json", "[]", &none()).unwrap_err();
        assert!(matches!(error, PluginManagerError::BadOutput(_)), "{error:?}");
        let error = parse_snapshot(LIST, "{", &none()).unwrap_err();
        assert!(matches!(error, PluginManagerError::BadOutput(_)), "{error:?}");
    }

    #[test]
    fn missing_lists_and_unknown_fields_are_fine() {
        let snapshot = parse_snapshot(r#"{"extra":1}"#, "[]", &none()).unwrap();
        assert!(snapshot.plugins.is_empty() && snapshot.marketplaces.is_empty());
    }

    #[test]
    fn an_unreadable_known_marketplaces_file_gives_no_dates() {
        assert!(parse_last_updated("nope").is_empty());
    }

    #[test]
    fn only_installed_plugins_without_a_description_read_their_manifest() {
        let mut snapshot = parse_snapshot(LIST, MARKETS, &none()).unwrap();
        fill_descriptions(&mut snapshot, |path| Some(format!("from {}", path.display())));
        assert_eq!(snapshot.plugins[0].description, "from /h/plugins/cache/spike-market/hello/1.0.0");
        assert_eq!(snapshot.plugins[1].description, "second plugin", "an offered plugin keeps the marketplace's text");
    }

    #[test]
    fn a_failed_command_shows_stderr_without_the_cross() {
        assert_eq!(clean_message("✘ Path does not exist: /no/such/dir\n", "", Some(1)), "Path does not exist: /no/such/dir");
    }

    #[test]
    fn stdout_is_used_when_stderr_is_empty_and_the_exit_code_when_both_are() {
        assert_eq!(clean_message("", "Something went wrong\n", Some(1)), "Something went wrong");
        assert_eq!(clean_message("  \n", "", Some(3)), "claude exited with code 3");
        assert_eq!(clean_message("", "", None), "claude stopped without an exit code");
    }
}
```

- [ ] **Step 3: Run to see it fail**

Run: `cargo test -p forge-adapter-claude-plugins`
Expected: compile error (`parse_snapshot` not found).

- [ ] **Step 4: Write the implementation above the tests**

Put this at the top of `src/parse.rs`:

```rust
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forge_domain::{split_plugin_id, Marketplace, PluginEntry, PluginSnapshot};
use forge_ports::PluginManagerError;
use serde::Deserialize;

#[derive(Deserialize)]
struct ListOutput {
    #[serde(default)]
    installed: Vec<InstalledJson>,
    #[serde(default)]
    available: Vec<AvailableJson>,
}

#[derive(Deserialize)]
struct InstalledJson {
    id: String,
    version: Option<String>,
    scope: Option<String>,
    #[serde(rename = "installPath")]
    install_path: Option<String>,
}

#[derive(Deserialize)]
struct AvailableJson {
    #[serde(rename = "pluginId")]
    plugin_id: String,
    name: String,
    description: Option<String>,
    #[serde(rename = "marketplaceName")]
    marketplace_name: String,
    version: Option<String>,
}

#[derive(Deserialize)]
struct MarketplaceJson {
    name: String,
    source: String,
    repo: Option<String>,
    url: Option<String>,
    path: Option<String>,
}

/// Joins `plugin list --available --json` and `plugin marketplace list --json` into one snapshot.
/// `last_updated` maps a marketplace name to the time `known_marketplaces.json` has for it.
pub fn parse_snapshot(
    list_json: &str,
    marketplaces_json: &str,
    last_updated: &HashMap<String, String>,
) -> Result<PluginSnapshot, PluginManagerError> {
    let list: ListOutput =
        serde_json::from_str(list_json).map_err(|error| PluginManagerError::BadOutput(format!("plugin list: {error}")))?;
    let markets: Vec<MarketplaceJson> = serde_json::from_str(marketplaces_json)
        .map_err(|error| PluginManagerError::BadOutput(format!("marketplace list: {error}")))?;

    let mut installed = list.installed;
    // The user scope first (the sort is stable), so a plugin installed twice shows the user's copy.
    installed.sort_by_key(|item| item.scope.as_deref() != Some("user"));
    let mut seen: HashSet<String> = HashSet::new();
    let mut plugins: Vec<PluginEntry> = Vec::new();
    for item in installed {
        if !seen.insert(item.id.clone()) {
            continue;
        }
        let (name, marketplace) = split_plugin_id(&item.id);
        plugins.push(PluginEntry {
            name: name.to_string(),
            marketplace: marketplace.unwrap_or_default().to_string(),
            id: item.id,
            version: item.version,
            description: String::new(),
            installed: true,
            install_path: item.install_path.map(PathBuf::from),
        });
    }
    for item in list.available {
        if !seen.insert(item.plugin_id.clone()) {
            continue;
        }
        plugins.push(PluginEntry {
            id: item.plugin_id,
            name: item.name,
            marketplace: item.marketplace_name,
            version: item.version,
            description: item.description.unwrap_or_default(),
            installed: false,
            install_path: None,
        });
    }
    plugins.sort_by(|a, b| a.marketplace.cmp(&b.marketplace).then_with(|| a.name.cmp(&b.name)));

    let marketplaces = markets
        .into_iter()
        .map(|market| Marketplace {
            last_updated: last_updated.get(&market.name).cloned(),
            location: market.repo.or(market.url).or(market.path).unwrap_or_default(),
            kind: market.source,
            name: market.name,
        })
        .collect();
    Ok(PluginSnapshot { plugins, marketplaces })
}

/// The `lastUpdated` of each marketplace in `known_marketplaces.json`. A file that cannot be read gives no dates.
pub fn parse_last_updated(known_marketplaces_json: &str) -> HashMap<String, String> {
    let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(known_marketplaces_json) else {
        return HashMap::new();
    };
    map.into_iter()
        .filter_map(|(name, value)| Some((name, value.get("lastUpdated")?.as_str()?.to_string())))
        .collect()
}

/// An installed plugin has no description in the CLI's list; `read` gets its install folder and
/// returns the description from its manifest.
pub fn fill_descriptions(snapshot: &mut PluginSnapshot, read: impl Fn(&Path) -> Option<String>) {
    for plugin in snapshot.plugins.iter_mut().filter(|plugin| plugin.installed && plugin.description.is_empty()) {
        if let Some(text) = plugin.install_path.as_deref().and_then(&read) {
            plugin.description = text;
        }
    }
}

/// What to show for a failed command: stderr, else stdout, else the exit code. The CLI starts an
/// error with `✘ `, which is dropped.
pub fn clean_message(stderr: &str, stdout: &str, code: Option<i32>) -> String {
    for text in [stderr, stdout] {
        let text = text.trim().trim_start_matches('✘').trim();
        if !text.is_empty() {
            return text.to_string();
        }
    }
    match code {
        Some(code) => format!("claude exited with code {code}"),
        None => "claude stopped without an exit code".to_string(),
    }
}
```

- [ ] **Step 5: Run the tests**

Run: `cargo test -p forge-adapter-claude-plugins`
Expected: 11 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-plugins Cargo.lock
git commit -m "feat: parse the output of claude plugin list and marketplace list (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Finding and running the bundled `claude`

**Files:**
- Create: `crates/forge-adapter-claude-plugins/src/binary.rs`, `src/run.rs`, `tests/round_trip.rs`
- Modify: `crates/forge-adapter-claude-plugins/src/lib.rs`

**Interfaces:**
- Consumes: Task 2's `parse::*`, `forge_ports::{PluginManagerPort, PluginManagerError}`.
- Produces: `forge_adapter_claude_plugins::{ClaudePluginCli, claude_binary_for}`:
  - `ClaudePluginCli::new(binary: PathBuf, claude_home: PathBuf, config_dir: Option<PathBuf>) -> ClaudePluginCli` (`config_dir` is `Some` only when `FORGE_CLAUDE_HOME` is set; `claude_home` is where `plugins/known_marketplaces.json` is read)
  - `claude_binary_for(sidecar_script: &Path) -> Option<PathBuf>` (this machine's os and arch)

- [ ] **Step 1: Write the failing tests for `binary.rs`**

```rust
use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sdk_names_its_platform_packages_darwin_win32_and_arm64_x64() {
        assert_eq!(sdk_platform("macos", "aarch64").as_deref(), Some("darwin-arm64"));
        assert_eq!(sdk_platform("macos", "x86_64").as_deref(), Some("darwin-x64"));
        assert_eq!(sdk_platform("windows", "x86_64").as_deref(), Some("win32-x64"));
        assert_eq!(sdk_platform("windows", "aarch64").as_deref(), Some("win32-arm64"));
        assert_eq!(sdk_platform("linux", "x86_64").as_deref(), Some("linux-x64"));
        assert_eq!(sdk_platform("freebsd", "x86_64"), None);
        assert_eq!(sdk_platform("macos", "riscv64"), None);
    }

    #[test]
    fn the_binary_sits_in_node_modules_beside_the_sidecars_dist_folder() {
        let path = claude_binary("/runtime/sidecar/dist/index.js".as_ref(), "macos", "aarch64").unwrap();
        assert_eq!(path, PathBuf::from("/runtime/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-darwin-arm64/claude"));
    }

    #[test]
    fn on_windows_the_file_is_claude_exe() {
        let path = claude_binary("C:/runtime/sidecar/dist/index.js".as_ref(), "windows", "x86_64").unwrap();
        assert!(path.ends_with("claude-agent-sdk-win32-x64/claude.exe"), "{path:?}");
    }

    #[test]
    fn a_script_path_with_no_parent_folders_gives_none() {
        assert_eq!(claude_binary("index.js".as_ref(), "macos", "aarch64"), None);
    }
}
```

- [ ] **Step 2: Implement `binary.rs` above the tests**

```rust
/// The folder name the SDK uses for a platform's Claude package, or `None` for a platform it has none for.
fn sdk_platform(os: &str, arch: &str) -> Option<String> {
    let os = match os {
        "macos" => "darwin",
        "windows" => "win32",
        "linux" => "linux",
        _ => return None,
    };
    let arch = match arch {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        _ => return None,
    };
    Some(format!("{os}-{arch}"))
}

/// Where the Claude program that the SDK ships is, from the sidecar script
/// (`<...>/sidecar/dist/index.js`): `<...>/sidecar/node_modules/@anthropic-ai/claude-agent-sdk-<platform>/claude`.
/// A development build and a packaged one keep the same layout (`scripts/package/stage-runtime.mjs`).
fn claude_binary(sidecar_script: &Path, os: &str, arch: &str) -> Option<PathBuf> {
    let sidecar = sidecar_script.parent()?.parent()?;
    let file = if os == "windows" { "claude.exe" } else { "claude" };
    Some(
        sidecar
            .join("node_modules")
            .join("@anthropic-ai")
            .join(format!("claude-agent-sdk-{}", sdk_platform(os, arch)?))
            .join(file),
    )
}

/// `claude_binary` for the machine Forge is running on.
pub fn claude_binary_for(sidecar_script: &Path) -> Option<PathBuf> {
    claude_binary(sidecar_script, std::env::consts::OS, std::env::consts::ARCH)
}
```

(`sdk_platform` and `claude_binary` stay private; the tests are in the same file.)

- [ ] **Step 3: Run**

Run: `cargo test -p forge-adapter-claude-plugins binary`
Expected: 4 pass (add `mod binary;` to `lib.rs` first: `mod binary;` and `pub use binary::claude_binary_for;`).

- [ ] **Step 4: Write the failing tests for `run.rs`**

These use a small shell script as a stand-in program, so they run on macOS and Linux only.

```rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("fake-claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn invocation<'a>(program: &'a Path, config_dir: Option<&'a Path>, args: &'a [&'a str], seconds: u64) -> Invocation<'a> {
        Invocation { program, config_dir, args, timeout: Duration::from_secs(seconds) }
    }

    #[test]
    fn a_successful_command_returns_its_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"args: $@\"");
        let out = run(&invocation(&program, None, &["plugin", "list"], 5)).unwrap();
        assert_eq!(out.trim(), "args: plugin list");
    }

    #[test]
    fn a_failed_command_returns_its_stderr_without_the_cross() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo '✘ Path does not exist: /x' >&2\nexit 1");
        let error = run(&invocation(&program, None, &["x"], 5)).unwrap_err();
        assert_eq!(error, PluginManagerError::Cli("Path does not exist: /x".into()));
    }

    #[test]
    fn a_command_that_runs_too_long_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "exec sleep 5");
        let started = Instant::now();
        let error = run(&Invocation { program: &program, config_dir: None, args: &[], timeout: Duration::from_millis(200) }).unwrap_err();
        assert_eq!(error, PluginManagerError::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_missing_program_is_reported_with_its_path() {
        let error = run(&invocation(Path::new("/no/such/claude"), None, &[], 5)).unwrap_err();
        assert_eq!(error, PluginManagerError::BinaryMissing("/no/such/claude".into()));
    }

    #[test]
    fn the_config_dir_is_passed_only_when_one_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"${CLAUDE_CONFIG_DIR:-unset}\"");
        let with = run(&invocation(&program, Some(Path::new("/fixture/.claude")), &[], 5)).unwrap();
        assert_eq!(with.trim(), "/fixture/.claude");
        // The variable of the test process itself must not leak through either.
        let without = run(&invocation(&program, None, &[], 5)).unwrap();
        assert_eq!(without.trim(), "unset");
    }

    #[test]
    fn git_is_told_never_to_ask_for_a_password() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"$GIT_TERMINAL_PROMPT\"");
        assert_eq!(run(&invocation(&program, None, &[], 5)).unwrap().trim(), "0");
    }
}
```

Note: the `without` case must hold even if the machine running the tests has `CLAUDE_CONFIG_DIR` set, so `run` calls `env_remove("CLAUDE_CONFIG_DIR")` when `config_dir` is `None`.

- [ ] **Step 5: Implement `run.rs` above the tests**

```rust
use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_ports::PluginManagerError;

use crate::parse::clean_message;

pub(crate) struct Invocation<'a> {
    pub program: &'a Path,
    /// `Some` only when Forge runs against another Claude home (`FORGE_CLAUDE_HOME`).
    pub config_dir: Option<&'a Path>,
    pub args: &'a [&'a str],
    pub timeout: Duration,
}

/// Runs the program, waits at most `timeout`, and returns what it wrote to stdout. A non-zero exit
/// is `Cli` with the message from `clean_message`.
pub(crate) fn run(invocation: &Invocation) -> Result<String, PluginManagerError> {
    if !invocation.program.is_file() {
        return Err(PluginManagerError::BinaryMissing(invocation.program.display().to_string()));
    }
    let mut command = Command::new(invocation.program);
    command
        .args(invocation.args)
        // A private Git server must fail with a message, not wait for a password nobody can type.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match invocation.config_dir {
        Some(dir) => command.env("CLAUDE_CONFIG_DIR", dir),
        None => command.env_remove("CLAUDE_CONFIG_DIR"),
    };
    let mut child = command.spawn().map_err(|error| PluginManagerError::Cli(error.to_string()))?;
    // Both pipes are drained on their own threads so a large output cannot fill a pipe and stop the program.
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_thread = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err_thread = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + invocation.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            // The reader threads are left to finish on their own: a grandchild may hold the pipes open.
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PluginManagerError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(PluginManagerError::Cli(error.to_string())),
        }
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    if status.success() {
        Ok(stdout)
    } else {
        Err(PluginManagerError::Cli(clean_message(&stderr, &stdout, status.code())))
    }
}
```

Add `mod run;` to `lib.rs`.

- [ ] **Step 6: Run**

Run: `cargo test -p forge-adapter-claude-plugins run::`
Expected: 6 pass.

- [ ] **Step 7: Write the adapter with its tests**

Replace `src/lib.rs`:

```rust
mod binary;
mod parse;
mod run;

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::PluginSnapshot;
use forge_ports::{PluginManagerError, PluginManagerPort};
use tokio::sync::Mutex;

pub use binary::claude_binary_for;
use run::{run, Invocation};

const QUICK: Duration = Duration::from_secs(20);
const SLOW: Duration = Duration::from_secs(120);

/// Manages plugins and marketplaces by running the `claude plugin ...` commands of the Claude Code
/// that ships with the SDK, so the state is the one the terminal's Claude Code uses.
pub struct ClaudePluginCli {
    binary: PathBuf,
    /// Where `plugins/known_marketplaces.json` is read from.
    claude_home: PathBuf,
    /// Passed as `CLAUDE_CONFIG_DIR`; `None` for the real home (see the plan's Global Constraints).
    config_dir: Option<PathBuf>,
    /// Write commands run one at a time: two of them would write the settings file together.
    write_lock: Mutex<()>,
}

impl ClaudePluginCli {
    pub fn new(binary: PathBuf, claude_home: PathBuf, config_dir: Option<PathBuf>) -> Self {
        Self { binary, claude_home, config_dir, write_lock: Mutex::new(()) }
    }

    async fn exec(&self, args: Vec<String>, timeout: Duration) -> Result<String, PluginManagerError> {
        let binary = self.binary.clone();
        let config_dir = self.config_dir.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run(&Invocation { program: &binary, config_dir: config_dir.as_deref(), args: &refs, timeout })
        })
        .await
        .map_err(|error| PluginManagerError::Cli(error.to_string()))?
    }

    async fn write(&self, args: &[&str], user_text: &str, timeout: Duration) -> Result<(), PluginManagerError> {
        refuse_flag_like(user_text)?;
        let _one_at_a_time = self.write_lock.lock().await;
        self.exec(args.iter().map(|arg| arg.to_string()).collect(), timeout).await.map(|_| ())
    }
}

/// A plugin id, marketplace name or source that starts with `-` would be read as an option.
fn refuse_flag_like(text: &str) -> Result<(), PluginManagerError> {
    if text.trim_start().starts_with('-') {
        return Err(PluginManagerError::Cli(format!("\"{text}\" cannot start with a dash.")));
    }
    Ok(())
}

fn read_manifest_description(install_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(install_path.join(".claude-plugin").join("plugin.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(value.get("description")?.as_str()?.to_string())
}

#[async_trait]
impl PluginManagerPort for ClaudePluginCli {
    async fn snapshot(&self) -> Result<PluginSnapshot, PluginManagerError> {
        let list = self.exec(["plugin", "list", "--available", "--json"].map(String::from).to_vec(), QUICK).await?;
        let markets = self.exec(["plugin", "marketplace", "list", "--json"].map(String::from).to_vec(), QUICK).await?;
        let known_file = self.claude_home.join("plugins").join("known_marketplaces.json");
        tokio::task::spawn_blocking(move || {
            let dates = parse::parse_last_updated(&std::fs::read_to_string(known_file).unwrap_or_default());
            let mut snapshot = parse::parse_snapshot(&list, &markets, &dates)?;
            parse::fill_descriptions(&mut snapshot, read_manifest_description);
            Ok(snapshot)
        })
        .await
        .map_err(|error| PluginManagerError::Cli(error.to_string()))?
    }

    async fn add_marketplace(&self, source: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "add", source], source, SLOW).await
    }

    async fn remove_marketplace(&self, name: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "remove", name], name, QUICK).await
    }

    async fn update_marketplace(&self, name: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "update", name], name, SLOW).await
    }

    async fn install(&self, id: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "install", id], id, SLOW).await
    }

    async fn uninstall(&self, id: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "uninstall", id], id, QUICK).await
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // A stand-in `claude` that writes its arguments to a file, so a test can see what was run.
    fn recording_cli(dir: &Path) -> (ClaudePluginCli, PathBuf) {
        let log = dir.join("calls.log");
        let program = dir.join("fake-claude");
        std::fs::write(&program, format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display())).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        (ClaudePluginCli::new(program, dir.join(".claude"), None), log)
    }

    #[tokio::test]
    async fn install_runs_plugin_install_with_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        cli.install("hello@market").await.unwrap();
        assert_eq!(std::fs::read_to_string(log).unwrap().trim(), "plugin install hello@market");
    }

    #[tokio::test]
    async fn a_source_or_id_that_starts_with_a_dash_is_refused_and_nothing_runs() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        for text in ["--help", "-x", "  --version"] {
            assert!(matches!(cli.add_marketplace(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.install(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.uninstall(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.remove_marketplace(text).await, Err(PluginManagerError::Cli(_))), "{text}");
        }
        assert!(!log.exists(), "the program must not have been started");
    }

    #[tokio::test]
    async fn a_source_with_spaces_is_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        cli.add_marketplace("/tmp/my market").await.unwrap();
        // The recorder joins arguments with spaces; the point is that no shell was involved.
        assert_eq!(std::fs::read_to_string(log).unwrap().trim(), "plugin marketplace add /tmp/my market");
    }

    #[tokio::test]
    async fn the_manifest_description_is_read_from_the_install_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
        std::fs::write(dir.path().join(".claude-plugin/plugin.json"), r#"{"name":"x","description":"does x"}"#).unwrap();
        assert_eq!(read_manifest_description(dir.path()).as_deref(), Some("does x"));
        assert_eq!(read_manifest_description(&dir.path().join("nope")), None);
    }
}
```

- [ ] **Step 8: Write the round trip test against the real binary**

Create `tests/round_trip.rs`:

```rust
//! Runs the real `claude` that ships with the SDK against a temporary config folder and a local
//! marketplace. Skipped when the program is not installed (`npm ci` in `sidecar/` installs it).

use std::path::PathBuf;

use forge_adapter_claude_plugins::{claude_binary_for, ClaudePluginCli};
use forge_ports::PluginManagerPort;

fn binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FORGE_CLAUDE_BIN") {
        return Some(PathBuf::from(path));
    }
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../sidecar/dist/index.js");
    claude_binary_for(&script).filter(|path| path.is_file())
}

fn write(path: PathBuf, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

#[tokio::test]
async fn a_local_marketplace_can_be_added_installed_from_uninstalled_and_removed() {
    let Some(binary) = binary() else {
        eprintln!("skipped: the bundled claude is not installed");
        return;
    };
    let tmp = tempfile::tempdir().unwrap();
    let market = tmp.path().join("market");
    write(
        market.join(".claude-plugin/marketplace.json"),
        r#"{"name":"test-market","owner":{"name":"t"},"plugins":[
            {"name":"hello","source":"./plugins/hello","description":"first"},
            {"name":"second","source":"./plugins/second","description":"second"}]}"#,
    );
    write(market.join("plugins/hello/.claude-plugin/plugin.json"), r#"{"name":"hello","version":"1.0.0","description":"first"}"#);
    write(market.join("plugins/second/.claude-plugin/plugin.json"), r#"{"name":"second","version":"0.2.0","description":"second"}"#);

    let home = tmp.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let cli = ClaudePluginCli::new(binary, home.clone(), Some(home));

    cli.add_marketplace(market.to_str().unwrap()).await.unwrap();
    let snapshot = cli.snapshot().await.unwrap();
    assert_eq!(snapshot.marketplaces.len(), 1);
    assert_eq!(snapshot.marketplaces[0].name, "test-market");
    let ids: Vec<&str> = snapshot.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids, ["hello@test-market", "second@test-market"]);
    assert!(snapshot.plugins.iter().all(|p| !p.installed));

    cli.install("hello@test-market").await.unwrap();
    let snapshot = cli.snapshot().await.unwrap();
    let hello = snapshot.plugins.iter().find(|p| p.id == "hello@test-market").unwrap();
    assert!(hello.installed);
    assert_eq!(hello.description, "first", "the description is read from the installed manifest");
    assert!(!snapshot.plugins.iter().find(|p| p.id == "second@test-market").unwrap().installed);

    cli.uninstall("hello@test-market").await.unwrap();
    assert!(!cli.snapshot().await.unwrap().plugins.iter().any(|p| p.installed));

    let error = cli.install("nope@test-market").await.unwrap_err();
    assert!(error.to_string().contains("not found"), "{error}");

    cli.remove_marketplace("test-market").await.unwrap();
    assert!(cli.snapshot().await.unwrap().marketplaces.is_empty());
}
```

- [ ] **Step 9: Make sure the binary is there, then run everything**

The worktree has no `node_modules`. Install them once: `cd sidecar && npm ci && cd ..`
Run: `cargo test -p forge-adapter-claude-plugins`
Expected: every test passes, and `a_local_marketplace_can_be_added_installed_from_uninstalled_and_removed` runs (it prints nothing; "skipped" must not appear with `-- --nocapture`).

- [ ] **Step 10: Commit**

```bash
git add crates/forge-adapter-claude-plugins Cargo.lock
git commit -m "feat: run the bundled claude to manage plugins and marketplaces (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The use case

**Files:**
- Create: `crates/forge-application/src/use_cases/plugin_catalog.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`, `crates/forge-application/src/lib.rs`

**Interfaces:**
- Consumes: `PluginManagerPort`, `ConfigRepositoryPort` (`load() -> Result<AppConfig, ConfigError>`, `save(&AppConfig)`), `forge_domain::{PluginSnapshot, split_plugin_id}`.
- Produces:
  - `CatalogView { pub snapshot: PluginSnapshot, pub enabled: Vec<String> }` (`Clone, Debug, PartialEq`)
  - `PluginCatalogError { Manager(PluginManagerError), Config(ConfigError) }` (`Display` is the inner message)
  - `PluginCatalogUseCase::new(manager: Arc<dyn PluginManagerPort>, config: Arc<dyn ConfigRepositoryPort>)` with async methods `catalog() -> Result<CatalogView, PluginCatalogError>`, `install(&self, id: &str)`, `uninstall(&self, id: &str)`, `set_enabled(&self, id: &str, on: bool)`, `add_marketplace(&self, source: &str)`, `remove_marketplace(&self, name: &str)`, `update_marketplace(&self, name: &str)`; all but `catalog` return `Result<(), PluginCatalogError>`.

- [ ] **Step 1: Write the failing tests**

Create the file with the tests at the bottom (the implementation goes above them in Step 3). Look at `use_cases/toggles.rs` tests for the mock style used with `MockConfigRepositoryPort`.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{AppConfig, PluginSnapshot};
    use forge_ports::{MockConfigRepositoryPort, MockPluginManagerPort};
    use std::sync::Mutex as StdMutex;

    // A config repository that keeps one AppConfig in memory and counts saves.
    fn memory_config(enabled: &[&str]) -> (Arc<MockConfigRepositoryPort>, Arc<StdMutex<AppConfig>>, Arc<StdMutex<u32>>) {
        let state = Arc::new(StdMutex::new(AppConfig {
            enabled_plugins: enabled.iter().map(|s| s.to_string()).collect(),
            ..AppConfig::default()
        }));
        let saves = Arc::new(StdMutex::new(0));
        let mut mock = MockConfigRepositoryPort::new();
        let loaded = state.clone();
        mock.expect_load().returning(move || Ok(loaded.lock().unwrap().clone()));
        let stored = state.clone();
        let counted = saves.clone();
        mock.expect_save().returning(move |config| {
            *stored.lock().unwrap() = config.clone();
            *counted.lock().unwrap() += 1;
            Ok(())
        });
        (Arc::new(mock), state, saves)
    }

    fn use_case(manager: MockPluginManagerPort, config: Arc<MockConfigRepositoryPort>) -> PluginCatalogUseCase {
        PluginCatalogUseCase::new(Arc::new(manager), config)
    }

    #[tokio::test]
    async fn the_catalog_is_the_snapshot_and_the_enabled_list() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_snapshot().returning(|| Ok(PluginSnapshot::default()));
        let (config, _, _) = memory_config(&["a@m"]);
        let view = use_case(manager, config).catalog().await.unwrap();
        assert_eq!(view.enabled, vec!["a@m".to_string()]);
        assert_eq!(view.snapshot, PluginSnapshot::default());
    }

    #[tokio::test]
    async fn an_install_switches_the_plugin_on_in_forge() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().withf(|id| id == "hello@m").times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["other@m"]);
        use_case(manager, config).install("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m", "hello@m"]);
    }

    #[tokio::test]
    async fn an_install_that_fails_leaves_the_enabled_list_alone() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().returning(|_| Err(PluginManagerError::Cli("clone failed".into())));
        let (config, state, saves) = memory_config(&["other@m"]);
        let error = use_case(manager, config).install("hello@m").await.unwrap_err();
        assert_eq!(error.to_string(), "clone failed");
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m"]);
        assert_eq!(*saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn an_install_of_a_plugin_that_is_already_switched_on_does_not_duplicate_it() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_install().returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["hello@m"]);
        use_case(manager, config).install("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["hello@m"]);
    }

    #[tokio::test]
    async fn an_uninstall_takes_the_plugin_out_of_the_enabled_list() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_uninstall().times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["hello@m", "other@m"]);
        use_case(manager, config).uninstall("hello@m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["other@m"]);
    }

    #[tokio::test]
    async fn an_uninstall_that_fails_leaves_the_enabled_list_alone() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_uninstall().returning(|_| Err(PluginManagerError::TimedOut));
        let (config, state, saves) = memory_config(&["hello@m"]);
        assert!(use_case(manager, config).uninstall("hello@m").await.is_err());
        assert_eq!(state.lock().unwrap().enabled_plugins, ["hello@m"]);
        assert_eq!(*saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn switching_on_and_off_is_explicit_and_never_touches_the_manager() {
        let manager = MockPluginManagerPort::new(); // any call to it would panic
        let (config, state, _) = memory_config(&[]);
        let use_case = use_case(manager, config);
        use_case.set_enabled("a@m", true).await.unwrap();
        use_case.set_enabled("a@m", true).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["a@m"]);
        use_case.set_enabled("a@m", false).await.unwrap();
        use_case.set_enabled("a@m", false).await.unwrap();
        assert!(state.lock().unwrap().enabled_plugins.is_empty());
    }

    #[tokio::test]
    async fn removing_a_marketplace_takes_every_enabled_key_of_its_plugins_out() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_remove_marketplace().withf(|name| name == "m").times(1).returning(|_| Ok(()));
        let (config, state, _) = memory_config(&["a@m", "b@m", "c@other", "m@other"]);
        use_case(manager, config).remove_marketplace("m").await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_plugins, ["c@other", "m@other"]);
    }

    #[tokio::test]
    async fn adding_and_updating_a_marketplace_change_nothing_in_forge() {
        let mut manager = MockPluginManagerPort::new();
        manager.expect_add_marketplace().returning(|_| Ok(()));
        manager.expect_update_marketplace().returning(|_| Ok(()));
        let (config, _, saves) = memory_config(&["a@m"]);
        let use_case = use_case(manager, config);
        use_case.add_marketplace("owner/repo").await.unwrap();
        use_case.update_marketplace("m").await.unwrap();
        assert_eq!(*saves.lock().unwrap(), 0);
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-application plugin_catalog`
Expected: compile error (`PluginCatalogUseCase` not found; add `mod plugin_catalog;` to `use_cases/mod.rs` first).

- [ ] **Step 3: Implement above the tests**

```rust
use std::sync::Arc;

use forge_domain::{split_plugin_id, AppConfig, PluginSnapshot};
use forge_ports::{ConfigError, ConfigRepositoryPort, PluginManagerError, PluginManagerPort};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PluginCatalogError {
    #[error("{0}")]
    Manager(#[from] PluginManagerError),
    #[error("Could not save the Forge settings: {0}")]
    Config(#[from] ConfigError),
}

/// What the Customize page draws: the plugins and marketplaces, and which plugins Forge has switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogView {
    pub snapshot: PluginSnapshot,
    pub enabled: Vec<String>,
}

/// Installing, uninstalling and finding plugins. The install itself belongs to the bundled Claude
/// Code (shared with the terminal); whether Forge's sessions use a plugin is Forge's own
/// `enabled_plugins` list, which this keeps in step.
pub struct PluginCatalogUseCase {
    manager: Arc<dyn PluginManagerPort>,
    config: Arc<dyn ConfigRepositoryPort>,
}

impl PluginCatalogUseCase {
    pub fn new(manager: Arc<dyn PluginManagerPort>, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { manager, config }
    }

    pub async fn catalog(&self) -> Result<CatalogView, PluginCatalogError> {
        let snapshot = self.manager.snapshot().await?;
        let enabled = self.config.load().await?.enabled_plugins;
        Ok(CatalogView { snapshot, enabled })
    }

    /// Installs, then switches the plugin on in Forge. If the install fails nothing else changes.
    pub async fn install(&self, id: &str) -> Result<(), PluginCatalogError> {
        self.manager.install(id).await?;
        self.edit_enabled(|list| {
            if !list.iter().any(|key| key == id) {
                list.push(id.to_string());
            }
        })
        .await
    }

    pub async fn uninstall(&self, id: &str) -> Result<(), PluginCatalogError> {
        self.manager.uninstall(id).await?;
        self.edit_enabled(|list| list.retain(|key| key != id)).await
    }

    /// Switches a plugin on or off in Forge only (Claude Code's own `enabledPlugins` is not touched).
    pub async fn set_enabled(&self, id: &str, on: bool) -> Result<(), PluginCatalogError> {
        self.edit_enabled(|list| {
            let present = list.iter().any(|key| key == id);
            if on && !present {
                list.push(id.to_string());
            } else if !on {
                list.retain(|key| key != id);
            }
        })
        .await
    }

    pub async fn add_marketplace(&self, source: &str) -> Result<(), PluginCatalogError> {
        Ok(self.manager.add_marketplace(source).await?)
    }

    /// Removing a marketplace removes its plugins too, so their keys leave the enabled list.
    pub async fn remove_marketplace(&self, name: &str) -> Result<(), PluginCatalogError> {
        self.manager.remove_marketplace(name).await?;
        self.edit_enabled(|list| list.retain(|key| split_plugin_id(key).1 != Some(name))).await
    }

    pub async fn update_marketplace(&self, name: &str) -> Result<(), PluginCatalogError> {
        Ok(self.manager.update_marketplace(name).await?)
    }

    // Loads, edits and saves the list; saves only when it changed.
    async fn edit_enabled(&self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), PluginCatalogError> {
        let mut config: AppConfig = self.config.load().await?;
        let before = config.enabled_plugins.clone();
        edit(&mut config.enabled_plugins);
        if config.enabled_plugins != before {
            self.config.save(&config).await?;
        }
        Ok(())
    }
}
```

The test `an_install_of_a_plugin_that_is_already_switched_on_does_not_duplicate_it` and `adding_and_updating...` rely on "saves only when it changed".

- [ ] **Step 4: Export**

In `use_cases/mod.rs` add `mod plugin_catalog;` and `pub use plugin_catalog::{CatalogView, PluginCatalogError, PluginCatalogUseCase};`, and make `forge-application/src/lib.rs` re-export the same three names where it re-exports `ExtensionsUseCase`. (Match how `ExtensionsUseCase` is exported; if `PluginManagerError` is needed by tests, `forge_ports` is already a dependency.)

- [ ] **Step 5: Run**

Run: `cargo test -p forge-application plugin_catalog`
Expected: 9 pass.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-application
git commit -m "feat: PluginCatalogUseCase keeps Forge's enabled plugins in step with installs (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Pure page logic

**Files:**
- Create: `crates/forge-features/src/customize.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::PluginEntry`.
- Produces (all `pub`, exported from `forge_features`):
  - `enum CustomizeTab { Skills, Connectors, Plugins }`, `enum CustomizeMode { Mine, Explore }` (both `Debug, Clone, Copy, PartialEq, Eq`)
  - `struct PluginRow { pub entry: PluginEntry, pub enabled: bool }`
  - `struct PluginGroup { pub marketplace: String, pub rows: Vec<PluginRow> }`
  - `fn mine_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup>` (installed only)
  - `fn explore_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup>` (all)
  - `fn group_title(marketplace: &str) -> String` (`"(no marketplace)"` for empty)
  - `enum Job { Install(String), Uninstall(String), SetEnabled(String, bool), AddMarketplace(String), RemoveMarketplace(String), UpdateMarketplace(String) }` with `fn plugin_id(&self) -> Option<&str>`
  - `struct JobQueue` (`Default`) with `fn push(&mut self, job: Job) -> Option<Job>` (the job to start now, if the queue was idle), `fn finish(&mut self) -> Option<Job>` (the next job to start, if any), `fn running(&self) -> Option<&Job>`, `fn waits_for(&self, id: &str) -> bool`
  - `fn check_source(input: &str) -> Result<String, &'static str>`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::PluginEntry;

    fn entry(name: &str, marketplace: &str, installed: bool, description: &str) -> PluginEntry {
        PluginEntry {
            id: format!("{name}@{marketplace}"),
            name: name.into(),
            marketplace: marketplace.into(),
            version: None,
            description: description.into(),
            installed,
            install_path: None,
        }
    }

    fn sample() -> Vec<PluginEntry> {
        vec![
            entry("alpha", "one", true, "Finds bugs"),
            entry("beta", "one", false, "Writes docs"),
            entry("alpha", "two", true, "Another alpha"),
        ]
    }

    #[test]
    fn mine_lists_installed_plugins_grouped_by_marketplace_in_order() {
        let groups = mine_groups(&sample(), &[], "");
        let names: Vec<(&str, Vec<&str>)> =
            groups.iter().map(|g| (g.marketplace.as_str(), g.rows.iter().map(|r| r.entry.name.as_str()).collect())).collect();
        assert_eq!(names, [("one", vec!["alpha"]), ("two", vec!["alpha"])]);
    }

    #[test]
    fn explore_lists_everything_and_keeps_the_installed_flag() {
        let groups = explore_groups(&sample(), &[], "");
        assert_eq!(groups[0].rows.len(), 2);
        assert!(groups[0].rows[0].entry.installed && !groups[0].rows[1].entry.installed);
    }

    #[test]
    fn a_row_is_on_when_its_full_id_is_in_the_enabled_list() {
        let groups = mine_groups(&sample(), &["alpha@two".to_string()], "");
        assert!(!groups[0].rows[0].enabled, "alpha@one is off");
        assert!(groups[1].rows[0].enabled, "alpha@two is on");
    }

    #[test]
    fn the_search_matches_name_description_and_marketplace_ignoring_case() {
        assert_eq!(mine_groups(&sample(), &[], "ALPHA").len(), 2);
        assert_eq!(explore_groups(&sample(), &[], "docs").len(), 1);
        assert_eq!(explore_groups(&sample(), &[], "TWO").len(), 1, "the marketplace name counts");
        assert!(explore_groups(&sample(), &[], "zzz").is_empty());
        assert_eq!(explore_groups(&sample(), &[], "  ").len(), 2, "blank means everything");
    }

    #[test]
    fn a_plugin_without_a_marketplace_gets_a_readable_group_title() {
        assert_eq!(group_title(""), "(no marketplace)");
        assert_eq!(group_title("one"), "one");
    }

    #[test]
    fn the_first_job_starts_at_once_and_later_ones_wait() {
        let mut queue = JobQueue::default();
        assert_eq!(queue.push(Job::Install("a@m".into())), Some(Job::Install("a@m".into())));
        assert_eq!(queue.push(Job::Install("b@m".into())), None);
        assert!(queue.waits_for("b@m") && !queue.waits_for("a@m"));
        assert_eq!(queue.running(), Some(&Job::Install("a@m".into())));
    }

    #[test]
    fn finishing_a_job_hands_out_the_next_one_in_order_whether_or_not_it_failed() {
        let mut queue = JobQueue::default();
        queue.push(Job::Install("a@m".into()));
        queue.push(Job::Install("b@m".into()));
        queue.push(Job::Uninstall("c@m".into()));
        assert_eq!(queue.finish(), Some(Job::Install("b@m".into())));
        assert_eq!(queue.finish(), Some(Job::Uninstall("c@m".into())));
        assert_eq!(queue.finish(), None);
        assert_eq!(queue.running(), None);
        // Idle again: the next push starts at once.
        assert!(queue.push(Job::Install("d@m".into())).is_some());
    }

    #[test]
    fn a_job_knows_which_plugin_it_is_about() {
        assert_eq!(Job::Install("a@m".into()).plugin_id(), Some("a@m"));
        assert_eq!(Job::SetEnabled("a@m".into(), true).plugin_id(), Some("a@m"));
        assert_eq!(Job::AddMarketplace("o/r".into()).plugin_id(), None);
    }

    #[test]
    fn a_source_is_trimmed_and_must_be_one_non_empty_line() {
        assert_eq!(check_source("  owner/repo \n"), Ok("owner/repo".to_string()));
        assert_eq!(check_source("https://git.example.com/team/plugins.git"), Ok("https://git.example.com/team/plugins.git".to_string()));
        assert!(check_source("   ").is_err());
        assert!(check_source("a\nb").is_err());
        assert!(check_source("--help").is_err(), "an option is not a source");
    }
}
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-features customize::` (add `mod customize;` and the `pub use` lines to `lib.rs` first, listing the names above).
Expected: compile error, then red tests.

- [ ] **Step 3: Implement above the tests**

```rust
use std::collections::VecDeque;

use forge_domain::PluginEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeTab {
    Skills,
    Connectors,
    Plugins,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomizeMode {
    Mine,
    Explore,
}

/// One plugin in a list: the entry and whether Forge has it switched on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRow {
    pub entry: PluginEntry,
    pub enabled: bool,
}

/// The plugins of one marketplace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginGroup {
    pub marketplace: String,
    pub rows: Vec<PluginRow>,
}

fn matches(entry: &PluginEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [&entry.name, &entry.description, &entry.marketplace].iter().any(|text| text.to_lowercase().contains(&query))
}

// `plugins` arrives sorted by marketplace then name, so equal marketplaces are next to each other.
fn groups(plugins: &[PluginEntry], enabled: &[String], query: &str, keep: impl Fn(&PluginEntry) -> bool) -> Vec<PluginGroup> {
    let mut out: Vec<PluginGroup> = Vec::new();
    for entry in plugins.iter().filter(|entry| keep(entry) && matches(entry, query)) {
        let row = PluginRow { enabled: enabled.contains(&entry.id), entry: entry.clone() };
        match out.last_mut() {
            Some(group) if group.marketplace == entry.marketplace => group.rows.push(row),
            _ => out.push(PluginGroup { marketplace: entry.marketplace.clone(), rows: vec![row] }),
        }
    }
    out
}

/// The Mine list: installed plugins only, one group per marketplace.
pub fn mine_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup> {
    groups(plugins, enabled, query, |entry| entry.installed)
}

/// The Explore list: every plugin the marketplaces offer, installed ones included.
pub fn explore_groups(plugins: &[PluginEntry], enabled: &[String], query: &str) -> Vec<PluginGroup> {
    groups(plugins, enabled, query, |_| true)
}

pub fn group_title(marketplace: &str) -> String {
    if marketplace.is_empty() {
        "(no marketplace)".to_string()
    } else {
        marketplace.to_string()
    }
}

/// One thing the page asks the use case to do. Jobs run one at a time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Job {
    Install(String),
    Uninstall(String),
    SetEnabled(String, bool),
    AddMarketplace(String),
    RemoveMarketplace(String),
    UpdateMarketplace(String),
}

impl Job {
    /// The plugin id, for the jobs about one plugin.
    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            Job::Install(id) | Job::Uninstall(id) | Job::SetEnabled(id, _) => Some(id),
            _ => None,
        }
    }
}

/// The jobs waiting and the one running. The caller starts what `push` and `finish` return.
#[derive(Debug, Default)]
pub struct JobQueue {
    running: Option<Job>,
    waiting: VecDeque<Job>,
}

impl JobQueue {
    /// Adds a job. Returns it back when nothing was running, so the caller starts it now.
    pub fn push(&mut self, job: Job) -> Option<Job> {
        if self.running.is_none() {
            self.running = Some(job.clone());
            Some(job)
        } else {
            self.waiting.push_back(job);
            None
        }
    }

    /// The running job is done (or failed). Returns the next job to start, if any.
    pub fn finish(&mut self) -> Option<Job> {
        self.running = self.waiting.pop_front();
        self.running.clone()
    }

    pub fn running(&self) -> Option<&Job> {
        self.running.as_ref()
    }

    /// Whether a job about this plugin is waiting (not yet running).
    pub fn waits_for(&self, id: &str) -> bool {
        self.waiting.iter().any(|job| job.plugin_id() == Some(id))
    }
}

/// The text of the Add marketplace field, trimmed. One non-empty line that is not an option.
pub fn check_source(input: &str) -> Result<String, &'static str> {
    let source = input.trim();
    if source.is_empty() {
        Err("Type a GitHub repository (owner/repo), a Git URL or a folder.")
    } else if source.contains('\n') || source.contains('\r') {
        Err("Use one line.")
    } else if source.starts_with('-') {
        Err("A source cannot start with a dash.")
    } else {
        Ok(source.to_string())
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo test -p forge-features customize::`
Expected: 9 pass.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: pure logic of the Customize page, groups, search, job queue (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Page state, the new main-pane state and the wiring

**Files:**
- Create: `crates/forge-features/src/customize_state.rs`
- Modify: `crates/forge-features/src/workspace.rs`, `crates/forge-features/src/lib.rs`, `crates/forge-pages/src/workspace_page.rs`, `crates/forge-app/Cargo.toml`, `crates/forge-app/src/main.rs`

**Interfaces:**
- Consumes: `PluginCatalogUseCase`, `CatalogView` (Task 4), `Job`, `JobQueue`, `CustomizeTab`, `CustomizeMode`, `check_source` (Task 5), `ClaudePluginCli`, `claude_binary_for` (Task 3).
- Produces (exported from `forge_features`):
  - `enum Loaded { Loading, Ready(CatalogView), Failed(String) }`
  - `enum Dialog { AddMarketplace, ManageMarketplaces, ConfirmUninstall(String), ConfirmRemoveMarketplace(String) }` (`Debug, Clone, PartialEq, Eq`)
  - `enum Busy { No, Waiting, Running }`
  - `struct Customize` (`Copy`) with: `open_page()`, `reload()`, `view() -> Loaded`, `tab()`, `set_tab(CustomizeTab)`, `mode()`, `set_mode(CustomizeMode)`, `query() -> String`, `set_query(String)`, `detail() -> Option<String>`, `open_detail(String)`, `close_detail()`, `dialog() -> Option<Dialog>`, `open_dialog(Dialog)`, `close_dialog()`, `error() -> Option<String>`, `dismiss_error()`, `dialog_error() -> Option<String>`, `run(Job)`, `busy(&str) -> Busy`, `adding_marketplace() -> bool`
  - `use_customize_provider() -> Customize`, `use_customize() -> Customize`
  - `Active::Customize`, `Workspace::show_customize()`, `WorkspaceServices.plugin_catalog: Arc<PluginCatalogUseCase>`

State code is signals and async tasks; it is checked by the real window (Task 9), as the other signal state is. Its logic lives in Task 5.

- [ ] **Step 1: Add the main-pane state**

In `crates/forge-features/src/workspace.rs`:

1. `Active` gets a variant: `Customize,` after `NewSession`.
2. `WorkspaceServices` gets `pub plugin_catalog: Arc<forge_application::PluginCatalogUseCase>,` (add it to the `forge_application` import list instead of the path if you prefer).
3. Add next to `show_new_session`:

```rust
    /// Shows the Customize page in the main pane.
    pub fn show_customize(&self) {
        self.close_lightbox();
        let mut active = self.active;
        active.set(Active::Customize);
    }
```

4. Fix the three matches the compiler will point at:
   - `arranged_sidebar` (about line 250) and `cycle` (about line 968): `Active::NewSession | Active::Customize => None,`
   - `context_project` (about line 820): `Active::NewSession | Active::Customize => self.new_folder()...` (the Customize page reads the same project as the new-session panel).

- [ ] **Step 2: Write `customize_state.rs`**

```rust
use std::sync::Arc;

use dioxus::prelude::*;
use forge_application::{CatalogView, PluginCatalogUseCase};

use crate::customize::{check_source, CustomizeMode, CustomizeTab, Job, JobQueue};
use crate::workspace::WorkspaceServices;

/// What the last read of the catalogue gave.
#[derive(Debug, Clone, PartialEq)]
pub enum Loaded {
    Loading,
    Ready(CatalogView),
    Failed(String),
}

/// A dialog over the page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    AddMarketplace,
    ManageMarketplaces,
    ConfirmUninstall(String),
    ConfirmRemoveMarketplace(String),
}

/// Whether a job about one plugin is running, waiting, or neither.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Busy {
    No,
    Waiting,
    Running,
}

/// The Customize page's state. `Copy` (every field is a signal), like `Workspace`.
#[derive(Clone, Copy)]
pub struct Customize {
    usecase: Signal<Arc<PluginCatalogUseCase>>,
    tab: Signal<CustomizeTab>,
    mode: Signal<CustomizeMode>,
    query: Signal<String>,
    detail: Signal<Option<String>>,
    dialog: Signal<Option<Dialog>>,
    queue: Signal<JobQueue>,
    view: Signal<Loaded>,
    error: Signal<Option<String>>,
    dialog_error: Signal<Option<String>>,
}

/// Creates the page state and provides it to every descendant. Call it in the page at the top of the tree.
pub fn use_customize_provider() -> Customize {
    let services = use_context::<WorkspaceServices>();
    let customize = use_hook(move || Customize {
        usecase: Signal::new(services.plugin_catalog.clone()),
        tab: Signal::new(CustomizeTab::Plugins),
        mode: Signal::new(CustomizeMode::Mine),
        query: Signal::new(String::new()),
        detail: Signal::new(None),
        dialog: Signal::new(None),
        queue: Signal::new(JobQueue::default()),
        view: Signal::new(Loaded::Loading),
        error: Signal::new(None),
        dialog_error: Signal::new(None),
    });
    use_context_provider(move || customize);
    customize
}

pub fn use_customize() -> Customize {
    use_context::<Customize>()
}

impl Customize {
    /// Called when the page appears: back to the list, then read the catalogue.
    pub fn open_page(&self) {
        let mut detail = self.detail;
        detail.set(None);
        let mut dialog = self.dialog;
        dialog.set(None);
        let mut view = self.view;
        view.set(Loaded::Loading);
        self.reload();
    }

    /// Reads the catalogue again. Pinned to the root scope so leaving the page does not cancel it.
    pub fn reload(&self) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let loaded = match usecase.catalog().await {
                Ok(view) => Loaded::Ready(view),
                Err(error) => Loaded::Failed(error.to_string()),
            };
            let mut view = this.view;
            view.set(loaded);
        });
    }

    pub fn view(&self) -> Loaded {
        self.view.read().clone()
    }

    pub fn tab(&self) -> CustomizeTab {
        *self.tab.read()
    }

    pub fn set_tab(&self, tab: CustomizeTab) {
        let mut current = self.tab;
        current.set(tab);
        self.close_detail();
    }

    pub fn mode(&self) -> CustomizeMode {
        *self.mode.read()
    }

    pub fn set_mode(&self, mode: CustomizeMode) {
        let mut current = self.mode;
        current.set(mode);
        self.close_detail();
    }

    pub fn query(&self) -> String {
        self.query.read().clone()
    }

    pub fn set_query(&self, text: String) {
        let mut query = self.query;
        query.set(text);
    }

    pub fn detail(&self) -> Option<String> {
        self.detail.read().clone()
    }

    pub fn open_detail(&self, id: String) {
        let mut detail = self.detail;
        detail.set(Some(id));
    }

    pub fn close_detail(&self) {
        let mut detail = self.detail;
        detail.set(None);
    }

    pub fn dialog(&self) -> Option<Dialog> {
        self.dialog.read().clone()
    }

    pub fn open_dialog(&self, dialog: Dialog) {
        let mut error = self.dialog_error;
        error.set(None);
        let mut current = self.dialog;
        current.set(Some(dialog));
    }

    pub fn close_dialog(&self) {
        let mut error = self.dialog_error;
        error.set(None);
        let mut current = self.dialog;
        current.set(None);
    }

    pub fn error(&self) -> Option<String> {
        self.error.read().clone()
    }

    pub fn dismiss_error(&self) {
        let mut error = self.error;
        error.set(None);
    }

    pub fn dialog_error(&self) -> Option<String> {
        self.dialog_error.read().clone()
    }

    /// Asks for a job. It starts at once when nothing is running, otherwise it waits its turn.
    pub fn run(&self, job: Job) {
        let mut queue = self.queue;
        let start = queue.write().push(job);
        if let Some(job) = start {
            self.start(job);
        }
    }

    /// Whether a job about this plugin is running or waiting (drives the spinner).
    pub fn busy(&self, id: &str) -> Busy {
        let queue = self.queue.read();
        if queue.running().and_then(Job::plugin_id) == Some(id) {
            Busy::Running
        } else if queue.waits_for(id) {
            Busy::Waiting
        } else {
            Busy::No
        }
    }

    /// Whether an Add marketplace job is running (the dialog shows a spinner).
    pub fn adding_marketplace(&self) -> bool {
        matches!(self.queue.read().running(), Some(Job::AddMarketplace(_)))
    }

    /// The Add marketplace button: checks the text, then queues the job.
    pub fn add_marketplace(&self, text: &str) {
        match check_source(text) {
            Ok(source) => {
                let mut error = self.dialog_error;
                error.set(None);
                self.run(Job::AddMarketplace(source));
            }
            Err(message) => {
                let mut error = self.dialog_error;
                error.set(Some(message.to_string()));
            }
        }
    }

    fn start(&self, job: Job) {
        let usecase = (*self.usecase.peek()).clone();
        let this = *self;
        // Pinned to the root scope: a plain `spawn` is cancelled when the page unmounts, which
        // could drop an install half-way.
        dioxus::core::spawn_forever(async move {
            let result = match &job {
                Job::Install(id) => usecase.install(id).await,
                Job::Uninstall(id) => usecase.uninstall(id).await,
                Job::SetEnabled(id, on) => usecase.set_enabled(id, *on).await,
                Job::AddMarketplace(source) => usecase.add_marketplace(source).await,
                Job::RemoveMarketplace(name) => usecase.remove_marketplace(name).await,
                Job::UpdateMarketplace(name) => usecase.update_marketplace(name).await,
            }
            .map_err(|error| error.to_string());
            this.finished(job, result);
        });
    }

    fn finished(&self, job: Job, result: Result<(), String>) {
        match (&job, result) {
            (Job::AddMarketplace(_), Ok(())) => self.close_dialog(),
            (Job::AddMarketplace(_), Err(message)) => {
                let mut error = self.dialog_error;
                error.set(Some(message));
            }
            (_, Err(message)) => {
                let mut error = self.error;
                error.set(Some(message));
            }
            (_, Ok(())) => {}
        }
        // An uninstalled plugin has no detail to show, and a removed marketplace takes its plugins.
        if matches!(job, Job::Uninstall(_) | Job::RemoveMarketplace(_)) {
            self.close_detail();
        }
        let mut queue = self.queue;
        let next = queue.write().finish();
        // The list is read again, never patched by hand.
        self.reload();
        if let Some(next) = next {
            self.start(next);
        }
    }
}
```

In `lib.rs` add `mod customize_state;` and `pub use customize_state::{use_customize, use_customize_provider, Busy, Customize, Dialog, Loaded};` (the `customize` re-exports were added in Task 5).

- [ ] **Step 3: Wire the page**

In `crates/forge-pages/src/workspace_page.rs`: add `use_customize_provider` to the `forge_features` import and call `use_customize_provider();` right after `let settings = use_settings_provider();`. In the two `match workspace.active()` blocks:

```rust
        Active::Live(id) => { /* unchanged */ }
        Active::NewSession | Active::Customize => 0,
```
and
```rust
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Customize => rsx! { CustomizePage {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
```
and add `CustomizePage` to the `forge_widgets` import. (The widget is written in Task 7; to keep this task compiling, create `crates/forge-widgets/src/customize_page.rs` now with `#[component] pub fn CustomizePage() -> Element { rsx! { div { "Customize" } } }`, `mod customize_page;` and `pub use customize_page::CustomizePage;` in the widgets `lib.rs`.)

In `crates/forge-app/Cargo.toml` add `forge-adapter-claude-plugins = { path = "../forge-adapter-claude-plugins" }`. In `main.rs`, add `use forge_adapter_claude_plugins::{claude_binary_for, ClaudePluginCli};`, `use forge_application::PluginCatalogUseCase;` (extend the existing import), and in `build_services()` after `extensions`:

```rust
    // The Claude program that ships with the SDK sits beside the sidecar script. `CLAUDE_CONFIG_DIR` is
    // set for it only when `FORGE_CLAUDE_HOME` moves the Claude home (the harness); with the real home
    // the CLI would keep `.claude.json` inside the folder instead of beside it.
    let plugin_binary = claude_binary_for(std::path::Path::new(&script)).unwrap_or_else(|| "claude-is-not-available".into());
    let plugin_config_dir = std::env::var_os("FORGE_CLAUDE_HOME").filter(|home| !home.is_empty()).map(std::path::PathBuf::from);
    let plugin_catalog = Arc::new(PluginCatalogUseCase::new(
        Arc::new(ClaudePluginCli::new(plugin_binary, claude_home.clone(), plugin_config_dir)),
        config.clone(),
    ));
```
and put `plugin_catalog,` in the `WorkspaceServices { ... }` literal. `runtime::locate_here` returns `Runtime { node, script }`: the `script` is a `String` that is moved into the factory call above, so clone it first (`let runtime::Runtime { node, script } = ...;` then `claude_binary_for(Path::new(&script))` before `SidecarProcessFactory::new(node, script, ...)`, or pass `script.clone()`).

- [ ] **Step 4: Build and run the tests**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: builds; every existing test and the new ones pass. (A compile error about a non-exhaustive match on `Active` means a site was missed: treat `Customize` like `NewSession` there.)

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "feat: Customize page state, Active::Customize and the plugin catalogue wiring (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: The page, the sidebar entry, Mine and the detail

**Files:**
- Modify: `crates/forge-widgets/src/customize_page.rs` (replace the stub), `crates/forge-widgets/src/sidebar.rs`, `crates/forge-widgets/src/lib.rs`, `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`
- Create: `crates/forge-widgets/src/customize_plugins.rs`

**Interfaces:**
- Consumes: `use_customize`, `Customize`, `Loaded`, `Busy`, `Dialog`, `CustomizeTab`, `CustomizeMode`, `Job`, `mine_groups`, `explore_groups`, `group_title`, `PluginRow` (Tasks 5 and 6), `use_settings`, `SettingsSection`, `Active`, `use_workspace`.
- Produces: `CustomizePage`, `PluginsTab` (and `ExploreGrid`, `CustomizeDialogs`, which Task 8 fills in; this task stubs `CustomizeDialogs` as an empty component).

Widgets are checked in the real window (Task 9), as the other widgets are: this repository has no render tests for them (OTH-06).

- [ ] **Step 1: The sidebar entry**

In `sidebar.rs`, after the `sidebar-new` div and before `sidebar-scroll`:

```rust
            button {
                class: if active == Active::Customize { "sidebar-link active" } else { "sidebar-link" },
                onclick: move |_| workspace.show_customize(),
                span { "✦" }
                span { "Customize" }
            }
```

- [ ] **Step 2: The page frame**

Replace `customize_page.rs`:

```rust
use dioxus::prelude::*;
use forge_features::{use_customize, use_settings, CustomizeMode, CustomizeTab, Dialog, SettingsSection};
use forge_shared::{Button, ButtonKind};

use crate::customize_dialogs::CustomizeDialogs;
use crate::customize_plugins::PluginsTab;

const TABS: [(CustomizeTab, &str); 3] =
    [(CustomizeTab::Skills, "Skills"), (CustomizeTab::Connectors, "Connectors"), (CustomizeTab::Plugins, "Plugins")];

/// The Customize page (spec 2026-10-08): tabs, the Mine / Explore switch, search and the Add menu.
/// Only Plugins is built; Skills and Connectors point at Settings until their own plans.
#[component]
pub fn CustomizePage() -> Element {
    let customize = use_customize();
    use_hook(move || customize.open_page());
    let tab = customize.tab();
    let mode = customize.mode();
    let mut add_menu = use_signal(|| false);

    rsx! {
        div { class: "customize",
            h1 { class: "customize-title", "Customize" }
            div { class: "customize-bar",
                div { class: "customize-tabs",
                    for (value, label) in TABS {
                        button {
                            key: "{label}",
                            class: if tab == value { "customize-tab active" } else { "customize-tab" },
                            onclick: move |_| customize.set_tab(value),
                            "{label}"
                        }
                    }
                }
                if tab == CustomizeTab::Plugins {
                    div { class: "customize-switch",
                        button {
                            class: if mode == CustomizeMode::Mine { "active" } else { "" },
                            onclick: move |_| customize.set_mode(CustomizeMode::Mine),
                            "Mine"
                        }
                        button {
                            class: if mode == CustomizeMode::Explore { "active" } else { "" },
                            onclick: move |_| customize.set_mode(CustomizeMode::Explore),
                            "Explore"
                        }
                    }
                    input {
                        class: "customize-search",
                        r#type: "text",
                        placeholder: "Search",
                        value: "{customize.query()}",
                        oninput: move |event| customize.set_query(event.value()),
                    }
                    div { class: "customize-add",
                        Button { label: "+ Add", kind: ButtonKind::Primary, onclick: move |_| add_menu.set(!add_menu()) }
                        if add_menu() {
                            div { class: "menu-backdrop", onclick: move |_| add_menu.set(false) }
                            div { class: "customize-menu",
                                button {
                                    onclick: move |_| {
                                        add_menu.set(false);
                                        customize.open_dialog(Dialog::AddMarketplace);
                                    },
                                    "Add marketplace"
                                }
                                button {
                                    onclick: move |_| {
                                        add_menu.set(false);
                                        customize.open_dialog(Dialog::ManageMarketplaces);
                                    },
                                    "Manage marketplaces"
                                }
                            }
                        }
                    }
                }
            }
            if let Some(message) = customize.error() {
                div { class: "banner error",
                    "{message} "
                    Button { label: "Dismiss", onclick: move |_| customize.dismiss_error() }
                }
            }
            match tab {
                CustomizeTab::Plugins => rsx! { PluginsTab {} },
                CustomizeTab::Skills => rsx! { SettingsNote { what: "Skills", section: SettingsSection::Skills } },
                CustomizeTab::Connectors => rsx! { SettingsNote { what: "Connectors", section: SettingsSection::McpServers } },
            }
            CustomizeDialogs {}
        }
    }
}

// Until the Skills and Connectors tabs are built, their switches are in Settings.
#[component]
fn SettingsNote(what: String, section: SettingsSection) -> Element {
    let settings = use_settings();
    rsx! {
        div { class: "customize-note",
            p { "{what} are switched on and off in Settings for now." }
            Button {
                label: "Open in Settings",
                onclick: move |_| {
                    settings.open();
                    settings.set_section(section);
                },
            }
        }
    }
}
```

- [ ] **Step 3: Mine, Explore (list only for now) and the detail**

Create `customize_plugins.rs`. `PluginsTab` chooses between loading, error, detail, Mine and Explore:

```rust
use dioxus::prelude::*;
use forge_domain::PluginEntry;
use forge_features::{
    explore_groups, group_title, mine_groups, use_customize, Busy, CustomizeMode, Dialog, Job, Loaded, PluginGroup,
};
use forge_shared::{Button, ButtonKind};

#[component]
pub fn PluginsTab() -> Element {
    let customize = use_customize();
    let view = match customize.view() {
        Loaded::Loading => return rsx! { div { class: "hint", "Loading…" } },
        Loaded::Failed(message) => {
            return rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            }
        }
        Loaded::Ready(view) => view,
    };
    if let Some(id) = customize.detail() {
        return match view.snapshot.plugins.iter().find(|plugin| plugin.id == id) {
            Some(plugin) => rsx! { PluginDetail { plugin: plugin.clone() } },
            // The plugin is gone (uninstalled elsewhere): back to the list.
            None => rsx! { div { class: "hint", "That plugin is no longer there." Button { label: "Back", onclick: move |_| customize.close_detail() } } },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = mine_groups(&view.snapshot.plugins, &view.enabled, &query);
            if groups.is_empty() {
                return rsx! { div { class: "hint", "No plugins installed. Use Explore to find some." } };
            }
            rsx! { MineList { groups } }
        }
        CustomizeMode::Explore => {
            let groups = explore_groups(&view.snapshot.plugins, &view.enabled, &query);
            if view.snapshot.marketplaces.is_empty() {
                return rsx! {
                    div { class: "customize-note",
                        p { "Add a marketplace to find plugins." }
                        Button { label: "Add marketplace", kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            if groups.is_empty() {
                return rsx! { div { class: "hint", "Nothing matches." } };
            }
            rsx! { ExploreGrid { groups } }
        }
    }
}

#[component]
fn MineList(groups: Vec<PluginGroup>) -> Element {
    let customize = use_customize();
    let mut menu: Signal<Option<String>> = use_signal(|| None);
    rsx! {
        for group in groups {
            div { key: "{group.marketplace}", class: "customize-group",
                h2 { "{group_title(&group.marketplace)} " span { class: "count", "{group.rows.len()}" } }
                for row in group.rows {
                    {
                        let id = row.entry.id.clone();
                        let busy = customize.busy(&id);
                        rsx! {
                            div {
                                key: "{id}",
                                class: "customize-row",
                                onclick: {
                                    let id = id.clone();
                                    move |_| customize.open_detail(id.clone())
                                },
                                div { class: "customize-icon", "⌬" }
                                div { class: "customize-main",
                                    div { class: "customize-name",
                                        "{row.entry.name}"
                                        span { class: "tag", "{row.entry.marketplace}" }
                                        if !row.enabled { span { class: "tag off", "Off" } }
                                    }
                                    div { class: "customize-desc", "{row.entry.description}" }
                                }
                                if let Some(version) = row.entry.version.clone() { div { class: "customize-meta", "v{version}" } }
                                if busy != Busy::No { span { class: "spinner", title: "Working…" } }
                                button {
                                    class: "icon-button",
                                    title: "More",
                                    onclick: {
                                        let id = id.clone();
                                        move |event| {
                                            event.stop_propagation();
                                            menu.set(if menu() == Some(id.clone()) { None } else { Some(id.clone()) });
                                        }
                                    },
                                    "⋮"
                                }
                                if menu() == Some(id.clone()) {
                                    div { class: "menu-backdrop", onclick: move |event| { event.stop_propagation(); menu.set(None) } }
                                    div { class: "customize-menu row-menu",
                                        button {
                                            onclick: {
                                                let id = id.clone();
                                                let on = !row.enabled;
                                                move |event| {
                                                    event.stop_propagation();
                                                    menu.set(None);
                                                    customize.run(Job::SetEnabled(id.clone(), on));
                                                }
                                            },
                                            if row.enabled { "Turn off" } else { "Turn on" }
                                        }
                                        button {
                                            class: "danger",
                                            onclick: {
                                                let id = id.clone();
                                                move |event| {
                                                    event.stop_propagation();
                                                    menu.set(None);
                                                    customize.open_dialog(Dialog::ConfirmUninstall(id.clone()));
                                                }
                                            },
                                            "Uninstall"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The Explore list is written in the next task; this keeps the page compiling meanwhile.
#[component]
fn ExploreGrid(groups: Vec<PluginGroup>) -> Element {
    rsx! { div { class: "hint", "{groups.len()} marketplaces" } }
}

#[component]
fn PluginDetail(plugin: PluginEntry) -> Element {
    let customize = use_customize();
    let busy = customize.busy(&plugin.id);
    let id = plugin.id.clone();
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), "← Plugins" }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "⌬" }
                div {
                    h2 { "{plugin.name}" }
                    div { class: "hint",
                        "{group_title(&plugin.marketplace)}"
                        if let Some(version) = plugin.version.clone() { " · v{version}" }
                    }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: "Working…" }
                    } else if plugin.installed {
                        Button {
                            label: "Uninstall",
                            kind: ButtonKind::Danger,
                            onclick: {
                                let id = id.clone();
                                move |_| customize.open_dialog(Dialog::ConfirmUninstall(id.clone()))
                            },
                        }
                    } else {
                        Button {
                            label: "Install",
                            kind: ButtonKind::Primary,
                            onclick: {
                                let id = id.clone();
                                move |_| customize.run(Job::Install(id.clone()))
                            },
                        }
                    }
                }
            }
            if !plugin.description.is_empty() {
                div { class: "customize-section", div { class: "label", "Description" } p { "{plugin.description}" } }
            }
            if let Some(path) = plugin.install_path.clone() {
                div { class: "customize-section", div { class: "label", "Installed at" } p { class: "mono", "{path.display()}" } }
            }
        }
    }
}
```

Add `mod customize_dialogs; mod customize_plugins;` to `forge-widgets/src/lib.rs`, and create `customize_dialogs.rs` with the stub `#[component] pub fn CustomizeDialogs() -> Element { rsx! {} }` (filled in Task 8).

- [ ] **Step 4: Styles**

Append to `crates/forge-shared/src/theme.css` (colour tokens `--fg`, `--fg-muted`, `--border`, `--bg-card`, `--bg-hover`, `--accent`, `--danger`, `--radius` already exist):

```css
.sidebar-link { display: flex; align-items: center; gap: 8px; margin: 0 4px 8px; padding: 6px 8px; border: none; border-radius: 8px; background: transparent; color: var(--fg); font: inherit; cursor: pointer; text-align: left; }
.sidebar-link:hover, .sidebar-link.active { background: var(--bg-hover); }
.customize { flex: 1; min-width: 0; overflow-y: auto; padding: 24px 40px 40px; box-sizing: border-box; }
.customize-title { margin: 0 0 16px; font-size: 24px; font-weight: 600; }
.customize-bar { display: flex; align-items: center; gap: 12px; flex-wrap: wrap; margin-bottom: 16px; }
.customize-tabs { display: flex; gap: 4px; margin-right: auto; }
.customize-tab { padding: 6px 12px; border: none; border-radius: 8px; background: transparent; color: var(--fg-muted); font: inherit; cursor: pointer; }
.customize-tab:hover { background: var(--bg-hover); }
.customize-tab.active { background: var(--bg-hover); color: var(--fg); font-weight: 600; }
.customize-switch { display: inline-flex; padding: 2px; border: 1px solid var(--border); border-radius: 10px; }
.customize-switch button { padding: 4px 12px; border: none; border-radius: 8px; background: transparent; color: var(--fg-muted); font: inherit; cursor: pointer; }
.customize-switch button.active { background: var(--bg-card); color: var(--fg); box-shadow: var(--shadow); }
.customize-search { width: 180px; padding: 5px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-card); color: var(--fg); font: inherit; }
.customize-add { position: relative; }
.customize-menu { position: absolute; right: 0; top: calc(100% + 4px); z-index: 6; min-width: 190px; padding: 4px; border: 1px solid var(--border); border-radius: 10px; background: var(--bg-card); box-shadow: var(--shadow); display: flex; flex-direction: column; }
.customize-menu button { padding: 7px 10px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; text-align: left; cursor: pointer; }
.customize-menu button:hover { background: var(--bg-hover); }
.customize-menu button.danger { color: var(--danger); }
.customize-menu.row-menu { top: 36px; right: 8px; }
.customize-note { padding: 24px 0; color: var(--fg-muted); display: flex; flex-direction: column; align-items: flex-start; gap: 12px; }
.customize-group { margin-bottom: 24px; }
.customize-group h2 { margin: 0 0 8px; font-size: 15px; font-weight: 600; }
.customize-group .count { margin-left: 6px; padding: 1px 8px; border-radius: 999px; background: var(--bg-hover); color: var(--fg-muted); font-size: 12px; font-weight: 400; }
.customize-row { position: relative; display: flex; align-items: center; gap: 12px; padding: 10px 8px; border-bottom: 1px solid var(--border); cursor: pointer; }
.customize-row:hover { background: var(--bg-hover); }
.customize-icon { flex: none; width: 36px; height: 36px; display: flex; align-items: center; justify-content: center; border: 1px solid var(--border); border-radius: 10px; background: var(--bg-card); color: var(--fg-muted); }
.customize-icon.big { width: 48px; height: 48px; font-size: 22px; }
.customize-main { flex: 1; min-width: 0; }
.customize-name { font-weight: 600; display: flex; align-items: center; gap: 8px; }
.customize-desc { color: var(--fg-muted); font-size: 13px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.customize-meta { flex: none; color: var(--fg-muted); font-size: 12px; }
.tag { padding: 1px 8px; border-radius: 6px; background: var(--bg-hover); color: var(--fg-muted); font-size: 12px; font-weight: 400; }
.tag.off { border: 1px solid var(--border); background: transparent; }
.spinner { flex: none; width: 14px; height: 14px; border: 2px solid var(--border); border-top-color: var(--fg); border-radius: 50%; animation: customize-spin 0.8s linear infinite; }
@keyframes customize-spin { to { transform: rotate(360deg); } }
@media (prefers-reduced-motion: reduce) { .spinner { animation: none; } }
.customize-detail { max-width: 760px; }
.customize-back { margin-bottom: 12px; padding: 4px 8px; border: none; border-radius: 6px; background: transparent; color: var(--fg-muted); font: inherit; cursor: pointer; }
.customize-back:hover { background: var(--bg-hover); color: var(--fg); }
.customize-detail-head { display: flex; align-items: center; gap: 14px; margin-bottom: 20px; }
.customize-detail-head h2 { margin: 0; font-size: 20px; }
.customize-detail-action { margin-left: auto; }
.customize-section { margin-bottom: 16px; }
.customize-section .label { color: var(--fg-muted); font-size: 12px; margin-bottom: 4px; }
.customize-section p { margin: 0; white-space: pre-wrap; }
.customize-section .mono { font-family: ui-monospace, Menlo, Consolas, monospace; font-size: 12px; word-break: break-all; }
```

In `theme.rs` tests add a rule test in the style of the others:

```rust
    #[test]
    fn the_customize_menu_sits_above_the_menu_backdrop_that_closes_it() {
        assert!(z_index(".customize-menu") > z_index(".menu-backdrop"));
    }
```

(`z_index` finds the one line that starts with the selector followed by ` {`. `.menu-backdrop` is `z-index: 5` and `.customize-menu` is `6`.)

- [ ] **Step 5: Build, test, commit**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: builds and passes.

```bash
git add -A
git commit -m "feat: Customize page with the Plugins tab, Mine list and plugin detail (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Explore and the dialogs

**Files:**
- Modify: `crates/forge-widgets/src/customize_plugins.rs` (replace the `ExploreGrid` stub), `crates/forge-widgets/src/customize_dialogs.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 6's `Customize` API, `Dialog`, `Job`, `Loaded`; `PluginGroup`.
- Produces: the finished `ExploreGrid` and `CustomizeDialogs`.

- [ ] **Step 1: The Explore grid**

Replace the `ExploreGrid` stub in `customize_plugins.rs`:

```rust
#[component]
fn ExploreGrid(groups: Vec<PluginGroup>) -> Element {
    let customize = use_customize();
    rsx! {
        for group in groups {
            div { key: "{group.marketplace}", class: "customize-group",
                h2 { "{group_title(&group.marketplace)} " span { class: "count", "{group.rows.len()}" } }
                div { class: "customize-grid",
                    for row in group.rows {
                        {
                            let id = row.entry.id.clone();
                            let busy = customize.busy(&id);
                            let installed = row.entry.installed;
                            rsx! {
                                div {
                                    key: "{id}",
                                    class: "customize-card",
                                    onclick: {
                                        let id = id.clone();
                                        move |_| customize.open_detail(id.clone())
                                    },
                                    div { class: "customize-icon", "⌬" }
                                    div { class: "customize-main",
                                        div { class: "customize-name", "{row.entry.name}" }
                                        div { class: "customize-card-desc", "{row.entry.description}" }
                                        div { class: "customize-source", "{group_title(&row.entry.marketplace)}" }
                                    }
                                    if busy != Busy::No {
                                        span { class: "spinner", title: if busy == Busy::Waiting { "Waiting…" } else { "Installing…" } }
                                    } else if installed {
                                        span { class: "customize-check", title: "Installed", "✓" }
                                    } else {
                                        button {
                                            class: "customize-plus",
                                            title: "Install",
                                            onclick: {
                                                let id = id.clone();
                                                move |event| {
                                                    event.stop_propagation();
                                                    customize.run(Job::Install(id.clone()));
                                                }
                                            },
                                            "+"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: The dialogs**

Replace `customize_dialogs.rs`:

```rust
use dioxus::prelude::*;
use forge_domain::Marketplace;
use forge_features::{use_customize, Dialog, Job, Loaded};
use forge_shared::{Button, ButtonKind};

/// Whichever dialog is open, over the page. A click on the backdrop closes it.
#[component]
pub fn CustomizeDialogs() -> Element {
    let customize = use_customize();
    let Some(dialog) = customize.dialog() else {
        return rsx! {};
    };
    rsx! {
        div { class: "settings-backdrop", onclick: move |_| customize.close_dialog(),
            div { class: "customize-dialog", onclick: move |event| event.stop_propagation(),
                match dialog {
                    Dialog::AddMarketplace => rsx! { AddMarketplace {} },
                    Dialog::ManageMarketplaces => rsx! { ManageMarketplaces {} },
                    Dialog::ConfirmUninstall(id) => rsx! { ConfirmUninstall { id } },
                    Dialog::ConfirmRemoveMarketplace(name) => rsx! { ConfirmRemove { name } },
                }
            }
        }
    }
}

#[component]
fn AddMarketplace() -> Element {
    let customize = use_customize();
    let mut text = use_signal(String::new);
    let adding = customize.adding_marketplace();
    rsx! {
        h2 { "Add marketplace" }
        p { class: "hint", "A GitHub repository (owner/repo), a Git URL or a folder on this computer." }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: "owner/repo, https://git.example.com/team/plugins.git or /path/to/folder",
            value: "{text}",
            disabled: adding,
            oninput: move |event| text.set(event.value()),
            onkeydown: move |event| {
                if event.key() == Key::Enter && !adding {
                    customize.add_marketplace(&text());
                }
            },
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if adding { span { class: "spinner", title: "Adding…" } }
            Button { label: "Cancel", disabled: adding, onclick: move |_| customize.close_dialog() }
            Button { label: "Add", kind: ButtonKind::Primary, disabled: adding, onclick: move |_| customize.add_marketplace(&text()) }
        }
    }
}

#[component]
fn ManageMarketplaces() -> Element {
    let customize = use_customize();
    let mut menu: Signal<Option<String>> = use_signal(|| None);
    let markets: Vec<Marketplace> = match customize.view() {
        Loaded::Ready(view) => view.snapshot.marketplaces,
        _ => Vec::new(),
    };
    rsx! {
        h2 { "Manage marketplaces" }
        p { class: "hint", "These are the marketplaces added with Claude Code on this computer. Removing one also removes its plugins." }
        if markets.is_empty() {
            div { class: "hint", "No marketplaces yet." }
        }
        for market in markets {
            div { key: "{market.name}", class: "customize-market",
                div { class: "customize-main",
                    div { class: "customize-name", "{market.name}" }
                    div { class: "customize-desc", "{market.location}" }
                    if let Some(when) = market.last_updated.clone() { div { class: "customize-source", "Last updated {when}" } }
                }
                button {
                    class: "icon-button",
                    title: "More",
                    onclick: {
                        let name = market.name.clone();
                        move |_| menu.set(if menu() == Some(name.clone()) { None } else { Some(name.clone()) })
                    },
                    "⋮"
                }
                if menu() == Some(market.name.clone()) {
                    div { class: "menu-backdrop", onclick: move |_| menu.set(None) }
                    div { class: "customize-menu row-menu",
                        button {
                            onclick: {
                                let name = market.name.clone();
                                move |_| {
                                    menu.set(None);
                                    customize.run(Job::UpdateMarketplace(name.clone()));
                                }
                            },
                            "Update"
                        }
                        button {
                            class: "danger",
                            onclick: {
                                let name = market.name.clone();
                                move |_| {
                                    menu.set(None);
                                    customize.open_dialog(Dialog::ConfirmRemoveMarketplace(name.clone()));
                                }
                            },
                            "Remove"
                        }
                    }
                }
            }
        }
        div { class: "customize-dialog-buttons",
            Button { label: "Done", kind: ButtonKind::Primary, onclick: move |_| customize.close_dialog() }
        }
    }
}

#[component]
fn ConfirmUninstall(id: String) -> Element {
    let customize = use_customize();
    let name = forge_domain::split_plugin_id(&id).0.to_string();
    rsx! {
        h2 { "Uninstall {name}?" }
        p { "Other Claude Code installs on this computer lose it too." }
        div { class: "customize-dialog-buttons",
            Button { label: "Cancel", onclick: move |_| customize.close_dialog() }
            Button {
                label: "Uninstall",
                kind: ButtonKind::Danger,
                onclick: move |_| {
                    customize.close_dialog();
                    customize.run(Job::Uninstall(id.clone()));
                },
            }
        }
    }
}

#[component]
fn ConfirmRemove(name: String) -> Element {
    let customize = use_customize();
    rsx! {
        h2 { "Remove {name}?" }
        p { "The plugins installed from it are removed too, here and in other Claude Code installs on this computer." }
        div { class: "customize-dialog-buttons",
            Button { label: "Cancel", onclick: move |_| customize.open_dialog(Dialog::ManageMarketplaces) }
            Button {
                label: "Remove",
                kind: ButtonKind::Danger,
                onclick: move |_| {
                    customize.open_dialog(Dialog::ManageMarketplaces);
                    customize.run(Job::RemoveMarketplace(name.clone()));
                },
            }
        }
    }
}
```

Note: `Button`'s `disabled` prop is `#[props(default)] disabled: bool` (see `forge-shared/src/components.rs`).

- [ ] **Step 3: Styles for the grid, the cards and the dialogs**

Append to `theme.css`:

```css
.customize-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.chat-narrow .customize-grid { grid-template-columns: 1fr; }
.customize-card { position: relative; display: flex; align-items: flex-start; gap: 12px; padding: 14px; border: 1px solid var(--border); border-radius: 12px; background: var(--bg-card); cursor: pointer; min-width: 0; }
.customize-card:hover { background: var(--bg-hover); }
.customize-card-desc { color: var(--fg-muted); font-size: 13px; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
.customize-source { margin-top: 4px; color: var(--fg-muted); font-size: 12px; }
.customize-plus { flex: none; width: 28px; height: 28px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-card); color: var(--fg); font: inherit; font-size: 16px; line-height: 1; cursor: pointer; }
.customize-plus:hover { background: var(--bg-hover); }
.customize-check { flex: none; width: 28px; text-align: center; color: var(--fg-muted); }
.customize-dialog { width: min(520px, calc(100% - 32px)); max-height: calc(100% - 64px); overflow-y: auto; padding: 20px 24px; border: 1px solid var(--border); border-radius: 14px; background: var(--bg-card); box-shadow: var(--shadow); box-sizing: border-box; }
.customize-dialog h2 { margin: 0 0 8px; font-size: 18px; }
.customize-field { width: 100%; padding: 8px 10px; margin: 8px 0; border: 1px solid var(--border); border-radius: 8px; background: var(--bg); color: var(--fg); font: inherit; box-sizing: border-box; }
.customize-dialog-buttons { display: flex; align-items: center; justify-content: flex-end; gap: 8px; margin-top: 16px; }
.customize-market { position: relative; display: flex; align-items: center; gap: 12px; padding: 10px 12px; margin-bottom: 8px; border: 1px solid var(--border); border-radius: 10px; }
```

Check that `--bg` exists in `theme.css` (`grep -n "^  --bg:" crates/forge-shared/src/theme.css`); if the page background token has another name, use it.

- [ ] **Step 4: Build, test, commit**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: builds and passes.

```bash
git add -A
git commit -m "feat: Explore grid and the marketplace dialogs of the Customize page (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Check it in the real window

**Files:**
- Create: `scripts/harness/add_market_fixture.py`, `scripts/harness/test_add_market_fixture.py`

**Interfaces:**
- Produces: `add_market_fixture.add(marketplace_dir: Path) -> Path` writing a local marketplace named `harness-market` with two plugins (`hello`, `second`); run through `FORGE_HARNESS_EXTRA`.

- [ ] **Step 1: Write the failing test**

`scripts/harness/test_add_market_fixture.py`:

```python
import json
import os
import tempfile
import unittest
from pathlib import Path

import add_market_fixture as market


class AddMarketFixtureTest(unittest.TestCase):
    def setUp(self):
        self.dir = Path(os.path.realpath(tempfile.mkdtemp())) / "market"
        market.add(self.dir)

    def test_the_marketplace_file_names_two_plugins(self):
        data = json.loads((self.dir / ".claude-plugin" / "marketplace.json").read_text())
        self.assertEqual(data["name"], "harness-market")
        self.assertEqual([p["name"] for p in data["plugins"]], ["hello", "second"])

    def test_each_plugin_has_a_manifest_and_a_skill(self):
        for name in ("hello", "second"):
            plugin = self.dir / "plugins" / name
            self.assertEqual(json.loads((plugin / ".claude-plugin" / "plugin.json").read_text())["name"], name)
            self.assertTrue((plugin / "skills" / "greet" / "SKILL.md").is_file())


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run to see it fail**

Run: `cd scripts/harness && python3 -m unittest test_add_market_fixture`
Expected: ImportError (`add_market_fixture`).

- [ ] **Step 3: Write the script**

`scripts/harness/add_market_fixture.py`:

```python
#!/usr/bin/env python3
"""Writes a local plugin marketplace (two plugins) to /private/tmp/forge-harness-market, so the Customize page can
be tried in the harness: in the app, Add marketplace with that folder, then install `hello`.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_market_fixture.py scripts/harness/launch.sh
"""
import json
from pathlib import Path

DEFAULT_DIR = Path("/private/tmp/forge-harness-market")

PLUGINS = [
    ("hello", "1.0.0", "Says hello. The first plugin of the harness marketplace."),
    ("second", "0.2.0", "A second plugin, still not installed."),
]


def add(directory: Path = DEFAULT_DIR) -> Path:
    (directory / ".claude-plugin").mkdir(parents=True, exist_ok=True)
    (directory / ".claude-plugin" / "marketplace.json").write_text(
        json.dumps(
            {
                "name": "harness-market",
                "owner": {"name": "harness"},
                "plugins": [
                    {"name": name, "source": f"./plugins/{name}", "description": description, "version": version}
                    for name, version, description in PLUGINS
                ],
            },
            indent=2,
        )
    )
    for name, version, description in PLUGINS:
        plugin = directory / "plugins" / name
        (plugin / ".claude-plugin").mkdir(parents=True, exist_ok=True)
        (plugin / ".claude-plugin" / "plugin.json").write_text(
            json.dumps({"name": name, "version": version, "description": description}, indent=2)
        )
        skill = plugin / "skills" / "greet"
        skill.mkdir(parents=True, exist_ok=True)
        (skill / "SKILL.md").write_text(f"---\nname: greet\ndescription: Greets from {name}\n---\nSay hello from {name}.\n")
    return directory


if __name__ == "__main__":
    print(f"marketplace written to {add()}")
```

- [ ] **Step 4: Run the test**

Run: `cd scripts/harness && python3 -m unittest test_add_market_fixture`
Expected: 2 pass. Also run the whole harness set: `python3 -m unittest discover -s scripts/harness` (expected: all pass).

- [ ] **Step 5: Run the app and look**

The worktree needs the sidecar build and its modules (`cd sidecar && npm ci && npm run build`, done in Task 3 for `npm ci`).

```bash
FORGE_HARNESS_EXTRA=scripts/harness/add_market_fixture.py scripts/harness/launch.sh
python3 scripts/harness/capture.py     # then Read /private/tmp/forge-harness-shot.png
python3 scripts/harness/click.py X Y   # pixels of that PNG (multiply by the viewer's scale)
```

Check these, one capture after each click (the harness runs with `FORGE_CLAUDE_HOME` set, so `claude` runs with a fixture `CLAUDE_CONFIG_DIR`; the real `~/.claude` is not touched):

1. The sidebar shows "Customize" under "+ New session"; clicking it shows the page with tabs and "Mine" empty text "No plugins installed. Use Explore to find some."
2. Explore with no marketplace shows "Add a marketplace to find plugins." and the button.
3. `+ Add` → "Add marketplace"; typing needs the keyboard (osascript `keystroke` drops letters under a Korean input method): paste or type `/private/tmp/forge-harness-market`. A wrong path (`/no/such/dir`) shows "Path does not exist: /no/such/dir" under the field; a text starting with `-` shows "A source cannot start with a dash."
4. After a good add the dialog closes; Explore shows the group `harness-market` with two cards.
5. `+` on `hello` shows a spinner on the card, then a check; Mine shows `hello` under `harness-market` with `v1.0.0` and no "Off" tag.
6. The row's `⋮` → "Turn off" shows the "Off" tag; "Turn on" removes it. Open the settings (⌘,) → Plugins: the `hello@harness-market` switch matches.
7. Click the row: the detail shows name, "harness-market · v1.0.0", the description, "Installed at" with a path under `forge-harness-home`, and `Uninstall`. Uninstall → the confirmation text reads "Other Claude Code installs on this computer lose it too." → confirm → back in Mine, the list is empty.
8. `+ Add` → "Manage marketplaces" lists `harness-market` with its folder; `⋮` → Remove → confirmation → the marketplace is gone, Explore shows the empty state again.
9. Shrink the window to about 800 pt wide (`python3 scripts/harness/resize.py 800 700`): the Explore cards become one column; the toolbar wraps without cutting the `+ Add` button.
10. Stop: `scripts/harness/stop.sh`.

If a check fails, fix it in the widget or CSS, rebuild, and look again. IME and dark mode are left to the user.

- [ ] **Step 6: Commit**

```bash
git add scripts/harness
git commit -m "test: harness fixture, a local plugin marketplace, for the Customize page (Plan 30A)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Record what was built

**Files:**
- Modify: `docs/superpowers/specs/2026-10-08-customize-plugins-design.md`, `docs/superpowers/roadmap/backlog.md`, `docs/superpowers/roadmap/README.md`

- [ ] **Step 1: Add an "As built" section to the spec**

Append to the spec: what changed from the design, namely:

- The port has no `update(id)` (plugin update): the page has no button for it.
- The page search is an always visible text field, not an icon.
- `CLAUDE_CONFIG_DIR` is set only when `FORGE_CLAUDE_HOME` is set.
- Fields of `available` entries seen: `pluginId`, `name`, `description`, `marketplaceName`, `source`, `version` (when the marketplace has one); no author. An installed plugin's description is read from its `.claude-plugin/plugin.json`; the marketplace's "last updated" comes from `known_marketplaces.json`.
- What the real-window check (Task 9) found, and anything not checked (Windows; a GitHub or Git URL marketplace; a failing Git clone message).

- [ ] **Step 2: Backlog and README**

In `backlog.md`, PG-02: `Plan` column becomes `Plan 30A (built: page frame and Plugins tab); 30B Skills and 30C Connectors later`. In `README.md`, the 30A row says `(built)`.

- [ ] **Step 3: Run everything and commit**

Run: `cargo test --workspace && python3 -m unittest discover -s scripts/harness`
Expected: all pass.

```bash
git add docs
git commit -m "docs: Plan 30A as built, Customize page and plugin marketplaces

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-review (done while writing)

**Spec coverage.** Back end: port, adapter, errors, serial writes, time limits, flag-like refusal, config dir rule (Tasks 1 to 3); use case with the on/off list (Task 4); screen: entry, frame, tabs and notes, Mine by marketplace, detail, Explore with empty state, Add and Manage dialogs, uninstall and remove confirmations, error bar, one column when narrow (Tasks 7 and 8); state and flow, queue and re-read (Tasks 5 and 6); testing: unit, adapter fixtures, gated integration, real window (Tasks 2 to 5, 9). The spec's "Risks" about the packaged binary is covered by `claude_binary_for` and its layout test (the layout is the one `stage-runtime.mjs` makes); Windows is left for a real machine, as the spec says.

**Changes from the spec, recorded in Task 10:** no plugin `update`; a search field instead of an icon; the manifest read for installed descriptions; no author line.

**Placeholders.** None: every step has its code or command. Widget code is checked by compiling and by the real window (no widget render tests exist in this repository).

**Types.** `Job`, `JobQueue`, `Busy`, `Dialog`, `Loaded`, `CatalogView`, `PluginGroup`, `PluginRow` are used with the names and signatures given in the Interfaces blocks.
