# File previews in the right panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Files panel shows csv, html, svg and pdf files the way Claude Code desktop does, the tree has an icon per file kind, any absolute path in a reply is a link, and a picture path opens a lightbox.

**Architecture:** Two pure modules in `forge-entities` (`preview.rs`: file kind and icon by name; `csv_table.rs`: a CSV reader) plus a prose-path pass in `links.rs`. `FileContent` gets a `Pdf` variant (the adapter reads `.pdf` as bytes and `.svg` as text). In `forge-widgets`, the markdown-only "rendered unless in the source set" rule becomes a general `Rendered` view (markdown, csv, html, svg) kept per tab in `FilesCtx.rendered`; a PDF is a `TabKind::Pdf` shown with `<embed>`. `Workspace::open_file` sends a picture path to a new window-level `Lightbox` instead of the Files panel.

**Tech Stack:** Rust, Dioxus 0.7 desktop (WKWebView), `theme.css`, Python harness (`scripts/harness/`).

**Spec:** `docs/superpowers/specs/2026-10-08-file-previews-design.md`. Observations: `docs/superpowers/roadmap/observations.md`, Round 12 (parts 1 to 4).

## Global Constraints

- A csv table shows at most 500 data rows (`CSV_ROW_LIMIT = 500`); over that a note shows above it. The first record is the header.
- An html file renders in `<iframe srcdoc sandbox="">` (no scripts and nothing else allowed; see spec Deviation 5).
- `READ_LIMIT` (10 MiB) stays the cap for every read; a PDF over it is `TooLarge`.
- html, pdf and svg paths clicked in the chat open in the Files panel (a chosen deviation); only picture paths (png, jpg, jpeg, gif, webp, bmp, ico) open the lightbox.
- A bare absolute path is linked only when it starts with `/`, has no space, is not preceded by a letter, digit, `/`, `:`, `.`, `~`, `-`, `_`, `+` or `@`, and ends in an extension of 1 to 8 ASCII letters or digits. Trailing `.` is cut. Paths inside `code`, `pre` and `a` are never linked.
- Zoom of an svg: step 1.25, range 0.1 to 8, Fit sets 1.0.
- UI text in Forge is English ("Show source", "Pick a file"); the csv note is "Only the first 500 rows are shown. Switch to the source to see the rest."
- Code style: match the surrounding crates (plain English doc comments on public items, test names as sentences, signals are `Copy`, no new dependencies).
- Commits end with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` as the history does.

## Review Focus

- A quoted csv field that holds a comma, a doubled quote or a newline stays one cell (Task 2 tests).
- A csv with a UTF-8 BOM, CRLF line ends, a blank line, a row shorter than the header, or no rows at all does not panic and does not show a stray cell (Task 2 tests).
- A path with Korean text or punctuation right after it (`/p/a.md.`, `(/p/a.md)`, `경로: /p/a.md`) links only the path (Task 3 tests).
- A URL (`https://x.com/a.html`), a relative path (`a/b.md`) and `foo:/p/a.md` are not turned into file links (Task 3 tests).
- An svg or html file with a rendered view that is edited in source mode shows the new text when the person toggles back to rendered (Task 6 wiring; checked in Task 9).
- A picture that cannot be read closes the lightbox with a notice instead of leaving a blank dimmed window (Task 7 effect; checked in Task 9).

## File Structure

| File | Change | Responsibility |
|---|---|---|
| `crates/forge-entities/src/preview.rs` | create | `PreviewKind`, `preview_kind`, `has_rendered_view`, `FileIcon`, `icon_of` |
| `crates/forge-entities/src/csv_table.rs` | create | `CsvTable`, `parse_csv`, `CSV_ROW_LIMIT` |
| `crates/forge-entities/src/links.rs` | modify | link bare absolute paths in prose |
| `crates/forge-entities/src/lib.rs` | modify | declare and export the two new modules |
| `crates/forge-ports/src/files_port.rs` | modify | `FileContent::Pdf` |
| `crates/forge-adapter-files/src/lib.rs` | modify | read `.pdf` as `Pdf`, `.svg` as text |
| `crates/forge-widgets/src/files_kinds.rs` | modify | `TabKind::Pdf`, `Rendered`, `render_for`, `RenderedView`, `zoom_step`, PDF body |
| `crates/forge-widgets/src/files_icons.rs` | create | `FileTypeIcon` component |
| `crates/forge-widgets/src/files_state.rs` | modify | `markdown` map becomes `rendered`; `is_markdown` becomes `has_rendered_view` |
| `crates/forge-widgets/src/files_panel.rs` | modify | rendered view, `</>` button for every rendered kind, tree icons |
| `crates/forge-widgets/src/lightbox.rs` | create | `Lightbox` overlay |
| `crates/forge-widgets/src/lib.rs` | modify | declare `files_icons`, `lightbox`; export `Lightbox` |
| `crates/forge-features/src/workspace.rs` | modify | `Picture`, lightbox state, picture routing in `open_file` |
| `crates/forge-features/src/lib.rs` | modify | export `Picture` |
| `crates/forge-pages/src/workspace_page.rs` | modify | mount `Lightbox` |
| `crates/forge-shared/src/theme.css`, `theme.rs` | modify | styles and their tests |
| `scripts/harness/add_preview_samples.py`, `test_add_preview_samples.py`, `launch.sh` | create / modify | sample files and a reply with paths for the real window check |
| `docs/superpowers/roadmap/README.md`, `backlog.md` | modify | record Plan 28 |

Run tests with `cargo test -p <crate>`; the whole workspace with `cargo test --workspace`.

---

### Task 1: File kind and icon by name

**Files:**
- Create: `crates/forge-entities/src/preview.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces: `forge_entities::{PreviewKind, FileIcon, preview_kind, has_rendered_view, icon_of}`:
  - `enum PreviewKind { Csv, Html, Svg, Pdf, Image, Markdown, Text }`
  - `enum FileIcon { Table, Globe, Picture, Code, Doc, File }` (both `Debug, Clone, Copy, PartialEq, Eq`)
  - `fn preview_kind(path: &str) -> PreviewKind`, `fn has_rendered_view(path: &str) -> bool` (true for `Csv`, `Html`, `Svg`, `Markdown`), `fn icon_of(path: &str) -> FileIcon`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/preview.rs` with only the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_kind_comes_from_its_extension_in_any_case() {
        let cases = [
            ("data.csv", PreviewKind::Csv),
            ("DATA.CSV", PreviewKind::Csv),
            ("a/page.html", PreviewKind::Html),
            ("a/page.HTM", PreviewKind::Html),
            ("pic.svg", PreviewKind::Svg),
            ("dir/doc.PDF", PreviewKind::Pdf),
            ("a.png", PreviewKind::Image),
            ("a.JPEG", PreviewKind::Image),
            ("a.jpg", PreviewKind::Image),
            ("a.gif", PreviewKind::Image),
            ("a.webp", PreviewKind::Image),
            ("a.bmp", PreviewKind::Image),
            ("a.ico", PreviewKind::Image),
            ("README.md", PreviewKind::Markdown),
            ("notes.markdown", PreviewKind::Markdown),
            ("main.rs", PreviewKind::Text),
            ("data.json", PreviewKind::Text),
            ("nb.ipynb", PreviewKind::Text),
        ];
        for (path, kind) in cases {
            assert_eq!(preview_kind(path), kind, "{path}");
        }
    }

    #[test]
    fn a_name_with_no_stem_or_no_dot_or_a_longer_ending_is_plain_text() {
        for path in [".csv", "csv", "a.csv.bak", "dir.csv/readme", "", "dir/"] {
            assert_eq!(preview_kind(path), PreviewKind::Text, "{path}");
        }
    }

    #[test]
    fn only_markdown_csv_html_and_svg_have_a_rendered_view() {
        for yes in ["a.md", "a.csv", "a.html", "a.svg"] {
            assert!(has_rendered_view(yes), "{yes}");
        }
        for no in ["a.pdf", "a.png", "a.rs", "a.json", "a"] {
            assert!(!has_rendered_view(no), "{no}");
        }
    }

    #[test]
    fn a_tree_icon_follows_the_kind_and_code_files_get_the_code_icon() {
        let cases = [
            ("a.csv", FileIcon::Table),
            ("a.html", FileIcon::Globe),
            ("a.png", FileIcon::Picture),
            ("a.svg", FileIcon::Picture),
            ("a.md", FileIcon::Doc),
            ("a.py", FileIcon::Code),
            ("a.JSON", FileIcon::Code),
            ("a.css", FileIcon::Code),
            ("a.rs", FileIcon::Code),
            ("a.pdf", FileIcon::File),
            ("a.txt", FileIcon::File),
            ("Makefile", FileIcon::File),
            (".gitignore", FileIcon::File),
            ("a.aiff", FileIcon::File),
        ];
        for (path, icon) in cases {
            assert_eq!(icon_of(path), icon, "{path}");
        }
    }
}
```

- [ ] **Step 2: Declare the module and run the tests to see them fail**

In `crates/forge-entities/src/lib.rs` add `mod preview;` after `mod permission_view;` and `pub use preview::{has_rendered_view, icon_of, preview_kind, FileIcon, PreviewKind};` after the `pub use permission_view::...` line.

Run: `cargo test -p forge-entities preview`
Expected: FAIL to compile (`PreviewKind`, `preview_kind`, ... not found).

- [ ] **Step 3: Write the implementation**

Put this above the `#[cfg(test)]` block in `crates/forge-entities/src/preview.rs`:

```rust
//! What the Files panel shows a file as, and which icon its tree row has. Both come from the file
//! name alone (the extension, in any case), so they are cheap enough to ask for every row.

/// How the Files panel shows a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewKind {
    Csv,
    Html,
    Svg,
    Pdf,
    /// A picture drawn as it is (not svg, which has a source).
    Image,
    Markdown,
    /// Everything else: an editor.
    Text,
}

/// The small picture before a file name in the tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileIcon {
    Table,
    Globe,
    Picture,
    Code,
    Doc,
    File,
}

const IMAGE_EXTENSIONS: [&str; 7] = ["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico"];
const CODE_EXTENSIONS: [&str; 30] = [
    "rs", "py", "js", "mjs", "ts", "tsx", "jsx", "json", "css", "scss", "toml", "yaml", "yml", "sh", "zsh", "go", "java", "c", "h", "cpp", "hpp", "rb",
    "php", "swift", "kt", "sql", "xml", "lua", "ex", "dart",
];

// The extension in lower case; `None` for a name with no dot or with nothing before it.
fn extension_of(path: &str) -> Option<String> {
    let name = path.rsplit('/').next().unwrap_or(path);
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_ascii_lowercase())
}

/// The kind a file is shown as, by its name.
pub fn preview_kind(path: &str) -> PreviewKind {
    match extension_of(path).as_deref() {
        Some("csv") => PreviewKind::Csv,
        Some("html" | "htm") => PreviewKind::Html,
        Some("svg") => PreviewKind::Svg,
        Some("pdf") => PreviewKind::Pdf,
        Some("md" | "markdown") => PreviewKind::Markdown,
        Some(extension) if IMAGE_EXTENSIONS.contains(&extension) => PreviewKind::Image,
        _ => PreviewKind::Text,
    }
}

/// Whether the file opens rendered, with a `</>` button for its source.
pub fn has_rendered_view(path: &str) -> bool {
    matches!(preview_kind(path), PreviewKind::Csv | PreviewKind::Html | PreviewKind::Svg | PreviewKind::Markdown)
}

/// The icon of the file's row in the tree.
pub fn icon_of(path: &str) -> FileIcon {
    match preview_kind(path) {
        PreviewKind::Csv => FileIcon::Table,
        PreviewKind::Html => FileIcon::Globe,
        PreviewKind::Svg | PreviewKind::Image => FileIcon::Picture,
        PreviewKind::Markdown => FileIcon::Doc,
        PreviewKind::Pdf => FileIcon::File,
        PreviewKind::Text => match extension_of(path) {
            Some(extension) if CODE_EXTENSIONS.contains(&extension.as_str()) => FileIcon::Code,
            _ => FileIcon::File,
        },
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-entities preview`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities/src/preview.rs crates/forge-entities/src/lib.rs
git commit -m "feat: the kind and the tree icon of a file come from its name (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: CSV reader

**Files:**
- Create: `crates/forge-entities/src/csv_table.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces: `forge_entities::{CsvTable, parse_csv, CSV_ROW_LIMIT}`:
  - `pub const CSV_ROW_LIMIT: usize = 500;`
  - `pub struct CsvTable { pub header: Vec<String>, pub rows: Vec<Vec<String>>, pub total_rows: usize, pub truncated: bool }` (`Debug, Clone, PartialEq, Eq`); `header` and every row have the same length (the widest record, short ones padded with `""`); `total_rows` counts every data record in the text; `truncated` is `total_rows > limit`
  - `pub fn parse_csv(text: &str, limit: usize) -> CsvTable`

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/csv_table.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn cells(row: &[&str]) -> Vec<String> {
        row.iter().map(|cell| cell.to_string()).collect()
    }

    #[test]
    fn the_first_record_is_the_header_and_the_rest_are_rows() {
        let table = parse_csv("name,age\nAda,36\nLinus,55\n", 500);
        assert_eq!(table.header, cells(&["name", "age"]));
        assert_eq!(table.rows, vec![cells(&["Ada", "36"]), cells(&["Linus", "55"])]);
        assert_eq!(table.total_rows, 2);
        assert!(!table.truncated);
    }

    #[test]
    fn a_quoted_field_keeps_commas_doubled_quotes_and_new_lines() {
        let table = parse_csv("a,b\n\"x, y\",\"say \"\"hi\"\"\"\n\"line one\nline two\",z\n", 500);
        assert_eq!(table.rows[0], cells(&["x, y", "say \"hi\""]));
        assert_eq!(table.rows[1], cells(&["line one\nline two", "z"]));
        assert_eq!(table.total_rows, 2);
    }

    #[test]
    fn windows_line_ends_and_a_byte_order_mark_are_not_part_of_the_cells() {
        let table = parse_csv("\u{feff}a,b\r\n1,2\r\n3,4", 500);
        assert_eq!(table.header, cells(&["a", "b"]));
        assert_eq!(table.rows, vec![cells(&["1", "2"]), cells(&["3", "4"])]);
    }

    #[test]
    fn blank_lines_are_skipped_and_a_last_line_needs_no_new_line() {
        let table = parse_csv("a,b\n\n1,2\n\n\n3,4", 500);
        assert_eq!(table.total_rows, 2);
        assert_eq!(table.rows[1], cells(&["3", "4"]));
    }

    #[test]
    fn a_short_row_is_padded_and_a_wide_row_widens_the_header() {
        let table = parse_csv("a,b,c\n1\n1,2,3,4\n", 500);
        assert_eq!(table.header, cells(&["a", "b", "c", ""]));
        assert_eq!(table.rows[0], cells(&["1", "", "", ""]));
        assert_eq!(table.rows[1], cells(&["1", "2", "3", "4"]));
    }

    #[test]
    fn rows_past_the_limit_are_counted_but_not_kept() {
        let table = parse_csv("n\n1\n2\n3\n4\n5\n", 2);
        assert_eq!(table.rows, vec![cells(&["1"]), cells(&["2"])]);
        assert_eq!(table.total_rows, 5);
        assert!(table.truncated);
    }

    #[test]
    fn exactly_the_limit_is_not_truncated() {
        let table = parse_csv("n\n1\n2\n", 2);
        assert_eq!(table.total_rows, 2);
        assert!(!table.truncated);
    }

    #[test]
    fn an_empty_text_and_a_header_only_text_have_no_rows() {
        let empty = parse_csv("", 500);
        assert!(empty.header.is_empty() && empty.rows.is_empty());
        assert_eq!(empty.total_rows, 0);
        let header_only = parse_csv("a,b\n", 500);
        assert_eq!(header_only.header, cells(&["a", "b"]));
        assert!(header_only.rows.is_empty());
    }

    #[test]
    fn an_empty_quoted_cell_at_the_end_of_a_line_is_kept() {
        let table = parse_csv("a,b\n1,\"\"\n", 500);
        assert_eq!(table.rows[0], cells(&["1", ""]));
    }
}
```

- [ ] **Step 2: Declare the module and run the tests to see them fail**

In `crates/forge-entities/src/lib.rs` add `mod csv_table;` after `mod command_menu;` and `pub use csv_table::{parse_csv, CsvTable, CSV_ROW_LIMIT};` after the `pub use command_menu::...` line.

Run: `cargo test -p forge-entities csv_table`
Expected: FAIL to compile (`parse_csv` not found).

- [ ] **Step 3: Write the implementation**

Put this above the tests in `crates/forge-entities/src/csv_table.rs`:

```rust
//! A small CSV reader for the Files panel's table (RFC 4180: quoted fields, doubled quotes, new
//! lines inside quotes, CRLF). It keeps only the rows the table shows and counts the rest.

/// The most data rows the Files panel's table shows.
pub const CSV_ROW_LIMIT: usize = 500;

/// A csv text as a table: `header` and every row have the same number of cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvTable {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Every data record in the text, kept or not.
    pub total_rows: usize,
    /// `total_rows` is more than the limit the table was read with.
    pub truncated: bool,
}

// Ends the record being read. A line with nothing on it is not a record. `keep` is how many records
// (the header included) are stored.
fn end_record(record: &mut Vec<String>, field: &mut String, seen: &mut usize, kept: &mut Vec<Vec<String>>, keep: usize) {
    record.push(std::mem::take(field));
    let row = std::mem::take(record);
    if row.len() == 1 && row[0].is_empty() {
        return;
    }
    *seen += 1;
    if kept.len() < keep {
        kept.push(row);
    }
}

/// Reads `text` as csv, keeping the header and at most `limit` data rows.
pub fn parse_csv(text: &str, limit: usize) -> CsvTable {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let keep = limit.saturating_add(1);
    let mut kept: Vec<Vec<String>> = Vec::new();
    let mut seen = 0usize;
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => record.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                end_record(&mut record, &mut field, &mut seen, &mut kept, keep);
            }
            '\n' => end_record(&mut record, &mut field, &mut seen, &mut kept, keep),
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        end_record(&mut record, &mut field, &mut seen, &mut kept, keep);
    }

    let columns = kept.iter().map(Vec::len).max().unwrap_or(0);
    for row in &mut kept {
        row.resize(columns, String::new());
    }
    let mut records = kept.into_iter();
    let header = records.next().unwrap_or_default();
    let total_rows = seen.saturating_sub(1);
    CsvTable { header, rows: records.collect(), total_rows, truncated: total_rows > limit }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-entities csv_table`
Expected: PASS (9 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities/src/csv_table.rs crates/forge-entities/src/lib.rs
git commit -m "feat: a csv reader that keeps the first rows and counts the rest (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Link bare absolute paths in prose

**Files:**
- Modify: `crates/forge-entities/src/links.rs` (the function `render_markdown_linked` and the test `text_outside_code_and_code_in_a_fenced_block_stay_plain`)

**Interfaces:**
- Consumes: `render_markdown(source) -> String` (existing); the existing `resolve_escaped`, `file_name`.
- Produces: `render_markdown_linked(source, known)` keeps its signature. Besides the known-file inline code spans, every bare absolute path in a text node becomes `<span class="file-link" data-path="PATH" role="link" tabindex="0">PATH</span>`.

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `links.rs`, replace the test `text_outside_code_and_code_in_a_fenced_block_stay_plain` with this one, and add the others after it:

```rust
    #[test]
    fn a_bare_name_in_prose_and_code_in_a_fenced_block_stay_plain() {
        let prose = render_markdown_linked("guide.md and /p/docs/guide.md", &known());
        assert!(!prose.contains(r#"data-path="guide.md""#), "a bare name is not a path: {prose}");
        assert!(prose.contains(r#"data-path="/p/docs/guide.md""#), "{prose}");
        let fenced = render_markdown_linked("```\n/p/docs/guide.md\n```", &known());
        assert!(!fenced.contains("file-link"), "{fenced}");
        let language = render_markdown_linked("```text\nguide.md\n```", &known());
        assert!(!language.contains("file-link"), "{language}");
    }

    fn linked(text: &str) -> String {
        render_markdown_linked(text, &[])
    }

    #[test]
    fn an_absolute_path_in_prose_becomes_a_link_even_when_no_tool_named_it() {
        let html = linked("Saved to /tmp/out/report.csv for you");
        assert!(html.contains(r#"<span class="file-link" data-path="/tmp/out/report.csv" role="link" tabindex="0">/tmp/out/report.csv</span>"#), "{html}");
    }

    #[test]
    fn punctuation_and_text_around_a_path_stay_outside_the_link() {
        let html = linked("see /p/a.md.");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains("</span>."), "{html}");
        let html = linked("(/p/a.md)");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains("</span>)"), "{html}");
        let html = linked("저장했습니다: /p/보고서.md 입니다");
        assert!(html.contains(r#"data-path="/p/보고서.md""#) && html.contains("</span> 입니다"), "{html}");
    }

    #[test]
    fn two_paths_on_separate_lines_are_two_links() {
        let html = linked("/p/a.md\n\n/p/b.pdf");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains(r#"data-path="/p/b.pdf""#), "{html}");
    }

    #[test]
    fn a_url_a_relative_path_and_a_path_with_no_extension_are_not_linked() {
        for text in ["/usr/local/bin", "https://example.com/a.html", "see /", "a/b/c.md", "//cdn.example.com/a.js", "foo:/p/a.md", "~/a.md", "x/p/a.md"] {
            assert!(!linked(text).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn a_path_in_code_a_code_block_or_a_link_stays_plain() {
        for text in ["`/p/a.md`", "```\n/p/a.md\n```", "[/p/a.md](https://example.com)", "[x](/p/a.md)"] {
            assert!(!linked(text).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn a_path_with_an_ampersand_or_a_quote_is_cut_before_it() {
        assert!(!linked("/p/a&b.md").contains("file-link"));
        assert!(!linked("/p/a\"b.md").contains("file-link"));
    }

    #[test]
    fn prose_paths_and_known_code_spans_link_together() {
        let html = render_markdown_linked("`guide.md` and /p/other.txt", &known());
        assert!(html.contains(r#"data-path="/p/docs/guide.md""#) && html.contains(r#"data-path="/p/other.txt""#), "{html}");
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-entities links`
Expected: FAIL (the new prose tests fail; `a_bare_name_in_prose...` fails on the second assertion).

- [ ] **Step 3: Write the implementation**

Replace the whole function `render_markdown_linked` (including its doc comment) with the code below, and add the helper functions right after it (before `resolve_escaped`):

```rust
/// `render_markdown`, with links to files. Each inline code span that names a known file, and each
/// bare absolute path in the text (`/dir/name.ext`), is wrapped in
/// `<span class="file-link" data-path="…" role="link" tabindex="0">`. Code in a fenced or indented
/// block, text inside an existing link, and text outside code that is not an absolute path are left
/// alone; a path that holds a quote, `<` or `&` is never put in an attribute.
pub fn render_markdown_linked(source: &str, known: &[String]) -> String {
    let html = render_markdown(source);
    link_prose_paths(&link_known_code(&html, known))
}

// The inline code spans that name a known file become links.
fn link_known_code(html: &str, known: &[String]) -> String {
    if known.is_empty() {
        return html.to_string();
    }
    const OPEN: &str = "<code>";
    const CLOSE: &str = "</code>";
    const BLOCK: &str = "<pre class=\"code-block\">";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        let (before, from_code) = rest.split_at(start);
        out.push_str(before);
        let after_open = &from_code[OPEN.len()..];
        let Some(end) = after_open.find(CLOSE) else {
            out.push_str(from_code);
            return out;
        };
        // `before` is already in `out`: a code span right after the block's `<pre>` is block code.
        let inside_block = out.ends_with(BLOCK);
        let text = &after_open[..end];
        let link = if inside_block { None } else { resolve_escaped(text, known) }.filter(|path| !path.contains(['"', '<', '>', '&']));
        match link {
            Some(path) => out.push_str(&format!(
                "<span class=\"file-link\" data-path=\"{path}\" role=\"link\" tabindex=\"0\">{OPEN}{text}{CLOSE}</span>"
            )),
            None => {
                out.push_str(OPEN);
                out.push_str(text);
                out.push_str(CLOSE);
            }
        }
        rest = &after_open[end + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '@' | '~' | '/')
}

// A path names a file when its last part is `stem.ext` with an extension of 1 to 8 letters or digits.
fn has_file_extension(path: &str) -> bool {
    if path.len() < 2 || path.contains("//") {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, extension)) => !stem.is_empty() && (1..=8).contains(&extension.len()) && extension.chars().all(|c| c.is_ascii_alphanumeric()),
        None => false,
    }
}

// The byte ranges of the absolute file paths in one text node of the markdown HTML. The text is
// HTML-escaped, so a path never runs through `&`, `<`, `>` or a quote: those are not path characters.
fn prose_paths(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    let mut previous: Option<char> = None;
    for (index, c) in text.char_indices() {
        if index >= from && c == '/' && previous.is_none_or(|before| !is_path_char(before) && before != ':') {
            let run_end = text[index..].find(|ch: char| !is_path_char(ch)).map_or(text.len(), |offset| index + offset);
            let path = text[index..run_end].trim_end_matches('.');
            if has_file_extension(path) {
                found.push((index, index + path.len()));
            }
            from = run_end;
        }
        previous = Some(c);
    }
    found
}

fn push_linked(out: &mut String, text: &str, link: bool) {
    if !link {
        out.push_str(text);
        return;
    }
    let mut last = 0;
    for (start, end) in prose_paths(text) {
        out.push_str(&text[last..start]);
        let path = &text[start..end];
        out.push_str(&format!("<span class=\"file-link\" data-path=\"{path}\" role=\"link\" tabindex=\"0\">{path}</span>"));
        last = end;
    }
    out.push_str(&text[last..]);
}

// How deep the text is inside `code`, `pre` and `a` after this tag; text in there is never linked.
fn next_skipped(tag: &str, skipped: usize) -> usize {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let closing = inner.starts_with('/');
    let name = inner.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_ascii_lowercase();
    if !matches!(name.as_str(), "code" | "pre" | "a") {
        return skipped;
    }
    if closing {
        skipped.saturating_sub(1)
    } else if inner.ends_with('/') {
        skipped
    } else {
        skipped + 1
    }
}

// Wraps each bare absolute path in the text nodes of `html`.
fn link_prose_paths(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut skipped = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            push_linked(&mut out, rest, skipped == 0);
            break;
        };
        push_linked(&mut out, &rest[..open], skipped == 0);
        let from_tag = &rest[open..];
        let Some(close) = from_tag.find('>') else {
            out.push_str(from_tag);
            break;
        };
        let tag = &from_tag[..=close];
        skipped = next_skipped(tag, skipped);
        out.push_str(tag);
        rest = &from_tag[close + 1..];
    }
    out
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-entities`
Expected: PASS (all `links` tests including the old ones, and the rest of the crate).

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities/src/links.rs
git commit -m "feat: a bare absolute path in a reply is a file link (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: PDF is read as bytes, svg as text

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs` (the `FileContent` enum)
- Modify: `crates/forge-adapter-files/src/lib.rs` (`IMAGE_TYPES`, `read`, tests)
- Modify: `crates/forge-widgets/src/files_state.rs` (`contents_to_copy` and its test)
- Modify: `crates/forge-widgets/src/files_kinds.rs` (`TabKind::Pdf`, `kind_of`, `FileKindView`, tests)
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Produces: `FileContent::Pdf { bytes: Vec<u8> }`; `TabKind::Pdf { data_url: String }` where `data_url` is `data:application/pdf;base64,…`; `FileKindView` draws it as `<embed class="files-pdf" type="application/pdf">`.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-adapter-files/src/lib.rs`, replace the test `an_image_is_read_as_bytes_by_its_extension_in_any_case` and add the pdf and svg tests:

```rust
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
```

In `crates/forge-widgets/src/files_kinds.rs` tests, add:

```rust
    #[test]
    fn a_pdf_becomes_a_data_address_with_its_type_and_has_no_editor() {
        let kind = kind_of(FileContent::Pdf { bytes: vec![1, 2, 3] });
        assert_eq!(kind, TabKind::Pdf { data_url: "data:application/pdf;base64,AQID".to_string() });
        assert!(!kind.shows_editor());
        assert_eq!(kind.notice(), None);
    }
```

In `crates/forge-widgets/src/files_state.rs` tests, inside `only_a_text_file_can_be_copied_as_contents`, add this line after the `Image` assertion:

```rust
        assert!(contents_to_copy(&FileContent::Pdf { bytes: vec![1] }).is_err());
```

In `crates/forge-shared/src/theme.rs`, add `".files-pdf",` to the list of required classes (after `".files-kind-button",`).

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-adapter-files -p forge-widgets -p forge-shared`
Expected: FAIL to compile (`FileContent::Pdf` not found).

- [ ] **Step 3: Write the implementation**

`crates/forge-ports/src/files_port.rs`, in `enum FileContent` add after the `Image` variant:

```rust
    /// A PDF, by its extension; shown by the web view's own viewer.
    Pdf { bytes: Vec<u8> },
```

`crates/forge-adapter-files/src/lib.rs`: change `IMAGE_TYPES` to seven entries (remove the `svg` line):

```rust
const IMAGE_TYPES: [(&str, &str); 7] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("bmp", "image/bmp"),
    ("ico", "image/x-icon"),
];
```

Add above `fn image_mime`:

```rust
fn is_pdf(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| extension.eq_ignore_ascii_case("pdf"))
}
```

In `fn read` (the one that has `if let Some(mime) = image_mime(&file)`), insert before that `if let`:

```rust
        if is_pdf(&file) {
            return Ok(FileContent::Pdf { bytes });
        }
