use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{plugin_id, skill_id, PluginEntry, SkillEntry, SkillFile, SkillOrigin, SkillSnapshot};
use forge_ports::{SkillCatalogError, SkillCatalogPort};
use serde::Deserialize;

use crate::skills::collect_skills;

/// Reads and writes the skills the Customize page shows, below the Claude home.
pub struct ClaudeSkillCatalogAdapter {
    claude_home: PathBuf,
}

impl ClaudeSkillCatalogAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

#[derive(Deserialize)]
struct KnownMarketplace {
    #[serde(default, rename = "installLocation")]
    install_location: Option<PathBuf>,
}

#[derive(Deserialize)]
struct MarketplaceFile {
    #[serde(default)]
    plugins: Vec<OfferedPlugin>,
}

#[derive(Deserialize)]
struct OfferedPlugin {
    name: String,
    #[serde(default)]
    source: serde_json::Value,
}

async fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    serde_json::from_str(&text).ok()
}

/// The folder of a plugin the marketplace keeps inside itself (`"source": "./plugins/x"`); `None` for any other source.
fn local_plugin_dir(marketplace_dir: &Path, source: &serde_json::Value) -> Option<PathBuf> {
    let relative = Path::new(source.as_str()?.strip_prefix("./")?);
    if relative.components().any(|part| !matches!(part, Component::Normal(_))) {
        return None;
    }
    Some(marketplace_dir.join(relative))
}

// Adds the skills found below `root`. `visited` is shared so one folder is never read twice.
async fn add_root(
    root: &Path,
    origin: SkillOrigin,
    installed: bool,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<SkillEntry>,
) -> Result<(), SkillCatalogError> {
    let mut found = Vec::new();
    collect_skills(root, visited, &mut found).await.map_err(|e| SkillCatalogError::Io(e.to_string()))?;
    for skill in found {
        let folder = skill.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| skill.name.clone());
        out.push(SkillEntry {
            id: skill_id(&origin, &folder),
            name: skill.name,
            description: skill.description.unwrap_or_default(),
            origin: origin.clone(),
            installed,
            path: skill.path,
        });
    }
    Ok(())
}

impl ClaudeSkillCatalogAdapter {
    async fn read_snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<SkillSnapshot, SkillCatalogError> {
        let mut visited = HashSet::new();
        let mut skills = Vec::new();
        add_root(&self.claude_home.join("skills"), SkillOrigin::User, true, &mut visited, &mut skills).await?;
        if let Some(project) = project_path {
            add_root(&project.join(".claude").join("skills"), SkillOrigin::Project, true, &mut visited, &mut skills).await?;
        }
        // From here on a folder that cannot be read is skipped: it is somebody else's plugin.
        for plugin in plugins.iter().filter(|plugin| plugin.installed) {
            if let Some(path) = &plugin.install_path {
                let origin = SkillOrigin::Plugin { plugin_id: plugin.id.clone() };
                let _ = add_root(&path.join("skills"), origin, true, &mut visited, &mut skills).await;
            }
        }
        let installed: HashSet<&str> = plugins.iter().filter(|plugin| plugin.installed).map(|plugin| plugin.id.as_str()).collect();
        let known: HashMap<String, KnownMarketplace> =
            read_json(&self.claude_home.join("plugins").join("known_marketplaces.json")).await.unwrap_or_default();
        for (market, info) in known {
            let Some(dir) = info.install_location else { continue };
            let Some(file) = read_json::<MarketplaceFile>(&dir.join(".claude-plugin").join("marketplace.json")).await else {
                continue;
            };
            for offered in file.plugins {
                let id = plugin_id(&offered.name, &market);
                if installed.contains(id.as_str()) {
                    continue;
                }
                let Some(plugin_dir) = local_plugin_dir(&dir, &offered.source) else { continue };
                let _ = add_root(&plugin_dir.join("skills"), SkillOrigin::Plugin { plugin_id: id }, false, &mut visited, &mut skills).await;
            }
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        Ok(SkillSnapshot { skills })
    }
}

const NOT_BUILT: &str = "not built yet";

#[async_trait]
impl SkillCatalogPort for ClaudeSkillCatalogAdapter {
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<SkillSnapshot, SkillCatalogError> {
        self.read_snapshot(project_path, plugins).await
    }

