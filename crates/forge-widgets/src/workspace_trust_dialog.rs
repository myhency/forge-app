use dioxus::prelude::*;
use forge_features::{refocus_workspace, use_workspace};
use forge_shared::{tr, Button, ButtonKind};

/// The trust question of `폴더 변경…` for a folder off the trusted list. Renders nothing while no
/// change is waiting; Escape or a click outside cancels.
#[component]
pub fn WorkspaceTrustDialog() -> Element {
    let workspace = use_workspace();
    let Some(request) = workspace.trust_request() else {
        return rsx! {};
    };
    let answer = move |trust: bool| {
        workspace.answer_trust_request(trust);
        refocus_workspace();
    };
    rsx! {
        div {
            class: "settings-backdrop",
            onclick: move |_| answer(false),
            div {
                class: "env-ask",
                role: "alertdialog",
                tabindex: "0",
                "aria-label": tr!("Do you trust this workspace?"),
                // Key events reach the dialog only while it has focus.
                onmounted: move |mounted| async move {
                    let _ = mounted.set_focus(true).await;
                },
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event: KeyboardEvent| {
                    if event.key() == Key::Escape {
                        event.stop_propagation();
                        answer(false);
                    }
                },
                h3 { {tr!("Do you trust this workspace?")} }
                p { class: "trust-path", title: "{request.folder.display()}", "{request.folder.display()}" }
                div { class: "env-buttons",
                    span { class: "spacer" }
                    Button { label: tr!("Cancel"), onclick: move |_| answer(false) }
                    Button { label: tr!("Trust workspace"), kind: ButtonKind::Primary, onclick: move |_| answer(true) }
                }
            }
        }
    }
}
