use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use forge_application::{LiveSessionId, LiveSessionView, Phase};
use forge_domain::{MessageRole, ProjectSummary, SessionId, SessionSummary};
use forge_entities::SessionMarker;

const UNTITLED: &str = "Untitled session";
const NEW_SESSION: &str = "New session";
const MAX_TITLE_CHARS: usize = 40;

#[derive(Debug, Clone, PartialEq)]
pub enum EntryKey {
    Live(LiveSessionId),
    Past { project: PathBuf, session: SessionId },
}

impl EntryKey {
    /// A key that is unique among a sidebar's entries, for the renderer's keyed diff (two entries
    /// can share a title, and a new session has neither an id nor a time).
    pub fn dom_key(&self) -> String {
        match self {
            EntryKey::Live(id) => format!("live-{}", id.0),
            EntryKey::Past { project, session } => format!("past-{}-{}", project.display(), session.0),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarEntry {
    pub key: EntryKey,
    pub session_id: Option<SessionId>,
    pub title: String,
    pub updated_at: Option<String>,
    pub marker: SessionMarker,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteAction {
    Allowed,
    /// The session is open in Forge; it has to be closed first.
    WhileOpen,
    /// There is no transcript on disk to act on yet.
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RowActions {
    pub can_rename: bool,
    pub delete: DeleteAction,
}

impl SidebarEntry {
    /// What the row's menu offers. A session with no Claude id has no transcript to rename or delete.
    pub fn actions(&self) -> RowActions {
        match (&self.key, &self.session_id) {
            (_, None) => RowActions { can_rename: false, delete: DeleteAction::Unavailable },
            (EntryKey::Live(_), Some(_)) => RowActions { can_rename: true, delete: DeleteAction::WhileOpen },
            (EntryKey::Past { .. }, Some(_)) => RowActions { can_rename: true, delete: DeleteAction::Allowed },
        }
    }
}

/// Puts `sessions` in place of one project's listing and refreshes its count; other projects and
/// unknown paths are left alone.
pub fn replace_project_sessions(
    past: &mut [(ProjectSummary, Vec<SessionSummary>)],
    project_path: &std::path::Path,
    sessions: Vec<SessionSummary>,
) {
    if let Some((project, listed)) = past.iter_mut().find(|(project, _)| project.path == project_path) {
        project.session_count = sessions.len();
        *listed = sessions;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SidebarGroup {
    pub project: ProjectSummary,
    pub entries: Vec<SidebarEntry>,
}

/// The few facts about an open session that the sidebar needs, so drawing it never clones a
/// whole conversation.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveSummary {
    pub id: LiveSessionId,
    pub project_path: PathBuf,
    pub claude_session_id: Option<SessionId>,
    pub title: Option<String>,
    pub first_user_message: Option<String>,
    pub marker: SessionMarker,
    pub can_send: bool,
}

impl LiveSummary {
    pub fn from_view(view: &LiveSessionView) -> Self {
        Self {
            id: view.id,
            project_path: view.project_path.clone(),
            claude_session_id: view.claude_session_id.clone(),
            title: view.session.title.clone(),
            first_user_message: view
                .session
                .messages
                .iter()
                .find(|message| message.role == MessageRole::User)
                .map(|message| truncate_chars(&message.content, MAX_TITLE_CHARS)),
            marker: marker_for(view),
            can_send: view.can_send,
        }
    }

    fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.first_user_message.clone().filter(|text| !text.trim().is_empty()))
            .unwrap_or_else(|| NEW_SESSION.to_string())
    }
}

/// A session needing the user (a permission to answer, or a failure to look at) outranks one that
/// is merely running. Own choice: the real app's markers are not documented (spec §15.1).
pub fn marker_for(view: &LiveSessionView) -> SessionMarker {
    if !view.pending_permissions.is_empty() || view.phase == Phase::Failed {
        SessionMarker::NeedsAttention
    } else if view.phase == Phase::Running {
        SessionMarker::Running
    } else {
        SessionMarker::Idle
    }
}

/// Past sessions grouped by project in the order given (the adapter's newest-first order is kept),
/// with each open session drawn in place of the past entry that has the same Claude session id.
/// Open sessions with no past entry (a brand-new session, or one not listed yet) go to the top of
/// their project's group; a project only they belong to becomes a new group at the top. Sessions
/// are de-duplicated by id and projects by path.
pub fn merge_sidebar(
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    live: &[LiveSummary],
) -> Vec<SidebarGroup> {
    let live_by_claude_id: HashMap<&SessionId, &LiveSummary> = live
        .iter()
        .filter_map(|summary| summary.claude_session_id.as_ref().map(|id| (id, summary)))
        .collect();
    let mut seen: HashSet<SessionId> = HashSet::new();
    let mut groups: Vec<SidebarGroup> = Vec::new();

    for (project, summaries) in past {
        let index = match groups.iter().position(|group| group.project.path == project.path) {
            Some(index) => index,
            None => {
                groups.push(SidebarGroup { project: project.clone(), entries: Vec::new() });
                groups.len() - 1
            }
        };
        for summary in summaries {
            if !seen.insert(summary.id.clone()) {
                continue;
            }
            let entry = match live_by_claude_id.get(&summary.id) {
                Some(open) => SidebarEntry {
                    key: EntryKey::Live(open.id),
                    session_id: Some(summary.id.clone()),
                    title: open
                        .title
                        .clone()
                        .or_else(|| summary.title.clone())
                        .unwrap_or_else(|| UNTITLED.to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    marker: open.marker,
                },
                None => SidebarEntry {
                    key: EntryKey::Past { project: project.path.clone(), session: summary.id.clone() },
                    session_id: Some(summary.id.clone()),
                    title: summary.title.clone().unwrap_or_else(|| UNTITLED.to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    marker: SessionMarker::Idle,
                },
            };
            groups[index].entries.push(entry);
        }
    }

    // Open sessions that no past entry stands for, in the order they were opened.
    let mut extras: Vec<(PathBuf, bool, Vec<SidebarEntry>)> = Vec::new();
    for open in live {
        let listed = open
            .claude_session_id
            .as_ref()
            .is_some_and(|id| seen.contains(id));
        if listed {
            continue;
        }
        let entry = SidebarEntry {
            key: EntryKey::Live(open.id),
            session_id: open.claude_session_id.clone(),
            title: open.display_title(),
            updated_at: None,
            marker: open.marker,
        };
        match extras.iter_mut().find(|(path, _, _)| *path == open.project_path) {
            Some((_, _, entries)) => entries.push(entry),
            None => extras.push((open.project_path.clone(), open.can_send, vec![entry])),
        }
    }

    let mut new_groups: Vec<SidebarGroup> = Vec::new();
    for (path, exists, entries) in extras {
        match groups.iter_mut().find(|group| group.project.path == path) {
            Some(group) => {
                group.entries.splice(0..0, entries);
            }
            None => new_groups.push(SidebarGroup {
                project: ProjectSummary { path, last_active: String::new(), session_count: 0, exists },
                entries,
            }),
        }
    }
    new_groups.extend(groups);
    new_groups
}

/// The open sessions in the order the sidebar draws them; Ctrl+Tab cycles through this.
pub fn open_order(groups: &[SidebarGroup]) -> Vec<LiveSessionId> {
    groups
        .iter()
        .flat_map(|group| group.entries.iter())
        .filter_map(|entry| match entry.key {
            EntryKey::Live(id) => Some(id),
            EntryKey::Past { .. } => None,
        })
        .collect()
}

/// The next (or previous) open session, wrapping around. With no current session, forward starts
/// at the first and backward at the last.
pub fn cycle_target(
    order: &[LiveSessionId],
    current: Option<LiveSessionId>,
    forward: bool,
) -> Option<LiveSessionId> {
    if order.is_empty() {
        return None;
    }
    let position = current.and_then(|id| order.iter().position(|candidate| *candidate == id));
    let index = match (position, forward) {
        (None, true) => 0,
        (None, false) => order.len() - 1,
        (Some(index), true) => (index + 1) % order.len(),
        (Some(index), false) => (index + order.len() - 1) % order.len(),
    };
    Some(order[index])
}

// Counts characters, not bytes, so multi-byte text is never cut inside a character.
fn truncate_chars(text: &str, max: usize) -> String {
    let flattened = text.replace('\n', " ");
    if flattened.chars().count() <= max {
        return flattened;
    }
    let mut truncated: String = flattened.chars().take(max - 1).collect();
    truncated.push('…');
    truncated
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::path::{Path, PathBuf};

    use forge_application::{LiveSessionId, LiveSessionView, Phase};
    use forge_domain::{
        Message, MessageRole, PermissionMode, PermissionRequest, ProjectSummary, Session, SessionId,
        SessionSummary,
    };
    use forge_entities::SessionMarker;

    use super::*;

    fn project(path: &str) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 0,
            exists: true,
        }
    }

    fn summary(id: &str, title: Option<&str>) -> SessionSummary {
        SessionSummary {
            id: SessionId(id.to_string()),
            title: title.map(str::to_string),
            updated_at: format!("2026-09-29T00:00:0{}Z", id.len() % 10),
        }
    }

    fn entry(key: EntryKey, session: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key,
            session_id: session.map(|id| SessionId(id.to_string())),
            title: "t".to_string(),
            updated_at: None,
            marker: SessionMarker::Idle,
        }
    }

    #[test]
    fn a_past_session_can_be_renamed_and_deleted() {
        let past = entry(EntryKey::Past { project: PathBuf::from("/w"), session: SessionId("s".into()) }, Some("s"));
        assert_eq!(past.actions(), RowActions { can_rename: true, delete: DeleteAction::Allowed });
    }

    #[test]
    fn an_open_session_can_be_renamed_but_not_deleted() {
        let open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.actions(), RowActions { can_rename: true, delete: DeleteAction::WhileOpen });
    }

    #[test]
    fn an_open_session_without_a_claude_id_has_no_actions() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.actions(), RowActions { can_rename: false, delete: DeleteAction::Unavailable });
    }

