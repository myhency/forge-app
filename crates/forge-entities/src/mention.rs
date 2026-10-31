//! Small pure helpers for the composer's attachment cards and the `@` file mention: the type tag
//! and the shortened name on a card, the `@…` span under the cursor, and the ranked file rows.

/// The `@…` span the cursor is in: byte offsets into the text, and the typed query after the `@`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionRange {
    pub start: usize,
    pub end: usize,
    pub query: String,
}

/// One row of the mention list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionRow {
    pub path: String,
    pub name: String,
    /// The path without the name.
    pub folder: String,
    pub attached: bool,
}

/// The most rows the mention list shows.
const MENTION_ROW_LIMIT: usize = 50;
/// The most characters a type tag has.
const TAG_CHARS: usize = 5;

/// The tag on an attachment card: the extension in capitals, `FILE` when there is none.
pub fn type_tag(path: &str) -> String {
    let name = forge_domain::file_name(path);
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => extension.to_uppercase().chars().take(TAG_CHARS).collect(),
        _ => "FILE".to_string(),
    }
}

/// A name cut in the middle to at most `max_chars` characters, keeping the start and the extension.
pub fn shorten_middle(name: &str, max_chars: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max_chars {
        return name.to_string();
    }
    // The tail is the last character of the stem and the extension (`종.docx`); when the limit is
    // too small for a head plus that tail, the name is cut at the end instead.
    let tail_start = match name.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < name.len() => name[..dot].chars().count() - 1,
        _ => chars.len(),
    };
    let tail = &chars[tail_start..];
    match max_chars.checked_sub(1 + tail.len()) {
        Some(head) if head >= 1 => chars[..head].iter().chain(['…'].iter()).chain(tail).collect(),
        _ => chars.iter().take(max_chars.saturating_sub(1)).chain(['…'].iter().take(max_chars.min(1))).collect(),
    }
}

/// The `@…` span that ends at the cursor, when the `@` starts the text or follows whitespace.
pub fn mention_query(text: &str, cursor: usize) -> Option<MentionRange> {
    let before = text.get(..cursor)?;
    let start = before.rfind(char::is_whitespace).map_or(0, |at| at + before[at..].chars().next().map_or(1, char::len_utf8));
    let word = &before[start..];
    let query = word.strip_prefix('@')?;
    Some(MentionRange { start, end: cursor, query: query.to_string() })
}

