use std::rc::Rc;

use dioxus::prelude::*;
use forge_features::offers_default_program;
use forge_ports::{DocBlock, DocumentView, Slide, SlideDeck, TextRun};
use forge_shared::tr;

/// The marker of each block: `•` for a bullet item, `1.`, `2.` … for numbered items, `None` for other blocks.
/// A numbered count runs on through deeper items and restarts after a block that is not a list item or after a shallower item.
pub fn list_markers(blocks: &[DocBlock]) -> Vec<Option<String>> {
    // counts[level] is how many numbered items the run has at that level so far.
    let mut counts: Vec<usize> = Vec::new();
    blocks
        .iter()
        .map(|block| match block {
            DocBlock::ListItem { level, numbered, .. } => {
                let level = usize::from(*level);
                // A shallower item ends the deeper runs.
                counts.truncate(level + 1);
                counts.resize(level + 1, 0);
                if *numbered {
                    counts[level] += 1;
                    Some(format!("{}.", counts[level]))
                } else {
                    counts[level] = 0;
                    Some("•".to_string())
                }
            }
            _ => {
                counts.clear();
                None
            }
        })
        .collect()
}

/// The class of a heading of this level: 1 to 6, anything above clamps to 6 and 0 to 1.
pub fn heading_class(level: u8) -> &'static str {
    match level {
        0 | 1 => "office-h1",
        2 => "office-h2",
        3 => "office-h3",
        4 => "office-h4",
        5 => "office-h5",
        _ => "office-h6",
    }
}

/// Whether a slide has no body text; its title, if any, still shows.
pub fn slide_is_empty(slide: &Slide) -> bool {
    slide.items.is_empty()
}

/// The left indent of a list item or slide item, in pixels.
fn indent(level: u8) -> u32 {
    u32::from(level.min(8)) * 20
}

/// The runs of a block as spans; a plain function so a block's runs are borrowed, not cloned per render.
fn runs_view(runs: &[TextRun]) -> Element {
    rsx! {
        for (index, run) in runs.iter().enumerate() {
            span {
                key: "{index}",
                style: if run.bold && run.italic { "font-weight: 600; font-style: italic" } else if run.bold { "font-weight: 600" } else if run.italic { "font-style: italic" } else { "" },
                "{run.text}"
            }
        }
    }
}

