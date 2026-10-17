use std::collections::{BTreeMap, HashSet};

use forge_entities::{file_class, FileClass};
use forge_ports::{ChangeSource, ChangeStatus, ChangedFile, CommitInfo, GitError};

/// Only this many stack sections start unfolded: a thousand open diffs would freeze the window.
pub const OPEN_SECTIONS: usize = 50;
/// Only this many file rows are drawn.
pub const ROW_LIMIT: usize = 500;

/// What the panel shows. `All` is against the base branch; `Commit` is one commit of the branch.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ChangeView {
    #[default]
    All,
    Uncommitted,
    Commit(String),
}

/// What the person set in one session's Changes panel (kept while Forge runs).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ChangesSession {
    pub view: ChangeView,
    /// The base the person picked; `None` follows the default.
    pub base: Option<String>,
    /// Files folded by hand.
    pub folded_files: HashSet<String>,
    /// Files past `OPEN_SECTIONS` opened by hand.
    pub expanded_files: HashSet<String>,
    pub closed_folders: HashSet<String>,
    /// The `Test files` and `Generated files` sections opened by hand (they start closed).
    pub open_sections: HashSet<FileClass>,
}

/// All changes with no base to compare with are the uncommitted changes.
pub fn source_for(view: &ChangeView, base: Option<&str>) -> ChangeSource {
    match (view, base) {
        (ChangeView::All, Some(base)) => ChangeSource::All { base: base.to_string() },
        (ChangeView::All, None) | (ChangeView::Uncommitted, _) => ChangeSource::Uncommitted,
        (ChangeView::Commit(sha), _) => ChangeSource::Commit { sha: sha.clone() },
    }
}

/// What to load instead when loading `asked` failed with `error`: a base branch that is not there
/// falls back to the uncommitted changes (with the missing name, for the notice). A commit view
/// and any other error have no fallback. The panel keeps the source it really loaded with the
/// files, so a section reads more lines against that source.
pub fn fallback_source(asked: &ChangeSource, error: &GitError) -> Option<(ChangeSource, String)> {
    match (asked, error) {
        (ChangeSource::Commit { .. }, _) => None,
        (_, GitError::BaseNotFound(base)) => Some((ChangeSource::Uncommitted, base.clone())),
        _ => None,
    }
}

/// The folder (no trailing slash, empty at the top) and the name of a path.
/// True when the view picks a commit that a loaded commit list no longer holds (a reset or a
/// rebase). The list must have been loaded: an empty list from a successful load means the commit
/// is gone too, so the panel calls this only once its commits are `Some`, never while they load.
pub fn vanished_commit(view: &ChangeView, commits: &[CommitInfo]) -> bool {
    match view {
        ChangeView::Commit(sha) => !commits.iter().any(|commit| &commit.sha == sha),
        ChangeView::All | ChangeView::Uncommitted => false,
    }
}

pub fn split_folder(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder.to_string(), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    Folder { path: String, name: String, depth: usize, open: bool },
    /// The head of the `Test files` or `Generated files` section.
    Section { kind: FileClass, count: usize, open: bool },
    File { index: usize, path: String, name: String, folder: String, depth: usize, added: Option<usize>, removed: Option<usize>, status: ChangeStatus },
}

#[derive(Default)]
struct Node {
    folders: BTreeMap<String, Node>,
    files: Vec<usize>,
}

fn file_row(files: &[ChangedFile], index: usize, depth: usize) -> TreeRow {
    let file = &files[index];
    let (folder, name) = split_folder(&file.path);
    TreeRow::File { index, path: file.path.clone(), name, folder, depth, added: file.added, removed: file.removed, status: file.status }
}

