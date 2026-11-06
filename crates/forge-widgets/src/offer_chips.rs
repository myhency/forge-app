use dioxus::prelude::*;
use forge_features::use_workspace;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// The proposals open sessions are waiting on, as the desktop app lists them on its new-session screen
/// (observed 2026-11-06): pressing one puts its brief in the box here, in its session's folder; `×` dismisses it.
#[component]
pub fn OfferChips() -> Element {
    let workspace = use_workspace();
    let offers: Vec<_> = workspace.pending_offers().into_iter().take(3).collect();
    if offers.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "env-shortcut offer-chips", role: "menu",
            for (id, offer) in offers {
                div { key: "{offer.id}", class: "offer-chip",
                    button {
                        class: "picker-item",
                        r#type: "button",
                        role: "menuitem",
                        title: "{offer.lead}",
                        onclick: {
                            let offer = offer.clone();
                            move |_| {
                                workspace.draft_offer(id, offer.clone());
                                forge_features::refocus_composer();
                            }
                        },
                        span { class: "picker-text", "{offer.title}" }
                    }
                    button {
                        class: "icon-button offer-chip-close",
                        r#type: "button",
                        title: tr!("Dismiss"),
                        "aria-label": tr!("Dismiss"),
                        onclick: {
                            let offer = offer.clone();
                            move |_| workspace.act_on_offer(id, offer.clone(), forge_entities::OfferAction::Dismiss)
                        },
                        Icon { kind: IconKind::Close, size: IconSize::Small }
                    }
                }
            }
        }
    }
}
