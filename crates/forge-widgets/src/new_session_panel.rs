use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::plus_menu::PlusMenu;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    rsx! {
        div { class: "main-pane",
            div { class: "pane-header", "New session" }
            NewSessionComposer {
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
            }
        }
    }
}
