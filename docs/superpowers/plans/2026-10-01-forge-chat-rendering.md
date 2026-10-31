# Forge Chat Rendering Implementation Plan (Plan 8)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Assistant messages render as markdown with syntax-highlighted code blocks, and Edit / MultiEdit / Write tool calls expand into a diff, both in the chat and in the permission card.

**Architecture:** Everything is presentation and lives in `forge-entities`. Three pure modules — `highlight` (syntect, class-based HTML), `markdown` (pulldown-cmark → HTML with raw HTML escaped and unsafe links/images dropped) and `diff` (similar → `FileDiff`) — are unit-tested without rendering. Two small components, `DiffView` and the expandable `ToolCallLine`, plus `MessageBubble` and `PermissionCard`, consume them. Colors come from CSS variables in `forge-shared/src/theme.css` with a light and a dark palette.

**Tech Stack:** Rust 2021, Dioxus 0.7.10 desktop, `pulldown-cmark` 0.13, `syntect` 5 (pure-Rust regex backend), `similar` 2.

**Spec:** `docs/superpowers/specs/2026-09-27-forge-design.md` §18 (§16.6 for the theme, §7 for the Tier 1 list).

## Global Constraints

- Crate graph: `forge-entities` depends on `forge-domain` and `forge-shared` only. No port, adapter or use-case is touched by this plan.
- Markdown is rendered for the **assistant** role only; user messages stay plain text (spec §18.1).
- Raw HTML from the model is never emitted as markup — it is escaped as text (spec §18.2). Only `http`, `https` and `mailto` links keep an `href`; any other link, and every image, is rendered as its text only.
- `syntect` is used with `default-features = false, features = ["parsing", "default-syntaxes", "html", "regex-fancy"]`: pure Rust (`fancy-regex`), no `onig`, no `*-sys` crate in the build (spec §18.3). Classes carry the prefix `hl-`.
- No line numbers in diffs; the `Write` diff header says "new content" and never claims whether the file existed (spec §18.4).
- Colors use CSS variables with both palettes; nothing is hard-coded outside `:root` and the dark `@media` block (spec §16.6).
- UI text is English.
- Pinned: Dioxus `=0.7.10`. Dependencies are added as written in Task 1, no wider ranges.
- Every commit message ends with `Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>` (pass it as a second `-m`).
- Test commands: `cargo test -p forge-entities`; full check `cargo test --workspace` (baseline before this plan: run it once in Task 1 and note the count; the `block v0.1.6` future-incompat notice is expected).

## Review Focus

Inputs the spec implies but that are easy to get wrong, most likely first. Each has a test in the task named.

