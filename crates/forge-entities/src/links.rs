use forge_domain::Message;

use crate::highlight::escape_html;
use crate::markdown::render_markdown;

/// The absolute paths the session's own file tools named (Write, Edit, MultiEdit, NotebookEdit,
/// Read), each once, in order. A name that only appears in prose is not here.
pub fn known_paths(messages: &[Message]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for call in messages.iter().flat_map(|message| message.tool_calls()) {
        if !matches!(call.name.as_str(), "Write" | "Edit" | "MultiEdit" | "NotebookEdit" | "Read") {
            continue;
        }
        let path = ["file_path", "notebook_path"].iter().find_map(|key| call.input.get(*key).and_then(|value| value.as_str()));
        if let Some(path) = path.filter(|path| path.starts_with('/')) {
            if !paths.iter().any(|known| known == path) {
                paths.push(path.to_string());
            }
        }
    }
    paths
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `render_markdown`, with links to files. Each inline code span that names a known file, and each
/// bare absolute path in the text (`/dir/name.ext`), is wrapped in
/// `<span class="file-link" data-path="…" role="link" tabindex="0">`. Code in a fenced or indented
/// block, text inside an existing link, and text outside code that is not an absolute path are left
/// alone; a path that holds a quote, `<` or `&` is never put in an attribute.
pub fn render_markdown_linked(source: &str, known: &[String]) -> String {
    let html = render_markdown(source);
    link_prose_paths(&link_known_code(&html, known))
}

// The inline code spans that name a known file become links.
fn link_known_code(html: &str, known: &[String]) -> String {
    if known.is_empty() {
        return html.to_string();
    }
    const OPEN: &str = "<code>";
    const CLOSE: &str = "</code>";
    const BLOCK: &str = "<pre class=\"code-block\">";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        let (before, from_code) = rest.split_at(start);
        out.push_str(before);
        let after_open = &from_code[OPEN.len()..];
        let Some(end) = after_open.find(CLOSE) else {
            out.push_str(from_code);
            return out;
        };
        // `before` is already in `out`: a code span right after the block's `<pre>` is block code.
        let inside_block = out.ends_with(BLOCK);
        let text = &after_open[..end];
        let link = if inside_block { None } else { resolve_escaped(text, known) }.filter(|path| !path.contains(['"', '<', '>', '&']));
        match link {
            Some(path) => out.push_str(&format!(
                "<span class=\"file-link\" data-path=\"{path}\" role=\"link\" tabindex=\"0\">{OPEN}{text}{CLOSE}</span>"
            )),
            None => {
                out.push_str(OPEN);
                out.push_str(text);
                out.push_str(CLOSE);
            }
        }
        rest = &after_open[end + CLOSE.len()..];
    }
    out.push_str(rest);
    out
}

fn is_path_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | '+' | '@' | '~' | '/')
}

// A path names a file when its last part is `stem.ext` with an extension of 1 to 8 letters or digits.
fn has_file_extension(path: &str) -> bool {
    if path.len() < 2 || path.contains("//") {
        return false;
    }
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, extension)) => !stem.is_empty() && (1..=8).contains(&extension.len()) && extension.chars().all(|c| c.is_ascii_alphanumeric()),
        None => false,
    }
}

// The byte ranges of the absolute file paths in one text node of the markdown HTML. The text is
// HTML-escaped, so a path never runs through `&`, `<`, `>` or a quote: those are not path characters.
fn prose_paths(text: &str) -> Vec<(usize, usize)> {
    let mut found = Vec::new();
    let mut from = 0;
    let mut previous: Option<char> = None;
    for (index, c) in text.char_indices() {
        if index >= from && c == '/' && previous.is_none_or(|before| !is_path_char(before) && before != ':') {
            let run_end = text[index..].find(|ch: char| !is_path_char(ch)).map_or(text.len(), |offset| index + offset);
            let path = text[index..run_end].trim_end_matches('.');
            if has_file_extension(path) {
                found.push((index, index + path.len()));
            }
            from = run_end;
        }
        previous = Some(c);
    }
    found
}

fn push_linked(out: &mut String, text: &str, link: bool) {
    if !link {
        out.push_str(text);
        return;
    }
    let mut last = 0;
    for (start, end) in prose_paths(text) {
        out.push_str(&text[last..start]);
        let path = &text[start..end];
        out.push_str(&format!("<span class=\"file-link\" data-path=\"{path}\" role=\"link\" tabindex=\"0\">{path}</span>"));
        last = end;
    }
    out.push_str(&text[last..]);
}

