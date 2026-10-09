use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::{Card, CardKind};
use forge_entities::{modal_title, CardView};

use crate::workspace::use_workspace;
use forge_shared::tr;

/// The unsent cards above the text. An excerpt opens a modal with its lines; a comment asks the
/// Files panel to show its box.
#[component]
pub fn ComposerCards(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let mut shown = use_signal(|| None::<Card>);
    let cards = workspace.cards(id);
    if cards.is_empty() && shown.read().is_none() {
        return rsx! {};
    }
    rsx! {
        if !cards.is_empty() {
            div { class: "cards-row composer-cards",
                for card in cards {
                    CardView {
                        key: "{card.id}",
                        card: card.clone(),
                        on_click: {
                            let card = card.clone();
                            move |_| match card.kind {
                                CardKind::Excerpt => shown.set(Some(card.clone())),
                                CardKind::Comment => workspace.request_comment_focus(id, card.id),
                            }
                        },
                        on_remove: {
                            let card_id = card.id;
                            move |_| workspace.remove_card(id, card_id)
                        },
                    }
                }
            }
        }
        if let Some(card) = shown() {
            div { class: "picker-backdrop", onclick: move |_| shown.set(None) }
            div { class: "card-modal", role: "dialog",
                div { class: "card-modal-head",
                    span { class: "card-modal-title", "{modal_title(&card)}" }
                    button { class: "card-remove always", r#type: "button", "aria-label": tr!("Close"), onclick: move |_| shown.set(None), "×" }
                }
                div { class: "card-modal-note", {tr!("{size} • May not match the original formatting", size = card.size_label())} }
                pre { class: "card-modal-code", "{card.text}" }
            }
        }
    }
}
