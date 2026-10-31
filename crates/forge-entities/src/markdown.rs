use pulldown_cmark::{html, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

use crate::code_run::{is_shell_language, text_hash};
use crate::highlight::{escape_html, highlight_code};

// Above this many bytes a code block is shown escaped but not highlighted. The whole message is
// re-rendered on every streamed token, so the cost of highlighting is paid once per token:
// about 55 ms per token at this size in a release build, and it grows steeply beyond it.
const MAX_HIGHLIGHT_BYTES: usize = 20_000;

/// Assistant text as HTML. Raw HTML in the text is escaped, links keep an `href` only for
/// `http`, `https` and `mailto`, images become their alt text, and fenced code is highlighted
/// when its language is known.
pub fn render_markdown(source: &str) -> String {
    let parser = Parser::new_ext(source, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS);
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
    // The button's script is fixed text; nothing from the message is put into it.
    let copy = format!("<button class=\"code-copy\" type=\"button\" title=\"Copy\" onclick=\"navigator.clipboard.writeText(this.nextElementSibling.innerText)\">{}</button>", forge_shared::COPY_ICON_SVG);
    // Before the copy button, which copies its next sibling (the pre). The click is handled by the page-wide
    // handler of the chat (`code_run_watcher`); the key is the block's hash and holds no message text.
    let run = match language {
        Some(language) if is_shell_language(language) => format!("<button class=\"code-run\" type=\"button\" title=\"Run\" data-hash=\"{}\">{}</button>", text_hash(code), forge_shared::RUN_ICON_SVG),
        _ => String::new(),
    };
    match language {
        Some(language) => format!("<div class=\"code-wrap\">{run}{copy}<pre class=\"code-block\"><code class=\"language-{language}\">{body}</code></pre></div>\n"),
        None => format!("<div class=\"code-wrap\">{copy}<pre class=\"code-block\"><code>{body}</code></pre></div>\n"),
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
    fn task_list_items_get_a_checkbox() {
        let html = render_markdown("- [x] done\n- [ ] todo");
        assert!(html.contains("type=\"checkbox\""), "{html}");
        assert_eq!(html.matches("checked").count(), 1, "{html}");
    }

    #[test]
    fn a_plain_list_has_no_checkbox() {
        assert!(!render_markdown("- a\n- b").contains("checkbox"));
    }

    #[test]
    fn a_code_block_carries_a_copy_button_outside_the_pre() {
        let html = render_markdown("```rust\nfn main() {}\n```");
        assert!(html.starts_with("<div class=\"code-wrap\"><button"), "{html}");
        assert!(html.contains("class=\"code-copy\""), "{html}");
        assert!(html.contains("<pre class=\"code-block\">"), "{html}");
        assert!(html.contains("</pre></div>"), "{html}");
    }

    #[test]
    fn a_bash_block_has_a_run_button_before_the_copy_button() {
        let html = render_markdown("```bash\necho hi\n```");
        let run = html.find("class=\"code-run\"").expect("run button");
        let copy = html.find("class=\"code-copy\"").expect("copy button");
        // The copy button copies its next sibling, which must stay the pre.
        assert!(run < copy, "{html}");
        assert!(html.contains("data-hash=\""), "{html}");
        assert!(html.contains("</button><pre class=\"code-block\">"), "{html}");
    }

    #[test]
    fn a_python_block_has_no_run_button() {
        assert!(!render_markdown("```python\nprint(1)\n```").contains("code-run"));
    }

    #[test]
    fn a_block_without_a_language_has_no_run_button() {
        assert!(!render_markdown("```\nls\n```").contains("code-run"));
        assert!(!render_markdown("    ls\n").contains("code-run"));
    }

    #[test]
    fn the_run_button_has_no_script_and_no_message_text() {
        let html = render_markdown("```bash\n\"><script>alert(1)</script>\n```");
        let button = html.split("</button>").next().unwrap();
        assert!(!button.contains("onclick") && !button.contains("alert") && !button.contains("script"), "{button}");
    }

    #[test]
    fn the_copy_button_script_is_fixed_text_not_from_the_message() {
        let html = render_markdown("```\n\"); alert(1); (\"\n```");
        let button = html.split("</button>").next().unwrap();
        assert!(!button.contains("alert"), "{button}");
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
        assert!(html.contains("<pre class=\"code-block\"><code class=\"language-rust\">"), "{html}");
        assert!(html.contains("hl-"), "{html}");
        assert!(html.ends_with("</code></pre></div>\n"), "{html}");
    }

    #[test]
    fn a_fence_with_an_unknown_language_is_escaped_not_highlighted() {
        let html = render_markdown("```nosuchlang\na < b\n```");
        assert!(
            html.contains("<pre class=\"code-block\"><code class=\"language-nosuchlang\">a &lt; b\n</code></pre>"),
            "{html}"
        );
    }

    #[test]
    fn a_fence_without_a_language_has_no_class() {
        let html = render_markdown("```\nplain\n```");
        assert!(html.contains("<pre class=\"code-block\"><code>plain\n</code></pre>"), "{html}");
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

    #[test]
    fn a_code_block_too_big_to_re_highlight_on_every_streamed_token_is_not_highlighted() {
        let source = format!("```rust\n{}```", "let x = 1;\n".repeat(2_500));
        assert!(source.len() > 25_000);
        assert!(!render_markdown(&source).contains("hl-"));
        let small = format!("```rust\n{}```", "let x = 1;\n".repeat(500));
        assert!(render_markdown(&small).contains("hl-"));
    }
}
