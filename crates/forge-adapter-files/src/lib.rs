use std::fs;
use std::path::{Component, Path, PathBuf};

use forge_ports::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};

const SKIPPED: [&str; 2] = [".git", "node_modules"];
// A NUL byte in this many first bytes makes a file binary.
const BINARY_SNIFF: usize = 8000;
// A search looks at no more files than this, so a huge tree cannot keep it busy for long.
const SEARCH_FILE_LIMIT: usize = 20_000;
const HIT_TEXT_CHARS: usize = 200;

/// Reads files under a root with `std::fs`. A path is resolved (symlinks included) and must stay
/// inside the root.
#[derive(Default)]
pub struct LocalFiles;

impl LocalFiles {
    pub fn new() -> Self {
        Self
    }
}

fn failed(error: impl std::fmt::Display) -> FilesError {
    FilesError::Failed(error.to_string())
}

fn resolve(root: &Path, relative: &str) -> Result<PathBuf, FilesError> {
    let root = root.canonicalize().map_err(failed)?;
    let relative = Path::new(relative);
    if relative.is_absolute() || relative.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err(FilesError::Outside);
    }
    let resolved = root.join(relative).canonicalize().map_err(failed)?;
    if resolved.starts_with(&root) {
        Ok(resolved)
    } else {
        Err(FilesError::Outside)
    }
}

fn is_skipped(name: &str) -> bool {
    SKIPPED.contains(&name)
}

fn looks_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(BINARY_SNIFF)].contains(&0)
}

// A folder's entries sorted folders first, then by lower-case name.
fn sorted_entries(dir: &Path) -> Result<Vec<DirEntry>, FilesError> {
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(failed)? {
        let entry = entry.map_err(failed)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_skipped(&name) {
            continue;
        }
        // `metadata` follows a symlink; a broken one counts as a file.
        let is_dir = fs::metadata(entry.path()).map(|meta| meta.is_dir()).unwrap_or(false);
        entries.push(DirEntry { name, is_dir });
    }
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(entries)
}

impl FilesPort for LocalFiles {
    fn list(&self, root: &Path, dir: &str) -> Result<Vec<DirEntry>, FilesError> {
        sorted_entries(&resolve(root, dir)?)
    }

    fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError> {
        let file = resolve(root, path)?;
        let length = fs::metadata(&file).map_err(failed)?.len();
        if length > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        if looks_binary(&bytes) {
            return Ok(FileContent::Binary);
        }
        Ok(FileContent::Text(String::from_utf8_lossy(&bytes).into_owned()))
    }

    fn search(&self, root: &Path, query: &str, limit: usize) -> Result<Vec<ContentHit>, FilesError> {
        let root = root.canonicalize().map_err(failed)?;
        let needle = query.to_lowercase();
        let mut hits = Vec::new();
        if needle.is_empty() || limit == 0 {
            return Ok(hits);
        }
        let mut visited = 0;
        search_dir(&root, "", &needle, limit, &mut visited, &mut hits);
        Ok(hits)
    }
}