/// The body of a docx tab: a toolbar and a reading column of headings, paragraphs, lists and tables.
#[component]
pub fn DocumentViewer(doc: DocumentView, path: String, on_open: EventHandler<()>) -> Element {
    let doc = use_memo(use_reactive!(|doc| Rc::new(doc)));
    let doc = doc();
    let markers = list_markers(&doc.blocks);
    rsx! {
        div { class: "office-view", "aria-label": "{path}",
            OfficeToolbar { path: path.clone(), on_open }
            div { class: "office-scroll",
                div { class: "office-page",
                    for (index, block) in doc.blocks.iter().enumerate() {
                        match block {
                            DocBlock::Heading { level, text } => match (*level).clamp(1, 6) {
                                1 => rsx! { h1 { key: "{index}", class: heading_class(1), "{text}" } },
                                2 => rsx! { h2 { key: "{index}", class: heading_class(2), "{text}" } },
                                3 => rsx! { h3 { key: "{index}", class: heading_class(3), "{text}" } },
                                4 => rsx! { h4 { key: "{index}", class: heading_class(4), "{text}" } },
                                5 => rsx! { h5 { key: "{index}", class: heading_class(5), "{text}" } },
                                _ => rsx! { h6 { key: "{index}", class: heading_class(6), "{text}" } },
                            },
                            DocBlock::Paragraph { runs } => rsx! {
                                p { key: "{index}", {runs_view(runs)} }
                            },
                            DocBlock::ListItem { level, runs, .. } => rsx! {
                                div { key: "{index}", class: "office-item", style: "margin-left: {indent(*level)}px",
                                    span { class: "office-marker", "{markers[index].clone().unwrap_or_default()}" }
                                    span { {runs_view(runs)} }
                                }
                            },
                            DocBlock::Table { rows } => rsx! {
                                div { key: "{index}", class: "office-table",
                                    table {
                                        tbody {
                                            for (row_index, row) in rows.iter().enumerate() {
                                                tr { key: "{row_index}",
                                                    for (cell_index, cell) in row.iter().enumerate() {
                                                        td { key: "{cell_index}", "{cell}" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                        }
                    }
                    if doc.truncated {
                        p { class: "office-note", {tr!("Only part of the file is shown.")} }
                    }
                }
            }
        }
    }
}

/// The body of a pptx tab: a toolbar and one card per slide.
#[component]
pub fn SlideViewer(deck: SlideDeck, path: String, on_open: EventHandler<()>) -> Element {
    let deck = use_memo(use_reactive!(|deck| Rc::new(deck)));
    let deck = deck();
    rsx! {
        div { class: "office-view", "aria-label": "{path}",
            OfficeToolbar { path: path.clone(), on_open }
            div { class: "office-scroll",
                div { class: "office-page",
                    for slide in deck.slides.iter() {
                        div { key: "{slide.number}", class: "office-slide",
                            div { class: "office-slide-head", {tr!("Slide {number}", number = slide.number)} }
                            div { class: "office-slide-body",
                                if let Some(title) = &slide.title {
                                    p { class: "office-slide-title", "{title}" }
                                }
                                for (index, item) in slide.items.iter().enumerate() {
                                    p { key: "{index}", style: "margin-left: {indent(item.level)}px", "{item.text}" }
                                }
                                if slide_is_empty(slide) {
                                    p { class: "office-slide-empty", {tr!("This slide has no text.")} }
                                }
                            }
                        }
                    }
                    if deck.truncated {
                        p { class: "office-note", {tr!("Only part of the file is shown.")} }
                    }
                }
            }
        }
    }
}

/// The slim bar above a document or a deck, with the default program's button on the right.
#[component]
fn OfficeToolbar(path: String, on_open: EventHandler<()>) -> Element {
    rsx! {
        div { class: "office-toolbar",
            if offers_default_program(&path) {
                button { class: "files-kind-button", r#type: "button", onclick: move |_| on_open.call(()), {tr!("Open in the default program")} }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(level: u8, numbered: bool) -> DocBlock {
        DocBlock::ListItem { level, numbered, runs: vec![] }
    }

    fn paragraph() -> DocBlock {
        DocBlock::Paragraph { runs: vec![] }
    }

    fn marks(expected: &[Option<&str>]) -> Vec<Option<String>> {
        expected.iter().map(|m| m.map(str::to_string)).collect()
    }

    #[test]
    fn bullets_get_a_dot_and_other_blocks_none() {
        assert_eq!(list_markers(&[item(0, false), paragraph(), item(1, false)]), marks(&[Some("•"), None, Some("•")]));
    }

    #[test]
    fn numbered_items_count_and_restart_after_a_paragraph() {
        let blocks = [item(0, true), item(0, true), paragraph(), item(0, true)];
        assert_eq!(list_markers(&blocks), marks(&[Some("1."), Some("2."), None, Some("1.")]));
    }

    #[test]
    fn nested_numbered_items_count_per_level() {
        let blocks = [item(0, true), item(1, true), item(1, true), item(0, true), item(1, true)];
        assert_eq!(list_markers(&blocks), marks(&[Some("1."), Some("1."), Some("2."), Some("2."), Some("1.")]));
    }

    #[test]
    fn headings_clamp_to_six_levels() {
        let classes: Vec<_> = (1..=6).map(heading_class).collect();
        assert_eq!(classes, ["office-h1", "office-h2", "office-h3", "office-h4", "office-h5", "office-h6"]);
        assert_eq!(heading_class(9), "office-h6");
        assert_eq!(heading_class(0), "office-h1");
    }

    #[test]
    fn a_slide_without_items_is_empty_even_with_a_title() {
        let titled = Slide { number: 1, title: Some("T".to_string()), items: vec![] };
        assert!(slide_is_empty(&titled));
        let full = Slide { number: 2, title: None, items: vec![forge_ports::SlideItem { level: 0, text: "x".to_string() }] };
        assert!(!slide_is_empty(&full));
    }
}
