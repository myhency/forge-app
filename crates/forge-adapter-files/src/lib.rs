use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, SearchOptions, Sheet, SheetBook, SheetCell, WriteError, READ_LIMIT};
use calamine::{Data, Reader, SheetType, Xlsx};
use chrono::NaiveTime;
use regex::{Regex, RegexBuilder};

mod docx;
mod office_zip;

const SKIPPED: [&str; 2] = [".git", "node_modules"];
// A NUL byte in this many first bytes makes a file binary.
const BINARY_SNIFF: usize = 8000;
// A search looks at no more files than this, so a huge tree cannot keep it busy for long.
const SEARCH_FILE_LIMIT: usize = 20_000;
// A search reads no file larger than this, so the larger read limit does not slow it.
const SEARCH_FILE_BYTES: u64 = 1024 * 1024;
const HIT_TEXT_CHARS: usize = 200;
// A long line keeps this much text before the match, so the match shows in a narrow list.
const HIT_LEAD_CHARS: usize = 24;
const IMAGE_TYPES: [(&str, &str); 7] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("ico", "image/x-icon"),
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
    let root = dunce::canonicalize(root).map_err(failed)?;
    let relative = Path::new(relative);
    if relative.is_absolute() || relative.components().any(|part| matches!(part, Component::ParentDir)) {
        return Err(FilesError::Outside);
    }
    let resolved = dunce::canonicalize(root.join(relative)).map_err(failed)?;
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

fn has_extension(path: &Path, wanted: &str) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case(wanted))
}

const SHEET_MAX_ROWS: usize = 500;
const SHEET_MAX_COLUMNS: usize = 50;

// A number rounded to 15 significant digits, as Excel shows it, so `0.1 + 0.2` reads `0.3`.
fn number_text(number: f64) -> String {
    format!("{number:.14e}").parse::<f64>().unwrap_or(number).to_string()
}

// A cell as the sheet shows it: whole numbers without `.0`, dates as `YYYY-MM-DD`.
fn cell_text(data: &Data) -> String {
    match data {
        Data::Empty => String::new(),
        Data::String(text) => text.clone(),
        Data::Int(number) => number.to_string(),
        Data::Float(number) if number.fract() == 0.0 && number.abs() < 1e15 => format!("{}", *number as i64),
        Data::Float(number) => number_text(*number),
        Data::Bool(true) => "TRUE".to_string(),
        Data::Bool(false) => "FALSE".to_string(),
        Data::DateTime(date) if date.is_duration() => number_text(date.as_f64()),
        Data::DateTime(date) => match date.as_datetime() {
            Some(moment) if moment.time() == NaiveTime::MIN => moment.format("%Y-%m-%d").to_string(),
            Some(moment) => moment.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => number_text(date.as_f64()),
        },
        Data::DateTimeIso(text) | Data::DurationIso(text) => text.clone(),
        Data::Error(error) => error.to_string(),
    }
}

// A sheet stops being read after this many cells, so a sheet of millions of cells (or of one
// cell written over and over) cannot keep the read busy for long.
const SHEET_VISIT_LIMIT: usize = 2_000_000;

// Cells are streamed and placed by their sheet position, so a sheet whose data starts at C3 keeps
// its blank rows and columns; the caps count from A1. Nothing is sized by the sheet's declared
// extent, so one stray cell at XFD1048576 costs nothing; it only marks the sheet cut. Cells are
// kept by position (a later one at the same place wins), so at most 500 x 50 are held.
fn read_sheet(book: &mut Xlsx<Cursor<Vec<u8>>>, name: String, visit_limit: usize) -> Result<Sheet, FilesError> {
    let mut reader = book.worksheet_cells_reader(&name).map_err(failed)?;
    let mut cells: BTreeMap<(usize, usize), SheetCell> = BTreeMap::new();
    let mut truncated = false;
    let mut visited = 0;
    while let Some(cell) = reader.next_cell_with_formula().map_err(failed)? {
        if visited == visit_limit {
            truncated = true;
            break;
        }
        visited += 1;
        let text = cell_text(&Data::from(cell.value));
        let formula = cell.formula.filter(|formula| !formula.is_empty()).map(|formula| format!("={formula}"));
        if text.is_empty() && formula.is_none() {
            continue;
        }
        let (row, column) = (cell.pos.0 as usize, cell.pos.1 as usize);
        if row >= SHEET_MAX_ROWS || column >= SHEET_MAX_COLUMNS {
            truncated = true;
        } else {
            cells.insert((row, column), SheetCell { text, formula });
        }
    }
    let row_count = cells.keys().map(|(row, _)| row + 1).max().unwrap_or(0);
    let column_count = cells.keys().map(|(_, column)| column + 1).max().unwrap_or(0);
    let mut rows = vec![vec![SheetCell { text: String::new(), formula: None }; column_count]; row_count];
    for ((row, column), cell) in cells {
        rows[row][column] = cell;
    }
    Ok(Sheet { name, rows, truncated })
}

