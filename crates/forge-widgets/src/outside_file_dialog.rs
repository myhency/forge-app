use std::path::Path;

use dioxus::prelude::*;
use forge_features::offers_default_program;
use forge_features::{refocus_workspace, use_workspace};
use forge_shared::{tr, Button, ButtonKind};

/// The note for a file outside the session folder: its name and what to do with it. The default
/// program is not offered for a program. Renders nothing while no such file is asked for; Escape or
/// a click outside closes it.
#[component]
pub fn OutsideFileDialog() -> Element {
    let workspace = use_workspace();
    let Some(path) = workspace.outside_file() else {
        return rsx! {};
    };
    let name = Path::new(&path).file_name().map_or_else(|| path.clone(), |name| name.to_string_lossy().to_string());
    let close = move || {
        workspace.close_outside_file();
        refocus_workspace();
    };
    let (open_path, reveal_path) = (path.clone(), path.clone());
    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| close(),
            div {
                class: "env-ask",
                role: "alertdialog",
                tabindex: "0",
                "aria-label": tr!("This file is outside the session folder."),
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        close();
                    }
                },
                h3 { {tr!("This file is outside the session folder.")} }
                p { class: "outside-path", title: "{path}", "{name}" }
                div { class: "env-buttons",
                    span { class: "spacer" }
                    if offers_default_program(&path) {
                        Button {
                            label: tr!("Open in the default program"),
                            kind: ButtonKind::Primary,
                            onclick: move |_| {
                                workspace.open_with_app(Path::new(&open_path), None);
                                close();
                            },
                        }
                    }
                    Button {
                        label: forge_features::show_in_file_manager(),
                        onclick: move |_| {
                            workspace.reveal(Path::new(&reveal_path));
                            close();
                        },
                    }
                    Button { label: tr!("Close"), onclick: move |_| close() }
                }
            }
        }
    }
}
