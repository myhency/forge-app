use std::path::{Component, Path};

/// `path` relative to `folder` when it is a file inside it, else `None`. Both are cleaned lexically
/// (no disk access): `.` is dropped and `..` goes up one level, and a path that climbs out of the
/// folder, a sibling that only shares a name prefix (`/work/app2` against `/work/app`), a relative
/// path, the folder itself and an empty string are all `None`.
pub fn relative_to_folder(folder: &Path, path: &str) -> Option<String> {
    let cleaned = clean(Path::new(path))?;
    let folder = clean(folder)?;
    let relative = cleaned.strip_prefix(&folder).ok()?;
    let text = relative.to_string_lossy().replace('\\', "/");
    (!text.is_empty()).then_some(text)
}

// An absolute path without `.` and `..` components; `None` when it is relative or climbs above the root.
fn clean(path: &Path) -> Option<std::path::PathBuf> {
    if !path.is_absolute() {
        return None;
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in path.components() {
        match component {
            Component::RootDir | Component::Prefix(_) | Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::Normal(part) => parts.push(part),
        }
    }
    let mut cleaned = std::path::PathBuf::from("/");
    cleaned.extend(parts);
    Some(cleaned)
}

/// Whether the file is shown rendered by default: `.md` and `.markdown`, any case.
pub fn is_markdown(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, extension)) => !stem.is_empty() && matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn a_file_inside_the_folder_gets_a_relative_path() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/src/main.rs").as_deref(), Some("src/main.rs"));
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/a.txt").as_deref(), Some("a.txt"));
        assert_eq!(relative_to_folder(Path::new("/work/app/"), "/work/app/a.txt").as_deref(), Some("a.txt"));
    }

    #[test]
    fn a_path_outside_the_folder_gets_none() {
        for outside in ["/work/other/a.txt", "/etc/passwd", "/work/app2/x.txt", "/work", "relative/a.txt", ""] {
            assert_eq!(relative_to_folder(Path::new("/work/app"), outside), None, "{outside}");
        }
    }

    #[test]
    fn dot_dot_cannot_climb_out_of_the_folder() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/../secret.txt"), None);
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/a/../b.txt").as_deref(), Some("b.txt"));
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/../app/c.txt").as_deref(), Some("c.txt"));
    }

    #[test]
    fn the_folder_itself_is_not_a_file() {
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app"), None);
        assert_eq!(relative_to_folder(Path::new("/work/app"), "/work/app/"), None);
    }

    #[test]
    fn markdown_files_are_told_by_their_extension() {
        for yes in ["README.md", "a/b/Guide.MD", "notes.markdown", "x.Markdown"] {
            assert!(is_markdown(yes), "{yes}");
        }
        for no in ["main.rs", "md", "a.md.txt", "notes.mdx", "", "dir/"] {
            assert!(!is_markdown(no), "{no}");
        }
    }
}
