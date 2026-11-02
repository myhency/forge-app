//! The Sessions page (the desktop app's `모두 보기`): every session in one flat list, active or archived,
//! with a title search. Pure functions; the page is `forge-widgets/src/sessions_page.rs`.

use forge_domain::SortBy;
use forge_shared::tr;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::arrange::{sort_entries, ArrangedEntry};
use crate::sidebar::SidebarGroup;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionsTab {
    #[default]
    Active,
    Archived,
}

/// How search results are ordered: the best title match first (`유형`), or newest first (`날짜`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionsSort {
    #[default]
    Match,
    Date,
}

pub const SESSIONS_SORTS: [SessionsSort; 2] = [SessionsSort::Match, SessionsSort::Date];

pub fn sessions_sort_label(sort: SessionsSort) -> &'static str {
    match sort {
        SessionsSort::Match => tr!("Type"),
        SessionsSort::Date => tr!("Date"),
    }
}

/// The sessions of one tab as a flat list (forks are plain rows), newest first; with a `query`, only the
/// titles that hold it, in the order `sort` says. The search reads the titles that are shown, as the
/// desktop app's page says.
pub fn sessions_list(groups: &[SidebarGroup], tab: SessionsTab, query: &str, sort: SessionsSort) -> Vec<ArrangedEntry> {
    let query = query.trim();
    let wants_archived = tab == SessionsTab::Archived;
    let mut rows: Vec<ArrangedEntry> = groups
        .iter()
        .flat_map(|group| {
            group.entries.iter().map(move |entry| ArrangedEntry {
                project: group.project.clone(),
                entry: entry.clone(),
                depth: 0,
                fork_count: 0,
                forks_folded: false,
                hidden_forks: 0,
            })
        })
        .filter(|row| row.entry.archived == wants_archived)
        .filter(|row| query.is_empty() || match_at(&row.entry.title, query).is_some())
        .collect();
    sort_entries(&mut rows, SortBy::Activity, false);
    if !query.is_empty() && sort == SessionsSort::Match {
        // A stable sort: titles that match equally well stay newest first.
        rows.sort_by_key(|row| match_at(&row.entry.title, query).unwrap_or(usize::MAX));
    }
    rows
}

fn same_char(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// Where (in characters) `query` first appears in `title`, ignoring case.
fn match_at(title: &str, query: &str) -> Option<usize> {
    let title: Vec<char> = title.chars().collect();
    let query: Vec<char> = query.chars().collect();
    if query.is_empty() || query.len() > title.len() {
        return None;
    }
    (0..=title.len() - query.len()).find(|start| query.iter().enumerate().all(|(offset, ch)| same_char(title[start + offset], *ch)))
}

/// `title` cut into pieces, each with whether it is a match of `query` (to draw in bold).
pub fn split_match(title: &str, query: &str) -> Vec<(String, bool)> {
    let query = query.trim();
    let chars: Vec<char> = title.chars().collect();
    let wanted: Vec<char> = query.chars().collect();
    if wanted.is_empty() {
        return vec![(title.to_string(), false)];
    }
    let mut out: Vec<(String, bool)> = Vec::new();
    let mut plain = String::new();
    let mut at = 0;
    while at < chars.len() {
        let hit = at + wanted.len() <= chars.len() && wanted.iter().enumerate().all(|(offset, ch)| same_char(chars[at + offset], *ch));
        if hit {
            if !plain.is_empty() {
                out.push((std::mem::take(&mut plain), false));
            }
            out.push((chars[at..at + wanted.len()].iter().collect(), true));
            at += wanted.len();
        } else {
            plain.push(chars[at]);
            at += 1;
        }
    }
    if !plain.is_empty() {
        out.push((plain, false));
    }
    out
}

/// How long ago a session was last active, as the desktop app's page says it: minutes and hours under a
/// day, then yesterday, the day before, `N days ago` up to a week, and after that the date.
pub fn session_when(updated_at: &str, now: OffsetDateTime) -> String {
    let Ok(then) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
        return String::new();
    };
    let seconds = (now - then).whole_seconds();
    if seconds < 60 {
        return tr!("just now").to_string();
    }
    if seconds < 3_600 {
        return tr!("{n}m ago", n = seconds / 60);
    }
    if seconds < 86_400 {
        return tr!("{n}h ago", n = seconds / 3_600);
    }
    // Days are counted on the calendar in `now`'s own offset: 30 hours ago can still be two days back.
    let then = then.to_offset(now.offset());
    let days = (now.date() - then.date()).whole_days();
    match days {
        ..=1 => tr!("Yesterday").to_string(),
        2 => tr!("The day before yesterday").to_string(),
        3..=7 => tr!("{n}d ago", n = days),
        _ if then.year() == now.year() => tr!("{month}/{day}", month = u8::from(then.month()), day = then.day()),
        _ => tr!("{year}/{month}/{day}", year = then.year(), month = u8::from(then.month()), day = then.day()),
    }
}

