use forge_domain::{title_matches, ProjectSummary, SearchHit, Snippet};

use crate::arrange::{instant, newest_first};
use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

/// How many sessions the palette lists before anything is typed.
pub const RECENT_LIMIT: usize = 12;

/// One line of the search palette.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    /// What opens it: the open session if it is open, else the past session.
    pub key: EntryKey,
    pub project: ProjectSummary,
    pub title: String,
    pub snippet: Option<Snippet>,
    pub updated_at: Option<String>,
}

fn row_of(project: &ProjectSummary, entry: &SidebarEntry, snippet: Option<Snippet>) -> SearchRow {
    SearchRow {
        key: entry.key.clone(),
        project: project.clone(),
        title: entry.title.clone(),
        snippet,
        updated_at: entry.updated_at.clone(),
    }
}

fn sort_rows(rows: &mut [SearchRow]) {
    rows.sort_by_key(|row| newest_first(instant(row.updated_at.as_deref())));
}

/// The newest sessions, for the palette before a query is typed.
pub fn recent_rows(groups: &[SidebarGroup], limit: usize) -> Vec<SearchRow> {
    let mut rows: Vec<SearchRow> = groups
        .iter()
        .flat_map(|group| group.entries.iter().map(|entry| row_of(&group.project, entry, None)))
        .collect();
    sort_rows(&mut rows);
    rows.truncate(limit);
    rows
}

