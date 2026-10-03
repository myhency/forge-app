use std::sync::OnceLock;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

/// A line longer than this makes `code_html` skip highlighting the whole text.
pub(crate) const MAX_HIGHLIGHT_LINE: usize = 2000;

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

// Loading the bundled syntaxes takes a moment, so it happens once, on the first code block.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The code as escaped HTML, wrapped in `<span class="hl-…">` elements for the syntax that
/// `language` names (a name or a file extension, any case). `None` when no syntax matches.
pub(crate) fn highlight_code(language: &str, code: &str) -> Option<String> {
    let set = syntaxes();
    let syntax = set.find_syntax_by_token(language)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, set, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        generator.parse_html_for_line_which_includes_newline(line).ok()?;
    }
    Some(generator.finalize())
}

pub(crate) fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}

/// The code as HTML for a `<pre>`: highlighted for the language when one matches, else only
/// escaped.
pub fn code_html(language: &str, code: &str) -> String {
    // Highlighting a minified file (one line of many thousands of characters) takes seconds.
    if code.lines().any(|line| line.len() > MAX_HIGHLIGHT_LINE) {
        return escape_html(code);
    }
    highlight_code(language, code).unwrap_or_else(|| escape_html(code))
}

/// The language token of a file path: its extension, or `""` for a name without one (a dotfile
/// like `.gitignore` has none).
pub fn language_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rfind('.') {
        Some(0) | None => "",
        Some(dot) => &name[dot + 1..],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_language_is_wrapped_in_hl_classes() {
        let html = highlight_code("rust", "fn main() {}\n").unwrap();
        assert!(html.contains("<span class=\"hl-"), "{html}");
        assert!(html.contains("main"));
    }

    #[test]
    fn languages_are_found_by_name_and_by_extension_in_any_case() {
        assert!(highlight_code("python", "x = 1\n").is_some());
        assert!(highlight_code("py", "x = 1\n").is_some());
        assert!(highlight_code("Rust", "let a = 1;\n").is_some());
        assert!(highlight_code("sh", "ls\n").is_some());
    }

    #[test]
    fn an_unknown_language_gives_none() {
        assert_eq!(highlight_code("nosuchlang", "x"), None);
        assert_eq!(highlight_code("", "x"), None);
    }

    #[test]
    fn code_is_escaped_inside_the_spans() {
        let html = highlight_code("rust", "let a = \"<b>&</b>\";\n").unwrap();
        assert!(html.contains("&lt;b&gt;"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
    }

    #[test]
    fn escape_html_escapes_the_five_special_characters() {
        assert_eq!(escape_html("<a href=\"x\">&'</a>"), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;");
    }

    #[test]
    fn code_html_highlights_known_languages_and_escapes_the_rest() {
        assert!(code_html("rs", "fn main() {}\n").contains("<span class=\"hl-"));
        assert_eq!(code_html("", "a < b"), "a &lt; b");
        assert_eq!(code_html("nosuchlang", "<x>"), "&lt;x&gt;");
    }

    #[test]
    fn the_language_is_the_file_extension() {
        assert_eq!(language_of("src/main.rs"), "rs");
        assert_eq!(language_of("a.b.py"), "py");
        assert_eq!(language_of("Makefile"), "");
        assert_eq!(language_of("dir.d/file"), "");
        assert_eq!(language_of(".gitignore"), "");
    }

    #[test]
    fn code_with_a_very_long_line_is_only_escaped() {
        let long = format!("let a = \"{}\";\n", "x".repeat(MAX_HIGHLIGHT_LINE + 1));
        let html = code_html("rs", &long);
        assert!(!html.contains("<span"), "a minified line is not highlighted");
        assert!(html.contains("&quot;"));
        assert!(code_html("rs", "let a = 1;\n").contains("<span"), "short lines still are");
    }
}
