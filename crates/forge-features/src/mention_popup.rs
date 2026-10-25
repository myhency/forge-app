//! The `@` mention list's decisions without any UI: which row is highlighted, what a key does while
//! the list is open, which row a pick attaches, and where the text area's cursor is in the text.

use std::path::{Path, PathBuf};

use forge_domain::Attachment;
use forge_entities::{menu_action, rank_mentions, MenuAction, MenuKey, MentionRange, MentionRow};

use crate::file_lists::FileListState;

/// A file of the session folder by its `/`-separated relative path, joined part by part so the
/// separators are the platform's.
pub fn absolute_in(folder: &Path, relative: &str) -> PathBuf {
    relative.split('/').filter(|part| !part.is_empty()).fold(folder.to_path_buf(), |path, part| path.join(part))
}

/// Whether the file at `relative` in `folder` is attached already. Lexical: no path is resolved,
/// so a file attached from outside the folder never matches a row.
pub fn attached_in(folder: &Path, relative: &str, attachments: &[Attachment]) -> bool {
    let path = absolute_in(folder, relative);
    attachments.iter().any(|attachment| attachment.path == path)
}

/// What a key in the text area does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDecision {
    /// The open mention list takes the key.
    Mention(MenuAction),
    Send,
    /// The text area's own behaviour (a character, a new line with Shift+Enter, an IME composition).
    Pass,
}

/// While the mention list is open Enter never sends: it picks, or with Shift held or during an IME
/// composition it is the text area's. With the list closed Enter (without Shift, not composing) sends.
pub fn key_decision(mention_open: bool, key: MenuKey, composing: bool, shift: bool) -> KeyDecision {
    if mention_open {
        return menu_action(key, composing, shift).map_or(KeyDecision::Pass, KeyDecision::Mention);
    }
    if key == MenuKey::Enter && !composing && !shift {
        KeyDecision::Send
    } else {
        KeyDecision::Pass
    }
}

/// A pick: the `@…` text to remove and the file (relative to the session folder) to attach.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MentionPick {
    pub range: MentionRange,
    pub path: String,
}

/// What an open list did with a key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MentionOutcome {
    Moved,
    Picked(MentionPick),
    Closed,
    /// Taken, with nothing to do (a pick on an attached row, or with no rows).
    Nothing,
}

/// The list's own state: the highlighted row for the query it was moved on, and the query it was
/// closed on with Esc.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MentionPopup {
    moved_on: Option<(usize, String)>,
    highlight: usize,
    closed_on: Option<(usize, String)>,
}

impl MentionPopup {
    /// Open for a query at the cursor, unless Esc closed it on this very query (typing on opens it again).
    pub fn is_open(&self, range: Option<&MentionRange>) -> bool {
        range.is_some_and(|range| !self.closed_on.as_ref().is_some_and(|closed| is_on(closed, range)))
    }

    /// The highlighted row: the first for a query the highlight was not moved on, and never past the end.
    pub fn highlight(&self, range: &MentionRange, rows: usize) -> usize {
        match &self.moved_on {
            Some(moved) if is_on(moved, range) => self.highlight.min(rows.saturating_sub(1)),
            _ => 0,
        }
    }

    /// A key while the list is open. `action` comes from `key_decision`.
    pub fn act(&mut self, range: &MentionRange, rows: &[MentionRow], action: MenuAction) -> MentionOutcome {
        match action {
            MenuAction::Move(delta) => {
                let last = rows.len().saturating_sub(1) as isize;
                let current = self.highlight(range, rows.len()) as isize;
                self.highlight = (current + delta).clamp(0, last) as usize;
                self.moved_on = Some((range.start, range.query.clone()));
                MentionOutcome::Moved
            }
            MenuAction::Pick => {
                let index = self.highlight(range, rows.len());
                self.pick(range, rows, index)
            }
            MenuAction::Close => {
                self.closed_on = Some((range.start, range.query.clone()));
                MentionOutcome::Closed
            }
        }
    }

    /// A click on a row.
    pub fn pick(&mut self, range: &MentionRange, rows: &[MentionRow], index: usize) -> MentionOutcome {
        match rows.get(index) {
            Some(row) if !row.attached => MentionOutcome::Picked(MentionPick { range: range.clone(), path: row.path.clone() }),
            _ => MentionOutcome::Nothing,
        }
    }
}