    #[test]
    fn replacing_one_projects_sessions_leaves_the_others_and_updates_the_count() {
        let mut past = vec![
            (project("/a"), vec![summary("1", Some("one")), summary("2", Some("two"))]),
            (project("/b"), vec![summary("3", Some("three"))]),
        ];

        replace_project_sessions(&mut past, Path::new("/a"), vec![summary("2", Some("renamed"))]);

        assert_eq!(past[0].1.len(), 1);
        assert_eq!(past[0].1[0].title.as_deref(), Some("renamed"));
        assert_eq!(past[0].0.session_count, 1);
        assert_eq!(past[1].1.len(), 1, "the other project is untouched");
    }

    #[test]
    fn replacing_sessions_of_an_unknown_project_changes_nothing() {
        let mut past = vec![(project("/a"), vec![summary("1", None)])];
        replace_project_sessions(&mut past, Path::new("/zzz"), Vec::new());
        assert_eq!(past[0].1.len(), 1);
    }

    fn open(id: u64, path: &str, claude_id: Option<&str>, marker: SessionMarker) -> LiveSummary {
        LiveSummary {
            id: LiveSessionId(id),
            project_path: PathBuf::from(path),
            claude_session_id: claude_id.map(|id| SessionId(id.to_string())),
            title: None,
            first_user_message: None,
            marker,
            can_send: true,
        }
    }

