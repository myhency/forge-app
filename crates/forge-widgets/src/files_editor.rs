use std::cell::RefCell;
use std::rc::Rc;

use dioxus::prelude::*;
use forge_domain::EditorPrefs;
use forge_entities::language_of;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// What the page tells Rust (see `tools/editor-bundle/src/main.js`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
pub enum FromPage {
    /// The editor is built and takes commands.
    Ready,
    /// The bundle did not load (no `ForgeEditor`).
    Unavailable,
    /// The first edit of a tab since a load or a save.
    Dirty { key: String },
    /// The text, 2 s after the last edit.
    Change { key: String, s: String },
    /// `⌘S`.
    Save { key: String, s: String },
    /// The answer to a flush: the tab's text, or `None` when the page has no state for it.
    Flush { key: String, s: Option<String> },
    /// A tab was asked for that the page never loaded.
    Missing { key: String },
    /// A load that was only to happen while the tab was clean did not, because an edit was waiting.
    Skipped { key: String },
    /// `Attach as context` on the picked lines: the lines' text as the editor holds it.
    Attach { key: String, from: u32, to: u32, text: String },
    /// A comment was written (`id` is `None`) or edited on lines of a tab.
    Comment { key: String, from: u32, to: u32, text: String, id: Option<u64> },
    /// The trash icon of a saved comment.
    #[serde(rename = "comment_delete")]
    CommentDelete { key: String, id: u64 },
}

/// `if_clean`: the page does nothing (and says `skipped`) when an edit of this tab is waiting.
/// `dirty`: the text is not what is on disk (a draft that came back), so the page treats it as edited.
pub fn load_command(key: &str, doc: &str, language: &str, readonly: bool, if_clean: bool, dirty: bool) -> Value {
    json!({ "t": "load", "key": key, "doc": doc, "language": language, "readonly": readonly, "if_clean": if_clean, "dirty": dirty })
}

pub fn show_command(key: &str) -> Value {
    json!({ "t": "show", "key": key })
}

pub fn forget_command(key: &str) -> Value {
    json!({ "t": "drop", "key": key })
}

pub fn flush_command(key: &str) -> Value {
    json!({ "t": "flush", "key": key })
}

/// The comments of one file, as the page draws them.
pub fn comments_command(key: &str, cards: &[forge_domain::Card]) -> Value {
    let list: Vec<Value> = cards.iter().map(|c| json!({ "id": c.id, "from": c.range.start, "to": c.range.end, "text": c.text })).collect();
    json!({ "t": "comments", "key": key, "comments": list })
}

pub fn focus_comment_command(key: &str, id: u64) -> Value {
    json!({ "t": "focus_comment", "key": key, "id": id })
}

/// Opens the find bar of a tab on a line (1 based) with a query and its options.
pub fn find_command(key: &str, query: &str, options: forge_ports::SearchOptions, line: usize) -> Value {
    json!({
        "t": "find", "key": key, "query": query,
        "match_case": options.match_case, "whole_word": options.whole_word, "regex": options.regex,
        "line": line,
    })
}

pub fn settings_command(prefs: &EditorPrefs) -> Value {
    json!({
        "t": "settings",
        "settings": {
            "wrap": prefs.wrap,
            "indent_guides": prefs.indent_guides,
            "whitespace": prefs.whitespace,
            "scroll_past_end": prefs.scroll_past_end,
            "tab_indent": prefs.tab_indent,
            "tab_size": prefs.clamped_tab_size(),
        }
    })
}

/// The CodeMirror language id of a file, or `""` for plain text.
pub fn editor_language(path: &str) -> &'static str {
    match language_of(path).to_ascii_lowercase().as_str() {
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "py" | "pyi" => "python",
        "rs" => "rust",
        "json" | "jsonc" => "json",
        "md" | "markdown" => "markdown",
        "html" | "htm" => "html",
        "css" => "css",
        "yml" | "yaml" => "yaml",
        "sh" | "bash" | "zsh" => "shell",
        _ => "",
    }
}

type Inbox = Rc<RefCell<Option<UnboundedReceiver<Value>>>>;

/// Sends commands to the editor in the page. Commands wait in a queue until the page says it is
/// ready, so a tab can be loaded before the editor exists.
#[derive(Clone)]
pub struct EditorHandle {
    tx: UnboundedSender<Value>,
    inbox: Inbox,
}

impl EditorHandle {
    fn send(&self, command: Value) {
        let _ = self.tx.send(command);
    }

    pub fn load(&self, key: &str, doc: &str, language: &str, readonly: bool, if_clean: bool, dirty: bool) {
        self.send(load_command(key, doc, language, readonly, if_clean, dirty));
    }

