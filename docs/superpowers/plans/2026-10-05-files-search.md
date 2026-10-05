# Content search toggles, result list and the in-file find bar (Plan 25C-1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `?query` in the Files panel filter gets `Aa` / `ab` / `.*` toggles and a result list grouped by file with the match highlighted; a click on a hit opens the file, lands on the line and opens an in-file find bar; `⌘F` opens the same bar.

**Architecture:** `FilesPort::search` takes `SearchOptions` and returns hits with match offsets (the `regex` crate does all modes). Pure helpers group hits and split a hit's text around its match. The CodeMirror bundle gets a find bar (`find.js`: state field, match decorations, `showPanel`) driven from the page and from Rust by one `find` message, using the same pending-until-loaded pattern as `focusComment`.

**Tech Stack:** Rust (Dioxus 0.7.10, `regex` 1), CodeMirror 6 bundle (esbuild, jsdom tests).

**Spec:** `docs/superpowers/specs/2026-10-05-files-search-design.md`

## Global Constraints

- Branch `plan-25C` in `/Users/james/orca/workspaces/forge/plan-25B`; base `master` (25B is merged). Commit trailer: `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>`.
- English labels like the rest of Forge. Toggle button texts `Aa`, `ab`, `.*`, `×`; titles `Match case`, `Whole word`, `Regex`; find bar count `N of M` or `No results`; the invalid-pattern note is the regex error's last line.
- Search keeps today's limits: path order, skipped `.git` and `node_modules`, no symlinks, binary and large files skipped, at most 20 000 files, 1 MiB per file, `limit` hits, one hit (the first match) per line.
- A hit's `text` is at most 200 chars including `…` at a cut end; `start`/`end` are char offsets of the first match inside `text`.
- Whole word means Unicode word boundaries: Rust `\b(?:…)\b`; JS `(?<![\p{L}\p{N}_])(?:…)(?![\p{L}\p{N}_])` with the `u` flag.
- After a bundle change: `cd tools/editor-bundle && npm run build && npm test` (the built `crates/forge-shared/assets/editor.js` is committed). The worktree already has `node_modules` (`npm ci` was run).
- Out of scope: 25C-2 items, replace, other folders, keeping toggles between sessions.

## Review Focus

- A Hangul query with whole word on must match `가나 다` for `가나` and not `가나다` (adapter test and bundle test).
- An invalid regex (`(`) must give a note in the panel and a red field in the find bar, never a panic or "No matches" (adapter test, bundle test).
- A pattern that can match the empty string (`.*`, `a*`) must not hang or flood: empty matches are skipped (adapter test, bundle test).
- A match longer than the 200 char window or a line with multi-byte chars before the match keeps `start`/`end` on char boundaries (adapter test).
- A click on a hit while the tab is not loaded yet must still land on the line (page keeps a pending find; bundle test).
- Enter during an IME composition in the find field must not jump to the next match (bundle test).

---

### Task 1: Search options, match offsets and the `regex` crate

**Files:**
- Modify: `crates/forge-ports/src/files_port.rs`, `crates/forge-ports/src/lib.rs`, `crates/forge-adapter-files/Cargo.toml`, `crates/forge-adapter-files/src/lib.rs`
- Test: `crates/forge-adapter-files/src/lib.rs` (tests module)

**Interfaces:**
- Produces: `SearchOptions { match_case: bool, whole_word: bool, regex: bool }` (`Debug, Clone, Copy, Default, PartialEq, Eq`); `ContentHit { path, line, text, start: usize, end: usize }`; `FilesError::InvalidPattern(String)`; `FilesPort::search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError>`.

- [ ] **Step 1: Failing tests.** In the adapter tests module, change the two existing `search(...)` calls to pass `SearchOptions::default()` (third argument) and the `ContentHit` literals in `search_finds_lines_in_any_case_...` to carry `start`/`end`:

```rust
let expected = vec![
    ContentHit { path: "README.md".to_string(), line: 2, text: "hello World".to_string(), start: 0, end: 5 },
    ContentHit { path: "src/deep/notes.txt".to_string(), line: 2, text: "HELLO again".to_string(), start: 0, end: 5 },
    ContentHit { path: "src/main.rs".to_string(), line: 2, text: "println!(\"hello\");".to_string(), start: 10, end: 15 },
];
```

Replace `a_very_long_line_is_shortened_in_a_hit` with the version below and add the new tests (use a small helper `one_file(text)` that writes `f.txt` in a tempdir and returns the dir):

