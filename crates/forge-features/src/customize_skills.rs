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