1. Model output containing raw HTML (`<script>`, `<img onerror=…>`, `<b>` inline) → shown as visible text, never as elements (Task 2).
2. A link or image with a hostile or remote target (`javascript:`, `data:`, `![](http://tracker/p.png)`) → no `<a>` / `<img>` is produced; the visible text stays (Task 2).
3. A code fence with a hostile info string (`` ```"><script> ``) → no attribute injection (Task 2).
4. An unterminated code fence while streaming, and an unknown or missing language → the text is shown escaped and nothing is lost (Task 2).
5. A huge code block (hundreds of KB) → not highlighted, so the window does not freeze; and a huge `Write` (thousands of lines) → the diff shows at most 300 lines plus "… N more lines" (Tasks 2 and 3).
6. Tool input that is not the expected shape (`Edit` without `new_string`, `file_path` that is not a string, `MultiEdit` with a non-array) → no diff, the collapsed line still works (Task 3).
7. Empty message content and an empty `Write` → no empty markdown wrapper, no crash (Tasks 2, 3, 5).

---

## File Structure

| File | Change |
|---|---|
| `crates/forge-entities/Cargo.toml` | dependencies `pulldown-cmark`, `syntect`, `similar` |
| `crates/forge-entities/src/highlight.rs` | **new** `highlight_code`, `escape_html` |
| `crates/forge-entities/src/markdown.rs` | **new** `render_markdown` |
| `crates/forge-entities/src/diff.rs` | **new** `FileDiff`, `DiffLine`, `DiffLineKind`, `diff_for_tool`, `diff_for_tool_call` |
| `crates/forge-entities/src/diff_view.rs` | **new** `DiffView` component and its pure helpers |
| `crates/forge-entities/src/message.rs` | markdown in `MessageBubble`, expandable `ToolCallLine` |
| `crates/forge-entities/src/permission.rs` | diff inside `PermissionCard` |
| `crates/forge-entities/src/lib.rs` | modules and exports |
| `crates/forge-shared/src/theme.css` | markdown, `hl-*`, diff and expandable tool-call styles; both palettes |
| `crates/forge-shared/src/theme.rs` | extend the class-coverage test |
| `docs/superpowers/specs/2026-09-27-forge-design.md` | §18.7 "As built" |

---

### Task 1: Dependencies and `highlight_code`

**Files:**
- Modify: `crates/forge-entities/Cargo.toml`
- Create: `crates/forge-entities/src/highlight.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces: `pub fn highlight_code(language: &str, code: &str) -> Option<String>` — HTML-escaped code wrapped in `<span class="hl-…">` elements, or `None` when `language` names no known syntax. `pub fn escape_html(text: &str) -> String` — escapes `& < > " '`. Both are `pub(crate)`-used by `markdown.rs` (Task 2); export neither from the crate.

- [ ] **Step 1: Record the baseline and add the dependencies**

Run: `cargo test --workspace 2>&1 | grep -E "^test result" | awk '{s+=$4} END {print s}'` and note the number (expected 274 or more).

Edit `crates/forge-entities/Cargo.toml`, add under `[dependencies]`:

```toml
pulldown-cmark = { version = "0.13", default-features = false, features = ["html"] }
similar = "2"
syntect = { version = "5", default-features = false, features = ["parsing", "default-syntaxes", "html", "regex-fancy"] }
```

- [ ] **Step 2: Confirm the build has no C dependency**

Run: `cargo build -p forge-entities && cargo tree -p forge-entities -e normal | grep -iE "onig|-sys" ; echo "exit=$?"`
Expected: build succeeds; the grep prints nothing and `exit=1`. If `onig` or another `-sys` crate appears, stop: the feature list is wrong.

- [ ] **Step 3: Write the failing tests**

Create `crates/forge-entities/src/highlight.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_known_language_is_wrapped_in_hl_classes() {
        let html = highlight_code("rust", "fn main() {}\n").unwrap();
        assert!(html.contains("<span class=\"hl-"), "{html}");
        assert!(html.contains("main"));
    }

    #[test]
    fn languages_are_found_by_name_and_by_extension_in_any_case() {
        assert!(highlight_code("python", "x = 1\n").is_some());
        assert!(highlight_code("py", "x = 1\n").is_some());
        assert!(highlight_code("Rust", "let a = 1;\n").is_some());
        assert!(highlight_code("sh", "ls\n").is_some());
    }

    #[test]
    fn an_unknown_language_gives_none() {
        assert_eq!(highlight_code("nosuchlang", "x"), None);
        assert_eq!(highlight_code("", "x"), None);
    }

    #[test]
    fn code_is_escaped_inside_the_spans() {
        let html = highlight_code("rust", "let a = \"<b>&</b>\";\n").unwrap();
        assert!(html.contains("&lt;b&gt;"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
    }

    #[test]
    fn escape_html_escapes_the_five_special_characters() {
        assert_eq!(escape_html("<a href=\"x\">&'</a>"), "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;");
    }
}
```

Add to `crates/forge-entities/src/lib.rs` (alphabetical, among the `mod` lines): `mod highlight;`

- [ ] **Step 4: Run the tests to see them fail**

Run: `cargo test -p forge-entities highlight`
Expected: compile error — `highlight_code` and `escape_html` not found.

- [ ] **Step 5: Implement**

Put above the `#[cfg(test)]` block in `highlight.rs`:

```rust
use std::sync::OnceLock;

use syntect::html::{ClassStyle, ClassedHTMLGenerator};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

const CLASS_STYLE: ClassStyle = ClassStyle::SpacedPrefixed { prefix: "hl-" };

// Loading the bundled syntaxes takes a moment, so it happens once, on the first code block.
fn syntaxes() -> &'static SyntaxSet {
    static SYNTAXES: OnceLock<SyntaxSet> = OnceLock::new();
    SYNTAXES.get_or_init(SyntaxSet::load_defaults_newlines)
}

/// The code as escaped HTML, wrapped in `<span class="hl-…">` elements for the syntax that
/// `language` names (a name or a file extension, any case). `None` when no syntax matches.
pub(crate) fn highlight_code(language: &str, code: &str) -> Option<String> {
    let set = syntaxes();
    let syntax = set.find_syntax_by_token(language)?;
    let mut generator = ClassedHTMLGenerator::new_with_class_style(syntax, set, CLASS_STYLE);
    for line in LinesWithEndings::from(code) {
        generator.parse_html_for_line_which_includes_newline(line).ok()?;
    }
    Some(generator.finalize())
}

pub(crate) fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other),
        }
    }
    escaped
}
```

Note: the bundled syntax set has no TypeScript or TOML; those fences render unhighlighted (Task 2 covers the fallback).

- [ ] **Step 6: Run the tests to see them pass**

Run: `cargo test -p forge-entities highlight`
Expected: 5 passed. Until Task 2 uses them, `cargo build` warns that `highlight_code` and `escape_html` are never used; that is expected and goes away in Task 2.

- [ ] **Step 7: Commit**

```bash
git add crates/forge-entities Cargo.lock
git commit -m "feat: highlight code with syntect class-based HTML" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 2: `render_markdown`

**Files:**
- Create: `crates/forge-entities/src/markdown.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `crate::highlight::{highlight_code, escape_html}` (Task 1).
- Produces: `pub fn render_markdown(source: &str) -> String`, exported from the crate root (Task 5 uses it).

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/markdown.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_markdown_becomes_html() {
        assert_eq!(render_markdown("**hi**"), "<p><strong>hi</strong></p>\n");
        assert!(render_markdown("- a\n- b").contains("<li>a</li>"));
        assert!(render_markdown("# Title").contains("<h1>Title</h1>"));
        assert!(render_markdown("`x`").contains("<code>x</code>"));
    }

    #[test]
    fn tables_and_strikethrough_are_supported() {
        assert!(render_markdown("|a|b|\n|-|-|\n|1|2|\n").contains("<table>"));
        assert!(render_markdown("~~gone~~").contains("<del>gone</del>"));
    }

    #[test]
    fn empty_input_gives_empty_output() {
        assert_eq!(render_markdown(""), "");
    }

    #[test]
    fn raw_block_html_is_shown_as_text() {
        let html = render_markdown("<script>alert(1)</script>");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(!html.contains("<script"), "{html}");
    }

    #[test]
    fn raw_inline_html_is_shown_as_text() {
        let html = render_markdown("a <img src=x onerror=alert(1)> b <b>c</b>");
        assert!(!html.contains("<img"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
        assert!(html.contains("&lt;b&gt;c&lt;/b&gt;"), "{html}");
    }

    #[test]
    fn safe_links_keep_their_href() {
        assert!(render_markdown("[x](https://a.com/p)").contains("<a href=\"https://a.com/p\">x</a>"));
        assert!(render_markdown("[x](http://a.com)").contains("href=\"http://a.com\""));
        assert!(render_markdown("[x](mailto:a@b.c)").contains("href=\"mailto:a@b.c\""));
    }

    #[test]
    fn other_link_schemes_are_dropped_but_the_text_stays() {
        for source in [
            "[click](javascript:alert(1))",
            "[click](JaVaScript:alert(1))",
            "[click](data:text/html,hi)",
            "[click](file:///etc/passwd)",
            "[click](#anchor)",
            "[click](/relative)",
        ] {
            let html = render_markdown(source);
            assert!(!html.contains("<a"), "{source} -> {html}");
            assert!(html.contains("click"), "{source} -> {html}");
        }
    }

    #[test]
    fn images_become_their_alt_text() {
        let html = render_markdown("![the alt](http://tracker.example/p.png)");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("the alt"), "{html}");
    }

    #[test]
    fn a_fence_with_a_known_language_is_highlighted() {
        let html = render_markdown("```rust\nfn main() {}\n```");
        assert!(html.starts_with("<pre class=\"code-block\"><code class=\"language-rust\">"), "{html}");
        assert!(html.contains("hl-"), "{html}");
        assert!(html.ends_with("</code></pre>\n"), "{html}");
    }

    #[test]
    fn a_fence_with_an_unknown_language_is_escaped_not_highlighted() {
        let html = render_markdown("```nosuchlang\na < b\n```");
        assert_eq!(
            html,
            "<pre class=\"code-block\"><code class=\"language-nosuchlang\">a &lt; b\n</code></pre>\n"
        );
    }

    #[test]
    fn a_fence_without_a_language_has_no_class() {
        let html = render_markdown("```\nplain\n```");
        assert_eq!(html, "<pre class=\"code-block\"><code>plain\n</code></pre>\n");
    }

    #[test]
    fn an_indented_code_block_is_a_plain_block() {
        let html = render_markdown("    indented <b>\n");
        assert!(html.contains("<pre class=\"code-block\"><code>indented &lt;b&gt;"), "{html}");
    }

    #[test]
    fn a_hostile_info_string_cannot_break_out_of_the_attribute() {
        let html = render_markdown("```\"><script>alert(1)</script>\ncode\n```");
        assert!(!html.contains("<script"), "{html}");
        assert!(html.contains("code"), "{html}");
    }

    #[test]
    fn an_unterminated_fence_while_streaming_shows_the_code_so_far() {
        let html = render_markdown("text\n\n```rust\nfn main() {");
        assert!(html.contains("<pre class=\"code-block\">"), "{html}");
        assert!(html.contains("main"), "{html}");
    }

    #[test]
    fn a_huge_code_block_is_not_highlighted() {
        let source = format!("```rust\n{}```", "let x = 1;\n".repeat(20_000));
        let html = render_markdown(&source);
        assert!(!html.contains("hl-"), "highlighting should be skipped above the size limit");
        assert!(html.contains("let x = 1;"));
    }
}
```

Add `mod markdown;` to `crates/forge-entities/src/lib.rs` and `pub use markdown::render_markdown;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-entities markdown`
Expected: compile error — `render_markdown` not found.

- [ ] **Step 3: Implement**

Put above the test module in `markdown.rs`:

```rust
use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::highlight::{escape_html, highlight_code};

// Above this many bytes a code block is shown escaped but not highlighted, so one enormous
// block cannot stall the window while the message re-renders.
const MAX_HIGHLIGHT_BYTES: usize = 100_000;

/// Assistant text as HTML. Raw HTML in the text is escaped, links keep an `href` only for
/// `http`, `https` and `mailto`, images become their alt text, and fenced code is highlighted
/// when its language is known.
pub fn render_markdown(source: &str) -> String {
    let parser = Parser::new_ext(source, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH);
    let mut events: Vec<Event> = Vec::new();
    let mut code: Option<(Option<String>, String)> = None;
    // One entry per open link or image: whether its start/end tags are kept.
    let mut kept: Vec<bool> = Vec::new();

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                code = Some((fence_language(&kind), String::new()));
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((language, text)) = code.take() {
                    events.push(Event::Html(code_block_html(language.as_deref(), &text).into()));
                }
            }
            Event::Text(text) if code.is_some() => {
                if let Some((_, buffer)) = code.as_mut() {
                    buffer.push_str(&text);
                }
            }
            Event::Html(raw) | Event::InlineHtml(raw) => events.push(Event::Text(raw)),
            Event::Start(Tag::Link { ref dest_url, .. }) => {
                let keep = is_safe_url(dest_url);
                kept.push(keep);
                if keep {
                    events.push(event);
                }
            }
            Event::Start(Tag::Image { .. }) => kept.push(false),
            Event::End(TagEnd::Link) => {
                if kept.pop().unwrap_or(false) {
                    events.push(event);
                }
            }
            Event::End(TagEnd::Image) => {
                kept.pop();
            }
            other => events.push(other),
        }
    }

    let mut output = String::new();
    html::push_html(&mut output, events.into_iter());
    output
}