// Depth first in name order; symlinks are not followed, so a link cannot lead out of the root.
fn search_dir(dir: &Path, prefix: &str, needle: &str, limit: usize, visited: &mut usize, hits: &mut Vec<ContentHit>) {
    let Ok(read) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = read.filter_map(Result::ok).collect();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        if hits.len() >= limit || *visited >= SEARCH_FILE_LIMIT {
            return;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else { continue };
        if is_skipped(&name) || kind.is_symlink() {
            continue;
        }
        let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if kind.is_dir() {
            search_dir(&entry.path(), &path, needle, limit, visited, hits);
            continue;
        }
        *visited += 1;
        if entry.metadata().map(|meta| meta.len() > READ_LIMIT as u64).unwrap_or(true) {
            continue;
        }
        let Ok(bytes) = fs::read(entry.path()) else { continue };
        if looks_binary(&bytes) {
            continue;
        }
        for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.to_lowercase().contains(needle) {
                let text: String = line.trim().chars().take(HIT_TEXT_CHARS).collect();
                hits.push(ContentHit { path: path.clone(), line: index + 1, text });
                if hits.len() >= limit {
                    return;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;

    use forge_ports::{ContentHit, DirEntry, FileContent, FilesError, FilesPort, READ_LIMIT};
    use tempfile::TempDir;

    use super::*;

    fn tree() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("src/deep")).unwrap();
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        fs::write(root.join("README.md"), "# Title\nhello World\n").unwrap();
        fs::write(root.join("src/main.rs"), "fn main() {\n    println!(\"hello\");\n}\n").unwrap();
        fs::write(root.join("src/deep/notes.txt"), "nothing here\nHELLO again\n").unwrap();
        fs::write(root.join(".git/config"), "hello\n").unwrap();
        fs::write(root.join("node_modules/pkg/index.js"), "hello\n").unwrap();
        dir
    }

    fn names(entries: &[DirEntry]) -> Vec<String> {
        entries.iter().map(|entry| format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" })).collect()
    }

    #[test]
    fn a_folder_lists_folders_first_then_by_name_without_git_or_node_modules() {
        let dir = tree();
        fs::write(dir.path().join("Zed.txt"), "").unwrap();
        fs::write(dir.path().join("apple.txt"), "").unwrap();
        let entries = LocalFiles::new().list(dir.path(), "").unwrap();
        assert_eq!(names(&entries), vec!["src/", "apple.txt", "README.md", "Zed.txt"]);
    }

    #[test]
    fn a_subfolder_is_listed_by_its_relative_path() {
        let dir = tree();
        let entries = LocalFiles::new().list(dir.path(), "src").unwrap();
        assert_eq!(names(&entries), vec!["deep/", "main.rs"]);
    }

    #[test]
    fn an_empty_folder_lists_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(LocalFiles::new().list(dir.path(), "").unwrap(), vec![]);
    }

    #[test]
    fn a_text_file_is_read_with_its_content() {
        let dir = tree();
        assert_eq!(LocalFiles::new().read(dir.path(), "README.md").unwrap(), FileContent::Text("# Title\nhello World\n".to_string()));
    }

    #[test]
    fn a_file_with_a_nul_byte_is_binary_and_a_big_file_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.bin"), [1u8, 2, 0, 3]).unwrap();
        fs::write(dir.path().join("big.txt"), vec![b'a'; READ_LIMIT + 1]).unwrap();
        fs::write(dir.path().join("exact.txt"), vec![b'a'; READ_LIMIT]).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.read(dir.path(), "b.bin").unwrap(), FileContent::Binary);
        assert_eq!(files.read(dir.path(), "big.txt").unwrap(), FileContent::TooLarge);
        assert!(matches!(files.read(dir.path(), "exact.txt").unwrap(), FileContent::Text(_)));
    }

    #[test]
    fn text_that_is_not_utf8_is_read_lossily() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("l.txt"), [b'a', 0xff, b'b']).unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "l.txt").unwrap(), FileContent::Text("a\u{fffd}b".to_string()));
    }

    #[test]
    fn paths_that_leave_the_folder_are_refused() {
        let dir = tree();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path().join("secret.txt"), dir.path().join("link.txt")).unwrap();
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.read(dir.path(), "../secret.txt"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "src/../../x"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "/etc/passwd"), Err(FilesError::Outside));
        assert_eq!(files.read(dir.path(), "link.txt"), Err(FilesError::Outside));
        assert_eq!(files.list(dir.path(), "linkdir"), Err(FilesError::Outside));
        assert_eq!(files.list(dir.path(), ".."), Err(FilesError::Outside));
    }

    #[test]
    fn a_missing_file_is_a_message_not_a_panic() {
        let dir = tree();
        assert!(matches!(LocalFiles::new().read(dir.path(), "nope.txt"), Err(FilesError::Failed(_))));
        assert!(matches!(LocalFiles::new().list(&dir.path().join("gone"), ""), Err(FilesError::Failed(_))));
    }

    #[test]
    fn search_finds_lines_in_any_case_in_path_order_and_skips_git_and_node_modules() {
        let dir = tree();
        let hits = LocalFiles::new().search(dir.path(), "hello", 50).unwrap();
        let expected = vec![
            ContentHit { path: "README.md".to_string(), line: 2, text: "hello World".to_string() },
            ContentHit { path: "src/deep/notes.txt".to_string(), line: 2, text: "HELLO again".to_string() },
            ContentHit { path: "src/main.rs".to_string(), line: 2, text: "println!(\"hello\");".to_string() },
        ];
        assert_eq!(hits, expected);
    }

    #[test]
    fn search_stops_at_the_limit_and_ignores_an_empty_query_binary_files_and_symlinks() {
        let dir = tree();
        fs::write(dir.path().join("b.bin"), b"hello\0hello").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("o.txt"), "hello").unwrap();
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.search(dir.path(), "hello", 2).unwrap().len(), 2);
        assert_eq!(files.search(dir.path(), "", 10).unwrap(), vec![]);
        let all = files.search(dir.path(), "hello", 50).unwrap();
        assert!(all.iter().all(|hit| hit.path != "b.bin" && !hit.path.starts_with("linkdir")), "{all:?}");
    }

    #[test]
    fn a_very_long_line_is_shortened_in_a_hit() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("long.txt"), format!("needle {}\n", "x".repeat(5000))).unwrap();
        let hits = LocalFiles::new().search(dir.path(), "needle", 5).unwrap();
        assert_eq!(hits[0].text.chars().count(), 200);
    }
}
