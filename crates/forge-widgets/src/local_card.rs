use dioxus::prelude::*;
use forge_application::LiveSessionId;
use forge_domain::StatsRange;
use forge_entities::{stats_range_label, CardBlock, LocalCard, RingLevel, STATS_RANGES};
use forge_features::use_workspace;
use forge_shared::{tr, Icon, IconKind};

/// The cards `/status` and `/usage` put at the end of a session's chat. A new message takes them away.
// A key for the card of `/stats`, which is not in the list of the others.
const STATS_KEY: &str = "stats";

#[component]
pub fn LocalCards(id: LiveSessionId) -> Element {
    let workspace = use_workspace();
    let cards = workspace.local_cards(id);
    let stats = workspace.stats_card_view(id);
    // A card that has just been added is brought into view, under the message box's reach.
    let count = cards.len() + usize::from(stats.is_some());
    use_effect(use_reactive!(|count| {
        if count > 0 {
            document::eval("requestAnimationFrame(() => document.querySelector('.pane.focused .local-card:last-child')?.scrollIntoView({ block: 'end', behavior: 'smooth' }));");
        }
    }));
    if cards.is_empty() && stats.is_none() {
        return rsx! {};
    }
    rsx! {
        div { class: "local-cards",
            for (index, card) in cards.into_iter().enumerate() {
                LocalCardView { key: "{index}", id, card, range: None }
            }
            if let Some((card, range)) = stats {
                LocalCardView { key: "{STATS_KEY}", id, card, range: Some(range) }
            }
        }
    }
}

#[component]
fn LocalCardView(id: LiveSessionId, card: LocalCard, range: Option<StatsRange>) -> Element {
    let workspace = use_workspace();
    // What the copy button puts on the clipboard: the card as plain text, one `label  value` line each.
    let text = plain_text(&card);
    let mut copied = use_signal(|| false);
    rsx! {
        section { class: "local-card", "aria-label": "{card.title}",
            header { class: "local-card-head",
                h3 { "{card.title}" }
                if let Some(current) = range {
                    div { class: "local-card-range", role: "radiogroup", "aria-label": tr!("Range"),
                        for choice in STATS_RANGES {
                            button {
                                key: "{stats_range_label(choice)}",
                                class: if choice == current { "range-option selected" } else { "range-option" },
                                role: "radio",
                                "aria-checked": "{choice == current}",
                                r#type: "button",
                                onclick: move |_| workspace.set_stats_range(id, choice),
                                {stats_range_label(choice)}
                            }
                        }
                    }
                }
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
                    CardBlock::Loading => rsx! { div { key: "{at}", class: "usage-note", role: "status", {tr!("Reading every session…")} } },
                    CardBlock::Tiles(tiles) => rsx! {
                        div { key: "{at}", class: "stat-tiles",
                            for (tile_at, (label, value)) in tiles.into_iter().enumerate() {
                                div { key: "{tile_at}", class: "stat-tile",
                                    div { class: "stat-tile-label", "{label}" }
                                    div { class: "stat-tile-value", title: "{value}", "{value}" }
                                }
                            }
                        }
                    },
                    CardBlock::Heatmap { rows, months } => rsx! {
                        div { key: "{at}", class: "stat-heat", role: "img", "aria-label": tr!("Use per day, the last weeks"),
                            div { class: "stat-heat-months",
                                for (month_at, month) in months.into_iter().enumerate() {
                                    span { key: "{month_at}", class: "stat-heat-month", "{month.as_deref().unwrap_or_default()}" }
                                }
                            }
                            for (row_at, row) in rows.into_iter().enumerate() {
                                div { key: "{row_at}", class: "stat-heat-row",
                                    for (cell_at, cell) in row.into_iter().enumerate() {
                                        match cell {
                                            Some(level) => rsx! { span { key: "{cell_at}", class: "stat-heat-cell level-{level}" } },
                                            None => rsx! { span { key: "{cell_at}", class: "stat-heat-cell none" } },
                                        }
                                    }
                                }
                            }
                            div { class: "stat-heat-legend",
                                span { {tr!("Lighter use")} }
                                for level in 0..=4u8 {
                                    span { key: "{level}", class: "stat-heat-cell level-{level}" }
                                }
                                span { {tr!("Heavier use")} }
                            }
                        }
                    },
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
            CardBlock::Tiles(tiles) => lines.extend(tiles.iter().map(|(label, value)| format!("{label}: {value}"))),
            CardBlock::Heatmap { .. } | CardBlock::Loading => {}
        }
    }
    lines.join("\n")
}
