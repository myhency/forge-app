# Content search toggles, result list and the in-file find bar (Plan 25C-1)

Third piece of the Files panel work (25A editor, 25B excerpts and comments). 25C is split by the user (2026-10-05): **25C-1 (this spec)** the `?` search with its toggles and grouped results, and the in-file find bar; **25C-2 (later)** the tree row and tab right click items (copy, open in, reveal, reload, close, find in file), `Go to line`, and moving the panel's state up into `Workspace`. Source: observation round 9 part 5 (`docs/superpowers/roadmap/observations.md`). Branch `plan-25C`, started from `plan-25B` (25B is not merged yet).

## Observed behaviour to copy

- Typing `?Buzz` in the filter field shows three toggles at the right end of the field: `Aa` (match case), `ab` underlined (whole word), `.*` (regular expression), and `×`.
- Results are grouped by file: the file row with its glyph, under it one row per hit: a grey line number, then the line text with the match on a blue background; long lines are cut with `…` on both sides around the match. Several files in one list.
- A click on a hit opens the file (an existing tab is reused, a new file gets an italic tab), puts the caret on that line and opens the in-file find bar at the top of the editor: a field with the term, `Aa`, `ab`, `.*`, the count ("2개 중 2개"), an up and a down arrow and `×`. Every match in the file is shaded, the current one has a green box.

## Decisions (with the user, 2026-10-05)

- Scope: search toggles + results + find bar now, right click items later.
- Rust: `regex` crate (a new Cargo dependency, MIT/Apache) does all three modes; labels in English like the rest of Forge (`Match case`, `Whole word`, `Regex` as titles of the toggles; the toggles themselves read `Aa`, `ab`, `.*`).
- The find bar is written in the bundle on `showPanel` and `Decoration.mark` (no `@codemirror/search` dependency); matching uses JS regular expressions with the `u` flag.

## Search (forge-ports, forge-adapter-files)

- `SearchOptions { match_case: bool, whole_word: bool, regex: bool }` (all false = today's behaviour except that the default stays case insensitive).
- `FilesPort::search(root, query, options, limit)`. `ContentHit` gets `start` and `end`: char offsets of the first match inside `text`. When a line is longer than 200 chars, `text` is a window of at most 200 chars around the match, with `…` at a cut end, and `start`/`end` index into that text.
- Pattern: literal queries are escaped; whole word wraps in `(?<![\p{L}\p{N}_])` … `(?![\p{L}\p{N}_])` (the regex crate has no look-around, so use `\b` with Unicode word boundaries: `\b(?:…)\b`); case insensitive adds `(?i)`. A query that is not a valid regex gives `FilesError::InvalidPattern(String)`; the panel shows it as a note, not as "No matches".
- Everything else stays: path order, skipped folders, symlinks, binary files, size and file limits.

## Result list (forge-features, forge-widgets)

- `group_hits(&[ContentHit]) -> Vec<HitGroup { path, hits: Vec<ContentHit> }>` keeps path order.
- The panel keeps `search_options: Signal<SearchOptions>`; the toggles show only for a `?` query and re-run the search when changed; `×` clears the field.
- A hit row shows the line number and the text with the match in `<mark>` (split at `start`/`end`). A click does `ctx.open(path)` and then `ctx.find_in_file(path, query, options, line)`.

## Find bar (tools/editor-bundle)

- State: query, options, the open flag and the current match index, one per editor (not per tab). Matches are found over the whole document with a JS `RegExp` (`u`, `g`, `i` unless match case; literal queries escaped; whole word with `(?<![\p{L}\p{N}_])…(?![\p{L}\p{N}_])`). An invalid regex shows a red border on the field and no matches.
- UI: a panel at the top with the field, `Aa`, `ab`, `.*`, the count `N of M` (`No results` when none), up and down arrows and `×`. Enter next, Shift-Enter previous (not while an IME composition is going on), Escape closes, `⌘F` opens (selected text fills the field). All matches are marked; the current one has a green outline and is scrolled into view.
- Rust to page: `find(key, { query, match_case, whole_word, regex, line })` opens the bar for that tab, sets the query and options, moves the caret to `line` and makes the first match on that line (or the next one after it) current. Page to Rust: nothing new (the bar is local).

## Tests

Adapter tests (each option alone and together, a Hangul word, an invalid regex, the window and offsets of a long line, existing tests with the new argument); `group_hits`; bundle jsdom tests (counts, next and previous wrap, toggles, an invalid regex, a Hangul whole word, `find()` landing on a line, Escape closes); then a real window check with the harness (`?Buzz`, each toggle, a hit click, the bar, `⌘F`).

## Out of scope

25C-2 items; replacing text; searching in a folder other than the session folder; keeping the toggles between sessions.

## As built (2026-10-05)

Built on branch `plan-25C` (from `plan-25B`, which was merged to `master` as bf8f3e1 first), executed inline. 1073 Rust tests and 23 jsdom bundle tests pass.

Differs from the spec above:

- `FilesError` gained `InvalidPattern`; the files adapter's `write` maps it to `WriteError::Failed` (it cannot happen there).
- A long line's window keeps only 24 chars before the match (`HIT_LEAD_CHARS`), not a centred match: the real window check showed a centred match falling outside the narrow list. The list wraps a hit's text over at most 4 lines (the match stays visible) instead of one clipped row.
- The hit row shows the line number to the left of the text (a row), not above it.
- The find bar's state belongs to the editor: `find.js` remembers it and puts it back when a tab is shown; a `find` for a tab that is not loaded yet waits in `pendingFind`.

**Checked in the real window (2026-10-05, harness).** With a Korean input method on, so the query was Hangul (`?가나`): the three toggles and `×` under the filter field; hits grouped by file with the line number and the match marked; a long line shows the match with `…` around it; a click on a hit opens the file in an italic tab, puts the caret on the line and opens the find bar with the query, `1 of 2`, every match shaded and the current one outlined green; the bar's `ab` toggle made it `1 of 1` and `가나다` was left unmarked; the panel's `ab` toggle dropped the `가나다` lines from the list; `×` closed the bar and the marks; `⌘F` reopened it with the selected word and the remembered options.

Not checked in the real window: `Aa` and `.*` (unit and jsdom tests cover them; Latin letters cannot be typed while the Korean input method is on), an invalid regex (note in the list, red field in the bar: tests only), Enter and the arrows in the bar, dark mode.
