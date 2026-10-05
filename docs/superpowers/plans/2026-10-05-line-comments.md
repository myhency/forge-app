# Line excerpts, comments and attach (Plan 25B) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Pick lines in the Files panel editor, attach them to the next message as an excerpt or write a comment on them, show both as cards in the composer and in the chat, and keep the cards when a session is reopened. Also attach a whole file from a tree row or a tab.

**Architecture:** One card type (`Card`) lives in `forge-domain` with a text format (`<forge-excerpt …>` / `<forge-comment …>` blocks before the typed text) and a parser. The send path writes the blocks into the message text; the transcript reader and the live view parse them back into `Message.cards`. Unsent cards live in `Workspace` per live session (like `file_requests`), so the panel (editor page) and the message box (composer) share them. The CodeMirror bundle owns selection, the bar, comment boxes and the right click menu, and talks to Rust with the existing page/Rust message bridge.

**Tech Stack:** Rust (Dioxus 0.7.10), CodeMirror 6 bundle (`tools/editor-bundle`, esbuild, jsdom tests).

**Spec:** `docs/superpowers/specs/2026-10-05-line-comments-design.md`

## Global Constraints

- Branch `plan-25B` in `/Users/james/orca/workspaces/forge/plan-25B`; base `master`. Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- Card text format, exactly: `<forge-excerpt path="P" lines="5-8">TEXT</forge-excerpt>` and `<forge-comment path="P" lines="3">TEXT</forge-comment>`; `&`, `<`, `>`, `"` escaped as `&amp; &lt; &gt; &quot;`; one `\n` after each block and one blank line after the last block, then the typed text. A single line is `lines="3"`.
- Card size 200x200 px, badges `Excerpt` / `Comment`, `×` on hover. Strings are English like the rest of Forge's menus ("Keep open", "Close others"): `Attach as context`, `Add comment`, `Line 3`, `Lines 4-9`, modal title `<absolute path>:L5` or `:L5-L8`, size line `0.02KB • 1 line • May not match the original formatting`. (The spec quotes the real app's Korean; Forge's existing UI is English, so this plan follows Forge.)
- Nothing a person typed or a card held is ever dropped: a block that does not parse stays in the text.
- Do not touch the sidecar. Run `cargo test` for each crate you change and `cd tools/editor-bundle && npm run build && npm test` after a bundle change (the built `crates/forge-shared/assets/editor.js` is committed).
- Out of scope: `Go to line` in the editor menu, the 25C items, following a comment when lines move, an inline chip inside the text (file attach uses the existing chip row above the text).

## Review Focus

- Typed text that itself starts with a `<forge-comment …>` line and no blank line after it must stay text (Task 1 test).
- Text with `<`, `&`, `"`, newlines, Hangul and the closing tag spelled inside it must round trip (Task 1 test).
- A send that fails must give the cards back, not show raw tags in the draft (Task 3 test of `restore` plus parsing `unsent`).
- A range whose end is past the file end (the file shrank) is clamped when sent, never sent as `L9-L40` of a 12 line file (Task 3 test).
- The same line attached twice gives one card; comments on the same line are separate cards (Task 3 test).
- A transcript user line with only cards and no text gives a message with cards and an empty bubble (Task 2 test).
- The sidebar title or any preview that uses the first user text must not show raw tags (Task 2 step).

---

### Task 1: Card type, text format and parser (forge-domain)

**Files:**
- Create: `crates/forge-domain/src/card.rs`
- Modify: `crates/forge-domain/src/lib.rs`, `crates/forge-domain/src/session.rs` (add `cards` to `Message`)

**Interfaces:**
- Produces:
  - `LineRange { start: u32, end: u32 }` with `new(a, b)` (orders, min 1), `len()`, `clamp_to(lines: u32)`, `label() -> String` (`"5"`/`"5-8"`).
  - `CardKind { Excerpt, Comment }`.
  - `Card { id: u64, kind: CardKind, path: String, range: LineRange, text: String }` with `size_label() -> String` and `badge() -> &'static str`.
  - `with_cards(cards: &[Card], text: &str) -> String` and `split_cards(text: &str) -> (Vec<Card>, String)` (cards from a parse have `id: 0`).
  - `Message.cards: Vec<Card>` (default empty).

- [ ] **Step 1: Write the failing tests** — create `crates/forge-domain/src/card.rs` containing only the tests module below (plus `use super::*;`), and add `mod card;` and `pub use card::{split_cards, with_cards, Card, CardKind, LineRange};` to `lib.rs`.

```rust
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
```

- [ ] **Step 2: Run, expect a compile failure** — `cargo test -p forge-domain card` → errors "cannot find type `Card`".

- [ ] **Step 3: Implement** — put this above the tests in `card.rs`:

```rust
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
```

Add to `Message` in `session.rs`: field `pub cards: Vec<crate::Card>,` (doc: "Excerpts and comments the person attached (user messages only)."), and `cards: Vec::new()` in `Message::new`.

- [ ] **Step 4: Run** — `cargo test -p forge-domain` → all pass; then `cargo build` (any other `Message { .. }` literal fails to compile; fix by using `Message::new`).

- [ ] **Step 5: Commit** — `git add -A && git commit -m "feat: Card type with a text format that reads back (Plan 25B)"` (with the trailer).

---

### Task 2: Read cards back from a transcript and the live view

**Files:**
- Modify: `crates/forge-adapter-claude-fs/src/sessions.rs` (`push_user_message`, around line 277; tests near line 766), the application's `push_user_message(session, text, attachments)` used by `SendMessageUseCase` (`grep -rn "fn push_user_message" crates/forge-application`), and any first-user-text title fallback (`grep -n "first user\|first_prompt\|Role::User\|MessageRole::User" crates/forge-adapter-claude-fs/src/sessions.rs crates/forge-entities/src/sidebar_item.rs`).

**Interfaces:**
- Consumes: `split_cards(&str) -> (Vec<Card>, String)`.
- Produces: user `Message`s whose `cards` hold the parsed blocks and whose text blocks hold only the typed text.

- [ ] **Step 1: Failing test** in `sessions.rs` tests (use the existing `user_text` helper and the existing test that loads a session from a written transcript, copy its shape):

