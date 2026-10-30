//! The interactive page Claude draws with `generate_sandboxed_ui`: read from the recorded call, built into
//! the frame's document, and the one message the page may send back. No Dioxus in here.

use forge_domain::ToolCall;
use serde_json::Value;

const TOOL_NAME: &str = "mcp__forge_ui__generate_sandboxed_ui";
/// The pieces of a page together; the tool refuses more, and the app does not draw more.
pub const MAX_PAGE_CHARS: usize = 200_000;
pub const MIN_HEIGHT: u32 = 100;
pub const MAX_HEIGHT: u32 = 800;
const DEFAULT_HEIGHT: u32 = 240;
const MAX_PLACEHOLDERS: usize = 6;
const MAX_PLACEHOLDER_CHARS: usize = 120;

pub const MAX_VALUES: usize = 20;
const MAX_KEY_CHARS: usize = 40;
const MAX_VALUE_CHARS: usize = 200;
const MAX_LABEL_CHARS: usize = 40;
const MAX_MESSAGE_CHARS: usize = 600;

/// Whether a call is the page tool (its row is left out of the folded activity line).
pub fn is_ui_call(call: &ToolCall) -> bool {
    call.name == TOOL_NAME
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiState {
    /// The call has no page yet: the placeholder lines show.
    Pending,
    Ready,
    /// The call was refused or its page cannot be drawn: one line of why.
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedUi {
    /// The id of the call: a card is told apart from its neighbours, and its frame answers to it, by this.
    pub id: String,
    pub height: u32,
    pub placeholders: Vec<String>,
    pub css: String,
    pub html: String,
    pub js_functions: String,
    pub js_expressions: String,
    pub state: UiState,
}

fn text(input: &Value, key: &str) -> String {
    input.get(key).and_then(Value::as_str).unwrap_or_default().to_string()
}

/// The card a `generate_sandboxed_ui` call asks for.
pub fn generated_ui_of(call: &ToolCall) -> Option<GeneratedUi> {
    if !is_ui_call(call) {
        return None;
    }
    let input = &call.input;
    let height = input
        .get("initialHeight")
        .and_then(Value::as_f64)
        .map_or(DEFAULT_HEIGHT, |h| h.round().clamp(MIN_HEIGHT as f64, MAX_HEIGHT as f64) as u32);
    let placeholders = input
        .get("placeholderMessages")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(Value::as_str)
                .map(|line| line.trim().chars().take(MAX_PLACEHOLDER_CHARS).collect::<String>())
                .filter(|line| !line.is_empty())
                .take(MAX_PLACEHOLDERS)
                .collect()
        })
        .unwrap_or_default();
    let mut ui = GeneratedUi {
        id: call.id.clone(),
        height,
        placeholders,
        css: text(input, "css"),
        html: text(input, "html"),
        js_functions: text(input, "jsFunctions"),
        js_expressions: text(input, "jsExpressions"),
        state: UiState::Ready,
    };
    let size = ui.css.chars().count() + ui.html.chars().count() + ui.js_functions.chars().count() + ui.js_expressions.chars().count();
    ui.state = match &call.result {
        Some(result) if result.is_error => UiState::Failed(result.output.lines().next().unwrap_or_default().trim().to_string()),
        _ if size > MAX_PAGE_CHARS => UiState::Failed("The page is too large".to_string()),
        None if ui.html.trim().is_empty() => UiState::Pending,
        _ if ui.html.trim().is_empty() => UiState::Failed("The page has no content".to_string()),
        _ => UiState::Ready,
    };
    Some(ui)
}

/// Script and style hosts a page may load libraries from (three.js, MapLibre, chart libraries).
const LIBRARY_HOSTS: &str = "https://cdnjs.cloudflare.com https://cdn.jsdelivr.net https://unpkg.com";
/// The one host a page may talk to: OpenFreeMap serves map styles, tiles, fonts and sprites for the whole world
/// with no key and no usage limit. A page can ask it for data, and nothing else on the network.
const MAP_HOST: &str = "https://tiles.openfreemap.org";
/// Where a page may show photos from: Wikimedia Commons (free-licensed pictures). `commons.wikimedia.org` is the
/// `Special:FilePath` address that redirects through `commons.wikimedia.org` to `thumb.wikimedia.org` (or
/// `upload.wikimedia.org`); all three are needed because every hop of a redirect is checked. Images only: no script, style or connection.
const PHOTO_HOSTS: &str = "https://upload.wikimedia.org https://thumb.wikimedia.org https://commons.wikimedia.org";