    async fn add_from_folder(&self, _source: &Path) -> Result<PathBuf, SkillCatalogError> {
        Err(SkillCatalogError::Io(NOT_BUILT.into()))
    }

    async fn add_from_zip(&self, _source: &Path) -> Result<PathBuf, SkillCatalogError> {
        Err(SkillCatalogError::Io(NOT_BUILT.into()))
    }

    async fn create(&self, _name: &str, _description: &str) -> Result<PathBuf, SkillCatalogError> {
        Err(SkillCatalogError::Io(NOT_BUILT.into()))
    }

    async fn list_files(&self, _skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError> {
        Err(SkillCatalogError::Io(NOT_BUILT.into()))
    }

    async fn read_file(&self, _skill_path: &Path, _relative: &str) -> Result<String, SkillCatalogError> {
        Err(SkillCatalogError::Io(NOT_BUILT.into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::PluginEntry;
    use std::path::Path;

    fn write_skill(root: &Path, folder: &str, name: &str, description: &str) {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), format!("---\nname: {name}\ndescription: {description}\n---\n\nBody\n")).unwrap();
    }

    fn entry(id: &str, installed: bool, install_path: Option<&Path>) -> PluginEntry {
        let (name, marketplace) = id.split_once('@').unwrap();
        PluginEntry {
            id: id.to_string(),
            name: name.to_string(),
            marketplace: marketplace.to_string(),
            version: None,
            description: String::new(),
            installed,
            scope: None,
            install_path: install_path.map(Path::to_path_buf),
        }
    }

    // Writes `known_marketplaces.json` for one marketplace folder and returns the folder.
    fn add_marketplace(home: &Path, name: &str, marketplace_json: &str) -> std::path::PathBuf {
        let dir = home.join("market").join(name);
        std::fs::create_dir_all(dir.join(".claude-plugin")).unwrap();
        std::fs::write(dir.join(".claude-plugin").join("marketplace.json"), marketplace_json).unwrap();
        let plugins = home.join("plugins");
        std::fs::create_dir_all(&plugins).unwrap();
        let path = plugins.join("known_marketplaces.json");
        let mut known: serde_json::Map<String, serde_json::Value> =
            std::fs::read_to_string(&path).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
        known.insert(name.to_string(), serde_json::json!({ "installLocation": dir }));
        std::fs::write(&path, serde_json::to_string(&known).unwrap()).unwrap();
        dir
    }

    async fn snapshot(home: &Path, project: Option<&Path>, plugins: Vec<PluginEntry>) -> SkillSnapshot {
        ClaudeSkillCatalogAdapter::new(home.to_path_buf())
            .snapshot(project.map(Path::to_path_buf), plugins)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn skills_of_the_user_folder_and_the_project_folder_are_found_with_their_origin() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "pdf", "pdf", "Reads PDFs");
        write_skill(&project.path().join(".claude").join("skills"), "lint", "lint", "Lints");

        let snap = snapshot(home.path(), Some(project.path()), vec![]).await;

        let found: Vec<_> = snap.skills.iter().map(|s| (s.id.as_str(), s.origin.clone(), s.installed)).collect();
        assert_eq!(found, vec![("project:lint", SkillOrigin::Project, true), ("user:pdf", SkillOrigin::User, true)]);
        assert_eq!(snap.skills[1].description, "Reads PDFs");
        assert_eq!(snap.skills[1].path, home.path().join("skills").join("pdf"));
    }

    #[tokio::test]
    async fn without_a_project_only_the_user_folder_is_read() {
        let home = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "pdf", "pdf", "d");
        let snap = snapshot(home.path(), None, vec![]).await;
        assert_eq!(snap.skills.len(), 1);
    }

    #[tokio::test]
    async fn a_project_that_is_the_home_folder_does_not_list_the_user_skills_twice() {
        let home = tempfile::tempdir().unwrap();
        let claude = home.path().join(".claude");
        write_skill(&claude.join("skills"), "pdf", "pdf", "d");
        let snap = snapshot(&claude, Some(home.path()), vec![]).await;
        assert_eq!(snap.skills.len(), 1);
    }