```rust
#[test]
fn a_user_line_with_card_blocks_becomes_cards_and_the_typed_text() {
    let sent = forge_domain::with_cards(
        &[forge_domain::Card {
            id: 0,
            kind: forge_domain::CardKind::Comment,
            path: "/p/a.py".into(),
            range: forge_domain::LineRange::new(3, 3),
            text: "이 줄 설명해줘".into(),
        }],
        "ok라고만 답해줘",
    );
    let mut session = empty_session(); // the helper the neighbouring tests use to build a Session
    apply_user_line(&mut session, &user_text(&sent));
    let message = &session.messages[0];
    assert_eq!(message.text(), "ok라고만 답해줘");
    assert_eq!(message.cards.len(), 1);
    assert_eq!(message.cards[0].text, "이 줄 설명해줘");
}

#[test]
fn a_user_line_of_cards_only_keeps_the_cards_and_an_empty_text() {
    let sent = forge_domain::with_cards(
        &[forge_domain::Card { id: 0, kind: forge_domain::CardKind::Excerpt, path: "/p/a.py".into(), range: forge_domain::LineRange::new(5, 5), text: "x".into() }],
        "",
    );
    let mut session = empty_session();
    apply_user_line(&mut session, &user_text(&sent));
    assert_eq!(session.messages.len(), 1);
    assert_eq!(session.messages[0].text(), "");
    assert_eq!(session.messages[0].cards.len(), 1);
}
```

If no `empty_session()` helper exists, build the `Session` literal the way the nearest existing test in that module does and name the local helper `empty_session`.

- [ ] **Step 2: Run** — `cargo test -p forge-adapter-claude-fs card` → FAIL (text still holds the tags).

- [ ] **Step 3: Implement**:

```rust
fn push_user_message(session: &mut Session, text: String) {
    let (cards, rest) = forge_domain::split_cards(&text);
    let mut message = Message::user(rest);
    message.cards = cards;
    session.messages.push(message);
}
```

In the application's `push_user_message(session, text, attachments)` do the same split and set `message.cards`. Add a test beside its existing ones: sending `with_cards(...)` text through `SendMessageUseCase` leaves a message whose `text()` is only the typed part and whose `cards` has the card.

- [ ] **Step 4: Title and preview** — run the grep above. For every place that takes the first user message text for a title or snippet, take `message.text()` (already stripped after Step 3) instead of re-reading raw transcript text; if a place reads the raw jsonl text (a summary pass in `sessions.rs` that does not go through `apply_user_line`), pass that text through `split_cards(..).1`. Add a test for each such place changed.

- [ ] **Step 5: Run** — `cargo test -p forge-adapter-claude-fs -p forge-application` → pass. **Commit** `feat: cards are read back from sent text`.

---

### Task 3: The unsent card store and the send path

**Files:**
- Create: `crates/forge-features/src/cards.rs`
- Modify: `crates/forge-features/src/lib.rs`, `crates/forge-features/src/workspace.rs` (field `cards`, `queued_files`, `comment_focus` + wrappers, next to `file_requests`), `crates/forge-features/src/composer_state.rs` (`Outgoing`, `plan_send`, `take_for_send`, `add_paths`), `crates/forge-features/src/send_message.rs`

**Interfaces:**
- Produces, pure (`cards.rs`): `Cards` (`Default`) with
  `add_excerpt(&mut self, id: LiveSessionId, path: String, range: LineRange, text: String)`,
  `add_comment(&mut self, id, path, range, text) -> u64`,
  `update_comment(&mut self, id, card_id: u64, text: String)`,
  `remove(&mut self, id, card_id: u64)`,
  `list(&self, id) -> Vec<Card>`,
  `comments_for(&self, id, path: &str) -> Vec<Card>`,
  `take(&mut self, id) -> Vec<Card>`,
  `restore(&mut self, id, cards: Vec<Card>)` (fresh ids).
- Produces on `Workspace`: `cards(id) -> Vec<Card>`, `comments_for(id, path: &str) -> Vec<Card>`, `add_excerpt(id, path, range, text)`, `add_comment(id, path, range, text) -> u64`, `update_comment(id, card_id, text)`, `remove_card(id, card_id)`, `take_cards(id, line_counts: impl Fn(&str) -> Option<u32>) -> Vec<Card>`, `restore_cards(id, Vec<Card>)`, `request_comment_focus(id, card_id)`, `comment_focus(id) -> Option<u64>`, `clear_comment_focus(id)`, `queue_file(id, PathBuf)`, `take_queued_files(id) -> Vec<PathBuf>`.
- Produces: `plan_send(text, attachments, unannounced, cards: &[Card]) -> Option<Outgoing>` (text is `with_cards(cards, with_folder_hints(..))`), `ComposerState::add_paths(Vec<PathBuf>)`.

- [ ] **Step 1: Failing tests** in `cards.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use forge_application::LiveSessionId;

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
}
```

and in `composer_state.rs` tests:

```rust
#[test]
fn cards_alone_can_be_sent_and_come_before_the_text() {
    let card = Card { id: 1, kind: CardKind::Comment, path: "/p/a.py".into(), range: LineRange::new(3, 3), text: "why".into() };
    let out = plan_send("", &[], &[], &[card.clone()]).unwrap();
    assert_eq!(split_cards(&out.text), (vec![Card { id: 0, ..card.clone() }], String::new()));
    let out = plan_send("  hi ", &[], &[], &[card]).unwrap();
    assert!(out.text.ends_with("\n\nhi"));
}
```

(update the four existing `plan_send` tests to pass `&[]` as the fourth argument).

And a workspace-free test for the clamping done in `take_cards` — put the clamp in a pure function in `cards.rs`:

```rust
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
```

- [ ] **Step 2: Run** — `cargo test -p forge-features cards composer` → FAIL (missing items).

- [ ] **Step 3: Implement `cards.rs`**:

```rust
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
                CardKind::Comment => {
                    self.add_comment(id, card.path, card.range, card.text);
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
```

(Check `LiveSessionId` derives `Hash + Eq + Copy`; it is already the key of `file_requests`, so it does.) Export `Cards`, `clamp_cards` from `lib.rs`.

`Workspace` (`workspace.rs`): add fields `cards: Signal<Cards>`, `queued_files: Signal<HashMap<LiveSessionId, Vec<PathBuf>>>`, `comment_focus: Signal<HashMap<LiveSessionId, u64>>`, initialised with `Signal::new(...)` in `use_workspace_provider`. Wrappers follow the `file_requests` style at line ~500; mutating ones do `let mut cards = self.cards; cards.write().add_excerpt(...)`, readers do `self.cards.read().list(id)`. `take_cards(id, lines_of)` takes with `cards.write().take(id)` then `clamp_cards(&mut taken, lines_of)`. `take_queued_files` removes and returns the id's list.

`composer_state.rs`: `plan_send` gains `cards: &[Card]`; "nothing to send" now also requires `cards.is_empty()`; the text is `with_cards(cards, &with_folder_hints(text, unannounced))`. Add `pub fn add_paths(&self, paths: Vec<PathBuf>)` doing `add_attachments(&mut list.write(), paths)`. `take_for_send(&self, text, cards: &[Card])` and `take_for_new_session` pass `&[]`. Callers pass cards.

