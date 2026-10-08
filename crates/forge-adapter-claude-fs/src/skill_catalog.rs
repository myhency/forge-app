use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{plugin_id, skill_id, PluginEntry, SkillEntry, SkillFile, SkillOrigin, SkillSnapshot};
use forge_ports::{SkillCatalogError, SkillCatalogPort};
use serde::Deserialize;

use crate::skills::{collect_skills, parse_frontmatter};

/// A folder copy or a zip may hold this many entries ...
pub const ENTRY_LIMIT: usize = 500;
/// ... and this many bytes of file content.
pub const SIZE_LIMIT: u64 = 20 * 1024 * 1024;

fn io_error(error: std::io::Error) -> SkillCatalogError {
    SkillCatalogError::Io(error.to_string())
}

fn invalid(text: &str) -> SkillCatalogError {
    SkillCatalogError::Invalid(text.to_string())
}

fn too_big() -> SkillCatalogError {
    invalid("That skill is too big to add (at most 500 files and 20 MB).")
}

// Names Windows keeps for devices, with or without an extension (`con`, `NUL.txt`, `com1`).
fn is_windows_device_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix).is_some_and(|rest| rest.len() == 1 && rest.chars().all(|c| ('1'..='9').contains(&c)))
        })
}

fn check_name(name: &str) -> Result<(), SkillCatalogError> {
    let allowed = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !is_windows_device_name(name)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if allowed {
        Ok(())
    } else {
        Err(invalid("A skill name can use letters, numbers, - and _ (at most 64 characters, not ending in a dot)."))
    }
}

struct Budget {
    entries: usize,
    bytes: u64,
}

// Copies the files and folders of `from` into `to` (which exists). Symlinks are skipped.
fn copy_dir(from: &Path, to: &Path, budget: &mut Budget) -> Result<(), SkillCatalogError> {
    for entry in std::fs::read_dir(from).map_err(io_error)? {
        let entry = entry.map_err(io_error)?;
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_symlink() {
            continue;
        }
        budget.entries += 1;
        if budget.entries > ENTRY_LIMIT {
            return Err(too_big());
        }
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            std::fs::create_dir(&target).map_err(io_error)?;
            copy_dir(&entry.path(), &target, budget)?;
        } else if kind.is_file() {
            budget.bytes += entry.metadata().map_err(io_error)?.len();
            if budget.bytes > SIZE_LIMIT {
                return Err(too_big());
            }
            std::fs::copy(entry.path(), &target).map_err(io_error)?;
        }
    }
    Ok(())
}

// Copies a folder holding a SKILL.md to `<claude home>/skills/<name>`. Never overwrites.
fn install_folder(claude_home: &Path, source: &Path) -> Result<PathBuf, SkillCatalogError> {
    // A link is not followed (the copy skips links, so the skill would arrive without its SKILL.md).
    let skill_md = source.join("SKILL.md");
    let meta = std::fs::symlink_metadata(&skill_md).map_err(|_| invalid("That folder has no SKILL.md."))?;
    if !meta.is_file() {
        return Err(invalid("That folder has no SKILL.md."));
    }
    if meta.len() > TEXT_LIMIT {
        return Err(invalid("SKILL.md is too large to add."));
    }
    let text = std::fs::read_to_string(&skill_md).map_err(|_| invalid("That folder has no SKILL.md."))?;
    let name = parse_frontmatter(&text)
        .and_then(|fields| fields.get("name").cloned())
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid("SKILL.md needs a name in its front matter."))?;
    check_name(&name)?;
    let skills = claude_home.join("skills");
    std::fs::create_dir_all(&skills).map_err(io_error)?;
    let dest = skills.join(&name);
    match std::fs::create_dir(&dest) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(SkillCatalogError::Exists(name)),
        Err(e) => return Err(io_error(e)),
    }
    let mut budget = Budget { entries: 0, bytes: 0 };
    if let Err(error) = copy_dir(source, &dest, &mut budget) {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(error);
    }
    Ok(dest)
}