    fn view(phase: Phase, pending: usize) -> LiveSessionView {
        let request = PermissionRequest {
            id: "p".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({}),
        };
        LiveSessionView {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: Session {
                id: SessionId(String::new()),
                project_path: PathBuf::from("/work/a"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
            },
            claude_session_id: None,
            phase,
            pending_permissions: VecDeque::from(vec![request; pending]),
            last_error: None,
            can_send: true,
            unsent: None,
            failed_sends: 0,
        }
    }

    fn titles(group: &SidebarGroup) -> Vec<&str> {
        group.entries.iter().map(|entry| entry.title.as_str()).collect()
    }

    #[test]
    fn groups_and_entries_keep_the_order_they_were_given() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("first")), summary("s-22", Some("second"))]),
            (project("/work/b"), vec![summary("s-333", Some("third"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project.path, PathBuf::from("/work/a"));
        assert_eq!(titles(&groups[0]), vec!["first", "second"]);
        assert_eq!(titles(&groups[1]), vec!["third"]);
        assert!(groups[0].entries.iter().all(|e| e.marker == SessionMarker::Idle));
    }

    #[test]
    fn an_untitled_past_session_gets_a_placeholder() {
        let past = vec![(project("/work/a"), vec![summary("s-1", None)])];

        assert_eq!(titles(&merge_sidebar(&past, &[])[0]), vec!["Untitled session"]);
    }

    #[test]
    fn an_open_session_replaces_the_past_entry_with_the_same_claude_id() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("old title")), summary("s-2", Some("other"))])];
        let live = vec![open(7, "/work/a", Some("s-1"), SessionMarker::Running)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].entries.len(), 2, "replaced in place, not duplicated");
        assert_eq!(groups[0].entries[0].key, EntryKey::Live(LiveSessionId(7)));
        assert_eq!(groups[0].entries[0].marker, SessionMarker::Running);
        assert_eq!(groups[0].entries[0].title, "old title");
        assert!(matches!(groups[0].entries[1].key, EntryKey::Past { .. }));
    }

    #[test]
    fn a_brand_new_open_session_goes_to_the_top_of_its_projects_group() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("first"))])];
        let live = vec![open(7, "/work/a", None, SessionMarker::Idle)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 1);
        assert_eq!(titles(&groups[0]), vec!["New session", "first"]);
        assert_eq!(groups[0].entries[0].updated_at, None);
    }

    #[test]
    fn open_sessions_for_an_unlisted_project_form_a_new_group_at_the_top() {
        let past = vec![(project("/work/a"), vec![summary("s-1", Some("first"))])];
        let live = vec![open(7, "/work/z", None, SessionMarker::Idle), open(8, "/work/z", None, SessionMarker::Running)];

        let groups = merge_sidebar(&past, &live);

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].project.path, PathBuf::from("/work/z"));
        assert_eq!(groups[0].entries.len(), 2);
        assert_eq!(groups[0].entries[0].key, EntryKey::Live(LiveSessionId(7)), "in the order opened");
        assert_eq!(groups[1].project.path, PathBuf::from("/work/a"));
    }

    #[test]
    fn a_new_session_is_titled_by_its_first_message() {
        let mut new = open(7, "/work/a", None, SessionMarker::Idle);
        new.first_user_message = Some("fix the login bug".to_string());

        let groups = merge_sidebar(&[], &[new]);

        assert_eq!(titles(&groups[0]), vec!["fix the login bug"]);
    }

    #[test]
    fn a_blank_first_message_falls_back_to_new_session() {
        let mut new = open(7, "/work/a", None, SessionMarker::Idle);
        new.first_user_message = Some("   ".to_string());

        assert_eq!(titles(&merge_sidebar(&[], &[new])[0]), vec!["New session"]);
    }

    #[test]
    fn a_session_listed_under_two_projects_appears_once() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("dup"))]),
            (project("/work/b"), vec![summary("s-1", Some("dup")), summary("s-22", Some("kept"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(titles(&groups[0]), vec!["dup"]);
        assert_eq!(titles(&groups[1]), vec!["kept"]);
    }

    #[test]
    fn two_spellings_of_one_project_path_merge_into_one_group() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("one"))]),
            (project("/work/a"), vec![summary("s-22", Some("two"))]),
        ];

        let groups = merge_sidebar(&past, &[]);

        assert_eq!(groups.len(), 1);
        assert_eq!(titles(&groups[0]), vec!["one", "two"]);
    }

    #[test]
    fn markers_rank_needs_attention_above_running_above_idle() {
        assert_eq!(marker_for(&view(Phase::Idle, 0)), SessionMarker::Idle);
        assert_eq!(marker_for(&view(Phase::Running, 0)), SessionMarker::Running);
        assert_eq!(marker_for(&view(Phase::Running, 1)), SessionMarker::NeedsAttention);
        assert_eq!(marker_for(&view(Phase::Failed, 0)), SessionMarker::NeedsAttention);
    }

    #[test]
    fn a_summary_carries_only_what_the_sidebar_needs() {
        let mut open_view = view(Phase::Running, 0);
        open_view.session.messages.push(Message {
            role: MessageRole::User,
            content: "한".repeat(100),
            tool_calls: Vec::new(),
        });

        let summary = LiveSummary::from_view(&open_view);

        assert_eq!(summary.marker, SessionMarker::Running);
        assert_eq!(summary.first_user_message.as_ref().unwrap().chars().count(), 40);
        assert!(summary.can_send);
    }

    #[test]
    fn open_order_follows_the_sidebar_and_skips_past_entries() {
        let past = vec![
            (project("/work/a"), vec![summary("s-1", Some("a1")), summary("s-22", Some("a2"))]),
            (project("/work/b"), vec![summary("s-333", Some("b1"))]),
        ];
        let live = vec![
            open(1, "/work/b", Some("s-333"), SessionMarker::Idle),
            open(2, "/work/a", Some("s-22"), SessionMarker::Idle),
        ];

        let order = open_order(&merge_sidebar(&past, &live));

        assert_eq!(order, vec![LiveSessionId(2), LiveSessionId(1)]);
    }

    #[test]
    fn cycling_wraps_in_both_directions() {
        let order = [LiveSessionId(1), LiveSessionId(2), LiveSessionId(3)];

        assert_eq!(cycle_target(&order, Some(LiveSessionId(1)), true), Some(LiveSessionId(2)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(3)), true), Some(LiveSessionId(1)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(1)), false), Some(LiveSessionId(3)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(2)), false), Some(LiveSessionId(1)));
    }

    #[test]
    fn cycling_with_no_current_session_starts_at_an_end() {
        let order = [LiveSessionId(1), LiveSessionId(2)];

        assert_eq!(cycle_target(&order, None, true), Some(LiveSessionId(1)));
        assert_eq!(cycle_target(&order, None, false), Some(LiveSessionId(2)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(99)), true), Some(LiveSessionId(1)));
    }

    #[test]
    fn cycling_with_nothing_open_is_none() {
        assert_eq!(cycle_target(&[], None, true), None);
        assert_eq!(cycle_target(&[], Some(LiveSessionId(1)), false), None);
    }

    #[test]
    fn every_entry_gets_its_own_dom_key_even_when_titles_and_ids_repeat() {
        // Two brand-new sessions in one folder whose first send failed: same title, no ids, no times.
        let groups = merge_sidebar(
            &[],
            &[open(1, "/work/a", None, SessionMarker::Idle), open(2, "/work/a", None, SessionMarker::Idle)],
        );

        let keys: Vec<String> = groups[0].entries.iter().map(|entry| entry.key.dom_key()).collect();

        assert_eq!(keys.len(), 2);
        assert_ne!(keys[0], keys[1]);
        let past = EntryKey::Past { project: PathBuf::from("/work/a"), session: SessionId("s-1".to_string()) };
        assert_ne!(past.dom_key(), EntryKey::Live(LiveSessionId(1)).dom_key());
    }

    #[test]
    fn cycling_with_one_open_session_stays_on_it() {
        let order = [LiveSessionId(5)];

        assert_eq!(cycle_target(&order, Some(LiveSessionId(5)), true), Some(LiveSessionId(5)));
        assert_eq!(cycle_target(&order, Some(LiveSessionId(5)), false), Some(LiveSessionId(5)));
    }
}