`send_message.rs` (`SendMessageBox`):
- `submit`: `let cards = workspace.take_cards(id, |path| line_count(path));` before `composer.take_for_send(&draft.read(), &cards)`; if `take_for_send` returns `None` (refused), `workspace.restore_cards(id, cards)` — nothing lost. `line_count(path)` reads the file (`std::fs::read_to_string(path).ok().map(|t| t.lines().count().max(1) as u32)`) — put it in `cards.rs` as `pub fn line_count(path: &str) -> Option<u32>` with a test using a temp file.
- The failed send effect: where it restores `unsent` text, first `let (cards, rest) = split_cards(&text);` restore `cards` with `workspace.restore_cards(id, cards)` and put `rest` in the draft.
- A new effect drains `workspace.take_queued_files(id)` into `composer.add_paths(..)` (re-run when the queue signal changes; write it like the file request effect in `files_panel.rs`).

- [ ] **Step 4: Run** — `cargo test -p forge-features` → pass; `cargo build` compiles.

- [ ] **Step 5: Commit** — `feat: unsent cards per session and the send path (Plan 25B)`.

---

### Task 4: Cards in the composer, the excerpt modal and the chat

**Files:**
- Create: `crates/forge-entities/src/card_view.rs`, `crates/forge-features/src/composer_cards.rs`
- Modify: `crates/forge-entities/src/lib.rs`, `crates/forge-entities/src/turn.rs` (`TurnItem::User`, line ~52), `crates/forge-features/src/send_message.rs`, `crates/forge-features/src/lib.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: `Card`, `Workspace::cards/remove_card/request_comment_focus`.
- Produces: `CardView { card: Card, on_click: Option<EventHandler<()>>, on_remove: Option<EventHandler<()>> }`; `ComposerCards { id: LiveSessionId }`.

- [ ] **Step 1: Test the pure parts** — add to `card_view.rs` a function and its tests:

```rust
/// The title of the excerpt modal: `/abs/a.py:L5` or `/abs/a.py:L5-L8`.
pub fn modal_title(card: &Card) -> String {
    if card.range.start == card.range.end {
        format!("{}:L{}", card.path, card.range.start)
    } else {
        format!("{}:L{}-L{}", card.path, card.range.start, card.range.end)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{CardKind, LineRange};

    fn card(a: u32, b: u32) -> Card {
        Card { id: 1, kind: CardKind::Excerpt, path: "/p/fizz.py".into(), range: LineRange::new(a, b), text: "x".into() }
    }

    #[test]
    fn the_modal_title_names_the_lines() {
        assert_eq!(modal_title(&card(5, 5)), "/p/fizz.py:L5");
        assert_eq!(modal_title(&card(5, 8)), "/p/fizz.py:L5-L8");
    }
}
```

Run `cargo test -p forge-entities card_view` → FAIL, then implement.

- [ ] **Step 2: `CardView` component** (same file):

```rust
use dioxus::prelude::*;
use forge_domain::Card;

/// One card: the text at the top and a badge at the bottom. In the composer it can be clicked and
/// removed; in the chat it is only shown.
#[component]
pub fn CardView(card: Card, on_click: Option<EventHandler<()>>, on_remove: Option<EventHandler<()>>) -> Element {
    let clickable = on_click.is_some();
    rsx! {
        div {
            class: if clickable { "card clickable" } else { "card" },
            title: "{card.path}",
            onclick: move |_| {
                if let Some(handler) = &on_click {
                    handler.call(());
                }
            },
            div { class: "card-text", "{card.text}" }
            span { class: "card-badge", "{card.badge()}" }
            if let Some(remove) = on_remove {
                button {
                    class: "card-remove",
                    r#type: "button",
                    "aria-label": "Remove",
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        remove.call(());
                    },
                    "×"
                }
            }
        }
    }
}
```

Export `CardView`, `modal_title` from `forge-entities/lib.rs`.

- [ ] **Step 3: Chat** — in `turn.rs` `TurnItem::User`, before the bubble:

```rust
if !message.cards.is_empty() {
    div { class: "cards-row",
        for (index, card) in message.cards.iter().enumerate() {
            CardView { key: "{index}", card: card.clone(), on_click: None, on_remove: None }
        }
    }
}
```

(Cards above the bubble, right aligned; the bubble condition `if !text.is_empty()` stays.) Add a test in `turn_view.rs`/`turn.rs` style used there if the file already tests user items; otherwise rely on Task 8's real window check.

- [ ] **Step 4: Composer** — `composer_cards.rs`:

```rust
use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{Card, CardKind};
use forge_entities::{modal_title, CardView};

use crate::workspace::use_workspace;