```rust
fn one_file(text: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("f.txt"), text).unwrap();
    dir
}

fn hits(dir: &TempDir, query: &str, options: SearchOptions) -> Vec<ContentHit> {
    LocalFiles::new().search(dir.path(), query, options, 50).unwrap()
}

#[test]
fn a_long_line_is_a_window_around_the_match_with_offsets_into_the_window() {
    let dir = one_file(&format!("{} needle {}\n", "x".repeat(5000), "y".repeat(5000)));
    let found = hits(&dir, "needle", SearchOptions::default());
    let hit = &found[0];
    assert!(hit.text.chars().count() <= 200, "{}", hit.text.chars().count());
    assert!(hit.text.starts_with('…') && hit.text.ends_with('…'));
    let matched: String = hit.text.chars().skip(hit.start).take(hit.end - hit.start).collect();
    assert_eq!(matched, "needle");
}

#[test]
fn a_match_at_the_start_of_a_long_line_cuts_only_the_end() {
    let dir = one_file(&format!("needle {}\n", "x".repeat(5000)));
    let hit = &hits(&dir, "needle", SearchOptions::default())[0];
    assert!(hit.text.starts_with("needle") && hit.text.ends_with('…'));
    assert_eq!((hit.start, hit.end), (0, 6));
}

#[test]
fn offsets_count_chars_not_bytes() {
    let dir = one_file("한글 한글 hello\n");
    let hit = &hits(&dir, "hello", SearchOptions::default())[0];
    let matched: String = hit.text.chars().skip(hit.start).take(hit.end - hit.start).collect();
    assert_eq!(matched, "hello");
}

#[test]
fn match_case_and_the_default_differ() {
    let dir = one_file("Buzz\nbuzz\n");
    assert_eq!(hits(&dir, "buzz", SearchOptions::default()).len(), 2);
    assert_eq!(hits(&dir, "buzz", SearchOptions { match_case: true, ..SearchOptions::default() }).len(), 1);
}

#[test]
fn whole_word_uses_unicode_word_boundaries() {
    let dir = one_file("가나 다\n가나다\nfoo foobar\n");
    let whole = SearchOptions { whole_word: true, ..SearchOptions::default() };
    let lines: Vec<usize> = hits(&dir, "가나", whole).iter().map(|h| h.line).collect();
    assert_eq!(lines, vec![1]);
    let lines: Vec<usize> = hits(&dir, "foo", whole).iter().map(|h| h.line).collect();
    assert_eq!(lines, vec![3]);
}

#[test]
fn a_literal_query_is_not_a_pattern_unless_regex_is_on() {
    let dir = one_file("a.b\naxb\n");
    assert_eq!(hits(&dir, "a.b", SearchOptions::default()).len(), 1);
    let regex = SearchOptions { regex: true, ..SearchOptions::default() };
    assert_eq!(hits(&dir, "a.b", regex).len(), 2);
}

#[test]
fn an_invalid_regex_is_an_error_with_a_short_message() {
    let dir = one_file("x\n");
    let regex = SearchOptions { regex: true, ..SearchOptions::default() };
    let error = LocalFiles::new().search(dir.path(), "(", regex, 10).unwrap_err();
    match error {
        FilesError::InvalidPattern(message) => assert!(!message.contains('\n') && !message.is_empty(), "{message}"),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_pattern_that_matches_nothing_in_a_line_gives_no_hit_and_an_empty_match_is_skipped() {
    let dir = one_file("abc\n\n");
    let regex = SearchOptions { regex: true, ..SearchOptions::default() };
    // `x*` matches the empty string everywhere: no line is a hit.
    assert_eq!(hits(&dir, "x*", regex), vec![]);
    assert_eq!(hits(&dir, "b", regex).len(), 1);
}
```

Add `SearchOptions` to the test module's `use forge_ports::{…}`.

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p forge-adapter-files 2>&1 | tail -20` → compile errors (`SearchOptions` missing, `search` arity).

- [ ] **Step 3: Implement.**

`forge-ports/src/files_port.rs`: add

```rust
/// How a content search reads its query.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SearchOptions {
    pub match_case: bool,
    pub whole_word: bool,
    /// The query is a regular expression instead of plain text.
    pub regex: bool,
}
```

`ContentHit` gains `/// Char offsets of the first match inside `text`.` `pub start: usize, pub end: usize`. `FilesError` gains `#[error("{0}")] InvalidPattern(String),`. The trait method becomes `fn search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError>;` (doc: "One hit per line holding a match of `query`, at most `limit`, in path order."). Export `SearchOptions` from `forge-ports/src/lib.rs` next to `ContentHit`.

`forge-adapter-files/Cargo.toml`: `regex = "1"` under `[dependencies]`.

`forge-adapter-files/src/lib.rs`: import `regex::{Regex, RegexBuilder}` and `SearchOptions`; replace `search`, `search_dir`:

```rust
fn search(&self, root: &Path, query: &str, options: SearchOptions, limit: usize) -> Result<Vec<ContentHit>, FilesError> {
    let root = root.canonicalize().map_err(failed)?;
    let mut hits = Vec::new();
    if query.is_empty() || limit == 0 {
        return Ok(hits);
    }
    let matcher = build_matcher(query, options)?;
    let mut visited = 0;
    search_dir(&root, "", &matcher, limit, &mut visited, &mut hits);
    Ok(hits)
}
```

