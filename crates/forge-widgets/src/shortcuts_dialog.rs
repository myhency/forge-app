use dioxus::prelude::*;
use forge_features::{refocus_workspace, shortcut_sections, use_workspace};

/// The list of keyboard shortcuts (⌘/): every key Forge binds, from the same table the key handler
/// reads. Renders nothing while closed; Escape or the close button puts it away.
#[component]
pub fn ShortcutsDialog() -> Element {
    let workspace = use_workspace();
    if !workspace.shortcuts_open() {
        return rsx! {};
    }
    let sections = shortcut_sections(cfg!(target_os = "macos"));
    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| {
                workspace.close_shortcuts();
                refocus_workspace();
            },
            div {
                class: "shortcuts-panel",
                role: "dialog",
                "aria-label": "Keyboard shortcuts",
                // A click inside the panel must not reach the backdrop and close it.
                onclick: move |event| event.stop_propagation(),
                button {
                    class: "settings-close",
                    title: "Close (Esc)",
                    onclick: move |_| {
                        workspace.close_shortcuts();
                        refocus_workspace();
                    },
                    "✕"
                }
                h2 { "Keyboard shortcuts" }
                for section in sections {
                    div { key: "{section.title}",
                        h3 { "{section.title}" }
                        for (label, keys) in section.rows {
                            div { class: "shortcut-row", key: "{label}",
                                span { "{label}" }
                                span { class: "shortcut-keys", "{keys}" }
                            }
                        }
                    }
                }
            }
        }
    }
}
