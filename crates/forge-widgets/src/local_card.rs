use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_entities::{CardBlock, LocalCard, RingLevel};
use forge_features::use_workspace;
use forge_shared::{tr, Icon, IconKind};

/// The cards `/status` and `/usage` put at the end of a session's chat. A new message takes them away.
#[component]
pub fn LocalCards(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let cards = workspace.local_cards(id);
    // A card that has just been added is brought into view, under the message box's reach.
    let count = cards.len();
    use_effect(use_reactive!(|count| {
        if count > 0 {
            document::eval("requestAnimationFrame(() => document.querySelector('.pane.focused .local-card:last-child')?.scrollIntoView({ block: 'end', behavior: 'smooth' }));");
        }
    }));
    if cards.is_empty() {
        return rsx! {};
    }
    rsx! {
        div { class: "local-cards",
            for (index, card) in cards.into_iter().enumerate() {
                LocalCardView { key: "{index}", id, index, card }
            }
        }
    }
}

#[component]
fn LocalCardView(id: LiveSessionId, index: usize, card: LocalCard) -> Element {
    // What the copy button puts on the clipboard: the card as plain text, one `label  value` line each.
    let text = plain_text(&card);
    let mut copied = use_signal(|| false);
    rsx! {
        section { class: "local-card", "aria-label": "{card.title}",
            header { class: "local-card-head",
                h3 { "{card.title}" }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Copy"),
                    "aria-label": tr!("Copy"),
                    onclick: move |_| {
                        forge_entities::copy_text(&text);
                        copied.set(true);
                    },
                    Icon { kind: if copied() { IconKind::Check } else { IconKind::Copy }, size: forge_shared::IconSize::Small }
                }
            }
            for (at, block) in card.blocks.into_iter().enumerate() {
                match block {
                    CardBlock::Heading(text) => rsx! { div { key: "{at}", class: "usage-heading local-card-heading", "{text}" } },
                    CardBlock::Rows(rows) => rsx! {
                        div { key: "{at}", class: "local-card-rows",
                            for (row_at, (label, value)) in rows.into_iter().enumerate() {
                                div { key: "{row_at}", class: "usage-row",
                                    span { "{label}" }
                                    span { class: "value local-card-value", title: "{value}", "{value}" }
                                }
                            }
                        }
                    },
                    CardBlock::Meter { label, percent, level, note } => rsx! {
                        div { key: "{at}", class: "local-card-meter",
                            div { class: "usage-row", span { "{label}" } span { class: "value", "{percent}%" } }
                            div {
                                class: "meter-track",
                                role: "progressbar",
                                "aria-label": "{label}",
                                "aria-valuemin": "0",
                                "aria-valuemax": "100",
                                "aria-valuenow": "{percent}",
                                div { class: "meter-fill {meter_class(level)}", style: "width: {percent}%" }
                            }
                            div { class: "usage-note", "{note}" }
                        }
                    },
                    CardBlock::Note(text) => rsx! { div { key: "{at}", class: "usage-note", "{text}" } },
                }
            }
        }
    }
}

fn meter_class(level: RingLevel) -> &'static str {
    level.class()
}

fn plain_text(card: &LocalCard) -> String {
    let mut lines = vec![card.title.clone()];
    for block in &card.blocks {
        match block {
            CardBlock::Heading(text) => lines.push(format!("\n{text}")),
            CardBlock::Rows(rows) => lines.extend(rows.iter().map(|(label, value)| format!("{label}: {value}"))),
            CardBlock::Meter { label, percent, note, .. } => lines.push(format!("{label}: {percent}% ({note})")),
            CardBlock::Note(text) => lines.push(text.clone()),
        }
    }
    lines.join("\n")
}
