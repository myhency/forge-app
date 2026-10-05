/// Lines of a file, 1 based and inclusive; one line is `start == end`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

impl LineRange {
    pub fn new(a: u32, b: u32) -> Self {
        let (start, end) = if a <= b { (a, b) } else { (b, a) };
        Self { start: start.max(1), end: end.max(1) }
    }

    pub fn len(&self) -> u32 {
        self.end - self.start + 1
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Keeps the range inside a file of `lines` lines (a file with none counts as one).
    pub fn clamp_to(self, lines: u32) -> Self {
        let last = lines.max(1);
        Self { start: self.start.min(last), end: self.end.min(last) }
    }

    pub fn label(&self) -> String {
        if self.start == self.end {
            self.start.to_string()
        } else {
            format!("{}-{}", self.start, self.end)
        }
    }

    fn parse(text: &str) -> Option<Self> {
        let (a, b) = match text.split_once('-') {
            Some((a, b)) => (a.parse::<u32>().ok()?, b.parse::<u32>().ok()?),
            None => {
                let line = text.parse::<u32>().ok()?;
                (line, line)
            }
        };
        (a >= 1 && b >= a).then_some(Self { start: a, end: b })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Excerpt,
    Comment,
}

impl CardKind {
    fn tag(self) -> &'static str {
        match self {
            CardKind::Excerpt => "forge-excerpt",
            CardKind::Comment => "forge-comment",
        }
    }
}

/// A piece of a file the person attached to a message: the lines themselves, or a comment on them.
/// `id` ties a comment to its box in the editor; a card read back from a transcript has id 0.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: u64,
    pub kind: CardKind,
    /// Absolute path.
    pub path: String,
    pub range: LineRange,
    pub text: String,
}

impl Card {
    pub fn badge(&self) -> &'static str {
        match self.kind {
            CardKind::Excerpt => "Excerpt",
            CardKind::Comment => "Comment",
        }
    }

    /// `0.02KB • 1 line`, shown in the excerpt modal.
    pub fn size_label(&self) -> String {
        let lines = self.range.len();
        let noun = if lines == 1 { "line" } else { "lines" };
        format!("{:.2}KB • {lines} {noun}", self.text.len() as f64 / 1024.0)
    }

    fn to_block(&self) -> String {
        let tag = self.kind.tag();
        format!("<{tag} path=\"{}\" lines=\"{}\">{}</{tag}>", escape(&self.path), self.range.label(), escape(&self.text))
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&")
}

/// The text to send: a block per card, a blank line, then what the person typed.
pub fn with_cards(cards: &[Card], text: &str) -> String {
    if cards.is_empty() {
        return text.to_string();
    }
    let mut out = String::new();
    for card in cards {
        out.push_str(&card.to_block());
        out.push('\n');
    }
    out.push('\n');
    out.push_str(text);
    out
}

// One block at the start of `text`: the card and what follows it.
fn parse_block(text: &str) -> Option<(Card, &str)> {
    for kind in [CardKind::Excerpt, CardKind::Comment] {
        let tag = kind.tag();
        let Some(rest) = text.strip_prefix(&format!("<{tag} path=\"")) else { continue };
        let (path, rest) = rest.split_once("\" lines=\"")?;
        let (lines, rest) = rest.split_once("\">")?;
        let (body, rest) = rest.split_once(&format!("</{tag}>"))?;
        let range = LineRange::parse(lines)?;
        return Some((Card { id: 0, kind, path: unescape(path), range, text: unescape(body) }, rest));
    }
    None
}