// The shortest shared string a writer makes (`<si><t/></si>`) is longer than this many bytes.
const SHARED_STRING_MIN_BYTES: u64 = 7;

// A whole number written as digits (an optional `+` and leading zeros allowed), saturating, so
// it reads every form calamine reads.
fn declared_count(value: &[u8]) -> Option<u64> {
    let digits = value.trim_ascii();
    let digits = digits.strip_prefix(b"+").unwrap_or(digits);
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    Some(digits.iter().fold(0u64, |total, digit| total.saturating_mul(10).saturating_add(u64::from(digit - b'0'))))
}

// The `uniqueCount` on the shared strings table's `sst` tag, if it has one.
fn unique_count(part: impl std::io::Read) -> Option<u64> {
    let mut xml = quick_xml::Reader::from_reader(std::io::BufReader::new(part));
    let mut buffer = Vec::new();
    loop {
        match xml.read_event_into(&mut buffer).ok()? {
            quick_xml::events::Event::Start(tag) | quick_xml::events::Event::Empty(tag) if tag.local_name().as_ref() == "sst" => {
                let attribute = tag.attributes().flatten().find(|attribute| attribute.key.as_ref() == "uniqueCount")?;
                return declared_count(attribute.value.as_bytes());
            }
            quick_xml::events::Event::Eof => return None,
            _ => buffer.clear(),
        }
    }
}

// Refuses a workbook calamine would choke on: a part that unpacks to more than `part_limit`
// (the size the zip declares, and for the shared strings the size it really unpacks to), a zip
// of more parts than any real workbook, or a
// shared strings table that claims more strings than its size can hold, since calamine reserves
// room for the claimed count up front and a huge one aborts the process. A file that is not a zip
// is left to calamine, which says what it is (a password-protected workbook, say).
fn check_xlsx(bytes: &[u8], part_limit: u64) -> Result<(), FilesError> {
    let Ok(mut archive) = zip::ZipArchive::new(Cursor::new(bytes)) else { return Ok(()) };
    let too_big = || FilesError::Failed("This workbook unpacks to too much data to show.".to_string());
    if office_zip::declares_too_much(&mut archive, part_limit) {
        return Err(too_big());
    }
    for index in 0..archive.len() {
        let Ok(part) = archive.by_index_raw(index) else { continue };
        if !part.name().is_ok_and(|name| name.to_ascii_lowercase().ends_with("sharedstrings.xml")) {
            continue;
        }
        drop(part);
        let Ok(part) = archive.by_index(index) else { continue };
        let unpacked = office_zip::copy_bounded(part, part_limit, &mut std::io::sink()).unwrap_or(0);
        if unpacked > part_limit {
            return Err(too_big());
        }
        let Ok(part) = archive.by_index(index) else { continue };
        if unique_count(part).is_some_and(|claimed| claimed > unpacked / SHARED_STRING_MIN_BYTES) {
            return Err(FilesError::Failed("This workbook's list of strings is damaged.".to_string()));
        }
    }
    Ok(())
}

