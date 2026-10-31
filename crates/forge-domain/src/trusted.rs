//! The folders the person trusts. A folder on the list, and every folder below it, may be added to
//! a session without a question. Paths are compared as text (`parts`), so the Windows rules
//! (case, `\`, `\\?\`) are tested on every platform; `windows` says which rules apply.

use std::path::{Path, PathBuf};

use crate::is_absolute_path;

/// Whether `path` is a listed folder or lies below one. A relative path or entry never matches.
pub fn is_trusted(list: &[PathBuf], path: &Path) -> bool {
    is_trusted_on(list, path, cfg!(windows))
}

/// Puts `path` on the list unless it is already trusted; entries below it go, so the list stays
/// short. Returns whether the list changed. A relative path is refused.
pub fn add_trusted(list: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    add_trusted_on(list, path, cfg!(windows))
}

/// The path without the `\\?\` (or `\\?\UNC\`) prefix `canonicalize` returns on Windows; any other path as it is.
pub fn without_extended_prefix(path: &Path) -> PathBuf {
    PathBuf::from(strip_extended_prefix(&path.to_string_lossy()))
}

fn is_trusted_on(list: &[PathBuf], path: &Path, windows: bool) -> bool {
    let Some(child) = parts(path, windows) else {
        return false;
    };
    list.iter().filter_map(|entry| parts(entry, windows)).any(|parent| child.starts_with(&parent))
}

fn add_trusted_on(list: &mut Vec<PathBuf>, path: PathBuf, windows: bool) -> bool {
    let Some(path) = tidy_path(&path, windows) else {
        return false;
    };
    if is_trusted_on(list, &path, windows) {
        return false;
    }
    let new = parts(&path, windows).unwrap_or_default();
    list.retain(|entry| parts(entry, windows).map_or(true, |existing| !existing.starts_with(&new)));
    list.push(path);
    true
}

// An absolute path read as text: how it starts (`prefix`), its parts with their case, and whether it
// begins with a drive. `.` and empty parts go; `..` removes the part before it but never a drive or a
// share. `None` for a relative path, and on Windows for a root that names nothing (`\`, `\\srv`).
struct Tidy {
    prefix: &'static str,
    parts: Vec<String>,
    drive: bool,
}

fn tidy(path: &Path, windows: bool) -> Option<Tidy> {
    let text = path.to_string_lossy();
    let text = if windows { strip_extended_prefix(&text) } else { text.to_string() };
    let (prefix, floor, drive) = if !windows {
        if !text.starts_with('/') {
            return None;
        }
        ("/", 0, false)
    } else if !is_absolute_path(&text) {
        return None;
    } else if text.starts_with("\\\\") || text.starts_with("//") {
        ("\\\\", 2, false)
    } else if text.starts_with(|c| c == '/' || c == '\\') {
        ("\\", 0, false)
    } else {
        ("", 1, true)
    };
    let mut parts: Vec<String> = Vec::new();
    for part in text.split(|c: char| c == '/' || (windows && c == '\\')) {
        match part {
            "" | "." => {}
            ".." => {
                if parts.len() > floor {
                    parts.pop();
                }
            }
            other => parts.push(other.to_string()),
        }
    }
    if windows && parts.len() < floor.max(1) {
        return None;
    }
    Some(Tidy { prefix, parts, drive })
}

// The comparable parts: lower case on Windows.
fn parts(path: &Path, windows: bool) -> Option<Vec<String>> {
    let tidy = tidy(path, windows)?;
    Some(if windows { tidy.parts.iter().map(|part| part.to_lowercase()).collect() } else { tidy.parts })
}

// The path as it is stored: no `.`, no doubled or trailing separators, no extended prefix, the case kept.
fn tidy_path(path: &Path, windows: bool) -> Option<PathBuf> {
    let tidy = tidy(path, windows)?;
    let separator = if windows { "\\" } else { "/" };
    let mut text = String::from(tidy.prefix);
    text.push_str(&tidy.parts.join(separator));
    if tidy.drive && tidy.parts.len() == 1 {
        text.push_str(separator);
    }
    Some(PathBuf::from(text))
}

