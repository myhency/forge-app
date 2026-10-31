use std::path::PathBuf;

use crate::SessionSummary;

/// How many characters of the text are kept on each side of a match.
const SNIPPET_CONTEXT: usize = 40;

/// A quote of the text around the first match: the match is shown in bold, the rest plain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snippet {
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// One session the search found. `snippet` is `None` when only the title matched.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchHit {
    pub project: PathBuf,
    pub session: SessionSummary,
    pub snippet: Option<Snippet>,
}

// One character to one character, so positions in the folded text are positions in the original.
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

// The first case-insensitive occurrence of the trimmed query, as a range of character positions.
fn locate(chars: &[char], query: &str) -> Option<(usize, usize)> {
    let needle: Vec<char> = query.trim().chars().map(fold).collect();
    if needle.is_empty() {
        return None;
    }
    let folded: Vec<char> = chars.iter().copied().map(fold).collect();
    let start = folded.windows(needle.len()).position(|window| window == needle.as_slice())?;
    Some((start, start + needle.len()))
}

fn flatten(chars: &[char]) -> String {
    chars.iter().map(|c| if matches!(c, '\n' | '\r') { ' ' } else { *c }).collect()
}

/// The text before, at and after the first match of `query`, for drawing the match in bold.
pub fn split_at_match(text: &str, query: &str) -> Option<(String, String, String)> {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = locate(&chars, query)?;
    Some((flatten(&chars[..start]), flatten(&chars[start..end]), flatten(&chars[end..])))
}

/// The first match of `query` in `text` with up to 40 characters around it; a cut side gets `…`.
pub fn find_snippet(text: &str, query: &str) -> Option<Snippet> {
    let chars: Vec<char> = text.chars().collect();
    let (start, end) = locate(&chars, query)?;
    let from = start.saturating_sub(SNIPPET_CONTEXT);
    let to = (end + SNIPPET_CONTEXT).min(chars.len());
    let mut before = flatten(&chars[from..start]);
    if from > 0 {
        before.insert(0, '…');
    }
    let mut after = flatten(&chars[end..to]);
    if to < chars.len() {
        after.push('…');
    }
    Some(Snippet { before, matched: flatten(&chars[start..end]), after })
}

pub fn title_matches(title: &str, query: &str) -> bool {
    let chars: Vec<char> = title.chars().collect();
    locate(&chars, query).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_match_is_found_whatever_the_case() {
        let snippet = find_snippet("Please Explain the Borrow checker", "borrow").unwrap();

        assert_eq!(snippet.matched, "Borrow");
        assert_eq!(snippet.before, "Please Explain the ");
        assert_eq!(snippet.after, " checker");
    }

    #[test]
    fn korean_text_is_found_and_never_cut_inside_a_character() {
        let text = "안녕하세요 로컬 LLM 모델을 추천해 주세요";

        let snippet = find_snippet(text, "로컬").unwrap();

        assert_eq!(snippet.before, "안녕하세요 ");
        assert_eq!(snippet.matched, "로컬");
        assert_eq!(snippet.after, " LLM 모델을 추천해 주세요");
    }

    #[test]
    fn a_long_text_is_cut_to_a_window_with_ellipses() {
        let text = format!("{}needle{}", "a".repeat(100), "b".repeat(100));

        let snippet = find_snippet(&text, "needle").unwrap();

        assert_eq!(snippet.before, format!("…{}", "a".repeat(40)));
        assert_eq!(snippet.after, format!("{}…", "b".repeat(40)));
    }

    #[test]
    fn a_match_at_either_end_has_no_ellipsis_on_that_side() {
        let start = find_snippet("needle then more", "needle").unwrap();
        let end = find_snippet("more then needle", "needle").unwrap();

        assert_eq!(start.before, "");
        assert_eq!(end.after, "");
    }

    #[test]
    fn multibyte_characters_at_the_window_edge_are_not_split() {
        let text = format!("{}x{}", "😀".repeat(60), "😀".repeat(60));

        let snippet = find_snippet(&text, "x").unwrap();

        assert_eq!(snippet.before.chars().filter(|c| *c == '😀').count(), 40);
        assert!(snippet.before.starts_with('…'));
    }

    #[test]
    fn newlines_in_a_snippet_become_spaces() {
        let snippet = find_snippet("line one\nneedle\nline three", "needle").unwrap();

        assert_eq!(snippet.before, "line one ");
        assert_eq!(snippet.after, " line three");
    }

    #[test]
    fn no_match_and_an_empty_or_blank_query_give_nothing() {
        assert!(find_snippet("hello", "xyz").is_none());
        assert!(find_snippet("hello", "").is_none());
        assert!(find_snippet("hello", "   ").is_none());
        assert!(!title_matches("hello", ""));
    }

    #[test]
    fn titles_match_in_any_case_and_split_around_the_match() {
        assert!(title_matches("Debug and identify error cause", "ERROR"));
        assert!(!title_matches("Debug", "error"));
        assert_eq!(
            split_at_match("Debug and identify error cause", "error"),
            Some(("Debug and identify ".to_string(), "error".to_string(), " cause".to_string()))
        );
    }
}
