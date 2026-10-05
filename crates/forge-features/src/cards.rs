use std::collections::HashMap;

use forge_application::LiveSessionId;
use forge_domain::{Card, CardKind, LineRange};

/// The cards (excerpts and comments) not sent yet, per live session.
#[derive(Debug, Default)]
pub struct Cards {
    by_session: HashMap<LiveSessionId, Vec<Card>>,
    next_id: u64,
}

impl Cards {
    fn fresh_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn add_excerpt(&mut self, id: LiveSessionId, path: String, range: LineRange, text: String) {
        let list = self.by_session.entry(id).or_default();
        if let Some(existing) = list.iter_mut().find(|c| c.kind == CardKind::Excerpt && c.path == path && c.range == range) {
            existing.text = text;
            return;
        }
        let card_id = self.fresh_id();
        self.by_session.entry(id).or_default().push(Card { id: card_id, kind: CardKind::Excerpt, path, range, text });
    }

    pub fn add_comment(&mut self, id: LiveSessionId, path: String, range: LineRange, text: String) -> u64 {
        let card_id = self.fresh_id();
        self.by_session.entry(id).or_default().push(Card { id: card_id, kind: CardKind::Comment, path, range, text });
        card_id
    }

    pub fn update_comment(&mut self, id: LiveSessionId, card_id: u64, text: String) {
        if let Some(card) = self.by_session.get_mut(&id).and_then(|l| l.iter_mut().find(|c| c.id == card_id)) {
            card.text = text;
        }
    }

    pub fn remove(&mut self, id: LiveSessionId, card_id: u64) {
        if let Some(list) = self.by_session.get_mut(&id) {
            list.retain(|c| c.id != card_id);
        }
    }

    pub fn list(&self, id: LiveSessionId) -> Vec<Card> {
        self.by_session.get(&id).cloned().unwrap_or_default()
    }

    pub fn comments_for(&self, id: LiveSessionId, path: &str) -> Vec<Card> {
        self.list(id).into_iter().filter(|c| c.kind == CardKind::Comment && c.path == path).collect()
    }

    pub fn take(&mut self, id: LiveSessionId) -> Vec<Card> {
        self.by_session.remove(&id).unwrap_or_default()
    }

    pub fn restore(&mut self, id: LiveSessionId, cards: Vec<Card>) {
        for card in cards {
            match card.kind {
                CardKind::Excerpt => self.add_excerpt(id, card.path, card.range, card.text),
                // A message box that mounts again hands the same failed send back: never twice.
                CardKind::Comment => {
                    let there = self.by_session.get(&id).is_some_and(|list| {
                        list.iter().any(|c| c.kind == CardKind::Comment && c.path == card.path && c.range == card.range && c.text == card.text)
                    });
                    if !there {
                        self.add_comment(id, card.path, card.range, card.text);
                    }
                }
            }
        }
    }
}

/// Keeps every range inside its file. `lines_of` is a file's line count, `None` when unknown.
pub fn clamp_cards(cards: &mut [Card], lines_of: impl Fn(&str) -> Option<u32>) {
    for card in cards {
        if let Some(lines) = lines_of(&card.path) {
            card.range = card.range.clamp_to(lines);
        }
    }
}

/// The number of lines of a file on disk, `None` when it cannot be read as text.
pub fn line_count(path: &str) -> Option<u32> {
    std::fs::read_to_string(path).ok().map(|text| text.lines().count().max(1) as u32)
}


#[cfg(test)]
mod tests {
    use super::*;
    use forge_application::LiveSessionId;
    use std::io::Write;

    fn id(n: u64) -> LiveSessionId {
        LiveSessionId(n)
    }

    #[test]
    fn the_same_lines_attached_twice_is_one_card_with_the_newer_text() {
        let mut cards = Cards::default();
        cards.add_excerpt(id(1), "/p/a.py".into(), LineRange::new(5, 5), "old".into());
        cards.add_excerpt(id(1), "/p/a.py".into(), LineRange::new(5, 5), "new".into());
        assert_eq!(cards.list(id(1)).len(), 1);
        assert_eq!(cards.list(id(1))[0].text, "new");
    }