```rust
fn build_matcher(query: &str, options: SearchOptions) -> Result<Regex, FilesError> {
    let body = if options.regex { query.to_string() } else { regex::escape(query) };
    let body = if options.whole_word { format!(r"\b(?:{body})\b") } else { body };
    RegexBuilder::new(&body).case_insensitive(!options.match_case).build().map_err(|error| {
        let text = error.to_string();
        let last = text.lines().last().unwrap_or("").trim().trim_start_matches("error:").trim();
        FilesError::InvalidPattern(if last.is_empty() { "Invalid pattern".to_string() } else { last.to_string() })
    })
}

// The first non-empty match of `matcher` in the trimmed line, as a window of at most
// HIT_TEXT_CHARS chars with `…` at a cut end, and the match's char offsets inside that window.
fn hit_of(line: &str, matcher: &Regex) -> Option<(String, usize, usize)> {
    let line = line.trim();
    let found = matcher.find_iter(line).find(|m| !m.is_empty())?;
    let start = line[..found.start()].chars().count();
    let end = start + line[found.start()..found.end()].chars().count();
    let chars: Vec<char> = line.chars().collect();
    if chars.len() <= HIT_TEXT_CHARS {
        return Some((line.to_string(), start, end));
    }
    let room = HIT_TEXT_CHARS - 2;
    let length = (end - start).min(room);
    let from = start.saturating_sub((room - length) / 2).min(chars.len() - room);
    let to = from + room;
    let mut text = String::new();
    let lead = usize::from(from > 0);
    if from > 0 {
        text.push('…');
    }
    text.extend(&chars[from..to]);
    if to < chars.len() {
        text.push('…');
    }
    Some((text, start - from + lead, end.min(to) - from + lead))
}
```

In `search_dir` change the `needle: &str` parameter to `matcher: &Regex` and the per-line loop to:

```rust
for (index, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
    if let Some((text, start, end)) = hit_of(line, matcher) {
        hits.push(ContentHit { path: path.clone(), line: index + 1, text, start, end });
        if hits.len() >= limit {
            return;
        }
    }
}
```

and the recursive call passes `matcher`. (`start >= from` always holds because `from <= start`; if the match is longer than `room`, `end.min(to)` clamps it.)

- [ ] **Step 4: Run** — `cargo test -p forge-adapter-files -p forge-ports 2>&1 | grep -E "test result|FAILED|^error"` → all pass. Then `cargo build 2>&1 | grep -E "^error" -A6` — `files_panel.rs` fails to compile (`search` arity, `ContentHit` literals if any): fix in Task 4, so for now make the panel compile with a one-line stopgap: pass `forge_ports::SearchOptions::default()` as the third argument in `files_panel.rs`'s `files.search(...)` call.

- [ ] **Step 5: Commit** — `feat: content search takes options and returns match offsets (Plan 25C-1)`.

---

### Task 2: Grouping hits and splitting a hit's text

**Files:**
- Create: `crates/forge-features/src/hit_list.rs`
- Modify: `crates/forge-features/src/lib.rs`

**Interfaces:**
- Consumes: `forge_ports::ContentHit`.
- Produces: `HitGroup { path: String, hits: Vec<ContentHit> }`, `group_hits(&[ContentHit]) -> Vec<HitGroup>`, `split_hit(&ContentHit) -> (String, String, String)` (before, match, after).