/// The unsent cards above the text. An excerpt opens a modal with its lines; a comment asks the
/// Files panel to show its box.
#[component]
pub fn ComposerCards(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut shown = use_signal(|| None::<Card>);
    let cards = workspace.cards(id);
    if cards.is_empty() && shown.read().is_none() {
        return rsx! {};
    }
    rsx! {
        if !cards.is_empty() {
            div { class: "cards-row composer-cards",
                for card in cards {
                    CardView {
                        key: "{card.id}",
                        card: card.clone(),
                        on_click: {
                            let card = card.clone();
                            move |_| match card.kind {
                                CardKind::Excerpt => shown.set(Some(card.clone())),
                                CardKind::Comment => workspace.request_comment_focus(id, card.id),
                            }
                        },
                        on_remove: {
                            let card_id = card.id;
                            move |_| workspace.remove_card(id, card_id)
                        },
                    }
                }
            }
        }
        if let Some(card) = shown() {
            div { class: "picker-backdrop", onclick: move |_| shown.set(None) }
            div { class: "card-modal", role: "dialog",
                div { class: "card-modal-head",
                    span { class: "card-modal-title", "{modal_title(&card)}" }
                    button { class: "card-remove always", r#type: "button", "aria-label": "Close", onclick: move |_| shown.set(None), "×" }
                }
                div { class: "card-modal-note", "{card.size_label()} • May not match the original formatting" }
                pre { class: "card-modal-code", "{card.text}" }
            }
        }
    }
}
```

In `SendMessageBox`, render `ComposerCards { id }` right before `ComposerChips {}`. Export from `forge-features/lib.rs`.

- [ ] **Step 5: CSS** in `theme.css` (use the existing variables `--bg`, `--border`, `--fg-muted`, `--accent-soft`; match the neighbouring `.chip` rules):

```css
.cards-row { display: flex; flex-wrap: wrap; gap: 8px; margin: 0 0 8px; }
.turn-user .cards-row { justify-content: flex-end; }
.card { position: relative; width: 200px; height: 200px; box-sizing: border-box; padding: 10px 12px; border: 1px solid var(--border); border-radius: 12px; background: var(--bg); display: flex; flex-direction: column; justify-content: space-between; overflow: hidden; }
.card.clickable { cursor: pointer; }
.card-text { font-size: 12px; white-space: pre-wrap; overflow: hidden; font-family: var(--mono); }
.card-badge { align-self: flex-start; font-size: 11px; color: var(--fg-muted); background: var(--bg-hover); border-radius: 6px; padding: 1px 6px; }
.card-remove { position: absolute; top: 6px; right: 6px; display: none; border: none; border-radius: 50%; width: 20px; height: 20px; cursor: pointer; background: var(--bg-hover); color: var(--fg); }
.card:hover .card-remove, .card-remove.always { display: block; }
.card-modal { position: fixed; z-index: 60; top: 12vh; left: 50%; transform: translateX(-50%); width: min(640px, 90vw); max-height: 70vh; overflow: auto; background: var(--bg); border: 1px solid var(--border); border-radius: 12px; padding: 14px 16px; }
.card-modal-head { display: flex; justify-content: space-between; align-items: center; font-weight: 600; }
.card-modal-note { color: var(--fg-muted); font-size: 12px; margin: 6px 0 10px; }
.card-modal-code { border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; margin: 0; overflow: auto; font-family: var(--mono); font-size: 12px; }
```

(`.card-modal-head .card-remove` sits inline: add `position: static;` for `.card-modal-head .card-remove`.)

- [ ] **Step 6: Run** — `cargo test -p forge-entities -p forge-features` → pass; `cargo build`.

- [ ] **Step 7: Commit** — `feat: cards in the composer, the excerpt modal and the chat (Plan 25B)`.

---

### Task 5: Line selection, bar, comment boxes and right click menu in the editor bundle

**Files:**
- Create: `tools/editor-bundle/src/lines.js`
- Modify: `tools/editor-bundle/src/main.js`, `tools/editor-bundle/test/bundle.test.mjs`, rebuilt `crates/forge-shared/assets/editor.js`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Produces (page to Rust, via `send`): `{ t: "attach", key, from, to, text }`, `{ t: "comment", key, from, to, text, id }` (`id` is `null` for a new comment), `{ t: "comment_delete", key, id }`.
- Produces (Rust to page, editor methods): `comments(key, [{ id, from, to, text }])`, `focusComment(key, id)`.
- `lines.js` exports `lineTools({ send, getKey })` returning `{ extensions, setComments(view, list), focusComment(view, id), reset(view) }`.

- [ ] **Step 1: Failing jsdom tests** — append to `bundle.test.mjs` (reuse `boot()`; `load` is queued through `whenIdle`, so `await wait(20)` after it as the existing tests do; check how the existing tests locate the editor DOM and reuse that approach for `view`):

```js
test("clicking a gutter number picks the line and the bar attaches it", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "one\ntwo\nthree\n", "python", false, false);
  await wait(20);
  const numbers = w.document.querySelectorAll(".cm-lineNumbers .cm-gutterElement");
  // The first gutter element is a hidden spacer: the lines start at index 1.
  numbers[2].dispatchEvent(new w.MouseEvent("mousedown", { bubbles: true, button: 0 }));
  w.document.dispatchEvent(new w.MouseEvent("mouseup", { bubbles: true }));
  const attach = w.document.querySelector(".cm-pick-bar [data-act=attach]");
  assert.ok(attach, "the bar shows under a picked line");
  attach.dispatchEvent(new w.MouseEvent("mousedown", { bubbles: true }));
  assert.deepEqual(plain(sent.filter((m) => m.t === "attach")), [{ t: "attach", key: "a.py", from: 2, to: 2, text: "two" }]);
});

test("a comment is sent with its lines and shows as a saved box once Rust sends it back", async () => {
  const { w, editor, sent } = boot();
  editor.load("a.py", "one\ntwo\nthree\n", "python", false, false);
  await wait(20);
  editor.startComment(1, 2);
  const input = w.document.querySelector(".cm-comment-box.editing input");
  assert.ok(input, "an editing box opens under the last picked line");
  input.value = "why?";
  w.document.querySelector(".cm-comment-box.editing .cm-comment-ok").dispatchEvent(new w.MouseEvent("mousedown", { bubbles: true }));
  assert.deepEqual(plain(sent.filter((m) => m.t === "comment")), [{ t: "comment", key: "a.py", from: 1, to: 2, text: "why?", id: null }]);
  editor.comments("a.py", [{ id: 7, from: 1, to: 2, text: "why?" }]);
  const saved = w.document.querySelector(".cm-comment-box.saved");
  assert.ok(saved);
  assert.equal(saved.querySelector(".cm-comment-text").textContent, "why?");
  saved.querySelector(".cm-comment-delete").dispatchEvent(new w.MouseEvent("mousedown", { bubbles: true }));
  assert.deepEqual(plain(sent.filter((m) => m.t === "comment_delete")), [{ t: "comment_delete", key: "a.py", id: 7 }]);
});

test("comments of another tab do not show, and come back with their tab", async () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\n", "python", false, false);
  editor.load("b.py", "two\n", "python", false, false);
  await wait(20);
  editor.comments("a.py", [{ id: 1, from: 1, to: 1, text: "mine" }]);
  assert.equal(w.document.querySelectorAll(".cm-comment-box").length, 0);
  editor.show("a.py");
  await wait(20);
  assert.equal(w.document.querySelectorAll(".cm-comment-box.saved").length, 1);
});

test("focusing a comment turns its box into the editing one", async () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\ntwo\n", "python", false, false);
  await wait(20);
  editor.comments("a.py", [{ id: 3, from: 2, to: 2, text: "hello" }]);
  editor.focusComment("a.py", 3);
  assert.ok(w.document.querySelector(".cm-comment-box.editing input"));
});
```

`startComment(from, to)` is a testing and Rust hook on the editor handle: it opens the draft box for those lines (the bar's `Add comment` and the menu call the same function). Run `cd tools/editor-bundle && npm run build && npm test` → the four new tests FAIL.

- [ ] **Step 2: Implement `lines.js`**:

```js
// Line tools for the editor: pick lines (gutter click, drag, shift click), the bar over them,
// comment boxes between lines, and the right click menu.
//
// Selection and comments are plain CodeMirror state, so they follow the tab's EditorState. A pick
// is { from, to } in 1 based line numbers.
import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, GutterMarker, WidgetType, gutter, lineNumbers, showTooltip } from "@codemirror/view";
import { redo, selectAll, undo } from "@codemirror/commands";

