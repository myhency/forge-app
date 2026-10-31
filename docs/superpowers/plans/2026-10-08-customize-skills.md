# Customize page, Skills tab (30B) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Skills tab of the Customize page lists the skills a person has (made by them, or from plugins), opens one to read its files, finds skills in the marketplaces they added, adds a skill from a folder or a zip, and creates an empty one.

**Architecture:** A new read-and-write port `SkillCatalogPort` (forge-ports) with one adapter in `forge-adapter-claude-fs` that scans the user, project, installed-plugin and marketplace skill folders, copies a folder or unpacks a zip into `<claude home>/skills`, and lists and reads a skill's files. A use case `SkillCatalogUseCase` (forge-application) joins that with Forge's `enabled_skills` and the plugin catalogue that 30A already reads. Pure page logic in forge-features, the existing `Customize` state grows the skills fields, and a new widget `customize_skills.rs` draws Mine, Explore and the detail. The session start path (`SkillRepositoryPort`, `ActiveExtensions::select`) is not touched.

**Tech Stack:** Rust, Dioxus 0.7 desktop, tokio, serde_json, mockall, the `zip` crate (new), `theme.css`, the Python harness in `scripts/harness/`.

**Spec:** `docs/superpowers/specs/2026-10-08-customize-skills-design.md`. It builds on `docs/superpowers/specs/2026-10-08-customize-plugins-design.md` (30A) and `docs/superpowers/plans/2026-10-08-customize-plugins.md`. Observations: `docs/superpowers/roadmap/observations.md`, Round 13.

## Global Constraints

- `SkillRepositoryPort`, `ClaudeSkillAdapter::list_skills` and `ActiveExtensions::select` keep their behaviour: a session starts exactly as before.
- A user or project skill is switched on and off by `enabled_skills` (by skill name). A plugin skill has no switch of its own: its row's switch is disabled with the title "Turned on and off with its plugin", and "on" means the plugin's id is in `enabled_plugins`.
- Adding or creating a skill never overwrites: if `<claude home>/skills/<name>` exists, refuse. A skill name uses only ASCII letters, digits, `-`, `_`, `.`, is 1 to 64 characters long and does not start with `.`.
- Limits for a folder copy and for a zip: at most 500 entries and 20 MB (20 * 1024 * 1024 bytes) of file content. A folder copy skips symlinks. A zip is refused when an entry's path leaves its folder (zip-slip), when an entry is a symlink, or when it passes a limit. A failed add leaves nothing behind (no half-copied folder, no temp folder).
- Reading a skill's file: only paths inside the skill's folder (no `..`, no absolute path, no symlink out), at most 512 KB, UTF-8 text only.
- Explore reads only plugins whose marketplace `source` is a relative path starting with `./` and without `..`; any other plugin is skipped. A broken or unreadable marketplace, plugin or skill folder is skipped and the rest still show.
- A skill added or created is switched on in Forge (its name goes into `enabled_skills`), like an install switches a plugin on.
- UI text is English. Code style: match the surrounding crates (plain English doc comments on public items, test names as sentences, signals are `Copy`, `spawn_forever` for work that must survive the component).
- Every commit ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.

## Review Focus

- A zip entry named `../evil.txt` (or an absolute path) is refused, nothing is written outside the unpack folder, and no folder is left behind (Task 4 test).
- Uploading a folder whose skill name already exists is refused and the existing skill's files are untouched (Task 4 test).
- Create with a name like `../x`, `a/b`, `.hidden` or an empty name is refused before anything is written (Task 4 test).
- `read_file` with `../SKILL.md`, an absolute path, or a symlink pointing out of the skill folder returns an error and never the outside file (Task 5 tests).
- A marketplace whose `marketplace.json` is not valid JSON, or whose plugin has a `source` that is an object or contains `..`, is skipped while the other marketplaces' skills still show (Task 3 test).
- A user skill and a plugin skill with the same name show as two rows with different ids; the user one has a switch, the plugin one a disabled switch that follows the plugin (Task 3 and Task 7 tests).

## File Structure

Create:
- `crates/forge-domain/src/skill_catalog.rs`: `SkillOrigin`, `SkillEntry`, `SkillFile`, `SkillSnapshot`, `skill_id`.
- `crates/forge-ports/src/skill_catalog_port.rs`: `SkillCatalogPort`, `SkillCatalogError`.
- `crates/forge-adapter-claude-fs/src/skill_catalog.rs`: `ClaudeSkillCatalogAdapter` (scan, add, create, files).
- `crates/forge-application/src/use_cases/skill_catalog.rs`: `SkillCatalogUseCase`, `SkillView`, `SkillsError`.
- `crates/forge-features/src/customize_skills.rs`: pure page logic (rows, groups, explore cards, file helpers, `SkillPane`, `SkillText`).
- `crates/forge-widgets/src/customize_skills.rs`: `SkillsTab`, `SkillDetail`, `SkillFiles`.

Modify:
- `crates/forge-adapter-claude-fs/src/skills.rs` (make `parse_frontmatter`, `read_skill`, `collect_skills` `pub(crate)`), `.../lib.rs`, `.../Cargo.toml`.
- `crates/forge-domain/src/lib.rs`, `crates/forge-ports/src/lib.rs`, `crates/forge-application/src/lib.rs`, `.../use_cases/mod.rs`, `crates/forge-features/src/lib.rs`.
- `crates/forge-features/src/workspace.rs` (`WorkspaceServices.skill_catalog`), `crates/forge-app/src/main.rs` (wiring).
- `crates/forge-features/src/customize_state.rs` (skills fields and actions), `crates/forge-widgets/src/customize_page.rs`, `.../customize_dialogs.rs`, `.../lib.rs`, `crates/forge-shared/src/theme.css`.
- `scripts/harness/add_market_fixture.py` and its test, the 30B spec ("As built"), the roadmap, README.

---

### Task 1: Domain types, the shared front-matter reader, and the spec fix

**Files:**
- Create: `crates/forge-domain/src/skill_catalog.rs`
- Modify: `crates/forge-domain/src/lib.rs` (add `mod skill_catalog;` and the `pub use` line), `crates/forge-adapter-claude-fs/src/skills.rs` (three `pub(crate)`), `docs/superpowers/specs/2026-10-08-customize-skills-design.md`

**Interfaces:**
- Produces (all in `forge_domain`):
  - `enum SkillOrigin { User, Project, Plugin { plugin_id: String } }` with `fn is_plugin(&self) -> bool` and `fn plugin_id(&self) -> Option<&str>`.
  - `struct SkillEntry { id: String, name: String, description: String, origin: SkillOrigin, installed: bool, path: PathBuf }`.
  - `struct SkillFile { relative: String, is_dir: bool }`; `relative` uses `/`.
  - `struct SkillSnapshot { skills: Vec<SkillEntry> }` (`Default`).
  - `fn skill_id(origin: &SkillOrigin, folder: &str) -> String`.

- [ ] **Step 1: Write the failing test**

Create `crates/forge-domain/src/skill_catalog.rs`:

```rust
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_folder_name_gets_a_different_id_in_each_place() {
        let plugin = SkillOrigin::Plugin { plugin_id: "hello@m".to_string() };
        assert_eq!(skill_id(&SkillOrigin::User, "greet"), "user:greet");
        assert_eq!(skill_id(&SkillOrigin::Project, "greet"), "project:greet");
        assert_eq!(skill_id(&plugin, "greet"), "plugin:hello@m:greet");
    }

    #[test]
    fn only_a_plugin_origin_is_a_plugin() {
        assert!(!SkillOrigin::User.is_plugin());
        assert!(!SkillOrigin::Project.is_plugin());
        let plugin = SkillOrigin::Plugin { plugin_id: "a@b".to_string() };
        assert!(plugin.is_plugin());
        assert_eq!(plugin.plugin_id(), Some("a@b"));
        assert_eq!(SkillOrigin::User.plugin_id(), None);
    }
}
```

In `crates/forge-domain/src/lib.rs` add `mod skill_catalog;` next to `mod plugin_catalog;` and, next to the `pub use plugin_catalog::...` line:

```rust
pub use skill_catalog::{skill_id, SkillEntry, SkillFile, SkillOrigin, SkillSnapshot};
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p forge-domain skill_catalog`
Expected: FAIL to compile ("cannot find ... `skill_id`", `SkillOrigin`).

- [ ] **Step 3: Write minimal implementation**

Put above the `#[cfg(test)]` line in `skill_catalog.rs`:

```rust
/// Where a skill comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillOrigin {
    /// `<claude home>/skills`.
    User,
    /// `<project>/.claude/skills`.
    Project,
    /// A plugin's `skills` folder; `plugin_id` is `name@marketplace`.
    Plugin { plugin_id: String },
}

impl SkillOrigin {
    pub fn is_plugin(&self) -> bool {
        matches!(self, SkillOrigin::Plugin { .. })
    }

    pub fn plugin_id(&self) -> Option<&str> {
        match self {
            SkillOrigin::Plugin { plugin_id } => Some(plugin_id),
            _ => None,
        }
    }
}

/// One skill as the Customize page lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillEntry {
    /// `skill_id(origin, folder)`: unique across places, so two skills of one name stay apart.
    pub id: String,
    /// The `name` in the SKILL.md front matter; what `enabled_skills` stores.
    pub name: String,
    pub description: String,
    pub origin: SkillOrigin,
    /// False only for a skill of a plugin that a marketplace offers and is not installed.
    pub installed: bool,
    /// The skill's folder (the one holding SKILL.md).
    pub path: PathBuf,
}

/// A file or folder inside a skill's folder. `relative` uses `/` on every platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFile {
    pub relative: String,
    pub is_dir: bool,
}

/// Every skill the Customize page shows, sorted by name then id.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillSnapshot {
    pub skills: Vec<SkillEntry>,
}

/// `user:<folder>`, `project:<folder>` or `plugin:<plugin id>:<folder>`.
pub fn skill_id(origin: &SkillOrigin, folder: &str) -> String {
    match origin {
        SkillOrigin::User => format!("user:{folder}"),
        SkillOrigin::Project => format!("project:{folder}"),
        SkillOrigin::Plugin { plugin_id } => format!("plugin:{plugin_id}:{folder}"),
    }
}
```

