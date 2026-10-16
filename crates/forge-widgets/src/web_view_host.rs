use dioxus::prelude::*;
use forge_features::use_workspace;
use forge_ports::{KeyForward, Surface, WebViewEventKind};

// Reports the Preview surface rectangle when it changes: one message per change, empty when there
// is no surface. A new run takes over from an older one (`dioxus.send` belongs to its eval).
const SURFACE_SCRIPT: &str = r#"
const token = (window.__forgeSurfaceToken = {});
let last = null;
function tick() {
  if (window.__forgeSurfaceToken !== token) return;
  const el = document.querySelector('.preview-surface');
  const r = el ? el.getBoundingClientRect() : null;
  const text = r ? [r.x, r.y, r.width, r.height].join(',') : '';
  if (text !== last) { last = text; dioxus.send(text); }
  requestAnimationFrame(tick);
}
tick();
"#;

// Reports whether any backdrop (dialog, palette, lightbox) or a menu without one is in the document.
const OVERLAY_SCRIPT: &str = r#"
if (window.__forgeOverlayObserver) { window.__forgeOverlayObserver.disconnect(); }
let last = null;
function check() {
  const open = document.querySelector('[class*="backdrop"], .row-menu, .perm-menu') !== null;
  if (open !== last) { last = open; dioxus.send(open); }
}
window.__forgeOverlayObserver = new MutationObserver(check);
window.__forgeOverlayObserver.observe(document.body, { subtree: true, childList: true, attributes: true, attributeFilter: ['class'] });
check();
"#;

fn parse_surface(text: &str) -> Option<Surface> {
    let numbers: Vec<f64> = text.split(',').map(|part| part.parse::<f64>()).collect::<Result<_, _>>().ok()?;
    match numbers.as_slice() {
        [x, y, width, height] if numbers.iter().all(|n| n.is_finite()) => Some(Surface { x: *x, y: *y, width: *width, height: *height }),
        _ => None,
    }
}

// The chord as a `keydown` on Forge's own page root, which is where its shortcuts are read. The
// text is quoted as JSON, so what a page sends can never leave the string.
fn key_script(key: &KeyForward) -> String {
    let quote = |text: &str| serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        "(function(){{var t=document.querySelector('.workspace')||document.body;t.focus();\
         t.dispatchEvent(new KeyboardEvent('keydown',{{key:{},code:{},ctrlKey:{},metaKey:{},altKey:{},shiftKey:{},bubbles:true,cancelable:true}}));}})();",
        quote(&key.key),
        quote(&key.code),
        key.ctrl,
        key.meta,
        key.alt,
        key.shift
    )
}

/// Draws nothing. It keeps the Preview web views in step with the page: where the surface is,
/// whether an overlay is open, what the web views report, and which session and panel are on screen.
#[component]
pub fn WebViewHost() -> Element {
    let workspace = use_workspace();
    use_future(move || async move {
        let mut reports = document::eval(SURFACE_SCRIPT);
        while let Ok(text) = reports.recv::<String>().await {
            workspace.set_surface(parse_surface(&text));
            workspace.sync_web_views();
        }
    });
    use_future(move || async move {
        let mut reports = document::eval(OVERLAY_SCRIPT);
        while let Ok(open) = reports.recv::<bool>().await {
            workspace.set_overlay_open(open);
            workspace.sync_web_views();
        }
    });
    use_future(move || async move {
        let Some(mut events) = workspace.webviews().take_events() else { return };
        while let Some(event) = events.recv().await {
            // A chord pressed in a page that is one of Forge's shortcuts: the keyboard goes back to
            // the app and the key is played again on its page.
            if let WebViewEventKind::Key(key) = &event.kind {
                match forge_features::forwarded_shortcut(key, cfg!(target_os = "macos")) {
                    // A browser key pressed in the page acts on the Preview panel of the session on screen.
                    Some(forge_features::Shortcut::Preview(browser_key)) => {
                        if let forge_features::Active::Live(id) = workspace.active() {
                            if browser_key != forge_features::PreviewKey::Reload {
                                workspace.webviews().focus_app(event.tab);
                            }
                            crate::preview_panel::run_preview_key(workspace, id, browser_key);
                        }
                    }
                    Some(_) => {
                        workspace.webviews().focus_app(event.tab);
                        document::eval(&key_script(key));
                    }
                    None => {}
                }
                continue;
            }
            workspace.apply_web_view_event(event);
        }
    });
    // A session switch or a panel toggle changes what may be seen.
    use_effect(move || {
        let _ = (workspace.active(), workspace.active_dock_open_preview());
        workspace.sync_web_views();
    });
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reported_rectangle_is_read_and_an_empty_report_means_no_surface() {
        assert_eq!(
            parse_surface("10.5,20,400,300.25"),
            Some(Surface { x: 10.5, y: 20.0, width: 400.0, height: 300.25 })
        );
        for bad in ["", "1,2,3", "a,b,c,d", "1,2,3,4,5", "NaN,0,1,1"] {
            assert_eq!(parse_surface(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn each_script_replaces_its_old_self_so_a_new_page_scope_takes_over() {
        // `dioxus.send` belongs to the eval that started the script.
        assert!(SURFACE_SCRIPT.contains("__forgeSurfaceToken"));
        assert!(SURFACE_SCRIPT.contains("requestAnimationFrame"));
        assert!(OVERLAY_SCRIPT.contains("__forgeOverlayObserver.disconnect()"));
        assert!(OVERLAY_SCRIPT.contains("[class*=\"backdrop\"]"));
        // Menus that have no backdrop of their own lie over the panel just the same.
        assert!(OVERLAY_SCRIPT.contains(".row-menu"));
        assert!(OVERLAY_SCRIPT.contains(".perm-menu"));
    }

    #[test]
    fn the_key_script_carries_the_chord_and_quotes_the_text() {
        let script = key_script(&KeyForward { key: "k".into(), code: "KeyK".into(), ctrl: false, meta: true, alt: false, shift: false });
        assert!(script.contains("key:\"k\""), "{script}");
        assert!(script.contains("metaKey:true"));
        assert!(script.contains(".workspace"));
        let hostile = key_script(&KeyForward { key: "\"});alert(1);//".into(), code: "x".into(), ctrl: false, meta: false, alt: false, shift: false });
        assert!(hostile.contains("key:\"\\\"});alert(1);//\""), "the quote is escaped: {hostile}");
    }
}