/// The files that match the query: rows whose name matches first, then the rest, at most 50.
pub fn rank_mentions(files: &[String], query: &str, attached: &dyn Fn(&str) -> bool) -> Vec<MentionRow> {
    let needle = query.to_lowercase();
    let (mut by_name, mut by_path) = (Vec::new(), Vec::new());
    for path in files {
        let name = forge_domain::file_name(path);
        if name.to_lowercase().contains(&needle) {
            by_name.push(path);
        } else if path.to_lowercase().contains(&needle) {
            by_path.push(path);
        }
    }
    by_name
        .into_iter()
        .chain(by_path)
        .take(MENTION_ROW_LIMIT)
        .map(|path| {
            let name = forge_domain::file_name(path);
            MentionRow { path: path.clone(), name: name.to_string(), folder: path[..path.len() - name.len()].to_string(), attached: attached(path) }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_tag_is_the_extension_in_capitals() {
        assert_eq!(type_tag("a.xlsx"), "XLSX");
        assert_eq!(type_tag("/x/y/A.DocX"), "DOCX");
        assert_eq!(type_tag("a.tar.gz"), "GZ");
        assert_eq!(type_tag("x.verylongext"), "VERYL");
        assert_eq!(type_tag("보고서.한글문서파일"), "한글문서파");
    }

    #[test]
    fn type_tag_is_file_without_an_extension() {
        assert_eq!(type_tag("noext"), "FILE");
        assert_eq!(type_tag(".env"), "FILE");
        assert_eq!(type_tag("a."), "FILE");
        assert_eq!(type_tag(""), "FILE");
        assert_eq!(type_tag("dir.d/noext"), "FILE");
    }

    #[test]
    fn shorten_middle_keeps_short_names() {
        assert_eq!(shorten_middle("a.txt", 16), "a.txt");
        assert_eq!(shorten_middle("", 0), "");
        assert_eq!(shorten_middle("abcd", 4), "abcd");
    }

    #[test]
    fn shorten_middle_keeps_the_start_and_the_extension() {
        let out = shorten_middle("분기_영업_실적_보고서_최종.docx", 16);
        assert_eq!(out.chars().count(), 16);
        assert!(out.starts_with("분기_영업"));
        assert!(out.ends_with("종.docx"));
        assert!(out.contains('…'));
        assert_eq!(shorten_middle("abcdefghijklmnop.xlsx", 12), "abcde…p.xlsx");
    }

    #[test]
    fn shorten_middle_never_panics_on_tiny_limits() {
        for name in ["", "a", "한글한글.docx", "abcdef.docx", ".env", "abcdef"] {
            for max in 0..=12 {
                let out = shorten_middle(name, max);
                assert!(out.chars().count() <= max, "{name} {max} {out}");
                if name.chars().count() <= max {
                    assert_eq!(out, name);
                }
            }
        }
        assert_eq!(shorten_middle("abcdef.docx", 0), "");
        assert_eq!(shorten_middle("abcdef.docx", 1), "…");
        assert_eq!(shorten_middle("abcdef.docx", 2), "a…");
        assert_eq!(shorten_middle("abcdef.docx", 3), "ab…");
    }

    #[test]
    fn mention_query_finds_the_span_before_the_cursor() {
        let at = |text: &str, cursor: usize| mention_query(text, cursor);
        assert_eq!(at("@", 1), Some(MentionRange { start: 0, end: 1, query: String::new() }));
        assert_eq!(at("@보", 4), Some(MentionRange { start: 0, end: 4, query: "보".into() }));
        assert_eq!(at("hello @보고", 13), Some(MentionRange { start: 6, end: 13, query: "보고".into() }));
        assert_eq!(at("a\n@x", 4), Some(MentionRange { start: 2, end: 4, query: "x".into() }));
    }

    #[test]
    fn mention_query_stops_at_the_cursor() {
        assert_eq!(mention_query("@abc def", 3), Some(MentionRange { start: 0, end: 3, query: "ab".into() }));
    }

    #[test]
    fn mention_query_rejects_what_is_not_a_mention() {
        assert_eq!(mention_query("a@b.com", 7), None);
        assert_eq!(mention_query("foo@bar", 7), None);
        assert_eq!(mention_query("@a b", 4), None);
        assert_eq!(mention_query("hello", 5), None);
        assert_eq!(mention_query("", 0), None);
        assert_eq!(mention_query("@a ", 3), None);
    }

    #[test]
    fn mention_query_needs_a_cursor_on_a_boundary() {
        assert_eq!(mention_query("@보", 2), None);
        assert_eq!(mention_query("@보", 3), None);
        assert_eq!(mention_query("@보", 99), None);
        assert_eq!(mention_query("", 5), None);
    }

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn rank_mentions_puts_name_matches_first_in_input_order() {
        let list = files(&["src/report/a.rs", "docs/Report.md", "report_x/b.rs", "z/report2.txt"]);
        let rows = rank_mentions(&list, "REPORT", &|_| false);
        let paths: Vec<&str> = rows.iter().map(|row| row.path.as_str()).collect();
        assert_eq!(paths, ["docs/Report.md", "z/report2.txt", "src/report/a.rs", "report_x/b.rs"]);
        assert_eq!(rows[0].name, "Report.md");
        assert_eq!(rows[0].folder, "docs/");
        assert_eq!(rows[3].folder, "report_x/");
    }

    #[test]
    fn rank_mentions_matches_korean_and_marks_attached() {
        let list = files(&["문서/보고서.docx", "문서/메모.md", "other.txt"]);
        let rows = rank_mentions(&list, "보고", &|path| path == "문서/보고서.docx");
        assert_eq!(rows.len(), 1);
        assert!(rows[0].attached);
        assert_eq!(rows[0].name, "보고서.docx");
        let rows = rank_mentions(&list, "문서", &|_| false);
        assert_eq!(rows.len(), 2);
        assert!(!rows[0].attached);
    }

    #[test]
    fn rank_mentions_lists_the_first_fifty_for_an_empty_query() {
        let list: Vec<String> = (0..80).map(|n| format!("d/f{n}.txt")).collect();
        let rows = rank_mentions(&list, "", &|_| false);
        assert_eq!(rows.len(), 50);
        assert_eq!(rows[0].path, "d/f0.txt");
        assert_eq!(rows[49].path, "d/f49.txt");
        assert_eq!(rank_mentions(&list, "txt", &|_| false).len(), 50);
        assert!(rank_mentions(&[], "", &|_| false).is_empty());
    }

    #[test]
    fn rank_mentions_handles_windows_separators() {
        let rows = rank_mentions(&files(&["a\\b\\c.rs"]), "c", &|_| false);
        assert_eq!(rows[0].name, "c.rs");
        assert_eq!(rows[0].folder, "a\\b\\");
    }
}
