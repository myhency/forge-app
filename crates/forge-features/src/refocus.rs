use dioxus::prelude::*;

/// Gives focus back to the workspace root, but only if nothing has it. Key events reach the root
/// only while something inside it is focused; when the focused element unmounts, focus falls to
/// `<body>` and every shortcut stays silent until the next click. A composer being typed in keeps
/// its focus.
pub fn refocus_workspace() {
    document::eval(
        "setTimeout(() => { const a = document.activeElement; if (!a || a === document.body) { document.querySelector('.workspace')?.focus(); } }, 0);",
    );
}

/// Puts the cursor back in the message box. For a menu that opened from the composer and closed:
/// the person was typing there and goes on.
pub fn refocus_composer() {
    document::eval(&format!("setTimeout(() => {{ document.querySelector('{COMPOSER_TEXTAREA}')?.focus(); }}, 0);"));
}

// The message box of an open session, and the one on the new-session screen.
const COMPOSER_TEXTAREA: &str = ".composer textarea, .new-session .composer-box textarea";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_composer_selector_reaches_the_box_of_a_session_and_the_box_of_the_new_session_screen() {
        assert!(COMPOSER_TEXTAREA.contains(".composer textarea"));
        assert!(COMPOSER_TEXTAREA.contains(".new-session .composer-box textarea"));
    }
}
