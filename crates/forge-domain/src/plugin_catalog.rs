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
    /// The scope the install is in (`user`, `project`, `local`), when the CLI says. Only an installed plugin has one.
    pub scope: Option<String>,
    pub install_path: Option<PathBuf>,
}

impl PluginEntry {
    /// Whether this page can uninstall it: Forge always runs `plugin uninstall` for the user scope, so a plugin
    /// that is installed only in a project or local scope has to be removed there.
    pub fn can_uninstall(&self) -> bool {
        self.installed && self.scope.as_deref().map_or(true, |scope| scope == "user")
    }
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

    fn entry(installed: bool, scope: Option<&str>) -> PluginEntry {
        PluginEntry {
            id: "a@m".into(),
            name: "a".into(),
            marketplace: "m".into(),
            version: None,
            description: String::new(),
            installed,
            scope: scope.map(String::from),
            install_path: None,
        }
    }

    #[test]
    fn only_a_user_scope_install_can_be_uninstalled_from_here() {
        assert!(entry(true, Some("user")).can_uninstall());
        assert!(entry(true, None).can_uninstall(), "no scope reported: the CLI default is the user scope");
        assert!(!entry(true, Some("project")).can_uninstall());
        assert!(!entry(true, Some("local")).can_uninstall());
        assert!(!entry(false, None).can_uninstall(), "nothing to uninstall");
    }
}
