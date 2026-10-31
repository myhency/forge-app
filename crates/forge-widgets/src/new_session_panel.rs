use dioxus::prelude::*;
use forge_features::{use_workspace, user_name, NewSessionComposer};

use crate::environment_chip::EnvironmentChip;
use crate::new_session_controls::{AddFolderButton, BranchChip, FolderChip, NewEffortPicker, NewModePicker, NewModelPicker};
use crate::plus_menu::PlusMenu;
use crate::recent_folders_card::RecentFoldersCard;
use crate::usage_card::UsageCard;
use forge_shared::tr;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    let name = user_name(std::env::var("USER").ok().as_deref(), std::env::var("USERNAME").ok().as_deref());
    rsx! {
        div { class: "main-pane new-session-pane",
            div { class: "home-top",
                h1 { class: "greeting", {tr!("{name}, what's next?", name = name)} }
                UsageCard {}
            }
            NewSessionComposer {
                above: rsx! { RecentFoldersCard {} },
                leading: rsx! { PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true } },
                start: rsx! {
                    EnvironmentChip {}
                    FolderChip {}
                    BranchChip {}
                    AddFolderButton {}
                },
                end: rsx! {
                    NewModelPicker {}
                    NewEffortPicker {}
                    NewModePicker {}
                },
            }
        }
    }
}
