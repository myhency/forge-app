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