// How deep the text is inside `code`, `pre` and `a` after this tag; text in there is never linked.
fn next_skipped(tag: &str, skipped: usize) -> usize {
    let inner = tag.trim_start_matches('<').trim_end_matches('>').trim();
    let closing = inner.starts_with('/');
    let name = inner.trim_start_matches('/').split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("").to_ascii_lowercase();
    if !matches!(name.as_str(), "code" | "pre" | "a") {
        return skipped;
    }
    if closing {
        skipped.saturating_sub(1)
    } else if inner.ends_with('/') {
        skipped
    } else {
        skipped + 1
    }
}

// Wraps each bare absolute path in the text nodes of `html`.
fn link_prose_paths(html: &str) -> String {
    let mut out = String::with_capacity(html.len());
    let mut skipped = 0usize;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            push_linked(&mut out, rest, skipped == 0);
            break;
        };
        push_linked(&mut out, &rest[..open], skipped == 0);
        let from_tag = &rest[open..];
        let Some(close) = from_tag.find('>') else {
            out.push_str(from_tag);
            break;
        };
        let tag = &from_tag[..=close];
        skipped = next_skipped(tag, skipped);
        out.push_str(tag);
        rest = &from_tag[close + 1..];
    }
    out
}

// The text between the tags is HTML-escaped; compare it with the escaped known paths and names.
fn resolve_escaped<'a>(escaped_text: &str, known: &'a [String]) -> Option<&'a str> {
    let escaped_known: Vec<String> = known.iter().map(|path| escape_html(path)).collect();
    let index = {
        let mut found = None;
        if let Some(position) = escaped_known.iter().position(|path| path == escaped_text) {
            found = Some(position);
        } else if !escaped_text.contains('/') {
            let matches: Vec<usize> = known.iter().enumerate().filter(|(_, path)| escape_html(file_name(path)) == escaped_text).map(|(index, _)| index).collect();
            if matches.len() == 1 {
                found = Some(matches[0]);
            }
        }
        found?
    };
    Some(known[index].as_str())
}

#[cfg(test)]
mod tests {
    use forge_domain::{Block, Message, MessageRole, ToolCall};
    use serde_json::json;

    use super::*;

    fn call(name: &str, input: serde_json::Value) -> Block {
        Block::Tool(ToolCall { id: "t".to_string(), name: name.to_string(), input, result: None })
    }
    fn assistant(blocks: Vec<Block>) -> Message {
        let mut message = Message::new(MessageRole::Assistant);
        message.blocks = blocks;
        message
    }
    fn known() -> Vec<String> {
        vec!["/p/docs/guide.md".to_string(), "/p/src/a.rs".to_string(), "/p/other/a.rs".to_string()]
    }

    #[test]
    fn known_paths_come_from_file_tools_once_each_in_order() {
        let messages = vec![assistant(vec![
            call("Write", json!({"file_path": "/p/docs/guide.md", "content": "x"})),
            call("Read", json!({"file_path": "/p/src/a.rs"})),
            call("Edit", json!({"file_path": "/p/docs/guide.md", "old_string": "a", "new_string": "b"})),
            call("NotebookEdit", json!({"notebook_path": "/p/n.ipynb", "new_source": "x"})),
            call("Bash", json!({"command": "cat /p/secret"})),
            call("Write", json!({"file_path": "relative.txt", "content": "x"})),
            call("Write", json!({"content": "no path"})),
        ])];
        assert_eq!(known_paths(&messages), vec!["/p/docs/guide.md", "/p/src/a.rs", "/p/n.ipynb"]);
    }