/// Nothing may be sent anywhere but `MAP_HOST`: `default-src 'none'` closes fetch, XHR and WebSocket to every other
/// address, and there are no frames, forms or navigation. `worker-src blob:` is what the map library's tile
/// workers run from.
fn csp() -> String {
    format!(
        "default-src 'none'; script-src 'unsafe-inline' {LIBRARY_HOSTS}; style-src 'unsafe-inline' {LIBRARY_HOSTS}; connect-src {MAP_HOST}; img-src data: blob: {LIBRARY_HOSTS} {MAP_HOST} {PHOTO_HOSTS}; font-src data: {LIBRARY_HOSTS}; worker-src blob:; child-src blob:; media-src data:; base-uri 'none'; form-action 'none'"
    )
}

/// The colours a page may use. They follow the app's theme: the parent tells the frame (`forgeTheme`), and the
/// bridge script sets `data-forge-theme` on the root.
const THEME_CSS: &str = ":root{color-scheme:light;--bg:#ffffff;--fg:#1f1e1c;--muted:#6f6b63;--line:#e4e1da;--accent:#c4552d;--surface:#faf9f6;--c1:#c4552d;--c2:#3e63dd;--c3:#30a46c;--c4:#e5a50a;--c5:#8e4ec6;--c6:#0b8da3}:root[data-forge-theme=dark]{color-scheme:dark;--bg:#1f1e1c;--fg:#ece9e2;--muted:#9d998f;--line:#3a3834;--accent:#d97757;--surface:#2a2926;--c1:#d97757;--c2:#7d96f2;--c3:#4cc38a;--c4:#f0b429;--c5:#b88be0;--c6:#35b9cc}";

/// The Forge UI kit (`fx-` classes): see `ui_kit.css` and `sidecar/src/ui_examples/kit.md`.
pub const UI_KIT_CSS: &str = include_str!("ui_kit.css");

const BASE_CSS: &str = "html,body{margin:0;padding:0}body{background:var(--bg);color:var(--fg);font:14px/1.5 -apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif;padding:12px;box-sizing:border-box}button,input,select,textarea{font:inherit;color:inherit}*{box-sizing:border-box}";

/// The page's script can end a `<script>` or `<style>` element early by writing the closing tag; a `<` before `/`
/// is escaped so it cannot. In JavaScript `<\/script` is the same text as `</script`; in CSS the escape is
/// `\3c `.
fn guard_script(js: &str) -> String {
    js.replace("</", "<\\/").replace("<!--", "<\\!--")
}
fn guard_style(css: &str) -> String {
    css.replace("</", "\\3c /")
}

/// The tallest a card grows to by following its page (the page reports its height, see `BRIDGE_JS`).
pub const MAX_AUTO_HEIGHT: u32 = 1400;
/// The shortest a card shrinks to.
pub const MIN_AUTO_HEIGHT: u32 = 80;

/// The script the frame starts with. It sets the theme the parent tells it, cancels link clicks, wires the kit's
/// tabs, reports the page's height (`resize`) and gives the page its one way out, `forge.send`: a label and a few
/// values, tagged with this card's nonce. `__NONCE__` is replaced by the card's token.
const BRIDGE_JS: &str = r#"(function(){
var post=window.parent.postMessage.bind(window.parent);var n=__NONCE__;
window.addEventListener('message',function(e){if(e.source===window.parent&&e.data&&typeof e.data.forgeTheme==='string'){document.documentElement.setAttribute('data-forge-theme',e.data.forgeTheme==='dark'?'dark':'light')}});
function pick(set,name){set.querySelectorAll('.fx-tab[data-tab]').forEach(function(b){b.setAttribute('aria-selected',b.getAttribute('data-tab')===name?'true':'false')});set.querySelectorAll('.fx-panel[data-panel]').forEach(function(p){p.hidden=p.getAttribute('data-panel')!==name})}
document.addEventListener('click',function(e){var t=e.target;if(!t||!t.closest)return;if(t.closest('a'))e.preventDefault();var tab=t.closest('.fx-tab[data-tab]');if(tab){pick(tab.closest('.fx-tabset')||document,tab.getAttribute('data-tab'))}},true);
document.querySelectorAll('.fx-tabset').forEach(function(set){var first=set.querySelector('.fx-tab[aria-selected="true"]')||set.querySelector('.fx-tab[data-tab]');if(first)pick(set,first.getAttribute('data-tab'))});
var last=0;function size(){var h=Math.ceil(document.body.getBoundingClientRect().height);if(Math.abs(h-last)>=2){last=h;try{post({n:n,resize:h},'*')}catch(e){}}}
if(window.ResizeObserver){new ResizeObserver(function(){requestAnimationFrame(size)}).observe(document.body)}
window.addEventListener('load',size);size();
window.forge=Object.freeze({send:function(m){try{var v=m&&m.values&&typeof m.values==='object'?m.values:{};post({n:n,label:String(m&&m.label||''),values:v},'*')}catch(e){}}});
})();"#;