- [ ] **Step 1: Failing tests** (`hit_list.rs` with only the tests module, plus `mod hit_list;` and `pub use hit_list::{group_hits, split_hit, HitGroup};` in `lib.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, line: usize, text: &str, start: usize, end: usize) -> ContentHit {
        ContentHit { path: path.into(), line, text: text.into(), start, end }
    }

    #[test]
    fn hits_are_grouped_by_path_in_the_order_they_arrive() {
        let hits = vec![hit("a.py", 1, "x", 0, 1), hit("a.py", 5, "y", 0, 1), hit("b/c.py", 2, "z", 0, 1)];
        let groups = group_hits(&hits);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].path, "a.py");
        assert_eq!(groups[0].hits.iter().map(|h| h.line).collect::<Vec<_>>(), vec![1, 5]);
        assert_eq!(groups[1].path, "b/c.py");
    }

    #[test]
    fn no_hits_no_groups() {
        assert_eq!(group_hits(&[]), vec![]);
    }

    #[test]
    fn a_hit_splits_at_char_offsets_including_hangul() {
        let (before, matched, after) = split_hit(&hit("a", 1, "한글 hello 끝", 3, 8));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("한글 ", "hello", " 끝"));
    }

    #[test]
    fn offsets_past_the_text_are_clamped_not_a_panic() {
        let (before, matched, after) = split_hit(&hit("a", 1, "abc", 2, 99));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("ab", "c", ""));
        let (before, matched, after) = split_hit(&hit("a", 1, "abc", 9, 12));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("abc", "", ""));
    }
}
```

- [ ] **Step 2: Run** — `cargo test -p forge-features hit_list 2>&1 | grep -E "^error" | head -3` → FAIL (missing items).

- [ ] **Step 3: Implement** (above the tests):

```rust
use forge_ports::ContentHit;

/// The hits of one file, in the order the search found them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitGroup {
    pub path: String,
    pub hits: Vec<ContentHit>,
}

/// Groups hits by file. The search returns them in path order, so a group is a run of one path.
pub fn group_hits(hits: &[ContentHit]) -> Vec<HitGroup> {
    let mut groups: Vec<HitGroup> = Vec::new();
    for hit in hits {
        match groups.last_mut() {
            Some(group) if group.path == hit.path => group.hits.push(hit.clone()),
            _ => groups.push(HitGroup { path: hit.path.clone(), hits: vec![hit.clone()] }),
        }
    }
    groups
}

/// The text before, inside and after the match (char offsets, clamped to the text).
pub fn split_hit(hit: &ContentHit) -> (String, String, String) {
    let chars: Vec<char> = hit.text.chars().collect();
    let start = hit.start.min(chars.len());
    let end = hit.end.clamp(start, chars.len());
    (chars[..start].iter().collect(), chars[start..end].iter().collect(), chars[end..].iter().collect())
}
```

- [ ] **Step 4: Run** — `cargo test -p forge-features hit_list` → pass.

- [ ] **Step 5: Commit** — `feat: group search hits by file and split a hit around its match (Plan 25C-1)`.

---

### Task 3: The in-file find bar in the editor bundle

**Files:**
- Create: `tools/editor-bundle/src/find.js`
- Modify: `tools/editor-bundle/src/main.js`, `tools/editor-bundle/test/bundle.test.mjs`, rebuilt `crates/forge-shared/assets/editor.js`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Produces (Rust to page): `find(key, { query, match_case, whole_word, regex, line })`.
- `find.js` exports `findTools({ remember })` returning `{ extensions, open(view, opts), close(view), restore(view, state) }`; `findMatches(text, { query, match_case, whole_word, regex }) -> { matches: [{from, to}], invalid: boolean }` (pure, exported for the tests).

- [ ] **Step 1: Failing tests** — append to `bundle.test.mjs` (reuse `boot`, `plain`, `mouse`, `viewOf`; `find` is queued only through the page, so call `editor.find` directly):

```js
const ALL = { match_case: false, whole_word: false, regex: false };
const count = (w) => w.document.querySelector(".cm-find-count")?.textContent;

test("find opens the bar on the line, marks every match and counts them", () => {
  const { w, editor } = boot();
  editor.load("a.py", "fizz\nbuzz\nBuzz\n", "python", false);
  editor.find("a.py", { query: "buzz", ...ALL, line: 3 });
  assert.ok(w.document.querySelector(".cm-find-bar"));
  assert.equal(w.document.querySelectorAll(".cm-find-match").length, 2);
  assert.equal(count(w), "2 of 2");
  editor.destroy();
});

test("next and previous wrap around", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\na\n", "python", false);
  editor.find("a.py", { query: "a", ...ALL, line: 1 });
  assert.equal(count(w), "1 of 3");
  w.document.querySelector(".cm-find-prev").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "3 of 3");
  w.document.querySelector(".cm-find-next").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 3");
  editor.destroy();
});

test("typing in the field and the toggles change the matches", () => {
  const { w, editor } = boot();
  editor.load("a.py", "Buzz\nbuzz\nbuzzer\n", "python", false);
  editor.find("a.py", { query: "buzz", ...ALL, line: 1 });
  assert.equal(count(w), "1 of 3");
  w.document.querySelector("[data-opt=match_case]").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 2");
  w.document.querySelector("[data-opt=whole_word]").dispatchEvent(mouse(w, "mousedown"));
  assert.equal(count(w), "1 of 1");
  const input = w.document.querySelector(".cm-find-bar input");
  input.value = "zzz";
  input.dispatchEvent(new w.Event("input", { bubbles: true }));
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("an invalid regex marks the field and finds nothing", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a(b\n", "python", false);
  editor.find("a.py", { query: "(", ...ALL, regex: true, line: 1 });
  assert.ok(w.document.querySelector(".cm-find-bar input.invalid"));
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("a pattern that can match nothing does not flood or hang", () => {
  const { w, editor } = boot();
  editor.load("a.py", "abc\n", "python", false);
  editor.find("a.py", { query: "x*", ...ALL, regex: true, line: 1 });
  assert.equal(count(w), "No results");
  editor.destroy();
});

test("whole word works for Hangul", () => {
  const { w, editor } = boot();
  editor.load("a.py", "가나 다\n가나다\n", "python", false);
  editor.find("a.py", { query: "가나", ...ALL, whole_word: true, line: 1 });
  assert.equal(count(w), "1 of 1");
  editor.destroy();
});

test("Escape closes the bar and Enter during an IME composition does not move", () => {
  const { w, editor } = boot();
  editor.load("a.py", "a\na\n", "python", false);
  editor.find("a.py", { query: "a", ...ALL, line: 1 });
  const input = w.document.querySelector(".cm-find-bar input");
  input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Enter", isComposing: true, bubbles: true }));
  assert.equal(count(w), "1 of 2");
  input.dispatchEvent(new w.KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  assert.equal(w.document.querySelector(".cm-find-bar"), null);
  assert.equal(w.document.querySelectorAll(".cm-find-match").length, 0);
  editor.destroy();
});

test("a find for a tab that is not shown yet is applied when it shows, and the bar follows tab switches", () => {
  const { w, editor } = boot();
  editor.load("a.py", "one\n", "python", false);
  editor.load("b.py", "two\nneedle\n", "python", false);
  editor.show("a.py");
  editor.find("b.py", { query: "needle", ...ALL, line: 2 });
  assert.equal(w.document.querySelector(".cm-find-bar"), null);
  editor.show("b.py");
  assert.equal(count(w), "1 of 1");
  editor.show("a.py");
  // The bar is the editor's, not the tab's: it stays open with the same query.
  assert.equal(w.document.querySelector(".cm-find-bar input").value, "needle");
  editor.destroy();
});
```

Run `cd tools/editor-bundle && npm run build && npm test 2>&1 | grep -E "^# (pass|fail)|^not ok"` → the eight new tests FAIL.

- [ ] **Step 2: Implement `find.js`:**

```js
// The in-file find bar: a panel at the top of the editor with the query, three toggles, a count
// and next / previous, plus a mark on every match (the current one outlined).
//
// The state is the editor's, not a tab's (`remember` is told every change, and the editor puts it
// back after a tab switch). Matching is a JS RegExp with the `u` flag.
import { StateEffect, StateField } from "@codemirror/state";
import { Decoration, EditorView, keymap, showPanel } from "@codemirror/view";

const MAX_MATCHES = 20000;
const WORD_BEFORE = "(?<![\\p{L}\\p{N}_])";
const WORD_AFTER = "(?![\\p{L}\\p{N}_])";

const setFind = StateEffect.define();

export function findMatches(text, { query, match_case, whole_word, regex }) {
  if (query === "") return { matches: [], invalid: false };
  let body = regex ? query : query.replace(/[.*+?^${}()|[\]\\\/-]/g, "\\$&");
  if (whole_word) body = `${WORD_BEFORE}(?:${body})${WORD_AFTER}`;
  let re;
  try {
    re = new RegExp(body, match_case ? "gu" : "giu");
  } catch {
    return { matches: [], invalid: true };
  }
  const matches = [];
  for (let m = re.exec(text); m !== null && matches.length < MAX_MATCHES; m = re.exec(text)) {
    if (m[0] === "") {
      re.lastIndex += 1;
      continue;
    }
    matches.push({ from: m.index, to: m.index + m[0].length });
  }
  return { matches, invalid: false };
}

const empty = { open: false, query: "", match_case: false, whole_word: false, regex: false, current: 0 };

const findField = StateField.define({
  create: () => empty,
  update(value, tr) {
    for (const e of tr.effects) if (e.is(setFind)) return { ...value, ...e.value };
    return value;
  },
});

function result(state) {
  const f = state.field(findField);
  return f.open ? findMatches(state.doc.toString(), f) : { matches: [], invalid: false };
}

const markAll = Decoration.mark({ class: "cm-find-match" });
const markCurrent = Decoration.mark({ class: "cm-find-match cm-find-current" });

const decorations = EditorView.decorations.compute(["doc", findField], (state) => {
  const f = state.field(findField);
  if (!f.open) return Decoration.none;
  const { matches } = result(state);
  return Decoration.set(matches.map((m, i) => (i === f.current ? markCurrent : markAll).range(m.from, m.to)));
});

export function findTools({ remember }) {
  function apply(view, patch, { scroll = false } = {}) {
    view.dispatch({ effects: setFind.of(patch) });
    remember(view.state.field(findField));
    if (scroll) goTo(view);
  }

  function goTo(view) {
    const f = view.state.field(findField);
    const { matches } = result(view.state);
    const m = matches[f.current];
    if (!m) return;
    view.dispatch({ selection: { anchor: m.from }, effects: EditorView.scrollIntoView(m.from, { y: "center" }) });
  }

  // The first match at or after `pos`, wrapping to the first one.
  function indexAt(matches, pos) {
    const i = matches.findIndex((m) => m.from >= pos);
    return i === -1 ? 0 : i;
  }

  function step(view, delta) {
    const { matches } = result(view.state);
    if (matches.length === 0) return;
    const f = view.state.field(findField);
    apply(view, { current: (f.current + delta + matches.length) % matches.length }, { scroll: true });
  }

  function makePanel(view) {
    const dom = document.createElement("div");
    dom.className = "cm-find-bar";
    const input = document.createElement("input");
    input.type = "text";
    input.placeholder = "Find";
    const toggles = [];
    for (const [opt, label, title] of [["match_case", "Aa", "Match case"], ["whole_word", "ab", "Whole word"], ["regex", ".*", "Regex"]]) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.opt = opt;
      button.title = title;
      button.textContent = label;
      button.addEventListener("mousedown", (event) => {
        event.preventDefault();
        const f = view.state.field(findField);
        apply(view, { [opt]: !f[opt], current: 0 });
        jumpToCurrent(view);
      });
      toggles.push(button);
    }
    const countEl = document.createElement("span");
    countEl.className = "cm-find-count";
    const mk = (cls, text, delta) => {
      const button = document.createElement("button");
      button.type = "button";
      button.className = cls;
      button.textContent = text;
      button.addEventListener("mousedown", (event) => { event.preventDefault(); step(view, delta); });
      return button;
    };
    const close = document.createElement("button");
    close.type = "button";
    close.className = "cm-find-close";
    close.textContent = "×";
    close.addEventListener("mousedown", (event) => { event.preventDefault(); closeBar(view); });

    input.addEventListener("input", () => {
      apply(view, { query: input.value, current: 0 });
      jumpToCurrent(view);
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.isComposing) {
        event.preventDefault();
        step(view, event.shiftKey ? -1 : 1);
      } else if (event.key === "Escape") {
        event.preventDefault();
        closeBar(view);
      }
    });

    dom.append(input, ...toggles, countEl, mk("cm-find-prev", "↑", -1), mk("cm-find-next", "↓", 1), close);

    function refresh(state) {
      const f = state.field(findField);
      const { matches, invalid } = result(state);
      if (document.activeElement !== input) input.value = f.query;
      input.classList.toggle("invalid", invalid);
      for (const button of toggles) {
        const on = f[button.dataset.opt];
        button.classList.toggle("on", on);
        button.setAttribute("aria-pressed", String(on));
      }
      countEl.textContent = f.query === "" ? "" : matches.length === 0 ? "No results" : `${Math.min(f.current, matches.length - 1) + 1} of ${matches.length}`;
    }
    refresh(view.state);
    return { dom, top: true, mount: () => input.focus(), update: (update) => refresh(update.state) };
  }

  // After the query or an option changed, the current match is the first one at or after the caret.
  function jumpToCurrent(view) {
    const { matches } = result(view.state);
    if (matches.length === 0) return;
    const index = indexAt(matches, view.state.selection.main.head);
    apply(view, { current: index }, { scroll: true });
  }

  function closeBar(view) {
    apply(view, { open: false });
    view.focus();
  }

  const panel = showPanel.compute([findField], (state) => (state.field(findField).open ? makePanel : null));

  const keys = keymap.of([
    {
      key: "Mod-f",
      run(view) {
        const sel = view.state.sliceDoc(view.state.selection.main.from, view.state.selection.main.to);
        const patch = { open: true };
        if (sel !== "" && !sel.includes("\n")) patch.query = sel;
        apply(view, patch);
        jumpToCurrent(view);
        view.dom.querySelector(".cm-find-bar input")?.select();
        return true;
      },
    },
  ]);

  return {
    extensions: [findField, decorations, panel, keys],
    // `line` is 1 based. The first match on that line, else the next one after it, is current.
    open(view, { line, ...options }) {
      apply(view, { open: true, ...options, current: 0 });
      const { matches } = result(view.state);
      if (matches.length === 0) return;
      const at = view.state.doc.line(Math.min(Math.max(line || 1, 1), view.state.doc.lines)).from;
      apply(view, { current: indexAt(matches, at) }, { scroll: true });
    },
    // Puts the editor's remembered state into a state that was just built (a tab switch).
    restore(view, state) {
      if (state && state.open) apply(view, state);
      else if (view.state.field(findField).open) apply(view, { open: false });
    },
  };
}
```

- [ ] **Step 3: Wire into `main.js`.** `import { findTools } from "./find.js";`. In `create`: `let findState = null; const find = findTools({ remember: (state) => { findState = state; } });` and `const pendingFind = new Map();`. Add `...find.extensions` to `makeState`'s extension list. A helper `function afterSwitch(newKey) { find.restore(view, findState); const pending = pendingFind.get(newKey); if (pending) { pendingFind.delete(newKey); find.open(view, pending); } }` called in `load` and `show` after `tools.setComments(view, …)`/`applyPendingFocus(newKey)` (the same two places). Add the method:

```js
find(forKey, opts) {
  // A tab that is not shown (or not loaded) yet takes the request when it shows.
  if (forKey === key) find.open(view, opts);
  else pendingFind.set(forKey, opts);
},
```

and `pendingFind.delete(oldKey)` in `drop`. Update the header comment (Rust to page: `find(key, { query, match_case, whole_word, regex, line })`).

- [ ] **Step 4: CSS** in `theme.css`:

```css
.cm-find-bar { display: flex; align-items: center; gap: 4px; padding: 4px 8px; background: var(--bg); border-bottom: 1px solid var(--border); font-size: 12px; }
.cm-find-bar input { flex: 1; min-width: 0; padding: 3px 6px; border: 1px solid var(--border); border-radius: 6px; background: transparent; color: var(--fg); font-size: 12px; outline: none; }
.cm-find-bar input.invalid { border-color: #d44; }
.cm-find-bar button { border: none; background: transparent; color: var(--fg-muted); padding: 2px 6px; border-radius: 6px; cursor: pointer; font-size: 12px; }
.cm-find-bar button:hover { background: var(--bg-hover); }
.cm-find-bar button.on { background: var(--accent-soft); color: var(--fg); }
.cm-find-count { color: var(--fg-muted); white-space: nowrap; min-width: 4.5em; text-align: right; }
.cm-find-match { background-color: rgba(250, 204, 21, .35); }
.cm-find-current { outline: 2px solid #22a45d; border-radius: 2px; }
```

- [ ] **Step 5: Run** — `npm run build && npm test` → all pass (15 old + 8 new). If the jsdom `showPanel` does not render because the editor has no layout, the panel is still DOM; if a test fails for jsdom reasons only (selection or scroll), adjust the test, never the behaviour, and say so in the commit.

- [ ] **Step 6: Commit** (include the rebuilt `editor.js`) — `feat: an in-file find bar with toggles, count and next/previous (Plan 25C-1)`.

---

### Task 4: Toggles, grouped results, and the click that opens the bar

**Files:**
- Modify: `crates/forge-widgets/src/files_editor.rs`, `crates/forge-widgets/src/files_state.rs`, `crates/forge-widgets/src/files_panel.rs`, `crates/forge-shared/src/theme.css`

**Interfaces:**
- Consumes: Task 1 `SearchOptions`/`ContentHit`/`FilesError::InvalidPattern`; Task 2 `group_hits`, `split_hit`; Task 3 page `find`.
- Produces: `find_command(key, query, options, line) -> Value`, `EditorHandle::find(key, query, options, line)`, `FilesCtx::find_in_file(&self, path: &str, query: &str, options: SearchOptions, line: usize)`.

- [ ] **Step 1: Failing test** in `files_editor.rs` tests:

```rust
#[test]
fn the_find_command_carries_the_query_options_and_line() {
    let options = forge_ports::SearchOptions { match_case: true, whole_word: false, regex: true };
    assert_eq!(
        find_command("a.py", "Bu.z", options, 7),
        serde_json::json!({ "t": "find", "key": "a.py", "query": "Bu.z", "match_case": true, "whole_word": false, "regex": true, "line": 7 })
    );
}
```

Run `cargo test -p forge-widgets find_command` → FAIL.

- [ ] **Step 2: Implement the bridge.** In `files_editor.rs`:

```rust
/// Opens the find bar of a tab on a line (1 based) with a query and its options.
pub fn find_command(key: &str, query: &str, options: forge_ports::SearchOptions, line: usize) -> Value {
    json!({
        "t": "find", "key": key, "query": query,
        "match_case": options.match_case, "whole_word": options.whole_word, "regex": options.regex,
        "line": line,
    })
}
```

`EditorHandle::find(&self, key, query, options, line)` sends it; in `BRIDGE_JS` add `else if (m.t === "find") editor.find(m.key, { query: m.query, match_case: m.match_case, whole_word: m.whole_word, regex: m.regex, line: m.line });`. In `files_state.rs`:

```rust
/// A hit was clicked: show the file, then open the find bar on the line. The command waits behind
/// the load in the page's queue; a tab that is not loaded yet keeps it until it shows.
pub fn find_in_file(&self, path: &str, query: &str, options: forge_ports::SearchOptions, line: usize) {
    self.open(path.to_string());
    self.handle.find(path, query, options, line);
}
```

(`forge-ports` is already a dependency of `forge-widgets`.)

- [ ] **Step 3: Panel.** In `files_panel.rs`:
  - `let mut options = use_signal(SearchOptions::default);` and change `hits` to `use_signal(|| None::<Result<Vec<ContentHit>, String>>)`.
  - The search effect reads `options()` as well as the filter, calls `files.search(&root, &query, opts, SEARCH_LIMIT)` and stores `Some(Ok(found))` or `Some(Err(error.to_string()))` (an `InvalidPattern` message is shown as is; any other error too). Replace the Task 1 stopgap.
  - Toggle row inside `.files-filter`, shown only when `parse_files_query(&typed)` is `Content(_)`:

```rust
if matches!(query, FilesQuery::Content(_)) {
    div { class: "files-search-toggles",
        for (title, label, on, flip) in [
            ("Match case", "Aa", options().match_case, 0u8),
            ("Whole word", "ab", options().whole_word, 1),
            ("Regex", ".*", options().regex, 2),
        ] {
            button {
                key: "{title}",
                class: if on { "files-toggle on" } else { "files-toggle" },
                r#type: "button",
                title: "{title}",
                "aria-pressed": "{on}",
                onclick: move |_| options.with_mut(|o| match flip {
                    0 => o.match_case = !o.match_case,
                    1 => o.whole_word = !o.whole_word,
                    _ => o.regex = !o.regex,
                }),
                "{label}"
            }
        }
        button { class: "files-toggle", r#type: "button", title: "Clear", "aria-label": "Clear", onclick: move |_| filter.set(String::new()), "×" }
    }
}
```

  (the `query` binding already exists later in the component; move `let query = parse_files_query(&typed);` above the `rsx!` use or compute `is_content` once.)
  - The list for `FilesQuery::Content`:

```rust
Some(Err(message)) => rsx! { div { class: "dock-note error", "{message}" } },
Some(Ok(found)) if found.is_empty() => rsx! { div { class: "dock-note", "No matches" } },
Some(Ok(found)) => rsx! {
    div { class: "files-list",
        for group in group_hits(&found) {
            div { key: "{group.path}", class: "files-hit-group",
                div { class: "files-hit-file", "{group.path}" }
                for hit in group.hits.iter() {
                    {
                        let (before, matched, after) = split_hit(hit);
                        let (ctx, path, line, needle, opts) = (ctx.clone(), hit.path.clone(), hit.line, text.to_string(), options());
                        rsx! {
                            button {
                                key: "{hit.line}",
                                class: "files-hit",
                                r#type: "button",
                                onclick: move |_| ctx.find_in_file(&path, &needle, opts, line),
                                span { class: "files-hit-line", "{hit.line}" }
                                span { class: "files-hit-text", "{before}", mark { "{matched}" }, "{after}" }
                            }
                        }
                    }
                }
            }
        }
    }
},
```

  Keep the existing `Searching…` (`None`) and empty-query arms.
  - Import `group_hits`, `split_hit` from `forge_features` and `SearchOptions` from `forge_ports`.

- [ ] **Step 4: CSS:**

```css
.files-search-toggles { display: flex; gap: 2px; align-items: center; margin-top: 4px; }
.files-toggle { border: none; background: transparent; color: var(--fg-muted); padding: 1px 6px; border-radius: 6px; font-size: 12px; cursor: pointer; }
.files-toggle:hover { background: var(--bg-hover); }
.files-toggle.on { background: var(--accent-soft); color: var(--fg); }
.files-hit-group { margin-bottom: 6px; }
.files-hit-file { padding: 2px 6px; font-size: 12px; font-weight: 600; color: var(--fg); }
.files-hit-line { flex: none; min-width: 3ch; text-align: right; color: var(--fg-muted); }
.files-hit-text mark { background: var(--accent-soft); color: inherit; border-radius: 2px; }
```

- [ ] **Step 5: Run** — `cargo test --workspace 2>&1 | grep "test result" | awk '{s+=$4; f+=$6} END {print s" passed, "f" failed"}'` → 0 failed; `cargo build` clean (the existing `files-hit` rows may already be styled as flex; if the new two spans look wrong in Task 5, fix the CSS there).

- [ ] **Step 6: Commit** — `feat: search toggles, grouped hits with the match marked, and a click that opens the find bar (Plan 25C-1)`.

---

### Task 5: Check in the real window, fix, write down what was found

**Files:**
- Modify: `docs/superpowers/specs/2026-10-05-files-search-design.md` (As built + results), `docs/superpowers/roadmap/*.md` if it tracks PNL-04

- [ ] **Step 1: Whole run** — `cargo test --workspace` and `cd tools/editor-bundle && npm test` green; record the counts.

- [ ] **Step 2: Drive the app** with the harness (memory `driving-the-forge-app-visually`; the screen must be unlocked; `scripts/harness/launch.sh`, open a session, the Files panel from the session `⋮` menu, which toggles; create a few files in `/private/tmp/forge-harness/my-app` first, e.g. `fizz.py` with `Buzz`/`buzz` lines and a Hangul file; `capture.py`; `click.py` takes pixels of the 2400 wide capture, viewer coordinates times 1.2; a right click or drag script is at `…/scratchpad/ctl.py` if it still exists). Check, one line each:
  1. `?buzz` shows the three toggles and `×`; hits grouped by file with line numbers and the match marked.
  2. `Aa`, `ab`, `.*` each change the list (case, word, regex); an invalid regex `(` shows the note.
  3. A Hangul query with `ab` matches the word only.
  4. A click on a hit opens the file, the caret is on that line and the find bar shows the query, the options and `N of M`; matches shaded, the current one outlined green.
  5. In the bar: typing changes matches; Enter and Shift-Enter move; the arrows move; the toggles work; `×` and Escape close it; an invalid regex turns the field red.
  6. `⌘F` in the editor opens the bar with the selected word; the bar stays when another tab is selected.
  7. A very long line hit shows `…` around the match.
  8. Dark mode: marks, outline and toggles readable (look if the harness can; otherwise note not checked).
  Each defect found: failing test if one can exist, fix, commit `fix: …`.

- [ ] **Step 3: Docs** — add "As built" and "Checked in the real window" sections to the spec (what differs from it; what was seen and not seen); update the roadmap status for PNL-04 if tracked (`grep -n "PNL-04" docs/superpowers/roadmap/*.md`).

- [ ] **Step 4: Commit** — `docs: Plan 25C-1 checked in the real window`.

- [ ] **Step 5: Finish** — invoke `superpowers:finishing-a-development-branch`; the base is `master`. Do not merge without the user's word.