fn is_safe_url(url: &str) -> bool {
    let lower = url.trim().to_ascii_lowercase();
    ["http://", "https://", "mailto:"].iter().any(|scheme| lower.starts_with(scheme))
}

// The first word of a fence's info string, when it is made only of characters that are safe in
// a class attribute.
fn fence_language(kind: &CodeBlockKind) -> Option<String> {
    let CodeBlockKind::Fenced(info) = kind else {
        return None;
    };
    let word = info.split_whitespace().next()?;
    let safe = word.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-' | '#' | '.'));
    safe.then(|| word.to_string())
}

fn code_block_html(language: Option<&str>, code: &str) -> String {
    let body = language
        .filter(|_| code.len() <= MAX_HIGHLIGHT_BYTES)
        .and_then(|language| highlight_code(language, code))
        .unwrap_or_else(|| escape_html(code));
    match language {
        Some(language) => format!("<pre class=\"code-block\"><code class=\"language-{language}\">{body}</code></pre>\n"),
        None => format!("<pre class=\"code-block\"><code>{body}</code></pre>\n"),
    }
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-entities markdown`
Expected: 15 passed. If `a_hostile_info_string…` shows a fence info of `"><script>alert(1)</script>` being split on whitespace, the first word `"><script>alert(1)</script>` fails the safe-character check, so no class is emitted and the code body is escaped — that is the expected path.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: render assistant markdown with escaped HTML and safe links" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Diff model — `diff_for_tool`

**Files:**
- Create: `crates/forge-entities/src/diff.rs`
- Modify: `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Produces:
  - `pub enum DiffLineKind { Context, Added, Removed }` (`Debug, Clone, Copy, PartialEq, Eq`)
  - `pub struct DiffLine { pub kind: DiffLineKind, pub text: String }`
  - `pub struct FileDiff { pub path: String, pub new_content: bool, pub added: usize, pub removed: usize, pub hunks: Vec<Vec<DiffLine>>, pub hidden_lines: usize }` (`Debug, Clone, PartialEq, Eq`)
  - `pub fn diff_for_tool(name: &str, input: &serde_json::Value) -> Option<FileDiff>`
  - `pub fn diff_for_tool_call(tool_call: &forge_domain::ToolCall) -> Option<FileDiff>`
  - all re-exported from the crate root.
- `added` and `removed` count every changed line before truncation; `hunks` holds at most `MAX_DIFF_LINES` (300) lines in total; `hidden_lines` is how many were cut.

- [ ] **Step 1: Write the failing tests**

Create `crates/forge-entities/src/diff.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kinds(diff: &FileDiff) -> Vec<(DiffLineKind, &str)> {
        diff.hunks.iter().flatten().map(|line| (line.kind, line.text.as_str())).collect()
    }

    #[test]
    fn an_edit_shows_context_removed_and_added_lines() {
        let input = json!({ "file_path": "src/a.rs", "old_string": "a\nb\nc\n", "new_string": "a\nB\nc\n" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!(diff.path, "src/a.rs");
        assert!(!diff.new_content);
        assert_eq!((diff.added, diff.removed, diff.hidden_lines), (1, 1, 0));
        assert_eq!(
            kinds(&diff),
            vec![
                (DiffLineKind::Context, "a"),
                (DiffLineKind::Removed, "b"),
                (DiffLineKind::Added, "B"),
                (DiffLineKind::Context, "c"),
            ]
        );
    }

    #[test]
    fn text_without_a_final_newline_and_crlf_lose_their_line_endings() {
        let input = json!({ "file_path": "f", "old_string": "x\r\ny", "new_string": "x\r\nz" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!(
            kinds(&diff),
            vec![(DiffLineKind::Context, "x"), (DiffLineKind::Removed, "y"), (DiffLineKind::Added, "z")]
        );
    }

    #[test]
    fn an_edit_with_an_empty_old_string_is_all_additions() {
        let input = json!({ "file_path": "f", "old_string": "", "new_string": "one\ntwo\n" });
        let diff = diff_for_tool("Edit", &input).unwrap();
        assert_eq!((diff.added, diff.removed), (2, 0));
    }

    #[test]
    fn a_multi_edit_has_one_hunk_per_edit_in_order() {
        let input = json!({
            "file_path": "f",
            "edits": [
                { "old_string": "a", "new_string": "b" },
                { "old_string": "c", "new_string": "d" }
            ]
        });
        let diff = diff_for_tool("MultiEdit", &input).unwrap();
        assert_eq!(diff.hunks.len(), 2);
        assert_eq!((diff.added, diff.removed), (2, 2));
        assert_eq!(diff.hunks[1][0].text, "c");
    }

    #[test]
    fn a_write_is_all_additions_and_marked_as_new_content() {
        let input = json!({ "file_path": "new.txt", "content": "l1\nl2\nl3" });
        let diff = diff_for_tool("Write", &input).unwrap();
        assert!(diff.new_content);
        assert_eq!((diff.added, diff.removed), (3, 0));
        assert!(kinds(&diff).iter().all(|(kind, _)| *kind == DiffLineKind::Added));
    }

    #[test]
    fn an_empty_write_has_a_diff_with_no_lines() {
        let diff = diff_for_tool("Write", &json!({ "file_path": "e", "content": "" })).unwrap();
        assert_eq!((diff.added, diff.removed, diff.hidden_lines), (0, 0, 0));
        assert!(diff.hunks.is_empty());
    }

    #[test]
    fn a_huge_write_is_cut_to_the_line_limit() {
        let content = "line\n".repeat(1000);
        let diff = diff_for_tool("Write", &json!({ "file_path": "big", "content": content })).unwrap();
        assert_eq!(diff.added, 1000);
        assert_eq!(diff.hunks.iter().map(Vec::len).sum::<usize>(), 300);
        assert_eq!(diff.hidden_lines, 700);
    }

    #[test]
    fn the_limit_is_shared_across_the_hunks_of_a_multi_edit() {
        let block = "x\n".repeat(200);
        let input = json!({
            "file_path": "f",
            "edits": [
                { "old_string": "", "new_string": block },
                { "old_string": "", "new_string": block }
            ]
        });
        let diff = diff_for_tool("MultiEdit", &input).unwrap();
        assert_eq!(diff.hunks.iter().map(Vec::len).sum::<usize>(), 300);
        assert_eq!(diff.hidden_lines, 100);
    }

    #[test]
    fn other_tools_have_no_diff() {
        assert_eq!(diff_for_tool("Bash", &json!({ "command": "ls" })), None);
        assert_eq!(diff_for_tool("Read", &json!({ "file_path": "f" })), None);
    }

    #[test]
    fn malformed_input_has_no_diff() {
        assert_eq!(diff_for_tool("Edit", &json!({ "file_path": "f", "old_string": "a" })), None);
        assert_eq!(diff_for_tool("Edit", &json!({ "old_string": "a", "new_string": "b" })), None);
        assert_eq!(diff_for_tool("Edit", &json!({ "file_path": 7, "old_string": "a", "new_string": "b" })), None);
        assert_eq!(diff_for_tool("Edit", &json!("not an object")), None);
        assert_eq!(diff_for_tool("MultiEdit", &json!({ "file_path": "f", "edits": "nope" })), None);
        assert_eq!(diff_for_tool("MultiEdit", &json!({ "file_path": "f", "edits": [{ "old_string": "a" }] })), None);
        assert_eq!(diff_for_tool("Write", &json!({ "file_path": "f" })), None);
    }

    #[test]
    fn the_tool_call_wrapper_reads_name_and_input() {
        let call = forge_domain::ToolCall {
            id: "t".to_string(),
            name: "Write".to_string(),
            input: json!({ "file_path": "f", "content": "x" }),
            result: None,
        };
        assert!(diff_for_tool_call(&call).is_some());
    }
}
```

In `lib.rs` add `mod diff;` and `pub use diff::{diff_for_tool, diff_for_tool_call, DiffLine, DiffLineKind, FileDiff};`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-entities diff::`
Expected: compile error — items not found.

- [ ] **Step 3: Implement**

Put above the test module in `diff.rs`:

```rust
use forge_domain::ToolCall;
use serde_json::{Map, Value};
use similar::{ChangeTag, TextDiff};

// A diff shows at most this many lines in total; the rest is summarised as "… N more lines".
const MAX_DIFF_LINES: usize = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub text: String,
}

