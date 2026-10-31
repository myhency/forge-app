use std::collections::HashMap;

use forge_ports::{ChangeStatus, PatchState};

/// git's empty tree: what a root commit, or a repository with no commit, is compared with.
pub(crate) const EMPTY_TREE: &str = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
/// One `changes` call keeps at most this much patch text.
pub(crate) const PATCH_BUDGET: usize = 2 * 1024 * 1024;

pub(crate) enum Count {
    Lines(usize, usize),
    Binary,
}

/// A branch or commit name goes to git without `--`, so one that looks like an option is refused.
pub(crate) fn safe_ref(name: &str) -> bool {
    !name.is_empty() && !name.starts_with('-')
}

// `git diff --name-status -z --no-renames`: `S NUL path NUL` per file.
pub(crate) fn parse_name_status(bytes: &[u8]) -> Vec<(ChangeStatus, String)> {
    let text = String::from_utf8_lossy(bytes);
    let mut parts = text.split('\0');
    let mut files = Vec::new();
    while let Some(code) = parts.next() {
        if code.is_empty() {
            continue;
        }
        let Some(path) = parts.next() else { break };
        let status = match code.as_bytes()[0] {
            b'A' => ChangeStatus::Added,
            b'D' => ChangeStatus::Deleted,
            b'U' => ChangeStatus::Conflicted,
            _ => ChangeStatus::Modified,
        };
        files.push((status, path.to_string()));
    }
    files
}

// `git diff --numstat -z --no-renames`: `added TAB removed TAB path NUL`; `-` for a binary file.
pub(crate) fn parse_counts(bytes: &[u8]) -> HashMap<String, Count> {
    let text = String::from_utf8_lossy(bytes);
    let mut counts = HashMap::new();
    for entry in text.split('\0') {
        let mut fields = entry.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (fields.next(), fields.next(), fields.next()) else { continue };
        match (added.parse(), removed.parse()) {
            (Ok(added), Ok(removed)) => counts.insert(path.to_string(), Count::Lines(added, removed)),
            _ if added == "-" => counts.insert(path.to_string(), Count::Binary),
            _ => None,
        };
    }
    counts
}

/// Whether git writes this name in quotes in a diff header (even with `core.quotepath=off`).
pub(crate) fn is_quoted_name(path: &str) -> bool {
    path.chars().any(|c| c == '"' || c == '\\' || c.is_control())
}

// `P b/P` of a `diff --git a/P b/P` line; the diff is made with --no-renames so both names agree.
fn header_path(rest: &str) -> Option<String> {
    let names = rest.len().checked_sub(3)?;
    if names % 2 != 0 {
        return None;
    }
    let path = rest.get(..names / 2)?;
    (rest.get(names / 2..)? == format!(" b/{path}")).then(|| path.to_string())
}

/// The patch of each file, by path. A chunk whose header cannot be read (a name git had to quote)
/// is dropped; the caller then treats the file as having no text patch.
pub(crate) fn split_patch(text: &str) -> HashMap<String, String> {
    let mut parts = HashMap::new();
    let mut current: Option<(String, String)> = None;
    for line in text.split_inclusive('\n') {
        // A body line starts with space, `+`, `-` or `\`, so this only ever matches a header.
        if let Some(rest) = line.strip_prefix("diff --git ") {
            if let Some((path, chunk)) = current.take() {
                parts.insert(path, chunk);
            }
            // A quoted name (`"a/..."`) does not parse: its chunk is dropped, not glued to the last file.
            current = rest
                .strip_prefix("a/")
                .and_then(|rest| header_path(rest.trim_end_matches('\n')))
                .map(|path| (path, String::new()));
        }
        if let Some((_, chunk)) = current.as_mut() {
            chunk.push_str(line);
        }
    }
    if let Some((path, chunk)) = current {
        parts.insert(path, chunk);
    }
    parts
}

