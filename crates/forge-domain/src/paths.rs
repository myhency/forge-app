//! Paths held as text (a tool call's `file_path`, a link in the chat, a path in the Files panel).
//! Such a path comes from Claude or the page, not from this machine's `Path`, so it is read the
//! same way on every platform: `/` and `\` both separate, and a drive (`C:\`, `C:/`) or a share
//! (`\\server`) makes it absolute.

fn is_separator(c: char) -> bool {
    c == '/' || c == '\\'
}

/// Whether `path` starts at a root: `/dir`, `C:\dir`, `C:/dir` or `\\server\share`.
pub fn is_absolute_path(path: &str) -> bool {
    let mut chars = path.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some(first), _, _) if is_separator(first) => true,
        (Some(drive), Some(':'), Some(separator)) => drive.is_ascii_alphabetic() && is_separator(separator),
        _ => false,
    }
}

/// The last part of `path`, split at `/` or `\`.
pub fn file_name(path: &str) -> &str {
    path.rsplit(is_separator).next().unwrap_or(path)
}

/// The parts of `path` between separators, without the empty ones.
pub fn path_parts(path: &str) -> impl Iterator<Item = &str> {
    path.split(is_separator).filter(|part| !part.is_empty())
}

/// Whether some part of `path` is `..`.
pub fn has_parent_part(path: &str) -> bool {
    path.split(is_separator).any(|part| part == "..")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_root_makes_a_path_absolute_on_either_platform() {
        for path in ["/usr/a.md", "C:\\Users\\a.md", "c:/Users/a.md", "\\\\server\\share\\a.md"] {
            assert!(is_absolute_path(path), "{path}");
        }
        for path in ["", "a/b.md", "C:a.md", "C:", "~/a.md", "1:/a", "relative\\a.md"] {
            assert!(!is_absolute_path(path), "{path}");
        }
    }

    #[test]
    fn the_file_name_is_what_follows_the_last_separator() {
        assert_eq!(file_name("/a/b/c.rs"), "c.rs");
        assert_eq!(file_name("C:\\a\\b\\c.rs"), "c.rs");
        assert_eq!(file_name("a/b\\c.rs"), "c.rs");
        assert_eq!(file_name("c.rs"), "c.rs");
        assert_eq!(file_name("/a/b/"), "");
    }

    #[test]
    fn parts_skip_empty_ones_and_parent_parts_are_found_after_either_separator() {
        assert_eq!(path_parts("/a//b\\c").collect::<Vec<_>>(), ["a", "b", "c"]);
        assert!(has_parent_part("/a/../b"));
        assert!(has_parent_part("C:\\a\\..\\b"));
        assert!(!has_parent_part("/a/..b/c"));
    }
}
