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
    /// A spreadsheet (`.xlsx`), drawn as a table.
    Sheet,
    /// A Word document (`.docx`), drawn as paragraphs.
    Document,
    /// A PowerPoint deck (`.pptx`), drawn as one card per slide.
    Slides,
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
    let name = forge_domain::file_name(path);
    let (stem, extension) = name.rsplit_once('.')?;
    (!stem.is_empty() && !extension.is_empty()).then(|| extension.to_ascii_lowercase())
}

/// The kind a file is shown as, by its name.
pub fn preview_kind(path: &str) -> PreviewKind {
    match extension_of(path).as_deref() {
        Some("csv") => PreviewKind::Csv,
        Some("html" | "htm") => PreviewKind::Html,
        Some("svg") => PreviewKind::Svg,
        Some("xlsx") => PreviewKind::Sheet,
        Some("docx") => PreviewKind::Document,
        Some("pptx") => PreviewKind::Slides,
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
        PreviewKind::Csv | PreviewKind::Sheet => FileIcon::Table,
        PreviewKind::Html => FileIcon::Globe,
        PreviewKind::Svg | PreviewKind::Image => FileIcon::Picture,
        PreviewKind::Markdown | PreviewKind::Document | PreviewKind::Slides => FileIcon::Doc,
        PreviewKind::Pdf => FileIcon::File,
        PreviewKind::Text => match extension_of(path) {
            Some(extension) if CODE_EXTENSIONS.contains(&extension.as_str()) => FileIcon::Code,
            _ => FileIcon::File,
        },
    }
}

/// Whether a page may be loaded in a Preview web view: `http`, `https` or `file`, with something
/// after the scheme and no whitespace.
pub fn page_address(url: &str) -> bool {
    if url.chars().any(char::is_whitespace) {
        return false;
    }
    let Some((scheme, rest)) = url.split_once("://") else { return false };
    let allowed = ["http", "https", "file"].iter().any(|name| scheme.eq_ignore_ascii_case(name));
    allowed && !rest.is_empty()
}

/// Whether an address is a web page (`http` or `https`), the only kind a page may ask for.
pub fn web_address(url: &str) -> bool {
    let Some((scheme, _)) = url.split_once("://") else { return false };
    (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) && page_address(url)
}

// What a path keeps as it is in a `file://` address; everything else (a space, `#`, `?`, `%`, any
// non-ASCII byte) is written as `%XX`, the way a page reports its own address.
fn kept_in_file_address(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"-._~/:!$&'()*+,;=@".contains(&byte)
}