    #[test]
    fn two_comments_on_one_line_are_two_cards_with_their_own_ids() {
        let mut cards = Cards::default();
        let a = cards.add_comment(id(1), "/p/a.py".into(), LineRange::new(3, 3), "a".into());
        let b = cards.add_comment(id(1), "/p/a.py".into(), LineRange::new(3, 3), "b".into());
        assert_ne!(a, b);
        assert_eq!(cards.comments_for(id(1), "/p/a.py").len(), 2);
        assert!(cards.comments_for(id(1), "/p/other.py").is_empty());
    }

    #[test]
    fn removing_and_updating_a_comment_by_id() {
        let mut cards = Cards::default();
        let a = cards.add_comment(id(1), "/p/a.py".into(), LineRange::new(3, 3), "a".into());
        cards.update_comment(id(1), a, "changed".into());
        assert_eq!(cards.list(id(1))[0].text, "changed");
        cards.remove(id(1), a);
        assert!(cards.list(id(1)).is_empty());
    }

    #[test]
    fn sessions_do_not_share_cards() {
        let mut cards = Cards::default();
        cards.add_excerpt(id(1), "/p/a.py".into(), LineRange::new(1, 1), "x".into());
        assert!(cards.list(id(2)).is_empty());
    }

    #[test]
    fn restoring_a_comment_that_is_already_there_does_not_duplicate_it() {
        let mut cards = Cards::default();
        cards.add_comment(id(1), "/p/a.py".into(), LineRange::new(3, 3), "why".into());
        let again = cards.list(id(1));
        cards.restore(id(1), again);
        assert_eq!(cards.list(id(1)).len(), 1);
        // A different comment on the same line is still added.
        cards.restore(id(1), vec![Card { id: 0, kind: CardKind::Comment, path: "/p/a.py".into(), range: LineRange::new(3, 3), text: "other".into() }]);
        assert_eq!(cards.list(id(1)).len(), 2);
    }

    #[test]
    fn take_empties_the_session_and_restore_gives_fresh_ids() {
        let mut cards = Cards::default();
        let a = cards.add_comment(id(1), "/p/a.py".into(), LineRange::new(3, 3), "a".into());
        let taken = cards.take(id(1));
        assert_eq!(taken.len(), 1);
        assert!(cards.list(id(1)).is_empty());
        cards.restore(id(1), vec![Card { id: 0, ..taken[0].clone() }]);
        let back = cards.list(id(1));
        assert_eq!(back.len(), 1);
        assert_ne!(back[0].id, 0);
        assert_ne!(back[0].id, a);
    }

#[test]
fn a_range_past_the_end_of_a_shrunk_file_is_clamped_when_sent() {
    let mut card = Card { id: 1, kind: CardKind::Excerpt, path: "/p/a.py".into(), range: LineRange::new(9, 40), text: "t".into() };
    clamp_cards(std::slice::from_mut(&mut card), |path| (path == "/p/a.py").then_some(12));
    assert_eq!(card.range, LineRange { start: 9, end: 12 });
    // A file whose length is unknown is left alone.
    let mut other = Card { path: "/p/gone.py".into(), ..card.clone() };
    clamp_cards(std::slice::from_mut(&mut other), |_| None);
    assert_eq!(other.range, LineRange { start: 9, end: 12 });
}

    #[test]
    fn the_line_count_of_a_file_or_none() {
        let path = std::env::temp_dir().join(format!("forge-cards-{}.py", std::process::id()));
        std::fs::File::create(&path).unwrap().write_all(b"a\nb\nc\n").unwrap();
        assert_eq!(line_count(path.to_str().unwrap()), Some(3));
        let _ = std::fs::remove_file(&path);
        assert_eq!(line_count("/nonexistent/x.py"), None);
    }
}