/// What `git diff --no-index /dev/null file` would show, made here so a thousand untracked files
/// do not start a thousand processes. Returns the state and the number of added lines.
pub(crate) fn untracked_patch(bytes: &[u8], budget: usize) -> (PatchState, usize) {
    if bytes.iter().take(8000).any(|byte| *byte == 0) {
        return (PatchState::Binary, 0);
    }
    let text = String::from_utf8_lossy(bytes);
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return (PatchState::Text(String::new()), 0);
    }
    let mut patch = format!("@@ -0,0 +1,{} @@\n", lines.len());
    for line in &lines {
        patch.push('+');
        patch.push_str(line);
        patch.push('\n');
    }
    if patch.len() > budget {
        return (PatchState::TooLarge, lines.len());
    }
    (PatchState::Text(patch), lines.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_status_reads_a_letter_and_a_path_per_entry() {
        let files = parse_name_status(b"M\0a.txt\0A\0dir/b c.txt\0D\0gone.txt\0");
        assert_eq!(
            files,
            vec![
                (ChangeStatus::Modified, "a.txt".to_string()),
                (ChangeStatus::Added, "dir/b c.txt".to_string()),
                (ChangeStatus::Deleted, "gone.txt".to_string()),
            ]
        );
    }

    #[test]
    fn counts_mark_a_binary_file() {
        let counts = parse_counts(b"3\t1\ta.txt\0-\t-\timg.png\0");
        assert!(matches!(counts["a.txt"], Count::Lines(3, 1)));
        assert!(matches!(counts["img.png"], Count::Binary));
    }

    #[test]
    fn a_patch_is_split_by_its_diff_header_even_when_the_name_has_spaces() {
        let text = "diff --git a/a b.txt b/a b.txt\n--- a/a b.txt\n+++ b/a b.txt\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/한글.txt b/한글.txt\n--- a/한글.txt\n+++ b/한글.txt\n@@ -1 +1 @@\n-1\n+2\n";
        let parts = split_patch(text);
        assert_eq!(parts.len(), 2);
        assert!(parts["a b.txt"].starts_with("diff --git a/a b.txt b/a b.txt\n"));
        assert!(parts["a b.txt"].ends_with("+y\n"));
        assert!(parts["한글.txt"].contains("+2"));
    }

    #[test]
    fn a_quoted_header_ends_the_previous_patch() {
        let text = "diff --git a/a.txt b/a.txt\n@@ -1 +1 @@\n-x\n+y\ndiff --git \"a/q\\\"t.txt\" \"b/q\\\"t.txt\"\n@@ -1 +1 @@\n-1\n+2\n";
        let parts = split_patch(text);
        assert_eq!(parts.len(), 1);
        assert_eq!(parts["a.txt"], "diff --git a/a.txt b/a.txt\n@@ -1 +1 @@\n-x\n+y\n");
    }

    #[test]
    fn names_git_quotes_are_recognised() {
        assert!(is_quoted_name("q\"t.txt"));
        assert!(is_quoted_name("a\\b"));
        assert!(is_quoted_name("a\tb"));
        assert!(!is_quoted_name("한글 이름.txt"));
    }

    #[test]
    fn a_body_line_that_looks_like_a_header_is_not_a_split_point() {
        let text = "diff --git a/x b/x\n@@ -1 +1 @@\n- diff --git a/y b/y\n+z\n";
        assert_eq!(split_patch(text).len(), 1);
    }

    #[test]
    fn an_untracked_text_file_is_all_added() {
        let (patch, added) = untracked_patch(b"one\ntwo\n", 1000);
        assert_eq!(added, 2);
        assert_eq!(patch, PatchState::Text("@@ -0,0 +1,2 @@\n+one\n+two\n".to_string()));
    }

    #[test]
    fn an_untracked_file_with_a_nul_byte_is_binary() {
        assert_eq!(untracked_patch(b"ab\0cd", 1000).0, PatchState::Binary);
    }

    #[test]
    fn an_untracked_file_over_the_budget_is_too_large() {
        assert_eq!(untracked_patch(b"aaaa\nbbbb\n", 5).0, PatchState::TooLarge);
    }

    #[test]
    fn a_name_that_looks_like_an_option_is_not_a_ref() {
        assert!(safe_ref("main"));
        assert!(!safe_ref("-x"));
        assert!(!safe_ref(""));
    }
}
