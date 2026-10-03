use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::plus_menu::PlusMenu;
use crate::usage_card::UsageCard;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    rsx! {
        div { class: "main-pane",
            div { class: "home-top",
                h1 { class: "greeting", "What's next?" }
                UsageCard {}
            }
            NewSessionComposer {
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
            }
        }
    }
}