/// The `file://` address of an absolute path. Characters that would end or change the path, and
/// every non-ASCII byte, are percent-encoded; a Windows drive path gets forward slashes.
pub fn file_address(absolute_path: &str) -> String {
    let slashed = absolute_path.replace('\\', "/");
    let mut out = String::from("file://");
    if !slashed.starts_with('/') {
        out.push('/');
    }
    for byte in slashed.bytes() {
        if kept_in_file_address(byte) {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The path a `file://` address names (the reverse of `file_address`); `None` for another scheme
/// or an address with no path. A Windows drive path loses the slash before the drive letter. A
/// `%` that is not followed by two hex digits stays as it is.
pub fn path_of_file_address(url: &str) -> Option<String> {
    let (scheme, rest) = url.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("file") || rest.is_empty() {
        return None;
    }
    let bytes = rest.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = |index: usize| bytes.get(index).and_then(|&digit| (digit as char).to_digit(16));
        match (bytes[at], hex(at + 1), hex(at + 2)) {
            (b'%', Some(high), Some(low)) => {
                decoded.push((high * 16 + low) as u8);
                at += 3;
            }
            (byte, _, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    let mut path = String::from_utf8_lossy(&decoded).into_owned();
    let bytes = path.as_bytes();
    if bytes.len() >= 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b':' {
        path.remove(0);
    }
    Some(path)
}

/// Where a click on a path in the chat goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenTarget {
    Preview,
    Files,
    Lightbox,
}

pub fn open_target(path: &str) -> OpenTarget {
    match preview_kind(path) {
        PreviewKind::Html | PreviewKind::Pdf | PreviewKind::Svg => OpenTarget::Preview,
        PreviewKind::Image => OpenTarget::Lightbox,
        _ => OpenTarget::Files,
    }
}

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

    #[test]
    fn only_web_and_file_addresses_are_pages() {
        for ok in ["http://localhost:3000", "HTTPS://example.com/a", "file:///tmp/a.html"] {
            assert!(page_address(ok), "{ok}");
        }
        for bad in ["javascript:alert(1)", "data:text/html,hi", "ftp://x", "http://", "file://", "http://a b", "about:blank", ""] {
            assert!(!page_address(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_file_address_encodes_what_would_end_the_path() {
        assert_eq!(file_address("/tmp/a b/c#1?.html"), "file:///tmp/a%20b/c%231%3F.html");
        assert_eq!(file_address("/tmp/100%.html"), "file:///tmp/100%25.html");
        assert_eq!(file_address(r"C:\work\a b.html"), "file:///C:/work/a%20b.html");
    }

    #[test]
    fn a_file_address_gives_its_path_back() {
        for path in ["/tmp/a b/c#1?.html", "/tmp/100%.html", "/tmp/plain.pdf"] {
            assert_eq!(path_of_file_address(&file_address(path)).as_deref(), Some(path), "{path}");
        }
        assert_eq!(path_of_file_address("file:///C:/work/a%20b.html").as_deref(), Some("C:/work/a b.html"));
        assert_eq!(path_of_file_address("http://localhost"), None);
        assert_eq!(path_of_file_address("file://"), None);
    }

    #[test]
    fn a_web_address_is_http_or_https_only() {
        for ok in ["http://localhost:3000", "HTTPS://example.com"] {
            assert!(web_address(ok), "{ok}");
        }
        for bad in ["file:///tmp/a.html", "javascript:alert(1)", "data:text/html,hi", "ftp://x", "http://", ""] {
            assert!(!web_address(bad), "{bad:?}");
        }
    }

    #[test]
    fn a_file_address_encodes_non_ascii_names_the_way_a_page_reports_them_and_decodes_them_back() {
        let path = "/tmp/한글/보고서 1.html";
        assert_eq!(file_address(path), "file:///tmp/%ED%95%9C%EA%B8%80/%EB%B3%B4%EA%B3%A0%EC%84%9C%201.html");
        assert_eq!(path_of_file_address(&file_address(path)).as_deref(), Some(path));
        assert_eq!(path_of_file_address("file:///tmp/%ed%95%9c.html").as_deref(), Some("/tmp/한.html"), "lower case hex too");
        assert_eq!(path_of_file_address("file:///tmp/%ZZ.html").as_deref(), Some("/tmp/%ZZ.html"), "a bad escape stays as it is");
    }

    #[test]
    fn html_pdf_and_svg_open_in_preview_pictures_in_the_lightbox_the_rest_in_files() {
        for (path, target) in [
            ("a.html", OpenTarget::Preview),
            ("A.HTM", OpenTarget::Preview),
            ("doc.pdf", OpenTarget::Preview),
            ("pic.svg", OpenTarget::Preview),
            ("pic.png", OpenTarget::Lightbox),
            ("data.csv", OpenTarget::Files),
            ("README.md", OpenTarget::Files),
            ("main.rs", OpenTarget::Files),
            ("Makefile", OpenTarget::Files),
        ] {
            assert_eq!(open_target(path), target, "{path}");
        }
    }

    #[test]
    fn xlsx_is_a_sheet_in_any_case() {
        assert_eq!(preview_kind("/p/a.xlsx"), PreviewKind::Sheet);
        assert_eq!(preview_kind("/p/A.XLSX"), PreviewKind::Sheet);
        assert!(!has_rendered_view("/p/a.xlsx"));
        assert_eq!(open_target("/p/a.xlsx"), OpenTarget::Files);
    }

    #[test]
    fn docx_is_a_document_and_pptx_are_slides_in_any_case() {
        assert_eq!(preview_kind("/p/a.docx"), PreviewKind::Document);
        assert_eq!(preview_kind("/p/A.DOCX"), PreviewKind::Document);
        assert_eq!(preview_kind("/p/a.pptx"), PreviewKind::Slides);
        assert_eq!(preview_kind("/p/A.PPTX"), PreviewKind::Slides);
        assert!(!has_rendered_view("/p/a.docx"));
        assert!(!has_rendered_view("/p/a.pptx"));
    }

    #[test]
    fn documents_and_slides_have_the_doc_icon() {
        assert_eq!(icon_of("/p/a.docx"), FileIcon::Doc);
        assert_eq!(icon_of("/p/a.pptx"), FileIcon::Doc);
    }

    #[test]
    fn open_target_of_docx_and_pptx_is_files() {
        assert_eq!(open_target("/p/a.docx"), OpenTarget::Files);
        assert_eq!(open_target("/p/a.pptx"), OpenTarget::Files);
    }

    #[test]
    fn a_sheet_has_the_table_icon() {
        assert_eq!(icon_of("/p/a.xlsx"), FileIcon::Table);
    }
}
