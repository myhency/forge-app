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

fn is_trusted_on(list: &[PathBuf], path: &Path, windows: bool) -> bool {
    let Some(child) = parts(path, windows) else {
        return false;
    };
    list.iter().filter_map(|entry| parts(entry, windows)).any(|parent| child.starts_with(&parent))
}

fn add_trusted_on(list: &mut Vec<PathBuf>, path: PathBuf, windows: bool) -> bool {
    let Some(new) = parts(&path, windows) else {
        return false;
    };
    if is_trusted_on(list, &path, windows) {
        return false;
    }
    list.retain(|entry| parts(entry, windows).map_or(true, |existing| !existing.starts_with(&new)));
    list.push(path);
    true
}

// The comparable parts of an absolute path, or `None` for a relative one. `.` and empty parts go;
// `..` removes the part before it but never a drive or a share. On Windows the parts are lower case.
fn parts(path: &Path, windows: bool) -> Option<Vec<String>> {
    let text = path.to_string_lossy();
    let text = if windows { strip_extended_prefix(&text) } else { text.to_string() };
    let absolute = if windows { is_absolute_path(&text) } else { text.starts_with('/') };
    if !absolute {
        return None;
    }
    let floor = if !windows {
        0
    } else if text.starts_with("\\\\") || text.starts_with("//") {
        2
    } else if text.as_bytes().get(1) == Some(&b':') {
        1
    } else {
        0
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
            other => parts.push(if windows { other.to_lowercase() } else { other.to_string() }),
        }
    }
    Some(parts)
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
