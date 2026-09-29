use dioxus::prelude::*;
use forge_features::NewSessionComposer;

#[component]
pub fn NewSessionPanel() -> Element {
    rsx! {
        div { class: "main-pane",
            div { class: "pane-header", "New session" }
            NewSessionComposer {}
        }
    }
}