const setPick = StateEffect.define();
const setComments = StateEffect.define();
const setEditing = StateEffect.define(); // { id } of a saved comment shown as editing, or null
const openDraft = StateEffect.define(); // { from, to } of a new comment box, or null

const pickField = StateField.define({
  create: () => null,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setPick)) return e.value;
    return value;
  },
  provide: (f) => [
    EditorView.decorations.from(f, (pick) => (state) => pickDecorations(state, pick)),
    showTooltip.compute([f], (state) => {
      const pick = state.field(f);
      if (!pick) return null;
      const pos = state.doc.line(Math.min(pick.from, state.doc.lines)).from;
      return { pos, above: true, strictSide: true, arrow: false, create: () => ({ dom: barDom(state) }) };
    }),
  ],
});

function pickDecorations(state, pick) {
  if (!pick) return Decoration.none;
  const marks = [];
  const last = Math.min(pick.to, state.doc.lines);
  for (let n = pick.from; n <= last; n++) marks.push(Decoration.line({ class: "cm-picked-line" }).range(state.doc.line(n).from));
  return Decoration.set(marks);
}

let currentView = null; // set by `lineTools`, so the bar (built from state) can reach the view
function barDom() {
  const dom = document.createElement("div");
  dom.className = "cm-pick-bar";
  for (const [act, label] of [["attach", "Attach as context"], ["comment", "Add comment"]]) {
    const button = document.createElement("button");
    button.type = "button";
    button.dataset.act = act;
    button.textContent = label;
    // mousedown, not click: the editor must not lose the pick to a blur first.
    button.addEventListener("mousedown", (event) => {
      event.preventDefault();
      if (act === "attach") currentView.__lines.attach();
      else currentView.__lines.startComment();
    });
    dom.appendChild(button);
  }
  return dom;
}

class PlusMarker extends GutterMarker {
  toDOM() {
    const span = document.createElement("span");
    span.className = "cm-pick-plus";
    span.textContent = "+";
    return span;
  }
}
const plus = new PlusMarker();

// Saved comments and the draft box. Block widgets must come from a state field.
const commentsField = StateField.define({
  create: () => ({ list: [], editing: null, draft: null }),
  update(value, tr) {
    let next = value;
    for (const e of tr.effects) {
      if (e.is(setComments)) next = { ...next, list: e.value, editing: next.editing && e.value.some((c) => c.id === next.editing) ? next.editing : null };
      if (e.is(setEditing)) next = { ...next, editing: e.value, draft: null };
      if (e.is(openDraft)) next = { ...next, draft: e.value, editing: null };
    }
    return next;
  },
  provide: (f) => EditorView.decorations.from(f, (value) => (state) => commentDecorations(state, value)),
});

function commentDecorations(state, { list, editing, draft }) {
  const widgets = [];
  const at = (to) => state.doc.line(Math.min(Math.max(to, 1), state.doc.lines)).to;
  for (const c of list) widgets.push(Decoration.widget({ widget: new CommentBox(c, c.id === editing), block: true, side: 1 }).range(at(c.to)));
  if (draft) widgets.push(Decoration.widget({ widget: new CommentBox({ id: null, from: draft.from, to: draft.to, text: "" }, true), block: true, side: 1 }).range(at(draft.to)));
  return Decoration.set(widgets, true);
}

function title(c) {
  return c.from === c.to ? `Line ${c.from}` : `Lines ${c.from}-${c.to}`;
}

class CommentBox extends WidgetType {
  constructor(comment, editing) {
    super();
    this.comment = comment;
    this.editing = editing;
  }
  eq(other) {
    const a = this.comment, b = other.comment;
    return this.editing === other.editing && a.id === b.id && a.from === b.from && a.to === b.to && a.text === b.text;
  }
  ignoreEvent() {
    return true;
  }
  toDOM(view) {
    const c = this.comment;
    const box = document.createElement("div");
    box.className = "cm-comment-box " + (this.editing ? "editing" : "saved");
    const head = document.createElement("div");
    head.className = "cm-comment-title";
    head.textContent = title(c);
    box.appendChild(head);
    const send = view.__lines.send;
    if (this.editing) {
      const input = document.createElement("input");
      input.type = "text";
      input.placeholder = "Add comment";
      input.value = c.text;
      const ok = document.createElement("button");
      ok.type = "button";
      ok.className = "cm-comment-ok";
      ok.textContent = "⊕";
      const submit = () => {
        const text = input.value.trim();
        if (text === "") return;
        send({ t: "comment", key: view.__lines.key(), from: c.from, to: c.to, text, id: c.id });
        view.dispatch({ effects: [setEditing.of(null), openDraft.of(null)] });
      };
      ok.addEventListener("mousedown", (event) => { event.preventDefault(); submit(); });
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !event.isComposing) { event.preventDefault(); submit(); }
        if (event.key === "Escape") view.dispatch({ effects: [setEditing.of(null), openDraft.of(null)] });
      });
      box.append(input, ok);
    } else {
      const text = document.createElement("div");
      text.className = "cm-comment-text";
      text.textContent = c.text;
      const edit = document.createElement("button");
      edit.type = "button";
      edit.className = "cm-comment-edit";
      edit.textContent = "✎";
      const del = document.createElement("button");
      del.type = "button";
      del.className = "cm-comment-delete";
      del.textContent = "🗑";
      del.addEventListener("mousedown", (event) => { event.preventDefault(); event.stopPropagation(); send({ t: "comment_delete", key: view.__lines.key(), id: c.id }); });
      edit.addEventListener("mousedown", (event) => { event.preventDefault(); event.stopPropagation(); view.dispatch({ effects: setEditing.of(c.id) }); });
      box.addEventListener("mousedown", () => view.dispatch({ effects: setEditing.of(c.id) }));
      box.append(text, edit, del);
    }
    return box;
  }
}

