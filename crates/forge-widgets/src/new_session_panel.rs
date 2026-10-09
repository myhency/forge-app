use dioxus::prelude::*;
use forge_features::{use_workspace, NewSessionComposer};

use crate::new_session_controls::{AddFolderButton, FolderChip, NewEffortPicker, NewModePicker, NewModelPicker};
use crate::plus_menu::PlusMenu;
use crate::usage_card::UsageCard;
use forge_shared::tr;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    rsx! {
        div { class: "main-pane",
            div { class: "home-top",
                h1 { class: "greeting", {tr!("What's next?")} }
                UsageCard {}
            }
            NewSessionComposer {
                chips: rsx! {
                    FolderChip {}
                    AddFolderButton {}
                },
                PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true }
                NewModePicker {}
                span { class: "spacer" }
                NewModelPicker {}
                NewEffortPicker {}
            }
        }
    }
}
