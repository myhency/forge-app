use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};

const SKIPPED: [&str; 2] = [".git", "node_modules"];
// A NUL byte in this many first bytes makes a file binary.
const BINARY_SNIFF: usize = 8000;
// A search looks at no more files than this, so a huge tree cannot keep it busy for long.
const SEARCH_FILE_LIMIT: usize = 20_000;
// A search reads no file larger than this, so the larger read limit does not slow it.
const SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const HIT_TEXT_CHARS: usize = 200;
const IMAGE_TYPES: [(&str, &str); 8] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("ico", "image/x-icon"),
    ("svg", "image/svg+xml"),
];

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

fn image_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    IMAGE_TYPES.iter().find(|(known, _)| *known == extension).map(|(_, mime)| *mime)
}

fn version_of(meta: &fs::Metadata) -> FileVersion {
    let modified_ns = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map_or(0, |since| since.as_nanos());
    FileVersion { modified_ns, len: meta.len() }
}

fn text_content(bytes: Vec<u8>, version: FileVersion) -> FileContent {
    match String::from_utf8(bytes) {
        Ok(text) => FileContent::Text { text, version, lossy: false },
        Err(error) => FileContent::Text { text: String::from_utf8_lossy(error.as_bytes()).into_owned(), version, lossy: true },
    }
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
        let meta = fs::metadata(&file).map_err(failed)?;
        if meta.len() > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        if let Some(mime) = image_mime(&file) {
            return Ok(FileContent::Image { bytes, mime: mime.to_string() });
        }
        if looks_binary(&bytes) {
            return Ok(FileContent::Binary);
        }
        Ok(text_content(bytes, version_of(&meta)))
    }

    fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError> {
        let file = resolve(root, path)?;
        let meta = fs::metadata(&file).map_err(failed)?;
        if meta.len() > READ_LIMIT as u64 {
            return Ok(FileContent::TooLarge);
        }
        let bytes = fs::read(&file).map_err(failed)?;
        Ok(FileContent::Text { text: String::from_utf8_lossy(&bytes).into_owned(), version: version_of(&meta), lossy: true })
    }

    fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError> {
        let file = resolve(root, path)?;
        Ok(version_of(&fs::metadata(&file).map_err(failed)?))
    }

    fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError> {
        let file = resolve(root, path).map_err(|error| match error {
            FilesError::Outside => WriteError::Outside,
            FilesError::Failed(message) => WriteError::Failed(message),
        })?;
        let failed_write = |error: std::io::Error| WriteError::Failed(error.to_string());
        let meta = fs::metadata(&file).map_err(failed_write)?;
        if expected.is_some_and(|version| version != version_of(&meta)) {
            return Err(WriteError::Changed);
        }
        let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        let temp = file.with_file_name(format!(".{name}.forge-tmp"));
        let result = fs::write(&temp, text).and_then(|()| fs::set_permissions(&temp, meta.permissions())).and_then(|()| fs::rename(&temp, &file));
        if let Err(error) = result {
            let _ = fs::remove_file(&temp);
            return Err(failed_write(error));
        }
        Ok(version_of(&fs::metadata(&file).map_err(failed_write)?))
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

    fn read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError> {
        use std::io::{Read, Seek, SeekFrom};
        let mut file = fs::File::open(path).map_err(failed)?;
        let length = file.metadata().map_err(failed)?.len();
        file.seek(SeekFrom::Start(length.saturating_sub(max_bytes as u64))).map_err(failed)?;
        let mut bytes = Vec::new();
        file.take(max_bytes as u64).read_to_end(&mut bytes).map_err(failed)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
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
        if entry.metadata().map(|meta| meta.len() > SEARCH_FILE_BYTES).unwrap_or(true) {
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

    use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, WriteError, READ_LIMIT};
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
    fn a_text_file_is_read_with_its_content_and_version() {
        let dir = tree();
        let files = LocalFiles::new();
        let FileContent::Text { text, version, lossy } = files.read(dir.path(), "README.md").unwrap() else { panic!("not text") };
        assert_eq!(text, "# Title\nhello World\n");
        assert!(!lossy);
        assert_eq!(version.len, 20);
        assert_eq!(version, files.stat(dir.path(), "README.md").unwrap());
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
        assert!(matches!(files.read(dir.path(), "exact.txt").unwrap(), FileContent::Text { .. }));
    }

    #[test]
    fn text_that_is_not_utf8_is_read_lossily_and_marked_so() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("l.txt"), [b'a', 0xff, b'b']).unwrap();
        let FileContent::Text { text, lossy, .. } = LocalFiles::new().read(dir.path(), "l.txt").unwrap() else { panic!("not text") };
        assert_eq!(text, "a\u{fffd}b");
        assert!(lossy, "a file that is not valid UTF-8 must be marked so it is never written back");
    }

    #[test]
    fn an_image_is_read_as_bytes_by_its_extension_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.PNG"), [0x89, b'P', b'N', b'G', 0]).unwrap();
        fs::write(dir.path().join("v.svg"), "<svg/>").unwrap();
        let files = LocalFiles::new();
        assert_eq!(
            files.read(dir.path(), "a.PNG").unwrap(),
            FileContent::Image { bytes: vec![0x89, b'P', b'N', b'G', 0], mime: "image/png".to_string() }
        );
        assert_eq!(files.read(dir.path(), "v.svg").unwrap(), FileContent::Image { bytes: b"<svg/>".to_vec(), mime: "image/svg+xml".to_string() });
    }

    #[test]
    fn an_image_over_the_limit_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.png"), vec![0u8; READ_LIMIT + 1]).unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "big.png").unwrap(), FileContent::TooLarge);
    }

    #[test]
    fn text_anyway_reads_a_binary_file_as_text_and_still_refuses_a_big_one() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("b.bin"), [b'a', 0, b'b']).unwrap();
        fs::write(dir.path().join("big.bin"), vec![0u8; READ_LIMIT + 1]).unwrap();
        let files = LocalFiles::new();
        let FileContent::Text { text, .. } = files.read_text_anyway(dir.path(), "b.bin").unwrap() else { panic!("not text") };
        assert_eq!(text, "a\0b");
        assert_eq!(files.read_text_anyway(dir.path(), "big.bin").unwrap(), FileContent::TooLarge);
        assert_eq!(files.read_text_anyway(dir.path(), "../x"), Err(FilesError::Outside));
    }

    fn written(dir: &TempDir, name: &str, text: &str) -> FileVersion {
        fs::write(dir.path().join(name), text).unwrap();
        LocalFiles::new().stat(dir.path(), name).unwrap()
    }

    #[test]
    fn write_replaces_the_file_when_the_version_matches_and_returns_the_new_version() {
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        let files = LocalFiles::new();

        let after = files.write(dir.path(), "a.txt", "two two", Some(before)).unwrap();

        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "two two");
        assert_eq!(after.len, 7);
        assert_eq!(after, files.stat(dir.path(), "a.txt").unwrap());
    }

    #[test]
    fn write_refuses_when_the_file_changed_since_it_was_read() {
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        fs::write(dir.path().join("a.txt"), "changed by someone else").unwrap();

        let result = LocalFiles::new().write(dir.path(), "a.txt", "mine", Some(before));

        assert_eq!(result, Err(WriteError::Changed));
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "changed by someone else", "the other change is untouched");
    }

    #[test]
    fn write_with_no_expected_version_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        written(&dir, "a.txt", "one");
        fs::write(dir.path().join("a.txt"), "changed by someone else").unwrap();

        LocalFiles::new().write(dir.path(), "a.txt", "mine", None).unwrap();

        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "mine");
    }

    #[test]
    fn write_leaves_no_temporary_file_and_keeps_the_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        written(&dir, "run.sh", "echo 1\n");
        fs::set_permissions(dir.path().join("run.sh"), fs::Permissions::from_mode(0o755)).unwrap();
        let before = LocalFiles::new().stat(dir.path(), "run.sh").unwrap();

        LocalFiles::new().write(dir.path(), "run.sh", "echo 2\n", Some(before)).unwrap();

        let mode = fs::metadata(dir.path().join("run.sh")).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);
        let names: Vec<String> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        assert_eq!(names, vec!["run.sh".to_string()], "the temporary file is gone");
    }

    #[test]
    fn write_refuses_paths_that_leave_the_folder_and_says_why_a_missing_file_fails() {
        let dir = tree();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("secret.txt"), "secret").unwrap();
        symlink(outside.path().join("secret.txt"), dir.path().join("link.txt")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.write(dir.path(), "../secret.txt", "x", None), Err(WriteError::Outside));
        assert_eq!(files.write(dir.path(), "link.txt", "x", None), Err(WriteError::Outside));
        assert_eq!(fs::read_to_string(outside.path().join("secret.txt")).unwrap(), "secret");
        assert!(matches!(files.write(dir.path(), "nope.txt", "x", None), Err(WriteError::Failed(_))));
    }

    #[test]
    fn write_into_a_folder_that_cannot_be_written_is_a_message_and_keeps_the_old_text() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let before = written(&dir, "a.txt", "one");
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();

        let result = LocalFiles::new().write(dir.path(), "a.txt", "two", Some(before));

        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(result, Err(WriteError::Failed(_))), "{result:?}");
        assert_eq!(fs::read_to_string(dir.path().join("a.txt")).unwrap(), "one");
    }

    #[test]
    fn a_file_over_a_mebibyte_is_not_searched_even_though_it_can_be_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut text = "needle\n".to_string();
        text.push_str(&"x".repeat(2 * 1024 * 1024));
        fs::write(dir.path().join("big.txt"), text).unwrap();
        fs::write(dir.path().join("small.txt"), "needle\n").unwrap();

        let hits = LocalFiles::new().search(dir.path(), "needle", 10).unwrap();

        assert_eq!(hits.iter().map(|hit| hit.path.as_str()).collect::<Vec<_>>(), vec!["small.txt"]);
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

    #[test]
    fn read_tail_gives_the_end_of_a_long_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("out.txt");
        std::fs::write(&file, "0123456789").unwrap();
        assert_eq!(LocalFiles::new().read_tail(&file, 4).unwrap(), "6789");
        assert_eq!(LocalFiles::new().read_tail(&file, 100).unwrap(), "0123456789");
        assert_eq!(LocalFiles::new().read_tail(&file, 0).unwrap(), "");
    }

    #[test]
    fn read_tail_of_an_empty_or_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let empty = dir.path().join("empty.txt");
        std::fs::write(&empty, "").unwrap();
        assert_eq!(LocalFiles::new().read_tail(&empty, 10).unwrap(), "");
        assert!(matches!(LocalFiles::new().read_tail(&dir.path().join("nope.txt"), 10), Err(FilesError::Failed(_))));
    }

    #[test]
    fn read_tail_replaces_bytes_that_are_not_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("bin.txt");
        std::fs::write(&file, [b'a', 0xff, b'b']).unwrap();
        assert_eq!(LocalFiles::new().read_tail(&file, 10).unwrap(), "a\u{fffd}b");
    }
}