In `crates/forge-adapter-claude-fs/src/skills.rs` change `fn parse_frontmatter(` to `pub(crate) fn parse_frontmatter(`, `async fn read_skill(` to `pub(crate) async fn read_skill(`, and `async fn collect_skills(` to `pub(crate) async fn collect_skills(`.

- [ ] **Step 4: Fix the spec**

In `docs/superpowers/specs/2026-10-08-customize-skills-design.md`, replace deviation 6 with:

```
6. **Create** makes a `SKILL.md` template and opens the detail on the Files tab with a "Show in folder" button (`Workspace::reveal`). The Files panel (25A) cannot be used: it only opens files inside a session's folder, and this page has no session. There is no editor inside this page.
```

and in the Screen section replace "Create skill (dialog: name, description)." context unchanged. Also change the Goal line "or create an empty one" to stay as is.

- [ ] **Step 5: Run tests**

Run: `cargo test -p forge-domain skill_catalog && cargo test -p forge-adapter-claude-fs`
Expected: PASS (the adapter's existing tests still pass).

- [ ] **Step 6: Commit**

```bash
git add crates/forge-domain crates/forge-adapter-claude-fs/src/skills.rs docs/superpowers/specs/2026-10-08-customize-skills-design.md
git commit -m "feat: skill catalogue domain types; spec: Show in folder instead of Files panel (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: The port

**Files:**
- Create: `crates/forge-ports/src/skill_catalog_port.rs`
- Modify: `crates/forge-ports/src/lib.rs`

**Interfaces:**
- Consumes: `forge_domain::{PluginEntry, SkillFile, SkillSnapshot}`.
- Produces (in `forge_ports`):
  - `enum SkillCatalogError { Invalid(String), Exists(String), Io(String) }` (`Display`: `Invalid` and `Io` show their text; `Exists(name)` shows `A skill named <name> already exists.`).
  - `trait SkillCatalogPort` (async, `Send + Sync`, `MockSkillCatalogPort` under `mocks`):
    - `snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<SkillSnapshot, SkillCatalogError>`
    - `add_from_folder(&self, source: &Path) -> Result<PathBuf, SkillCatalogError>` returns the new skill's folder
    - `add_from_zip(&self, source: &Path) -> Result<PathBuf, SkillCatalogError>`
    - `create(&self, name: &str, description: &str) -> Result<PathBuf, SkillCatalogError>`
    - `list_files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError>`
    - `read_file(&self, skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError>`

- [ ] **Step 1: Write the failing test**

`crates/forge-ports/src/skill_catalog_port.rs`:

```rust
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{PluginEntry, SkillFile, SkillSnapshot};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_mock_can_record_a_create_call() {
        let mut mock = MockSkillCatalogPort::new();
        mock.expect_create()
            .withf(|name, description| name == "pdf" && description == "Reads PDFs")
            .times(1)
            .returning(|_, _| Ok(PathBuf::from("/home/.claude/skills/pdf")));
        assert_eq!(mock.create("pdf", "Reads PDFs").await.unwrap(), PathBuf::from("/home/.claude/skills/pdf"));
    }

    #[test]
    fn the_errors_show_text_a_person_can_read() {
        assert_eq!(SkillCatalogError::Exists("pdf".into()).to_string(), "A skill named pdf already exists.");
        assert_eq!(SkillCatalogError::Invalid("That folder has no SKILL.md.".into()).to_string(), "That folder has no SKILL.md.");
    }
}
```

In `crates/forge-ports/src/lib.rs` add `mod skill_catalog_port;`, `pub use skill_catalog_port::{SkillCatalogError, SkillCatalogPort};` and, with the other mock exports:

```rust
#[cfg(any(test, feature = "mocks"))]
pub use skill_catalog_port::MockSkillCatalogPort;
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p forge-ports skill_catalog`
Expected: FAIL to compile (`SkillCatalogError` not defined).

- [ ] **Step 3: Write minimal implementation**

Insert above `#[cfg(test)]`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkillCatalogError {
    /// The input is not a usable skill (no SKILL.md, a bad name, a zip with a link in it, too big, ...): a message for the person.
    #[error("{0}")]
    Invalid(String),
    #[error("A skill named {0} already exists.")]
    Exists(String),
    /// The disk said no.
    #[error("{0}")]
    Io(String),
}