fn bridge_script(nonce: &str) -> String {
    BRIDGE_JS.replace("__NONCE__", &format!("{nonce:?}"))
}

/// The document the frame shows. `nonce` is this card's token.
pub fn page_document(ui: &GeneratedUi, nonce: &str) -> String {
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{csp}\"><style>{THEME_CSS}{BASE_CSS}{UI_KIT_CSS}{css}</style></head><body>{html}<script>{bridge}</script><script>{functions}</script><script>{expressions}</script></body></html>",
        csp = csp(),
        css = guard_style(&ui.css),
        html = ui.html,
        bridge = bridge_script(nonce),
        functions = guard_script(&ui.js_functions),
        expressions = guard_script(&ui.js_expressions),
    )
}

fn clip(value: &str, max: usize) -> String {
    value.chars().take(max).collect::<String>().trim().to_string()
}

fn show(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(clip(text, MAX_VALUE_CHARS)),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        _ => None,
    }
}

/// The user message a page's `forge.send` becomes, or `None` when the message is not acceptable: not this
/// card's nonce, no values, too many, a value that is not text, a number or a flag, or a text that would run a
/// command. The text is Forge's own wording of the values; the page's markup never reaches the chat.
pub fn bridge_message(raw: &str, nonce: &str) -> Option<String> {
    let data: Value = serde_json::from_str(raw).ok()?;
    if data.get("n")?.as_str()? != nonce {
        return None;
    }
    let label = clip(data.get("label").and_then(Value::as_str).unwrap_or_default(), MAX_LABEL_CHARS);
    let values = data.get("values")?.as_object()?;
    if values.is_empty() || values.len() > MAX_VALUES {
        return None;
    }
    let mut parts = Vec::new();
    for (key, value) in values {
        let key = clip(key, MAX_KEY_CHARS);
        if key.is_empty() {
            return None;
        }
        parts.push(format!("{key}: {}", show(value)?));
    }
    let body = parts.join(", ");
    let message = if label.is_empty() { body } else { format!("{label} — {body}") };
    let message = clip(&message, MAX_MESSAGE_CHARS);
    (!message.starts_with('/') && !message.starts_with('!')).then_some(message)
}

#[cfg(test)]
mod tests {
    use forge_domain::ToolResult;
    use serde_json::json;

    use super::*;

    fn call(input: serde_json::Value, result: Option<(&str, bool)>) -> ToolCall {
        ToolCall {
            id: "u1".to_string(),
            name: TOOL_NAME.to_string(),
            input,
            result: result.map(|(output, is_error)| ToolResult { output: output.to_string(), is_error }),
        }
    }

    #[test]
    fn only_the_page_tool_is_a_page() {
        let other = ToolCall { id: "x".into(), name: "mcp__ccd_turn__wrap_up".into(), input: json!({"html": "<p/>"}), result: None };
        assert!(generated_ui_of(&other).is_none());
        assert!(is_ui_call(&call(json!({}), None)));
    }

    #[test]
    fn a_whole_call_becomes_a_ready_card_with_clamped_height() {
        let ui = generated_ui_of(&call(json!({"initialHeight": 5000, "placeholderMessages": ["a", " ", "b"], "css": "p{}", "html": "<p>x</p>", "jsFunctions": "function f(){}", "jsExpressions": "f()"}), Some(("ok", false)))).unwrap();
        assert_eq!(ui.state, UiState::Ready);
        assert_eq!(ui.height, MAX_HEIGHT);
        assert_eq!(ui.placeholders, vec!["a", "b"]);
        assert_eq!(ui.id, "u1");
        let small = generated_ui_of(&call(json!({"initialHeight": 3, "html": "x"}), None)).unwrap();
        assert_eq!(small.height, MIN_HEIGHT);
        assert_eq!(generated_ui_of(&call(json!({"html": "x"}), None)).unwrap().height, 240);
    }

    #[test]
    fn a_call_without_html_waits_while_it_runs_and_fails_when_it_ended() {
        assert_eq!(generated_ui_of(&call(json!({"placeholderMessages": ["Drawing"]}), None)).unwrap().state, UiState::Pending);
        assert!(matches!(generated_ui_of(&call(json!({}), Some(("ok", false)))).unwrap().state, UiState::Failed(_)));
    }