```

`crates/forge-widgets/src/files_state.rs`, `contents_to_copy`: change the last arm to

```rust
        FileContent::Binary | FileContent::Image { .. } | FileContent::Pdf { .. } => Err("Only a text file can be copied".to_string()),
```

`crates/forge-widgets/src/files_kinds.rs`:

```rust
// in enum TabKind, after `Image { data_url: String },`
    Pdf { data_url: String },
```

```rust
// in fn kind_of, after the Image arm
        FileContent::Pdf { bytes } => {
            TabKind::Pdf { data_url: format!("data:application/pdf;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
        }
```

```rust
// in fn FileKindView, after the TabKind::Image arm
        TabKind::Pdf { data_url } => rsx! {
            embed { class: "files-pdf", src: "{data_url}", r#type: "application/pdf" }
        },
```

`crates/forge-shared/src/theme.css`, after the `.files-kind-button:hover` line add:

```css
.files-pdf { flex: 1; width: 100%; min-height: 0; border: none; background: #2b2b2b; }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test --workspace`
Expected: PASS. If the compiler names another `match` on `FileContent` that is not exhaustive, add a `Pdf` arm there that behaves like `Binary` (nothing else in the workspace matches on it today).

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: a pdf is read as bytes and shown by the web view's viewer, an svg is read as text (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Rendered views (csv, html, svg, markdown) as components

**Files:**
- Modify: `crates/forge-widgets/src/files_kinds.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `forge_entities::{parse_csv, preview_kind, render_markdown, CsvTable, PreviewKind, CSV_ROW_LIMIT}` (Tasks 1 and 2).
- Produces (in `files_kinds.rs`):
  - `pub enum Rendered { Markdown(String), Csv(CsvTable), Html(String), Svg(String) }` (`Debug, Clone, PartialEq`); `Markdown` holds the HTML, `Html` the file's source text, `Svg` a `data:image/svg+xml;base64,…` address
  - `pub fn render_for(path: &str, text: &str) -> Option<Rendered>` (`None` unless `has_rendered_view(path)`)
  - `pub fn zoom_step(current: f64, direction: i32) -> f64` (direction `1` in, `-1` out, anything else leaves it)
  - `#[component] pub fn RenderedView(rendered: Rendered, path: String) -> Element`

- [ ] **Step 1: Write the failing tests**

In the `tests` module of `files_kinds.rs` add:

```rust
    #[test]
    fn each_rendered_kind_is_made_from_its_text_and_other_files_have_no_rendered_view() {
        assert!(matches!(render_for("a.md", "# Hi"), Some(Rendered::Markdown(html)) if html.contains("<h1>")));
        match render_for("a.csv", "a,b\n1,2\n") {
            Some(Rendered::Csv(table)) => assert_eq!(table.rows.len(), 1),
            other => panic!("{other:?}"),
        }
        assert_eq!(render_for("a.html", "<p>x</p>"), Some(Rendered::Html("<p>x</p>".to_string())));
        assert_eq!(
            render_for("a.svg", "<svg/>"),
            Some(Rendered::Svg("data:image/svg+xml;base64,PHN2Zy8+".to_string()))
        );
        for plain in ["a.rs", "a.json", "a.png", "a.pdf", "a"] {
            assert_eq!(render_for(plain, "x"), None, "{plain}");
        }
    }

    #[test]
    fn the_csv_view_is_cut_at_the_row_limit() {
        let text: String = std::iter::once("n\n".to_string()).chain((0..CSV_ROW_LIMIT + 3).map(|n| format!("{n}\n"))).collect();
        let Some(Rendered::Csv(table)) = render_for("big.csv", &text) else { panic!("not a table") };
        assert_eq!(table.rows.len(), CSV_ROW_LIMIT);
        assert!(table.truncated);
    }

    #[test]
    fn zooming_steps_by_a_quarter_and_stays_between_a_tenth_and_eight() {
        assert_eq!(zoom_step(1.0, 1), 1.25);
        assert_eq!(zoom_step(1.25, -1), 1.0);
        assert_eq!(zoom_step(7.0, 1), 8.0);
        assert_eq!(zoom_step(8.0, 1), 8.0);
        assert_eq!(zoom_step(0.11, -1), 0.1);
        assert_eq!(zoom_step(0.1, -1), 0.1);
        assert_eq!(zoom_step(2.0, 0), 2.0);
    }
```

In `crates/forge-shared/src/theme.rs` add these names to the required class list (after `".files-pdf",`): `".files-html",`, `".files-csv",`, `".files-csv-note",`, `".files-svg",`, `".files-svg-bar",`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-widgets files_kinds`
Expected: FAIL to compile (`render_for`, `Rendered`, `zoom_step` not found).

- [ ] **Step 3: Write the implementation**

At the top of `files_kinds.rs` change the imports to:

```rust
use base64::Engine;
use dioxus::prelude::*;
use forge_entities::{parse_csv, preview_kind, render_markdown, CsvTable, PreviewKind, CSV_ROW_LIMIT};
use forge_ports::FileContent;
```

Add after `kind_of`:

```rust
/// What a file with a rendered view shows when it is not in source mode.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendered {
    /// The markdown as HTML.
    Markdown(String),
    Csv(CsvTable),
    /// The file's own text, drawn in a sandboxed frame.
    Html(String),
    /// A `data:` address of the svg.
    Svg(String),
}

/// The rendered view of `text` for the file at `path`; `None` for a file with no rendered view.
pub fn render_for(path: &str, text: &str) -> Option<Rendered> {
    match preview_kind(path) {
        PreviewKind::Markdown => Some(Rendered::Markdown(render_markdown(text))),
        PreviewKind::Csv => Some(Rendered::Csv(parse_csv(text, CSV_ROW_LIMIT))),
        PreviewKind::Html => Some(Rendered::Html(text.to_string())),
        PreviewKind::Svg => Some(Rendered::Svg(format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(text)))),
        _ => None,
    }
}

const ZOOM_STEP: f64 = 1.25;
const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 8.0;

/// The zoom after one step: `1` in, `-1` out, any other direction leaves it.
pub fn zoom_step(current: f64, direction: i32) -> f64 {
    match direction {
        1 => (current * ZOOM_STEP).min(ZOOM_MAX),
        -1 => (current / ZOOM_STEP).max(ZOOM_MIN),
        _ => current,
    }
}

/// The body of a tab that is shown rendered. Keyed by the path by the caller, so a tab keeps its own zoom.
#[component]
pub fn RenderedView(rendered: Rendered, path: String) -> Element {
    match rendered {
        Rendered::Markdown(html) => rsx! {
            div { class: "changes-diff",
                div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{html}" }
            }
        },
        Rendered::Csv(table) => rsx! { CsvView { table, path } },
        // No `allow-same-origin`, popups, forms or top navigation: a page can run its own script and
        // nothing else, and a link in it goes nowhere.
        Rendered::Html(source) => rsx! {
            iframe { class: "files-html", title: "{path}", srcdoc: "{source}", "sandbox": HTML_SANDBOX }
        },
        Rendered::Svg(data_url) => rsx! { SvgView { data_url, path } },
    }
}

#[component]
fn CsvView(table: CsvTable, path: String) -> Element {
    rsx! {
        div { class: "changes-diff",
            div { class: "file-viewer file-markdown markdown files-csv", "aria-label": "{path}",
                if table.truncated {
                    p { class: "files-csv-note", "Only the first {CSV_ROW_LIMIT} rows are shown. Switch to the source to see the rest." }
                }
                table {
                    thead {
                        tr {
                            for (index, cell) in table.header.iter().enumerate() {
                                th { key: "{index}", "{cell}" }
                            }
                        }
                    }
                    tbody {
                        for (row_index, row) in table.rows.iter().enumerate() {
                            tr { key: "{row_index}",
                                for (index, cell) in row.iter().enumerate() {
                                    td { key: "{index}", "{cell}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SvgView(data_url: String, path: String) -> Element {
    let mut scale = use_signal(|| 1.0_f64);
    rsx! {
        div { class: "files-svg",
            div { class: "files-svg-stage",
                img { src: "{data_url}", alt: "{path}", style: "zoom: {scale}" }
            }
            div { class: "files-svg-bar",
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: "Zoom out",
                    "aria-label": "Zoom out",
                    onclick: move |_| scale.set(zoom_step(scale(), -1)),
                    "−"
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: "Zoom in",
                    "aria-label": "Zoom in",
                    onclick: move |_| scale.set(zoom_step(scale(), 1)),
                    "+"
                }
                button { class: "icon-button", r#type: "button", title: "Fit", "aria-label": "Fit", onclick: move |_| scale.set(1.0), "⤢" }
            }
        }
    }
}
```

Append to `crates/forge-shared/src/theme.css` (after the `.files-pdf` line from Task 4):

```css
.files-html { flex: 1; width: 100%; min-height: 0; border: none; background: #fff; }
.files-csv { overflow: auto; }
.files-csv table { width: max-content; min-width: 100%; }
.files-csv td { max-width: 320px; overflow-wrap: anywhere; }
.files-csv-note { margin: 0 0 8px; color: var(--fg-muted); font-size: 12px; }
.files-svg { position: relative; flex: 1; min-height: 0; display: flex; flex-direction: column; background: #2b2b2b; }
.files-svg-stage { flex: 1; min-height: 0; overflow: auto; padding: 12px; }
.files-svg-bar { position: absolute; right: 10px; bottom: 10px; display: flex; flex-direction: column; gap: 4px; padding: 4px; border-radius: 8px; background: rgba(255, 255, 255, 0.85); }
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-widgets -p forge-shared`
Expected: PASS. (`Rendered`, `RenderedView` are not used by the panel yet; unused-code warnings are fine until Task 6.)

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: csv table, sandboxed html frame and zoomable svg views for the Files panel (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Use the rendered views in the panel

**Files:**
- Modify: `crates/forge-widgets/src/files_state.rs`
- Modify: `crates/forge-widgets/src/files_panel.rs`

**Interfaces:**
- Consumes: `Rendered`, `render_for`, `RenderedView` (Task 5); `has_rendered_view` (Task 1).
- Produces: `FilesCtx.rendered: Signal<HashMap<String, Rendered>>` replaces `FilesCtx.markdown`; every file with a rendered view opens rendered, shows the `</>` button, and its editor only in source mode.

This task changes behavior of existing code and the real behavior is checked in Task 9; the compiler and the unit tests of Tasks 1 to 5 catch the rest.

- [ ] **Step 1: Edit `files_state.rs`**

Apply these replacements (each `old` appears once):

```rust
// 1 — imports
// old
use forge_entities::{code_html, is_markdown, language_of, render_markdown};
// new
use forge_entities::{code_html, has_rendered_view, language_of};
```

```rust
// 2
// old
use crate::files_kinds::{kind_of, TabKind};
// new
use crate::files_kinds::{kind_of, render_for, Rendered, TabKind};
```

```rust
// 3 — the field
// old
    /// Rendered markdown of a markdown tab.
    pub markdown: Signal<HashMap<String, String>>,
    /// Markdown tabs shown as source (editable) instead of rendered.
    pub source: Signal<HashSet<String>>,
// new
    /// The rendered view of a markdown, csv, html or svg tab.
    pub rendered: Signal<HashMap<String, Rendered>>,
    /// Tabs with a rendered view that show their source (editable) instead.
    pub source: Signal<HashSet<String>>,
```

```rust
// 4 — find_in_file
// old
        // A markdown file opens rendered, which has no lines to find in: show its source.
        if is_markdown(path) {
// new
        // A file with a rendered view opens rendered, which has no lines to find in: show its source.
        if has_rendered_view(path) {
```

```rust
// 5 — editor_visible
// old
    // Whether the editor in the page is what the tab shows: a text tab, and for markdown only in source mode.
// new
    // Whether the editor in the page is what the tab shows: a text tab, and for a file with a rendered view only in source mode.
// old
            && (!is_markdown(path) || self.source.peek().contains(path))
// new
            && (!has_rendered_view(path) || self.source.peek().contains(path))
```

```rust
// 6 — take_text
// old
    // The text of a file that was just read, line ends as `\n`: markdown is rendered, and the editor
    // takes it when it is the selected tab's view.
    fn take_text(&self, path: &str, text: &str, read_only: bool, quiet: bool, dirty: bool) {
        if is_markdown(path) {
            let mut markdown = self.markdown;
            markdown.write().insert(path.to_string(), render_markdown(text));
        }
// new
    // The text of a file that was just read, line ends as `\n`: a file with a rendered view is
    // rendered, and the editor takes it when it is the selected tab's view.
    fn take_text(&self, path: &str, text: &str, read_only: bool, quiet: bool, dirty: bool) {
        if let Some(view) = render_for(path, text) {
            let mut rendered = self.rendered;
            rendered.write().insert(path.to_string(), view);
        }
```

```rust
// 7 — flushed, the Render arm
// old
            Pending::Render => {
                if let Some(text) = text {
                    let mut markdown = self.markdown;
                    markdown.write().insert(key, render_markdown(&text));
                }
            }
// new
            Pending::Render => {
                let view = text.as_deref().and_then(|text| render_for(&key, text));
                if let Some(view) = view {
                    let mut rendered = self.rendered;
                    rendered.write().insert(key, view);
                }
            }
```

```rust
// 8 — forget
// old
        let (mut kinds, mut docs, mut markdown, mut source, mut pending, mut fallback) =
            (self.kinds, self.docs, self.markdown, self.source, self.pending, self.fallback);
// new
        let (mut kinds, mut docs, mut rendered, mut source, mut pending, mut fallback) =
            (self.kinds, self.docs, self.rendered, self.source, self.pending, self.fallback);
// old
        markdown.write().remove(path);
// new
        rendered.write().remove(path);
```

```rust
// 9 — the toggle's doc comment
// old
    /// The markdown `</>` button: source (editable) or rendered.
// new
    /// The `</>` button of a file with a rendered view: source (editable) or rendered.
```

The `Pending::Render` doc line (`/// A markdown tab goes back to its rendered view.`) becomes `/// A tab with a rendered view goes back to it.`

- [ ] **Step 2: Edit `files_panel.rs`**

```rust
// 1 — imports
// old
use forge_entities::is_markdown;
// new
use forge_entities::{has_rendered_view, icon_of};
// old
use crate::files_kinds::{FileKindView, TabKind};
// new
use crate::files_icons::FileTypeIcon;
use crate::files_kinds::{FileKindView, RenderedView, TabKind};
```

```rust
// 2 — the context
// old
        markdown: use_signal(HashMap::new),
// new
        rendered: use_signal(HashMap::new),
```

```rust
// 3 — the comment above the store effect
// old
    // The tabs and the markdown source set go to the store whenever they change, so they outlive the panel.
// new
    // The tabs and the source set go to the store whenever they change, so they outlive the panel.
```

```rust
// 4 — the flags
// old
    let markdown_selected = is_text && selected.as_deref().is_some_and(is_markdown);
    let source_on = selected.as_ref().is_some_and(|path| ctx.source.read().contains(path));
    let shows_editor = is_text && !unavailable && (!markdown_selected || source_on);
// new
    let rendered_selected = is_text && selected.as_deref().is_some_and(has_rendered_view);
    let source_on = selected.as_ref().is_some_and(|path| ctx.source.read().contains(path));
    let shows_editor = is_text && !unavailable && (!rendered_selected || source_on);
```

```rust
// 5 — the toggle button
// old
        if markdown_selected {
            button {
                class: "icon-button files-source-toggle",
// new
        if rendered_selected {
            button {
                class: "icon-button files-source-toggle",
```

```rust
// 6 — the body
// old
        (Some(path), Some(TabKind::Text { .. })) if markdown_selected && !source_on => {
            let html = ctx.markdown.read().get(path).cloned().unwrap_or_default();
            rsx! { div { class: "changes-diff", div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{html}" } } }
        }
// new
        (Some(path), Some(TabKind::Text { .. })) if rendered_selected && !source_on => match ctx.rendered.read().get(path).cloned() {
            Some(rendered) => rsx! { RenderedView { key: "{path}", rendered, path: path.clone() } },
            None => rsx! { div { class: "dock-note", "Loading…" } },
        },
```

- [ ] **Step 3: Create a stub for the icon module so the crate compiles**

The tree icon is Task 7's job; for now make `files_icons.rs` and register it so the import in step 2 resolves. Create `crates/forge-widgets/src/files_icons.rs`:

```rust
use dioxus::prelude::*;
use forge_entities::FileIcon;

/// The small picture before a file name in the tree.
#[component]
pub fn FileTypeIcon(icon: FileIcon) -> Element {
    let _ = icon;
    rsx! {}
}
```

In `crates/forge-widgets/src/lib.rs` add `mod files_icons;` after `mod files_editor;`.

- [ ] **Step 4: Build and run the tests**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: build OK, all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: csv, html and svg files open rendered with a </> source button like markdown (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Tree icons

**Files:**
- Modify: `crates/forge-widgets/src/files_icons.rs` (replace the stub)
- Modify: `crates/forge-widgets/src/files_panel.rs` (the tree row)
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `FileIcon`, `icon_of` (Task 1).
- Produces: `FileTypeIcon { icon: FileIcon }` draws a 14 px inline svg in the muted colour.

- [ ] **Step 1: Write the failing test**

In `crates/forge-shared/src/theme.rs` add `".files-icon",` to the required class list (after `".files-svg-bar",`).

- [ ] **Step 2: Run the test to see it fail**

Run: `cargo test -p forge-shared`
Expected: FAIL ("theme.css is missing .files-icon").

- [ ] **Step 3: Write the implementation**

Replace the whole content of `crates/forge-widgets/src/files_icons.rs`:

```rust
use dioxus::prelude::*;
use forge_entities::FileIcon;

/// The small picture before a file name in the tree: a table for csv, a globe for html, a picture
/// for images and svg, `</>` for code, a page with lines for markdown, a plain page for the rest.
#[component]
pub fn FileTypeIcon(icon: FileIcon) -> Element {
    rsx! {
        svg {
            class: "files-icon",
            view_box: "0 0 16 16",
            width: "14",
            height: "14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match icon {
                FileIcon::Table => rsx! {
                    rect { x: "2", y: "3", width: "12", height: "10", rx: "1.5" }
                    path { d: "M2 7h12M6 3v10" }
                },
                FileIcon::Globe => rsx! {
                    circle { cx: "8", cy: "8", r: "6" }
                    path { d: "M2 8h12M8 2c2 2 2 10 0 12M8 2c-2 2-2 10 0 12" }
                },
                FileIcon::Picture => rsx! {
                    rect { x: "2", y: "3", width: "12", height: "10", rx: "1.5" }
                    circle { cx: "6", cy: "7", r: "1" }
                    path { d: "M2.5 12l3.5-3 3 2.5 2-1.5 2.5 2" }
                },
                FileIcon::Code => rsx! { path { d: "M6 5L3 8l3 3M10 5l3 3-3 3" } },
                FileIcon::Doc => rsx! { path { d: "M4 2h5l3 3v9H4zM9 2v3h3M6 8h4M6 10.5h4" } },
                FileIcon::File => rsx! { path { d: "M4 2h5l3 3v9H4zM9 2v3h3" } },
            }
        }
    }
}
```

In `files_panel.rs` replace the row's mark line:

```rust
// old
                                        span { class: "files-mark", "{mark}" }
                                        span { "{row.name}" }
// new
                                        if is_dir {
                                            span { class: "files-mark", "{mark}" }
                                        } else {
                                            FileTypeIcon { icon: icon_of(&row.path) }
                                        }
                                        span { "{row.name}" }
```

Append to `crates/forge-shared/src/theme.css`:

```css
.files-icon { flex: none; align-self: center; color: var(--fg-muted); }
```

- [ ] **Step 4: Run the tests and build**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: PASS. If `stroke_linecap` or `stroke_linejoin` is not a known attribute of `svg` in Dioxus 0.7.10, write them as string attributes: `"stroke-linecap": "round"` and `"stroke-linejoin": "round"` (and `"stroke-width": "1.3"` likewise) and build again.

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: the Files tree shows an icon per kind of file (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 8: The picture lightbox

**Files:**
- Modify: `crates/forge-features/src/workspace.rs` (struct, constructor, `open_file`, new methods)
- Modify: `crates/forge-features/src/lib.rs` (export `Picture`)
- Create: `crates/forge-widgets/src/lightbox.rs`
- Modify: `crates/forge-widgets/src/lib.rs`
- Modify: `crates/forge-pages/src/workspace_page.rs`
- Modify: `crates/forge-shared/src/theme.css`, `crates/forge-shared/src/theme.rs`

**Interfaces:**
- Consumes: `forge_entities::{preview_kind, PreviewKind}` (Task 1); `Workspace::files()`, `Workspace::report(String)`.
- Produces: `forge_features::Picture { folder: PathBuf, relative: String }`; `Workspace::lightbox() -> Option<Picture>`, `Workspace::close_lightbox()`; `forge_widgets::Lightbox` component with no props. `Workspace::open_file(id, absolute_path)` sets the lightbox for a picture path (png, jpg, jpeg, gif, webp, bmp, ico) and does not touch the dock; every other path behaves as before.

- [ ] **Step 1: Write the failing tests**

In `crates/forge-shared/src/theme.rs` add `".lightbox-backdrop",`, `".lightbox-image",`, `".lightbox-close",`, `".lightbox-caption",` to the required class list (after `".files-icon",`), and add this test next to the other z-index tests:

```rust
    #[test]
    fn the_lightbox_sits_above_settings_and_the_search_palette() {
        let lightbox = z_index(".lightbox-backdrop");
        assert!(lightbox > z_index(".settings-backdrop"));
        assert!(lightbox > z_index(".palette-backdrop"));
    }
```

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-shared`
Expected: FAIL (no rule for `.lightbox-backdrop`).

- [ ] **Step 3: Write the implementation**

`crates/forge-shared/src/theme.css` — append:

```css
.lightbox-backdrop { position: fixed; inset: 0; z-index: 40; display: flex; align-items: center; justify-content: center; background: var(--bg-overlay); outline: none; cursor: zoom-out; }
.lightbox-figure { margin: 0; display: flex; flex-direction: column; align-items: center; gap: 10px; max-width: 80vw; }
.lightbox-frame { position: relative; display: flex; }
.lightbox-image { max-width: 80vw; max-height: calc(80vh - 40px); border-radius: 4px; }
.lightbox-close { position: absolute; top: -30px; right: -30px; width: 26px; height: 26px; border: none; background: transparent; color: #fff; font: inherit; font-size: 24px; line-height: 1; cursor: pointer; }
.lightbox-caption { max-width: 80vw; color: #fff; font-size: 14px; text-align: center; overflow-wrap: anywhere; }
```

`crates/forge-features/src/workspace.rs`:

1. Add the type above `pub struct Workspace` (after `WorkspaceServices`' definition, anywhere before the struct):

```rust
/// A picture shown over the window by a click on a picture path in the chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Picture {
    /// The session folder, which `FilesPort::read` takes as its root.
    pub folder: PathBuf,
    /// The picture's path inside the folder.
    pub relative: String,
}
```

2. Add the field in `pub struct Workspace` after `file_requests`:

```rust
    lightbox: Signal<Option<Picture>>,
```

and in the constructor after `file_requests: Signal::new(HashMap::new()),`:

```rust
            lightbox: Signal::new(None),
```

3. In `open_file`, after the `let Some(relative) = ... else { ... return; };` block and before `self.change_dock(...)`, insert:

```rust
        // A picture opens in the lightbox over the window, not in the Files panel.
        if forge_entities::preview_kind(&relative) == forge_entities::PreviewKind::Image {
            let mut lightbox = self.lightbox;
            lightbox.set(Some(Picture { folder, relative }));
            return;
        }
```

Update the doc comment of `open_file` by adding the sentence: `A picture opens in the lightbox instead.`

4. Add after `clear_file_request`:

```rust
    /// The picture shown over the window, if any.
    pub fn lightbox(&self) -> Option<Picture> {
        self.lightbox.read().clone()
    }

    pub fn close_lightbox(&self) {
        let mut lightbox = self.lightbox;
        lightbox.set(None);
    }
```

`crates/forge-features/src/lib.rs`: change the workspace export line to

```rust
pub use workspace::{use_workspace, use_workspace_provider, Active, Picture, Workspace, WorkspaceServices};
```

Create `crates/forge-widgets/src/lightbox.rs`:

```rust
use base64::Engine;
use dioxus::prelude::*;
use forge_features::{run_blocking, use_workspace, Picture};
use forge_ports::FileContent;

/// A picture over the dimmed window, opened by a click on a picture path in the chat: the picture
/// in the middle, its path under it, `×` at its corner. A click anywhere, `×` or Escape closes it.
/// It draws nothing while no picture is asked for.
#[component]
pub fn Lightbox() -> Element {
    let workspace = use_workspace();
    match workspace.lightbox() {
        Some(picture) => rsx! { LightboxBody { key: "{picture.relative}", picture } },
        None => rsx! {},
    }
}

#[component]
fn LightboxBody(picture: Picture) -> Element {
    let workspace = use_workspace();
    let source = picture.clone();
    let image = use_resource(move || {
        let (files, folder, relative) = (workspace.files(), source.folder.clone(), source.relative.clone());
        async move { run_blocking(move || files.read(&folder, &relative)).await }
    });
    // A picture that cannot be read or is too large closes the lightbox with a notice, so the window
    // is never left dimmed with nothing in it.
    use_effect(move || {
        if let Some(done) = &*image.read() {
            if !matches!(done, Some(Ok(FileContent::Image { .. }))) {
                workspace.report("This picture cannot be shown.".to_string());
                workspace.close_lightbox();
            }
        }
    });
    let caption = picture.folder.join(&picture.relative).to_string_lossy().to_string();
    let data_url = match &*image.read() {
        Some(Some(Ok(FileContent::Image { bytes, mime }))) => {
            Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
        }
        _ => None,
    };
    rsx! {
        div {
            class: "lightbox-backdrop",
            tabindex: "0",
            role: "dialog",
            "aria-label": "Picture",
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onclick: move |_| workspace.close_lightbox(),
            onkeydown: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.stop_propagation();
                    workspace.close_lightbox();
                }
            },
            if let Some(data_url) = data_url {
                figure { class: "lightbox-figure",
                    div { class: "lightbox-frame",
                        img { class: "lightbox-image", src: "{data_url}", alt: "{caption}" }
                        button {
                            class: "lightbox-close",
                            r#type: "button",
                            title: "Close",
                            "aria-label": "Close",
                            onclick: move |event| {
                                event.stop_propagation();
                                workspace.close_lightbox();
                            },
                            "×"
                        }
                    }
                    figcaption { class: "lightbox-caption", "{caption}" }
                }
            }
        }
    }
}
```

`crates/forge-widgets/src/lib.rs`: add `mod lightbox;` after `mod filter_menu;` and `pub use lightbox::Lightbox;` after `pub use filter_menu::SidebarFilterMenu;`.

`crates/forge-pages/src/workspace_page.rs`: change the import to `use forge_widgets::{ChatWindow, Lightbox, NewSessionPanel, SearchPalette, SettingsOverlay, Sidebar};` and add `Lightbox {}` on the line after `SearchPalette {}`.

- [ ] **Step 4: Run the tests and build**

Run: `cargo build -p forge-app && cargo test --workspace`
Expected: PASS. If `workspace.files()` is not callable from a `Copy` workspace inside the `use_resource` closure, clone the `Arc` first: `let files = workspace.files();` outside the closure and move it in (it is an `Arc<dyn FilesPort>`).

- [ ] **Step 5: Commit**

```bash
git add -A crates
git commit -m "feat: a picture path in the chat opens a lightbox over the window (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Samples for the real window, and the real window check

**Files:**
- Create: `scripts/harness/add_preview_samples.py`
- Create: `scripts/harness/test_add_preview_samples.py`
- Modify: `scripts/harness/launch.sh`
- Modify: `scripts/harness/README.md`

**Interfaces:**
- Consumes: `build_fixture.build`, `build_fixture.escape_project_path`, `build_fixture.DEFAULT_PROJECTS_ROOT`.
- Produces: `add_preview_samples.add(claude_home, projects_root)`: writes `samples/` (csv, big csv, json, html, svg, png, pdf, multi page pdf) into the fixture folder `my-app` and appends to session 6 ("Read the config") a reply whose paragraphs are those absolute paths (and one path outside the folder); `launch.sh` runs the script named in `FORGE_HARNESS_EXTRA` after the fixture is built.

- [ ] **Step 1: Write the failing test**

Create `scripts/harness/test_add_preview_samples.py`:

```python
import json
import os
import tempfile
import unittest
from pathlib import Path

import add_preview_samples as samples
import build_fixture as bf


class AddPreviewSamplesTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(os.path.realpath(tempfile.mkdtemp()))
        self.run_dir = self.tmp / "run"
        self.projects = self.tmp / "projects"
        self.result = bf.build(self.run_dir, self.projects)
        samples.add(self.result["claude_home"], self.result["projects_root"])
        self.folder = self.result["projects_root"] / "my-app" / "samples"

    def test_every_sample_file_is_written(self):
        for name in ("data.csv", "big.csv", "data.json", "page.html", "pic.svg", "pic.png", "doc.pdf", "multi.pdf"):
            self.assertTrue((self.folder / name).is_file(), name)

    def test_the_big_csv_is_over_the_row_limit(self):
        self.assertGreater(len((self.folder / "big.csv").read_text().splitlines()), 600)

    def test_the_pdfs_look_like_pdfs_and_the_long_one_has_twelve_pages(self):
        self.assertTrue((self.folder / "doc.pdf").read_bytes().startswith(b"%PDF"))
        self.assertIn(b"/Count 12", (self.folder / "multi.pdf").read_bytes())

    def test_the_reply_names_each_sample_by_its_absolute_path(self):
        project_dir = self.result["claude_home"] / "projects" / bf.escape_project_path(self.result["projects_root"] / "my-app")
        transcript = project_dir / "00000000-0000-4000-8000-000000000006.jsonl"
        last = json.loads(transcript.read_text().splitlines()[-1])
        text = last["message"]["content"][0]["text"]
        for name in ("data.csv", "page.html", "doc.pdf", "pic.png", "pic.svg"):
            self.assertIn(str(self.folder / name), text)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the test to see it fail**

Run: `cd scripts/harness && python3 -m unittest test_add_preview_samples -v; cd ../..`
Expected: FAIL (`ModuleNotFoundError: No module named 'add_preview_samples'`).

- [ ] **Step 3: Write the implementation**

Create `scripts/harness/add_preview_samples.py`:

```python
#!/usr/bin/env python3
"""Adds preview samples to the harness fixture: files in `my-app/samples/` and a reply in the session
"Read the config" that names five of them (and one path outside the folder) by absolute path, so a link in the chat can be clicked.

Run it through launch.sh:  FORGE_HARNESS_EXTRA=scripts/harness/add_preview_samples.py scripts/harness/launch.sh
"""
import json
import os
import struct
import sys
import zlib
from pathlib import Path

import build_fixture as bf

SESSION_ID = "00000000-0000-4000-8000-000000000006"


def make_png(width=60, height=40):
    rows = b"".join(b"\x00" + bytes([200, 60, 60]) * width for _ in range(height))

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b"")


def make_pdf(pages):
    objects = ["<</Type/Catalog/Pages 2 0 R>>"]
    kids = " ".join(f"{3 + 2 * i} 0 R" for i in range(pages))
    objects.append(f"<</Type/Pages/Kids[{kids}]/Count {pages}>>")
    font_id = 3 + 2 * pages
    for i in range(pages):
        objects.append(
            f"<</Type/Page/Parent 2 0 R/MediaBox[0 0 612 792]/Contents {4 + 2 * i} 0 R/Resources<</Font<</F1 {font_id} 0 R>>>>>>"
        )
        stream = f"BT /F1 48 Tf 72 700 Td (Page {i + 1} of {pages}) Tj ET"
        objects.append(f"<</Length {len(stream)}>>stream\n{stream}\nendstream")
    objects.append("<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>")
    out = b"%PDF-1.4\n"
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{number} 0 obj\n{body}\nendobj\n".encode()
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    for offset in offsets:
        out += f"{offset:010d} 00000 n \n".encode()
    out += f"trailer<</Size {len(objects) + 1}/Root 1 0 R>>\nstartxref\n{xref}\n%%EOF\n".encode()
    return out


HTML = (
    '<!doctype html><html><body><h1>Hello HTML</h1><p id="out">script has not run</p>'
    "<script>document.getElementById('out').textContent='script RAN';</script>"
    '<a href="https://example.com">link</a></body></html>\n'
)
SVG = (
    '<svg xmlns="http://www.w3.org/2000/svg" width="120" height="80"><rect width="120" height="80" fill="#4a7"/>'
    '<circle cx="60" cy="40" r="25" fill="#fc3"/></svg>\n'
)


def add(claude_home, projects_root):
    cwd = Path(projects_root) / "my-app"
    folder = cwd / "samples"
    folder.mkdir(parents=True, exist_ok=True)
    (folder / "data.csv").write_text("name,age,city\nAda,36,London\nLinus,55,Portland\n")
    big = ["id,name,note"] + [f"{n},user{n},a longer note for row {n}" for n in range(700)]
    (folder / "big.csv").write_text("\n".join(big) + "\n")
    (folder / "data.json").write_text('{"name":"forge","nested":{"a":[1,2,3]}}\n')
    (folder / "page.html").write_text(HTML)
    (folder / "pic.svg").write_text(SVG)
    (folder / "pic.png").write_bytes(make_png())
    (folder / "doc.pdf").write_bytes(make_pdf(1))
    (folder / "multi.pdf").write_bytes(make_pdf(12))

    shown = ["data.csv", "page.html", "doc.pdf", "pic.png", "pic.svg"]
    # The last path is outside the session folder: a click on it must only show a notice.
    text = "Here are the files:\n\n" + "\n\n".join([*(str(folder / name) for name in shown), "/private/tmp/outside.txt"])
    transcript = Path(claude_home) / "projects" / bf.escape_project_path(cwd) / f"{SESSION_ID}.jsonl"
    line = {"type": "assistant", "sessionId": SESSION_ID, "message": {"role": "assistant", "content": [{"type": "text", "text": text}]}}
    with transcript.open("a") as handle:
        handle.write(json.dumps(line) + "\n")


if __name__ == "__main__":
    run_dir = Path(os.environ.get("FORGE_HARNESS_DIR", "/private/tmp/forge-harness-home"))
    add(run_dir / ".claude", Path(os.path.realpath(bf.DEFAULT_PROJECTS_ROOT)))
    print(f"samples added under {bf.DEFAULT_PROJECTS_ROOT}/my-app/samples", file=sys.stderr)
```

In `scripts/harness/launch.sh` add after the line `python3 "$here/build_fixture.py" "$run_dir" >/dev/null`:

```bash
# An optional script that adds to the fixture (for example add_preview_samples.py).
if [[ -n "${FORGE_HARNESS_EXTRA:-}" ]]; then
  python3 "$FORGE_HARNESS_EXTRA"
fi
```

In `scripts/harness/README.md` add under the first code block's explanation (after the sentence ending `... never read or written.`):

```markdown
`FORGE_HARNESS_EXTRA=scripts/harness/add_preview_samples.py scripts/harness/launch.sh` also writes preview samples (csv, html, svg, png, pdf) into `my-app/samples/` and a reply in the session "Read the config" that names five of them by absolute path.
```

- [ ] **Step 4: Run the harness tests**

Run: `cd scripts/harness && python3 -m unittest discover -s . -v; cd ../..`
Expected: PASS (the new tests and the existing ones).

- [ ] **Step 5: Commit the script**

```bash
git add scripts/harness
git commit -m "test: harness samples for the Files panel previews and links in the chat (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 6: Check each kind in the real window**

Start the app with the samples and look at it (the README of `scripts/harness/` explains the click scale: multiply the coordinates read off the image by the factor the viewer prints):

```bash
FORGE_HARNESS_EXTRA=scripts/harness/add_preview_samples.py scripts/harness/launch.sh
python3 scripts/harness/capture.py
```

Open the session "Read the config" (group `my-app`), then open the Files panel from the header `⋮` menu. Check each line below with a capture, and fix the code and re-run before going on if one fails:

1. Tree: `big.csv`, `data.csv` show a table icon; `page.html` a globe; `pic.png` and `pic.svg` a picture; `data.json` the `</>` icon; `doc.pdf` and `multi.pdf` a plain page.
2. `data.csv` opens as a table with a grey header; the `</>` button shows its source; clicking it again shows the table; edit a cell in the source, toggle back: the table shows the new text.
3. `big.csv` shows 500 rows and the note "Only the first 500 rows are shown. Switch to the source to see the rest."
4. `page.html` shows "script has not run" in the frame; the link in it does nothing; `</>` shows the source.
5. `pic.svg` is drawn on a dark backdrop; `+`, `−` and `⤢` change the size; `</>` shows the source.
6. `doc.pdf` and `multi.pdf` show the PDF viewer on a dark backdrop; `multi.pdf` scrolls to page 12.
7. In the chat, the five paths in the last reply are links. A click on `data.csv` shows the table in the Files panel; `page.html`, `doc.pdf` and `pic.svg` open in the Files panel; `pic.png` opens the lightbox with the path under the picture; `×`, a click and Escape each close it.
8. The sixth path of the reply, `/private/tmp/outside.txt`, is a link too; a click on it shows the notice "Only files inside the session folder can be opened here." and opens nothing.

Leave IME, dark mode and a long PDF read to the end to the user. Then stop the app:

```bash
scripts/harness/stop.sh
```

- [ ] **Step 7: Commit any fixes the check needed**

```bash
git add -A
git commit -m "fix: what the real window check of the file previews found (Plan 28)

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

(Skip this step when the check found nothing to fix.)

---

### Task 10: Roadmap, full test run, and the final review

**Files:**
- Modify: `docs/superpowers/roadmap/README.md` (the plan order table)
- Modify: `docs/superpowers/roadmap/backlog.md` (row `PNL-04`)

- [ ] **Step 1: Record the plan**

In `docs/superpowers/roadmap/README.md`, after the row that starts with `| 26 | Responsive layout (built)`, add:

```markdown
| 28 | File previews (built) | PNL-04 | Spec `2026-10-08-file-previews-design.md`: csv, html, svg and pdf in the Files panel, tree icons, absolute path links, picture lightbox; observation round 12 |
```

In `docs/superpowers/roadmap/backlog.md`, append a sentence to the last cell of the `| PNL-04 |` row:

```bash
python3 - <<'PY'
path = "docs/superpowers/roadmap/backlog.md"
lines = open(path).read().split("\n")
for index, line in enumerate(lines):
    if line.startswith("| PNL-04 |"):
        assert line.rstrip().endswith("|")
        lines[index] = line.rstrip()[:-1].rstrip() + (
            " Plan 28 (built): csv table, rendered html, svg zoom, pdf, tree icons, absolute path links and a picture lightbox;"
            " html, pdf and svg paths clicked in the chat open in the Files panel, not the Browser pane (left until that pane can show pages). |"
        )
open(path, "w").write("\n".join(lines))
PY
git diff --stat docs/superpowers/roadmap/backlog.md
```

Expected: one line changed in `backlog.md`.

- [ ] **Step 2: Run everything**

```bash
cargo test --workspace
(cd scripts/harness && python3 -m unittest discover -s . )
cargo build -p forge-app
git status --short
```

Expected: all tests PASS, build OK, working tree clean apart from the docs edits.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/roadmap
git commit -m "docs: Plan 28, file previews, in the roadmap

Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

- [ ] **Step 4: Review the branch**

Run a review of the whole branch against the spec (`/code-review` at `high`, or the review skill the execution method names), fix what it finds, run `cargo test --workspace` again, and commit the fixes.
