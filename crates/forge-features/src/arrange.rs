use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use forge_application::LiveSessionId;
use forge_domain::{GroupBy, ProjectSummary, SessionId, SidebarPrefs, SortBy, StatusFilter};
use time::format_description::well_known::Rfc3339;
use std::sync::OnceLock;

use time::{OffsetDateTime, UtcOffset};

use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};
use forge_shared::tr;

/// The age buckets of date grouping, newest first (the derived order is the drawing order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DateBucket {
    Today,
    Yesterday,
    Previous7Days,
    Previous30Days,
    Older,
}

impl DateBucket {
    pub fn label(self) -> &'static str {
        match self {
            DateBucket::Today => tr!("Today"),
            DateBucket::Yesterday => tr!("Yesterday"),
            DateBucket::Previous7Days => tr!("Previous 7 days"),
            DateBucket::Previous30Days => tr!("Previous 30 days"),
            DateBucket::Older => tr!("Older"),
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            DateBucket::Today => "today",
            DateBucket::Yesterday => "yesterday",
            DateBucket::Previous7Days => "7d",
            DateBucket::Previous30Days => "30d",
            DateBucket::Older => "older",
        }
    }
}

/// What a group's heading says.
#[derive(Debug, Clone, PartialEq)]
pub enum Heading {
    Project(ProjectSummary),
    Bucket(DateBucket),
    /// No grouping: no heading at all.
    None,
}

/// A row, with the project it belongs to (a row in a date group needs it to open and to show it).
#[derive(Debug, Clone, PartialEq)]
pub struct ArrangedEntry {
    pub project: ProjectSummary,
    pub entry: SidebarEntry,
    /// How many forks deep the row sits: 0 for an ordinary session or a fork whose source is not in
    /// the same list, 1 for a fork drawn under its source (`└`), and so on.
    pub depth: u8,
    /// All forks under this row, shown or folded away; the `▾` is drawn when it is above 0.
    pub fork_count: usize,
    /// The forks under this row are folded away (`▸`).
    pub forks_folded: bool,
    /// How many of those forks the fold took out of the list (the open session's row stays); the
    /// line `└ N nested sessions` under a folded row says this number.
    pub hidden_forks: usize,
}

