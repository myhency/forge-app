use dioxus::prelude::*;
use forge_features::{use_settings_provider, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, SettingsOverlay, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    let workspace = use_workspace_provider();
    // Key events only reach the root while something inside it has focus. Whenever the overlay
    // closes or the main pane switches view, the focused input or button may have been removed
    // and focus falls to <body>, which silences every shortcut until the next click. Give focus
    // back to the root — but only if nothing has it, so a composer the person is typing in keeps it.
    use_effect(move || {
        let _ = (settings.is_open(), workspace.active());
        document::eval(
            "setTimeout(() => { const a = document.activeElement; if (!a || a === document.body) { document.querySelector('.workspace')?.focus(); } }, 0);",
        );
    });

    // The tabs list the definitions of the project the person is looking at.
    use_effect(move || settings.set_project(workspace.context_project()));

    rsx! {
        div {
            class: "workspace",
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                if !workspace.on_key(&event, settings.is_open()) {
                    settings.on_key(&event);
                }
            },
            Sidebar {}
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            SettingsOverlay {}
        }
    }
}