fn walk(node: &Node, prefix: &str, depth: usize, files: &[ChangedFile], closed: &HashSet<String>, respect_closed: bool, rows: &mut Vec<TreeRow>) {
    for (name, child) in &node.folders {
        let path = if prefix.is_empty() { name.clone() } else { format!("{prefix}/{name}") };
        let open = !closed.contains(&path);
        rows.push(TreeRow::Folder { path: path.clone(), name: name.clone(), depth, open });
        if open || !respect_closed {
            walk(child, &path, depth + 1, files, closed, respect_closed, rows);
        }
    }
    let mut ids = node.files.clone();
    ids.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
    rows.extend(ids.into_iter().map(|index| file_row(files, index, depth)));
}

/// The rows for `ids` (indexes into `files`), starting `depth0` levels in.
fn build(files: &[ChangedFile], ids: &[usize], group: bool, closed: &HashSet<String>, respect_closed: bool, depth0: usize) -> Vec<TreeRow> {
    if !group {
        let mut order = ids.to_vec();
        order.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
        return order.into_iter().map(|index| file_row(files, index, depth0)).collect();
    }
    let mut root = Node::default();
    for &index in ids {
        let mut node = &mut root;
        let mut parts: Vec<&str> = files[index].path.split('/').collect();
        parts.pop();
        for part in parts {
            node = node.folders.entry(part.to_string()).or_default();
        }
        node.files.push(index);
    }
    let mut rows = Vec::new();
    walk(&root, "", depth0, files, closed, respect_closed, &mut rows);
    rows
}

fn ids_of(files: &[ChangedFile], class: FileClass) -> Vec<usize> {
    (0..files.len()).filter(|index| file_class(&files[*index].path) == class).collect()
}

/// The rows of the right column: folders (folders first, then files, each by name) or a flat list.
/// With `separate`, the normal files come first; the test files and the generated files follow,
/// each group under a `Section` row, and are listed only while their section is in `open_sections`.
/// A section with no file has no row.
pub fn tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>, separate: bool, open_sections: &HashSet<FileClass>) -> Vec<TreeRow> {
    if !separate {
        let all: Vec<usize> = (0..files.len()).collect();
        return build(files, &all, group, closed, true, 0);
    }
    let mut rows = build(files, &ids_of(files, FileClass::Normal), group, closed, true, 0);
    for kind in [FileClass::Test, FileClass::Generated] {
        let ids = ids_of(files, kind);
        if ids.is_empty() {
            continue;
        }
        let open = open_sections.contains(&kind);
        rows.push(TreeRow::Section { kind, count: ids.len(), open });
        if open {
            rows.extend(build(files, &ids, group, closed, true, 1));
        }
    }
    rows
}

/// The order of the stacked diffs: the tree's order with every folder open and every section
/// open (a closed section does not hide its diffs, as in the desktop app).
pub fn stack_order(files: &[ChangedFile], group: bool, separate: bool) -> Vec<usize> {
    let groups: Vec<Vec<usize>> = if separate {
        [FileClass::Normal, FileClass::Test, FileClass::Generated].into_iter().map(|class| ids_of(files, class)).collect()
    } else {
        vec![(0..files.len()).collect()]
    };
    groups
        .iter()
        .flat_map(|ids| build(files, ids, group, &HashSet::new(), false, 0))
        .filter_map(|row| match row {
            TreeRow::File { index, .. } => Some(index),
            TreeRow::Folder { .. } | TreeRow::Section { .. } => None,
        })
        .collect()
}

/// A click on a section row: opens a closed section, closes an open one.
pub fn toggle_section(session: &mut ChangesSession, kind: FileClass) {
    if !session.open_sections.remove(&kind) {
        session.open_sections.insert(kind);
    }
}

/// The files whose path holds `query` (no case); every file for an empty query.
pub fn filter_files(files: &[ChangedFile], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    (0..files.len()).filter(|index| query.is_empty() || files[*index].path.to_lowercase().contains(&query)).collect()
}

/// The commits whose subject, short or full sha or author holds `query` (no case); every commit
/// for an empty query.
pub fn filter_commits(commits: &[CommitInfo], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    (0..commits.len())
        .filter(|index| {
            let commit = &commits[*index];
            query.is_empty()
                || commit.subject.to_lowercase().contains(&query)
                || commit.sha.to_lowercase().contains(&query)
                || commit.author.to_lowercase().contains(&query)
        })
        .collect()
}

