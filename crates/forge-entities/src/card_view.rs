use dioxus::prelude::*;
use forge_domain::Card;
use forge_shared::{tr, Icon, IconKind, IconSize};

/// The title of the excerpt modal: `/abs/a.py:L5` or `/abs/a.py:L5-L8`.
pub fn modal_title(card: &Card) -> String {
    if card.range.start == card.range.end {
        format!("{}:L{}", card.path, card.range.start)
    } else {
        format!("{}:L{}-L{}", card.path, card.range.start, card.range.end)
    }
}

/// One card: the text at the top and a badge at the bottom. In the composer it can be clicked and
/// removed; in the chat it is only shown.
#[component]
pub fn CardView(card: Card, on_click: Option<EventHandler<()>>, on_remove: Option<EventHandler<()>>) -> Element {
    let clickable = on_click.is_some();
    rsx! {
        div {
            class: if clickable { "card clickable" } else { "card" },
            title: "{card.path}",
            onclick: move |_| {
                if let Some(handler) = &on_click {
                    handler.call(());
                }
            },
            div { class: "card-text", "{card.text}" }
            span { class: "card-badge", {forge_shared::i18n::text(card.badge())} }
            if let Some(remove) = on_remove {
                button {
                    class: "card-remove",
                    r#type: "button",
                    "aria-label": tr!("Remove"),
                    onclick: move |event: MouseEvent| {
                        event.stop_propagation();
                        remove.call(());
                    },
                    Icon { kind: IconKind::Close, size: IconSize::Small }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::{CardKind, LineRange};

    fn card(a: u32, b: u32) -> Card {
        Card { id: 1, kind: CardKind::Excerpt, path: "/p/fizz.py".into(), range: LineRange::new(a, b), text: "x".into() }
    }

    #[test]
    fn the_modal_title_names_the_lines() {
        assert_eq!(modal_title(&card(5, 5)), "/p/fizz.py:L5");
        assert_eq!(modal_title(&card(5, 8)), "/p/fizz.py:L5-L8");
    }
}