#[cfg(test)]
mod tests {
    use forge_domain::ProjectSummary;
    use time::format_description::well_known::Rfc3339;

    use super::*;
    use crate::sidebar::{EntryKey, SidebarEntry};
    use forge_domain::SessionId;
    use forge_entities::SessionMarker;

    fn entry(id: &str, title: &str, updated: &str, archived: bool) -> SidebarEntry {
        SidebarEntry {
            key: EntryKey::Past { project: "/p".into(), session: SessionId(id.to_string()) },
            session_id: Some(SessionId(id.to_string())),
            title: title.to_string(),
            updated_at: Some(updated.to_string()),
            created_at: Some(updated.to_string()),
            marker: SessionMarker::Idle,
            archived,
            forked_from: None,
        }
    }

    fn groups() -> Vec<SidebarGroup> {
        vec![SidebarGroup {
            project: ProjectSummary { path: "/p".into(), last_active: String::new(), session_count: 0, exists: true },
            entries: vec![
                entry("a", "Fix the login", "2026-10-01T10:00:00Z", false),
                entry("b", "Login page", "2026-10-02T10:00:00Z", false),
                entry("c", "Old plan", "2026-09-01T10:00:00Z", true),
                entry("d", "Add a login test", "2026-10-03T10:00:00Z", false),
            ],
        }]
    }

    fn titles(rows: &[ArrangedEntry]) -> Vec<&str> {
        rows.iter().map(|row| row.entry.title.as_str()).collect()
    }

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-11-02T12:00:00Z", &Rfc3339).unwrap()
    }

    #[test]
    fn each_tab_lists_its_own_sessions_newest_first() {
        let active = sessions_list(&groups(), SessionsTab::Active, "", SessionsSort::Match);
        assert_eq!(titles(&active), ["Add a login test", "Login page", "Fix the login"]);
        let archived = sessions_list(&groups(), SessionsTab::Archived, "", SessionsSort::Match);
        assert_eq!(titles(&archived), ["Old plan"]);
    }

    #[test]
    fn a_search_keeps_the_titles_that_hold_the_text_ignoring_case() {
        let rows = sessions_list(&groups(), SessionsTab::Active, "  LOGIN ", SessionsSort::Date);
        assert_eq!(titles(&rows), ["Add a login test", "Login page", "Fix the login"]);
        assert!(sessions_list(&groups(), SessionsTab::Active, "nothing", SessionsSort::Date).is_empty());
    }

    #[test]
    fn the_match_order_puts_the_earliest_match_first_and_keeps_newest_first_among_equals() {
        let rows = sessions_list(&groups(), SessionsTab::Active, "login", SessionsSort::Match);
        // "Login page" matches at 0; the others later (index 8 and 6), so "Add a login test" (6) comes before "Fix the login" (8).
        assert_eq!(titles(&rows), ["Login page", "Add a login test", "Fix the login"]);
    }

    #[test]
    fn the_matches_in_a_title_are_marked_for_bold() {
        assert_eq!(
            split_match("Fix the Login and login", "login"),
            [("Fix the ".to_string(), false), ("Login".to_string(), true), (" and ".to_string(), false), ("login".to_string(), true)]
        );
        assert_eq!(split_match("abc", ""), [("abc".to_string(), false)]);
        assert_eq!(split_match("한글 제목", "제목"), [("한글 ".to_string(), false), ("제목".to_string(), true)]);
    }

    #[test]
    fn the_time_reads_like_the_desktop_apps_page() {
        assert_eq!(session_when("2026-11-02T11:59:30Z", now()), "just now");
        assert_eq!(session_when("2026-11-02T11:00:00Z", now()), "1h ago");
        assert_eq!(session_when("2026-11-01T16:00:00Z", now()), "20h ago");
        assert_eq!(session_when("2026-10-31T09:00:00Z", now()), "The day before yesterday");
        assert_eq!(session_when("2026-10-30T09:00:00Z", now()), "3d ago");
        assert_eq!(session_when("2026-10-26T09:00:00Z", now()), "7d ago");
        assert_eq!(session_when("2026-10-22T09:00:00Z", now()), "10/22");
        assert_eq!(session_when("2025-12-01T09:00:00Z", now()), "2025/12/1");
        assert_eq!(session_when("not a time", now()), "");
    }
}