/// The skills the Customize page shows, and the ways to add one. Reads are side-effect free; the
/// three adds write only below `<claude home>/skills` and never overwrite.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait SkillCatalogPort: Send + Sync {
    /// Skills of the user folder, the project folder (when there is a project), the installed
    /// plugins in `plugins` and the plugins that marketplaces offer and are not installed.
    async fn snapshot(&self, project_path: Option<PathBuf>, plugins: Vec<PluginEntry>) -> Result<SkillSnapshot, SkillCatalogError>;
    /// Copies a folder holding a SKILL.md. Returns the new skill's folder.
    async fn add_from_folder(&self, source: &Path) -> Result<PathBuf, SkillCatalogError>;
    /// Unpacks a .zip holding a SKILL.md (at its top or in its one top folder). Returns the new skill's folder.
    async fn add_from_zip(&self, source: &Path) -> Result<PathBuf, SkillCatalogError>;
    /// Writes a SKILL.md template. Returns the new skill's folder.
    async fn create(&self, name: &str, description: &str) -> Result<PathBuf, SkillCatalogError>;
    async fn list_files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError>;
    /// One text file of the skill; `relative` uses `/`.
    async fn read_file(&self, skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError>;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p forge-ports skill_catalog`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-ports
git commit -m "feat: SkillCatalogPort (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: The adapter, reading (snapshot)

**Files:**
- Create: `crates/forge-adapter-claude-fs/src/skill_catalog.rs`
- Modify: `crates/forge-adapter-claude-fs/src/lib.rs` (`mod skill_catalog;`, `pub use skill_catalog::ClaudeSkillCatalogAdapter;`)

**Interfaces:**
- Consumes: `crate::skills::{collect_skills, parse_frontmatter, read_skill}` (Task 1), the port (Task 2), `forge_domain::{plugin_id, skill_id, ...}`.
- Produces: `ClaudeSkillCatalogAdapter::new(claude_home: PathBuf)`, implementing `SkillCatalogPort`. In this task only `snapshot` is real; the other five methods return `Err(SkillCatalogError::Io("not built yet".into()))` and are replaced in Tasks 4 and 5.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-adapter-claude-fs/src/skill_catalog.rs` with only the test module (the header `use`s come in Step 3):

```rust
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
```

In `crates/forge-adapter-claude-fs/src/lib.rs` add `mod skill_catalog;` and `pub use skill_catalog::ClaudeSkillCatalogAdapter;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs skill_catalog`
Expected: FAIL to compile (`ClaudeSkillCatalogAdapter` not defined).

- [ ] **Step 3: Write minimal implementation**

Insert at the top of `skill_catalog.rs` (above the test module):

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs skill_catalog`
Expected: PASS (8 tests). If `a_project_that_is_the_home_folder_...` fails, the shared `visited` set is not being passed to both roots: check that both `add_root` calls use the same `visited`.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: read skills of the user, project, plugins and marketplaces (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: The adapter, writing (folder, zip, create)

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/skill_catalog.rs`, `crates/forge-adapter-claude-fs/Cargo.toml`

**Interfaces:**
- Consumes: Task 3's adapter.
- Produces: the real `add_from_folder`, `add_from_zip`, `create`. Constants `ENTRY_LIMIT: usize = 500`, `SIZE_LIMIT: u64 = 20 * 1024 * 1024`.

- [ ] **Step 1: Add the dependency**

In `crates/forge-adapter-claude-fs/Cargo.toml` under `[dependencies]` add:

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

Run: `cargo build -p forge-adapter-claude-fs`
Expected: builds (it downloads `zip`). If cargo picks a `zip` whose API differs from the calls below (`ZipArchive::new`, `by_index`, `enclosed_name`, `unix_mode`, `is_dir`, `ZipWriter`, `SimpleFileOptions`, `add_symlink`), adjust the calls, not the behaviour.

- [ ] **Step 2: Write the failing tests**

Add inside the existing `mod tests` of `skill_catalog.rs` (below the Task 3 tests):

```rust
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
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs skill_catalog`
Expected: the new tests FAIL ("not built yet", or compile errors for `ENTRY_LIMIT`).

- [ ] **Step 4: Write the implementation**

In `skill_catalog.rs`, add near the top (after the `use`s; add `use std::io::Read;` and `use crate::skills::parse_frontmatter;`):

```rust
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

fn check_name(name: &str) -> Result<(), SkillCatalogError> {
    let allowed = !name.is_empty()
        && name.len() <= 64
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if allowed {
        Ok(())
    } else {
        Err(invalid("A skill name can use letters, numbers, - and _ (at most 64 characters)."))
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
    let text = std::fs::read_to_string(source.join("SKILL.md")).map_err(|_| invalid("That folder has no SKILL.md."))?;
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
```

Replace the three stub methods in the `impl SkillCatalogPort` block:

```rust
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
```

Add `uuid = { version = "1", features = ["v4"] }` is already a dependency of the crate (no change). Remove the `NOT_BUILT` constant only after Task 5 (it is still used by `list_files` and `read_file`).

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs skill_catalog`
Expected: PASS. If `a_zip_with_the_skill_in_its_one_top_folder_is_added` fails because `__MACOSX` has no SKILL.md, check `find_skill_root` ignores that folder. If the zip-slip test fails with a file outside, `enclosed_name` returned `Some` for `../evil.txt`: it must return `None`; do not hand-roll the check unless the crate's behaviour differs, and if it does, add `relative.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_)))` as a second check before `into.join`.

- [ ] **Step 6: Commit**

```bash
git add crates/forge-adapter-claude-fs Cargo.lock
git commit -m "feat: add a skill from a folder or a zip, or create one (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: The adapter, a skill's files

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/skill_catalog.rs`

**Interfaces:**
- Produces: the real `list_files` and `read_file`. `TEXT_LIMIT: u64 = 512 * 1024`.

- [ ] **Step 1: Write the failing tests**

Add to `mod tests`:

```rust
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
        std::fs::write(dir.join("sub").join("a.md"), "hello").unwrap();
        assert_eq!(adapter(home.path()).read_file(&dir, "sub/a.md").await.unwrap(), "hello");
    }

    #[tokio::test]
    async fn read_file_refuses_paths_that_leave_the_skill() {
        let home = tempfile::tempdir().unwrap();
        let dir = home.path().join("skills").join("pdf");
        std::fs::create_dir_all(&dir).unwrap();
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
        std::fs::write(dir.join("big.txt"), "x".repeat(TEXT_LIMIT as usize + 1)).unwrap();
        std::fs::write(dir.join("bin.dat"), [0xff, 0xfe, 0x00, 0x80]).unwrap();
        let adapter = adapter(home.path());
        for bad in ["big.txt", "bin.dat", "sub"] {
            assert!(matches!(adapter.read_file(&dir, bad).await, Err(SkillCatalogError::Invalid(_))), "{bad}");
        }
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p forge-adapter-claude-fs skill_catalog`
Expected: the new tests FAIL.

- [ ] **Step 3: Write the implementation**

Add to `skill_catalog.rs`:

```rust
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

fn list_skill_files(skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError> {
    let mut files = Vec::new();
    walk(skill_path, skill_path, &mut files);
    // Compared part by part, so `a` and `a/one.md` stay together and `a-b.md` comes after them.
    files.sort_by_key(|file| file.relative.split('/').map(str::to_lowercase).collect::<Vec<_>>());
    Ok(files)
}

fn read_skill_file(skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError> {
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
```

Replace the two stub methods and delete the `NOT_BUILT` constant:

```rust
    async fn list_files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillCatalogError> {
        let skill_path = skill_path.to_path_buf();
        blocking(move || list_skill_files(&skill_path)).await
    }

    async fn read_file(&self, skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError> {
        let (skill_path, relative) = (skill_path.to_path_buf(), relative.to_string());
        blocking(move || read_skill_file(&skill_path, &relative)).await
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p forge-adapter-claude-fs`
Expected: PASS (all adapter tests, old and new).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-adapter-claude-fs
git commit -m "feat: list and read a skill's files, inside the skill only (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: The use case and the wiring

**Files:**
- Create: `crates/forge-application/src/use_cases/skill_catalog.rs`
- Modify: `crates/forge-application/src/use_cases/mod.rs`, `crates/forge-application/src/lib.rs`, `crates/forge-features/src/workspace.rs` (field), `crates/forge-app/src/main.rs` (construct)

**Interfaces:**
- Consumes: `SkillCatalogPort` (Task 2), `CatalogView` (30A), `ConfigLock`, `ConfigRepositoryPort`.
- Produces (in `forge_application`):
  - `struct SkillView { snapshot: SkillSnapshot, enabled_skills: Vec<String>, enabled_plugins: Vec<String>, has_marketplaces: bool }` (`Clone, PartialEq`).
  - `enum SkillsError { Catalog(SkillCatalogError), Config(ConfigError) }` (`Display` shows the inner text).
  - `SkillCatalogUseCase::new(port: Arc<dyn SkillCatalogPort>, config: Arc<dyn ConfigRepositoryPort>)`, `.with_config_lock(ConfigLock)`.
  - `async fn skills(&self, project: Option<&Path>, plugins: &CatalogView) -> Result<SkillView, SkillsError>`
  - `async fn set_enabled(&self, name: &str, on: bool) -> Result<(), SkillsError>`
  - `async fn add_folder(&self, source: &Path) -> Result<AddedSkill, SkillsError>`, `add_zip(&self, source: &Path)`, `create(&self, name: &str, description: &str)`, each switching the new skill on.
  - `struct AddedSkill { name: String, id: String, path: PathBuf }` (`id` = `skill_id(User, name)`).
  - `async fn files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillsError>`, `async fn read(&self, skill_path: &Path, relative: &str) -> Result<String, SkillsError>`.
  - `WorkspaceServices.skill_catalog: Arc<SkillCatalogUseCase>`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-application/src/use_cases/skill_catalog.rs` with the test module only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{AppConfig, Marketplace, PluginSnapshot, SkillEntry, SkillOrigin};
    use forge_ports::{MockConfigRepositoryPort, MockSkillCatalogPort};
    use std::sync::Mutex as StdMutex;

    fn memory_config(enabled: &[&str]) -> (Arc<MockConfigRepositoryPort>, Arc<StdMutex<AppConfig>>, Arc<StdMutex<u32>>) {
        let state = Arc::new(StdMutex::new(AppConfig {
            enabled_skills: enabled.iter().map(|s| s.to_string()).collect(),
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

    fn use_case(port: MockSkillCatalogPort, config: Arc<MockConfigRepositoryPort>) -> SkillCatalogUseCase {
        SkillCatalogUseCase::new(Arc::new(port), config)
    }

    fn plugin_view(enabled: &[&str], markets: usize) -> CatalogView {
        CatalogView {
            snapshot: PluginSnapshot {
                plugins: vec![],
                marketplaces: (0..markets)
                    .map(|i| Marketplace { name: format!("m{i}"), kind: "git".into(), location: String::new(), last_updated: None })
                    .collect(),
            },
            enabled: enabled.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[tokio::test]
    async fn the_view_joins_the_snapshot_with_both_enabled_lists() {
        let mut port = MockSkillCatalogPort::new();
        let skill = SkillEntry {
            id: "user:pdf".into(),
            name: "pdf".into(),
            description: String::new(),
            origin: SkillOrigin::User,
            installed: true,
            path: "/s/pdf".into(),
        };
        let returned = skill.clone();
        port.expect_snapshot()
            .withf(|project, _| project.as_deref() == Some(Path::new("/work")))
            .returning(move |_, _| Ok(SkillSnapshot { skills: vec![returned.clone()] }));
        let (config, _, _) = memory_config(&["pdf"]);

        let view = use_case(port, config).skills(Some(Path::new("/work")), &plugin_view(&["hello@m"], 1)).await.unwrap();

        assert_eq!(view.snapshot.skills, vec![skill]);
        assert_eq!(view.enabled_skills, vec!["pdf".to_string()]);
        assert_eq!(view.enabled_plugins, vec!["hello@m".to_string()]);
        assert!(view.has_marketplaces);
    }

    #[tokio::test]
    async fn no_marketplace_in_the_plugin_view_means_has_marketplaces_is_false() {
        let mut port = MockSkillCatalogPort::new();
        port.expect_snapshot().returning(|_, _| Ok(SkillSnapshot::default()));
        let (config, _, _) = memory_config(&[]);
        let view = use_case(port, config).skills(None, &plugin_view(&[], 0)).await.unwrap();
        assert!(!view.has_marketplaces);
    }

    #[tokio::test]
    async fn switching_a_skill_on_and_off_edits_only_the_enabled_skills() {
        let (config, state, saves) = memory_config(&["a"]);
        let use_case = use_case(MockSkillCatalogPort::new(), config);
        use_case.set_enabled("b", true).await.unwrap();
        use_case.set_enabled("b", true).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_skills, ["a", "b"]);
        use_case.set_enabled("a", false).await.unwrap();
        assert_eq!(state.lock().unwrap().enabled_skills, ["b"]);
        assert_eq!(*saves.lock().unwrap(), 2, "the repeated on does not save again");
    }

    #[tokio::test]
    async fn a_folder_that_is_added_is_switched_on_and_described() {
        let mut port = MockSkillCatalogPort::new();
        port.expect_add_from_folder().returning(|_| Ok(PathBuf::from("/home/.claude/skills/pdf")));
        let (config, state, _) = memory_config(&[]);

        let added = use_case(port, config).add_folder(Path::new("/downloads/pdf")).await.unwrap();

        assert_eq!(added, AddedSkill { name: "pdf".into(), id: "user:pdf".into(), path: "/home/.claude/skills/pdf".into() });
        assert_eq!(state.lock().unwrap().enabled_skills, ["pdf"]);
    }

    #[tokio::test]
    async fn an_add_that_fails_changes_nothing() {
        let mut port = MockSkillCatalogPort::new();
        port.expect_add_from_zip().returning(|_| Err(SkillCatalogError::Exists("pdf".into())));
        let (config, state, saves) = memory_config(&["x"]);
        let error = use_case(port, config).add_zip(Path::new("/d/pdf.zip")).await.unwrap_err();
        assert_eq!(error.to_string(), "A skill named pdf already exists.");
        assert_eq!(state.lock().unwrap().enabled_skills, ["x"]);
        assert_eq!(*saves.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn create_passes_the_text_on_and_switches_the_skill_on() {
        let mut port = MockSkillCatalogPort::new();
        port.expect_create()
            .withf(|name, description| name == "my-skill" && description == "Does a thing")
            .returning(|_, _| Ok(PathBuf::from("/h/skills/my-skill")));
        let (config, state, _) = memory_config(&[]);
        let added = use_case(port, config).create("my-skill", "Does a thing").await.unwrap();
        assert_eq!(added.id, "user:my-skill");
        assert_eq!(state.lock().unwrap().enabled_skills, ["my-skill"]);
    }
}
```

In `use_cases/mod.rs` add `mod skill_catalog;` and `pub use skill_catalog::{AddedSkill, SkillCatalogUseCase, SkillView, SkillsError};`. In `forge-application/src/lib.rs` extend the existing `pub use use_cases::{...}` line with `AddedSkill, SkillCatalogUseCase, SkillView, SkillsError`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p forge-application skill_catalog`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

Insert above the test module:

```rust
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::config_lock::ConfigLock;
use super::plugin_catalog::CatalogView;

use forge_domain::{skill_id, AppConfig, SkillFile, SkillOrigin, SkillSnapshot};
use forge_ports::{ConfigError, ConfigRepositoryPort, SkillCatalogError, SkillCatalogPort};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SkillsError {
    #[error("{0}")]
    Catalog(#[from] SkillCatalogError),
    #[error("Could not read or save the Forge settings: {0}")]
    Config(#[from] ConfigError),
}

/// What the Skills tab draws: the skills, and what Forge has switched on.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillView {
    pub snapshot: SkillSnapshot,
    /// Names of user and project skills that are on.
    pub enabled_skills: Vec<String>,
    /// Ids of plugins that are on; a plugin skill is on when its plugin is.
    pub enabled_plugins: Vec<String>,
    /// Whether any marketplace was added (Explore has something to look at).
    pub has_marketplaces: bool,
}

/// A skill that was just added or created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedSkill {
    pub name: String,
    /// `skill_id` of the user skill: what the page opens.
    pub id: String,
    pub path: PathBuf,
}

/// Finding, adding and switching skills. The files belong to the port; whether Forge's sessions use
/// a user or project skill is Forge's own `enabled_skills` list, which this keeps in step.
pub struct SkillCatalogUseCase {
    port: Arc<dyn SkillCatalogPort>,
    config: Arc<dyn ConfigRepositoryPort>,
    lock: ConfigLock,
}

impl SkillCatalogUseCase {
    pub fn new(port: Arc<dyn SkillCatalogPort>, config: Arc<dyn ConfigRepositoryPort>) -> Self {
        Self { port, config, lock: ConfigLock::default() }
    }

    /// Shares the lock the other config writers hold (see `ConfigLock`).
    pub fn with_config_lock(mut self, lock: ConfigLock) -> Self {
        self.lock = lock;
        self
    }

    /// The skills for the page. `plugins` is the plugin catalogue the page has just read, so the
    /// bundled `claude` is not asked twice.
    pub async fn skills(&self, project: Option<&Path>, plugins: &CatalogView) -> Result<SkillView, SkillsError> {
        let snapshot = self.port.snapshot(project.map(Path::to_path_buf), plugins.snapshot.plugins.clone()).await?;
        let enabled_skills = self.config.load().await?.enabled_skills;
        Ok(SkillView {
            snapshot,
            enabled_skills,
            enabled_plugins: plugins.enabled.clone(),
            has_marketplaces: !plugins.snapshot.marketplaces.is_empty(),
        })
    }

    /// Switches a user or project skill on or off in Forge.
    pub async fn set_enabled(&self, name: &str, on: bool) -> Result<(), SkillsError> {
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

    pub async fn add_folder(&self, source: &Path) -> Result<AddedSkill, SkillsError> {
        let path = self.port.add_from_folder(source).await?;
        self.added(path).await
    }

    pub async fn add_zip(&self, source: &Path) -> Result<AddedSkill, SkillsError> {
        let path = self.port.add_from_zip(source).await?;
        self.added(path).await
    }

    pub async fn create(&self, name: &str, description: &str) -> Result<AddedSkill, SkillsError> {
        let path = self.port.create(name, description).await?;
        self.added(path).await
    }

    pub async fn files(&self, skill_path: &Path) -> Result<Vec<SkillFile>, SkillsError> {
        Ok(self.port.list_files(skill_path).await?)
    }

    pub async fn read(&self, skill_path: &Path, relative: &str) -> Result<String, SkillsError> {
        Ok(self.port.read_file(skill_path, relative).await?)
    }

    // The folder is named after the skill, so its last part is the name to switch on.
    async fn added(&self, path: PathBuf) -> Result<AddedSkill, SkillsError> {
        let name = path.file_name().map(|part| part.to_string_lossy().into_owned()).unwrap_or_default();
        self.set_enabled(&name, true).await?;
        Ok(AddedSkill { id: skill_id(&SkillOrigin::User, &name), name, path })
    }

    // Loads, edits and saves the list; saves only when it changed.
    async fn edit_enabled(&self, edit: impl FnOnce(&mut Vec<String>)) -> Result<(), SkillsError> {
        let _held = self.lock.hold().await;
        let mut config: AppConfig = self.config.load().await?;
        let before = config.enabled_skills.clone();
        edit(&mut config.enabled_skills);
        if config.enabled_skills != before {
            self.config.save(&config).await?;
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p forge-application skill_catalog`
Expected: PASS (6 tests). If `config_lock` is not a sibling module path, copy the `use` lines from the top of `plugin_catalog.rs`.

- [ ] **Step 5: Wire it**

In `crates/forge-features/src/workspace.rs` add to `WorkspaceServices` (after `plugin_catalog`) `pub skill_catalog: Arc<SkillCatalogUseCase>,` and add `SkillCatalogUseCase` to the existing `forge_application` import there.

In `crates/forge-app/src/main.rs`, after the `plugin_catalog` binding add:

```rust
    let skill_catalog = Arc::new(
        SkillCatalogUseCase::new(Arc::new(ClaudeSkillCatalogAdapter::new(claude_home.clone())), config.clone())
            .with_config_lock(config_lock.clone()),
    );
```

and `skill_catalog,` after `plugin_catalog,` in the `WorkspaceServices { ... }` literal. Add `SkillCatalogUseCase` and `ClaudeSkillCatalogAdapter` to the existing imports from `forge_application` and `forge_adapter_claude_fs`.

Run: `cargo build --workspace`
Expected: builds. Fix any other place that builds a `WorkspaceServices` (the compiler lists it).

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: SkillCatalogUseCase and its wiring (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Pure page logic

**Files:**
- Create: `crates/forge-features/src/customize_skills.rs`
- Modify: `crates/forge-features/src/lib.rs` (`mod customize_skills;` and the `pub use`)

**Interfaces:**
- Consumes: `forge_application::SkillView`, domain types.
- Produces (all `pub`, re-exported from `forge_features`):
  - `struct SkillRow { entry: SkillEntry, enabled: bool }` with `fn switchable(&self) -> bool`.
  - `struct SkillGroups { mine: Vec<SkillRow>, from_plugins: Vec<SkillRow> }`.
  - `fn skill_groups(view: &SkillView, query: &str) -> SkillGroups` (installed skills only).
  - `fn explore_skills(view: &SkillView, query: &str) -> Vec<SkillRow>` (skills of uninstalled plugins only).
  - `fn find_skill(view: &SkillView, id: &str) -> Option<SkillRow>`.
  - `fn origin_label(origin: &SkillOrigin) -> String`: `"You"`, `"Project"`, or the plugin's name.
  - `fn file_depth(relative: &str) -> usize`, `fn file_leaf(relative: &str) -> &str`, `fn is_markdown_file(relative: &str) -> bool`.
  - `fn body_after_front_matter(text: &str) -> &str`.
  - `enum SkillPane { Overview, Files }`, `enum SkillText { Empty, Loading, Ready(String), Failed(String) }`.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-features/src/customize_skills.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::SkillSnapshot;
    use std::path::PathBuf;

    fn skill(id: &str, name: &str, origin: SkillOrigin, installed: bool) -> SkillEntry {
        SkillEntry { id: id.into(), name: name.into(), description: format!("about {name}"), origin, installed, path: PathBuf::from("/s") }
    }

    fn plugin(id: &str) -> SkillOrigin {
        SkillOrigin::Plugin { plugin_id: id.into() }
    }

    fn view() -> SkillView {
        SkillView {
            snapshot: SkillSnapshot {
                skills: vec![
                    skill("user:greet", "greet", SkillOrigin::User, true),
                    skill("project:lint", "lint", SkillOrigin::Project, true),
                    skill("plugin:hello@m:greet", "greet", plugin("hello@m"), true),
                    skill("plugin:second@m:wave", "wave", plugin("second@m"), false),
                ],
            },
            enabled_skills: vec!["greet".into()],
            enabled_plugins: vec!["hello@m".into()],
            has_marketplaces: true,
        }
    }

    #[test]
    fn mine_has_two_groups_and_leaves_out_the_skills_of_uninstalled_plugins() {
        let groups = skill_groups(&view(), "");
        let mine: Vec<_> = groups.mine.iter().map(|r| r.entry.id.as_str()).collect();
        let theirs: Vec<_> = groups.from_plugins.iter().map(|r| r.entry.id.as_str()).collect();
        assert_eq!(mine, vec!["user:greet", "project:lint"]);
        assert_eq!(theirs, vec!["plugin:hello@m:greet"]);
    }

    #[test]
    fn a_user_skill_follows_enabled_skills_and_a_plugin_skill_follows_its_plugin() {
        let groups = skill_groups(&view(), "");
        assert!(groups.mine[0].enabled && groups.mine[0].switchable());
        assert!(!groups.mine[1].enabled, "lint is not in enabled_skills");
        // The plugin's skill is also named `greet`, but its switch is the plugin's.
        assert!(groups.from_plugins[0].enabled && !groups.from_plugins[0].switchable());
        let mut off = view();
        off.enabled_plugins.clear();
        assert!(!skill_groups(&off, "").from_plugins[0].enabled);
    }

    #[test]
    fn explore_lists_only_the_skills_of_plugins_that_are_not_installed() {
        let cards = explore_skills(&view(), "");
        assert_eq!(cards.iter().map(|r| r.entry.id.as_str()).collect::<Vec<_>>(), vec!["plugin:second@m:wave"]);
    }

    #[test]
    fn search_matches_name_description_and_origin_ignoring_case() {
        assert_eq!(skill_groups(&view(), "LINT").mine.len(), 1);
        assert_eq!(skill_groups(&view(), "about greet").mine.len(), 1);
        assert_eq!(skill_groups(&view(), "hello").from_plugins.len(), 1, "the plugin's name is searchable");
        assert!(skill_groups(&view(), "nothing like this").mine.is_empty());
        assert_eq!(explore_skills(&view(), "wave").len(), 1);
    }

    #[test]
    fn find_skill_finds_by_id_in_any_installed_state() {
        assert_eq!(find_skill(&view(), "plugin:second@m:wave").unwrap().entry.name, "wave");
        assert!(find_skill(&view(), "user:missing").is_none());
    }

    #[test]
    fn the_origin_label_names_the_plugin_without_its_marketplace() {
        assert_eq!(origin_label(&SkillOrigin::User), "You");
        assert_eq!(origin_label(&SkillOrigin::Project), "Project");
        assert_eq!(origin_label(&plugin("hello@m")), "hello");
    }

    #[test]
    fn file_helpers_read_the_slash_path() {
        assert_eq!(file_depth("SKILL.md"), 0);
        assert_eq!(file_depth("a/b/c.md"), 2);
        assert_eq!(file_leaf("a/b/c.md"), "c.md");
        assert_eq!(file_leaf("SKILL.md"), "SKILL.md");
        assert!(is_markdown_file("a/README.MD"));
        assert!(!is_markdown_file("run.sh"));
    }

    #[test]
    fn the_rendered_view_drops_the_front_matter() {
        let text = "---\nname: pdf\ndescription: d\n---\n\n# Title\n\nBody\n";
        assert_eq!(body_after_front_matter(text), "# Title\n\nBody\n");
        assert_eq!(body_after_front_matter("# No front matter\n"), "# No front matter\n");
        assert_eq!(body_after_front_matter("---\nnever closed\n"), "---\nnever closed\n");
        assert_eq!(body_after_front_matter("---\r\nname: x\r\n---\r\nBody"), "Body");
    }
}
```

In `crates/forge-features/src/lib.rs` add `mod customize_skills;` and

```rust
pub use customize_skills::{
    body_after_front_matter, explore_skills, file_depth, file_leaf, find_skill, is_markdown_file, origin_label, skill_groups,
    SkillGroups, SkillPane, SkillRow, SkillText,
};
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p forge-features customize_skills`
Expected: FAIL to compile.

- [ ] **Step 3: Write the implementation**

Insert above the test module:

```rust
use forge_application::SkillView;
use forge_domain::{split_plugin_id, SkillEntry, SkillOrigin};

/// One skill in a list: the entry and whether it is on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillRow {
    pub entry: SkillEntry,
    pub enabled: bool,
}

impl SkillRow {
    /// A plugin skill has no switch of its own: it is on when its plugin is.
    pub fn switchable(&self) -> bool {
        !self.entry.origin.is_plugin()
    }
}

/// The Mine list: skills made by the person (user and project folders), and skills of plugins.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SkillGroups {
    pub mine: Vec<SkillRow>,
    pub from_plugins: Vec<SkillRow>,
}

/// Which tab of a skill's detail is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillPane {
    Overview,
    Files,
}

/// The text of the file the Files tab shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillText {
    Empty,
    Loading,
    Ready(String),
    Failed(String),
}

/// `You`, `Project`, or the plugin's name.
pub fn origin_label(origin: &SkillOrigin) -> String {
    match origin {
        SkillOrigin::User => "You".to_string(),
        SkillOrigin::Project => "Project".to_string(),
        SkillOrigin::Plugin { plugin_id } => split_plugin_id(plugin_id).0.to_string(),
    }
}

fn row(view: &SkillView, entry: &SkillEntry) -> SkillRow {
    let enabled = match &entry.origin {
        SkillOrigin::Plugin { plugin_id } => view.enabled_plugins.contains(plugin_id),
        _ => view.enabled_skills.contains(&entry.name),
    };
    SkillRow { entry: entry.clone(), enabled }
}

fn matches(entry: &SkillEntry, query: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty()
        || [entry.name.as_str(), entry.description.as_str(), origin_label(&entry.origin).as_str()]
            .iter()
            .any(|text| text.to_lowercase().contains(&query))
}

/// The Mine groups: installed skills only, matching `query`.
pub fn skill_groups(view: &SkillView, query: &str) -> SkillGroups {
    let mut groups = SkillGroups::default();
    for entry in view.snapshot.skills.iter().filter(|entry| entry.installed && matches(entry, query)) {
        let row = row(view, entry);
        if entry.origin.is_plugin() {
            groups.from_plugins.push(row);
        } else {
            groups.mine.push(row);
        }
    }
    groups
}

/// The Explore cards: skills of plugins a marketplace offers and the person has not installed.
pub fn explore_skills(view: &SkillView, query: &str) -> Vec<SkillRow> {
    view.snapshot.skills.iter().filter(|entry| !entry.installed && matches(entry, query)).map(|entry| row(view, entry)).collect()
}

pub fn find_skill(view: &SkillView, id: &str) -> Option<SkillRow> {
    view.snapshot.skills.iter().find(|entry| entry.id == id).map(|entry| row(view, entry))
}

/// How many folders deep a `/` path is.
pub fn file_depth(relative: &str) -> usize {
    relative.matches('/').count()
}

/// The last part of a `/` path.
pub fn file_leaf(relative: &str) -> &str {
    relative.rsplit('/').next().unwrap_or(relative)
}

pub fn is_markdown_file(relative: &str) -> bool {
    relative.to_lowercase().ends_with(".md")
}

/// `text` without a leading `---` ... `---` front matter block; unchanged when there is none or it never closes.
pub fn body_after_front_matter(text: &str) -> &str {
    let rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = rest.split_inclusive('\n');
    let mut offset = match lines.next() {
        Some(first) if first.trim_end() == "---" => first.len(),
        _ => return text,
    };
    for line in lines {
        offset += line.len();
        if line.trim_end() == "---" {
            return rest[offset..].trim_start_matches(['\r', '\n']);
        }
    }
    text
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p forge-features customize_skills`
Expected: PASS (8 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-features
git commit -m "feat: pure logic of the Skills tab (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: Page state

**Files:**
- Modify: `crates/forge-features/src/customize_state.rs`, `crates/forge-features/src/lib.rs` (nothing new to export except `Dialog` already exported)

**Interfaces:**
- Consumes: Tasks 6 and 7, `WorkspaceServices.skill_catalog`, `.folder_picker`, `.file_picker`.
- Produces on `Customize`:
  - `open_page(&self, project: Option<PathBuf>)` (signature changes; the project is what the page's skills are read for).
  - `skills(&self) -> Option<SkillView>`
  - `set_skill_enabled(&self, name: String, on: bool)`
  - `upload_skill_folder(&self)`, `upload_skill_zip(&self)`
  - `create_skill(&self, name: &str, description: &str)`
  - `skills_busy(&self) -> bool`
  - `open_skill(&self, id: String, path: PathBuf)`; `skill_pane() -> SkillPane`, `set_skill_pane(SkillPane)`
  - `skill_files() -> Vec<SkillFile>`, `skill_file() -> Option<String>`, `select_skill_file(&self, relative: String)`, `skill_text() -> SkillText`
  - `skill_source() -> bool`, `set_skill_source(bool)`
  - `Dialog::CreateSkill`.
  - `reload()` now also reads the skills.

State has no unit tests of its own in 30A (signals need a runtime); the logic it relies on is tested in Tasks 6 and 7, and the whole is checked in Task 10. Build and clippy are the gate here.

- [ ] **Step 1: Add the dialog variant and the fields**

In `customize_state.rs` add to the imports:

```rust
use std::path::PathBuf;

use forge_application::{PluginCatalogUseCase, SkillCatalogUseCase, SkillView, SkillsError};
use forge_domain::SkillFile;
use forge_ports::{FilePickerPort, FolderPickerPort};

use crate::customize_skills::{SkillPane, SkillText};
```

(merge with the existing `forge_application` import line, which already names `CatalogView, PluginCatalogUseCase`.) Add `CreateSkill,` to `enum Dialog`. Add these fields to `struct Customize` (all `Signal`s):

```rust
    skills_usecase: Signal<Arc<SkillCatalogUseCase>>,
    folder_picker: Signal<Arc<dyn FolderPickerPort>>,
    file_picker: Signal<Arc<dyn FilePickerPort>>,
    project: Signal<Option<PathBuf>>,
    skills: Signal<Option<SkillView>>,
    skills_busy: Signal<bool>,
    skill_dir: Signal<Option<PathBuf>>,
    skill_pane: Signal<SkillPane>,
    skill_files: Signal<Vec<SkillFile>>,
    skill_file: Signal<Option<String>>,
    skill_text: Signal<SkillText>,
    skill_source: Signal<bool>,
    file_reads: Signal<Latest>,
```

and initialise them in `use_customize_provider` inside the struct literal, each with `Signal::new_in_scope(..., ScopeId::ROOT)`:

```rust
        skills_usecase: Signal::new_in_scope(services.skill_catalog.clone(), ScopeId::ROOT),
        folder_picker: Signal::new_in_scope(services.folder_picker.clone(), ScopeId::ROOT),
        file_picker: Signal::new_in_scope(services.file_picker.clone(), ScopeId::ROOT),
        project: Signal::new_in_scope(None, ScopeId::ROOT),
        skills: Signal::new_in_scope(None, ScopeId::ROOT),
        skills_busy: Signal::new_in_scope(false, ScopeId::ROOT),
        skill_dir: Signal::new_in_scope(None, ScopeId::ROOT),
        skill_pane: Signal::new_in_scope(SkillPane::Overview, ScopeId::ROOT),
        skill_files: Signal::new_in_scope(Vec::new(), ScopeId::ROOT),
        skill_file: Signal::new_in_scope(None, ScopeId::ROOT),
        skill_text: Signal::new_in_scope(SkillText::Empty, ScopeId::ROOT),
        skill_source: Signal::new_in_scope(false, ScopeId::ROOT),
        file_reads: Signal::new_in_scope(Latest::default(), ScopeId::ROOT),
```

- [ ] **Step 2: Change `open_page` and `reload`**

Replace `open_page` and `reload`:

```rust
    /// Called when the page appears: back to the list, then read the catalogue. `project` is the
    /// folder whose `.claude/skills` the Skills tab lists as well.
    pub fn open_page(&self, project: Option<PathBuf>) {
        let mut project_signal = self.project;
        project_signal.set(project);
        let mut detail = self.detail;
        detail.set(None);
        let mut dialog = self.dialog;
        dialog.set(None);
        // A message from an earlier visit is not news any more.
        self.dismiss_error();
        let mut view = self.view;
        view.set(Loaded::Loading);
        self.reload();
    }

    /// Reads the catalogue (plugins, then the skills that depend on it) again. Pinned to the root
    /// scope so leaving the page does not cancel it.
    pub fn reload(&self) {
        self.read_all(None);
    }

    // `open` is a skill to show once the lists are fresh (one that was just added).
    fn read_all(&self, open: Option<(String, PathBuf)>) {
        let usecase = (*self.usecase.peek()).clone();
        let skills_usecase = (*self.skills_usecase.peek()).clone();
        let project = self.project.peek().clone();
        let this = *self;
        let mut reads = self.reads;
        let token = reads.write().issue();
        dioxus::core::spawn_forever(async move {
            let loaded = match usecase.catalog().await {
                Ok(view) => Loaded::Ready(view),
                Err(error) => Loaded::Failed(error.to_string()),
            };
            let skills = match &loaded {
                Loaded::Ready(plugins) => Some(skills_usecase.skills(project.as_deref(), plugins).await),
                _ => None,
            };
            // A read that was overtaken by a newer one is dropped, so a stale list never replaces a fresh one.
            if this.reads.peek().is_latest(token) {
                let mut view = this.view;
                view.set(loaded);
                match skills {
                    Some(Ok(skills)) => {
                        let mut slot = this.skills;
                        slot.set(Some(skills));
                    }
                    Some(Err(error)) => {
                        let mut slot = this.error;
                        slot.set(Some(format!("Could not read the skills: {error}")));
                    }
                    None => {}
                }
                if let Some((id, path)) = open {
                    this.open_skill(id, path);
                    this.set_skill_pane(SkillPane::Files);
                }
            }
        });
    }
```

- [ ] **Step 3: Add the skills methods**

Add to `impl Customize`:

```rust
    pub fn skills(&self) -> Option<SkillView> {
        self.skills.read().clone()
    }

    pub fn skills_busy(&self) -> bool {
        *self.skills_busy.read()
    }

    pub fn skill_pane(&self) -> SkillPane {
        *self.skill_pane.read()
    }

    pub fn set_skill_pane(&self, pane: SkillPane) {
        let mut current = self.skill_pane;
        current.set(pane);
    }

    pub fn skill_files(&self) -> Vec<SkillFile> {
        self.skill_files.read().clone()
    }

    pub fn skill_file(&self) -> Option<String> {
        self.skill_file.read().clone()
    }

    pub fn skill_text(&self) -> SkillText {
        self.skill_text.read().clone()
    }

    pub fn skill_source(&self) -> bool {
        *self.skill_source.read()
    }

    pub fn set_skill_source(&self, source: bool) {
        let mut current = self.skill_source;
        current.set(source);
    }

    /// Opens a skill's detail (Overview) and starts reading its files; SKILL.md is selected first.
    pub fn open_skill(&self, id: String, path: PathBuf) {
        let mut detail = self.detail;
        detail.set(Some(id));
        self.set_skill_pane(SkillPane::Overview);
        self.set_skill_source(false);
        let mut dir = self.skill_dir;
        dir.set(Some(path.clone()));
        let mut files = self.skill_files;
        files.set(Vec::new());
        let mut file = self.skill_file;
        file.set(None);
        let mut text = self.skill_text;
        text.set(SkillText::Empty);
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        let mut reads = self.file_reads;
        let token = reads.write().issue();
        dioxus::core::spawn_forever(async move {
            let listed = usecase.files(&path).await;
            if !this.file_reads.peek().is_latest(token) {
                return;
            }
            match listed {
                Ok(listed) => {
                    let has_skill_md = listed.iter().any(|file| file.relative == "SKILL.md");
                    let mut files = this.skill_files;
                    files.set(listed);
                    if has_skill_md {
                        this.select_skill_file("SKILL.md".to_string());
                    }
                }
                Err(error) => {
                    let mut text = this.skill_text;
                    text.set(SkillText::Failed(error.to_string()));
                }
            }
        });
    }

    /// Shows one file of the open skill.
    pub fn select_skill_file(&self, relative: String) {
        let Some(path) = self.skill_dir.peek().clone() else { return };
        let mut file = self.skill_file;
        file.set(Some(relative.clone()));
        let mut text = self.skill_text;
        text.set(SkillText::Loading);
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        let mut reads = self.file_reads;
        let token = reads.write().issue();
        dioxus::core::spawn_forever(async move {
            let result = usecase.read(&path, &relative).await;
            if this.file_reads.peek().is_latest(token) {
                let mut text = this.skill_text;
                text.set(match result {
                    Ok(content) => SkillText::Ready(content),
                    Err(error) => SkillText::Failed(error.to_string()),
                });
            }
        });
    }

    /// Switches a user or project skill on or off. The list is read again afterwards.
    pub fn set_skill_enabled(&self, name: String, on: bool) {
        let usecase = (*self.skills_usecase.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            if let Err(error) = usecase.set_enabled(&name, on).await {
                let mut slot = this.error;
                slot.set(Some(error.to_string()));
            }
            this.reload();
        });
    }

    /// The Upload skill items: the system picker, then the copy. A cancelled picker does nothing.
    pub fn upload_skill_folder(&self) {
        self.upload(false);
    }

    pub fn upload_skill_zip(&self) {
        self.upload(true);
    }

    fn upload(&self, zip: bool) {
        if self.skills_busy() {
            return;
        }
        let mut busy = self.skills_busy;
        busy.set(true);
        let usecase = (*self.skills_usecase.peek()).clone();
        let folder_picker = (*self.folder_picker.peek()).clone();
        let file_picker = (*self.file_picker.peek()).clone();
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let outcome = if zip {
                match file_picker.pick_files().await.into_iter().next() {
                    None => None,
                    Some(path) if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("zip")) => Some(usecase.add_zip(&path).await),
                    Some(_) => Some(Err(SkillsError::Catalog(forge_ports::SkillCatalogError::Invalid("Choose a .zip file.".to_string())))),
                }
            } else {
                match folder_picker.pick_folder().await {
                    None => None,
                    Some(path) => Some(usecase.add_folder(&path).await),
                }
            };
            this.skill_added(outcome, false);
        });
    }

    /// The Create dialog's button: writes the template, then opens the new skill on its Files tab.
    pub fn create_skill(&self, name: &str, description: &str) {
        if self.skills_busy() {
            return;
        }
        let mut busy = self.skills_busy;
        busy.set(true);
        let mut dialog_error = self.dialog_error;
        dialog_error.set(None);
        let usecase = (*self.skills_usecase.peek()).clone();
        let (name, description) = (name.to_string(), description.to_string());
        let this = *self;
        dioxus::core::spawn_forever(async move {
            let outcome = Some(usecase.create(&name, &description).await);
            this.skill_added(outcome, true);
        });
    }

    // The end of an upload or a create. A success shows the new skill; an error goes where the
    // person is looking (the Create dialog's line, else the page bar).
    fn skill_added(&self, outcome: Option<Result<forge_application::AddedSkill, SkillsError>>, from_dialog: bool) {
        let mut busy = self.skills_busy;
        busy.set(false);
        match outcome {
            None => {}
            Some(Ok(added)) => {
                if self.dialog() == Some(Dialog::CreateSkill) {
                    self.close_dialog();
                }
                let mut tab = self.tab;
                tab.set(crate::customize::CustomizeTab::Skills);
                let mut mode = self.mode;
                mode.set(crate::customize::CustomizeMode::Mine);
                // The new skill opens on its Files tab, once the lists are fresh.
                self.read_all(Some((added.id, added.path)));
            }
            Some(Err(error)) => {
                let place = if from_dialog && self.dialog() == Some(Dialog::CreateSkill) { self.dialog_error } else { self.error };
                let mut place = place;
                place.set(Some(error.to_string()));
            }
        }
    }
```

`set_tab` and `set_mode` in this file also call `close_detail()`. Make `close_detail` clear the open skill's reads too: replace its body with

```rust
        let mut detail = self.detail;
        detail.set(None);
        // A file read that is still on its way has no screen to land on.
        let mut reads = self.file_reads;
        reads.write().issue();
```

- [ ] **Step 4: Build and lint**

Update the one caller of `open_page` (Task 9 changes the page; for now pass `None` in `customize_page.rs`: `customize.open_page(None)`).

Run: `cargo build --workspace && cargo clippy -p forge-features -p forge-application -p forge-adapter-claude-fs -p forge-ports -p forge-domain -- -D warnings`
Expected: builds, no warnings. Fix unused imports the compiler names. `cargo test -p forge-features` must still pass.

- [ ] **Step 5: Commit**

```bash
git add crates
git commit -m "feat: Customize state for skills: read, open, switch, upload, create (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: The Skills tab screen

**Files:**
- Create: `crates/forge-widgets/src/customize_skills.rs`
- Modify: `crates/forge-widgets/src/lib.rs` (`mod customize_skills;`), `crates/forge-widgets/src/customize_page.rs`, `crates/forge-widgets/src/customize_dialogs.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Tasks 7 and 8, `forge_entities::render_markdown`, `Workspace::reveal(&Path)`, `Workspace::context_project()`.
- Produces: `SkillsTab` component; the page shows it for `CustomizeTab::Skills`; the Add menu differs by tab; the Create dialog.

- [ ] **Step 1: The Skills tab widget**

Create `crates/forge-widgets/src/customize_skills.rs`:

```rust
use dioxus::prelude::*;
use forge_entities::render_markdown;
use forge_features::{
    body_after_front_matter, explore_skills, file_depth, file_leaf, find_skill, is_markdown_file, origin_label, skill_groups,
    use_customize, use_workspace, Busy, CustomizeMode, Dialog, Job, Loaded, SkillPane, SkillRow, SkillText,
};
use forge_shared::{Button, ButtonKind};

#[component]
pub fn SkillsTab() -> Element {
    let customize = use_customize();
    let Some(view) = customize.skills() else {
        return match customize.view() {
            Loaded::Failed(message) => rsx! {
                div { class: "status-line error", "{message}" }
                Button { label: "Try again", onclick: move |_| customize.reload() }
            },
            _ => rsx! { div { class: "hint", "Loading…" } },
        };
    };
    if let Some(id) = customize.detail() {
        return match find_skill(&view, &id) {
            Some(row) => rsx! { SkillDetail { row } },
            None => rsx! {
                div { class: "hint",
                    "That skill is no longer there. "
                    Button { label: "Back", onclick: move |_| customize.close_detail() }
                }
            },
        };
    }
    let query = customize.query();
    match customize.mode() {
        CustomizeMode::Mine => {
            let groups = skill_groups(&view, &query);
            if groups.mine.is_empty() && groups.from_plugins.is_empty() {
                let text = if query.trim().is_empty() { "No skills yet. Use + Add to upload or create one." } else { "Nothing matches." };
                return rsx! { div { class: "hint", "{text}" } };
            }
            rsx! {
                if !groups.mine.is_empty() { SkillGroup { title: "Made by me", rows: groups.mine } }
                if !groups.from_plugins.is_empty() { SkillGroup { title: "From plugins", rows: groups.from_plugins } }
            }
        }
        CustomizeMode::Explore => {
            if !view.has_marketplaces {
                return rsx! {
                    div { class: "customize-note",
                        p { "Add a marketplace in the Plugins tab to find skills." }
                        Button { label: "Add marketplace", kind: ButtonKind::Primary, onclick: move |_| customize.open_dialog(Dialog::AddMarketplace) }
                    }
                };
            }
            let cards = explore_skills(&view, &query);
            if cards.is_empty() {
                return rsx! {
                    div { class: "hint",
                        "No skills to find in the marketplaces you added. Skills of a plugin that lives in another Git repository show after the plugin is installed."
                    }
                };
            }
            rsx! {
                div { class: "customize-grid",
                    for row in cards {
                        SkillCard { key: "{row.entry.id}", row }
                    }
                }
            }
        }
    }
}

#[component]
fn SkillGroup(title: String, rows: Vec<SkillRow>) -> Element {
    let customize = use_customize();
    rsx! {
        div { class: "customize-group",
            h2 { "{title} " span { class: "count", "{rows.len()}" } }
            for row in rows {
                {
                    let (id, path) = (row.entry.id.clone(), row.entry.path.clone());
                    let (name, on) = (row.entry.name.clone(), row.enabled);
                    rsx! {
                        div {
                            key: "{id}",
                            class: "customize-row",
                            onclick: move |_| customize.open_skill(id.clone(), path.clone()),
                            div { class: "customize-icon", "✦" }
                            div { class: "customize-main",
                                div { class: "customize-name", "{row.entry.name}" span { class: "tag", "{origin_label(&row.entry.origin)}" } }
                                div { class: "customize-desc", "{row.entry.description}" }
                            }
                            button {
                                class: if on { "switch on" } else { "switch" },
                                disabled: !row.switchable(),
                                title: if row.switchable() { if on { "Turn off" } else { "Turn on" } } else { "Turned on and off with its plugin" },
                                onclick: move |event| {
                                    event.stop_propagation();
                                    customize.set_skill_enabled(name.clone(), !on);
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
fn SkillCard(row: SkillRow) -> Element {
    let customize = use_customize();
    let plugin_id = row.entry.origin.plugin_id().unwrap_or_default().to_string();
    let busy = customize.busy(&plugin_id);
    let (id, path) = (row.entry.id.clone(), row.entry.path.clone());
    let install = plugin_id.clone();
    rsx! {
        div { class: "customize-card", onclick: move |_| customize.open_skill(id.clone(), path.clone()),
            div { class: "customize-icon", "✦" }
            div { class: "customize-main",
                div { class: "customize-name", "{row.entry.name}" }
                div { class: "customize-card-desc", "{row.entry.description}" }
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
fn SkillDetail(row: SkillRow) -> Element {
    let customize = use_customize();
    let pane = customize.skill_pane();
    let plugin_id = row.entry.origin.plugin_id().map(str::to_string);
    let busy = plugin_id.as_deref().map_or(Busy::No, |id| customize.busy(id));
    rsx! {
        div { class: "customize-detail",
            button { class: "customize-back", onclick: move |_| customize.close_detail(), "← Skills" }
            div { class: "customize-detail-head",
                div { class: "customize-icon big", "✦" }
                div {
                    h2 { "{row.entry.name}" }
                    div { class: "hint", "{origin_label(&row.entry.origin)}" }
                }
                div { class: "customize-detail-action",
                    if busy != Busy::No {
                        span { class: "spinner", title: "Working…" }
                    } else if !row.entry.installed {
                        if let Some(id) = plugin_id.clone() {
                            Button { label: "Install plugin", kind: ButtonKind::Primary, onclick: move |_| customize.run(Job::Install(id.clone())) }
                        }
                    }
                }
            }
            div { class: "customize-tabs",
                button {
                    class: if pane == SkillPane::Overview { "customize-tab active" } else { "customize-tab" },
                    onclick: move |_| customize.set_skill_pane(SkillPane::Overview),
                    "Overview"
                }
                button {
                    class: if pane == SkillPane::Files { "customize-tab active" } else { "customize-tab" },
                    onclick: move |_| customize.set_skill_pane(SkillPane::Files),
                    "Files"
                }
            }
            match pane {
                SkillPane::Overview => rsx! {
                    div { class: "customize-section", div { class: "label", "Description" } p { "{row.entry.description}" } }
                    div { class: "customize-section", div { class: "label", "Made by" } p { "{origin_label(&row.entry.origin)}" } }
                    if let Some(id) = plugin_id.clone() {
                        div { class: "customize-section", div { class: "label", "Plugin" } p { "{id}" } }
                    }
                    div { class: "customize-section", div { class: "label", "Folder" } p { class: "mono", "{row.entry.path.display()}" } }
                },
                SkillPane::Files => rsx! { SkillFiles { folder: row.entry.path.clone() } },
            }
        }
    }
}

#[component]
fn SkillFiles(folder: std::path::PathBuf) -> Element {
    let customize = use_customize();
    let workspace = use_workspace();
    let files = customize.skill_files();
    let selected = customize.skill_file();
    let source = customize.skill_source();
    rsx! {
        div { class: "skill-files-bar",
            Button { label: "Show in folder", onclick: move |_| workspace.reveal(&folder) }
        }
        div { class: "skill-files",
            div { class: "skill-tree",
                for file in files {
                    {
                        let relative = file.relative.clone();
                        let depth = file_depth(&relative);
                        let active = selected.as_deref() == Some(relative.as_str());
                        rsx! {
                            button {
                                key: "{relative}",
                                class: if active { "skill-tree-row active" } else { "skill-tree-row" },
                                style: "padding-left: {8 + depth * 14}px",
                                disabled: file.is_dir,
                                onclick: move |_| customize.select_skill_file(relative.clone()),
                                if file.is_dir { "▸ " }
                                "{file_leaf(&file.relative)}"
                            }
                        }
                    }
                }
            }
            div { class: "skill-view",
                if let Some(path) = selected.clone() {
                    div { class: "skill-view-head",
                        span { class: "mono", "{path}" }
                        if is_markdown_file(&path) {
                            div { class: "customize-switch",
                                button { class: if !source { "active" } else { "" }, onclick: move |_| customize.set_skill_source(false), "Rendered" }
                                button { class: if source { "active" } else { "" }, onclick: move |_| customize.set_skill_source(true), "Source" }
                            }
                        }
                    }
                }
                match customize.skill_text() {
                    SkillText::Empty => rsx! { div { class: "hint", "Choose a file." } },
                    SkillText::Loading => rsx! { div { class: "hint", "Loading…" } },
                    SkillText::Failed(message) => rsx! { div { class: "status-line error", "{message}" } },
                    SkillText::Ready(text) => {
                        let rendered = !source && selected.as_deref().is_some_and(is_markdown_file);
                        if rendered {
                            let html = render_markdown(body_after_front_matter(&text));
                            rsx! { div { class: "file-viewer file-markdown markdown", dangerous_inner_html: "{html}" } }
                        } else {
                            rsx! { pre { class: "file-code", "{text}" } }
                        }
                    }
                }
            }
        }
    }
}
```

In `crates/forge-widgets/src/lib.rs` add `mod customize_skills;` next to `mod customize_plugins;`. Check that `forge_features` exports `Job`, `Busy`, `use_workspace` (it does: see lib.rs) and that `forge-widgets` has `forge-entities` as a dependency (it does: `files_kinds.rs` uses it).

- [ ] **Step 2: The page**

In `customize_page.rs`:

1. Imports: add `use_workspace` to the `forge_features` import and `use crate::customize_skills::SkillsTab;`.
2. Replace `use_hook(move || customize.open_page());` with

```rust
    let workspace = use_workspace();
    use_hook(move || customize.open_page(workspace.context_project()));
```

3. Replace `if tab == CustomizeTab::Plugins {` (the controls block) with `if tab == CustomizeTab::Plugins || tab == CustomizeTab::Skills {`.
4. Replace the two menu `button`s inside `div { class: "customize-menu", ... }` with a per-tab menu:

```rust
                                if tab == CustomizeTab::Skills {
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.upload_skill_folder();
                                        },
                                        "Upload skill folder"
                                    }
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.upload_skill_zip();
                                        },
                                        "Upload skill zip"
                                    }
                                    button {
                                        onclick: move |_| {
                                            add_menu.set(false);
                                            customize.open_dialog(Dialog::CreateSkill);
                                        },
                                        "Create skill"
                                    }
                                } else {
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
```

5. In the `match tab`, replace the Skills arm with `CustomizeTab::Skills => rsx! { SkillsTab {} },`; the `SettingsNote` for Connectors stays (its `what` text still says "Connectors are switched on and off in Settings for now.").
6. Disable the Add button while an upload runs: change `Button { label: "+ Add", kind: ButtonKind::Primary, onclick: ... }` to include `disabled: customize.skills_busy(),`.
7. Update the doc comment of `CustomizePage`: "Plugins and Skills are built; Connectors points at Settings until its own plan."

- [ ] **Step 3: The Create dialog**

In `customize_dialogs.rs` add the arm `Dialog::CreateSkill => rsx! { CreateSkill {} },` to the `match dialog`, and the component:

```rust
#[component]
fn CreateSkill() -> Element {
    let customize = use_customize();
    let mut name = use_signal(String::new);
    let mut description = use_signal(String::new);
    let busy = customize.skills_busy();
    rsx! {
        h2 { "Create skill" }
        p { class: "hint", "Makes a SKILL.md in your skills folder. Letters, numbers, - and _ in the name." }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: "name, for example my-skill",
            value: "{name}",
            disabled: busy,
            oninput: move |event| name.set(event.value()),
        }
        input {
            class: "customize-field",
            r#type: "text",
            placeholder: "what it does and when to use it",
            value: "{description}",
            disabled: busy,
            oninput: move |event| description.set(event.value()),
            onkeydown: move |event| {
                if event.key() == Key::Enter && !busy {
                    customize.create_skill(&name(), &description());
                }
            },
        }
        if let Some(message) = customize.dialog_error() {
            div { class: "status-line error", "{message}" }
        }
        div { class: "customize-dialog-buttons",
            if busy { span { class: "spinner", title: "Creating…" } }
            Button { label: "Cancel", disabled: busy, onclick: move |_| customize.close_dialog() }
            Button { label: "Create", kind: ButtonKind::Primary, disabled: busy, onclick: move |_| customize.create_skill(&name(), &description()) }
        }
    }
}
```

The backdrop click closes any dialog unless an add-marketplace job runs; that is fine for Create (a click on the backdrop while it is busy only closes the dialog; the result still opens the skill).

- [ ] **Step 4: CSS**

Append to the Customize block of `crates/forge-shared/src/theme.css` (after `.customize-check`):

```css
.switch { flex: none; position: relative; width: 36px; height: 20px; padding: 0; border: none; border-radius: 999px; background: var(--border); cursor: pointer; }
.switch .knob { position: absolute; top: 2px; left: 2px; width: 16px; height: 16px; border-radius: 50%; background: var(--bg-card); box-shadow: var(--shadow); transition: left 0.12s; }
.switch.on { background: var(--fg); }
.switch.on .knob { left: 18px; }
.switch:disabled { cursor: not-allowed; opacity: 0.55; }
.skill-files-bar { margin: 12px 0 8px; }
.skill-files { display: grid; grid-template-columns: 220px minmax(0, 1fr); gap: 12px; align-items: start; }
.chat-narrow .skill-files { grid-template-columns: 1fr; }
.skill-tree { display: flex; flex-direction: column; padding: 4px; border: 1px solid var(--border); border-radius: 10px; background: var(--bg-card); max-height: 420px; overflow-y: auto; }
.skill-tree-row { padding: 4px 8px; border: none; border-radius: 6px; background: transparent; color: var(--fg); font: inherit; font-size: 13px; text-align: left; cursor: pointer; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.skill-tree-row:hover:not(:disabled) { background: var(--bg-hover); }
.skill-tree-row.active { background: var(--bg-hover); font-weight: 600; }
.skill-tree-row:disabled { color: var(--fg-muted); cursor: default; }
.skill-view { min-width: 0; border: 1px solid var(--border); border-radius: 10px; background: var(--bg-card); padding: 12px; max-height: 420px; overflow: auto; }
.skill-view-head { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 8px; font-size: 12px; color: var(--fg-muted); }
```

- [ ] **Step 5: Build, lint, run the whole suite**

Run: `cargo build --workspace && cargo clippy --workspace -- -D warnings && cargo test --workspace`
Expected: builds, no warnings, all tests pass. If `Workspace::reveal` has a different receiver or argument type than `workspace.reveal(&folder)`, adjust the call to the signature at `workspace.rs:701`.

- [ ] **Step 6: Commit**

```bash
git add crates
git commit -m "feat: Skills tab with Mine, Explore, detail, upload and create (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 10: Check it in the real window

**Files:**
- Modify: `scripts/harness/add_market_fixture.py`, `scripts/harness/test_add_market_fixture.py`

**Interfaces:**
- Produces: `add_market_fixture.add_uploads(base: Path = /private/tmp/forge-harness-skills) -> Path` writing a skill folder `upload-me/` (with `SKILL.md` named `upload-me` and `notes/extra.md`) and a zip `zipped.zip` holding a folder `zipped/SKILL.md` (name `zipped`). `add()` stays as is: its two plugins already each carry a `greet` skill.

- [ ] **Step 1: Write the failing test**

Read the existing `scripts/harness/test_add_market_fixture.py` first and add, in the same style:

```python
    def test_the_upload_fixture_has_a_skill_folder_and_a_zip(self):
        base = Path(os.path.realpath(tempfile.mkdtemp())) / "uploads"
        market.add_uploads(base)
        folder = base / "upload-me"
        self.assertIn("name: upload-me", (folder / "SKILL.md").read_text())
        self.assertTrue((folder / "notes" / "extra.md").is_file())
        import zipfile

        with zipfile.ZipFile(base / "zipped.zip") as archive:
            self.assertIn("zipped/SKILL.md", archive.namelist())
            self.assertIn("name: zipped", archive.read("zipped/SKILL.md").decode())
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd scripts/harness && python3 -m unittest test_add_market_fixture -v`
Expected: FAIL (`add_uploads` missing).

- [ ] **Step 3: Implement**

Add to `add_market_fixture.py` (before the `__main__` block; `import zipfile` at the top):

```python
UPLOAD_DIR = Path("/private/tmp/forge-harness-skills")


def _skill_text(name: str) -> str:
    return f"---\nname: {name}\ndescription: Fixture skill {name}\n---\n\n# {name}\n\nSay hello from {name}.\n"


def add_uploads(directory: Path = UPLOAD_DIR) -> Path:
    """A skill folder and a skill zip to try Upload skill with."""
    folder = directory / "upload-me"
    (folder / "notes").mkdir(parents=True, exist_ok=True)
    (folder / "SKILL.md").write_text(_skill_text("upload-me"))
    (folder / "notes" / "extra.md").write_text("# Extra\n\nA second file.\n")
    with zipfile.ZipFile(directory / "zipped.zip", "w") as archive:
        archive.writestr("zipped/SKILL.md", _skill_text("zipped"))
    return directory
```

and make the `__main__` block print both: `print(f"marketplace written to {add()}")` then `print(f"upload fixtures written to {add_uploads()}")`.

Run: `cd scripts/harness && python3 -m unittest test_add_market_fixture -v`
Expected: PASS.

- [ ] **Step 4: Look at the real window**

Follow the memory notes `driving-the-forge-app-visually` and `plan-30-customize-plugins` (launch through `scripts/harness/launch.sh` with `FORGE_CLAUDE_HOME` set to a fixture folder and `FORGE_HARNESS_EXTRA=scripts/harness/add_market_fixture.py`; keep the display awake with `caffeinate -d -u`; type one key at a time; check `click.py --image` scale). Put one skill in the fixture home first: `mkdir -p $FORGE_CLAUDE_HOME/skills/mine && printf -- '---\nname: mine\ndescription: My own skill\n---\n\nBody\n' > $FORGE_CLAUDE_HOME/skills/mine/SKILL.md`. Then check and record each line below with a screenshot name:

1. Customize → Skills → Mine shows "Made by me (1)" with `mine` and its switch on; no "From plugins" group yet.
2. Plugins tab → Add marketplace `/private/tmp/forge-harness-market` → Skills → Explore shows two cards `greet` (from `hello@harness-market` and `second@harness-market`). Search `second` leaves one.
3. Click `+` on the `hello` card: a spinner, then the card is gone from Explore; Mine shows "From plugins (1)" with `greet`, tag `hello`, switch on but disabled; its hover title reads "Turned on and off with its plugin". In the Plugins tab turn `hello` off: the skill's switch goes off.
4. Click the `mine` switch: off, and `$FORGE_CLAUDE_HOME`'s Forge config (the file `FORGE_CONFIG_PATH` points at) no longer lists `mine` in `enabled_skills`; on again lists it.
5. Click the `mine` row: detail shows Overview (description, "Made by You", folder); the Files tab lists `SKILL.md`, shows it Rendered without the `---` block, Source shows the whole text; "Show in folder" opens the folder in the system file manager.
6. + Add → Upload skill folder → pick `/private/tmp/forge-harness-skills/upload-me`: the detail of `upload-me` opens on Files with `notes/extra.md` in the tree. Upload the same folder again: the page bar says "A skill named upload-me already exists." and `~/.claude`-fixture files are unchanged.
7. + Add → Upload skill zip → pick `zipped.zip`: the `zipped` skill opens. Pick a non-zip file: "Choose a .zip file."
8. + Add → Create skill → name `my-new`, description `Does a thing`: dialog closes, `my-new` opens on Files showing the template. Create again with name `../x`: the dialog line shows the name rule and stays open.
9. Cancel a picker: nothing changes and the Add button works again.
10. Narrow the window below 700 px: the file tree stacks above the file view, the Explore grid is one column.

Not checked here (say so in the record): Windows paths and the Windows folder picker for zips, a marketplace that needs credentials.

If a step fails, fix the code (with a test where a unit can show it), then repeat the step. Do not mark the task done on a partly failed list.

- [ ] **Step 5: Commit**

```bash
git add scripts/harness
git commit -m "test: harness fixture for skill uploads; Skills tab checked in the real window (Plan 30B)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 11: Record what was built

**Files:**
- Modify: `docs/superpowers/specs/2026-10-08-customize-skills-design.md` (add "As built"), `docs/superpowers/roadmap/README.md` (row 30B), `docs/superpowers/roadmap/backlog.md` (PG-02 line), `~/.claude/projects/-Users-james-Developer-forge/memory/plan-30-customize-plugins.md` and `MEMORY.md`.

- [ ] **Step 1: Write the record**

Append to the spec an "## As built (<date>)" section stating, from what actually happened: the files added, the deviations found while building (the Show in folder change from Task 1, any other), what Task 10 checked and what it did not (Windows, credentials), and the deferred minors found in review. Add a row `| 30B | Customize Skills tab (built) | PG-02 | ... |` to the roadmap table next to the 30A row, and update the PG-02 line of `backlog.md` to say 30B is built and 30C Connectors is left.

- [ ] **Step 2: Update the memory**

Update `plan-30-customize-plugins.md`: 30B built on `plan-30` (commit range, merge state), the lessons found (anything new about the harness, `zip`, pickers), and that 30C Connectors is next. Keep the `MEMORY.md` index line to one line.

- [ ] **Step 3: Commit**

```bash
git add docs
git commit -m "docs: Plan 30B as built

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

## Self-review (done while writing)

**Spec coverage.**
- Domain types and ids: Task 1. Port: Task 2. Adapter scanning three places plus marketplaces: Task 3. Add (folder/zip/create) with limits, zip-slip, symlinks, never-overwrite, cleanup: Task 4. Files list/read with path safety: Task 5. Use case joining `enabled_skills` and the plugin catalogue, switching new skills on, reusing the plugin install for Explore `+`: Task 6 (the install itself is the existing `Job::Install`, wired in Task 9). Session start path untouched: no task touches `SkillRepositoryPort` or `ActiveExtensions::select` (Global Constraints).
- Screen (Mine two groups, search, switch with disabled plugin switch, Explore cards and empty states, detail with Overview/Files, rendered/source, Install button for uninstalled plugin skills, Add menu with upload and create, Create dialog): Tasks 7 to 9.
- Deviation 6 of the spec (Files panel) turned out impossible (the panel only opens files inside a session folder; the page has no session). Task 1 amends the spec to "Show in folder" and Task 9 builds that.
- Testing section: adapter tests Tasks 3 to 5, add tests Task 4, state logic Tasks 6 and 7, real window Task 10.

**Placeholders.** None left: every code step carries its code. The one place that depends on a crate version (`zip` API) says what to adjust and what not to.

**Type consistency.** `SkillCatalogPort::snapshot(Option<PathBuf>, Vec<PluginEntry>)` is the same in Tasks 2, 3 and 6. `add_*`/`create` return `PathBuf` in the port and `AddedSkill` in the use case; the state (Task 8) uses `AddedSkill { id, name, path }` as defined in Task 6. `SkillView` fields used in Task 7 match Task 6. `open_page(Option<PathBuf>)` is changed in Task 8 and its only caller in Task 9 (Task 8 passes `None` to keep the build green).

**Known soft spot.** Task 8 has no unit tests because the state needs a Dioxus runtime; 30A did the same. Its pure parts are tested in Tasks 6 and 7 and the whole in Task 10.