/// What an Edit, MultiEdit or Write tool call changes. There are no line numbers: the tool
/// input does not say where in the file the change lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    /// A Write: the content is all there is to show; the input does not say what the file held before.
    pub new_content: bool,
    /// Changed lines before truncation.
    pub added: usize,
    pub removed: usize,
    /// One hunk per edit, at most `MAX_DIFF_LINES` lines in total.
    pub hunks: Vec<Vec<DiffLine>>,
    pub hidden_lines: usize,
}

pub fn diff_for_tool_call(tool_call: &ToolCall) -> Option<FileDiff> {
    diff_for_tool(&tool_call.name, &tool_call.input)
}

/// `None` for any other tool and for input that lacks the expected string fields.
pub fn diff_for_tool(name: &str, input: &Value) -> Option<FileDiff> {
    let object = input.as_object()?;
    let path = object.get("file_path")?.as_str()?.to_string();
    let (mut hunks, new_content) = match name {
        "Edit" => (vec![edit_hunk(object)?], false),
        "MultiEdit" => {
            let mut hunks = Vec::new();
            for edit in object.get("edits")?.as_array()? {
                hunks.push(edit_hunk(edit.as_object()?)?);
            }
            (hunks, false)
        }
        "Write" => (vec![line_diff("", string_field(object, "content")?)], true),
        _ => return None,
    };
    let count = |kind| hunks.iter().flatten().filter(|line| line.kind == kind).count();
    let (added, removed) = (count(DiffLineKind::Added), count(DiffLineKind::Removed));
    let hidden_lines = truncate_hunks(&mut hunks, MAX_DIFF_LINES);
    Some(FileDiff { path, new_content, added, removed, hunks, hidden_lines })
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key)?.as_str()
}

