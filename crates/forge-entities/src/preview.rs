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
