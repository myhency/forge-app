use crate::settings::SettingsSection;
use forge_shared::tr;

pub struct NavGroup {
    pub title: &'static str,
    pub pages: Vec<(SettingsSection, &'static str)>,
}

pub fn nav_groups() -> Vec<NavGroup> {
    vec![
        NavGroup { title: tr!("Settings"), pages: vec![(SettingsSection::General, tr!("General")), (SettingsSection::Code, tr!("Claude Code"))] },
        NavGroup { title: tr!("This computer"), pages: vec![(SettingsSection::Connection, tr!("Connection")), (SettingsSection::Model, tr!("Model")), (SettingsSection::Developer, tr!("Developer"))] },
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
            tr!("Appearance"), tr!("Theme"), tr!("Font"), tr!("Text size"), tr!("Chat width"), tr!("Reduce motion"), tr!("Work"), tr!("Trusted folders"), tr!("Manage"), tr!("Notifications"), tr!("Reply done"),
            tr!("Scheduled task"), tr!("Code notice"), tr!("Permission request"),
        ],
        SettingsSection::Code => vec![tr!("Default transcript view"), tr!("Archive inactive sessions"), tr!("Worktree location"), tr!("Output style"), tr!("Prompt suggestions"), tr!("Allow bypass permissions mode"), tr!("Bounce the Dock icon")],
        SettingsSection::Connection => vec![tr!("Base URL"), tr!("API key")],
        SettingsSection::Model => vec![tr!("Default model"), tr!("Other model ID")],
        SettingsSection::Developer => vec![tr!("Local MCP servers"), tr!("Edit config"), tr!("View logs")],
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

/// A row title cut around the letters a search matched, so the match can be drawn in bold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Highlighted {
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// Cuts `text` around the first case-insensitive match of `needle`; `None` when it does not contain it. The cut
/// is made on characters, so a Korean or accented title is never split inside a letter.
pub fn highlight(text: &str, needle: &str) -> Option<Highlighted> {
    let needle: Vec<char> = needle.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return None;
    }
    let chars: Vec<char> = text.chars().collect();
    let lowered: Vec<char> = chars.iter().map(|c| c.to_lowercase().next().unwrap_or(*c)).collect();
    let start = (0..=lowered.len().checked_sub(needle.len())?).find(|at| lowered[*at..*at + needle.len()] == needle[..])?;
    let end = start + needle.len();
    Some(Highlighted { before: chars[..start].iter().collect(), matched: chars[start..end].iter().collect(), after: chars[end..].iter().collect() })
}

/// One page of the search result with the rows under it that matched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchHit {
    pub section: SettingsSection,
    pub name: &'static str,
    /// The page name, cut around the match, when the name itself matched.
    pub name_match: Option<Highlighted>,
    /// The titles of the rows that matched, each cut around the match. A page whose name matched lists no rows
    /// unless they match too.
    pub rows: Vec<Highlighted>,
}

/// The pages and rows a search finds, in list order. A blank query returns nothing: the list shows every page.
pub fn search_hits(query: &str) -> Vec<SearchHit> {
    let needle = query.trim();
    if needle.is_empty() {
        return Vec::new();
    }
    nav_groups()
        .into_iter()
        .flat_map(|group| group.pages)
        .filter_map(|(section, name)| {
            let name_match = highlight(name, needle);
            let rows: Vec<Highlighted> = row_titles(section).into_iter().filter_map(|title| highlight(title, needle)).collect();
            (name_match.is_some() || !rows.is_empty()).then_some(SearchHit { section, name, name_match, rows })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_is_cut_around_the_letters_ignoring_case() {
        let cut = highlight("Reduce motion", "MOT").unwrap();
        assert_eq!((cut.before.as_str(), cut.matched.as_str(), cut.after.as_str()), ("Reduce ", "mot", "ion"));
        assert_eq!(highlight("Reduce motion", "zzz"), None);
        assert_eq!(highlight("Reduce motion", "  "), None);
        // A needle longer than the text does not match and does not panic.
        assert_eq!(highlight("ab", "abc"), None);
    }

    #[test]
    fn a_korean_match_is_cut_on_whole_letters() {
        let cut = highlight("테마 설정", "설정").unwrap();
        assert_eq!((cut.before.as_str(), cut.matched.as_str(), cut.after.as_str()), ("테마 ", "설정", ""));
    }

    #[test]
    fn a_blank_search_has_no_hits_and_a_row_match_names_its_page() {
        assert!(search_hits("   ").is_empty());
        let hits = search_hits("trusted");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].section, SettingsSection::General);
        assert_eq!(hits[0].name_match, None);
        assert_eq!(hits[0].rows[0].matched, "Trusted");
    }

    #[test]
    fn a_page_whose_name_matches_comes_first_with_the_name_cut() {
        let hits = search_hits("model");
        let model = hits.iter().find(|hit| hit.section == SettingsSection::Model).unwrap();
        assert_eq!(model.name_match.as_ref().map(|cut| cut.matched.as_str()), Some("Model"));
        // The pages stay in the order of the list.
        let order: Vec<SettingsSection> = hits.iter().map(|hit| hit.section).collect();
        let list: Vec<SettingsSection> = nav_groups().into_iter().flat_map(|group| group.pages).map(|(section, _)| section).filter(|section| order.contains(section)).collect();
        assert_eq!(order, list);
    }

    #[test]
    fn the_hits_cover_the_same_pages_as_the_page_filter() {
        for query in ["model", "dock", "theme", "notif", "logs", "zzzz", "trusted"] {
            let from_hits: Vec<SettingsSection> = search_hits(query).iter().map(|hit| hit.section).collect();
            assert_eq!(from_hits, filter_pages(query), "{query}");
        }
    }

    #[test]
    fn a_search_finds_the_trusted_folders_row_on_the_general_page() {
        assert_eq!(filter_pages("trusted"), [SettingsSection::General]);
        assert_eq!(filter_pages("TRUSTED FOLDERS"), [SettingsSection::General]);
    }

    #[test]
    fn groups_follow_the_real_app() {
        let groups = nav_groups();
        let titles: Vec<_> = groups.iter().map(|group| group.title).collect();
        assert_eq!(titles, ["Settings", "This computer", "Customize"]);
        assert_eq!(groups[0].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["General", "Claude Code"]);
        assert_eq!(groups[1].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Connection", "Model", "Developer"]);
        assert_eq!(groups[2].pages.iter().map(|page| page.1).collect::<Vec<_>>(), ["Skills", "Plugins", "MCP servers"]);
    }

    #[test]
    fn an_empty_search_keeps_every_page() {
        assert_eq!(filter_pages("  ").len(), 8);
    }

    #[test]
    fn a_search_matches_a_page_name_or_one_of_its_row_titles_ignoring_case() {
        assert_eq!(filter_pages("MODEL"), [SettingsSection::Model]);
        assert_eq!(filter_pages("dock"), [SettingsSection::Code]);
        assert_eq!(filter_pages("theme"), [SettingsSection::General]);
        assert_eq!(filter_pages("notif"), [SettingsSection::General]);
        assert_eq!(filter_pages("logs"), [SettingsSection::Developer]);
    }

    #[test]
    fn a_search_with_no_match_returns_nothing() {
        assert!(filter_pages("zzzz").is_empty());
    }
}