    #[test]
    fn a_refused_or_oversized_page_fails_with_one_line() {
        let refused = generated_ui_of(&call(json!({"html": "x"}), Some(("The page has no html.\nmore", true)))).unwrap();
        assert_eq!(refused.state, UiState::Failed("The page has no html.".to_string()));
        let big = generated_ui_of(&call(json!({"html": "x".repeat(MAX_PAGE_CHARS + 1)}), Some(("ok", false)))).unwrap();
        assert!(matches!(big.state, UiState::Failed(_)));
    }

    fn ready(css: &str, html: &str, functions: &str, expressions: &str) -> GeneratedUi {
        GeneratedUi { id: "u".into(), height: 200, placeholders: vec![], css: css.into(), html: html.into(), js_functions: functions.into(), js_expressions: expressions.into(), state: UiState::Ready }
    }

    #[test]
    fn the_document_starts_with_the_policy_and_orders_the_pieces() {
        let doc = page_document(&ready("p{color:red}", "<p>x</p>", "function f(){}", "f()"), "abc");
        let csp = doc.find("Content-Security-Policy").unwrap();
        let style = doc.find("p{color:red}").unwrap();
        let html = doc.find("<p>x</p>").unwrap();
        let bridge = doc.find("window.forge=").unwrap();
        let functions = doc.find("function f(){}").unwrap();
        let expressions = doc.find("f()</script>").unwrap();
        assert!(csp < style && style < html && html < bridge && bridge < functions && functions < expressions);
        assert!(doc.contains("default-src 'none'"));
        assert!(doc.contains("color-scheme:light") && doc.contains("data-forge-theme=dark"));
    }

    #[test]
    fn the_policy_opens_only_libraries_and_the_map_host_and_keeps_every_other_way_out_closed() {
        let policy = csp();
        for closed in ["frame-src", "object-src", "navigate-to", "frame-ancestors"] {
            assert!(!policy.contains(closed), "{closed} must fall back to default-src 'none'");
        }
        assert!(policy.starts_with("default-src 'none'; "));
        assert!(policy.contains("form-action 'none'") && policy.contains("base-uri 'none'"));
        assert!(!policy.contains("'unsafe-eval'") && !policy.contains(" * ") && !policy.contains("https: "), "{policy}");
        let directive = |name: &str| policy.split("; ").find(|d| d.starts_with(name)).unwrap_or_else(|| panic!("{name}")).to_string();
        // Connections go to the map host and nowhere else.
        assert_eq!(directive("connect-src"), "connect-src https://tiles.openfreemap.org");
        // Workers only from a blob the page's own library makes.
        assert_eq!(directive("worker-src"), "worker-src blob:");
        let allowed = ["cdnjs.cloudflare.com", "cdn.jsdelivr.net", "unpkg.com", "tiles.openfreemap.org", "upload.wikimedia.org", "thumb.wikimedia.org", "commons.wikimedia.org"];
        for host in policy.split_whitespace().filter(|part| part.starts_with("https://")) {
            assert!(allowed.contains(&host.trim_start_matches("https://").trim_end_matches(';')), "{host}");
        }
        // Scripts and styles only from the library hosts, never from the map or photo hosts.
        for kind in ["script-src", "style-src", "font-src"] {
            let d = directive(kind);
            assert!(!d.contains("openfreemap") && !d.contains("wikimedia"), "{d}");
        }
        // Photos are images only.
        for host in ["upload.wikimedia.org", "thumb.wikimedia.org", "commons.wikimedia.org"] {
            assert!(directive("img-src").contains(host), "{host}");
        }
        assert!(!directive("connect-src").contains("wikimedia"));
        // The raster tile hosts that limit use or need a key are gone.
        for gone in ["openstreetmap", "arcgisonline", "nationalmap", "cartocdn"] {
            assert!(!policy.contains(gone), "{gone}");
        }
    }

    #[test]
    fn model_text_cannot_end_its_script_or_style_early() {
        let doc = page_document(&ready("a{} </style><script>evil()</script>", "x", "var s='</script><img src=x onerror=evil()>';", "// </SCRIPT>"), "n");
        assert_eq!(doc.matches("</script>").count(), 3, "only the three scripts' own closing tags");
        assert_eq!(doc.matches("</style>").count(), 1);
        assert!(doc.contains("<\\/script>"));
    }

    #[test]
    fn the_bridge_script_carries_the_nonce_and_cannot_be_replaced() {
        let script = bridge_script("tok\"en");
        assert!(script.contains("\"tok\\\"en\""), "{script}");
        assert!(script.contains("Object.freeze"));
        assert!(!script.contains("__NONCE__"));
    }