fn is_on(query: &(usize, String), range: &MentionRange) -> bool {
    query.0 == range.start && query.1 == range.query
}

/// What the list shows besides its rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MentionStatus {
    /// The first walk of the folder is not done yet.
    Loading,
    /// The folder has no files (or could not be walked).
    Empty,
    /// Files, none of which match the query.
    NoMatch,
    /// Rows; `cut` when the walk stopped at its cap, so a trailing row says the list is partial.
    Listed { cut: bool },
}

/// The rows for a query from what the walk gave so far. While a refresh runs the old list is used.
pub fn mention_rows(state: &FileListState, query: &str, attached: &dyn Fn(&str) -> bool) -> (Vec<MentionRow>, MentionStatus) {
    let (files, cut) = match state {
        FileListState::Idle | FileListState::Loading(None) => return (Vec::new(), MentionStatus::Loading),
        FileListState::Loading(Some(files)) => (files, false),
        FileListState::Ready { files, cut } => (files, *cut),
    };
    if files.is_empty() {
        return (Vec::new(), MentionStatus::Empty);
    }
    let rows = rank_mentions(files, query, attached);
    let status = if rows.is_empty() { MentionStatus::NoMatch } else { MentionStatus::Listed { cut } };
    (rows, status)
}

/// The byte offset of a text area's cursor given in UTF-16 units (what `selectionStart` counts);
/// `None` past the end or inside a character.
pub fn utf16_to_byte(text: &str, utf16: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, ch) in text.char_indices() {
        if units == utf16 {
            return Some(byte);
        }
        if units > utf16 {
            return None;
        }
        units += ch.len_utf16();
    }
    (units == utf16).then_some(text.len())
}

/// The UTF-16 offset of a byte offset (for `setSelectionRange`).
pub fn byte_to_utf16(text: &str, byte: usize) -> usize {
    text.char_indices().take_while(|(at, _)| *at < byte).map(|(_, ch)| ch.len_utf16()).sum()
}

