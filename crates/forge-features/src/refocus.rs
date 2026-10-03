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
    document::eval("setTimeout(() => { document.querySelector('.composer textarea')?.focus(); }, 0);");
}