/// Whether the `Uncommitted changes` row is offered: not on a clean tree (the desktop app hides it
/// there), except while it is the chosen view, so the person can still see which view this is.
pub fn show_uncommitted(dirty: bool, view: &ChangeView) -> bool {
    dirty || *view == ChangeView::Uncommitted
}

pub fn is_folded(session: &ChangesSession, path: &str, position: usize) -> bool {
    session.folded_files.contains(path) || (position >= OPEN_SECTIONS && !session.expanded_files.contains(path))
}

/// A click on a section header: folds an open section, opens a folded one.
pub fn toggle_fold(session: &mut ChangesSession, path: &str, folded_now: bool) {
    if folded_now {
        session.folded_files.remove(path);
        session.expanded_files.insert(path.to_string());
    } else {
        session.expanded_files.remove(path);
        session.folded_files.insert(path.to_string());
    }
}

pub fn fold_all(session: &mut ChangesSession, paths: &[String]) {
    session.expanded_files.clear();
    session.folded_files = paths.iter().cloned().collect();
}

pub fn unfold_all(session: &mut ChangesSession, paths: &[String]) {
    session.folded_files.clear();
    session.expanded_files = paths.iter().cloned().collect();
}

use std::path::Path;

use forge_domain::{ChangeModeRef, SavedChangeView};

fn to_saved(view: &ChangeView) -> SavedChangeView {
    match view {
        ChangeView::All => SavedChangeView::All,
        ChangeView::Uncommitted => SavedChangeView::Uncommitted,
        ChangeView::Commit(sha) => SavedChangeView::Commit(sha.clone()),
    }
}

fn from_saved(view: &SavedChangeView) -> ChangeView {
    match view {
        SavedChangeView::All => ChangeView::All,
        SavedChangeView::Uncommitted => ChangeView::Uncommitted,
        SavedChangeView::Commit(sha) => ChangeView::Commit(sha.clone()),
    }
}

fn same_session(entry: &ChangeModeRef, project: &Path, session: &str) -> bool {
    entry.project == project && entry.session_id == session
}

/// The saved view and base of one session.
pub fn mode_of(list: &[ChangeModeRef], project: &Path, session: &str) -> Option<(ChangeView, Option<String>)> {
    list.iter().find(|entry| same_session(entry, project, session)).map(|entry| (from_saved(&entry.view), entry.base.clone()))
}

/// Saves a session's view and base. The default (all changes, no picked base) has no entry, so an
/// untouched session adds nothing. True when the list changed.
pub fn upsert_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str, view: &ChangeView, base: Option<&str>) -> bool {
    let is_default = *view == ChangeView::All && base.is_none();
    if is_default {
        return forget_mode(list, project, session);
    }
    let next = ChangeModeRef { project: project.to_path_buf(), session_id: session.to_string(), view: to_saved(view), base: base.map(str::to_string) };
    match list.iter_mut().find(|entry| same_session(entry, project, session)) {
        Some(entry) if *entry == next => false,
        Some(entry) => {
            *entry = next;
            true
        }
        None => {
            list.push(next);
            true
        }
    }
}

