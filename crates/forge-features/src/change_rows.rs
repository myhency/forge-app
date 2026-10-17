use std::collections::{BTreeMap, HashSet};

use forge_ports::{ChangeSource, ChangeStatus, ChangedFile};

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
}

/// All changes with no base to compare with are the uncommitted changes.
pub fn source_for(view: &ChangeView, base: Option<&str>) -> ChangeSource {
    match (view, base) {
        (ChangeView::All, Some(base)) => ChangeSource::All { base: base.to_string() },
        (ChangeView::All, None) | (ChangeView::Uncommitted, _) => ChangeSource::Uncommitted,
        (ChangeView::Commit(sha), _) => ChangeSource::Commit { sha: sha.clone() },
    }
}

/// The folder (no trailing slash, empty at the top) and the name of a path.
pub fn split_folder(path: &str) -> (String, String) {
    match path.rsplit_once('/') {
        Some((folder, name)) => (folder.to_string(), name.to_string()),
        None => (String::new(), path.to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeRow {
    Folder { path: String, name: String, depth: usize, open: bool },
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

fn build(files: &[ChangedFile], group: bool, closed: &HashSet<String>, respect_closed: bool) -> Vec<TreeRow> {
    if !group {
        let mut order: Vec<usize> = (0..files.len()).collect();
        order.sort_by(|a, b| files[*a].path.cmp(&files[*b].path));
        return order.into_iter().map(|index| file_row(files, index, 0)).collect();
    }
    let mut root = Node::default();
    for (index, file) in files.iter().enumerate() {
        let mut node = &mut root;
        let mut parts: Vec<&str> = file.path.split('/').collect();
        parts.pop();
        for part in parts {
            node = node.folders.entry(part.to_string()).or_default();
        }
        node.files.push(index);
    }
    let mut rows = Vec::new();
    walk(&root, "", 0, files, closed, respect_closed, &mut rows);
    rows
}

/// The rows of the right column: folders (folders first, then files, each by name) or a flat list.
pub fn tree_rows(files: &[ChangedFile], group: bool, closed: &HashSet<String>) -> Vec<TreeRow> {
    build(files, group, closed, true)
}

/// The order of the stacked diffs: the tree's order with every folder open.
pub fn stack_order(files: &[ChangedFile], group: bool) -> Vec<usize> {
    build(files, group, &HashSet::new(), false)
        .into_iter()
        .filter_map(|row| match row {
            TreeRow::File { index, .. } => Some(index),
            TreeRow::Folder { .. } => None,
        })
        .collect()
}

/// The files whose path holds `query` (no case); every file for an empty query.
pub fn filter_files(files: &[ChangedFile], query: &str) -> Vec<usize> {
    let query = query.to_lowercase();
    (0..files.len()).filter(|index| query.is_empty() || files[*index].path.to_lowercase().contains(&query)).collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> ChangedFile {
        ChangedFile { path: path.to_string(), status: ChangeStatus::Modified, added: Some(1), removed: Some(0) }
    }

    fn paths(rows: &[TreeRow]) -> Vec<String> {
        rows.iter()
            .map(|row| match row {
                TreeRow::Folder { path, depth, .. } => format!("{}{path}/", " ".repeat(*depth)),
                TreeRow::File { path, depth, .. } => format!("{}{path}", " ".repeat(*depth)),
            })
            .collect()
    }

    #[test]
    fn a_grouped_tree_lists_folders_first_then_files() {
        let files = vec![file("b.txt"), file("src/z.rs"), file("a.txt"), file("src/a.rs"), file("docs/x/y.md")];
        let rows = tree_rows(&files, true, &HashSet::new());
        assert_eq!(paths(&rows), vec!["docs/", " docs/x/", "  docs/x/y.md", "src/", " src/a.rs", " src/z.rs", "a.txt", "b.txt"]);
    }

    #[test]
    fn a_flat_list_is_by_path() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        let rows = tree_rows(&files, false, &HashSet::new());
        assert_eq!(paths(&rows), vec!["a.txt", "b.txt", "src/a.rs"]);
    }

    #[test]
    fn a_closed_folder_hides_what_is_inside_it() {
        let files = vec![file("src/a.rs"), file("a.txt")];
        let closed: HashSet<String> = ["src".to_string()].into();
        assert_eq!(paths(&tree_rows(&files, true, &closed)), vec!["src/", "a.txt"]);
    }

    #[test]
    fn the_stack_follows_the_tree_order_even_when_a_folder_is_closed() {
        let files = vec![file("b.txt"), file("src/a.rs"), file("a.txt")];
        assert_eq!(stack_order(&files, true), vec![1, 2, 0]);
        assert_eq!(stack_order(&files, false), vec![2, 0, 1]);
    }

    #[test]
    fn a_file_row_knows_its_folder_and_name() {
        let rows = tree_rows(&[file("src/deep/a.rs")], false, &HashSet::new());
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
}
