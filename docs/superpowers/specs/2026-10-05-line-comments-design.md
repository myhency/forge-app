# Line excerpts, comments and attach from the Files panel (Plan 25B)

Second of three pieces after 25A (the editor). Source: observation round 9 parts 1, 2, 5 (`docs/superpowers/roadmap/observations.md`). 25C (search toggles, in-file find bar, copy / open in / reveal items) is not in this spec.

## Scope, decided with the user on 2026-10-05

- Line selection in the editor (gutter click, gutter drag, shift click) with the `컨텍스트로 첨부 | 댓글 추가` bar, the inline comment box, the editor right click items for both.
- Cards in the composer (`발췌`, `댓글`), the excerpt modal, the same cards in the chat above the user bubble, kept after reopening the session.
- `컨텍스트로 첨부` on a tree row and on a tab: a file chip in the composer (the existing `Attachment` path).
- Approach A for persistence: the text sent to the model carries tagged blocks and the transcript reader turns them back into cards. No side file (B) and no "send only" (C).
- Out: following a comment when edits move lines, 25C items, matching the real app's model-side format (not observable).

## Data

- `forge-domain`: `LineRange { start, end }` (1 based, inclusive, one line = start == end), `Excerpt { path, range, text }`, `Comment { id, path, range, text }`, `Card = Excerpt | Comment`. `Message` gets `cards: Vec<Card>` (user messages; comments read back from a transcript carry id 0 and are inert).
- `FilesCtx` keeps the selected range per tab and the comments per path. The composer keeps `cards: Vec<Card>` beside its attachments.
- Removing a comment card also removes the panel comment of the same id, and the reverse. Sending consumes the comments (panel boxes disappear); an excerpt's blue band stays.
- Line numbers are clamped to the file length when sent. A comment keeps its line numbers when the text changes.

## Editor (tools/editor-bundle)

- Gutter click selects a line, drag or shift click a range. Light blue band across the code column, round `+` in the gutter, the bar floats above the first selected line.
- The comment box is a block widget under the last selected line. Editing state: blue border, title `N번 줄` (`N–M번 줄` for a range), text field, `⊕`. Saved state: grey border, text, pencil and red trash. Clicking a saved box edits it again.
- Right click in the text moves the caret to the clicked line (keeps an existing selection it falls in) and opens a menu with the two items first, then the existing undo / cut / copy / paste / select all / go to line items.
- Page to Rust events: `select`, `attach`, `comment_add`, `comment_edit`, `comment_delete`. Rust to page: `setComments`, `focusComment`, `setSelection`. jsdom tests for each.

## Widgets

- Composer: a row of 200x200 cards above the text, body text at the top, badge bottom left, `×` on hover. Excerpt card click opens a modal titled `<absolute path>:L5` (`:L5-L8` for a range), a grey line `0.02KB • 1줄 • 원본과 형식이 일치하지 않을 수 있습니다`, the text in a bordered code box, `×`. Comment card click focuses the comment in the panel (no modal).
- Chat: the same cards right aligned in one row above the user bubble; clicking does nothing.
- Tree row and tab right click: `컨텍스트로 첨부` puts a file chip in the composer text with the caret after it (existing `add_attachments`, de-duplicated).

## Send and restore (approach A)

- `plan_send` writes, before the typed text, one block per card:
  `<forge-excerpt path="P" lines="5-8">TEXT</forge-excerpt>` and `<forge-comment path="P" lines="3">TEXT</forge-comment>`, with `&`, `<`, `>` and `"` escaped. A message of cards only is allowed. The sidecar is unchanged (it receives text).
- `forge-adapter-claude-fs` `apply_user_line` strips leading blocks of exactly this shape into `Message.cards`; the bubble shows only what remains. A malformed or unknown block stays in the text (nothing is dropped). Fork copies the transcript, so cards follow.

## Tests

Round trip of serialise and parse (Korean, newlines, `<`, `&`, ranges, text that looks like a tag); card and comment linking; clamping; send with cards only; bundle jsdom tests; then a real window check with the harness (add, send, reopen the session, right click, tree and tab attach, range, dark mode).
