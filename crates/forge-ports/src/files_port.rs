use std::path::Path;

use thiserror::Error;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// The most bytes of a file that `FilesPort::read` shows: 10 MiB (the real app opened a 10 MiB
/// text file and refused a 15 MiB one).
pub const READ_LIMIT: usize = 10 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// When a file was last changed and how long it is. Two reads that give the same version saw the
/// same file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileVersion {
    pub modified_ns: u128,
    pub len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileContent {
    /// `lossy` is true when the bytes were not valid UTF-8 and some were replaced; such a file must
    /// not be written back.
    Text { text: String, version: FileVersion, lossy: bool },
    /// A picture, by its extension; `mime` is for a `data:` address.
    Image { bytes: Vec<u8>, mime: String },
    /// A PDF, by its extension; shown by the web view's own viewer.
    Pdf { bytes: Vec<u8> },
    /// An xlsx workbook, by its extension, read into text cells.
    Sheet(SheetBook),
    /// A docx document, by its extension, read into blocks of text.
    Document(DocumentView),
    /// A pptx deck, by its extension, read into slides of text.
    Slides(SlideDeck),
    /// The file has a NUL byte near its start.
    Binary,
    /// The file is over `READ_LIMIT`.
    TooLarge,
}

/// The sheets of an xlsx file, each cut to `rows` of at most 500 rows and 50 cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetBook {
    pub sheets: Vec<Sheet>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sheet {
    pub name: String,
    pub rows: Vec<Vec<SheetCell>>,
    /// True when the row or the cell cap cut something off.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetCell {
    /// The value as shown; empty for an empty cell or a formula without a stored value.
    pub text: String,
    /// The formula text with a leading `=`.
    pub formula: Option<String>,
}

