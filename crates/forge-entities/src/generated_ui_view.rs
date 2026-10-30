//! The card of a generated page: the frame, its placeholder, and the bridge from the page to the chat.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dioxus::prelude::*;
use forge_shared::tr;

use crate::generated_ui::{bridge_message, page_document, GeneratedUi, UiState};

/// A page may send at most one message in this time.
const SEND_GAP: Duration = Duration::from_secs(1);

/// A token for one card, known to the page and to this card only: two numbers from the standard library's
/// randomly keyed hasher.
fn fresh_nonce() -> String {
    use std::hash::{BuildHasher, Hasher};
    let part = || {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos()));
        hasher.finish()
    };
    format!("{:016x}{:016x}", part(), part())
}

/// What a frame id may be made of: the tool call's id, cleaned so it can sit in an element id and a script.
fn dom_id(id: &str) -> String {
    id.chars().map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).collect()
}

/// The parent page's side of the bridge. One `message` listener serves every card: it takes a message only from
/// a frame of ours, only while that frame holds the focus (the person clicked in it), and passes the text on to
/// the card whose id the frame carries. It also tells each frame whether the app is dark.
fn bridge_watch(id: &str) -> String {
    format!(
        r#"(function () {{
  var id = "{id}";
  var dark = function () {{
    var t = document.documentElement.getAttribute('data-theme');
    return t === 'dark' || (t !== 'light' && window.matchMedia('(prefers-color-scheme: dark)').matches);
  }};
  var push = function (f) {{ if (f && f.contentWindow) f.contentWindow.postMessage({{ forgeTheme: dark() ? 'dark' : 'light' }}, '*'); }};
  window.__forgeUiHandlers = window.__forgeUiHandlers || {{}};
  window.__forgeUiHandlers[id] = function (raw) {{ dioxus.send(raw); }};
  if (!window.__forgeUiBridge) {{
    window.__forgeUiBridge = true;
    window.addEventListener('message', function (e) {{
      if (!e.data || typeof e.data !== 'object' || typeof e.data.n !== 'string') return;
      var frames = document.querySelectorAll('iframe.generated-ui-frame');
      for (var i = 0; i < frames.length; i++) {{
        if (frames[i].contentWindow === e.source) {{
          if (document.activeElement !== frames[i]) return;
          var handler = window.__forgeUiHandlers[frames[i].dataset.id];
          if (handler) handler(JSON.stringify(e.data));
          return;
        }}
      }}
    }});
    new MutationObserver(function () {{
      document.querySelectorAll('iframe.generated-ui-frame').forEach(push);
    }}).observe(document.documentElement, {{ attributes: true, attributeFilter: ['data-theme'] }});
  }}
  var frame = document.getElementById("gui-" + id);
  if (frame) {{ frame.addEventListener('load', function () {{ push(frame); }}); push(frame); }}
}})();"#
    )
}

/// An interactive page Claude drew. Without `on_send` (an archived or view-only session) the page is drawn but
/// whatever it sends is dropped.
#[component]
pub fn GeneratedUiView(ui: GeneratedUi, on_send: Option<EventHandler<String>>) -> Element {
    let nonce = use_hook(fresh_nonce);
    let last_sent = use_hook(|| Rc::new(Cell::new(None::<Instant>)));
    let id = dom_id(&ui.id);
    {
        let nonce = nonce.clone();
        let id = id.clone();
        use_future(move || {
            let nonce = nonce.clone();
            let id = id.clone();
            let last_sent = last_sent.clone();
            async move {
                let mut incoming = document::eval(&bridge_watch(&id));
                while let Ok(raw) = incoming.recv::<String>().await {
                    let Some(send) = on_send else { continue };
                    if last_sent.get().is_some_and(|at| at.elapsed() < SEND_GAP) {
                        continue;
                    }
                    if let Some(text) = bridge_message(&raw, &nonce) {
                        last_sent.set(Some(Instant::now()));
                        send.call(text);
                    }
                }
            }
        });
    }
    let height = ui.height;
    match &ui.state {
        UiState::Pending => rsx! {
            div { class: "generated-ui pending", style: "min-height: {height}px",
                if ui.placeholders.is_empty() {
                    div { class: "generated-ui-line", {tr!("Drawing the page…")} }
                }
                for (index, line) in ui.placeholders.iter().enumerate() {
                    div { key: "{index}", class: "generated-ui-line", "{line}" }
                }
            }
        },
        UiState::Failed(reason) => rsx! {
            div { class: "generated-ui failed",
                div { class: "generated-ui-line", {tr!("The page could not be drawn")} }
                if !reason.is_empty() {
                    div { class: "generated-ui-reason", "{reason}" }
                }
            }
        },
        UiState::Ready => {
            let document = page_document(&ui, &nonce);
            rsx! {
                div { class: "generated-ui",
                    iframe {
                        class: "generated-ui-frame",
                        id: "gui-{id}",
                        "data-id": "{id}",
                        title: tr!("Interactive page"),
                        srcdoc: "{document}",
                        "sandbox": "allow-scripts",
                        style: "height: {height}px",
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_id_keeps_only_safe_characters() {
        assert_eq!(dom_id("toolu_01AbC-9"), "toolu_01AbC-9");
        assert_eq!(dom_id("a\"b');alert(1)//"), "a_b___alert_1___");
    }

    #[test]
    fn the_watcher_takes_messages_only_from_a_focused_frame_of_ours() {
        let script = bridge_watch("x");
        assert!(script.contains("contentWindow === e.source"));
        assert!(script.contains("document.activeElement !== frames[i]"));
        assert!(script.contains("iframe.generated-ui-frame"));
        assert!(script.contains("forgeTheme"));
    }

    #[test]
    fn nonces_differ() {
        assert_ne!(fresh_nonce(), fresh_nonce());
        assert_eq!(fresh_nonce().len(), 32);
    }
}
