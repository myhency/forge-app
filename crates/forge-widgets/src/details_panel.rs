use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_features::Panel;

use crate::panel_frame::PanelFrame;

#[component]
pub fn DetailsPanel(id: LiveSessionId) -> Element {
    rsx! {
        PanelFrame { id, panel: Panel::Details, tools: rsx! {}, div { class: "details-body" } }
    }
}