/// The palette's rows for `query`: every listed session whose title matches, plus the sessions the
/// transcript search found (`hits`), each session once, newest first. A hit of a session the
/// sidebar lists adds its quote to that session's row; a hit the sidebar has not listed yet (a
/// session that appeared since the last reload) gets a row of its own in the folder it names.
pub fn merge_search(groups: &[SidebarGroup], query: &str, hits: &[SearchHit]) -> Vec<SearchRow> {
    let mut rows: Vec<SearchRow> = groups
        .iter()
        .flat_map(|group| {
            group
                .entries
                .iter()
                .filter(|entry| title_matches(&entry.title, query))
                .map(|entry| row_of(&group.project, entry, None))
        })
        .collect();

    for hit in hits {
        let listed = groups.iter().find_map(|group| {
            group
                .entries
                .iter()
                .find(|entry| entry.session_id.as_ref() == Some(&hit.session.id))
                .map(|entry| (&group.project, entry))
        });
        let row = match listed {
            Some((project, entry)) => row_of(project, entry, hit.snippet.clone()),
            None => SearchRow {
                key: EntryKey::Past { project: hit.project.clone(), session: hit.session.id.clone() },
                project: ProjectSummary {
                    path: hit.project.clone(),
                    last_active: hit.session.updated_at.clone(),
                    session_count: 0,
                    exists: true,
                },
                title: hit.session.title.clone().unwrap_or_else(|| "Untitled".to_string()),
                snippet: hit.snippet.clone(),
                updated_at: Some(hit.session.updated_at.clone()),
            },
        };
        match rows.iter_mut().find(|known| known.key.dom_key() == row.key.dom_key()) {
            Some(known) => {
                if known.snippet.is_none() {
                    known.snippet = row.snippet;
                }
            }
            None => rows.push(row),
        }
    }
    sort_rows(&mut rows);
    rows
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_application::LiveSessionId;
    use forge_domain::{ProjectSummary, SearchHit, SessionId, SessionSummary, Snippet};
    use forge_entities::SessionMarker;

    use super::*;
    use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

    fn entry(id: &str, title: &str, updated: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key: EntryKey::Past { project: PathBuf::from("/a"), session: SessionId(id.to_string()) },
            session_id: Some(SessionId(id.to_string())),
            title: title.to_string(),
            updated_at: updated.map(str::to_string),
            created_at: None,
            marker: SessionMarker::Idle,
            archived: false,
            forked_from: None,
        }
    }

    fn groups(entries: Vec<SidebarEntry>) -> Vec<SidebarGroup> {
        vec![SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from("/a"),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }]
    }

    fn hit(id: &str, title: &str, updated: &str, snippet: Option<&str>) -> SearchHit {
        SearchHit {
            project: PathBuf::from("/elsewhere"),
            session: SessionSummary {
                id: SessionId(id.to_string()),
                title: Some(title.to_string()),
                updated_at: updated.to_string(),
                created_at: updated.to_string(),
                forked_from: None,
            },
            snippet: snippet.map(|matched| Snippet {
                before: String::new(),
                matched: matched.to_string(),
                after: String::new(),
            }),
        }
    }

    fn titles(rows: &[SearchRow]) -> Vec<&str> {
        rows.iter().map(|row| row.title.as_str()).collect()
    }

    #[test]
    fn a_title_match_is_a_row_without_a_quote() {
        let sidebar = groups(vec![
            entry("s1", "Debug and identify error cause", Some("2026-10-01T00:00:00Z")),
            entry("s2", "Report writing", Some("2026-10-02T00:00:00Z")),
        ]);

        let rows = merge_search(&sidebar, "ERROR", &[]);

        assert_eq!(titles(&rows), ["Debug and identify error cause"]);
        assert_eq!(rows[0].snippet, None);
        assert_eq!(rows[0].project.path, PathBuf::from("/a"));
    }

    #[test]
    fn a_body_hit_on_a_listed_session_adds_its_quote_to_that_row_once() {
        let sidebar = groups(vec![entry("s1", "Error cause", Some("2026-10-01T00:00:00Z"))]);
        let hits = vec![hit("s1", "Error cause", "2026-10-01T00:00:00Z", Some("error"))];

        let rows = merge_search(&sidebar, "error", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].snippet.as_ref().unwrap().matched, "error");
    }

    #[test]
    fn a_body_hit_whose_title_does_not_match_still_gets_a_row_from_the_sidebar_entry() {
        let sidebar = groups(vec![entry("s1", "Untitled", Some("2026-10-01T00:00:00Z"))]);
        let hits = vec![hit("s1", "Untitled", "2026-10-01T00:00:00Z", Some("borrow"))];

        let rows = merge_search(&sidebar, "borrow", &hits);

        assert_eq!(titles(&rows), ["Untitled"]);
        assert_eq!(rows[0].key, EntryKey::Past { project: PathBuf::from("/a"), session: SessionId("s1".to_string()) });
    }

    #[test]
    fn a_hit_for_an_open_session_opens_the_open_session() {
        let mut open = entry("s1", "Chat", Some("2026-10-01T00:00:00Z"));
        open.key = EntryKey::Live(LiveSessionId(4));
        let sidebar = groups(vec![open]);
        let hits = vec![hit("s1", "Chat", "2026-10-01T00:00:00Z", Some("needle"))];

        let rows = merge_search(&sidebar, "needle", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].key, EntryKey::Live(LiveSessionId(4)));
    }

    #[test]
    fn a_hit_the_sidebar_has_not_listed_yet_becomes_a_row_in_the_hits_own_project() {
        let sidebar = groups(vec![]);
        let hits = vec![hit("fresh", "Fresh", "2026-10-03T00:00:00Z", Some("needle"))];

        let rows = merge_search(&sidebar, "needle", &hits);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].project.path, PathBuf::from("/elsewhere"));
        assert_eq!(
            rows[0].key,
            EntryKey::Past { project: PathBuf::from("/elsewhere"), session: SessionId("fresh".to_string()) }
        );
    }

    #[test]
    fn rows_come_newest_first_and_an_unsaved_session_first_of_all() {
        let mut unsaved = entry("u", "needle unsaved", None);
        unsaved.session_id = None;
        unsaved.key = EntryKey::Live(LiveSessionId(9));
        let sidebar = groups(vec![
            entry("old", "needle old", Some("2026-10-01T00:00:00Z")),
            entry("new", "needle new", Some("2026-10-03T00:00:00Z")),
            unsaved,
        ]);

        let rows = merge_search(&sidebar, "needle", &[]);

        assert_eq!(titles(&rows), ["needle unsaved", "needle new", "needle old"]);
    }

    #[test]
    fn korean_titles_match() {
        let sidebar = groups(vec![entry("s1", "최적의 로컬 LLM 모델", Some("2026-10-01T00:00:00Z"))]);

        assert_eq!(merge_search(&sidebar, "로컬", &[]).len(), 1);
    }

    #[test]
    fn an_empty_query_lists_the_most_recent_sessions() {
        let sidebar = groups(vec![
            entry("a", "one", Some("2026-10-01T00:00:00Z")),
            entry("b", "two", Some("2026-10-03T00:00:00Z")),
            entry("c", "three", Some("2026-10-02T00:00:00Z")),
        ]);

        let rows = recent_rows(&sidebar, 2);

        assert_eq!(titles(&rows), ["two", "three"]);
    }
}