// `\\?\C:\a` is `C:\a` and `\\?\UNC\srv\share` is `\\srv\share`: the form `canonicalize` returns on Windows.
fn strip_extended_prefix(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("\\\\?\\UNC\\") {
        format!("\\\\{rest}")
    } else if let Some(rest) = path.strip_prefix("\\\\?\\") {
        rest.to_string()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_is_stored_tidied_with_its_case_kept() {
        let tidy_on = |path: &str, windows: bool| {
            let mut folders = Vec::new();
            assert!(add_trusted_on(&mut folders, path.into(), windows), "{path}");
            folders.remove(0)
        };
        assert_eq!(tidy_on("/work/app/", false), PathBuf::from("/work/app"));
        assert_eq!(tidy_on("/work/./a//b/../c", false), PathBuf::from("/work/a/c"));
        assert_eq!(tidy_on("/", false), PathBuf::from("/"));
        assert_eq!(tidy_on("C:\\Work\\App\\", true), PathBuf::from("C:\\Work\\App"));
        assert_eq!(tidy_on("c:/Work/./App", true), PathBuf::from("c:\\Work\\App"));
        assert_eq!(tidy_on("C:\\", true), PathBuf::from("C:\\"));
        assert_eq!(tidy_on("\\\\srv\\Share\\a\\", true), PathBuf::from("\\\\srv\\Share\\a"));
        assert_eq!(tidy_on("\\\\?\\C:\\Work", true), PathBuf::from("C:\\Work"), "the extended prefix is not stored");
    }

    #[test]
    fn a_windows_root_with_no_drive_and_a_server_with_no_share_are_refused() {
        for entry in ["\\", "/", "\\\\srv", "\\\\"] {
            assert!(!windows(&[entry], "C:\\work"), "{entry}");
            assert!(!windows(&[entry], "\\\\srv\\share\\a"), "{entry}");
            let mut folders = Vec::new();
            assert!(!add_trusted_on(&mut folders, entry.into(), true), "{entry}");
            assert!(folders.is_empty());
        }
        assert!(windows(&["\\\\srv\\share"], "\\\\srv\\share\\a"), "a share is fine");
    }

    #[test]
    fn the_extended_prefix_can_be_taken_off_a_path() {
        assert_eq!(without_extended_prefix(Path::new("\\\\?\\C:\\a")), PathBuf::from("C:\\a"));
        assert_eq!(without_extended_prefix(Path::new("\\\\?\\UNC\\srv\\sh\\a")), PathBuf::from("\\\\srv\\sh\\a"));
        assert_eq!(without_extended_prefix(Path::new("C:\\a")), PathBuf::from("C:\\a"));
        assert_eq!(without_extended_prefix(Path::new("/a/b")), PathBuf::from("/a/b"));
    }

    fn list(paths: &[&str]) -> Vec<PathBuf> {
        paths.iter().map(PathBuf::from).collect()
    }

    fn unix(entries: &[&str], path: &str) -> bool {
        is_trusted_on(&list(entries), Path::new(path), false)
    }

    fn windows(entries: &[&str], path: &str) -> bool {
        is_trusted_on(&list(entries), Path::new(path), true)
    }

    #[test]
    fn a_listed_folder_and_the_folders_below_it_are_trusted() {
        assert!(unix(&["/work/app"], "/work/app"));
        assert!(unix(&["/work/app"], "/work/app/src/deep"));
        assert!(!unix(&["/work/app"], "/work"));
        assert!(!unix(&[], "/work/app"));
    }

    #[test]
    fn a_sibling_that_starts_with_the_same_name_is_not_trusted() {
        assert!(!unix(&["/work/app"], "/work/app-old"));
        assert!(!unix(&["/work/app"], "/work/application/src"));
        assert!(!windows(&["C:\\work\\app"], "C:\\work\\app-old"));
    }

    #[test]
    fn a_trailing_separator_and_dot_parts_change_nothing() {
        assert!(unix(&["/work/app/"], "/work/app"));
        assert!(unix(&["/work/app"], "/work/app/"));
        assert!(unix(&["/work/app"], "/work/./app//src"));
    }

    #[test]
    fn dot_dot_cannot_walk_out_of_a_trusted_folder() {
        assert!(!unix(&["/work/app"], "/work/app/../secret"));
        assert!(unix(&["/work/app"], "/work/app/src/../lib"));
        assert!(!unix(&["/work/app"], "/../../etc"));
        assert!(!windows(&["C:\\work"], "C:\\work\\..\\Windows"));
    }

    #[test]
    fn dot_dot_cannot_leave_a_windows_drive_or_share() {
        assert!(!windows(&["D:\\"], "C:\\..\\..\\x"), "C:\\.. stays on C:");
        assert!(!windows(&["\\\\srv\\share\\a"], "\\\\srv\\share\\..\\..\\b"));
        assert!(windows(&["C:\\"], "C:\\anything\\below"));
    }

    #[test]
    fn windows_ignores_case_and_the_kind_of_slash_but_other_systems_do_not() {
        assert!(windows(&["C:\\Work\\App"], "c:/work/app/src"));
        assert!(!unix(&["/Work/App"], "/work/app/src"));
        assert!(!unix(&["/work/app"], "/work\\app"), "a backslash is a name character outside Windows");
    }

    #[test]
    fn the_extended_windows_prefix_is_ignored() {
        assert!(windows(&["C:\\work"], "\\\\?\\C:\\work\\app"));
        assert!(windows(&["\\\\?\\C:\\work"], "C:\\work\\app"));
        assert!(windows(&["\\\\srv\\share\\a"], "\\\\?\\UNC\\srv\\share\\a\\b"));
    }

    #[test]
    fn a_relative_path_or_entry_is_never_trusted() {
        assert!(!unix(&["/work"], "work/app"));
        assert!(!unix(&["work"], "work/app"));
        assert!(!unix(&["work"], "/work/app"));
        assert!(!unix(&[""], "/work"));
        assert!(!windows(&["C:work"], "C:\\work"));
    }

    #[test]
    fn adding_a_folder_appends_it_once() {
        let mut folders = list(&["/a"]);
        assert!(add_trusted_on(&mut folders, "/b".into(), false));
        assert!(!add_trusted_on(&mut folders, "/b".into(), false), "the same folder twice");
        assert_eq!(folders, list(&["/a", "/b"]));
    }

    #[test]
    fn a_folder_below_a_trusted_one_is_not_added() {
        let mut folders = list(&["/work"]);
        assert!(!add_trusted_on(&mut folders, "/work/app".into(), false));
        assert_eq!(folders, list(&["/work"]));
    }

    #[test]
    fn adding_a_parent_removes_the_entries_below_it() {
        let mut folders = list(&["/work/app", "/other", "/work/lib/src"]);
        assert!(add_trusted_on(&mut folders, "/work".into(), false));
        assert_eq!(folders, list(&["/other", "/work"]));
    }

    #[test]
    fn a_relative_folder_is_refused() {
        let mut folders = Vec::new();
        assert!(!add_trusted_on(&mut folders, "work".into(), false));
        assert!(folders.is_empty());
    }
}