fn edit_hunk(object: &Map<String, Value>) -> Option<Vec<DiffLine>> {
    Some(line_diff(string_field(object, "old_string")?, string_field(object, "new_string")?))
}

fn line_diff(old: &str, new: &str) -> Vec<DiffLine> {
    TextDiff::from_lines(old, new)
        .iter_all_changes()
        .map(|change| DiffLine {
            kind: match change.tag() {
                ChangeTag::Equal => DiffLineKind::Context,
                ChangeTag::Insert => DiffLineKind::Added,
                ChangeTag::Delete => DiffLineKind::Removed,
            },
            text: change.value().trim_end_matches(['\n', '\r']).to_string(),
        })
        .collect()
}

// Keeps the first `max` lines across all hunks and returns how many were cut.
fn truncate_hunks(hunks: &mut Vec<Vec<DiffLine>>, max: usize) -> usize {
    let mut remaining = max;
    let mut hidden = 0;
    for hunk in hunks.iter_mut() {
        let kept = hunk.len().min(remaining);
        hidden += hunk.len() - kept;
        hunk.truncate(kept);
        remaining -= kept;
    }
    hunks.retain(|hunk| !hunk.is_empty());
    hidden
}
```

- [ ] **Step 4: Run the tests to see them pass**

Run: `cargo test -p forge-entities diff::`
Expected: 11 passed.

- [ ] **Step 5: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: compute line diffs for Edit, MultiEdit and Write tool calls" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 4: `DiffView`, the expandable tool-call line and the permission-card diff

**Files:**
- Create: `crates/forge-entities/src/diff_view.rs`
- Modify: `crates/forge-entities/src/message.rs` (`ToolCallLine` only), `crates/forge-entities/src/permission.rs`, `crates/forge-entities/src/lib.rs`

**Interfaces:**
- Consumes: `FileDiff`, `DiffLineKind`, `diff_for_tool`, `diff_for_tool_call` (Task 3).
- Produces: `#[component] pub fn DiffView(diff: FileDiff) -> Element`; pure helpers `pub(crate) fn diff_line_class(DiffLineKind) -> &'static str` (`"context"`, `"added"`, `"removed"`), `pub(crate) fn diff_line_sign(DiffLineKind) -> &'static str` (`" "`, `"+"`, `"-"`), `pub(crate) fn diff_counts_label(&FileDiff) -> String` (`"+3 −1"`). CSS classes used: `diff`, `diff-header`, `diff-path`, `diff-note`, `diff-counts`, `diff-hunk`, `diff-line`, `context|added|removed`, `diff-sign`, `diff-text`, `diff-more`, and on the line button `expandable` (styled in Task 5).