    #[test]
    fn the_frame_carries_the_kit_after_the_base_and_before_the_page() {
        let doc = page_document(&ready("p{color:red}", "<p>x</p>", "", ""), "abc");
        let base = doc.find("html,body{margin:0").unwrap();
        let kit = doc.find(".fx-card").unwrap();
        let page = doc.find("p{color:red}").unwrap();
        assert!(base < kit && kit < page, "a page's own css overrides the kit");
        for token in ["--surface", "--c1", "--c6", "data-forge-theme=dark"] {
            assert!(doc.contains(token), "{token}");
        }
    }

    #[test]
    fn the_bridge_wires_tabs_and_reports_the_height() {
        let script = bridge_script("n");
        for needle in [".fx-tab[data-tab]", ".fx-panel[data-panel]", "resize:h", "ResizeObserver", "aria-selected"] {
            assert!(script.contains(needle), "{needle}");
        }
        assert!(MIN_AUTO_HEIGHT < MAX_AUTO_HEIGHT);
    }

    #[test]
    fn every_kit_class_is_documented_and_every_documented_class_exists() {
        let css_classes = fx_classes(UI_KIT_CSS);
        let doc = include_str!("../../../sidecar/src/ui_examples/kit.md");
        let doc_classes = fx_classes(doc);
        assert!(css_classes.len() > 25, "{css_classes:?}");
        let undocumented: Vec<_> = css_classes.difference(&doc_classes).collect();
        let imaginary: Vec<_> = doc_classes.difference(&css_classes).collect();
        assert!(undocumented.is_empty(), "in the css but not in kit.md: {undocumented:?}");
        assert!(imaginary.is_empty(), "in kit.md but not in the css: {imaginary:?}");
    }

    /// The `fx-` class names a text mentions (`.fx-card`, `class="fx-card fx-row"`).
    fn fx_classes(text: &str) -> std::collections::BTreeSet<String> {
        let mut found = std::collections::BTreeSet::new();
        let bytes: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i + 3 < bytes.len() {
            let boundary = i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == '-' || bytes[i - 1] == '_');
            if boundary && bytes[i] == 'f' && bytes[i + 1] == 'x' && bytes[i + 2] == '-' {
                let mut j = i;
                while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == '-' || bytes[j] == '_') {
                    j += 1;
                }
                found.insert(bytes[i..j].iter().collect::<String>().trim_end_matches('-').to_string());
                i = j;
            } else {
                i += 1;
            }
        }
        found
    }

    fn msg(label: &str, values: serde_json::Value) -> String {
        json!({"n": "abc", "label": label, "values": values}).to_string()
    }

    #[test]
    fn a_good_message_becomes_the_peoples_words() {
        assert_eq!(bridge_message(&msg("Bill", json!({"total": 120, "tip": "15%", "split": true})), "abc").unwrap(), "Bill — split: true, tip: 15%, total: 120");
        assert_eq!(bridge_message(&msg("", json!({"a": 1})), "abc").unwrap(), "a: 1");
    }

    #[test]
    fn a_bad_message_is_dropped() {
        let ok = json!({"a": 1});
        assert!(bridge_message(&msg("x", ok.clone()), "other").is_none(), "wrong nonce");
        assert!(bridge_message("not json", "abc").is_none());
        assert!(bridge_message(&json!({"label": "x", "values": {"a": 1}}).to_string(), "abc").is_none(), "no nonce");
        assert!(bridge_message(&msg("x", json!({})), "abc").is_none(), "no values");
        assert!(bridge_message(&msg("x", json!([1])), "abc").is_none(), "values not an object");
        assert!(bridge_message(&msg("x", json!({"a": {"b": 1}})), "abc").is_none(), "nested value");
        assert!(bridge_message(&msg("x", json!({"a": null})), "abc").is_none(), "null value");
        assert!(bridge_message(&msg("x", json!({" ": 1})), "abc").is_none(), "blank key");
        let many: serde_json::Map<String, serde_json::Value> = (0..=MAX_VALUES).map(|i| (format!("k{i}"), json!(i))).collect();
        assert!(bridge_message(&msg("x", json!(many)), "abc").is_none(), "too many values");
    }

    #[test]
    fn a_message_that_would_run_a_command_is_dropped_and_long_text_is_cut() {
        assert!(bridge_message(&msg("/compact", json!({"a": 1})), "abc").is_none());
        assert!(bridge_message(&msg("!rm", json!({"a": 1})), "abc").is_none());
        let long = bridge_message(&msg("x", json!({"a": "z".repeat(5000)})), "abc").unwrap();
        assert!(long.chars().count() <= MAX_MESSAGE_CHARS);
    }
}
