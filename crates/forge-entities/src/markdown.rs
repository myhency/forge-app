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
