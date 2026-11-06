use dioxus::prelude::*;
use forge_features::{use_new_session_draft_provider, use_workspace, user_name, NewSessionComposer};

use crate::environment_chip::EnvironmentChip;
use crate::new_session_controls::{FolderChip, WorktreeCheckbox, NewModePicker, NewModelPicker};
use crate::plus_menu::PlusMenu;
use crate::auto_mode_card::AutoModeCard;
use crate::offer_chips::OfferChips;
use crate::environment_card::EnvironmentCard;
use forge_shared::tr;

#[component]
pub fn NewSessionPanel() -> Element {
    let workspace = use_workspace();
    use_new_session_draft_provider();
    let name = user_name(std::env::var("USER").ok().as_deref(), std::env::var("USERNAME").ok().as_deref());
    rsx! {
        div { class: "main-pane new-session-pane",
            div { class: "home-top",
                h1 { class: "greeting", {tr!("{name}, what's next?", name = name)} }
            }
            div { class: "env-shortcut-slot",
                div { class: "env-shortcut-stack", EnvironmentCard {} AutoModeCard {} OfferChips {} }
            }
            NewSessionComposer {
                leading: rsx! { PlusMenu { project: workspace.chosen_new_folder(), add_folder_enabled: true } },
                start: rsx! {
                    EnvironmentChip {}
                    FolderChip {}
                    WorktreeCheckbox {}
                },
                end: rsx! {
                    NewModelPicker {}
                    NewModePicker {}
                },
            }
        }
    }
}
