use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::new_session_controls::{AddFolderButton, FolderChip, NewModePicker, NewModelPicker};
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
                chips: rsx! {
                    span { class: "chip", "Local" }
                    FolderChip {}
                    AddFolderButton {}
                },
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
                NewModePicker {}
                span { class: "spacer" }
                NewModelPicker {}
            }
        }
    }
}