// Unpacks `zip_path` into `into` (an empty folder), refusing anything that could write elsewhere.
fn unpack_zip(zip_path: &Path, into: &Path) -> Result<(), SkillCatalogError> {
    let file = std::fs::File::open(zip_path).map_err(io_error)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| invalid("That file is not a zip archive."))?;
    if archive.len() > ENTRY_LIMIT {
        return Err(too_big());
    }
    let mut total: u64 = 0;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|e| SkillCatalogError::Invalid(e.to_string()))?;
        let Some(relative) = entry.enclosed_name() else {
            return Err(invalid("The zip has a file outside its own folder."));
        };
        if entry.unix_mode().is_some_and(|mode| mode & 0o170000 == 0o120000) {
            return Err(invalid("The zip has a link in it."));
        }
        let target = into.join(relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&target).map_err(io_error)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        let mut out = std::fs::File::create(&target).map_err(io_error)?;
        // Count what is really written: the size an entry declares can be a lie.
        let written = std::io::copy(&mut (&mut entry).take(SIZE_LIMIT - total + 1), &mut out).map_err(io_error)?;
        total += written;
        if total > SIZE_LIMIT {
            return Err(too_big());
        }
    }
    Ok(())
}

// The folder of an unpacked zip that holds SKILL.md: the top, or the one folder at the top.
fn find_skill_root(unpacked: &Path) -> Result<PathBuf, SkillCatalogError> {
    if unpacked.join("SKILL.md").is_file() {
        return Ok(unpacked.to_path_buf());
    }
    let mut folders = Vec::new();
    for entry in std::fs::read_dir(unpacked).map_err(io_error)?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_dir() && name != "__MACOSX" && !name.starts_with('.') {
            folders.push(entry.path());
        }
    }
    match folders.as_slice() {
        [only] if only.join("SKILL.md").is_file() => Ok(only.clone()),
        _ => Err(invalid("The zip has no SKILL.md at its top.")),
    }
}

