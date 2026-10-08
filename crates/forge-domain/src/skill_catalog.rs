use std::path::PathBuf;

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