/// The body of a docx file as blocks, cut to a block cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentView {
    pub blocks: Vec<DocBlock>,
    /// True when the block cap cut something off.
    pub truncated: bool,
    /// The file itself, for the page view (`None` when it was not kept).
    pub source: Option<std::sync::Arc<[u8]>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocBlock {
    /// `level` is 1 to 6; `color` is the one its paragraph style gives it.
    Heading { level: u8, text: String, color: Option<Rgb> },
    Paragraph { runs: Vec<TextRun> },
    /// `level` is the nesting depth from 0.
    ListItem { level: u8, numbered: bool, runs: Vec<TextRun> },
    Table { rows: Vec<Vec<TableCell>> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextRun {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strike: bool,
    pub color: Option<Rgb>,
    /// A named highlight or the run's shading.
    pub highlight: Option<Rgb>,
}

/// A table cell's text with its formatting (a nested table's text is part of it) and its
/// background colour.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TableCell {
    pub runs: Vec<TextRun>,
    pub fill: Option<Rgb>,
}

impl TableCell {
    /// The cell's text without its formatting.
    pub fn text(&self) -> String {
        self.runs.iter().map(|run| run.text.as_str()).collect()
    }
}

/// A colour read from a file. It is only ever made from six hex digits, so it cannot carry
/// anything else into the page.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Exactly six ASCII hex digits, in any case; anything else (`auto`, a `#`, more or fewer
    /// digits) is `None`.
    pub fn from_hex(hex: &str) -> Option<Rgb> {
        let digits = hex.as_bytes();
        if digits.len() != 6 || !digits.iter().all(u8::is_ascii_hexdigit) {
            return None;
        }
        let byte = |at: usize| u8::from_str_radix(&hex[at..at + 2], 16).ok();
        Some(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }

    /// `#rrggbb` in lower case.
    pub fn css(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// The slides of a pptx file, cut to a slide cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideDeck {
    pub slides: Vec<Slide>,
    /// True when the slide cap cut something off.
    pub truncated: bool,
    /// The file itself, for the slide pictures (`None` when it was not kept).
    pub source: Option<std::sync::Arc<[u8]>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Slide {
    /// Counted from 1.
    pub number: usize,
    pub title: Option<String>,
    pub items: Vec<SlideItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideItem {
    /// The nesting depth from 0.
    pub level: u8,
    pub text: String,
}

/// A line of a file that holds the searched text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentHit {
    pub path: String,
    pub line: usize,
    pub text: String,
    /// Char offsets of the first match inside `text`.
    pub start: usize,
    pub end: usize,
}

/// How a content search reads its query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    /// The query is a regular expression instead of plain text.
    pub regex: bool,
}

/// The files under a folder, for the `@` mention: relative `/`-separated paths, newest first.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileList {
    pub files: Vec<String>,
    /// True when the walk stopped at its cap of visited entries, so some files are missing.
    pub cut: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FilesError {
    #[error("That path is outside the session folder")]
    Outside,
    /// The file or folder is not there (any more).
    #[error("No such file or folder")]
    NotFound,
    /// A folder was asked for where a file was needed (a link to `Todo.app`, which is a folder).
    #[error("That path is a folder")]
    IsDirectory,
    #[error("{0}")]
    Failed(String),
    #[error("{0}")]
    InvalidPattern(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WriteError {
    #[error("The file changed on disk since it was read")]
    Changed,
    #[error("That path is outside the session folder")]
    Outside,
    #[error("{0}")]
    Failed(String),
}

/// Reads the files under a root folder. Every path is relative to the root, `/`-separated, with
/// `""` for the root itself; a path that leaves the root is refused.
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait FilesPort: Send + Sync {
    /// The entries of a folder, folders first, then by name; `.git` and `node_modules` are left out.
    fn list(&self, root: &Path, dir: &str) -> Result<Vec<DirEntry>, FilesError>;
    fn read(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>;
    /// The file's bytes as text even when it looks binary (invalid UTF-8 is replaced). `TooLarge`
    /// over `READ_LIMIT`. For "Show as text anyway"; the result must be shown read only.
    fn read_text_anyway(&self, root: &Path, path: &str) -> Result<FileContent, FilesError>;
    /// The file's version now.
    fn stat(&self, root: &Path, path: &str) -> Result<FileVersion, FilesError>;
    /// Replaces the file's text. With `Some(version)` it first checks the file is still at that
    /// version and gives `WriteError::Changed` when it is not; with `None` it overwrites. The text
    /// goes to a temporary file in the same folder that is renamed over the target, so a crash
    /// leaves the old or the new file whole. Returns the version after the write.
    fn write(&self, root: &Path, path: &str, text: &str, expected: Option<FileVersion>) -> Result<FileVersion, WriteError>;
    /// One hit per line that holds a match of `query`, at most `limit`, in path order.
    fn search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError>;
    /// The files (not folders) under the root, newest modified first, at most `limit`. Hidden
    /// entries and build folders are left out, symlinks are not followed, and the walk is capped in
    /// depth and in entries visited (`FileList::cut` says the cap was hit).
    fn list_files(&self, root: &Path, limit: usize) -> Result<FileList, FilesError>;
    /// The last `max_bytes` of the file at an absolute `path`, as text (bytes that are not UTF-8 are
    /// replaced). Unlike `read` it is not confined to a root: the caller must have checked the path.
    fn read_tail(&self, path: &Path, max_bytes: usize) -> Result<String, FilesError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_read_as_messages() {
        assert_eq!(FilesError::Outside.to_string(), "That path is outside the session folder");
        assert_eq!(FilesError::Failed("denied".to_string()).to_string(), "denied");
    }

    #[test]
    fn rgb_from_hex_takes_exactly_six_hex_digits_in_any_case() {
        assert_eq!(Rgb::from_hex("FF0000"), Some(Rgb(0xFF, 0, 0)));
        assert_eq!(Rgb::from_hex("1f3864"), Some(Rgb(0x1F, 0x38, 0x64)));
        assert_eq!(Rgb::from_hex("aBcDeF"), Some(Rgb(0xAB, 0xCD, 0xEF)));
        for bad in ["FF00", "FF00000", "GG0000", "#FF0000", "FF0000;x", "", "auto", " FF0000", "+F0000", "FF000\u{e9}"] {
            assert_eq!(Rgb::from_hex(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn rgb_css_is_lowercase_hash_hex() {
        assert_eq!(Rgb(0xAB, 0x0C, 0xEF).css(), "#ab0cef");
        assert_eq!(Rgb::default().css(), "#000000");
        assert_eq!(Rgb(255, 255, 255).css(), "#ffffff");
    }

    #[test]
    fn a_table_cell_text_is_its_runs_joined() {
        let run = |text: &str| TextRun { text: text.to_string(), ..TextRun::default() };
        let cell = TableCell { runs: vec![run("a "), TextRun { color: Some(Rgb(1, 2, 3)), ..run("red") }], fill: None };
        assert_eq!(cell.text(), "a red");
        assert_eq!(TableCell::default().text(), "");
    }

    #[test]
    fn write_errors_read_as_messages() {
        assert_eq!(WriteError::Changed.to_string(), "The file changed on disk since it was read");
        assert_eq!(WriteError::Failed("denied".to_string()).to_string(), "denied");
    }
}
