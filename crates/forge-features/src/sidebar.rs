use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use forge_application::{LiveSessionId, LiveSessionView, Phase};
use forge_domain::{ArchivedRef, MessageRole, ProjectSummary, SessionId, SessionSummary};
use forge_entities::SessionMarker;
use forge_shared::tr;

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
    /// When the session began; `None` for an open session that is not on disk yet.
    pub created_at: Option<String>,
    pub marker: SessionMarker,
    /// The person archived it (Forge's config remembers; `mark_archived` sets this).
    pub archived: bool,
    /// The session this one was forked from, when it is a fork (sidebar nesting).
    pub forked_from: Option<SessionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveAction {
    Archive,
    Unarchive,
    /// An open session that is not archived (close it first), or one with no transcript yet.
    Unavailable,
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
    pub can_fork: bool,
    pub delete: DeleteAction,
    pub archive: ArchiveAction,
}

impl SidebarEntry {
    /// What the row's menu offers. A session with no Claude id has no transcript to rename or delete.
    pub fn actions(&self) -> RowActions {
        let archive = match (&self.key, &self.session_id, self.archived) {
            (_, None, _) => ArchiveAction::Unavailable,
            (_, Some(_), true) => ArchiveAction::Unarchive,
            (EntryKey::Past { .. }, Some(_), false) => ArchiveAction::Archive,
            (EntryKey::Live(_), Some(_), false) => ArchiveAction::Unavailable,
        };
        match (&self.key, &self.session_id) {
            (_, None) => RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable, archive },
            (EntryKey::Live(_), Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::WhileOpen, archive },
            (EntryKey::Past { .. }, Some(_)) => RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Allowed, archive },
        }
    }
}

impl SidebarEntry {
    /// What the menu under the title of the open session offers: the row's actions, except that the
    /// session can be archived while open (archiving closes it, as ⌥⌘A does) and there is no delete.
    pub fn title_menu_actions(&self) -> RowActions {
        let row = self.actions();
        let archive = match (self.session_id.is_some(), self.archived) {
            (false, _) => ArchiveAction::Unavailable,
            (true, true) => ArchiveAction::Unarchive,
            (true, false) => ArchiveAction::Archive,
        };
        RowActions { delete: DeleteAction::Unavailable, archive, ..row }
    }
}

/// What a key does while a session row's menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuKeyAction {
    Rename,
    Fork,
    Delete,
    ToggleArchive,
}

/// The single letters the menu shows: R renames, F forks, A archives or unarchives, D deletes (Claude Code desktop's menu keys). A key
/// does what its item does, so a key for a disabled item does nothing. Any modifier turns the key
/// into something else (⌘R reloads), so it is left alone.
///
/// `code` is the physical key (`"KeyR"`), not the character it types: with a Korean input method
/// the R key types "ㄱ", and the menu letter must still work.
pub fn menu_key_action(code: &str, modified: bool, actions: RowActions) -> Option<MenuKeyAction> {
    if modified {
        return None;
    }
    match code {
        "KeyR" if actions.can_rename => Some(MenuKeyAction::Rename),
        "KeyF" if actions.can_fork => Some(MenuKeyAction::Fork),
        "KeyA" if actions.archive != ArchiveAction::Unavailable => Some(MenuKeyAction::ToggleArchive),
        "KeyD" if actions.delete == DeleteAction::Allowed => Some(MenuKeyAction::Delete),
        _ => None,
    }
}

/// What ⌥⌘A does to the active (open) session. The row menu only archives sessions that are not
/// open; the key closes the session as part of archiving it, and does not close one that is
/// still working.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveKey {
    Nothing,
    Unarchive,
    ArchiveAndClose,
    /// A turn is running: tell the person to stop it first.
    StopFirst,
}

pub fn archive_key_plan(archived: bool, has_id: bool, running: bool) -> ArchiveKey {
    match (has_id, archived, running) {
        (false, _, _) => ArchiveKey::Nothing,
        (true, true, _) => ArchiveKey::Unarchive,
        (true, false, true) => ArchiveKey::StopFirst,
        (true, false, false) => ArchiveKey::ArchiveAndClose,
    }
}

/// Sets `archived` on every entry that `archived` names (same project and same session id).
/// Names with no session are ignored.
pub fn mark_archived(groups: &mut [SidebarGroup], archived: &[ArchivedRef]) {
    for group in groups {
        for entry in &mut group.entries {
            entry.archived = entry.session_id.as_ref().is_some_and(|id| {
                archived
                    .iter()
                    .any(|item| item.project == group.project.path && item.session_id == id.0)
            });
        }
    }
}