fn install_zip(claude_home: &Path, zip_path: &Path) -> Result<PathBuf, SkillCatalogError> {
    let temp = claude_home.join(format!(".forge-upload-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp).map_err(io_error)?;
    let result = unpack_zip(zip_path, &temp).and_then(|()| find_skill_root(&temp)).and_then(|root| install_folder(claude_home, &root));
    let _ = std::fs::remove_dir_all(&temp);
    result
}

fn create_skill(claude_home: &Path, name: &str, description: &str) -> Result<PathBuf, SkillCatalogError> {
    check_name(name)?;
    let description = description.split_whitespace().collect::<Vec<_>>().join(" ");
    if description.is_empty() {
        return Err(invalid("Write a short description."));
    }
    let skills = claude_home.join("skills");
    std::fs::create_dir_all(&skills).map_err(io_error)?;
    let dest = skills.join(name);
    match std::fs::create_dir(&dest) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => return Err(SkillCatalogError::Exists(name.to_string())),
        Err(e) => return Err(io_error(e)),
    }
    let quoted = description.replace('\\', "\\\\").replace('"', "\\\"");
    let text = format!(
        "---\nname: {name}\ndescription: \"{quoted}\"\n---\n\n# {name}\n\nDescribe what this skill does and when Claude should use it.\n"
    );
    if let Err(error) = std::fs::write(dest.join("SKILL.md"), text) {
        let _ = std::fs::remove_dir_all(&dest);
        return Err(io_error(error));
    }
    Ok(dest)
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, SkillCatalogError> + Send + 'static) -> Result<T, SkillCatalogError> {
    tokio::task::spawn_blocking(work).await.map_err(|e| SkillCatalogError::Io(e.to_string()))?
}

/// A file larger than this is not shown.
pub const TEXT_LIMIT: u64 = 512 * 1024;

fn walk(root: &Path, dir: &Path, out: &mut Vec<SkillFile>) {
    let Ok(read) = std::fs::read_dir(dir) else { return };
    for entry in read.flatten() {
        if out.len() >= ENTRY_LIMIT {
            return;
        }
        let Ok(kind) = entry.file_type() else { continue };
        if kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        let Ok(relative) = path.strip_prefix(root) else { continue };
        let relative = relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        out.push(SkillFile { relative, is_dir: kind.is_dir() });
        if kind.is_dir() {
            walk(root, &path, out);
        }
    }
}

// Only a folder with a SKILL.md of its own is a skill; any other folder is not shown.
fn check_skill_folder(skill_path: &Path) -> Result<(), SkillCatalogError> {
    match std::fs::symlink_metadata(skill_path.join("SKILL.md")) {
        Ok(meta) if meta.is_file() => Ok(()),
        _ => Err(invalid("That folder is not a skill.")),
    }
}

fn list_skill_files(skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError> {
    check_skill_folder(skill_path)?;
    let mut files = Vec::new();
    walk(skill_path, skill_path, &mut files);
    // Compared part by part, so `a` and `a/one.md` stay together and `a-b.md` comes after them.
    files.sort_by_key(|file| file.relative.split('/').map(str::to_lowercase).collect::<Vec<_>>());
    Ok(files)
}

fn read_skill_file(skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError> {
    check_skill_folder(skill_path)?;
    let outside = || invalid("That path is outside the skill.");
    let relative = Path::new(relative);
    if relative.components().any(|part| !matches!(part, Component::Normal(_))) {
        return Err(outside());
    }
    let root = std::fs::canonicalize(skill_path).map_err(io_error)?;
    let target = std::fs::canonicalize(root.join(relative)).map_err(io_error)?;
    if !target.starts_with(&root) {
        return Err(outside());
    }
    let meta = std::fs::metadata(&target).map_err(io_error)?;
    if !meta.is_file() {
        return Err(invalid("That is not a file."));
    }
    if meta.len() > TEXT_LIMIT {
        return Err(invalid("This file is too large to show."));
    }
    let bytes = std::fs::read(&target).map_err(io_error)?;
    String::from_utf8(bytes).map_err(|_| invalid("This is not a text file."))
}

/// Reads and writes the skills the Customize page shows, below the Claude home.
pub struct ClaudeSkillCatalogAdapter {
    claude_home: PathBuf,
}

impl ClaudeSkillCatalogAdapter {
    /// Also removes what a zip add that crashed left behind (`.forge-upload-*` in the Claude home):
    /// nothing is running yet, so none of it is in use.
    pub fn new(claude_home: PathBuf) -> Self {
        if let Ok(entries) = std::fs::read_dir(&claude_home) {
            for entry in entries.flatten() {
                if entry.file_name().to_string_lossy().starts_with(".forge-upload-") {
                    let _ = std::fs::remove_dir_all(entry.path());
                }
            }
        }
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
    // Each entry on its own, so one broken entry does not hide the others.
    #[serde(default)]
    plugins: Vec<serde_json::Value>,
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
    add_root_within(root, None, origin, installed, visited, out).await
}

// `within` keeps out a skill folder that is a link to somewhere outside it (a marketplace is somebody
// else's folder: what it links to is not its content).
async fn add_root_within(
    root: &Path,
    within: Option<&Path>,
    origin: SkillOrigin,
    installed: bool,
    visited: &mut HashSet<PathBuf>,
    out: &mut Vec<SkillEntry>,
) -> Result<(), SkillCatalogError> {
    let mut found = Vec::new();
    collect_skills(root, visited, &mut found).await.map_err(|e| SkillCatalogError::Io(e.to_string()))?;
    let within = match within {
        Some(dir) => Some(tokio::fs::canonicalize(dir).await.map_err(|e| SkillCatalogError::Io(e.to_string()))?),
        None => None,
    };
    for skill in found {
        if let Some(dir) = &within {
            let inside = tokio::fs::canonicalize(&skill.path).await.is_ok_and(|path| path.starts_with(dir));
            if !inside {
                continue;
            }
        }
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
            for offered in file.plugins.into_iter().filter_map(|entry| serde_json::from_value::<OfferedPlugin>(entry).ok()) {
                let id = plugin_id(&offered.name, &market);
                if installed.contains(id.as_str()) {
                    continue;
                }
                let Some(plugin_dir) = local_plugin_dir(&dir, &offered.source) else { continue };
                let _ = add_root_within(
                    &plugin_dir.join("skills"),
                    Some(&plugin_dir),
                    SkillOrigin::Plugin { plugin_id: id },
                    false,
                    &mut visited,
                    &mut skills,
                )
                .await;
            }
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id)));
        Ok(SkillSnapshot { skills })
    }
}

#[async_trait]
impl SkillCatalogPort for ClaudeSkillCatalogAdapter {
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<SkillSnapshot, SkillCatalogError> {
        self.read_snapshot(project_path, plugins).await
    }

    async fn add_from_folder(&self, source: &Path) -> Result<PathBuf, SkillCatalogError> {
        let (home, source) = (self.claude_home.clone(), source.to_path_buf());
        blocking(move || install_folder(&home, &source)).await
    }

    async fn add_from_zip(&self, source: &Path) -> Result<PathBuf, SkillCatalogError> {
        let (home, source) = (self.claude_home.clone(), source.to_path_buf());
        blocking(move || install_zip(&home, &source)).await
    }

    async fn create(&self, name: &str, description: &str) -> Result<PathBuf, SkillCatalogError> {
        let (home, name, description) = (self.claude_home.clone(), name.to_string(), description.to_string());
        blocking(move || create_skill(&home, &name, &description)).await
    }

    async fn list_files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError> {
        let skill_path = skill_path.to_path_buf();
        blocking(move || list_skill_files(&skill_path)).await
    }

    async fn read_file(&self, skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError> {
        let (skill_path, relative) = (skill_path.to_path_buf(), relative.to_string());
        blocking(move || read_skill_file(&skill_path, &relative)).await
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

    use std::io::Write;

    fn make_zip(path: &Path, files: &[(&str, &str)]) {
        let mut writer = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        for (name, body) in files {
            writer.start_file(*name, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
    }

    fn skill_md(name: &str) -> String {
        format!("---\nname: {name}\ndescription: A skill\n---\n\nBody\n")
    }

    fn adapter(home: &Path) -> ClaudeSkillCatalogAdapter {
        ClaudeSkillCatalogAdapter::new(home.to_path_buf())
    }

    // Everything below `dir`, as relative paths, to prove nothing was left behind.
    fn tree(dir: &Path) -> Vec<String> {
        fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
            if let Ok(read) = std::fs::read_dir(dir) {
                for entry in read.flatten() {
                    let path = entry.path();
                    out.push(path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/"));
                    if path.is_dir() {
                        walk(root, &path, out);
                    }
                }
            }
        }
        let mut out = Vec::new();
        walk(dir, dir, &mut out);
        out.sort();
        out
    }

    #[tokio::test]
    async fn a_folder_with_a_skill_is_copied_into_the_user_skills() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("SKILL.md"), skill_md("pdf")).unwrap();
        std::fs::create_dir(source.path().join("scripts")).unwrap();
        std::fs::write(source.path().join("scripts").join("run.sh"), "echo hi").unwrap();

        let added = adapter(home.path()).add_from_folder(source.path()).await.unwrap();

        assert_eq!(added, home.path().join("skills").join("pdf"));
        assert_eq!(tree(&added), vec!["SKILL.md", "scripts", "scripts/run.sh"]);
    }

    #[tokio::test]
    async fn a_folder_without_skill_md_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("README.md"), "hi").unwrap();
        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();
        assert_eq!(error, SkillCatalogError::Invalid("That folder has no SKILL.md.".to_string()));
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn a_skill_md_without_a_name_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("SKILL.md"), "---\ndescription: no name\n---\n").unwrap();
        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();
        assert!(matches!(error, SkillCatalogError::Invalid(_)));
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn adding_a_name_that_exists_is_refused_and_the_old_files_stay() {
        let home = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "pdf", "pdf", "the old one");
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("SKILL.md"), skill_md("pdf")).unwrap();

        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();

        assert_eq!(error, SkillCatalogError::Exists("pdf".to_string()));
        let kept = std::fs::read_to_string(home.path().join("skills").join("pdf").join("SKILL.md")).unwrap();
        assert!(kept.contains("the old one"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_folder_copy_skips_symlinks() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("SKILL.md"), skill_md("pdf")).unwrap();
        std::os::unix::fs::symlink("/etc/passwd", source.path().join("link")).unwrap();
        let added = adapter(home.path()).add_from_folder(source.path()).await.unwrap();
        assert_eq!(tree(&added), vec!["SKILL.md"]);
    }

    #[tokio::test]
    async fn a_folder_with_too_many_entries_is_refused_and_nothing_is_left() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        std::fs::write(source.path().join("SKILL.md"), skill_md("big")).unwrap();
        for i in 0..ENTRY_LIMIT {
            std::fs::write(source.path().join(format!("f{i}.txt")), "x").unwrap();
        }
        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();
        assert!(matches!(error, SkillCatalogError::Invalid(_)));
        assert!(!home.path().join("skills").join("big").exists());
    }

    #[tokio::test]
    async fn a_zip_with_the_skill_at_its_top_is_added() {
        let home = tempfile::tempdir().unwrap();
        let zip_dir = tempfile::tempdir().unwrap();
        let zip = zip_dir.path().join("pdf.zip");
        make_zip(&zip, &[("SKILL.md", &skill_md("pdf")), ("notes/a.txt", "a")]);

        let added = adapter(home.path()).add_from_zip(&zip).await.unwrap();

        assert_eq!(added, home.path().join("skills").join("pdf"));
        assert_eq!(tree(&added), vec!["SKILL.md", "notes", "notes/a.txt"]);
        // No temp folder is left in the Claude home.
        assert_eq!(tree(home.path()), vec!["skills", "skills/pdf", "skills/pdf/SKILL.md", "skills/pdf/notes", "skills/pdf/notes/a.txt"]);
    }

    #[tokio::test]
    async fn a_zip_with_the_skill_in_its_one_top_folder_is_added() {
        let home = tempfile::tempdir().unwrap();
        let zip_dir = tempfile::tempdir().unwrap();
        let zip = zip_dir.path().join("pdf.zip");
        make_zip(&zip, &[("pdf-folder/SKILL.md", &skill_md("pdf")), ("__MACOSX/pdf-folder/._SKILL.md", "junk")]);
        let added = adapter(home.path()).add_from_zip(&zip).await.unwrap();
        assert_eq!(tree(&added), vec!["SKILL.md"]);
    }

    #[tokio::test]
    async fn a_zip_entry_that_leaves_its_folder_is_refused_and_nothing_is_written() {
        let home = tempfile::tempdir().unwrap();
        let zip_dir = tempfile::tempdir().unwrap();
        let zip = zip_dir.path().join("evil.zip");
        make_zip(&zip, &[("SKILL.md", &skill_md("evil")), ("../evil.txt", "owned")]);

        let error = adapter(home.path()).add_from_zip(&zip).await.unwrap_err();

        assert!(matches!(error, SkillCatalogError::Invalid(_)));
        assert!(!home.path().join("evil.txt").exists());
        assert!(!home.path().parent().unwrap().join("evil.txt").exists());
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn a_zip_with_a_symlink_entry_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let zip_dir = tempfile::tempdir().unwrap();
        let zip = zip_dir.path().join("link.zip");
        let mut writer = zip::ZipWriter::new(std::fs::File::create(&zip).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("SKILL.md", options).unwrap();
        writer.write_all(skill_md("link").as_bytes()).unwrap();
        writer.add_symlink("shortcut", "/etc/passwd", options).unwrap();
        writer.finish().unwrap();

        let error = adapter(home.path()).add_from_zip(&zip).await.unwrap_err();

        assert!(matches!(error, SkillCatalogError::Invalid(_)));
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn a_zip_with_too_many_entries_or_too_much_content_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let zip_dir = tempfile::tempdir().unwrap();
        let many = zip_dir.path().join("many.zip");
        let names: Vec<String> = (0..=ENTRY_LIMIT).map(|i| format!("f{i}.txt")).collect();
        let mut files: Vec<(&str, &str)> = names.iter().map(|n| (n.as_str(), "x")).collect();
        let md = skill_md("many");
        files.push(("SKILL.md", &md));
        make_zip(&many, &files);
        assert!(matches!(adapter(home.path()).add_from_zip(&many).await, Err(SkillCatalogError::Invalid(_))));

        let heavy = zip_dir.path().join("heavy.zip");
        let md = skill_md("heavy");
        let blob = "x".repeat(SIZE_LIMIT as usize + 1);
        make_zip(&heavy, &[("SKILL.md", &md), ("blob.txt", &blob)]);
        assert!(matches!(adapter(home.path()).add_from_zip(&heavy).await, Err(SkillCatalogError::Invalid(_))));
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn a_file_that_is_not_a_zip_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("fake.zip");
        std::fs::write(&fake, "not a zip").unwrap();
        let error = adapter(home.path()).add_from_zip(&fake).await.unwrap_err();
        assert_eq!(error, SkillCatalogError::Invalid("That file is not a zip archive.".to_string()));
    }

    #[tokio::test]
    async fn create_writes_a_template_that_reads_back_as_a_skill() {
        let home = tempfile::tempdir().unwrap();
        let created = adapter(home.path()).create("my-skill", "Does \"things\": well").await.unwrap();
        assert_eq!(created, home.path().join("skills").join("my-skill"));
        let snap = snapshot(home.path(), None, vec![]).await;
        assert_eq!(snap.skills.len(), 1);
        assert_eq!(snap.skills[0].name, "my-skill");
        assert!(snap.skills[0].description.contains("things"));
    }

    #[tokio::test]
    async fn create_refuses_bad_names_and_existing_names_and_an_empty_description() {
        let home = tempfile::tempdir().unwrap();
        let adapter = adapter(home.path());
        for bad in ["", "../x", "a/b", "a\\b", ".hidden", "has space", &"x".repeat(65)] {
            let error = adapter.create(bad, "d").await.unwrap_err();
            assert!(matches!(error, SkillCatalogError::Invalid(_)), "name {bad:?}");
        }
        assert!(matches!(adapter.create("ok", "  ").await, Err(SkillCatalogError::Invalid(_))));
        assert!(tree(home.path()).is_empty());
        adapter.create("ok", "d").await.unwrap();
        assert_eq!(adapter.create("ok", "d").await.unwrap_err(), SkillCatalogError::Exists("ok".to_string()));
    }

    #[tokio::test]
    async fn list_files_gives_a_tree_order_with_slashes() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::fs::write(dir.join("a").join("one.md"), "x").unwrap();
        std::fs::write(dir.join("a-b.md"), "x").unwrap();

        let files = adapter(home.path()).list_files(&dir).await.unwrap();

        let shown: Vec<_> = files.iter().map(|f| (f.relative.as_str(), f.is_dir)).collect();
        assert_eq!(shown, vec![("a", true), ("a/one.md", false), ("a-b.md", false), ("SKILL.md", false)]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_files_leaves_out_symlinks() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::os::unix::fs::symlink("/etc", dir.join("etc")).unwrap();
        let files = adapter(home.path()).list_files(&dir).await.unwrap();
        assert_eq!(files.len(), 1);
    }

    #[tokio::test]
    async fn read_file_returns_the_text_of_a_file_inside_the_skill() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::fs::write(dir.join("sub").join("a.md"), "hello").unwrap();
        assert_eq!(adapter(home.path()).read_file(&dir, "sub/a.md").await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn read_file_refuses_paths_that_leave_the_skill() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::fs::write(home.path().join("secret.txt"), "secret").unwrap();
        let adapter = adapter(home.path());
        for bad in ["../../secret.txt", "..", "/etc/passwd", "sub/../../../secret.txt"] {
            let result = adapter.read_file(&dir, bad).await;
            assert!(matches!(result, Err(SkillCatalogError::Invalid(_))), "path {bad:?}: {result:?}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn read_file_refuses_a_symlink_that_points_out_of_the_skill() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::fs::write(home.path().join("secret.txt"), "secret").unwrap();
        std::os::unix::fs::symlink(home.path().join("secret.txt"), dir.join("link.txt")).unwrap();
        let result = adapter(home.path()).read_file(&dir, "link.txt").await;
        assert!(matches!(result, Err(SkillCatalogError::Invalid(_))));
    }

    #[tokio::test]
    async fn read_file_refuses_a_big_file_a_binary_file_and_a_folder() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("SKILL.md"), "x").unwrap();
        std::fs::write(dir.join("big.txt"), "x".repeat(TEXT_LIMIT as usize + 1)).unwrap();
        std::fs::write(dir.join("bin.dat"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
        let adapter = adapter(home.path());
        for bad in ["big.txt", "bin.dat", "sub"] {
            assert!(matches!(adapter.read_file(&dir, bad).await, Err(SkillCatalogError::Invalid(_))), "{bad}");
        }
    }

    #[tokio::test]
    async fn create_refuses_a_trailing_dot_and_windows_device_names() {
        let home = tempfile::tempdir().unwrap();
        let adapter = adapter(home.path());
        for bad in ["name.", "con", "CON", "nul", "Aux", "prn", "com1", "LPT9", "con.txt", "aux.md"] {
            let result = adapter.create(bad, "d").await;
            assert!(matches!(result, Err(SkillCatalogError::Invalid(_))), "name {bad:?}: {result:?}");
        }
        assert!(tree(home.path()).is_empty());
        // Names that only look like them are fine.
        adapter.create("console", "d").await.unwrap();
        adapter.create("com10", "d").await.unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_folder_whose_skill_md_is_a_symlink_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        std::fs::write(elsewhere.path().join("real.md"), skill_md("sneaky")).unwrap();
        std::os::unix::fs::symlink(elsewhere.path().join("real.md"), source.path().join("SKILL.md")).unwrap();
        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();
        assert_eq!(error, SkillCatalogError::Invalid("That folder has no SKILL.md.".to_string()));
        assert!(tree(home.path()).is_empty());
    }

    #[tokio::test]
    async fn a_skill_md_that_is_too_big_to_read_is_refused() {
        let home = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let mut text = skill_md("big");
        text.push_str(&"x".repeat(TEXT_LIMIT as usize));
        std::fs::write(source.path().join("SKILL.md"), text).unwrap();
        let error = adapter(home.path()).add_from_folder(source.path()).await.unwrap_err();
        assert!(matches!(error, SkillCatalogError::Invalid(_)));
        assert!(tree(home.path()).is_empty());
    }

    #[test]
    fn leftovers_of_a_zip_add_that_crashed_are_swept_when_the_adapter_starts() {
        let home = tempfile::tempdir().unwrap();
        let stale = home.path().join(".forge-upload-1234");
        std::fs::create_dir_all(stale.join("pdf")).unwrap();
        std::fs::write(stale.join("pdf").join("SKILL.md"), "x").unwrap();
        std::fs::create_dir_all(home.path().join("skills").join("keep")).unwrap();
        std::fs::create_dir_all(home.path().join(".forge-other")).unwrap();

        let _adapter = adapter(home.path());

        assert!(!stale.exists());
        assert!(home.path().join("skills").join("keep").exists(), "only .forge-upload-* goes");
        assert!(home.path().join(".forge-other").exists());
    }

    #[tokio::test]
    async fn the_files_of_a_folder_that_is_not_a_skill_are_not_listed_or_read() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("not-a-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "private").unwrap();
        let adapter = adapter(home.path());
        assert_eq!(adapter.list_files(&dir).await.unwrap_err(), SkillCatalogError::Invalid("That folder is not a skill.".to_string()));
        assert_eq!(adapter.read_file(&dir, "notes.txt").await.unwrap_err(), SkillCatalogError::Invalid("That folder is not a skill.".to_string()));
    }

    #[tokio::test]
    async fn one_plugin_entry_without_a_name_does_not_hide_the_rest_of_its_marketplace() {
        let home = tempfile::tempdir().unwrap();
        let market = add_marketplace(
            home.path(),
            "m",
            r#"{"plugins":[{"source":"./plugins/nameless"},"just text",{"name":"fine","source":"./plugins/fine"}]}"#,
        );
        write_skill(&market.join("plugins").join("fine").join("skills"), "ok", "ok", "d");
        let snap = snapshot(home.path(), None, vec![]).await;
        let ids: Vec<_> = snap.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["plugin:fine@m:ok"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a_skill_folder_that_is_a_symlink_out_of_the_marketplace_is_not_followed() {
        let home = tempfile::tempdir().unwrap();
        let market = add_marketplace(home.path(), "m", r#"{"plugins":[{"name":"p","source":"./plugins/p"}]}"#);
        let outside = tempfile::tempdir().unwrap();
        write_skill(outside.path(), "secret", "secret", "outside the marketplace");
        let skills = market.join("plugins").join("p").join("skills");
        std::fs::create_dir_all(&skills).unwrap();
        std::os::unix::fs::symlink(outside.path().join("secret"), skills.join("linked")).unwrap();
        write_skill(&skills, "inside", "inside", "d");

        let snap = snapshot(home.path(), None, vec![]).await;

        let ids: Vec<_> = snap.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["plugin:p@m:inside"]);
    }
}
