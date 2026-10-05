use std::collections::{HashMap, HashSet};

use forge_ports::DirEntry;

/// What the Files filter field holds: a name filter, or, after a leading `?`, a text to search for
/// inside the files.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesQuery<'a> {
    Filter(&'a str),
    Content(&'a str),
}

pub fn parse_files_query(input: &str) -> FilesQuery<'_> {
    match input.trim_start().strip_prefix('?') {
        Some(rest) => FilesQuery::Content(rest.trim()),
        None => FilesQuery::Filter(input.trim()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeRow {
    /// From the session folder, `/`-separated.
    pub path: String,
    pub name: String,
    pub depth: usize,
    pub is_dir: bool,
    pub expanded: bool,
}

/// The folders the Files panel has read so far and which of them are open. Folders are read when
/// they are first opened; the root (`""`) is always open.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FileTree {
    children: HashMap<String, Vec<DirEntry>>,
    open: HashSet<String>,
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

impl FileTree {
    pub fn set_children(&mut self, dir: &str, entries: Vec<DirEntry>) {
        self.children.insert(dir.to_string(), entries);
    }

    pub fn is_loaded(&self, dir: &str) -> bool {
        self.children.contains_key(dir)
    }

    /// Opens a closed folder or closes an open one. True when the folder is now open and its
    /// entries have not been read yet, so the caller must read them.
    pub fn toggle(&mut self, dir: &str) -> bool {
        if self.open.remove(dir) {
            return false;
        }
        self.open.insert(dir.to_string());
        !self.is_loaded(dir)
    }

    /// The folders above `path`, outermost first (`src/deep/a.txt` gives `src`, `src/deep`).
    pub fn reveal_dirs(path: &str) -> Vec<String> {
        let parts: Vec<&str> = path.split('/').filter(|part| !part.is_empty()).collect();
        (1..parts.len()).map(|n| parts[..n].join("/")).collect()
    }

    /// Opens a folder (never closes one). True when its entries have not been read yet.
    pub fn open_dir(&mut self, dir: &str) -> bool {
        self.open.insert(dir.to_string());
        !self.is_loaded(dir)
    }

    /// The visible rows, depth first. With a filter, only the entries whose name contains it (any
    /// case) and the folders that hold such an entry among what has been read; those folders show
    /// open.
    pub fn rows(&self, filter: &str) -> Vec<TreeRow> {
        let mut rows = Vec::new();
        self.collect("", 0, &filter.to_lowercase(), &mut rows);
        rows
    }

    fn matches(name: &str, filter: &str) -> bool {
        name.to_lowercase().contains(filter)
    }

    // Whether anything read below `dir` has a name that matches.
    fn holds_match(&self, dir: &str, filter: &str) -> bool {
        self.children.get(dir).is_some_and(|entries| {
            entries.iter().any(|entry| Self::matches(&entry.name, filter) || (entry.is_dir && self.holds_match(&join(dir, &entry.name), filter)))
        })
    }

    fn collect(&self, dir: &str, depth: usize, filter: &str, rows: &mut Vec<TreeRow>) {
        let Some(entries) = self.children.get(dir) else { return };
        for entry in entries {
            let path = join(dir, &entry.name);
            let (show, expanded) = if filter.is_empty() {
                (true, entry.is_dir && self.open.contains(&path))
            } else {
                let holds = entry.is_dir && self.holds_match(&path, filter);
                (Self::matches(&entry.name, filter) || holds, holds)
            };
            if !show {
                continue;
            }
            rows.push(TreeRow { path: path.clone(), name: entry.name.clone(), depth, is_dir: entry.is_dir, expanded });
            if expanded {
                self.collect(&path, depth + 1, filter, rows);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use forge_ports::DirEntry;

    use super::*;

    fn dir(name: &str) -> DirEntry {
        DirEntry { name: name.to_string(), is_dir: true }
    }

    fn file(name: &str) -> DirEntry {
        DirEntry { name: name.to_string(), is_dir: false }
    }

    fn paths(rows: &[TreeRow]) -> Vec<(&str, usize)> {
        rows.iter().map(|row| (row.path.as_str(), row.depth)).collect()
    }

    fn sample() -> FileTree {
        let mut tree = FileTree::default();
        tree.set_children("", vec![dir("src"), file("README.md")]);
        tree
    }

    #[test]
    fn the_root_entries_show_and_folders_start_closed() {
        let tree = sample();
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(!tree.rows("")[0].expanded);
    }

    #[test]
    fn opening_a_folder_asks_for_its_entries_once() {
        let mut tree = sample();
        assert!(tree.toggle("src"), "not loaded yet");
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)], "nothing to show until loaded");
        tree.set_children("src", vec![file("main.rs"), dir("deep")]);
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("src/main.rs", 1), ("src/deep", 1), ("README.md", 0)]);
        assert!(!tree.toggle("src"), "closing needs no load");
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(!tree.toggle("src"), "opening again: already loaded");
        assert_eq!(tree.rows("").len(), 4);
    }

    #[test]
    fn an_empty_folder_stays_open_with_no_rows_under_it() {
        let mut tree = sample();
        tree.toggle("src");
        tree.set_children("src", vec![]);
        assert!(tree.is_loaded("src"));
        assert_eq!(paths(&tree.rows("")), vec![("src", 0), ("README.md", 0)]);
        assert!(tree.rows("")[0].expanded);
    }

    #[test]
    fn the_filter_keeps_names_that_match_in_any_case_and_the_folders_that_hold_them() {
        let mut tree = sample();
        tree.toggle("src");
        tree.set_children("src", vec![file("main.rs"), file("lib.rs")]);
        assert_eq!(paths(&tree.rows("MAIN")), vec![("src", 0), ("src/main.rs", 1)]);
        assert_eq!(paths(&tree.rows("readme")), vec![("README.md", 0)]);
        assert_eq!(paths(&tree.rows("nothing")), Vec::<(&str, usize)>::new());
    }

    #[test]
    fn a_filter_shows_matches_in_folders_that_were_closed() {
        let mut tree = sample();
        tree.set_children("src", vec![file("main.rs")]);
        let rows = tree.rows("main");
        assert_eq!(paths(&rows), vec![("src", 0), ("src/main.rs", 1)]);
        assert!(rows[0].expanded);
    }

    #[test]
    fn a_question_mark_starts_a_content_search_and_anything_else_is_a_name_filter() {
        assert_eq!(parse_files_query("?hello world "), FilesQuery::Content("hello world"));
        assert_eq!(parse_files_query("  ?x"), FilesQuery::Content("x"));
        assert_eq!(parse_files_query("?"), FilesQuery::Content(""));
        assert_eq!(parse_files_query(" main "), FilesQuery::Filter("main"));
        assert_eq!(parse_files_query(""), FilesQuery::Filter(""));
    }

    #[test]
    fn the_ancestors_of_a_path_come_outermost_first() {
        assert_eq!(FileTree::reveal_dirs("src/deep/a.txt"), vec!["src".to_string(), "src/deep".to_string()]);
        assert_eq!(FileTree::reveal_dirs("a.txt"), Vec::<String>::new());
        assert_eq!(FileTree::reveal_dirs(""), Vec::<String>::new());
    }

    #[test]
    fn opening_a_folder_twice_keeps_it_open_and_asks_for_a_read_only_until_it_is_read() {
        let mut tree = FileTree::default();
        tree.set_children("", vec![dir("src")]);
        assert!(tree.open_dir("src"));
        assert!(tree.open_dir("src"));
        tree.set_children("src", vec![file("a.txt")]);
        assert!(!tree.open_dir("src"));
        // `toggle` would have closed it; `open_dir` never does.
        let rows = tree.rows("");
        assert!(rows[0].expanded);
        assert_eq!(rows[1].path, "src/a.txt");
    }
}
