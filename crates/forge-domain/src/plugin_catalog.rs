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