/// The text with the `@…` span taken out.
pub fn without_range(text: &str, range: &MentionRange) -> String {
    format!("{}{}", &text[..range.start], &text[range.end..])
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::AttachmentKind;
    use forge_entities::mention_query;

    fn file(path: &str) -> Attachment {
        Attachment { path: path.into(), kind: AttachmentKind::File }
    }

    fn row(path: &str, attached: bool) -> MentionRow {
        let name = path.rsplit('/').next().unwrap().to_string();
        MentionRow { path: path.into(), folder: path[..path.len() - name.len()].into(), name, attached }
    }

    fn range(text: &str) -> MentionRange {
        mention_query(text, text.len()).unwrap()
    }

    #[test]
    fn a_file_attached_from_inside_the_folder_ticks_its_row() {
        let attachments = [file("/w/p/docs/보고서.docx")];
        assert!(attached_in(Path::new("/w/p"), "docs/보고서.docx", &attachments));
        assert!(!attached_in(Path::new("/w/p"), "docs/other.docx", &attachments));
    }

    #[test]
    fn a_file_of_the_same_name_in_another_folder_ticks_nothing() {
        let attachments = [file("/elsewhere/docs/보고서.docx"), file("/w/docs/보고서.docx")];
        assert!(!attached_in(Path::new("/w/p"), "docs/보고서.docx", &attachments));
    }

    #[test]
    fn a_relative_attachment_path_is_not_the_absolute_file() {
        assert!(!attached_in(Path::new("/w/p"), "a.txt", &[file("a.txt")]));
        assert!(attached_in(Path::new("/w/p/"), "a.txt", &[file("/w/p/a.txt")]), "a trailing slash on the folder");
    }

    #[test]
    fn enter_picks_while_the_list_is_open_and_sends_when_it_is_closed() {
        assert_eq!(key_decision(true, MenuKey::Enter, false, false), KeyDecision::Mention(MenuAction::Pick));
        assert_eq!(key_decision(true, MenuKey::Tab, false, false), KeyDecision::Mention(MenuAction::Pick));
        assert_eq!(key_decision(false, MenuKey::Enter, false, false), KeyDecision::Send);
    }

    #[test]
    fn enter_never_sends_while_the_list_is_open() {
        for (composing, shift) in [(true, false), (false, true), (true, true)] {
            assert_eq!(key_decision(true, MenuKey::Enter, composing, shift), KeyDecision::Pass, "{composing} {shift}");
        }
    }

    #[test]
    fn a_composing_or_shifted_enter_with_the_list_closed_does_not_send() {
        assert_eq!(key_decision(false, MenuKey::Enter, true, false), KeyDecision::Pass);
        assert_eq!(key_decision(false, MenuKey::Enter, false, true), KeyDecision::Pass);
        assert_eq!(key_decision(false, MenuKey::Tab, false, false), KeyDecision::Pass);
        assert_eq!(key_decision(false, MenuKey::Other, false, false), KeyDecision::Pass);
    }

    #[test]
    fn arrows_and_escape_belong_to_the_open_list() {
        assert_eq!(key_decision(true, MenuKey::Up, false, false), KeyDecision::Mention(MenuAction::Move(-1)));
        assert_eq!(key_decision(true, MenuKey::Down, false, false), KeyDecision::Mention(MenuAction::Move(1)));
        assert_eq!(key_decision(true, MenuKey::Escape, false, false), KeyDecision::Mention(MenuAction::Close));
        assert_eq!(key_decision(false, MenuKey::Up, false, false), KeyDecision::Pass);
        assert_eq!(key_decision(true, MenuKey::Other, false, false), KeyDecision::Pass);
    }

    #[test]
    fn the_highlight_moves_without_wrapping() {
        let rows = [row("a.txt", false), row("b.txt", false), row("c.txt", false)];
        let at = range("@");
        let mut popup = MentionPopup::default();
        assert_eq!(popup.highlight(&at, rows.len()), 0);
        assert_eq!(popup.act(&at, &rows, MenuAction::Move(-1)), MentionOutcome::Moved);
        assert_eq!(popup.highlight(&at, rows.len()), 0, "no wrap at the top");
        popup.act(&at, &rows, MenuAction::Move(1));
        popup.act(&at, &rows, MenuAction::Move(1));
        popup.act(&at, &rows, MenuAction::Move(1));
        assert_eq!(popup.highlight(&at, rows.len()), 2, "no wrap at the bottom");
    }

    #[test]
    fn a_pick_gives_the_range_and_the_highlighted_path() {
        let rows = [row("a.txt", false), row("docs/b.txt", false)];
        let at = range("see @b");
        let mut popup = MentionPopup::default();
        popup.act(&at, &rows, MenuAction::Move(1));
        assert_eq!(
            popup.act(&at, &rows, MenuAction::Pick),
            MentionOutcome::Picked(MentionPick { range: MentionRange { start: 4, end: 6, query: "b".into() }, path: "docs/b.txt".into() })
        );
        assert_eq!(popup.pick(&at, &rows, 0), MentionOutcome::Picked(MentionPick { range: at.clone(), path: "a.txt".into() }));
    }

    #[test]
    fn a_changed_query_puts_the_highlight_back_on_the_first_row() {
        let rows = [row("a.txt", false), row("b.txt", false)];
        let mut popup = MentionPopup::default();
        popup.act(&range("@"), &rows, MenuAction::Move(1));
        assert_eq!(popup.highlight(&range("@"), 2), 1);
        assert_eq!(popup.highlight(&range("@a"), 2), 0);
    }

    #[test]
    fn the_highlight_never_points_past_a_list_that_shrank() {
        let rows = [row("a.txt", false), row("b.txt", false), row("c.txt", false)];
        let at = range("@");
        let mut popup = MentionPopup::default();
        popup.act(&at, &rows, MenuAction::Move(2));
        assert_eq!(popup.highlight(&at, 1), 0);
        assert_eq!(popup.highlight(&at, 0), 0);
        assert_eq!(popup.act(&at, &rows[..1], MenuAction::Pick), MentionOutcome::Picked(MentionPick { range: at.clone(), path: "a.txt".into() }));
    }

    #[test]
    fn an_attached_row_is_not_picked() {
        let rows = [row("a.txt", true)];
        let at = range("@a");
        let mut popup = MentionPopup::default();
        assert_eq!(popup.act(&at, &rows, MenuAction::Pick), MentionOutcome::Nothing);
        assert_eq!(popup.pick(&at, &rows, 0), MentionOutcome::Nothing);
        assert_eq!(popup.pick(&at, &rows, 5), MentionOutcome::Nothing, "a click past the end");
        assert_eq!(popup.act(&at, &[], MenuAction::Pick), MentionOutcome::Nothing, "Enter while loading");
    }

    #[test]
    fn escape_closes_until_the_query_changes() {
        let rows = [row("a.txt", false)];
        let mut popup = MentionPopup::default();
        assert!(!popup.is_open(None));
        assert!(popup.is_open(Some(&range("@a"))));
        assert_eq!(popup.act(&range("@a"), &rows, MenuAction::Close), MentionOutcome::Closed);
        assert!(!popup.is_open(Some(&range("@a"))));
        assert!(popup.is_open(Some(&range("@ab"))), "typing on opens it again");
        assert!(popup.is_open(Some(&range("x @a"))), "another @");
    }

    #[test]
    fn the_cursor_in_utf16_units_maps_to_bytes() {
        assert_eq!(utf16_to_byte("", 0), Some(0));
        assert_eq!(utf16_to_byte("ab", 2), Some(2));
        assert_eq!(utf16_to_byte("가@보", 2), Some(4));
        assert_eq!(utf16_to_byte("가@보", 3), Some(7));
        assert_eq!(utf16_to_byte("😀a", 2), Some(4));
        assert_eq!(utf16_to_byte("😀a", 1), None, "inside a surrogate pair");
        assert_eq!(utf16_to_byte("ab", 3), None);
        assert_eq!(byte_to_utf16("가@보", 4), 2);
        assert_eq!(byte_to_utf16("😀a", 4), 2);
        assert_eq!(byte_to_utf16("ab", 9), 2, "past the end is the end");
    }

    #[test]
    fn a_pick_takes_the_at_span_out_of_the_text() {
        let text = "read @보고 now";
        let at = mention_query(text, "read @보고".len()).unwrap();
        assert_eq!(without_range(text, &at), "read  now");
        assert_eq!(without_range("@a", &range("@a")), "");
    }

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn the_rows_say_loading_until_the_first_walk_is_done() {
        let none = |_: &str| false;
        assert_eq!(mention_rows(&FileListState::Idle, "", &none), (Vec::new(), MentionStatus::Loading));
        assert_eq!(mention_rows(&FileListState::Loading(None), "", &none), (Vec::new(), MentionStatus::Loading));
    }

    #[test]
    fn a_refresh_lists_the_old_files_meanwhile() {
        let (rows, status) = mention_rows(&FileListState::Loading(Some(files(&["a.txt"]))), "", &|_| false);
        assert_eq!(rows, [row("a.txt", false)]);
        assert_eq!(status, MentionStatus::Listed { cut: false });
    }

    #[test]
    fn an_empty_folder_and_a_query_matching_nothing_say_so() {
        let none = |_: &str| false;
        assert_eq!(mention_rows(&FileListState::Ready { files: Vec::new(), cut: false }, "x", &none).1, MentionStatus::Empty);
        assert_eq!(mention_rows(&FileListState::Ready { files: files(&["a.txt"]), cut: true }, "zz", &none).1, MentionStatus::NoMatch);
    }

    #[test]
    fn a_cut_list_is_marked_and_attached_rows_are_ticked() {
        let state = FileListState::Ready { files: files(&["docs/a.txt", "b.txt"]), cut: true };
        let (rows, status) = mention_rows(&state, "", &|path| path == "b.txt");
        assert_eq!(rows, [row("docs/a.txt", false), row("b.txt", true)]);
        assert_eq!(status, MentionStatus::Listed { cut: true });
    }

    #[test]
    fn a_file_in_a_subfolder_is_joined_part_by_part() {
        assert_eq!(absolute_in(Path::new("/w/p"), "docs/a.txt"), Path::new("/w/p").join("docs").join("a.txt"));
    }
}
