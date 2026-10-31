use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::{PluginEntry, SkillFileList, SkillSnapshot};
use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

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
    /// The skill's files, shallow ones first, at most 500; the list says when it was cut.
    async fn list_files(&self, skill_path: &Path) -> Result<SkillFileList, SkillCatalogError>;
    /// One text file of the skill; `relative` uses `/`.
    async fn read_file(&self, skill_path: &Path, relative: &str) -> Result<String, SkillCatalogError>;
}

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