/// The cards at the start of a sent text and the text after them. Only a run of blocks that ends
/// in a blank line (or the end of the text) counts; anything else is returned whole.
pub fn split_cards(text: &str) -> (Vec<Card>, String) {
    let mut cards = Vec::new();
    let mut rest = text;
    loop {
        let Some((card, after)) = parse_block(rest) else { break };
        cards.push(card);
        rest = after;
        if rest.is_empty() {
            return (cards, String::new());
        }
        let Some(after_newline) = rest.strip_prefix('\n') else { return (vec![], text.to_string()) };
        rest = after_newline;
        if let Some(after_blank) = rest.strip_prefix('\n') {
            return (cards, after_blank.to_string());
        }
        if rest.is_empty() {
            return (cards, String::new());
        }
    }
    (vec![], text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excerpt(path: &str, a: u32, b: u32, text: &str) -> Card {
        Card { id: 0, kind: CardKind::Excerpt, path: path.into(), range: LineRange::new(a, b), text: text.into() }
    }
    fn comment(path: &str, a: u32, text: &str) -> Card {
        Card { id: 0, kind: CardKind::Comment, path: path.into(), range: LineRange::new(a, a), text: text.into() }
    }

    #[test]
    fn a_range_orders_itself_and_never_starts_below_one() {
        assert_eq!(LineRange::new(8, 5), LineRange { start: 5, end: 8 });
        assert_eq!(LineRange::new(0, 0), LineRange { start: 1, end: 1 });
        assert_eq!(LineRange::new(5, 8).len(), 4);
        assert_eq!(LineRange::new(3, 3).label(), "3");
        assert_eq!(LineRange::new(5, 8).label(), "5-8");
    }

    #[test]
    fn clamping_keeps_the_range_inside_the_file() {
        assert_eq!(LineRange::new(9, 40).clamp_to(12), LineRange { start: 9, end: 12 });
        assert_eq!(LineRange::new(20, 40).clamp_to(12), LineRange { start: 12, end: 12 });
        assert_eq!(LineRange::new(2, 3).clamp_to(0), LineRange { start: 1, end: 1 });
    }

    #[test]
    fn no_cards_leaves_the_text_alone() {
        assert_eq!(with_cards(&[], "hello"), "hello");
        assert_eq!(split_cards("hello"), (vec![], "hello".to_string()));
    }

    #[test]
    fn cards_and_text_round_trip() {
        let cards = vec![excerpt("/p/a.py", 5, 8, "x = 1\ny = 2"), comment("/p/a.py", 3, "explain this")];
        let sent = with_cards(&cards, "붙인 내용 보고 ok라고만 답해줘");
        assert_eq!(
            sent,
            "<forge-excerpt path=\"/p/a.py\" lines=\"5-8\">x = 1\ny = 2</forge-excerpt>\n<forge-comment path=\"/p/a.py\" lines=\"3\">explain this</forge-comment>\n\n붙인 내용 보고 ok라고만 답해줘"
        );
        assert_eq!(split_cards(&sent), (cards, "붙인 내용 보고 ok라고만 답해줘".to_string()));
    }

    #[test]
    fn special_characters_and_a_spelled_out_closing_tag_survive() {
        let cards = vec![excerpt("/한글 \"폴더\"/a&b.py", 1, 1, "if a < b && c > \"d\": </forge-excerpt> done")];
        let (back, rest) = split_cards(&with_cards(&cards, "text"));
        assert_eq!(back, cards);
        assert_eq!(rest, "text");
    }

    #[test]
    fn cards_alone_round_trip_with_an_empty_text() {
        let cards = vec![comment("/p/a.py", 2, "why")];
        assert_eq!(split_cards(&with_cards(&cards, "")), (cards.clone(), String::new()));
        // Something trimmed the trailing blank lines on the way back.
        let trimmed = with_cards(&cards, "").trim_end().to_string();
        assert_eq!(split_cards(&trimmed), (cards, String::new()));
    }

    #[test]
    fn typed_text_that_looks_like_a_block_without_the_blank_line_stays_text() {
        let typed = "<forge-comment path=\"/p/a.py\" lines=\"1\">hi</forge-comment>\nmore";
        assert_eq!(split_cards(typed), (vec![], typed.to_string()));
    }

    #[test]
    fn a_broken_block_stays_in_the_text() {
        for broken in [
            "<forge-excerpt path=\"/a\" lines=\"x\">t</forge-excerpt>\n\nrest",
            "<forge-excerpt path=\"/a\" lines=\"8-5\">t</forge-excerpt>\n\nrest",
            "<forge-excerpt path=\"/a\" lines=\"1\">never closed\n\nrest",
            "<forge-other path=\"/a\" lines=\"1\">t</forge-other>\n\nrest",
        ] {
            assert_eq!(split_cards(broken), (vec![], broken.to_string()), "{broken}");
        }
    }

    #[test]
    fn the_size_label_counts_bytes_and_lines() {
        assert_eq!(excerpt("/a", 5, 5, "print(\"Foo\")").size_label(), "0.01KB • 1 line");
        assert_eq!(excerpt("/a", 4, 9, &"x".repeat(2048)).size_label(), "2.00KB • 6 lines");
    }
}
