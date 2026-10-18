//! Rules for comments on diff lines of the Changes panel. A comment is the same card the Files
//! panel makes (`Workspace::add_comment`), so these helpers only decide where one may be made and
//! which saved ones belong under a line.

use std::path::Path;

use forge_domain::{Card, CardKind};
use forge_entities::{DiffLineKind, PatchLine};
use forge_ports::ChangeSource;

/// Whether the new side of the diff is the working tree. In a commit's own diff it is the commit's
/// file, and a line number from it could point at the wrong line of the file Claude reads now.
pub fn commentable(source: &ChangeSource) -> bool {
    !matches!(source, ChangeSource::Commit { .. })
}

/// The line number a comment on this row would carry: the new-side number of an added or a context
/// line. A removed line is not in the working tree.
pub fn comment_line(line: &PatchLine) -> Option<u32> {
    match line.kind {
        DiffLineKind::Removed => None,
        DiffLineKind::Added | DiffLineKind::Context => line.new_no,
    }
}

/// The one number a unified diff row shows: the old number of a removed line, the new number of any
/// other (falling back to the old one).
pub fn line_number(line: &PatchLine) -> Option<u32> {
    match line.kind {
        DiffLineKind::Removed => line.old_no,
        DiffLineKind::Added | DiffLineKind::Context => line.new_no.or(line.old_no),
    }
}

/// The comment cards of the file `path` (absolute) drawn under `line`. A comment made over a range
/// in the Files editor shows once, under its last line.
pub fn comments_on(cards: &[Card], path: &str, line: u32) -> Vec<Card> {
    cards.iter().filter(|card| card.kind == CardKind::Comment && card.path == path && card.range.end == line).cloned().collect()
}

/// The comment cards of the file `path` (absolute), all lines: what one file section needs of the
/// session's cards, so a change to another file's cards does not redraw it.
pub fn comments_of(cards: &[Card], path: &str) -> Vec<Card> {
    cards.iter().filter(|card| card.kind == CardKind::Comment && card.path == path).cloned().collect()
}

/// The text to save: trimmed, or `None` when there is nothing but blanks.
pub fn comment_text(raw: &str) -> Option<String> {
    let text = raw.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// The absolute path of a changed file. Git prints paths relative to the repository root, which may
/// lie above the session folder.
pub fn absolute_path(root: &Path, relative: &str) -> String {
    root.join(relative).to_string_lossy().to_string()
}

/// What a key press in the comment field does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKey {
    Save,
    Cancel,
    Ignore,
}