    pub fn show(&self, key: &str) {
        self.send(show_command(key));
    }

    /// The tab is gone: the page drops its state.
    pub fn forget(&self, key: &str) {
        self.send(forget_command(key));
    }

    /// Shows these comments in the file's editor.
    pub fn comments(&self, key: &str, cards: &[forge_domain::Card]) {
        self.send(comments_command(key, cards));
    }

    /// Opens the comment's box for editing.
    pub fn focus_comment(&self, key: &str, id: u64) {
        self.send(focus_comment_command(key, id));
    }

    /// Opens the find bar of a tab on a line, with the query and options of a search hit.
    pub fn find(&self, key: &str, query: &str, options: forge_ports::SearchOptions, line: usize) {
        self.send(find_command(key, query, options, line));
    }

    pub fn settings(&self, prefs: &EditorPrefs) {
        self.send(settings_command(prefs));
    }

    /// Asks for the tab's text; the answer comes as `FromPage::Flush`.
    pub fn flush(&self, key: &str) {
        self.send(flush_command(key));
    }
}

/// The channel the panel talks to the page through. Call it first, then `use_editor_driver` once
/// the event handler exists (the handler needs the handle).
pub fn use_editor_channel() -> EditorHandle {
    use_hook(|| {
        let (tx, rx) = unbounded_channel();
        EditorHandle { tx, inbox: Rc::new(RefCell::new(Some(rx))) }
    })
}

// The page side: builds the editor in the element `__HOST__` and then relays messages both ways.
// It cleans itself up when its element leaves the page.
const BRIDGE_JS: &str = r#"
const host = document.getElementById("__HOST__");
if (!window.ForgeEditor || !host) {
    dioxus.send({ t: "unavailable" });
} else {
    const editor = window.ForgeEditor.create(host, (m) => dioxus.send(m));
    const watch = setInterval(() => {
        if (!host.isConnected) { clearInterval(watch); editor.destroy(); }
    }, 2000);
    dioxus.send({ t: "ready" });
    while (true) {
        const m = await dioxus.recv();
        if (m.t === "load") editor.load(m.key, m.doc, m.language, m.readonly, m.if_clean, m.dirty);
        else if (m.t === "show") editor.show(m.key);
        else if (m.t === "drop") editor.drop(m.key);
        else if (m.t === "settings") editor.settings(m.settings);
        else if (m.t === "flush") editor.flush(m.key);
        else if (m.t === "comments") editor.comments(m.key, m.comments);
        else if (m.t === "find") editor.find(m.key, { query: m.query, match_case: m.match_case, whole_word: m.whole_word, regex: m.regex, line: m.line });
        else if (m.t === "focus_comment") editor.focusComment(m.key, m.id);
        else if (m.t === "focus") editor.focus();
    }
}
"#;

async fn drive(host_id: String, inbox: Inbox, on_event: Callback<FromPage>) {
    let Some(mut commands) = inbox.borrow_mut().take() else { return };
    let mut page = document::eval(&BRIDGE_JS.replace("__HOST__", &host_id));
    // Nothing is sent to the page before it says it is ready.
    loop {
        match page.recv::<FromPage>().await {
            Ok(FromPage::Ready) => {
                on_event.call(FromPage::Ready);
                break;
            }
            Ok(FromPage::Unavailable) => {
                on_event.call(FromPage::Unavailable);
                return;
            }
            Ok(other) => on_event.call(other),
            Err(_) => return,
        }
    }
    loop {
        tokio::select! {
            event = page.recv::<FromPage>() => match event {
                Ok(event) => on_event.call(event),
                Err(_) => return,
            },
            command = commands.recv() => match command {
                Some(command) => {
                    let _ = page.send(command);
                }
                None => return,
            },
        }
    }
}