- [ ] **Step 1: Write the failing tests for the helpers**

Create `crates/forge-entities/src/diff_view.rs` with only:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::FileDiff;

    #[test]
    fn each_line_kind_has_a_class_and_a_sign() {
        assert_eq!(diff_line_class(DiffLineKind::Context), "context");
        assert_eq!(diff_line_class(DiffLineKind::Added), "added");
        assert_eq!(diff_line_class(DiffLineKind::Removed), "removed");
        assert_eq!(diff_line_sign(DiffLineKind::Context), " ");
        assert_eq!(diff_line_sign(DiffLineKind::Added), "+");
        assert_eq!(diff_line_sign(DiffLineKind::Removed), "-");
    }

    #[test]
    fn the_counts_label_uses_the_untruncated_totals() {
        let diff = FileDiff {
            path: "f".to_string(),
            new_content: false,
            added: 3,
            removed: 1,
            hunks: vec![],
            hidden_lines: 0,
        };
        assert_eq!(diff_counts_label(&diff), "+3 −1");
    }
}
```

Add to `lib.rs`: `mod diff_view;` and `pub use diff_view::DiffView;`.

- [ ] **Step 2: Run the tests to see them fail**

Run: `cargo test -p forge-entities diff_view`
Expected: compile error — helpers not found.

- [ ] **Step 3: Implement `DiffView`**

Put above the test module in `diff_view.rs`:

```rust
use dioxus::prelude::*;

use crate::diff::{DiffLineKind, FileDiff};

pub(crate) fn diff_line_class(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => "context",
        DiffLineKind::Added => "added",
        DiffLineKind::Removed => "removed",
    }
}

pub(crate) fn diff_line_sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Context => " ",
        DiffLineKind::Added => "+",
        DiffLineKind::Removed => "-",
    }
}

pub(crate) fn diff_counts_label(diff: &FileDiff) -> String {
    format!("+{} −{}", diff.added, diff.removed)
}

/// A file diff: a header with the path and counts, then the lines of each hunk with a `+` / `-`
/// sign. Shown under a tool call in the chat and inside a permission request.
#[component]
pub fn DiffView(diff: FileDiff) -> Element {
    let counts = diff_counts_label(&diff);
    rsx! {
        div { class: "diff",
            div { class: "diff-header",
                span { class: "diff-path", "{diff.path}" }
                if diff.new_content {
                    span { class: "diff-note", "new content" }
                }
                span { class: "diff-counts", "{counts}" }
            }
            for (index, hunk) in diff.hunks.iter().enumerate() {
                div { key: "{index}", class: "diff-hunk",
                    for (line_index, line) in hunk.iter().enumerate() {
                        div { key: "{line_index}", class: "diff-line {diff_line_class(line.kind)}",
                            span { class: "diff-sign", "{diff_line_sign(line.kind)}" }
                            span { class: "diff-text", "{line.text}" }
                        }
                    }
                }
            }
            if diff.hidden_lines > 0 {
                div { class: "diff-more", "… {diff.hidden_lines} more lines" }
            }
        }
    }
}
```

- [ ] **Step 4: Make `ToolCallLine` expandable**

In `crates/forge-entities/src/message.rs` replace the imports' `tool_summary` line area and the whole `ToolCallLine` function. Add to the `use` block:

```rust
use crate::diff::diff_for_tool_call;
use crate::diff_view::DiffView;
```

Replace `ToolCallLine` with:

```rust
/// One collapsed line per tool call: `▸ Read  src/lib.rs`. An Edit, MultiEdit or Write line is a
/// button that expands the diff underneath; every other call is plain text.
#[component]
pub fn ToolCallLine(tool_call: ToolCall) -> Element {
    let mut expanded = use_signal(|| false);
    let status_class = match tool_call_status(&tool_call) {
        ToolCallStatus::Running => "running",
        ToolCallStatus::Done => "done",
        ToolCallStatus::Failed => "failed",
    };
    let summary = tool_input_summary(&tool_call.input);
    let Some(diff) = diff_for_tool_call(&tool_call) else {
        return rsx! {
            div { class: "tool-call {status_class}", "▸ {tool_call.name}  {summary}" }
        };
    };
    let open = expanded();
    let arrow = if open { "▾" } else { "▸" };
    rsx! {
        button {
            class: "tool-call expandable {status_class}",
            r#type: "button",
            "aria-expanded": "{open}",
            onclick: move |_| expanded.set(!open),
            "{arrow} {tool_call.name}  {summary}"
        }
        if open {
            DiffView { diff: diff.clone() }
        }
    }
}
```

- [ ] **Step 5: Show the diff in the permission card**

Replace `crates/forge-entities/src/permission.rs` with:

```rust
use dioxus::prelude::*;
use forge_domain::PermissionRequest;

use crate::diff::diff_for_tool;
use crate::diff_view::DiffView;
use crate::tool_summary::tool_input_summary;

