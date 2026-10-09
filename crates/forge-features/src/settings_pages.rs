use crate::settings::SettingsSection;

pub struct NavGroup {
    pub title: &'static str,
    pub pages: Vec<(SettingsSection, &'static str)>,
}

pub fn nav_groups() -> Vec<NavGroup> {
    vec![
        NavGroup { title: "Settings", pages: vec![(SettingsSection::General, "General"), (SettingsSection::Code, "Claude Code")] },
        NavGroup { title: "This computer", pages: vec![(SettingsSection::Connection, "Connection"), (SettingsSection::Model, "Model")] },
        NavGroup {
            title: "Customize",
            pages: vec![(SettingsSection::Skills, "Skills"), (SettingsSection::Plugins, "Plugins"), (SettingsSection::McpServers, "MCP servers")],
        },
    ]
}

// The titles of the rows on each page, so a search finds a setting by its name.
fn row_titles(section: SettingsSection) -> &'static [&'static str] {
    match section {
        SettingsSection::General => &[
            "Appearance", "Theme", "Font", "Text size", "Chat width", "Reduce motion", "Notifications", "Reply done", "Scheduled task", "Code notice", "Permission request",
        ],
        SettingsSection::Code => &["Default transcript view", "Output style", "Prompt suggestions", "Allow bypass permissions mode", "Bounce the Dock icon"],
        SettingsSection::Connection => &["Base URL", "API key"],
        SettingsSection::Model => &["Default model", "Other model ID"],
        SettingsSection::Skills | SettingsSection::Plugins | SettingsSection::McpServers => &[],
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
