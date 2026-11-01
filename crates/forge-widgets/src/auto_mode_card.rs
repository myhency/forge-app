use dioxus::prelude::*;
use forge_domain::PermissionMode;
use forge_features::{auto_mode_offered, refocus_composer, use_new_session_draft};
use forge_shared::{tr, Icon, IconKind};

/// A one-row card under the environment card of the new-session screen: "Turn on Auto mode" sets the
/// mode of the box in one press, the same choice as the mode picker's first row. It goes away once the
/// mode is Auto, and is not shown for a mode the person chose on purpose.
#[component]
pub fn AutoModeCard() -> Element {
    let mut draft = use_new_session_draft();
    if !auto_mode_offered(*draft.mode.read()) {
        return rsx! {};
    }
    rsx! {
        div { class: "env-shortcut auto-mode-card", role: "menu",
            button {
                class: "picker-item",
                r#type: "button",
                role: "menuitem",
                onclick: move |_| {
                    draft.mode.set(PermissionMode::Auto);
                    refocus_composer();
                },
                Icon { kind: IconKind::FastForward }
                span { class: "picker-text", {tr!("Turn on Auto mode")} }
            }
        }
    }
}
