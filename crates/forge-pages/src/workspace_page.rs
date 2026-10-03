use dioxus::prelude::*;
use forge_features::{refocus_workspace, use_settings_provider, use_workspace_provider, Active};
use forge_widgets::{ChatWindow, NewSessionPanel, SearchPalette, SettingsOverlay, Sidebar};

/// The whole screen: the sidebar on the left and, on the right, either the new-session panel or
/// the active session. The root also catches the session shortcuts.
#[component]
pub fn WorkspacePage() -> Element {
    let settings = use_settings_provider();
    let workspace = use_workspace_provider();
    let mut dragging = use_signal(|| false);
    // Key events only reach the root while something inside it has focus. Whenever the overlay
    // closes or the main pane switches view, the focused input or button may have been removed
    // and focus falls to <body>, which silences every shortcut until the next click. Give focus
    // back to the root — but only if nothing has it, so a composer the person is typing in keeps it.
    use_effect(move || {
        let _ = (settings.is_open(), workspace.search_open(), workspace.active());
        refocus_workspace();
    });

    // The tabs list the definitions of the project the person is looking at.
    use_effect(move || settings.set_project(workspace.context_project()));

    let hidden = workspace.prefs().hidden;

    rsx! {
        div {
            class: if dragging() { "workspace resizing" } else { "workspace" },
            tabindex: "0",
            // Key events only reach the root while something inside it has focus.
            onmounted: move |mounted| async move {
                let _ = mounted.set_focus(true).await;
            },
            onkeydown: move |event| {
                // The palette is an overlay too: the session shortcuts wait while it is open.
                if !workspace.on_key(&event, settings.is_open() || workspace.search_open()) {
                    settings.on_key(&event);
                }
            },
            // The sidebar's width follows the pointer while the handle is held; it is saved once, on release.
            onmousemove: move |event| {
                if dragging() {
                    let x = event.client_coordinates().x.max(0.0) as u32;
                    workspace.change_prefs_unsaved(|prefs| prefs.width = x.clamp(forge_domain::MIN_SIDEBAR_WIDTH, forge_domain::MAX_SIDEBAR_WIDTH));
                }
            },
            onmouseup: move |_| {
                if dragging() {
                    dragging.set(false);
                    workspace.save_prefs();
                }
            },
            if hidden {
                div { class: "sidebar-rail",
                    button {
                        class: "icon-button",
                        title: "Show the sidebar",
                        onclick: move |_| workspace.update_prefs(|prefs| prefs.hidden = false),
                        "›"
                    }
                }
            } else {
                Sidebar {}
                div {
                    class: if dragging() { "sidebar-resize dragging" } else { "sidebar-resize" },
                    onmousedown: move |event| {
                        event.prevent_default();
                        dragging.set(true);
                    },
                }
            }
            match workspace.active() {
                Active::NewSession => rsx! { NewSessionPanel {} },
                Active::Live(id) => rsx! { ChatWindow { key: "{id:?}", id } },
            }
            SettingsOverlay {}
            SearchPalette {}
        }
    }
}