/// Shows what Claude wants to do — with the diff, for an Edit or Write, so the change can be
/// read before it is allowed. The buttons are the caller's `children`, so this entity stays
/// presentational and the approve-permission feature supplies the behaviour.
#[component]
pub fn PermissionCard(request: PermissionRequest, children: Element) -> Element {
    let summary = tool_input_summary(&request.input);
    let diff = diff_for_tool(&request.tool_name, &request.input);
    rsx! {
        div { class: "permission-card",
            div { class: "tool", "{request.tool_name}" }
            div { class: "input", "{summary}" }
            if let Some(diff) = diff {
                DiffView { diff }
            }
            {children}
        }
    }
}
```

- [ ] **Step 6: Build and run the tests**

Run: `cargo test -p forge-entities && cargo build -p forge-app`
Expected: all `forge-entities` tests pass (the earlier tasks' plus 2 new); the app builds. The components themselves are checked by hand in Task 6.

- [ ] **Step 7: Commit**

```bash
git add crates/forge-entities
git commit -m "feat: expandable diff under Edit and Write tool calls and in permission requests" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Markdown in `MessageBubble` and the stylesheet

**Files:**
- Modify: `crates/forge-entities/src/message.rs` (`MessageBubble` only)
- Modify: `crates/forge-shared/src/theme.css` (append), `crates/forge-shared/src/theme.rs` (tests)

**Interfaces:**
- Consumes: `render_markdown` (Task 2); CSS class names from Task 4.

- [ ] **Step 1: Write the failing stylesheet test**

In `crates/forge-shared/src/theme.rs`, add a new test inside `mod tests`:

```rust
    #[test]
    fn the_stylesheet_styles_markdown_highlighting_and_diffs() {
        for selector in [
            ".markdown",
            ".code-block",
            ".hl-comment",
            ".hl-string",
            ".diff",
            ".diff-line.added",
            ".diff-line.removed",
            ".tool-call.expandable",
        ] {
            assert!(THEME_CSS.contains(selector), "theme.css is missing {selector}");
        }
    }

    #[test]
    fn the_code_and_diff_colors_have_a_dark_palette() {
        let dark = THEME_CSS.rsplit("prefers-color-scheme: dark").next().unwrap();
        for variable in ["--hl-keyword", "--hl-string", "--diff-added-bg", "--diff-removed-bg"] {
            assert!(dark.contains(variable), "the dark palette is missing {variable}");
        }
    }
```

- [ ] **Step 2: Run to see it fail**

Run: `cargo test -p forge-shared theme`
Expected: the two new tests FAIL.

- [ ] **Step 3: Append the styles**

Append to the end of `crates/forge-shared/src/theme.css`:

```css

/* Markdown, code highlighting and diffs (spec §18). The colors are variables with a light and a dark palette. */
:root {
  --code-bg: #f3f1ee;
  --hl-comment: #8a837b;
  --hl-keyword: #a21caf;
  --hl-string: #15803d;
  --hl-number: #b45309;
  --hl-function: #1d4ed8;
  --hl-type: #0f766e;
  --diff-added-bg: rgba(22, 163, 74, 0.14);
  --diff-removed-bg: rgba(220, 38, 38, 0.14);
}
@media (prefers-color-scheme: dark) {
  :root {
    --code-bg: #171412;
    --hl-comment: #8a837b;
    --hl-keyword: #e879f9;
    --hl-string: #86efac;
    --hl-number: #fdba74;
    --hl-function: #93c5fd;
    --hl-type: #5eead4;
    --diff-added-bg: rgba(74, 222, 128, 0.16);
    --diff-removed-bg: rgba(248, 113, 113, 0.18);
  }
}

.message .content.markdown { white-space: normal; }
.markdown > :first-child { margin-top: 0; }
.markdown > :last-child { margin-bottom: 0; }
.markdown p, .markdown ul, .markdown ol, .markdown blockquote, .markdown table { margin: 0 0 10px; }
.markdown h1, .markdown h2, .markdown h3, .markdown h4 { margin: 16px 0 6px; line-height: 1.3; }
.markdown h1 { font-size: 1.3em; }
.markdown h2 { font-size: 1.15em; }
.markdown h3, .markdown h4 { font-size: 1.02em; }
.markdown ul, .markdown ol { padding-left: 1.5em; }
.markdown blockquote { padding-left: 12px; border-left: 3px solid var(--border); color: var(--fg-muted); }
.markdown a { color: var(--accent); }
.markdown code { font-family: var(--mono); font-size: 0.9em; padding: 1px 5px; background: var(--code-bg); border-radius: 5px; }
.markdown table { border-collapse: collapse; display: block; overflow-x: auto; }
.markdown th, .markdown td { padding: 4px 10px; border: 1px solid var(--border); }
.markdown th { background: var(--bg-hover); text-align: left; }
.code-block { margin: 0 0 10px; padding: 10px 12px; background: var(--code-bg); border: 1px solid var(--border); border-radius: var(--radius); overflow-x: auto; }
.code-block code { padding: 0; background: none; font-size: 12.5px; line-height: 1.5; white-space: pre; }

.hl-comment { color: var(--hl-comment); font-style: italic; }
.hl-string { color: var(--hl-string); }
.hl-constant.hl-numeric { color: var(--hl-number); }
.hl-keyword:not(.hl-operator), .hl-storage { color: var(--hl-keyword); }
.hl-entity.hl-name.hl-function, .hl-support.hl-function { color: var(--hl-function); }
.hl-entity.hl-name.hl-type, .hl-entity.hl-name.hl-class, .hl-support.hl-type, .hl-support.hl-class { color: var(--hl-type); }

button.tool-call { appearance: none; background: none; text-align: left; cursor: pointer; }
.tool-call.expandable:hover { background: var(--bg-hover); }

.diff { max-width: 760px; margin: 6px 0 0; border: 1px solid var(--border); border-radius: var(--radius); background: var(--bg-card); overflow: hidden; font-family: var(--mono); font-size: 12px; }
.permission-card .diff { margin: 4px 0 10px; }
.diff-header { display: flex; gap: 10px; align-items: baseline; padding: 6px 10px; border-bottom: 1px solid var(--border); color: var(--fg-muted); }
.diff-path { color: var(--fg); overflow-wrap: anywhere; }
.diff-counts { margin-left: auto; white-space: nowrap; }
.diff-hunk + .diff-hunk { border-top: 1px dashed var(--border); }
.diff-line { display: flex; min-height: 1.5em; line-height: 1.5; }
.diff-sign { flex: none; width: 2.2ch; text-align: center; color: var(--fg-muted); user-select: none; }
.diff-text { white-space: pre-wrap; overflow-wrap: anywhere; padding-right: 10px; }
.diff-line.added { background: var(--diff-added-bg); }
.diff-line.removed { background: var(--diff-removed-bg); }
.diff-more { padding: 5px 10px; color: var(--fg-muted); border-top: 1px solid var(--border); }
```