    #[tokio::test]
    async fn skills_of_an_installed_plugin_are_found_and_marked_installed() {
        let home = tempfile::tempdir().unwrap();
        let install = home.path().join("cache").join("hello");
        write_skill(&install.join("skills"), "greet", "greet", "Greets");

        let snap = snapshot(home.path(), None, vec![entry("hello@m", true, Some(&install))]).await;

        assert_eq!(snap.skills.len(), 1);
        assert_eq!(snap.skills[0].id, "plugin:hello@m:greet");
        assert_eq!(snap.skills[0].origin, SkillOrigin::Plugin { plugin_id: "hello@m".to_string() });
        assert!(snap.skills[0].installed);
    }

    #[tokio::test]
    async fn a_skill_of_a_plugin_a_marketplace_offers_is_found_and_marked_not_installed() {
        let home = tempfile::tempdir().unwrap();
        let market = add_marketplace(
            home.path(),
            "m",
            r#"{"name":"m","plugins":[{"name":"hello","source":"./plugins/hello"}]}"#,
        );
        write_skill(&market.join("plugins").join("hello").join("skills"), "greet", "greet", "Greets");

        let snap = snapshot(home.path(), None, vec![entry("hello@m", false, None)]).await;

        assert_eq!(snap.skills.len(), 1);
        assert_eq!(snap.skills[0].id, "plugin:hello@m:greet");
        assert!(!snap.skills[0].installed);
    }

    #[tokio::test]
    async fn an_installed_plugin_is_not_listed_a_second_time_from_its_marketplace() {
        let home = tempfile::tempdir().unwrap();
        let market = add_marketplace(home.path(), "m", r#"{"plugins":[{"name":"hello","source":"./plugins/hello"}]}"#);
        write_skill(&market.join("plugins").join("hello").join("skills"), "greet", "greet", "from the marketplace");
        let install = home.path().join("cache").join("hello");
        write_skill(&install.join("skills"), "greet", "greet", "from the install");

        let snap = snapshot(home.path(), None, vec![entry("hello@m", true, Some(&install))]).await;

        assert_eq!(snap.skills.len(), 1);
        assert_eq!(snap.skills[0].description, "from the install");
    }

    #[tokio::test]
    async fn a_user_skill_and_a_plugin_skill_of_one_name_stay_two_entries() {
        let home = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "greet", "greet", "mine");
        let install = home.path().join("cache").join("hello");
        write_skill(&install.join("skills"), "greet", "greet", "theirs");

        let snap = snapshot(home.path(), None, vec![entry("hello@m", true, Some(&install))]).await;

        let ids: Vec<_> = snap.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["plugin:hello@m:greet", "user:greet"]);
    }

    #[tokio::test]
    async fn a_broken_marketplace_or_an_unreadable_source_is_skipped_and_the_rest_still_show() {
        let home = tempfile::tempdir().unwrap();
        // Not JSON at all.
        add_marketplace(home.path(), "broken", "this is not json");
        // Sources that are not a plain relative path inside the marketplace.
        let odd = add_marketplace(
            home.path(),
            "odd",
            r#"{"plugins":[
                {"name":"remote","source":{"source":"github","repo":"a/b"}},
                {"name":"escape","source":"./../outside"},
                {"name":"absolute","source":"/etc"},
                {"name":"fine","source":"./plugins/fine"}
            ]}"#,
        );
        write_skill(&odd.join("plugins").join("fine").join("skills"), "ok", "ok", "d");
        write_skill(&odd.join("..").join("outside").join("skills"), "bad", "bad", "d");

        let snap = snapshot(home.path(), None, vec![]).await;

        let ids: Vec<_> = snap.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["plugin:fine@odd:ok"]);
    }

    #[tokio::test]
    async fn a_skill_with_broken_front_matter_is_skipped() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("broken");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "no front matter here").unwrap();
        write_skill(&home.path().join("skills"), "good", "good", "d");
        let snap = snapshot(home.path(), None, vec![]).await;
        assert_eq!(snap.skills.len(), 1);
    }
}
