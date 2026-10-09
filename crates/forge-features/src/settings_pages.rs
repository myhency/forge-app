use crate::settings::SettingsSection;
use forge_shared::tr;

pub struct NavGroup {
    pub title: &'static str,
    pub pages: Vec<(SettingsSection, &'static str)>,
}

pub fn nav_groups() -> Vec<NavGroup> {
    vec![
        NavGroup { title: tr!("Settings"), pages: vec![(SettingsSection::General, tr!("General")), (SettingsSection::Code, tr!("Claude Code"))] },
        NavGroup { title: tr!("This computer"), pages: vec![(SettingsSection::Connection, tr!("Connection")), (SettingsSection::Model, tr!("Model"))] },
        NavGroup {
            title: tr!("Customize"),
            pages: vec![(SettingsSection::Skills, tr!("Skills")), (SettingsSection::Plugins, tr!("Plugins")), (SettingsSection::McpServers, tr!("MCP servers"))],
        },
    ]
}

// The titles of the rows on each page, so a search finds a setting by its name.
fn row_titles(section: SettingsSection) -> Vec<&'static str> {
    match section {
        SettingsSection::General => vec![
            tr!("Appearance"), tr!("Theme"), tr!("Font"), tr!("Text size"), tr!("Chat width"), tr!("Reduce motion"), tr!("Notifications"), tr!("Reply done"),
            tr!("Scheduled task"), tr!("Code notice"), tr!("Permission request"),
        ],
        SettingsSection::Code => vec![tr!("Default transcript view"), tr!("Output style"), tr!("Prompt suggestions"), tr!("Allow bypass permissions mode"), tr!("Bounce the Dock icon")],
        SettingsSection::Connection => vec![tr!("Base URL"), tr!("API key")],
        SettingsSection::Model => vec![tr!("Default model"), tr!("Other model ID")],
        SettingsSection::Skills | SettingsSection::Plugins | SettingsSection::McpServers => vec![],
    }
}

/// The pages whose name or row titles contain `query` (any case), in list order. A blank query keeps all.
pub fn filter_pages(query: &str) -> Vec<SettingsSection> {
    let needle = query.trim().to_lowercase();
    nav_groups()
        .into_iter()
        .flat_map(|group| group.pages)
        .filter(|(section, name)| needle.is_empty() || name.to_lowercase().contains(&needle) || row_titles(*section).iter().any(|title| title.to_lowercase().contains(&needle)))
        .map(|(section, _)| section)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_follow_the_real_app() {
        let groups = nav_groups();
        let titles: Vec<_> = groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, ["Settings", "This computer", "Customize"]);
        assert_eq!(groups[0].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["General", "Claude Code"]);
        assert_eq!(groups[1].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Connection", "Model"]);
        assert_eq!(groups[2].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Skills", "Plugins", "MCP servers"]);
    }

    #[test]
    fn an_empty_search_keeps_every_page() {
        assert_eq!(filter_pages("  ").len(), 7);
    }

    #[test]
    fn a_search_matches_a_page_name_or_one_of_its_row_titles_ignoring_case() {
        assert_eq!(filter_pages("MODEL"), [SettingsSection::Model]);
        assert_eq!(filter_pages("dock"), [SettingsSection::Code]);
        assert_eq!(filter_pages("theme"), [SettingsSection::General]);
        assert_eq!(filter_pages("notif"), [SettingsSection::General]);
    }

    #[test]
    fn a_search_with_no_match_returns_nothing() {
        assert!(filter_pages("zzzz").is_empty());
    }
}