The `.markdown ul` / `ol` rules use the descendant combinator on the wrapper class set in Step 5.

- [ ] **Step 4: Run to see the stylesheet tests pass**

Run: `cargo test -p forge-shared theme`
Expected: all theme tests pass.

- [ ] **Step 5: Render markdown in `MessageBubble`**

In `crates/forge-entities/src/message.rs` add to the imports `use crate::markdown::render_markdown;` and replace `MessageBubble` with:

```rust
#[component]
pub fn MessageBubble(message: Message) -> Element {
    let (role_class, role_label) = match message.role {
        MessageRole::User => ("user", "You"),
        MessageRole::Assistant => ("assistant", "Claude"),
    };
    // Only Claude's text is markdown; what a person typed is shown exactly as typed.
    let markdown = match message.role {
        MessageRole::Assistant if !message.content.is_empty() => Some(render_markdown(&message.content)),
        _ => None,
    };
    rsx! {
        div { class: "message {role_class}",
            div { class: "role", "{role_label}" }
            if let Some(html) = markdown {
                div { class: "content markdown", dangerous_inner_html: "{html}" }
            } else if !message.content.is_empty() {
                div { class: "content", "{message.content}" }
            }
            for tool_call in message.tool_calls.iter() {
                ToolCallLine { key: "{tool_call.id}", tool_call: tool_call.clone() }
            }
        }
    }
}
```

- [ ] **Step 6: Build and run the whole workspace**

Run: `cargo test --workspace 2>&1 | grep -E "^test result|warning: unused|error" `
Expected: no errors, no new warnings; the total is the Task 1 baseline plus the new tests (about 35 more).

- [ ] **Step 7: Commit**

```bash
git add crates/forge-entities crates/forge-shared
git commit -m "feat: render assistant messages as markdown, with code, diff and tool-call styles" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Manual checklist, fixes, "As built"

Dioxus has no automated UI test here (spec §8), so this task is a person-style walk of the real window. Budget time for fixes found.

**Files:**
- Modify: `docs/superpowers/specs/2026-09-27-forge-design.md` (add §18.7 "As built")

- [ ] **Step 1: Prepare and start the app**

If `sidecar/node_modules` or `sidecar/dist` is missing in this worktree: `cd sidecar && npm ci && npm run build`. Then, from the repo root, run in the background: `cargo run -p forge-app`.

- [ ] **Step 2: Walk the checklist**

In one new session, ask Claude (with the default permission mode) for the following, and check each result. Take a screenshot of each in light mode, then repeat the visual ones in dark mode (macOS System Settings → Appearance).

1. "Answer with a heading, a bullet list, a numbered list, a table of two rows, `inline code`, a link to https://example.com, and a Rust and a Python code block." → all render as formatted HTML; code blocks are highlighted, on a code background, with horizontal scroll for long lines.
2. Click the link → it opens in the system browser and the Forge window does not navigate away.
3. "Print this text exactly: `<script>alert(1)</script> <b>x</b> ![a](http://example.com/a.png) [b](javascript:alert(1))`" → the tags show as literal text, no image, no live link, "b" and "a" visible as text.
4. "Show a TypeScript code block" → shown unhighlighted but readable (the bundled syntax set has no TypeScript or TOML — record it).
5. While a long answer streams, the half-written code block looks right and does not flicker badly.
6. Your own message containing `**not bold**` and `<b>` is shown exactly as typed.
7. "Create a file `scratch/hello.txt` with three lines" → the Write line shows `▸ Write  scratch/hello.txt`; clicking it expands a diff with three green `+` lines and the note "new content"; clicking again collapses it. Tab/Enter on the line also toggles it.
8. "Change the second line of that file" → the Edit line expands to a diff with context, one red and one green line.
9. In default permission mode the permission card for step 7/8 shows the same diff above the Allow/Deny buttons.
10. A Read or Bash tool call is still a plain, non-clickable line.
11. Resume an older session from the sidebar that contains Edit/Write calls → their diffs expand the same way.
12. A `Write` of a long file (ask for 400 lines) shows 300 lines and "… 100 more lines".

- [ ] **Step 3: Fix what the walk finds**

One commit per fix with a `fix:` message and the co-author line. Add a failing test first whenever the bug is in a pure function.

- [ ] **Step 4: Record "As built" in the spec**

Append to the spec after §18.6 a `### 18.7 As built` section in the style of §16.9 / §17.8: the branch, the date, the workspace test count (from `cargo test --workspace`), **checked live (passed)**, **not checked**, **runtime fixes** (each commit), **differences from §18** (at least: images render as alt text; code blocks over 100 000 bytes are not highlighted; a diff is cut at 300 lines; `diff_for_tool` takes a name and an input so the permission card can use it; the task order put highlighting before markdown) and **not fixed, on purpose** (at least: TypeScript and TOML fences are not highlighted; the whole message is re-parsed on every streamed update).

- [ ] **Step 5: Full check and commit**

Run: `cargo test --workspace` and `cd sidecar && npm test && npm run typecheck`
Expected: all pass.

```bash
git add docs crates
git commit -m "docs: record Plan 8 as built" -m "Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>"
```
