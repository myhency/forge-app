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