/// The project a session that was just made (a fork) opens in: the listed summary when there is
/// one, else one made from the path. A project can be missing from `past` (a folder added through the
/// dialog, or a path spelled differently from the listed one), and the copy is on disk already, so
/// that must not stop it from opening.
pub fn project_for_open(past: &[(ProjectSummary, Vec<SessionSummary>)], project_path: &std::path::Path) -> ProjectSummary {
    past.iter()
        .find(|(project, _)| project.path == project_path)
        .map(|(project, _)| project.clone())
        .unwrap_or_else(|| ProjectSummary {
            path: project_path.to_path_buf(),
            last_active: String::new(),
            session_count: 0,
            exists: project_path.is_dir(),
        })
}

/// Where a fork came from, as the `포크 원본` line at the top of its chat shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct ForkOrigin {
    pub project: ProjectSummary,
    pub session_id: SessionId,
    pub title: String,
    /// When the fork was made (RFC 3339); empty when its record does not say.
    pub forked_at: String,
}

/// The origin of a fork, looked up in the listing of `project_path`; `forked_at` is the time the fork's record gives. `None` (the line is
/// hidden) when the origin is not listed there or has no title.
pub fn fork_origin(
    past: &[(ProjectSummary, Vec<SessionSummary>)],
    project_path: &std::path::Path,
    origin: &SessionId,
    forked_at: Option<&str>,
) -> Option<ForkOrigin> {
    let (project, sessions) = past.iter().find(|(project, _)| project.path == project_path)?;
    let found = sessions.iter().find(|session| &session.id == origin)?;
    let title = found.title.clone().filter(|title| !title.trim().is_empty())?;
    Some(ForkOrigin { project: project.clone(), session_id: found.id.clone(), title, forked_at: forked_at.unwrap_or_default().to_string() })
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
                .map(|message| truncate_chars(&message.text(), MAX_TITLE_CHARS)),
            marker: marker_for(view),
            can_send: view.can_send,
        }
    }

    pub(crate) fn display_title(&self) -> String {
        self.title
            .clone()
            .or_else(|| self.first_user_message.clone().filter(|text| !text.trim().is_empty()))
            .unwrap_or_else(|| tr!("New session").to_string())
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
                        .unwrap_or_else(|| tr!("Untitled session").to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    created_at: Some(summary.created_at.clone()),
                    marker: open.marker,
                    archived: false,
                    forked_from: summary.forked_from.clone(),
                },
                None => SidebarEntry {
                    key: EntryKey::Past { project: project.path.clone(), session: summary.id.clone() },
                    session_id: Some(summary.id.clone()),
                    title: summary.title.clone().unwrap_or_else(|| tr!("Untitled session").to_string()),
                    updated_at: Some(summary.updated_at.clone()),
                    created_at: Some(summary.created_at.clone()),
                    marker: SessionMarker::Idle,
                    archived: false,
                    forked_from: summary.forked_from.clone(),
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
            created_at: None,
            marker: open.marker,
            archived: false,
            forked_from: None,
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

/// The open session at `index` (from 0) in the order the sidebar draws them; none when fewer are open.
pub fn nth_session(order: &[LiveSessionId], index: usize) -> Option<LiveSessionId> {
    order.get(index).copied()
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
        Message, PermissionMode, PermissionRequest, ProjectSummary, Session, SessionId,
        SessionSummary,
    };
    use forge_entities::SessionMarker;

    use super::*;

    #[test]
    fn the_archive_key_archives_and_closes_an_idle_session_with_an_id() {
        assert_eq!(archive_key_plan(false, true, false), ArchiveKey::ArchiveAndClose);
    }

    #[test]
    fn the_archive_key_unarchives_an_archived_session_even_while_it_runs() {
        assert_eq!(archive_key_plan(true, true, false), ArchiveKey::Unarchive);
        assert_eq!(archive_key_plan(true, true, true), ArchiveKey::Unarchive);
    }

    #[test]
    fn a_running_session_is_not_closed_by_the_archive_key() {
        assert_eq!(archive_key_plan(false, true, true), ArchiveKey::StopFirst);
    }

    #[test]
    fn a_session_without_an_id_cannot_be_archived() {
        assert_eq!(archive_key_plan(false, false, false), ArchiveKey::Nothing);
        assert_eq!(archive_key_plan(true, false, false), ArchiveKey::Nothing);
    }

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
            created_at: format!("2026-09-28T00:00:0{}Z", id.len() % 10),
            forked_from: None,
        }
    }

    fn entry(key: EntryKey, session: Option<&str>) -> SidebarEntry {
        SidebarEntry {
            key,
            session_id: session.map(|id| SessionId(id.to_string())),
            title: "t".to_string(),
            updated_at: None,
            created_at: None,
            marker: SessionMarker::Idle,
            archived: false,
            forked_from: None,
        }
    }

    fn actions(can_rename: bool, delete: DeleteAction) -> RowActions {
        // A session that can be renamed has a Claude id, and so can be forked.
        RowActions { can_rename, can_fork: can_rename, delete, archive: ArchiveAction::Archive }
    }

    #[test]
    fn the_r_key_renames_and_the_d_key_deletes() {
        let all = actions(true, DeleteAction::Allowed);
        assert_eq!(menu_key_action("KeyR", false, all), Some(MenuKeyAction::Rename));
        assert_eq!(menu_key_action("KeyD", false, all), Some(MenuKeyAction::Delete));
    }

    fn past_with(path: &str, sessions: Vec<SessionSummary>) -> Vec<(ProjectSummary, Vec<SessionSummary>)> {
        let (project, _) = listed(path);
        vec![(project, sessions)]
    }

    #[test]
    fn the_origin_of_a_fork_is_found_in_its_own_project_with_the_time_of_the_fork() {
        let past = past_with("/work/a", vec![summary("origin", Some("Fix login")), summary("fork", Some("Fix login (fork)"))]);

        let found = fork_origin(&past, std::path::Path::new("/work/a"), &SessionId("origin".to_string()), Some("2026-10-28T10:00:00Z")).unwrap();

        assert_eq!(found.title, "Fix login");
        assert_eq!(found.session_id, SessionId("origin".to_string()));
        assert_eq!(found.project.path, PathBuf::from("/work/a"));
        assert_eq!(found.forked_at, "2026-10-28T10:00:00Z");
    }

    #[test]
    fn a_missing_untitled_or_other_project_origin_hides_the_line() {
        let past = past_with("/work/a", vec![summary("origin", None), summary("fork", Some("F"))]);
        let path = std::path::Path::new("/work/a");

        assert_eq!(fork_origin(&past, path, &SessionId("origin".to_string()), None), None, "no title");
        assert_eq!(fork_origin(&past, path, &SessionId("gone".to_string()), None), None, "not listed");
        assert_eq!(fork_origin(&past, std::path::Path::new("/work/b"), &SessionId("origin".to_string()), None), None, "another folder");
    }

    #[test]
    fn a_fork_without_a_recorded_time_still_shows_its_origin() {
        let past = past_with("/work/a", vec![summary("origin", Some("Mine"))]);

        let found = fork_origin(&past, std::path::Path::new("/work/a"), &SessionId("origin".to_string()), None).unwrap();

        assert_eq!(found.forked_at, "");
    }

    fn listed(path: &str) -> (ProjectSummary, Vec<SessionSummary>) {
        (ProjectSummary { path: PathBuf::from(path), last_active: "2026-10-03T00:00:00Z".to_string(), session_count: 1, exists: true }, Vec::new())
    }

    #[test]
    fn a_listed_project_is_the_one_a_fork_opens_in() {
        let past = vec![listed("/work/a"), listed("/work/b")];
        assert_eq!(project_for_open(&past, std::path::Path::new("/work/b")).path, PathBuf::from("/work/b"));
        assert_eq!(project_for_open(&past, std::path::Path::new("/work/b")).session_count, 1, "the listed summary, not a made-up one");
    }

    #[test]
    fn a_project_that_is_not_listed_still_gets_a_summary_so_the_fork_can_open() {
        // A folder added through the dialog, or one spelled differently from the listed path
        // (/tmp/x against /private/tmp/x), is not in `past`; the copy is on disk and must open.
        let past = vec![listed("/work/a")];
        let here = std::env::temp_dir();
        let summary = project_for_open(&past, &here);
        assert_eq!(summary.path, here);
        assert!(summary.exists, "a folder that is there can take messages");
    }

    #[test]
    fn a_fork_of_a_session_in_a_missing_folder_opens_view_only() {
        let summary = project_for_open(&[], std::path::Path::new("/definitely/not/here/forge-test"));
        assert!(!summary.exists);
    }

    #[test]
    fn the_f_key_forks_when_the_session_has_an_id() {
        let all = actions(true, DeleteAction::Allowed);
        assert_eq!(menu_key_action("KeyF", false, all), Some(MenuKeyAction::Fork));
        assert_eq!(menu_key_action("KeyF", false, actions(true, DeleteAction::WhileOpen)), Some(MenuKeyAction::Fork), "an open session can be forked");
        assert_eq!(menu_key_action("KeyF", false, actions(false, DeleteAction::Unavailable)), None, "no Claude id yet");
        assert_eq!(menu_key_action("KeyF", true, all), None, "⌘F");
    }

    #[test]
    fn a_key_for_a_disabled_item_does_nothing() {
        assert_eq!(menu_key_action("KeyD", false, actions(true, DeleteAction::WhileOpen)), None, "open session");
        assert_eq!(menu_key_action("KeyD", false, actions(false, DeleteAction::Unavailable)), None);
        assert_eq!(menu_key_action("KeyR", false, actions(false, DeleteAction::Unavailable)), None);
    }

    #[test]
    fn a_modified_key_and_other_keys_do_nothing() {
        let all = actions(true, DeleteAction::Allowed);
        assert_eq!(menu_key_action("KeyR", true, all), None, "⌘R");
        assert_eq!(menu_key_action("KeyX", false, all), None, "a letter with no item");
        assert_eq!(menu_key_action("Enter", false, all), None);
    }

    #[test]
    fn a_past_session_can_be_renamed_and_deleted() {
        let past = entry(EntryKey::Past { project: PathBuf::from("/w"), session: SessionId("s".into()) }, Some("s"));
        assert_eq!(past.actions(), RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Allowed, archive: ArchiveAction::Archive });
    }

    #[test]
    fn an_open_session_can_be_renamed_but_not_deleted() {
        let open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.actions(), RowActions { can_rename: true, can_fork: true, delete: DeleteAction::WhileOpen, archive: ArchiveAction::Unavailable });
    }

    #[test]
    fn the_title_menu_of_an_open_session_archives_and_closes_it_and_never_deletes() {
        let mut open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.title_menu_actions(), RowActions { can_rename: true, can_fork: true, delete: DeleteAction::Unavailable, archive: ArchiveAction::Archive });

        open.archived = true;
        assert_eq!(open.title_menu_actions().archive, ArchiveAction::Unarchive);
        assert_eq!(menu_key_action("KeyD", false, open.title_menu_actions()), None);
    }

    #[test]
    fn the_title_menu_of_a_session_without_a_claude_id_offers_nothing() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.title_menu_actions(), RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable, archive: ArchiveAction::Unavailable });
    }

    #[test]
    fn an_open_session_without_a_claude_id_has_no_actions() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.actions(), RowActions { can_rename: false, can_fork: false, delete: DeleteAction::Unavailable, archive: ArchiveAction::Unavailable });
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
    fn an_entry_keeps_when_its_session_began_and_a_new_session_has_no_time() {
        let past = vec![(project("/a"), vec![summary("s1", Some("One"))])];
        let live = vec![open(1, "/a", None, SessionMarker::Idle)];

        let groups = merge_sidebar(&past, &live);

        let on_disk = groups[0].entries.iter().find(|e| e.title == "One").unwrap();
        assert_eq!(on_disk.created_at.as_deref(), Some("2026-09-28T00:00:02Z"));
        let fresh = groups[0].entries.iter().find(|e| matches!(e.key, EntryKey::Live(_))).unwrap();
        assert_eq!(fresh.created_at, None);
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
            rules: Vec::new(),
        };
        LiveSessionView {
            id: LiveSessionId(1),
            project_path: PathBuf::from("/work/a"),
            session: Session {
                forked_at: None,
                forked_from: None,
                finished_tasks: Vec::new(),
                id: SessionId(String::new()),
                project_path: PathBuf::from("/work/a"),
                title: None,
                messages: Vec::new(),
                permission_mode: PermissionMode::Default,
                usage: forge_domain::SessionUsage::default(),
            },
            claude_session_id: None,
            model: None,
            effort: None,
            available_models: Vec::new(),
            phase,
            turn_started: None,
            background_tasks: Vec::new(),
            suggestion: None,
            pending_permissions: VecDeque::from(vec![request; pending]),
            last_error: None,
            can_send: true,
            unsent: None,
            unsent_attachments: Vec::new(),
            failed_sends: 0,
            context: None,
            context_detail: None,
            mcp_servers: None,
            commands: Vec::new(),
            allowed_rules: Vec::new(),
            queued: Vec::new(),
            rate_limit: None,
            side_thread: Vec::new(),
            rewind: None,
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
        open_view.session.messages.push(Message::user("한".repeat(100)));

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

    fn past_entry(project: &str, id: &str) -> SidebarEntry {
        entry(EntryKey::Past { project: PathBuf::from(project), session: SessionId(id.to_string()) }, Some(id))
    }

    fn one_group(project: &str, entries: Vec<SidebarEntry>) -> SidebarGroup {
        SidebarGroup {
            project: ProjectSummary {
                path: PathBuf::from(project),
                last_active: String::new(),
                session_count: entries.len(),
                exists: true,
            },
            entries,
        }
    }

    fn archived(project: &str, id: &str) -> ArchivedRef {
        ArchivedRef { project: PathBuf::from(project), session_id: id.to_string() }
    }

    #[test]
    fn mark_archived_flags_the_entries_the_list_names() {
        let mut groups = vec![one_group("/a", vec![past_entry("/a", "s1"), past_entry("/a", "s2")])];

        mark_archived(&mut groups, &[archived("/a", "s2")]);

        assert!(!groups[0].entries[0].archived);
        assert!(groups[0].entries[1].archived);
    }

    #[test]
    fn the_same_session_id_in_another_project_is_not_archived() {
        let mut groups = vec![
            one_group("/a", vec![past_entry("/a", "s1")]),
            one_group("/b", vec![past_entry("/b", "s1")]),
        ];

        mark_archived(&mut groups, &[archived("/a", "s1")]);

        assert!(groups[0].entries[0].archived);
        assert!(!groups[1].entries[0].archived, "only /a's s1 was archived");
    }

    #[test]
    fn an_archived_id_with_no_session_is_ignored() {
        let mut groups = vec![one_group("/a", vec![past_entry("/a", "s1")])];

        mark_archived(&mut groups, &[archived("/a", "gone"), archived("/zzz", "s1")]);

        assert!(!groups[0].entries[0].archived);
    }

    #[test]
    fn a_past_session_can_be_archived_and_an_archived_one_unarchived() {
        let mut past = past_entry("/a", "s");
        assert_eq!(past.actions().archive, ArchiveAction::Archive);

        past.archived = true;
        assert_eq!(past.actions().archive, ArchiveAction::Unarchive);
    }

    #[test]
    fn an_open_session_cannot_be_archived_but_an_open_archived_one_can_be_restored() {
        let mut open = entry(EntryKey::Live(LiveSessionId(1)), Some("s"));
        assert_eq!(open.actions().archive, ArchiveAction::Unavailable);

        open.archived = true;
        assert_eq!(open.actions().archive, ArchiveAction::Unarchive);
    }

    #[test]
    fn a_session_with_no_claude_id_cannot_be_archived() {
        let fresh = entry(EntryKey::Live(LiveSessionId(1)), None);
        assert_eq!(fresh.actions().archive, ArchiveAction::Unavailable);
    }

    #[test]
    fn the_a_key_toggles_archive_by_the_physical_key() {
        let mut all = actions(true, DeleteAction::Allowed);
        all.archive = ArchiveAction::Archive;
        assert_eq!(menu_key_action("KeyA", false, all), Some(MenuKeyAction::ToggleArchive));
        all.archive = ArchiveAction::Unarchive;
        assert_eq!(menu_key_action("KeyA", false, all), Some(MenuKeyAction::ToggleArchive));
        all.archive = ArchiveAction::Unavailable;
        assert_eq!(menu_key_action("KeyA", false, all), None);
        all.archive = ArchiveAction::Archive;
        assert_eq!(menu_key_action("KeyA", true, all), None, "⌘A is select all, not archive");
    }

    #[test]
    fn nth_session_counts_from_zero_and_gives_nothing_past_the_end() {
        let order = [LiveSessionId(7), LiveSessionId(3), LiveSessionId(9)];
        assert_eq!(nth_session(&order, 0), Some(LiveSessionId(7)));
        assert_eq!(nth_session(&order, 2), Some(LiveSessionId(9)));
        assert_eq!(nth_session(&order, 3), None);
        assert_eq!(nth_session(&[], 0), None);
    }
}
