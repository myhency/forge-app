use dioxus::prelude::*;
use forge_features::{use_settings_provider, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    let workspace = use_workspace_provider();

    rsx! {
        div {
            class: "workspace",
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                if !workspace.on_key(&event) {
                    settings.on_key(&event);
                }
            },
            Sidebar {}
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
        }
    }
}