/// Drops a session's entry (its session was deleted). True when there was one.
pub fn forget_mode(list: &mut Vec<ChangeModeRef>, project: &Path, session: &str) -> bool {
    let before = list.len();
    list.retain(|entry| !same_session(entry, project, session));
    list.len() != before
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_commit_missing_from_the_loaded_list_has_vanished() {
        let commit = |sha: &str| CommitInfo { sha: sha.to_string(), subject: String::new(), author: String::new(), time: 0 };
        let list = vec![commit("aaa"), commit("bbb")];
        assert!(vanished_commit(&ChangeView::Commit("ccc".to_string()), &list));
        assert!(vanished_commit(&ChangeView::Commit("ccc".to_string()), &[]));
        assert!(!vanished_commit(&ChangeView::Commit("bbb".to_string()), &list));
        assert!(!vanished_commit(&ChangeView::All, &[]));
        assert!(!vanished_commit(&ChangeView::Uncommitted, &list));
    }

    use super::*;
    use std::path::Path;

    #[test]
    fn a_mode_is_kept_per_session() {
        let mut list = Vec::new();
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None));
        assert!(upsert_mode(&mut list, Path::new("/p"), "s2", &ChangeView::Commit("abc".to_string()), Some("dev")));
        assert_eq!(mode_of(&list, Path::new("/p"), "s1"), Some((ChangeView::Uncommitted, None)));
        assert_eq!(mode_of(&list, Path::new("/p"), "s2"), Some((ChangeView::Commit("abc".to_string()), Some("dev".to_string()))));
        assert_eq!(mode_of(&list, Path::new("/p"), "s3"), None);
        assert_eq!(mode_of(&list, Path::new("/other"), "s1"), None);
    }

    #[test]
    fn saving_the_same_mode_again_changes_nothing() {
        let mut list = Vec::new();
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        assert!(!upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None));
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn the_default_mode_leaves_no_entry() {
        let mut list = Vec::new();
        assert!(!upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, None));
        assert!(list.is_empty());
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, None));
        assert!(list.is_empty());
    }

    #[test]
    fn a_picked_base_with_all_changes_is_kept() {
        let mut list = Vec::new();
        assert!(upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::All, Some("dev")));
        assert_eq!(mode_of(&list, Path::new("/p"), "s1"), Some((ChangeView::All, Some("dev".to_string()))));
    }

    #[test]
    fn forgetting_a_session_drops_its_entry() {
        let mut list = Vec::new();
        upsert_mode(&mut list, Path::new("/p"), "s1", &ChangeView::Uncommitted, None);
        upsert_mode(&mut list, Path::new("/p"), "s2", &ChangeView::Uncommitted, None);
        assert!(forget_mode(&mut list, Path::new("/p"), "s1"));
        assert!(!forget_mode(&mut list, Path::new("/p"), "s1"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].session_id, "s2");
    }

    #[test]
    fn a_missing_base_falls_back_to_the_uncommitted_changes_and_names_the_base() {
        let asked = ChangeSource::All { base: "main".to_string() };
        assert_eq!(
            fallback_source(&asked, &GitError::BaseNotFound("main".to_string())),
            Some((ChangeSource::Uncommitted, "main".to_string()))
        );
    }

    #[test]
    fn a_commit_view_and_other_errors_have_no_fallback() {
        let commit = ChangeSource::Commit { sha: "abc".to_string() };
        assert_eq!(fallback_source(&commit, &GitError::BaseNotFound("main".to_string())), None);
        let all = ChangeSource::All { base: "main".to_string() };
        assert_eq!(fallback_source(&all, &GitError::NotARepository), None);
    }

    fn file(path: &str) -> ChangedFile {
        ChangedFile { path: path.to_string(), status: ChangeStatus::Modified, added: Some(1), removed: Some(0) }
    }

    fn paths(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                TreeRow::Folder { path, depth, .. } => format!("{}{path}/", " ".repeat(*depth)),
                TreeRow::File { path, depth, .. } => format!("{}{path}", " ".repeat(*depth)),
                TreeRow::Section { kind, count, open } => format!("section:{kind:?}:{count}:{}", if *open { "open" } else { "closed" }),
            })
            .collect()
    }

    #[test]
    fn a_grouped_tree_lists_folders_first_then_files() {
        let files = vec![file("b.txt"), file("src/z.rs"), file("a.txt"), file("src/a.rs"), file("docs/x/y.md")];
        let rows = tree_rows(&files, true, &HashSet::new(), false, &HashSet::new());
        assert_eq!(paths(&rows), vec!["docs/", " docs/x/", "  docs/x/y.md", "src/", " src/a.rs", " src/z.rs", "a.txt", "b.txt"]);
    }

    #[test]
    fn a_flat_list_is_by_path() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        let rows = tree_rows(&files, false, &HashSet::new(), false, &HashSet::new());
        assert_eq!(paths(&rows), vec!["a.txt", "b.txt", "src/a.rs"]);
    }

    #[test]
    fn a_closed_folder_hides_what_is_inside_it() {
        let files = vec![file("src/a.rs"), file("a.txt")];
        let closed: HashSet<String> = ["src".to_string()].into();
        assert_eq!(paths(&tree_rows(&files, true, &closed, false, &HashSet::new())), vec!["src/", "a.txt"]);
    }

    #[test]
    fn the_stack_follows_the_tree_order_even_when_a_folder_is_closed() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        assert_eq!(stack_order(&files, true, false), vec![1, 2, 0]);
        assert_eq!(stack_order(&files, false, false), vec![2, 0, 1]);
    }

    #[test]
    fn a_file_row_knows_its_folder_and_name() {
        let rows = tree_rows(&[file("src/deep/a.rs")], false, &HashSet::new(), false, &HashSet::new());
        match &rows[0] {
            TreeRow::File { folder, name, .. } => assert_eq!((folder.as_str(), name.as_str()), ("src/deep", "a.rs")),
            other => panic!("{other:?}"),
        }
        assert_eq!(split_folder("a.txt"), (String::new(), "a.txt".to_string()));
    }

    #[test]
    fn the_search_matches_a_part_of_the_path_without_case() {
        let files = vec![file("src/Main.rs"), file("docs/readme.md"), file("한글.txt")];
        assert_eq!(filter_files(&files, "MAIN"), vec![0]);
        assert_eq!(filter_files(&files, "한글"), vec![2]);
        assert_eq!(filter_files(&files, ""), vec![0, 1, 2]);
        assert!(filter_files(&files, "zzz").is_empty());
    }

    fn commit(sha: &str, subject: &str, author: &str) -> CommitInfo {
        CommitInfo { sha: sha.to_string(), subject: subject.to_string(), author: author.to_string(), time: 0 }
    }

    #[test]
    fn the_commit_filter_matches_subject_sha_and_author_without_case() {
        let list = vec![
            commit("d5bfbad1234", "Update a.txt", "myhency"),
            commit("0aa11bb2233", "한글 제목 수정", "Kim"),
            commit("77cc88dd991", "Fix Build", "Lee"),
        ];
        assert_eq!(filter_commits(&list, ""), vec![0, 1, 2]);
        assert_eq!(filter_commits(&list, "UPDATE"), vec![0]);
        assert_eq!(filter_commits(&list, "d5bf"), vec![0]);
        assert_eq!(filter_commits(&list, "kim"), vec![1]);
        assert_eq!(filter_commits(&list, "한글"), vec![1]);
        assert_eq!(filter_commits(&list, "build"), vec![2]);
        assert!(filter_commits(&list, "zzz").is_empty());
    }

    #[test]
    fn a_clean_tree_hides_the_uncommitted_row_unless_it_is_the_chosen_view() {
        assert!(show_uncommitted(true, &ChangeView::All));
        assert!(!show_uncommitted(false, &ChangeView::All));
        assert!(!show_uncommitted(false, &ChangeView::Commit("abc".to_string())));
        assert!(show_uncommitted(false, &ChangeView::Uncommitted));
    }

    #[test]
    fn a_missing_base_turns_all_changes_into_uncommitted() {
        assert_eq!(source_for(&ChangeView::All, Some("main")), ChangeSource::All { base: "main".to_string() });
        assert_eq!(source_for(&ChangeView::All, None), ChangeSource::Uncommitted);
        assert_eq!(source_for(&ChangeView::Uncommitted, Some("main")), ChangeSource::Uncommitted);
        assert_eq!(source_for(&ChangeView::Commit("abc".to_string()), None), ChangeSource::Commit { sha: "abc".to_string() });
    }

    #[test]
    fn sections_after_the_first_fifty_start_folded() {
        let session = ChangesSession::default();
        assert!(!is_folded(&session, "a", 0));
        assert!(!is_folded(&session, "a", OPEN_SECTIONS - 1));
        assert!(is_folded(&session, "a", OPEN_SECTIONS));
    }

    #[test]
    fn a_click_on_a_header_flips_the_fold_and_remembers_it() {
        let mut session = ChangesSession::default();
        toggle_fold(&mut session, "a", false);
        assert!(is_folded(&session, "a", 0));
        toggle_fold(&mut session, "a", true);
        assert!(!is_folded(&session, "a", 0));
        // A section past the cap, opened by hand, stays open.
        toggle_fold(&mut session, "z", true);
        assert!(!is_folded(&session, "z", OPEN_SECTIONS + 5));
    }

    #[test]
    fn collapse_all_and_expand_all_cover_every_file() {
        let mut session = ChangesSession::default();
        let all = vec!["a".to_string(), "b".to_string()];
        fold_all(&mut session, &all);
        assert!(is_folded(&session, "a", 0) && is_folded(&session, "b", 1));
        unfold_all(&mut session, &all);
        assert!(!is_folded(&session, "a", 0) && !is_folded(&session, "b", OPEN_SECTIONS + 9));
    }

    fn open(kinds: &[FileClass]) -> HashSet<FileClass> {
        kinds.iter().copied().collect()
    }

    #[test]
    fn with_the_split_off_the_rows_are_as_before() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("dist/x.js")];
        let off = tree_rows(&files, true, &HashSet::new(), false, &HashSet::new());
        assert_eq!(paths(&off), vec!["dist/", " dist/x.js", "tests/", " tests/a.rs", "b.txt"]);
    }

    #[test]
    fn the_split_lists_normal_files_then_a_closed_section_per_group() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("dist/x.js"), file("src/c.txt")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &HashSet::new());
        assert_eq!(paths(&rows), vec!["src/", " src/c.txt", "b.txt", "section:Test:1:closed", "section:Generated:1:closed"]);
    }

    #[test]
    fn an_open_section_lists_its_own_folder_tree_one_level_in() {
        let files = vec![file("b.txt"), file("tests/a.rs"), file("src/x.test.ts"), file("dist/x.js")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &open(&[FileClass::Test]));
        assert_eq!(
            paths(&rows),
            vec!["b.txt", "section:Test:2:open", " src/", "  src/x.test.ts", " tests/", "  tests/a.rs", "section:Generated:1:closed"]
        );
    }

    #[test]
    fn an_empty_section_has_no_row() {
        let files = vec![file("a.txt"), file("b.txt")];
        let rows = tree_rows(&files, true, &HashSet::new(), true, &open(&[FileClass::Test, FileClass::Generated]));
        assert_eq!(paths(&rows), vec!["a.txt", "b.txt"]);
    }

    #[test]
    fn a_flat_list_splits_too() {
        let files = vec![file("z.txt"), file("a_test.go"), file("a.txt")];
        let rows = tree_rows(&files, false, &HashSet::new(), true, &open(&[FileClass::Test]));
        assert_eq!(paths(&rows), vec!["a.txt", "z.txt", "section:Test:1:open", " a_test.go"]);
    }

    #[test]
    fn the_stack_shows_every_file_normal_then_test_then_generated() {
        let files = vec![file("dist/x.js"), file("tests/a.rs"), file("b.txt"), file("src/c.txt")];
        // Indexes: 0 dist, 1 tests, 2 b, 3 src/c. Grouped tree order inside each group.
        assert_eq!(stack_order(&files, true, true), vec![3, 2, 1, 0]);
        assert_eq!(stack_order(&files, true, false), vec![0, 3, 1, 2]);
    }

    #[test]
    fn a_click_on_a_section_row_opens_and_closes_it() {
        let mut session = ChangesSession::default();
        toggle_section(&mut session, FileClass::Test);
        assert!(session.open_sections.contains(&FileClass::Test));
        toggle_section(&mut session, FileClass::Test);
        assert!(session.open_sections.is_empty());
    }
}