impl ArrangedEntry {
    fn plain(project: ProjectSummary, entry: SidebarEntry) -> Self {
        Self { project, entry, depth: 0, fork_count: 0, forks_folded: false, hidden_forks: 0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrangedGroup {
    /// Stable between draws; the key of the collapsed set in `SidebarPrefs`.
    pub id: String,
    pub heading: Heading,
    pub collapsed: bool,
    pub entries: Vec<ArrangedEntry>,
}

/// The id in `SidebarPrefs::collapsed` of a session whose forks are folded away.
pub fn fork_fold_id(session: &SessionId) -> String {
    format!("fork:{}", session.0)
}

/// Draws each fork right under the session it was made from, when that session is in the same
/// list (a fork whose source is in another group, or hidden by the status filter, stays a plain
/// row). Forks keep their sorted order among their siblings. A folded session keeps its forks out
/// of sight, except the one on screen. `entries` must already be sorted.
fn nest_forks(entries: Vec<ArrangedEntry>, prefs: &SidebarPrefs, active: Option<LiveSessionId>) -> Vec<ArrangedEntry> {
    let listed: HashSet<SessionId> = entries.iter().filter_map(|item| item.entry.session_id.clone()).collect();
    let source_of = |item: &ArrangedEntry| {
        item.entry
            .forked_from
            .clone()
            .filter(|source| listed.contains(source) && Some(source) != item.entry.session_id.as_ref())
    };
    let mut children: HashMap<SessionId, Vec<usize>> = HashMap::new();
    let mut roots: Vec<usize> = Vec::new();
    for (index, item) in entries.iter().enumerate() {
        match source_of(item) {
            Some(source) => children.entry(source).or_default().push(index),
            None => roots.push(index),
        }
    }
    if children.is_empty() {
        return entries;
    }

    let mut slots: Vec<Option<ArrangedEntry>> = entries.into_iter().map(Some).collect();
    let mut out: Vec<ArrangedEntry> = Vec::with_capacity(slots.len());
    // A row is placed once, so a loop of forks (a damaged transcript) cannot recurse for ever.
    fn place(
        index: usize,
        depth: u8,
        slots: &mut Vec<Option<ArrangedEntry>>,
        children: &HashMap<SessionId, Vec<usize>>,
        prefs: &SidebarPrefs,
        active: Option<LiveSessionId>,
        out: &mut Vec<ArrangedEntry>,
    ) {
        let Some(mut item) = slots[index].take() else {
            return;
        };
        item.depth = depth;
        let own = item.entry.session_id.clone();
        let kids: Vec<usize> = own.as_ref().and_then(|id| children.get(id)).cloned().unwrap_or_default();
        let folded = own.as_ref().is_some_and(|id| prefs.is_collapsed(&fork_fold_id(id)));
        let start = out.len();
        out.push(item);
        for kid in kids {
            place(kid, depth.saturating_add(1), slots, children, prefs, active, out);
        }
        let below = out.len() - start - 1;
        out[start].fork_count = below;
        if folded && below > 0 {
            out[start].forks_folded = true;
            // The row of the session on screen stays, so the person never loses sight of where they are.
            let mut position = 0;
            out.retain(|item| {
                position += 1;
                position <= start + 1
                    || matches!((&item.entry.key, active), (EntryKey::Live(open), Some(shown)) if *open == shown)
            });
            out[start].hidden_forks = below - (out.len() - start - 1);
        }
    }
    for root in roots {
        place(root, 0, &mut slots, &children, prefs, active, &mut out);
    }
    // Rows that only a loop of forks reached are drawn plainly.
    for index in 0..slots.len() {
        place(index, 0, &mut slots, &children, prefs, active, &mut out);
    }
    out
}

pub fn folder_group_id(path: &Path) -> String {
    format!("folder:{}", path.display())
}

fn bucket_group_id(bucket: DateBucket) -> String {
    format!("date:{}", bucket.slug())
}

pub(crate) fn instant(text: Option<&str>) -> Option<OffsetDateTime> {
    text.and_then(|text| OffsetDateTime::parse(text, &Rfc3339).ok())
}

/// A sort key that puts the newest first and a missing time (a session not on disk yet) before
/// everything. Times are compared as instants, not as text: `…03.500Z` is later than `…03Z`.
pub(crate) fn newest_first(at: Option<OffsetDateTime>) -> (bool, Reverse<Option<i128>>) {
    (at.is_some(), Reverse(at.map(|at| at.unix_timestamp_nanos())))
}

fn activity_of(entry: &SidebarEntry) -> Option<OffsetDateTime> {
    instant(entry.updated_at.as_deref())
}

fn created_of(entry: &SidebarEntry) -> Option<OffsetDateTime> {
    instant(entry.created_at.as_deref()).or_else(|| activity_of(entry))
}

// A stable sort: sessions that tie keep the order the adapter gave them (newest first, then id).
fn sort_entries(entries: &mut [ArrangedEntry], sort_by: SortBy) {
    match sort_by {
        SortBy::Name => entries.sort_by_key(|item| item.entry.title.to_lowercase()),
        SortBy::Created => entries.sort_by_key(|item| newest_first(created_of(&item.entry))),
        SortBy::Activity => entries.sort_by_key(|item| newest_first(activity_of(&item.entry))),
    }
}

fn bucket_of(at: Option<OffsetDateTime>, now: OffsetDateTime) -> DateBucket {
    let Some(at) = at else {
        return DateBucket::Today;
    };
    // Both days are read in `now`'s own offset: a morning session in Korea is today, not yesterday.
    let days = (now.date() - at.to_offset(now.offset()).date()).whole_days();
    match days {
        days if days <= 0 => DateBucket::Today,
        1 => DateBucket::Yesterday,
        2..=7 => DateBucket::Previous7Days,
        8..=30 => DateBucket::Previous30Days,
        _ => DateBucket::Older,
    }
}

fn finish(
    id: String,
    heading: Heading,
    mut entries: Vec<ArrangedEntry>,
    prefs: &SidebarPrefs,
    active: Option<LiveSessionId>,
) -> ArrangedGroup {
    let collapsed = prefs.is_collapsed(&id);
    if collapsed {
        // The row of the session on screen stays, so the person never loses sight of where they are.
        entries.retain(|item| matches!((&item.entry.key, active), (EntryKey::Live(open), Some(shown)) if *open == shown));
    }
    ArrangedGroup { id, heading, collapsed, entries }
}

fn all_entries(groups: &[SidebarGroup]) -> Vec<ArrangedEntry> {
    groups
        .iter()
        .flat_map(|group| {
            group
                .entries
                .iter()
                .map(|entry| ArrangedEntry::plain(group.project.clone(), entry.clone()))
        })
        .collect()
}

static LOCAL_OFFSET: OnceLock<UtcOffset> = OnceLock::new();

/// Tells the sidebar which time zone the person is in. Read it once at start-up, before other
/// threads exist (`UtcOffset::current_local_offset` is only sound then); never called, dates are
/// cut in UTC.
pub fn set_local_offset(offset: UtcOffset) {
    let _ = LOCAL_OFFSET.set(offset);
}

/// The current time in `offset`, or in UTC when it is not known.
pub fn now_in(offset: Option<UtcOffset>) -> OffsetDateTime {
    OffsetDateTime::now_utc().to_offset(offset.unwrap_or(UtcOffset::UTC))
}

/// The person's UTC offset (see `set_local_offset`); UTC when it is not known.
pub fn local_offset() -> UtcOffset {
    LOCAL_OFFSET.get().copied().unwrap_or(UtcOffset::UTC)
}

/// The current time in the person's time zone (see `set_local_offset`).
pub fn now_local() -> OffsetDateTime {
    now_in(LOCAL_OFFSET.get().copied())
}

fn shown(entry: &SidebarEntry, status: StatusFilter) -> bool {
    match status {
        StatusFilter::Active => !entry.archived,
        StatusFilter::Archived => entry.archived,
        StatusFilter::All => true,
    }
}

/// The sidebar as it is drawn: `groups` (from `merge_sidebar`) limited to the sessions `prefs.status`
/// asks for, then sorted, grouped and collapsed as `prefs` says. `now` is passed in so the date
/// buckets are testable; `active` is the open session on screen, whose row stays visible in a
/// collapsed group.
pub fn arrange(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
) -> Vec<ArrangedGroup> {
    arrange_with_origins(groups, prefs, now, active, &|path: &Path| path.to_path_buf())
}

/// `arrange`, where the Folder grouping puts the projects that share an `origin` in one group (a
/// worktree's sessions join their repository's). Every row keeps its own project, which is what
/// opening, archiving and forking use.
pub fn arrange_with_origins(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
    origin: &dyn Fn(&Path) -> PathBuf,
) -> Vec<ArrangedGroup> {
    let limited: Vec<SidebarGroup> = groups
        .iter()
        .map(|group| SidebarGroup {
            project: group.project.clone(),
            entries: group.entries.iter().filter(|entry| shown(entry, prefs.status)).cloned().collect(),
        })
        .collect();
    arrange_filtered(&limited, prefs, now, active, origin)
}

fn arrange_filtered(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    active: Option<LiveSessionId>,
    origin: &dyn Fn(&Path) -> PathBuf,
) -> Vec<ArrangedGroup> {
    match prefs.group_by {
        GroupBy::Folder => {
            let mut merged: Vec<(PathBuf, ProjectSummary, Vec<ArrangedEntry>)> = Vec::new();
            for group in groups {
                let home = origin(&group.project.path);
                let items: Vec<ArrangedEntry> = group
                    .entries
                    .iter()
                    .map(|entry| ArrangedEntry::plain(group.project.clone(), entry.clone()))
                    .collect();
                match merged.iter_mut().find(|(path, _, _)| *path == home) {
                    Some((_, heading, entries)) => {
                        heading.exists |= group.project.exists;
                        heading.session_count += group.project.session_count;
                        entries.extend(items);
                    }
                    None => {
                        let mut heading = group.project.clone();
                        heading.path = home.clone();
                        merged.push((home, heading, items));
                    }
                }
            }
            merged
                .into_iter()
                .filter(|(_, _, entries)| prefs.show_empty || !entries.is_empty())
                .map(|(home, heading, mut entries)| {
                    sort_entries(&mut entries, prefs.sort_by);
                    let entries = nest_forks(entries, prefs, active);
                    finish(folder_group_id(&home), Heading::Project(heading), entries, prefs, active)
                })
                .collect()
        }
        GroupBy::Date => {
            let mut items = all_entries(groups);
            sort_entries(&mut items, prefs.sort_by);
            let mut buckets: Vec<(DateBucket, Vec<ArrangedEntry>)> = Vec::new();
            for item in items {
                let at = match prefs.sort_by {
                    SortBy::Created => created_of(&item.entry),
                    _ => activity_of(&item.entry),
                };
                let bucket = bucket_of(at, now);
                match buckets.iter_mut().find(|(known, _)| *known == bucket) {
                    Some((_, list)) => list.push(item),
                    None => buckets.push((bucket, vec![item])),
                }
            }
            // Sorting by name meets the buckets in any order; they are drawn newest first.
            buckets.sort_by_key(|(bucket, _)| *bucket);
            buckets
                .into_iter()
                .map(|(bucket, list)| {
                    let list = nest_forks(list, prefs, active);
                    finish(bucket_group_id(bucket), Heading::Bucket(bucket), list, prefs, active)
                })
                .collect()
        }
        GroupBy::Flat => {
            let mut items = all_entries(groups);
            if items.is_empty() {
                return Vec::new();
            }
            sort_entries(&mut items, prefs.sort_by);
            vec![ArrangedGroup { id: "flat".to_string(), heading: Heading::None, collapsed: false, entries: nest_forks(items, prefs, active) }]
        }
    }
}

/// The open sessions in the order the sidebar draws them, with no row hidden: a collapsed group or
/// the status filter hides rows, not sessions, and Ctrl+Tab must still reach them.
#[cfg(test)]
pub fn open_session_order(groups: &[SidebarGroup], prefs: &SidebarPrefs, now: OffsetDateTime) -> Vec<LiveSessionId> {
    open_session_order_with_origins(groups, prefs, now, &|path: &Path| path.to_path_buf())
}

/// `open_session_order` with the same `origin` the drawn sidebar uses, so the order matches it.
pub fn open_session_order_with_origins(
    groups: &[SidebarGroup],
    prefs: &SidebarPrefs,
    now: OffsetDateTime,
    origin: &dyn Fn(&Path) -> PathBuf,
) -> Vec<LiveSessionId> {
    let mut prefs = prefs.clone();
    prefs.collapsed.clear();
    prefs.status = StatusFilter::All;
    arranged_open_order(&arrange_with_origins(groups, &prefs, now, None, origin))
}

/// The open sessions in the order the sidebar draws them; Ctrl+Tab cycles through this. Pass
/// groups arranged with no collapsed ids, so a session in a collapsed group can still be reached.
pub fn arranged_open_order(groups: &[ArrangedGroup]) -> Vec<LiveSessionId> {
    groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .filter_map(|item| match item.entry.key {
            EntryKey::Live(id) => Some(id),
            EntryKey::Past { .. } => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use forge_application::LiveSessionId;
    use forge_domain::{GroupBy, ProjectSummary, SessionId, SidebarPrefs, SortBy, StatusFilter};
    use forge_entities::SessionMarker;
    use time::format_description::well_known::Rfc3339;
    use time::OffsetDateTime;

    use super::*;
    use crate::sidebar::{EntryKey, SidebarEntry, SidebarGroup};

    fn now() -> OffsetDateTime {
        OffsetDateTime::parse("2026-10-03T12:00:00Z", &Rfc3339).unwrap()
    }

    fn entry(title: &str, created: Option<&str>, updated: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key: EntryKey::Past { project: PathBuf::from("/p"), session: SessionId(title.to_string()) },
            session_id: Some(SessionId(title.to_string())),
            title: title.to_string(),
            updated_at: updated.map(str::to_string),
            created_at: created.map(str::to_string),
            marker: SessionMarker::Idle,
            archived: false,
            forked_from: None,
        }
    }

    fn group(path: &str, entries: Vec<SidebarEntry>) -> SidebarGroup {
        SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from(path),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }
    }

    fn prefs(sort_by: SortBy, group_by: GroupBy) -> SidebarPrefs {
        SidebarPrefs { sort_by, group_by, ..SidebarPrefs::default() }
    }

    fn titles(group: &ArrangedGroup) -> Vec<&str> {
        group.entries.iter().map(|item| item.entry.title.as_str()).collect()
    }

    fn headings(groups: &[ArrangedGroup]) -> Vec<String> {
        groups
            .iter()
            .map(|group| match &group.heading {
                Heading::Project(project) => project.path.display().to_string(),
                Heading::Bucket(bucket) => bucket.label().to_string(),
                Heading::None => String::new(),
            })
            .collect()
    }

    #[test]
    fn activity_sort_puts_the_newest_first_and_a_session_not_on_disk_before_all() {
        let groups = vec![group(
            "/a",
            vec![
                entry("old", None, Some("2026-10-01T00:00:00Z")),
                entry("new", None, Some("2026-10-03T10:00:00Z")),
                entry("unsaved", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["unsaved", "new", "old"]);
    }

    #[test]
    fn a_fractional_second_is_still_in_order() {
        let groups = vec![group(
            "/a",
            vec![
                entry("whole", None, Some("2026-10-03T01:02:03Z")),
                entry("later", None, Some("2026-10-03T01:02:03.500Z")),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["later", "whole"]);
    }

    #[test]
    fn name_sort_ignores_case_and_puts_korean_after_latin() {
        let groups = vec![group(
            "/a",
            vec![
                entry("banana", None, None),
                entry("가나다", None, None),
                entry("Apple", None, None),
                entry("cherry", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Name, GroupBy::Folder), now(), None);

        assert_eq!(titles(&arranged[0]), ["Apple", "banana", "cherry", "가나다"]);
    }

    #[test]
    fn created_sort_differs_from_activity_sort_and_falls_back_to_the_last_activity() {
        let groups = vec![group(
            "/a",
            vec![
                entry("began-first", Some("2026-10-01T00:00:00Z"), Some("2026-10-03T00:00:00Z")),
                entry("began-second", Some("2026-10-02T00:00:00Z"), Some("2026-10-02T00:00:00Z")),
                entry("no-created", None, Some("2026-10-02T12:00:00Z")),
            ],
        )];

        let by_created = arrange(&groups, &prefs(SortBy::Created, GroupBy::Folder), now(), None);
        let by_activity = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(titles(&by_created[0]), ["no-created", "began-second", "began-first"]);
        assert_eq!(titles(&by_activity[0]), ["began-first", "no-created", "began-second"]);
    }

    #[test]
    fn two_sessions_with_one_title_both_stay() {
        let mut second = entry("same", None, Some("2026-10-02T00:00:00Z"));
        second.key = EntryKey::Past { project: PathBuf::from("/p"), session: SessionId("other-id".to_string()) };
        let groups = vec![group("/a", vec![entry("same", None, Some("2026-10-03T00:00:00Z")), second])];

        for sort in [SortBy::Name, SortBy::Created, SortBy::Activity] {
            for by in [GroupBy::Folder, GroupBy::Date, GroupBy::Flat] {
                let arranged = arrange(&groups, &prefs(sort, by), now(), None);
                let count: usize = arranged.iter().map(|g| g.entries.len()).sum();
                assert_eq!(count, 2, "{sort:?} {by:?}");
            }
        }
    }

    #[test]
    fn folder_grouping_keeps_the_projects_in_their_order_and_hides_empty_ones() {
        let groups = vec![
            group("/empty", vec![]),
            group("/b", vec![entry("one", None, Some("2026-10-03T00:00:00Z"))]),
        ];

        let hidden = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);
        let shown = arrange(
            &groups,
            &SidebarPrefs { show_empty: true, ..prefs(SortBy::Activity, GroupBy::Folder) },
            now(),
            None,
        );

        assert_eq!(headings(&hidden), ["/b"]);
        assert_eq!(headings(&shown), ["/empty", "/b"]);
        assert_eq!(hidden[0].id, "folder:/b");
    }

    #[test]
    fn date_grouping_cuts_on_calendar_days_and_skips_empty_buckets() {
        let groups = vec![group(
            "/a",
            vec![
                entry("today", None, Some("2026-10-03T01:00:00Z")),
                entry("yesterday", None, Some("2026-10-02T23:59:00Z")),
                entry("seven", None, Some("2026-09-26T08:00:00Z")),
                entry("eight", None, Some("2026-09-25T08:00:00Z")),
                entry("thirty", None, Some("2026-09-03T08:00:00Z")),
                entry("thirty-one", None, Some("2026-09-02T08:00:00Z")),
                entry("unsaved", None, None),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), now(), None);

        assert_eq!(
            headings(&arranged),
            ["Today", "Yesterday", "Previous 7 days", "Previous 30 days", "Older"]
        );
        assert_eq!(titles(&arranged[0]), ["unsaved", "today"]);
        assert_eq!(titles(&arranged[1]), ["yesterday"]);
        assert_eq!(titles(&arranged[2]), ["seven"]);
        assert_eq!(titles(&arranged[3]), ["eight", "thirty"]);
        assert_eq!(titles(&arranged[4]), ["thirty-one"]);
    }

    #[test]
    fn date_grouping_by_created_uses_the_creation_day() {
        let groups = vec![group(
            "/a",
            vec![entry("old-but-active", Some("2026-08-01T00:00:00Z"), Some("2026-10-03T00:00:00Z"))],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Created, GroupBy::Date), now(), None);

        assert_eq!(headings(&arranged), ["Older"]);
    }

    #[test]
    fn date_grouping_mixes_projects_and_each_row_keeps_its_own() {
        let groups = vec![
            group("/a", vec![entry("from-a", None, Some("2026-10-03T02:00:00Z"))]),
            group("/b", vec![entry("from-b", None, Some("2026-10-03T03:00:00Z"))]),
        ];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), now(), None);

        assert_eq!(titles(&arranged[0]), ["from-b", "from-a"]);
        assert_eq!(arranged[0].entries[0].project.path, PathBuf::from("/b"));
        assert_eq!(arranged[0].entries[1].project.path, PathBuf::from("/a"));
    }

    #[test]
    fn name_sort_inside_date_buckets_keeps_the_buckets_in_time_order() {
        let groups = vec![group(
            "/a",
            vec![
                entry("zebra", None, Some("2026-10-03T01:00:00Z")),
                entry("apple", None, Some("2026-08-01T00:00:00Z")),
                entry("mango", None, Some("2026-10-03T02:00:00Z")),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Name, GroupBy::Date), now(), None);

        assert_eq!(headings(&arranged), ["Today", "Older"]);
        assert_eq!(titles(&arranged[0]), ["mango", "zebra"]);
    }

    #[test]
    fn no_grouping_gives_one_group_with_no_heading_and_nothing_when_there_is_nothing() {
        let groups = vec![
            group("/a", vec![entry("one", None, Some("2026-10-01T00:00:00Z"))]),
            group("/b", vec![entry("two", None, Some("2026-10-02T00:00:00Z"))]),
        ];

        let flat = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Flat), now(), None);
        let none = arrange(&[], &prefs(SortBy::Activity, GroupBy::Flat), now(), None);

        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].heading, Heading::None);
        assert_eq!(titles(&flat[0]), ["two", "one"]);
        assert!(none.is_empty());
    }

    #[test]
    fn a_collapsed_group_hides_its_rows_except_the_open_session() {
        let live_id = LiveSessionId(7);
        let mut live = entry("open", None, Some("2026-10-03T00:00:00Z"));
        live.key = EntryKey::Live(live_id);
        let groups = vec![group("/a", vec![live, entry("closed", None, Some("2026-10-02T00:00:00Z"))])];
        let collapsed = SidebarPrefs { collapsed: vec![folder_group_id(std::path::Path::new("/a"))], ..SidebarPrefs::default() };

        let with_open = arrange(&groups, &collapsed, now(), Some(live_id));
        let with_none = arrange(&groups, &collapsed, now(), None);

        assert!(with_open[0].collapsed);
        assert_eq!(titles(&with_open[0]), ["open"]);
        assert!(with_none[0].collapsed);
        assert!(with_none[0].entries.is_empty());
    }

    #[test]
    fn the_open_order_follows_the_arranged_rows() {
        let mut first = entry("first", None, Some("2026-10-01T00:00:00Z"));
        first.key = EntryKey::Live(LiveSessionId(1));
        let mut second = entry("second", None, Some("2026-10-02T00:00:00Z"));
        second.key = EntryKey::Live(LiveSessionId(2));
        let groups = vec![group("/a", vec![first, second])];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(arranged_open_order(&arranged), [LiveSessionId(2), LiveSessionId(1)]);
    }

    #[test]
    fn date_buckets_follow_the_day_of_now_in_its_own_offset() {
        // 12:00 on 3 October in Korea (+09:00) is 03:00Z. A session written at 23:30Z on 2 October
        // was written at 08:30 this morning there, so it belongs under Today, not Yesterday.
        let korea = OffsetDateTime::parse("2026-10-03T12:00:00+09:00", &Rfc3339).unwrap();
        let groups = vec![group(
            "/a",
            vec![
                entry("this-morning", None, Some("2026-10-02T23:30:00Z")),
                entry("yesterday-evening", None, Some("2026-10-02T10:00:00Z")),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), korea, None);

        assert_eq!(headings(&arranged), ["Today", "Yesterday"]);
        assert_eq!(titles(&arranged[0]), ["this-morning"]);
        assert_eq!(titles(&arranged[1]), ["yesterday-evening"]);
    }

    #[test]
    fn the_current_time_is_given_in_the_offset_it_is_asked_for() {
        let korea = UtcOffset::from_hms(9, 0, 0).unwrap();

        assert_eq!(now_in(Some(korea)).offset(), korea);
        assert_eq!(now_in(None).offset(), UtcOffset::UTC);
    }

    fn archived_entry(title: &str) -> SidebarEntry {
        SidebarEntry { archived: true, ..entry(title, None, Some("2026-09-29T00:00:00Z")) }
    }

    fn all_titles(arranged: &[ArrangedGroup]) -> Vec<String> {
        arranged.iter().flat_map(|g| g.entries.iter().map(|i| i.entry.title.clone())).collect()
    }

    fn with_status(status: StatusFilter, group_by: GroupBy) -> SidebarPrefs {
        SidebarPrefs { status, ..prefs(SortBy::Name, group_by) }
    }

    #[test]
    fn active_hides_archived_entries_and_archived_shows_only_them() {
        let groups = vec![group("/a", vec![entry("kept", None, None), archived_entry("shelved")])];

        let active = arrange(&groups, &with_status(StatusFilter::Active, GroupBy::Folder), now(), None);
        let archived = arrange(&groups, &with_status(StatusFilter::Archived, GroupBy::Folder), now(), None);
        let all = arrange(&groups, &with_status(StatusFilter::All, GroupBy::Folder), now(), None);

        assert_eq!(all_titles(&active), vec!["kept"]);
        assert_eq!(all_titles(&archived), vec!["shelved"]);
        assert_eq!(all_titles(&all), vec!["kept", "shelved"]);
    }

    #[test]
    fn the_status_filter_applies_in_date_and_flat_grouping_too() {
        let groups = vec![group("/a", vec![entry("kept", None, Some("2026-09-29T00:00:00Z")), archived_entry("shelved")])];

        for by in [GroupBy::Date, GroupBy::Flat] {
            let active = arrange(&groups, &with_status(StatusFilter::Active, by), now(), None);
            assert_eq!(all_titles(&active), vec!["kept"], "{by:?}");
        }
    }

    #[test]
    fn a_project_whose_sessions_are_all_archived_is_an_empty_group_in_the_active_view() {
        let groups = vec![group("/a", vec![archived_entry("shelved")])];

        let hidden = arrange(&groups, &with_status(StatusFilter::Active, GroupBy::Folder), now(), None);
        let shown = arrange(
            &groups,
            &SidebarPrefs { show_empty: true, ..with_status(StatusFilter::Active, GroupBy::Folder) },
            now(),
            None,
        );

        assert!(hidden.is_empty());
        assert_eq!(shown.len(), 1);
        assert!(shown[0].entries.is_empty());
    }

    #[test]
    fn ctrl_tab_order_reaches_open_sessions_whatever_the_filter_and_collapsed_groups_hide() {
        let open = |title: &str, id: u64, archived: bool| SidebarEntry {
            key: EntryKey::Live(LiveSessionId(id)),
            archived,
            ..entry(title, None, Some("2026-09-29T00:00:00Z"))
        };
        let groups = vec![group("/a", vec![open("one", 1, false), open("two", 2, true)])];
        let prefs = SidebarPrefs {
            status: StatusFilter::Active,
            collapsed: vec![folder_group_id(std::path::Path::new("/a"))],
            ..SidebarPrefs::default()
        };

        let order = open_session_order(&groups, &prefs, now());

        assert_eq!(order, vec![LiveSessionId(1), LiveSessionId(2)]);
    }

    fn worktree_project(path: &str) -> ProjectSummary {
        ProjectSummary { path: PathBuf::from(path), last_active: String::new(), session_count: 1, exists: true }
    }

    #[test]
    fn a_worktrees_sessions_join_the_group_of_their_repository_and_keep_their_own_folder() {
        let groups = vec![
            SidebarGroup { project: worktree_project("/app"), entries: vec![entry("main", None, Some("2026-09-29T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/app/.claude/worktrees/x-1"), entries: vec![entry("tree", None, Some("2026-09-30T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/other"), entries: vec![entry("o", None, Some("2026-09-28T00:00:00Z"))] },
        ];
        let origin = |path: &std::path::Path| match path.to_str().unwrap() {
            "/app/.claude/worktrees/x-1" => PathBuf::from("/app"),
            other => PathBuf::from(other),
        };
        let arranged = arrange_with_origins(&groups, &SidebarPrefs::default(), now(), None, &origin);
        assert_eq!(arranged.len(), 2);
        let app = &arranged[0];
        assert!(matches!(&app.heading, Heading::Project(project) if project.path == PathBuf::from("/app")));
        assert_eq!(app.entries.len(), 2);
        assert_eq!(app.entries[0].entry.title, "tree");
        assert_eq!(app.entries[0].project.path, PathBuf::from("/app/.claude/worktrees/x-1"));
        assert_eq!(app.entries[1].project.path, PathBuf::from("/app"));
    }

    #[test]
    fn every_scratch_session_joins_one_group_under_the_scratch_root() {
        let root = std::path::Path::new("/cfg/scratch-workspaces");
        let groups = vec![
            SidebarGroup { project: worktree_project("/cfg/scratch-workspaces/scratch-2026-10-20-aaaaaa"), entries: vec![entry("s1", None, Some("2026-09-29T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/work/forge"), entries: vec![entry("f", None, Some("2026-09-28T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/cfg/scratch-workspaces/scratch-2026-10-20-bbbbbb"), entries: vec![entry("s2", None, Some("2026-09-30T00:00:00Z"))] },
            SidebarGroup { project: worktree_project("/cfg/scratch-workspaces/scratch-2026-10-21-cccccc"), entries: vec![entry("s3", None, Some("2026-09-27T00:00:00Z"))] },
        ];
        let origin = |path: &std::path::Path| forge_domain::origin_for_grouping(&[], Some(root), path);
        let arranged = arrange_with_origins(&groups, &SidebarPrefs::default(), now(), None, &origin);
        assert_eq!(arranged.len(), 2);
        let scratch = arranged.iter().find(|g| g.id == folder_group_id(root)).expect("one scratch group");
        assert_eq!(scratch.entries.len(), 3);
        assert!(matches!(&scratch.heading, Heading::Project(p) if p.path == root && p.session_count == 3));
        assert_eq!(scratch.entries[0].project.path, PathBuf::from("/cfg/scratch-workspaces/scratch-2026-10-20-bbbbbb"));
        let forge = arranged.iter().find(|g| g.id == folder_group_id(std::path::Path::new("/work/forge"))).unwrap();
        assert_eq!(forge.entries.len(), 1);
    }

    #[test]
    fn a_group_made_only_of_a_worktree_is_headed_by_its_repository() {
        let groups = vec![SidebarGroup { project: worktree_project("/app/.claude/worktrees/x-1"), entries: vec![entry("tree", None, Some("2026-09-30T00:00:00Z"))] }];
        let origin = |_: &std::path::Path| PathBuf::from("/app");
        let arranged = arrange_with_origins(&groups, &SidebarPrefs::default(), now(), None, &origin);
        assert!(matches!(&arranged[0].heading, Heading::Project(project) if project.path == PathBuf::from("/app")));
        assert_eq!(arranged[0].id, folder_group_id(std::path::Path::new("/app")));
    }

    #[test]
    fn the_open_order_follows_the_sidebar_when_a_worktree_joins_its_repository() {
        let open = |title: &str, id: u64| SidebarEntry {
            key: EntryKey::Live(LiveSessionId(id)),
            ..entry(title, None, Some("2026-09-29T00:00:00Z"))
        };
        let groups = vec![
            SidebarGroup { project: worktree_project("/app"), entries: vec![open("app", 1)] },
            SidebarGroup { project: worktree_project("/other"), entries: vec![open("other", 2)] },
            SidebarGroup { project: worktree_project("/app/.claude/worktrees/x-1"), entries: vec![open("tree", 3)] },
        ];
        let origin = |path: &std::path::Path| match path.to_str().unwrap() {
            "/app/.claude/worktrees/x-1" => PathBuf::from("/app"),
            other => PathBuf::from(other),
        };
        let prefs = SidebarPrefs { sort_by: SortBy::Name, ..SidebarPrefs::default() };
        let order = open_session_order_with_origins(&groups, &prefs, now(), &origin);
        assert_eq!(order, vec![LiveSessionId(1), LiveSessionId(3), LiveSessionId(2)]);
    }

    fn fork_of(title: &str, source: &str, updated: &str) -> SidebarEntry {
        SidebarEntry { forked_from: Some(SessionId(source.to_string())), ..entry(title, None, Some(updated)) }
    }

    fn depths(group: &ArrangedGroup) -> Vec<(String, u8)> {
        group.entries.iter().map(|item| (item.entry.title.clone(), item.depth)).collect()
    }

    #[test]
    fn a_fork_is_drawn_under_its_source_with_its_siblings_in_sorted_order() {
        let groups = vec![group(
            "/a",
            vec![
                entry("other", None, Some("2026-10-03T09:00:00Z")),
                fork_of("fork-new", "src", "2026-10-03T08:00:00Z"),
                entry("src", None, Some("2026-10-01T00:00:00Z")),
                fork_of("fork-old", "src", "2026-10-02T00:00:00Z"),
                fork_of("grandchild", "fork-old", "2026-10-02T12:00:00Z"),
            ],
        )];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(
            depths(&arranged[0]),
            [("other".to_string(), 0), ("src".to_string(), 0), ("fork-new".to_string(), 1), ("fork-old".to_string(), 1), ("grandchild".to_string(), 2)]
        );
        let src = &arranged[0].entries[1];
        assert_eq!(src.fork_count, 3);
        assert!(!src.forks_folded);
        assert_eq!(arranged[0].entries[0].fork_count, 0);
    }

    #[test]
    fn a_fork_whose_source_is_not_in_the_list_is_a_plain_row() {
        let groups = vec![group("/a", vec![fork_of("orphan", "gone", "2026-10-03T08:00:00Z"), fork_of("self", "self", "2026-10-03T07:00:00Z")])];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(depths(&arranged[0]), [("orphan".to_string(), 0), ("self".to_string(), 0)]);
    }

    #[test]
    fn a_source_in_another_date_bucket_leaves_the_fork_a_plain_row() {
        let groups = vec![group("/a", vec![entry("src", None, Some("2026-09-01T00:00:00Z")), fork_of("fork", "src", "2026-10-03T08:00:00Z")])];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Date), now(), None);

        assert_eq!(arranged.len(), 2);
        assert_eq!(depths(&arranged[0]), [("fork".to_string(), 0)]);
        assert_eq!(depths(&arranged[1]), [("src".to_string(), 0)]);
    }

    #[test]
    fn folding_a_source_hides_its_forks_but_keeps_the_open_one() {
        let open = |title: &str, source: &str, id: u64| SidebarEntry {
            key: EntryKey::Live(LiveSessionId(id)),
            forked_from: Some(SessionId(source.to_string())),
            ..entry(title, None, Some("2026-10-02T00:00:00Z"))
        };
        let groups = vec![group(
            "/a",
            vec![entry("src", None, Some("2026-10-03T00:00:00Z")), open("shown", "src", 7), fork_of("hidden", "src", "2026-10-01T00:00:00Z"), fork_of("deep", "shown", "2026-09-30T00:00:00Z")],
        )];
        let folded = SidebarPrefs { collapsed: vec![fork_fold_id(&SessionId("src".to_string()))], ..prefs(SortBy::Activity, GroupBy::Folder) };

        let arranged = arrange(&groups, &folded, now(), Some(LiveSessionId(7)));

        assert_eq!(depths(&arranged[0]), [("src".to_string(), 0), ("shown".to_string(), 1)]);
        assert!(arranged[0].entries[0].forks_folded);
        assert_eq!(arranged[0].entries[0].fork_count, 3, "the count still includes the folded forks");
        assert_eq!(arranged[0].entries[0].hidden_forks, 2, "the open session's row is not hidden");

        let unfolded = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), Some(LiveSessionId(7)));
        assert_eq!(unfolded[0].entries.len(), 4);
    }

    #[test]
    fn a_folded_source_counts_the_forks_it_hides() {
        let groups = vec![group("/a", vec![entry("src", None, Some("2026-10-03T00:00:00Z")), fork_of("one", "src", "2026-10-01T00:00:00Z"), fork_of("two", "one", "2026-09-30T00:00:00Z")])];
        let folded = SidebarPrefs { collapsed: vec![fork_fold_id(&SessionId("src".to_string()))], ..prefs(SortBy::Activity, GroupBy::Folder) };

        let arranged = arrange(&groups, &folded, now(), None);
        assert_eq!(arranged[0].entries.len(), 1);
        assert_eq!(arranged[0].entries[0].hidden_forks, 2);

        let unfolded = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);
        assert!(unfolded[0].entries.iter().all(|item| item.hidden_forks == 0));
    }

    #[test]
    fn a_loop_of_forks_cannot_hang_the_sidebar() {
        let groups = vec![group("/a", vec![fork_of("a", "b", "2026-10-03T08:00:00Z"), fork_of("b", "a", "2026-10-03T07:00:00Z")])];

        let arranged = arrange(&groups, &prefs(SortBy::Activity, GroupBy::Folder), now(), None);

        assert_eq!(arranged[0].entries.len(), 2);
    }

    #[test]
    fn nested_forks_keep_their_place_in_the_ctrl_tab_order() {
        let open = |title: &str, id: u64, source: Option<&str>| SidebarEntry {
            key: EntryKey::Live(LiveSessionId(id)),
            forked_from: source.map(|source| SessionId(source.to_string())),
            ..entry(title, None, Some("2026-09-29T00:00:00Z"))
        };
        let groups = vec![group("/a", vec![open("src", 1, None), open("other", 2, None), open("fork", 3, Some("src"))])];
        let prefs = SidebarPrefs { sort_by: SortBy::Name, ..SidebarPrefs::default() };

        let order = open_session_order(&groups, &prefs, now());

        assert_eq!(order, vec![LiveSessionId(2), LiveSessionId(1), LiveSessionId(3)]);
    }
}
