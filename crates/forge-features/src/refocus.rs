use dioxus::prelude::*;

/// Gives focus back to the workspace root, but only if nothing has it. Key events reach the root
/// only while something inside it is focused; when the focused element unmounts, focus falls to
/// `<body>` and every shortcut stays silent until the next click. A composer being typed in keeps
/// its focus.
pub fn refocus_workspace() {
    document::eval(&format!("setTimeout(() => {{ {FOCUS_IF_NOTHING_HAS_IT} }}, 0);"));
}

// The one rule for "nothing has focus": the root takes it back only from <body> (or nothing).
const FOCUS_IF_NOTHING_HAS_IT: &str =
    "const a = document.activeElement; if (!a || a === document.body) { document.querySelector('.workspace')?.focus(); }";

/// Installs, once per page, a watcher that gives focus back to the workspace root whenever an
/// element that had it is removed (a closed panel, a row that left rename mode) and focus falls to
/// `<body>`. Call sites of `refocus_workspace` run before Dioxus removes the element and see the old
/// focus, so this watches the result instead: DOM changes and `focusout`, one check per frame.
pub fn watch_focus() {
    document::eval(&format!(
        "if (!window.__forgeFocusWatch) {{ window.__forgeFocusWatch = true; let queued = false; \
         const check = () => {{ if (queued) return; queued = true; requestAnimationFrame(() => {{ queued = false; {FOCUS_IF_NOTHING_HAS_IT} }}); }}; \
         new MutationObserver(check).observe(document.body, {{ childList: true, subtree: true }}); \
         document.addEventListener('focusout', () => setTimeout(check, 0)); }}"
    ));
}

/// Puts the cursor back in the message box. For a menu that opened from the composer and closed:
/// the person was typing there and goes on.
pub fn refocus_composer() {
    document::eval(&format!("setTimeout(() => {{ document.querySelector('{COMPOSER_TEXTAREA}')?.focus(); }}, 0);"));
}

// The message box of an open session, and the one on the new-session screen.
const COMPOSER_TEXTAREA: &str = ".pane.focused .composer textarea, .pane.focused .new-session .composer-box textarea";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_focus_rule_only_acts_when_nothing_or_body_has_focus() {
        assert!(FOCUS_IF_NOTHING_HAS_IT.contains("!a || a === document.body"));
        assert!(FOCUS_IF_NOTHING_HAS_IT.contains("'.workspace'"));
    }

    #[test]
    fn the_composer_selector_reaches_the_box_of_a_session_and_the_box_of_the_new_session_screen() {
        assert!(COMPOSER_TEXTAREA.contains(".composer textarea"));
        assert!(COMPOSER_TEXTAREA.contains(".new-session .composer-box textarea"));
    }

    #[test]
    fn the_composer_selector_is_limited_to_the_focused_pane() {
        assert!(COMPOSER_TEXTAREA.starts_with(".pane.focused .composer textarea"));
        assert!(COMPOSER_TEXTAREA.contains(".pane.focused .new-session .composer-box textarea"));
    }
}