fn read_xlsx(bytes: Vec<u8>) -> Result<FileContent, FilesError> {
    check_xlsx(&bytes, office_zip::PART_BYTES)?;
    let mut book: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).map_err(failed)?;
    let mut sheets = Vec::new();
    // Only worksheets hold cells; a chart sheet (or a dialog or macro sheet) is left out so the
    // rest of the workbook still shows.
    let names: Vec<String> = book.sheets_metadata().iter().filter(|sheet| sheet.typ == SheetType::WorkSheet).map(|sheet| sheet.name.clone()).collect();
    for name in names {
        sheets.push(read_sheet(&mut book, name, SHEET_VISIT_LIMIT)?);
    }
    Ok(FileContent::Sheet(SheetBook { sheets }))
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
        if has_extension(&file, "pdf") {
            return Ok(FileContent::Pdf { bytes });
        }
        if has_extension(&file, "xlsx") {
            return read_xlsx(bytes);
        }
        if has_extension(&file, "docx") {
            return docx::read_docx(bytes);
        }
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
            FilesError::Failed(message) | FilesError::InvalidPattern(message) => WriteError::Failed(message),
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

    fn search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError> {
        let root = dunce::canonicalize(root).map_err(failed)?;
        let mut hits = Vec::new();
        if query.is_empty() || limit == 0 {
            return Ok(hits);
        }
        let matcher = Matcher { regex: build_matcher(query, options)?, whole_word: options.whole_word };
        let mut visited = 0;
        search_dir(&root, "", &matcher, limit, &mut visited, &mut hits);
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

fn build_matcher(query: &str, options: SearchOptions) -> Result<Regex, FilesError> {
    let body = if options.regex { query.to_string() } else { regex::escape(query) };
    RegexBuilder::new(&body).case_insensitive(!options.match_case).build().map_err(|error| {
        let text = error.to_string();
        let last = text.lines().last().unwrap_or("").trim().trim_start_matches("error:").trim();
        FilesError::InvalidPattern(if last.is_empty() { "Invalid pattern".to_string() } else { last.to_string() })
    })
}

/// The pattern, and whether a match must stand alone as a word (judged by `first_match`, with the
/// same rule as the find bar in the editor: no letter, digit or `_` right before or after it).
struct Matcher {
    regex: Regex,
    whole_word: bool,
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

// The first non-empty match in the line (byte range). An empty match, or one that is not a whole
// word when that is asked, is skipped and the search goes on one char later.
fn first_match(line: &str, matcher: &Matcher) -> Option<(usize, usize)> {
    let mut at = 0;
    while at <= line.len() {
        let found = matcher.regex.find_at(line, at)?;
        let next = |from: usize| from + line[from..].chars().next().map_or(1, char::len_utf8);
        if found.is_empty() {
            at = next(found.end());
            continue;
        }
        if matcher.whole_word {
            let before = line[..found.start()].chars().next_back().is_some_and(is_word_char);
            let after = line[found.end()..].chars().next().is_some_and(is_word_char);
            if before || after {
                at = next(found.start());
                continue;
            }
        }
        return Some((found.start(), found.end()));
    }
    None
}

// The first match in a line as a window of the trimmed line of at most HIT_TEXT_CHARS chars with `…`
// at a cut end, and the match's char offsets inside that window. The pattern sees the line as
// written (so `^` and `$` mean the line's start and end); only the text shown is trimmed.
fn hit_of(line: &str, matcher: &Matcher) -> Option<(String, usize, usize)> {
    let (from_byte, to_byte) = first_match(line, matcher)?;
    let lead = line.len() - line.trim_start().len();
    let text = line.trim();
    let clip = |byte: usize| byte.saturating_sub(lead).min(text.len());
    let (from_byte, to_byte) = (clip(from_byte), clip(to_byte));
    let start = text[..from_byte].chars().count();
    let end = start + text[from_byte..to_byte].chars().count();
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= HIT_TEXT_CHARS {
        return Some((text.to_string(), start, end));
    }
    let room = HIT_TEXT_CHARS - 2;
    let from = start.saturating_sub(HIT_LEAD_CHARS).min(chars.len() - room);
    let to = from + room;
    let lead_mark = usize::from(from > 0);
    let mut shown = String::new();
    if from > 0 {
        shown.push('…');
    }
    shown.extend(&chars[from..to]);
    if to < chars.len() {
        shown.push('…');
    }
    Some((shown, start - from + lead_mark, end.min(to) - from + lead_mark))
}

// Depth first in name order; symlinks are not followed, so a link cannot lead out of the root.
fn search_dir(dir: &Path, prefix: &str, matcher: &Matcher, limit: usize, visited: &mut usize, hits: &mut Vec<ContentHit>) {
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
            search_dir(&entry.path(), &path, matcher, limit, visited, hits);
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
            if let Some((text, start, end)) = hit_of(line, matcher) {
                hits.push(ContentHit { path: path.clone(), line: index + 1, text, start, end });
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
    #[cfg(unix)]
    use std::os::unix::fs::symlink;

    use forge_ports::{ContentHit, DirEntry, FileContent, FileVersion, FilesError, FilesPort, SearchOptions, WriteError, READ_LIMIT};
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
        let files = LocalFiles::new();
        assert_eq!(
            files.read(dir.path(), "a.PNG").unwrap(),
            FileContent::Image { bytes: vec![0x89, b'P', b'N', b'G', 0], mime: "image/png".to_string() }
        );
    }

    #[test]
    fn an_svg_is_read_as_text_so_its_source_can_be_shown() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("v.svg"), "<svg/>").unwrap();
        let FileContent::Text { text, lossy, .. } = LocalFiles::new().read(dir.path(), "v.svg").unwrap() else { panic!("not text") };
        assert_eq!(text, "<svg/>");
        assert!(!lossy);
    }

    #[test]
    fn a_pdf_is_read_as_bytes_by_its_extension_in_any_case() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.PDF"), b"%PDF-1.4\0\x01").unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "a.PDF").unwrap(), FileContent::Pdf { bytes: b"%PDF-1.4\0\x01".to_vec() });
    }

    #[test]
    fn a_pdf_over_the_limit_is_too_large() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("big.pdf"), vec![0u8; READ_LIMIT + 1]).unwrap();
        assert_eq!(LocalFiles::new().read(dir.path(), "big.pdf").unwrap(), FileContent::TooLarge);
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

    #[cfg(unix)]
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

    #[cfg(unix)]
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

    #[cfg(unix)]
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

        let hits = LocalFiles::new().search(dir.path(), "needle", SearchOptions::default(), 10).unwrap();

        assert_eq!(hits.iter().map(|hit| hit.path.as_str()).collect::<Vec<_>>(), vec!["small.txt"]);
    }

    #[cfg(unix)]
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
        let hits = LocalFiles::new().search(dir.path(), "hello", SearchOptions::default(), 50).unwrap();
        let expected = vec![
            ContentHit { path: "README.md".to_string(), line: 2, text: "hello World".to_string(), start: 0, end: 5 },
            ContentHit { path: "src/deep/notes.txt".to_string(), line: 2, text: "HELLO again".to_string(), start: 0, end: 5 },
            ContentHit { path: "src/main.rs".to_string(), line: 2, text: "println!(\"hello\");".to_string(), start: 10, end: 15 },
        ];
        assert_eq!(hits, expected);
    }

    #[cfg(unix)]
    #[test]
    fn search_stops_at_the_limit_and_ignores_an_empty_query_binary_files_and_symlinks() {
        let dir = tree();
        fs::write(dir.path().join("b.bin"), b"hello\0hello").unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("o.txt"), "hello").unwrap();
        symlink(outside.path(), dir.path().join("linkdir")).unwrap();
        let files = LocalFiles::new();
        assert_eq!(files.search(dir.path(), "hello", SearchOptions::default(), 2).unwrap().len(), 2);
        assert_eq!(files.search(dir.path(), "", SearchOptions::default(), 10).unwrap(), vec![]);
        let all = files.search(dir.path(), "hello", SearchOptions::default(), 50).unwrap();
        assert!(all.iter().all(|hit| hit.path != "b.bin" && !hit.path.starts_with("linkdir")), "{all:?}");
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

    fn one_file(text: &str) -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("f.txt"), text).unwrap();
        dir
    }

    fn hits(dir: &TempDir, query: &str, options: SearchOptions) -> Vec<ContentHit> {
        LocalFiles::new().search(dir.path(), query, options, 50).unwrap()
    }

    #[test]
    fn a_long_line_is_a_window_around_the_match_with_offsets_into_the_window() {
        let dir = one_file(&format!("{} needle {}\n", "x".repeat(5000), "y".repeat(5000)));
        let found = hits(&dir, "needle", SearchOptions::default());
        let hit = &found[0];
        assert!(hit.text.chars().count() <= 200, "{}", hit.text.chars().count());
        assert!(hit.text.starts_with('…') && hit.text.ends_with('…'));
        let matched: String = hit.text.chars().skip(hit.start).take(hit.end - hit.start).collect();
        assert_eq!(matched, "needle");
    }

    #[test]
    fn a_hit_keeps_only_a_little_text_before_the_match_so_it_shows_in_a_narrow_list() {
        let dir = one_file(&format!("{} needle {}\n", "x".repeat(5000), "y".repeat(5000)));
        let hit = &hits(&dir, "needle", SearchOptions::default())[0];
        assert!(hit.start <= 26, "{}", hit.start);
    }

    #[test]
    fn a_match_at_the_start_of_a_long_line_cuts_only_the_end() {
        let dir = one_file(&format!("needle {}\n", "x".repeat(5000)));
        let hit = &hits(&dir, "needle", SearchOptions::default())[0];
        assert!(hit.text.starts_with("needle") && hit.text.ends_with('…'));
        assert_eq!((hit.start, hit.end), (0, 6));
    }

    #[test]
    fn offsets_count_chars_not_bytes() {
        let dir = one_file("한글 한글 hello\n");
        let hit = &hits(&dir, "hello", SearchOptions::default())[0];
        let matched: String = hit.text.chars().skip(hit.start).take(hit.end - hit.start).collect();
        assert_eq!(matched, "hello");
    }

    #[test]
    fn line_anchors_see_the_line_as_written_not_trimmed() {
        let dir = one_file("fn a;\n    fn b;\n  x\n");
        let regex = SearchOptions { regex: true, ..SearchOptions::default() };
        let found = hits(&dir, "^fn", regex);
        assert_eq!(found.iter().map(|h| h.line).collect::<Vec<_>>(), vec![1]);
        let found = hits(&dir, "^\\s+fn", regex);
        assert_eq!(found.iter().map(|h| h.line).collect::<Vec<_>>(), vec![2]);
        // The text shown is trimmed and the offsets follow it.
        assert_eq!(found[0].text, "fn b;");
        assert_eq!((found[0].start, found[0].end), (0, 2));
        let found = hits(&dir, ";$", regex);
        assert_eq!(found.iter().map(|h| h.line).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn whole_word_works_when_the_query_starts_or_ends_with_a_symbol() {
        let whole = SearchOptions { whole_word: true, ..SearchOptions::default() };
        let dir = one_file("x foo() y\n");
        assert_eq!(hits(&dir, "foo()", whole).len(), 1);
        let dir = one_file(" -x / a-x\n");
        let found = hits(&dir, "-x", whole);
        assert_eq!(found.len(), 1);
        // The first one (after a space) is the match, not the one after the letter `a`.
        assert_eq!((found[0].start, found[0].end), (0, 2));
        let dir = one_file("foobar foo\n");
        let found = hits(&dir, "foo", whole);
        assert_eq!((found[0].start, found[0].end), (7, 10));
    }

    #[test]
    fn match_case_and_the_default_differ() {
        let dir = one_file("Buzz\nbuzz\n");
        assert_eq!(hits(&dir, "buzz", SearchOptions::default()).len(), 2);
        assert_eq!(hits(&dir, "buzz", SearchOptions { match_case: true, ..SearchOptions::default() }).len(), 1);
    }

    #[test]
    fn whole_word_uses_unicode_word_boundaries() {
        let dir = one_file("가나 다\n가나다\nfoo foobar\n");
        let whole = SearchOptions { whole_word: true, ..SearchOptions::default() };
        let lines: Vec<usize> = hits(&dir, "가나", whole).iter().map(|h| h.line).collect();
        assert_eq!(lines, vec![1]);
        let lines: Vec<usize> = hits(&dir, "foo", whole).iter().map(|h| h.line).collect();
        assert_eq!(lines, vec![3]);
    }

    #[test]
    fn a_literal_query_is_not_a_pattern_unless_regex_is_on() {
        let dir = one_file("a.b\naxb\n");
        assert_eq!(hits(&dir, "a.b", SearchOptions::default()).len(), 1);
        let regex = SearchOptions { regex: true, ..SearchOptions::default() };
        assert_eq!(hits(&dir, "a.b", regex).len(), 2);
    }

    #[test]
    fn an_invalid_regex_is_an_error_with_a_short_message() {
        let dir = one_file("x\n");
        let regex = SearchOptions { regex: true, ..SearchOptions::default() };
        let error = LocalFiles::new().search(dir.path(), "(", regex, 10).unwrap_err();
        match error {
            FilesError::InvalidPattern(message) => assert!(!message.contains('\n') && !message.is_empty(), "{message}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_pattern_that_matches_nothing_in_a_line_gives_no_hit_and_an_empty_match_is_skipped() {
        let dir = one_file("abc\n\n");
        let regex = SearchOptions { regex: true, ..SearchOptions::default() };
        // `x*` matches the empty string everywhere: no line is a hit.
        assert_eq!(hits(&dir, "x*", regex), vec![]);
        assert_eq!(hits(&dir, "b", regex).len(), 1);
    }

    fn read_sheets(dir: &TempDir, name: &str) -> Vec<forge_ports::Sheet> {
        let FileContent::Sheet(book) = LocalFiles::new().read(dir.path(), name).unwrap() else { panic!("not a sheet") };
        book.sheets
    }

    fn cell_texts(sheet: &forge_ports::Sheet) -> Vec<Vec<&str>> {
        sheet.rows.iter().map(|row| row.iter().map(|cell| cell.text.as_str()).collect()).collect()
    }

    #[test]
    fn an_xlsx_is_read_as_sheets_with_names_and_cell_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let first = book.add_worksheet();
        first.set_name("Sales").unwrap();
        first.write_string(0, 0, "Name").unwrap();
        first.write_string(0, 1, "Total").unwrap();
        first.write_string(1, 0, "Ann").unwrap();
        first.write_number(1, 1, 2.5).unwrap();
        first.write_boolean(2, 0, true).unwrap();
        first.write_boolean(2, 1, false).unwrap();
        book.add_worksheet().set_name("Empty").unwrap();
        book.save(dir.path().join("a.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "a.xlsx");
        assert_eq!(sheets.iter().map(|sheet| sheet.name.as_str()).collect::<Vec<_>>(), vec!["Sales", "Empty"]);
        assert_eq!(cell_texts(&sheets[0]), vec![vec!["Name", "Total"], vec!["Ann", "2.5"], vec!["TRUE", "FALSE"]]);
        assert!(!sheets[0].truncated);
        assert_eq!(sheets[0].rows[0][0].formula, None);
    }

    #[test]
    fn a_whole_number_has_no_decimal_point() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_number(0, 0, 42.0).unwrap();
        sheet.write_number(0, 1, -7.0).unwrap();
        book.save(dir.path().join("n.xlsx")).unwrap();
        assert_eq!(cell_texts(&read_sheets(&dir, "n.xlsx")[0]), vec![vec!["42", "-7"]]);
    }

    #[test]
    fn a_fraction_shows_at_most_15_significant_digits_as_excel_does() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_number(0, 0, 0.1 + 0.2).unwrap();
        sheet.write_number(0, 1, 1.0 / 3.0).unwrap();
        sheet.write_number(0, 2, 2.5).unwrap();
        sheet.write_number(0, 3, -0.25).unwrap();
        book.save(dir.path().join("p.xlsx")).unwrap();
        assert_eq!(cell_texts(&read_sheets(&dir, "p.xlsx")[0]), vec![vec!["0.3", "0.333333333333333", "2.5", "-0.25"]]);
    }

    #[test]
    fn a_date_cell_shows_as_iso() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        let format = rust_xlsxwriter::Format::new().set_num_format("yyyy-mm-dd");
        let date = rust_xlsxwriter::ExcelDateTime::from_ymd(2026, 10, 25).unwrap();
        sheet.write_datetime_with_format(0, 0, &date, &format).unwrap();
        book.save(dir.path().join("d.xlsx")).unwrap();
        assert_eq!(cell_texts(&read_sheets(&dir, "d.xlsx")[0]), vec![vec!["2026-10-25"]]);
    }

    #[test]
    fn a_formula_cell_keeps_its_formula_and_stored_value() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_number(0, 0, 1.0).unwrap();
        sheet.write_number(0, 1, 2.0).unwrap();
        sheet.write_formula(0, 2, rust_xlsxwriter::Formula::new("=A1+B1").set_result("3")).unwrap();
        book.save(dir.path().join("f.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "f.xlsx");
        let cell = &sheets[0].rows[0][2];
        assert_eq!(cell.text, "3");
        assert_eq!(cell.formula.as_deref(), Some("=A1+B1"));
    }

    #[test]
    fn a_formula_without_a_stored_value_has_empty_text_and_the_formula() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_number(0, 0, 1.0).unwrap();
        sheet.set_formula_result_default("");
        sheet.write_formula(0, 1, "=A1*2").unwrap();
        book.save(dir.path().join("g.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "g.xlsx");
        let cell = &sheets[0].rows[0][1];
        assert_eq!(cell.text, "");
        assert_eq!(cell.formula.as_deref(), Some("=A1*2"));
    }

    #[test]
    fn a_sheet_over_500_rows_or_50_columns_is_cut_and_marked() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let tall = book.add_worksheet();
        for row in 0..501 {
            tall.write_number(row, 0, f64::from(row)).unwrap();
        }
        let wide = book.add_worksheet();
        for column in 0..51 {
            wide.write_number(0, column, f64::from(column)).unwrap();
        }
        let exact = book.add_worksheet();
        exact.write_number(499, 49, 1.0).unwrap();
        book.save(dir.path().join("big.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "big.xlsx");
        assert_eq!((sheets[0].rows.len(), sheets[0].rows[0].len(), sheets[0].truncated), (500, 1, true));
        assert_eq!((sheets[1].rows.len(), sheets[1].rows[0].len(), sheets[1].truncated), (1, 50, true));
        assert_eq!((sheets[2].rows.len(), sheets[2].rows[0].len(), sheets[2].truncated), (500, 50, false));
    }

    #[test]
    fn an_empty_sheet_has_no_rows() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet();
        book.save(dir.path().join("e.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "e.xlsx");
        assert_eq!(sheets.len(), 1);
        assert!(sheets[0].rows.is_empty());
        assert!(!sheets[0].truncated);
    }

    #[test]
    fn a_file_named_xlsx_that_is_not_a_zip_fails_with_text() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("bad.xlsx"), "just words").unwrap();
        let Err(FilesError::Failed(message)) = LocalFiles::new().read(dir.path(), "bad.xlsx") else { panic!("did not fail") };
        assert!(!message.is_empty());
    }

    #[test]
    fn a_far_corner_cell_does_not_size_the_sheet() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_string(0, 0, "top").unwrap();
        sheet.write_string(1_048_575, 16_383, "far").unwrap();
        let below = book.add_worksheet();
        below.write_string(1_000_000, 0, "deep").unwrap();
        book.save(dir.path().join("far.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "far.xlsx");
        assert_eq!(cell_texts(&sheets[0]), vec![vec!["top"]]);
        assert!(sheets[0].truncated);
        assert!(sheets[1].rows.is_empty());
        assert!(sheets[1].truncated);
    }

    #[test]
    fn a_sheet_of_only_formulas_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let sheet = book.add_worksheet();
        sheet.write_formula(1, 1, rust_xlsxwriter::Formula::new("=1+1").set_result("2")).unwrap();
        book.save(dir.path().join("o.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "o.xlsx");
        assert_eq!(sheets[0].rows.len(), 2);
        assert_eq!(sheets[0].rows[1][1].formula.as_deref(), Some("=1+1"));
        assert_eq!(sheets[0].rows[1][1].text, "2");
        assert_eq!(sheets[0].rows[0][0].text, "");
    }

    #[test]
    fn a_truncated_xlsx_fails_with_text() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().write_string(0, 0, "x").unwrap();
        let bytes = book.save_to_buffer().unwrap();
        fs::write(dir.path().join("half.xlsx"), &bytes[..bytes.len() / 2]).unwrap();
        let Err(FilesError::Failed(message)) = LocalFiles::new().read(dir.path(), "half.xlsx") else { panic!("did not fail") };
        assert!(!message.is_empty());
    }

    #[test]
    fn a_password_protected_xlsx_fails_with_text() {
        // An encrypted workbook is a compound file that holds an `EncryptedPackage` stream.
        let dir = tempfile::tempdir().unwrap();
        let mut compound = cfb::CompoundFile::create_with_version(cfb::Version::V3, fs::File::create(dir.path().join("locked.xlsx")).unwrap()).unwrap();
        std::io::Write::write_all(&mut compound.create_stream("/EncryptedPackage").unwrap(), &[0u8; 5000]).unwrap();
        compound.flush().unwrap();
        drop(compound);
        let Err(FilesError::Failed(message)) = LocalFiles::new().read(dir.path(), "locked.xlsx") else { panic!("did not fail") };
        assert!(message.to_lowercase().contains("password"), "{message}");
    }

    #[test]
    fn an_uppercase_xlsx_extension_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        book.add_worksheet().write_string(0, 0, "x").unwrap();
        book.save(dir.path().join("UP.XLSX")).unwrap();
        assert_eq!(cell_texts(&read_sheets(&dir, "UP.XLSX")[0]), vec![vec!["x"]]);
    }

    // A hand-made workbook of one sheet `S` with this sheet XML body and shared strings table.
    fn hand_made_xlsx(sheet_data: &str, shared_strings: &str) -> Vec<u8> {
        use std::io::Write;
        let parts = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/><Override PartName="/xl/sharedStrings.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sharedStrings+xml"/></Types>"#.to_string(),
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/></Relationships>"#.to_string(),
            ),
            (
                "xl/workbook.xml",
                r#"<?xml version="1.0"?><workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><sheets><sheet name="S" sheetId="1" r:id="rId1"/></sheets></workbook>"#.to_string(),
            ),
            (
                "xl/_rels/workbook.xml.rels",
                r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/sharedStrings" Target="sharedStrings.xml"/></Relationships>"#.to_string(),
            ),
            (
                "xl/worksheets/sheet1.xml",
                format!(r#"<?xml version="1.0"?><worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>{sheet_data}</sheetData></worksheet>"#),
            ),
            ("xl/sharedStrings.xml", shared_strings.to_string()),
        ];
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, text) in parts {
            zip.start_file(name, options).unwrap();
            zip.write_all(text.as_bytes()).unwrap();
        }
        zip.finish().unwrap().into_inner()
    }

    fn shared_strings(attributes: &str) -> String {
        format!(r#"<?xml version="1.0"?><sst xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" {attributes}><si><t>x</t></si></sst>"#)
    }

    const ONE_STRING_CELL: &str = r#"<row r="1"><c r="A1" t="s"><v>0</v></c></row>"#;

    #[test]
    fn a_shared_strings_table_that_claims_far_more_strings_than_it_holds_is_refused() {
        // calamine reserves room for the claimed count up front: 2^50 aborts the process and
        // u64::MAX panics, so the file must be refused before calamine sees it.
        for claimed in ["1125899906842624", "18446744073709551615", "+99999999999", "000000000000000000000000099999999999"] {
            let dir = tempfile::tempdir().unwrap();
            fs::write(dir.path().join("s.xlsx"), hand_made_xlsx(ONE_STRING_CELL, &shared_strings(&format!(r#"count="1" uniqueCount="{claimed}""#)))).unwrap();
            let Err(FilesError::Failed(message)) = LocalFiles::new().read(dir.path(), "s.xlsx") else { panic!("{claimed} was not refused") };
            assert!(!message.is_empty());
        }
    }

    #[test]
    fn an_honest_shared_strings_table_is_read_even_when_its_strings_are_used_many_times() {
        // `count` counts uses, not strings, so a big `count` is not a reason to refuse.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("s.xlsx"), hand_made_xlsx(ONE_STRING_CELL, &shared_strings(r#"count="99999999999" uniqueCount="1""#))).unwrap();
        assert_eq!(cell_texts(&read_sheets(&dir, "s.xlsx")[0]), vec![vec!["x"]]);
    }

    #[test]
    fn a_chart_sheet_is_skipped_and_the_other_sheets_show() {
        let dir = tempfile::tempdir().unwrap();
        let mut book = rust_xlsxwriter::Workbook::new();
        let data = book.add_worksheet();
        data.set_name("Data").unwrap();
        data.write_number(0, 0, 1.0).unwrap();
        data.write_number(1, 0, 2.0).unwrap();
        let mut chart = rust_xlsxwriter::Chart::new(rust_xlsxwriter::ChartType::Column);
        chart.add_series().set_values("Data!$A$1:$A$2");
        book.add_chartsheet().insert_chart(0, 0, &chart).unwrap();
        book.add_worksheet().set_name("After").unwrap();
        book.save(dir.path().join("c.xlsx")).unwrap();
        let sheets = read_sheets(&dir, "c.xlsx");
        assert_eq!(sheets.iter().map(|sheet| sheet.name.as_str()).collect::<Vec<_>>(), vec!["Data", "After"]);
        assert_eq!(cell_texts(&sheets[0]), vec![vec!["1"], vec!["2"]]);
    }

    #[test]
    fn a_cell_written_many_times_at_one_place_is_kept_once_with_its_last_value() {
        let cells: String = (0..30_000).map(|value| format!(r#"<c r="A1"><v>{value}</v></c>"#)).collect();
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("r.xlsx"), hand_made_xlsx(&format!(r#"<row r="1">{cells}</row>"#), &shared_strings(r#"uniqueCount="1""#))).unwrap();
        let sheets = read_sheets(&dir, "r.xlsx");
        assert_eq!(cell_texts(&sheets[0]), vec![vec!["29999"]]);
        assert!(sheets[0].rows.iter().map(Vec::len).sum::<usize>() <= SHEET_MAX_ROWS * SHEET_MAX_COLUMNS);
        assert!(!sheets[0].truncated);
    }

    #[test]
    fn a_sheet_stops_being_read_after_the_cell_limit_and_is_marked_cut() {
        let cells: String = (0..20).map(|column| format!(r#"<c r="{}1"><v>{column}</v></c>"#, char::from(b'A' + column))).collect();
        let bytes = hand_made_xlsx(&format!(r#"<row r="1">{cells}</row>"#), &shared_strings(r#"uniqueCount="1""#));
        let mut book: Xlsx<_> = Xlsx::new(Cursor::new(bytes)).unwrap();
        let sheet = read_sheet(&mut book, "S".to_string(), 10).unwrap();
        assert_eq!(sheet.rows[0].len(), 10);
        assert_eq!(sheet.rows[0][9].text, "9");
        assert!(sheet.truncated);
        let sheet = read_sheet(&mut book, "S".to_string(), 20).unwrap();
        assert_eq!((sheet.rows[0].len(), sheet.truncated), (20, false));
    }

    #[test]
    fn a_part_that_unpacks_past_the_limit_is_refused() {
        let bytes = hand_made_xlsx(ONE_STRING_CELL, &shared_strings(r#"uniqueCount="1""#));
        assert!(check_xlsx(&bytes, 100_000).is_ok());
        // The sheet part is the biggest one, at a few hundred bytes.
        let Err(FilesError::Failed(message)) = check_xlsx(&bytes, 150) else { panic!("not refused") };
        assert!(!message.is_empty());
    }
}

/// Plain folder work for scratch folders, with `std::fs`.
#[derive(Default)]
pub struct ScratchFs;

#[async_trait::async_trait]
impl forge_ports::ScratchPort for ScratchFs {
    async fn create_new_dir(&self, path: &Path) -> Result<bool, forge_ports::ScratchError> {
        let failed = |e: std::io::Error| forge_ports::ScratchError(e.to_string());
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(failed)?;
        }
        // `create_dir` is atomic: it fails when the folder exists, so two callers never share one.
        match fs::create_dir(path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(e) => Err(failed(e)),
        }
    }
    async fn is_empty_dir(&self, path: &Path) -> Result<bool, forge_ports::ScratchError> {
        match fs::read_dir(path) {
            Ok(mut entries) => Ok(entries.next().is_none()),
            Err(_) => Ok(false),
        }
    }
    // `remove_dir` (never `remove_dir_all`): it fails on a folder that has entries.
    async fn remove_empty_dir(&self, path: &Path) -> Result<(), forge_ports::ScratchError> {
        fs::remove_dir(path).map_err(|e| forge_ports::ScratchError(e.to_string()))
    }
}

#[cfg(test)]
mod scratch_fs_tests {
    use super::*;
    use forge_ports::ScratchPort;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("forge-scratchfs-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[tokio::test]
    async fn create_new_dir_makes_the_folder_once() {
        let base = temp("create");
        let target = base.join("a").join("b");
        assert!(ScratchFs.create_new_dir(&target).await.unwrap());
        assert!(target.is_dir());
        fs::write(target.join("f"), "x").unwrap();
        assert!(!ScratchFs.create_new_dir(&target).await.unwrap());
        assert!(target.join("f").is_file());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn is_empty_dir_cases() {
        let base = temp("empty");
        let full = base.join("full");
        fs::create_dir_all(&full).unwrap();
        fs::write(full.join("f"), "x").unwrap();
        let empty = base.join("empty");
        fs::create_dir_all(&empty).unwrap();
        fs::write(base.join("file"), "x").unwrap();
        assert!(ScratchFs.is_empty_dir(&empty).await.unwrap());
        assert!(!ScratchFs.is_empty_dir(&full).await.unwrap());
        assert!(!ScratchFs.is_empty_dir(&base.join("file")).await.unwrap());
        assert!(!ScratchFs.is_empty_dir(&base.join("missing")).await.unwrap());
        fs::remove_dir_all(&base).unwrap();
    }

    #[tokio::test]
    async fn remove_empty_dir_never_removes_contents() {
        let base = temp("remove");
        let empty = base.join("empty");
        fs::create_dir_all(&empty).unwrap();
        ScratchFs.remove_empty_dir(&empty).await.unwrap();
        assert!(!empty.exists());
        let full = base.join("full");
        fs::create_dir_all(&full).unwrap();
        fs::write(full.join("f"), "x").unwrap();
        assert!(ScratchFs.remove_empty_dir(&full).await.is_err());
        assert!(full.join("f").is_file());
        fs::remove_dir_all(&base).unwrap();
    }
}