/// Starts the bridge to the editor that lives in the element with id `host_id` (render the
/// element before this runs; it is built after the first render). `on_event` gets every message
/// from the page.
pub fn use_editor_driver(host_id: String, handle: &EditorHandle, on_event: Callback<FromPage>) {
    let inbox = handle.inbox.clone();
    use_future(move || {
        let (host_id, inbox) = (host_id.clone(), inbox.clone());
        async move { drive(host_id, inbox, on_event).await }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> FromPage {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn the_messages_from_the_page_parse() {
        assert_eq!(parse(r#"{"t":"ready"}"#), FromPage::Ready);
        assert_eq!(parse(r#"{"t":"unavailable"}"#), FromPage::Unavailable);
        assert_eq!(parse(r#"{"t":"dirty","key":"a.py"}"#), FromPage::Dirty { key: "a.py".to_string() });
        assert_eq!(parse(r#"{"t":"change","key":"a.py","s":"x = 1\n"}"#), FromPage::Change { key: "a.py".to_string(), s: "x = 1\n".to_string() });
        assert_eq!(parse(r#"{"t":"save","key":"a.py","s":""}"#), FromPage::Save { key: "a.py".to_string(), s: String::new() });
        assert_eq!(parse(r#"{"t":"flush","key":"a.py","s":"t"}"#), FromPage::Flush { key: "a.py".to_string(), s: Some("t".to_string()) });
        assert_eq!(parse(r#"{"t":"flush","key":"a.py","s":null}"#), FromPage::Flush { key: "a.py".to_string(), s: None });
        assert_eq!(parse(r#"{"t":"missing","key":"a.py"}"#), FromPage::Missing { key: "a.py".to_string() });
        assert_eq!(parse(r#"{"t":"skipped","key":"a.py"}"#), FromPage::Skipped { key: "a.py".to_string() });
    }

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
        let card = forge_domain::Card {
            id: 7,
            kind: forge_domain::CardKind::Comment,
            path: "/p/a.py".into(),
            range: forge_domain::LineRange::new(3, 4),
            text: "why".into(),
        };
        assert_eq!(
            comments_command("a.py", &[card]),
            serde_json::json!({ "t": "comments", "key": "a.py", "comments": [{ "id": 7, "from": 3, "to": 4, "text": "why" }] })
        );
        assert_eq!(focus_comment_command("a.py", 7), serde_json::json!({ "t": "focus_comment", "key": "a.py", "id": 7 }));
    }

    #[test]
    fn the_find_command_carries_the_query_options_and_line() {
        let options = forge_ports::SearchOptions { match_case: true, whole_word: false, regex: true };
        assert_eq!(
            find_command("a.py", "Bu.z", options, 7),
            serde_json::json!({ "t": "find", "key": "a.py", "query": "Bu.z", "match_case": true, "whole_word": false, "regex": true, "line": 7 })
        );
    }

    #[test]
    fn a_load_can_say_the_text_is_not_what_is_on_disk() {
        assert_eq!(load_command("a.py", "x", "python", false, false, true)["dirty"], serde_json::json!(true));
        assert_eq!(load_command("a.py", "x", "python", false, false, false)["dirty"], serde_json::json!(false));
    }

    #[test]
    fn a_path_with_quotes_and_hangul_survives_the_json() {
        let key = "한글 \"폴더\"/a.py";
        let message = serde_json::to_string(&serde_json::json!({ "t": "dirty", "key": key })).unwrap();
        assert_eq!(parse(&message), FromPage::Dirty { key: key.to_string() });
    }

    #[test]
    fn the_commands_to_the_page_carry_what_the_bridge_reads() {
        assert_eq!(
            load_command("a.py", "x = 1\n", "python", true, false, false),
            serde_json::json!({ "t": "load", "key": "a.py", "doc": "x = 1\n", "language": "python", "readonly": true, "if_clean": false, "dirty": false })
        );
        assert_eq!(load_command("a.py", "", "", false, true, false)["if_clean"], serde_json::json!(true));
        assert_eq!(show_command("a.py"), serde_json::json!({ "t": "show", "key": "a.py" }));
        assert_eq!(forget_command("a.py"), serde_json::json!({ "t": "drop", "key": "a.py" }));
        assert_eq!(flush_command("a.py"), serde_json::json!({ "t": "flush", "key": "a.py" }));
    }

    #[test]
    fn the_settings_message_uses_the_names_the_page_reads_and_a_usable_tab_size() {
        let prefs = EditorPrefs { wrap: false, whitespace: true, tab_size: 3, ..EditorPrefs::default() };
        assert_eq!(
            settings_command(&prefs),
            serde_json::json!({
                "t": "settings",
                "settings": {
                    "wrap": false,
                    "indent_guides": true,
                    "whitespace": true,
                    "scroll_past_end": false,
                    "tab_indent": true,
                    "tab_size": 4
                }
            })
        );
    }

    #[test]
    fn the_language_follows_the_extension() {
        for (path, expected) in [
            ("a.py", "python"),
            ("src/main.RS", "rust"),
            ("x.tsx", "tsx"),
            ("x.ts", "typescript"),
            ("x.jsx", "jsx"),
            ("x.mjs", "javascript"),
            ("x.json", "json"),
            ("README.md", "markdown"),
            ("index.html", "html"),
            ("a.css", "css"),
            ("a.yml", "yaml"),
            ("run.sh", "shell"),
            ("Makefile", ""),
            (".gitignore", ""),
            ("notes.txt", ""),
        ] {
            assert_eq!(editor_language(path), expected, "{path}");
        }
    }
}