    #[test]
    fn an_inline_code_span_with_a_full_known_path_becomes_a_link() {
        let html = render_markdown_linked("see `/p/docs/guide.md` now", &known());
        assert!(html.contains(r#"<span class="file-link" data-path="/p/docs/guide.md" role="link" tabindex="0"><code>/p/docs/guide.md</code></span>"#), "{html}");
    }

    #[test]
    fn a_unique_bare_name_becomes_a_link_to_its_path() {
        let html = render_markdown_linked("saved to `guide.md`", &known());
        assert!(html.contains(r#"data-path="/p/docs/guide.md""#), "{html}");
        assert!(html.contains("<code>guide.md</code>"), "{html}");
    }

    #[test]
    fn a_name_shared_by_two_known_paths_stays_plain() {
        let html = render_markdown_linked("see `a.rs`", &known());
        assert!(!html.contains("file-link"), "{html}");
        assert!(html.contains("<code>a.rs</code>"), "{html}");
    }

    #[test]
    fn code_that_is_not_a_known_path_stays_plain() {
        for text in ["`other.md`", "`/p/docs/missing.md`", "`guide.md.bak`", "`GUIDE.MD`"] {
            assert!(!render_markdown_linked(text, &known()).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn a_bare_name_in_prose_and_code_in_a_fenced_block_stay_plain() {
        let prose = render_markdown_linked("guide.md and /p/docs/guide.md", &known());
        assert!(!prose.contains(r#"data-path="guide.md""#), "a bare name is not a path: {prose}");
        assert!(prose.contains(r#"data-path="/p/docs/guide.md""#), "{prose}");
        let fenced = render_markdown_linked("```\n/p/docs/guide.md\n```", &known());
        assert!(!fenced.contains("file-link"), "{fenced}");
        let language = render_markdown_linked("```text\nguide.md\n```", &known());
        assert!(!language.contains("file-link"), "{language}");
    }

    fn linked(text: &str) -> String {
        render_markdown_linked(text, &[])
    }

    #[test]
    fn an_absolute_path_in_prose_becomes_a_link_even_when_no_tool_named_it() {
        let html = linked("Saved to /tmp/out/report.csv for you");
        assert!(html.contains(r#"<span class="file-link" data-path="/tmp/out/report.csv" role="link" tabindex="0">/tmp/out/report.csv</span>"#), "{html}");
    }

    #[test]
    fn punctuation_and_text_around_a_path_stay_outside_the_link() {
        let html = linked("see /p/a.md.");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains("</span>."), "{html}");
        let html = linked("(/p/a.md)");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains("</span>)"), "{html}");
        let html = linked("저장했습니다: /p/보고서.md 입니다");
        assert!(html.contains(r#"data-path="/p/보고서.md""#) && html.contains("</span> 입니다"), "{html}");
    }

    #[test]
    fn two_paths_on_separate_lines_are_two_links() {
        let html = linked("/p/a.md\n\n/p/b.pdf");
        assert!(html.contains(r#"data-path="/p/a.md""#) && html.contains(r#"data-path="/p/b.pdf""#), "{html}");
    }

    #[test]
    fn a_url_a_relative_path_and_a_path_with_no_extension_are_not_linked() {
        for text in ["/usr/local/bin", "https://example.com/a.html", "see /", "a/b/c.md", "//cdn.example.com/a.js", "foo:/p/a.md", "~/a.md", "x/p/a.md"] {
            assert!(!linked(text).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn a_path_in_code_a_code_block_or_a_link_stays_plain() {
        for text in ["`/p/a.md`", "```\n/p/a.md\n```", "[/p/a.md](https://example.com)", "[x](/p/a.md)"] {
            assert!(!linked(text).contains("file-link"), "{text}");
        }
    }

    #[test]
    fn a_path_with_an_ampersand_or_a_quote_is_cut_before_it() {
        assert!(!linked("/p/a&b.md").contains("file-link"));
        assert!(!linked("/p/a\"b.md").contains("file-link"));
    }

    #[test]
    fn prose_paths_and_known_code_spans_link_together() {
        let html = render_markdown_linked("`guide.md` and /p/other.txt", &known());
        assert!(html.contains(r#"data-path="/p/docs/guide.md""#) && html.contains(r#"data-path="/p/other.txt""#), "{html}");
    }

    #[test]
    fn a_path_with_a_quote_is_never_put_in_an_attribute() {
        let weird = vec!["/p/a\"b.md".to_string()];
        let html = render_markdown_linked("`/p/a\"b.md`", &weird);
        assert!(!html.contains("file-link"), "{html}");
    }

    #[test]
    fn raw_html_is_still_shown_as_text_and_with_no_known_paths_the_output_is_plain_markdown() {
        let html = render_markdown_linked("<script>x</script> `guide.md`", &known());
        assert!(!html.contains("<script"), "{html}");
        assert_eq!(render_markdown_linked("**hi**", &[]), crate::render_markdown("**hi**"));
    }
}