export function lineTools({ send, getKey }) {
  let anchor = null;

  function lineOf(view, pos) {
    return view.state.doc.lineAt(pos).number;
  }
  function pick(view, a, b) {
    view.dispatch({ effects: setPick.of({ from: Math.min(a, b), to: Math.max(a, b) }) });
  }
  function startPick(view, line, shift) {
    const current = view.state.field(pickField);
    anchor = shift && current ? (line < current.from ? current.to : current.from) : line;
    pick(view, anchor, line);
    const move = (event) => {
      const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
      if (pos != null) pick(view, anchor, lineOf(view, pos));
    };
    const up = () => {
      document.removeEventListener("mousemove", move);
      document.removeEventListener("mouseup", up);
    };
    document.addEventListener("mousemove", move);
    document.addEventListener("mouseup", up);
  }

  function attach(view) {
    const p = view.state.field(pickField);
    if (!p) return;
    const lines = [];
    for (let n = p.from; n <= Math.min(p.to, view.state.doc.lines); n++) lines.push(view.state.doc.line(n).text);
    send({ t: "attach", key: getKey(), from: p.from, to: p.to, text: lines.join("\n") });
  }
  function startComment(view, from, to) {
    const p = view.state.field(pickField);
    const range = from != null ? { from, to } : p;
    if (!range) return;
    pick(view, range.from, range.to);
    view.dispatch({ effects: openDraft.of({ from: range.from, to: range.to }) });
    setTimeout(() => view.dom.querySelector(".cm-comment-box.editing input")?.focus(), 0);
  }

  const gutterMarkers = gutter({
    class: "cm-plus-gutter",
    lineMarker(view, line) {
      const p = view.state.field(pickField);
      return p && view.state.doc.lineAt(line.from).number === p.from ? plus : null;
    },
    lineMarkerChange: (update) => update.transactions.some((tr) => tr.effects.some((e) => e.is(setPick))),
  });

  const extensions = [
    pickField,
    commentsField,
    gutterMarkers,
    lineNumbers({
      domEventHandlers: {
        mousedown(view, line, event) {
          if (event.button !== 0) return false;
          startPick(view, lineOf(view, line.from), event.shiftKey);
          return true;
        },
      },
    }),
    EditorView.domEventHandlers({
      contextmenu(event, view) {
        event.preventDefault();
        const pos = view.posAtCoords({ x: event.clientX, y: event.clientY });
        if (pos != null) {
          const line = lineOf(view, pos);
          const p = view.state.field(pickField);
          if (!p || line < p.from || line > p.to) pick(view, line, line);
        }
        openMenu(view, event.clientX, event.clientY);
        return true;
      },
    }),
  ];

  function openMenu(view, x, y) {
    document.querySelector(".cm-ctx-menu")?.remove();
    const menu = document.createElement("div");
    menu.className = "cm-ctx-menu";
    const items = [
      ["Attach as context", () => attach(view)],
      ["Add comment", () => startComment(view)],
      null,
      ["Undo", () => undo(view)],
      ["Redo", () => redo(view)],
      null,
      ["Cut", () => document.execCommand("cut")],
      ["Copy", () => document.execCommand("copy")],
      ["Paste", async () => { try { view.dispatch(view.state.replaceSelection(await navigator.clipboard.readText())); } catch { /* no clipboard access */ } }],
      null,
      ["Select all", () => selectAll(view)],
    ];
    for (const item of items) {
      if (item === null) {
        const rule = document.createElement("div");
        rule.className = "cm-ctx-rule";
        menu.appendChild(rule);
        continue;
      }
      const button = document.createElement("button");
      button.type = "button";
      button.textContent = item[0];
      button.addEventListener("mousedown", (event) => { event.preventDefault(); menu.remove(); item[1](); view.focus(); });
      menu.appendChild(button);
    }
    menu.style.left = `${Math.min(x, window.innerWidth - 190)}px`;
    menu.style.top = `${Math.min(y, window.innerHeight - 260)}px`;
    document.body.appendChild(menu);
    const close = () => { menu.remove(); document.removeEventListener("mousedown", close, true); };
    setTimeout(() => document.addEventListener("mousedown", close, true), 0);
  }

  return {
    extensions,
    bind(view) {
      currentView = view;
      view.__lines = { send, key: getKey, attach: () => attach(view), startComment: () => startComment(view) };
    },
    setComments(view, list) {
      view.dispatch({ effects: setComments.of(list) });
    },
    focusComment(view, id) {
      const c = view.state.field(commentsField).list.find((x) => x.id === id);
      if (!c) return;
      view.dispatch({ effects: [setEditing.of(id), EditorView.scrollIntoView(view.state.doc.line(Math.min(c.to, view.state.doc.lines)).from, { y: "center" })] });
      setTimeout(() => view.dom.querySelector(".cm-comment-box.editing input")?.focus(), 0);
    },
    startComment: (view, from, to) => startComment(view, from, to),
  };
}
```

- [ ] **Step 3: Wire into `main.js`** — import `lineTools`; in `create`, `const tools = lineTools({ send, getKey: () => key });`; in `makeState` replace `lineNumbers()` with `...tools.extensions`; after `const view = new EditorView(...)` call `tools.bind(view)`; keep `comments` per key: `const commentsByKey = new Map();`. In `load` and `show` after `view.setState(...)` and `reapply(...)` call `tools.bind(view); tools.setComments(view, commentsByKey.get(newKey) || []);` (a state's old comment list would otherwise show stale). Add methods:

```js
comments(forKey, list) {
  commentsByKey.set(forKey, list);
  if (forKey === key) tools.setComments(view, list);
},
focusComment(forKey, id) {
  if (forKey === key) tools.focusComment(view, id);
},
startComment(from, to) {
  tools.startComment(view, from, to);
},
```

`drop(oldKey)` also does `commentsByKey.delete(oldKey)`. Update the header comment block of `main.js` (Rust to page: `comments`, `focusComment`; page to Rust: `attach`, `comment`, `comment_delete`). Note: the `setPick` selection lives in the tab's `EditorState`, so a pick survives a tab switch; a fresh `load` starts without one.

- [ ] **Step 4: CSS** in `theme.css` (matching the editor's variables):

```css
.cm-picked-line { background-color: var(--accent-soft); }
.cm-plus-gutter { width: 18px; }
.cm-pick-plus { display: inline-block; width: 16px; height: 16px; line-height: 16px; text-align: center; border-radius: 50%; background: var(--accent); color: #fff; font-size: 12px; cursor: pointer; }
.cm-pick-bar { display: flex; gap: 2px; padding: 3px; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 2px 8px rgba(0,0,0,.15); }
.cm-pick-bar button { border: none; background: transparent; color: var(--fg); padding: 3px 8px; border-radius: 6px; font-size: 12px; cursor: pointer; }
.cm-pick-bar button:hover { background: var(--bg-hover); }
.cm-comment-box { box-sizing: border-box; margin: 4px 12px 4px 48px; padding: 6px 10px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg); font-family: var(--sans, inherit); font-size: 12px; }
.cm-comment-box.editing { border-color: var(--accent); }
.cm-comment-title { color: var(--fg-muted); margin-bottom: 4px; }
.cm-comment-box input { width: calc(100% - 34px); border: none; outline: none; background: transparent; color: var(--fg); font-size: 12px; }
.cm-comment-ok, .cm-comment-edit, .cm-comment-delete { border: none; background: transparent; cursor: pointer; color: var(--fg-muted); }
.cm-comment-delete { color: #d44; }
.cm-comment-box.saved { cursor: pointer; }
.cm-ctx-menu { position: fixed; z-index: 70; min-width: 180px; padding: 4px; background: var(--bg); border: 1px solid var(--border); border-radius: 8px; box-shadow: 0 4px 16px rgba(0,0,0,.2); display: flex; flex-direction: column; }
.cm-ctx-menu button { text-align: left; border: none; background: transparent; color: var(--fg); padding: 5px 10px; border-radius: 6px; font-size: 12px; cursor: pointer; }
.cm-ctx-menu button:hover { background: var(--bg-hover); }
.cm-ctx-rule { height: 1px; margin: 4px 0; background: var(--border); }
```

(If `--accent` is not defined, use the variable the existing `.chip` or selected-row rules use for blue.)

- [ ] **Step 5: Run** — `cd tools/editor-bundle && npm run build && npm test` → all pass (the 8 old tests plus 4 new). If the jsdom gutter test cannot find `.cm-lineNumbers .cm-gutterElement` because jsdom has no layout, build the pick through `editor.startComment(1, 2)`-style hooks for that test only and leave the gutter mousedown check to Task 8; note this in the test name.

- [ ] **Step 6: Commit** — `feat: line selection, comment boxes and a right click menu in the editor (Plan 25B)`, including the rebuilt `crates/forge-shared/assets/editor.js`.

---

### Task 6: The Rust side of the editor tools

**Files:**
- Modify: `crates/forge-widgets/src/files_editor.rs`, `crates/forge-widgets/src/files_state.rs` (`FilesCtx`, `on_event`), `crates/forge-widgets/src/files_panel.rs`

**Interfaces:**
- Consumes: Task 3 `Workspace` methods; Task 5 page messages.
- Produces: `FromPage::{Attach { key, from, to, text }, Comment { key, from, to, text, id: Option<u64> }, CommentDelete { key, id }}`; `EditorHandle::comments(key, &[Card])`, `EditorHandle::focus_comment(key, id)`; `FilesCtx.id: LiveSessionId`.

- [ ] **Step 1: Failing tests** in `files_editor.rs`:

```rust
#[test]
fn the_line_messages_from_the_page_parse() {
    assert_eq!(
        parse(r#"{"t":"attach","key":"a.py","from":2,"to":4,"text":"x\ny\nz"}"#),
        FromPage::Attach { key: "a.py".to_string(), from: 2, to: 4, text: "x\ny\nz".to_string() }
    );
    assert_eq!(
        parse(r#"{"t":"comment","key":"a.py","from":3,"to":3,"text":"why","id":null}"#),
        FromPage::Comment { key: "a.py".to_string(), from: 3, to: 3, text: "why".to_string(), id: None }
    );
    assert_eq!(
        parse(r#"{"t":"comment","key":"a.py","from":3,"to":3,"text":"why","id":7}"#),
        FromPage::Comment { key: "a.py".to_string(), from: 3, to: 3, text: "why".to_string(), id: Some(7) }
    );
    assert_eq!(parse(r#"{"t":"comment_delete","key":"a.py","id":7}"#), FromPage::CommentDelete { key: "a.py".to_string(), id: 7 });
}

#[test]
fn the_comment_commands_carry_what_the_bridge_reads() {
    let card = Card { id: 7, kind: CardKind::Comment, path: "/p/a.py".into(), range: LineRange::new(3, 4), text: "why".into() };
    assert_eq!(
        comments_command("a.py", &[card]),
        serde_json::json!({ "t": "comments", "key": "a.py", "comments": [{ "id": 7, "from": 3, "to": 4, "text": "why" }] })
    );
    assert_eq!(focus_comment_command("a.py", 7), serde_json::json!({ "t": "focus_comment", "key": "a.py", "id": 7 }));
}
```

Run `cargo test -p forge-widgets files_editor` → FAIL.

- [ ] **Step 2: Implement** — add the three `FromPage` variants (docs like the others), `use forge_domain::{Card, CardKind, LineRange}` where needed, and:

```rust
pub fn comments_command(key: &str, cards: &[Card]) -> Value {
    let list: Vec<Value> = cards.iter().map(|c| json!({ "id": c.id, "from": c.range.start, "to": c.range.end, "text": c.text })).collect();
    json!({ "t": "comments", "key": key, "comments": list })
}

pub fn focus_comment_command(key: &str, id: u64) -> Value {
    json!({ "t": "focus_comment", "key": key, "id": id })
}
```

`EditorHandle::comments(&self, key, cards)` and `focus_comment(&self, key, id)` send them. In `BRIDGE_JS` add `else if (m.t === "comments") editor.comments(m.key, m.comments); else if (m.t === "focus_comment") editor.focusComment(m.key, m.id);`.

- [ ] **Step 3: `FilesCtx`** — add `pub id: LiveSessionId` (set in `FilesPanel` where `FilesCtx { .. }` is built: `id`). Add arms to `on_event` (the compiler lists the missing ones). Absolute path is `self.cwd.join(&key)`:

```rust
FromPage::Attach { key, from, to, text } => {
    let path = self.cwd.join(&key).to_string_lossy().to_string();
    self.workspace.add_excerpt(self.id, path, LineRange::new(from, to), text);
}
FromPage::Comment { key, from, to, text, id } => {
    let path = self.cwd.join(&key).to_string_lossy().to_string();
    match id {
        Some(card_id) => self.workspace.update_comment(self.id, card_id, text),
        None => {
            self.workspace.add_comment(self.id, path, LineRange::new(from, to), text);
        }
    }
}
FromPage::CommentDelete { id, .. } => self.workspace.remove_card(self.id, id),
```

- [ ] **Step 4: Push comments and handle focus** in `FilesPanel` (next to the existing file request effect). The effect reads the cards signal and the open tabs, so it reruns when either changes, and only talks to the page once `ready`:

```rust
// The comments of every open file go to the page whenever they change.
{
    let ctx = ctx.clone();
    use_effect(move || {
        let cards = workspace.cards(id);
        let ready = ctx.ready.read().to_owned();
        let keys: Vec<String> = ctx.tabs.read().tabs().iter().map(|tab| tab.path.clone()).collect();
        if !ready {
            return;
        }
        for key in keys {
            let path = ctx.cwd.join(&key).to_string_lossy().to_string();
            let list: Vec<_> = cards.iter().filter(|c| c.kind == CardKind::Comment && c.path == path).cloned().collect();
            ctx.handle.comments(&key, &list);
        }
    });
}
// A comment card in the message box was clicked: open its file and show its box.
{
    let ctx = ctx.clone();
    use_effect(move || {
        let Some(card_id) = workspace.comment_focus(id) else { return };
        workspace.clear_comment_focus(id);
        let Some(card) = workspace.cards(id).into_iter().find(|c| c.id == card_id) else { return };
        let Ok(relative) = std::path::Path::new(&card.path).strip_prefix(&ctx.cwd) else { return };
        let key = relative.to_string_lossy().to_string();
        ctx.open(key.clone());
        ctx.handle.focus_comment(&key, card_id);
    });
}
```

If a tab is opened by the focus request and its editor has not loaded the file yet, the page ignores `focus_comment` for a key that is not current; resend it after load: in the existing arm that handles a finished load (search `handle.load(` in `files_state.rs`) nothing else is needed because the comments effect already runs again when `tabs` changes, and `commentsByKey` is applied on `load`; the focus call is queued behind `load` in the same command channel, so order is kept (the channel is FIFO and `load` is sent first by `ctx.open`). Verify in Task 8.

- [ ] **Step 5: Run** — `cargo test -p forge-widgets` → pass; `cargo build`.

- [ ] **Step 6: Commit** — `feat: the Files panel sends excerpts and comments to the message box (Plan 25B)`.

---

### Task 7: Attach a whole file from a tree row and a tab

**Files:**
- Modify: `crates/forge-widgets/src/files_tabs.rs` (tab menu, props), `crates/forge-widgets/src/files_panel.rs` (tree row menu, handlers), `crates/forge-widgets/src/files_state.rs` (`FilesCtx::attach_file`)

**Interfaces:**
- Consumes: `Workspace::queue_file`.
- Produces: `FilesCtx::attach_file(&self, path: &str)` (path relative to `cwd`); `FilesTabs` prop `on_attach: EventHandler<String>`.

- [ ] **Step 1: Failing test** — a pure helper in `files_state.rs`, tested without UI:

```rust
/// The absolute path a tree or tab path stands for.
pub fn absolute(cwd: &std::path::Path, path: &str) -> std::path::PathBuf {
    cwd.join(path)
}
```

with test `absolute(Path::new("/p"), "src/a.py") == PathBuf::from("/p/src/a.py")`. (Trivial; the real behaviour is checked in Task 8.) Run, see it fail, add, pass.

- [ ] **Step 2: `attach_file`**:

```rust
pub fn attach_file(&self, path: &str) {
    self.workspace.queue_file(self.id, absolute(&self.cwd, path));
}
```

- [ ] **Step 3: Tab menu** — in `files_tabs.rs` add the prop `on_attach: EventHandler<String>` and, as the first item of the tab menu (before `Keep open`), a button `Attach as context` calling `on_attach.call(path.clone()); menu.set(None);`, followed by a divider `div { class: "picker-rule" }` (reuse the class the other menus use for a divider; `grep -n "rule\|divider" crates/forge-widgets/src/*.rs`). Pass `on_attach` from `FilesPanel`: `move |path: String| ctx.attach_file(&path)`.

- [ ] **Step 4: Tree row menu** — in `files_panel.rs` add `let mut row_menu = use_signal(|| None::<(String, f64, f64)>);`; on each file row button (not directories) add

```rust
oncontextmenu: {
    let path = row.path.clone();
    move |event: MouseEvent| {
        event.prevent_default();
        let point = event.client_coordinates();
        row_menu.set(Some((path.clone(), point.x, point.y)));
    }
},
```

and render, next to the banners, the same backdrop and `picker-menu files-tab-menu` markup as the tab menu with one item `Attach as context` that calls `ctx.attach_file(&path)` and closes the menu. (Use the same `style: "left: min({x}px, calc(100vw - 210px)); top: {y}px;"`.)

- [ ] **Step 5: Run** — `cargo test -p forge-widgets` and `cargo build` → pass.

- [ ] **Step 6: Commit** — `feat: attach a file from a tree row or a tab (Plan 25B)`.

---

### Task 8: Check in the real window, fix, write down what was found

**Files:**
- Modify: `docs/superpowers/specs/2026-10-05-line-comments-design.md` (add "As built" and the real-window results)

- [ ] **Step 1: Whole run** — `cargo test --workspace` and `cd tools/editor-bundle && npm test` → all green. Record the counts.

- [ ] **Step 2: Drive the app** with the harness (memory: "Driving the Forge app visually": `screencapture`, Quartz clicks, `FORGE_CLAUDE_HOME` fixture; the screen must be unlocked; a Korean input method turns `keystroke` letters into jamo, use `key code`; `click.py` takes pixels of the 2400 wide capture, viewer coordinates times 1.2; right click and drag need a small Quartz script, screen = window origin + pixels / 2). Check, one line each:
  1. Click a gutter number: blue band, `+`, bar over the line; click another number: the pick moves.
  2. Drag over three numbers, and shift click: one band over the range, the bar over its first line.
  3. `Add comment`: box under the last line; type, `⊕`: saved box and a `Comment` card in the message box; pencil edits, trash deletes and the card goes.
  4. `Attach as context`: `Excerpt` card; click it: modal `<abs path>:L5` with size line and the code.
  5. The card `×` on a comment card removes the box in the panel.
  6. Click a comment card while another file is open: the comment's file opens and the box is focused.
  7. Send with one comment, one excerpt and text: cards above the bubble, comment box gone from the panel, excerpt band stays; the model answers (it can read the content).
  8. Close the session and open it again: the cards are still there; the bubble has only the typed text; the sidebar title has no tags.
  9. Right click in the text: the menu, caret moved to the line; the first two items work; Undo / Cut / Copy / Paste / Select all work.
  10. Right click a tree row and a tab: `Attach as context` puts a file chip above the text.
  11. Dark mode: bar, boxes, cards and menu are readable.
  12. A file edited so it is shorter than a stored excerpt, then sent: no `L40` beyond the file in the transcript (read the jsonl).
  Each defect found: write a failing test if one can exist, fix, commit `fix: …`.

- [ ] **Step 3: Docs** — add an "As built" section to the spec (what differs from it: single `Card` struct instead of an enum, no `setSelection` command because the pick lives in the page, English labels, file attach uses the chip row, `Go to line` left out, bar anchored at the line start) and a "Checked in the real window" list with what was seen and what was not. Update `docs/superpowers/roadmap/` status for PNL-04 if that file tracks it (`grep -n "PNL-04" docs/superpowers/roadmap/*.md`).

- [ ] **Step 4: Commit** — `docs: Plan 25B checked in the real window`.

- [ ] **Step 5: Finish** — invoke `superpowers:finishing-a-development-branch`; the base branch is `master` (the repo's main is `master`; the git status header says `main`, check with `git branch -a` first).