/// Enter saves, Shift+Enter is a new line (the field handles it), Escape cancels. While an input
/// method is composing, Enter confirms the letters being typed and must not save.
pub fn comment_key(enter: bool, escape: bool, shift: bool, composing: bool) -> CommentKey {
    if composing {
        return CommentKey::Ignore;
    }
    if escape {
        CommentKey::Cancel
    } else if enter && !shift {
        CommentKey::Save
    } else {
        CommentKey::Ignore
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::LineRange;

    fn line(kind: DiffLineKind, old_no: Option<u32>, new_no: Option<u32>) -> PatchLine {
        PatchLine { kind, text: String::new(), old_no, new_no }
    }

    fn comment(id: u64, path: &str, from: u32, to: u32) -> Card {
        Card { id, kind: CardKind::Comment, path: path.to_string(), range: LineRange::new(from, to), text: format!("c{id}") }
    }

    #[test]
    fn line_number_is_old_for_removed_and_new_otherwise() {
        assert_eq!(line_number(&line(DiffLineKind::Removed, Some(7), None)), Some(7));
        assert_eq!(line_number(&line(DiffLineKind::Added, None, Some(9))), Some(9));
        assert_eq!(line_number(&line(DiffLineKind::Context, Some(3), Some(5))), Some(5));
    }

    #[test]
    fn a_commit_view_cannot_be_commented() {
        assert!(commentable(&ChangeSource::Uncommitted));
        assert!(commentable(&ChangeSource::All { base: "main".to_string() }));
        assert!(!commentable(&ChangeSource::Commit { sha: "abc".to_string() }));
    }

    #[test]
    fn an_added_or_context_line_is_commented_by_its_new_number() {
        assert_eq!(comment_line(&line(DiffLineKind::Added, None, Some(7))), Some(7));
        assert_eq!(comment_line(&line(DiffLineKind::Context, Some(5), Some(7))), Some(7));
    }

    #[test]
    fn a_removed_line_has_no_comment_line() {
        assert_eq!(comment_line(&line(DiffLineKind::Removed, Some(5), None)), None);
        // Even a removed line that somehow carries a new number is not commentable.
        assert_eq!(comment_line(&line(DiffLineKind::Removed, Some(5), Some(5))), None);
    }

    #[test]
    fn a_line_with_no_new_number_has_no_comment_line() {
        assert_eq!(comment_line(&line(DiffLineKind::Added, None, None)), None);
    }

    #[test]
    fn comments_on_matches_the_path_and_the_line_only() {
        let cards = vec![
            comment(1, "/r/src/a.rs", 3, 3),
            comment(2, "/r/src/a.rs", 4, 4),
            comment(3, "/r/other/a.rs", 3, 3),
            Card { id: 4, kind: CardKind::Excerpt, path: "/r/src/a.rs".to_string(), range: LineRange::new(3, 3), text: "x".to_string() },
        ];
        let found: Vec<u64> = comments_on(&cards, "/r/src/a.rs", 3).iter().map(|c| c.id).collect();
        assert_eq!(found, vec![1]);
        assert!(comments_on(&cards, "/r/src/a.rs", 9).is_empty());
    }

    #[test]
    fn comments_of_keeps_one_files_comments_only() {
        let cards = vec![
            comment(1, "/r/a.rs", 3, 3),
            comment(2, "/r/a.rs", 9, 9),
            comment(3, "/r/b.rs", 3, 3),
            Card { id: 4, kind: CardKind::Excerpt, path: "/r/a.rs".to_string(), range: LineRange::new(3, 3), text: "x".to_string() },
        ];
        let found: Vec<u64> = comments_of(&cards, "/r/a.rs").iter().map(|c| c.id).collect();
        assert_eq!(found, vec![1, 2]);
        assert!(comments_of(&cards, "/r/none.rs").is_empty());
    }

    #[test]
    fn a_range_comment_shows_under_its_last_line() {
        let cards = vec![comment(1, "/r/a.rs", 3, 6)];
        assert!(comments_on(&cards, "/r/a.rs", 3).is_empty());
        assert_eq!(comments_on(&cards, "/r/a.rs", 6).len(), 1);
    }

    #[test]
    fn a_blank_text_saves_nothing() {
        assert_eq!(comment_text(""), None);
        assert_eq!(comment_text("  \n\t "), None);
        assert_eq!(comment_text("  fix this \n"), Some("fix this".to_string()));
        assert_eq!(comment_text("여기 고쳐주세요"), Some("여기 고쳐주세요".to_string()));
    }

    #[test]
    fn the_absolute_path_is_the_root_plus_the_relative_path() {
        assert_eq!(absolute_path(Path::new("/work/repo"), "src/a.rs"), "/work/repo/src/a.rs");
        assert_eq!(absolute_path(Path::new("/work/repo"), "한글 폴더/a b.rs"), "/work/repo/한글 폴더/a b.rs");
    }

    #[test]
    fn enter_saves_shift_enter_is_a_new_line_and_escape_cancels() {
        assert!(matches!(comment_key(true, false, false, false), CommentKey::Save));
        assert!(matches!(comment_key(true, false, true, false), CommentKey::Ignore));
        assert!(matches!(comment_key(false, true, false, false), CommentKey::Cancel));
        assert!(matches!(comment_key(false, false, false, false), CommentKey::Ignore));
    }

    #[test]
    fn enter_while_composing_is_ignored() {
        assert!(matches!(comment_key(true, false, false, true), CommentKey::Ignore));
        assert!(matches!(comment_key(false, true, false, true), CommentKey::Ignore));
    }
}
