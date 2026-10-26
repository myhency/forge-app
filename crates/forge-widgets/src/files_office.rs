use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

use base64::Engine;

use dioxus::prelude::*;
use forge_features::offers_default_program;
use forge_ports::{DocBlock, DocumentView, Rgb, Slide, SlideDeck, TableCell, TextRun};
use forge_shared::{tr, Segmented};

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

/// A colour's class for the theme rules: reset when it would vanish on the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumClass {
    Dark,
    Mid,
    Light,
}

impl LumClass {
    /// The value of the `data-lum` attribute that `theme.css` matches.
    fn attr(self) -> &'static str {
        match self {
            LumClass::Dark => "dark",
            LumClass::Mid => "mid",
            LumClass::Light => "light",
        }
    }
}

/// The relative luminance of a colour, 0 (black) to 1 (white), from the linearised sRGB channels.
pub fn luminance(color: Rgb) -> f32 {
    let linear = |channel: u8| {
        let value = f32::from(channel) / 255.0;
        if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * linear(color.0) + 0.7152 * linear(color.1) + 0.0722 * linear(color.2)
}

/// `Dark` below 0.12, `Light` above 0.88, else `Mid`.
pub fn luminance_class(color: Rgb) -> LumClass {
    let lum = luminance(color);
    if lum < 0.12 {
        LumClass::Dark
    } else if lum > 0.88 {
        LumClass::Light
    } else {
        LumClass::Mid
    }
}

/// Black on a light fill, white on a dark one.
pub fn text_on(fill: Rgb) -> Rgb {
    if luminance(fill) >= 0.5 { Rgb(0, 0, 0) } else { Rgb(255, 255, 255) }
}

/// Whether a text colour reads on a backdrop: their luminances differ by at least 0.35.
fn reads_on(color: Rgb, backdrop: Rgb) -> bool {
    (luminance(color) - luminance(backdrop)).abs() >= 0.35
}

/// The colour a run is drawn in on a backdrop it brings or sits on: its own if it reads there, else the
/// readable one for a highlight, else (on a cell fill) the cell's. `None` leaves the theme's colour.
fn text_color(run: &TextRun, backdrop: Option<Rgb>) -> Option<Rgb> {
    match (run.color, backdrop) {
        (Some(own), Some(back)) if reads_on(own, back) => Some(own),
        (_, Some(back)) if run.highlight.is_some() => Some(text_on(back)),
        (_, Some(_)) => None,
        (own, None) => own,
    }
}

/// The inline style of a run (colour, highlight, decoration), built only from `Rgb::css` and fixed
/// keywords; `None` for a run with none of them.
#[cfg(test)]
fn run_style(run: &TextRun) -> Option<String> {
    run_style_on(run, None)
}

/// `run_style` for a run inside a cell with this fill; a highlight is the nearer backdrop.
pub fn run_style_on(run: &TextRun, fill: Option<Rgb>) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if let Some(color) = text_color(run, run.highlight.or(fill)) {
        parts.push(format!("color:{}", color.css()));
    }
    if let Some(highlight) = run.highlight {
        parts.push(format!("background-color:{}", highlight.css()));
    }
    match (run.underline, run.strike) {
        (true, true) => parts.push("text-decoration:underline line-through".to_string()),
        (true, false) => parts.push("text-decoration:underline".to_string()),
        (false, true) => parts.push("text-decoration:line-through".to_string()),
        (false, false) => {}
    }
    if parts.is_empty() { None } else { Some(parts.join(";")) }
}

/// The `data-lum` of a run: only a colour that sits on the page (no highlight, no fill) can vanish
/// with the theme.
#[cfg(test)]
fn lum_attr(run: &TextRun) -> Option<&'static str> {
    lum_attr_on(run, None)
}

/// `lum_attr` for a run inside a cell with this fill.
pub fn lum_attr_on(run: &TextRun, fill: Option<Rgb>) -> Option<&'static str> {
    if run.highlight.is_some() || fill.is_some() {
        return None;
    }
    run.color.map(|color| luminance_class(color).attr())
}

/// The inline style of a cell: its fill and the readable text colour on it.
pub fn cell_style(cell: &TableCell) -> Option<String> {
    cell.fill.map(|fill| format!("background-color:{};color:{}", fill.css(), text_on(fill).css()))
}

/// The inline style of a coloured heading.
pub fn heading_style(color: Option<Rgb>) -> Option<String> {
    color.map(|color| format!("color:{}", color.css()))
}

/// The `data-lum` of a heading's colour.
fn heading_lum(color: Option<Rgb>) -> Option<&'static str> {
    color.map(|color| luminance_class(color).attr())
}

/// The runs of a block, each under a keyed fragment so the siblings are uniformly keyed; a plain function so a block's runs are borrowed, not cloned per render.
fn runs_view(runs: &[TextRun], fill: Option<Rgb>) -> Element {
    rsx! {
        for (index, run) in runs.iter().enumerate() {
            Fragment { key: "{index}", {run_view(run, fill)} }
        }
    }
}

/// One run: bare text when plain, else a span with its font and colour styles.
fn run_view(run: &TextRun, fill: Option<Rgb>) -> Element {
    let font = match (run.bold, run.italic) {
        (true, true) => "font-weight:600;font-style:italic",
        (true, false) => "font-weight:600",
        (false, true) => "font-style:italic",
        (false, false) => "",
    };
    let colours = run_style_on(run, fill);
    if font.is_empty() && colours.is_none() {
        return rsx! { "{run.text}" };
    }
    let style = match colours {
        Some(colours) if !font.is_empty() => format!("{font};{colours}"),
        Some(colours) => colours,
        None => font.to_string(),
    };
    rsx! { span { style: "{style}", "data-lum": lum_attr_on(run, fill), "{run.text}" } }
}

/// The body of a docx tab: a toolbar and a reading column of headings, paragraphs, lists and tables.
#[component]
pub fn DocumentViewer(doc: DocumentView, path: String, on_open: EventHandler<()>) -> Element {
    let doc = use_memo(use_reactive!(|doc| Rc::new(doc)));
    let doc = doc();
    let markers = list_markers(&doc.blocks);
    // Pages first when the file was kept; the reading view when drawing the pages failed.
    let mut mode = use_signal(|| if doc.source.is_some() { OfficeMode::Pages } else { OfficeMode::Reading });
    let mut failure = use_signal(|| None::<String>);
    rsx! {
        div { class: "office-view", "aria-label": "{path}",
            OfficeToolbar { path: path.clone(), on_open, modes: doc.source.is_some().then(|| (OfficeMode::Pages, tr!("Pages"))), mode: mode(), on_mode: move |next| mode.set(next) }
            if let (OfficeMode::Pages, Some(source)) = (mode(), doc.source.clone()) {
                OfficePages {
                    key: "{path}",
                    docx: true,
                    source,
                    on_failed: move |why: String| {
                        failure.set(Some(why));
                        mode.set(OfficeMode::Reading);
                    },
                }
            } else {
            div { class: "office-scroll",
                if let Some(why) = failure() {
                    p { class: "office-note", {tr!("The page view could not be drawn ({why}). This is the reading view.", why = why)} }
                }
                div { class: "office-page",
                    for (index, block) in doc.blocks.iter().enumerate() {
                        match block {
                            DocBlock::Heading { level, text, color } => match (*level).clamp(1, 6) {
                                1 => rsx! { h1 { key: "{index}", class: heading_class(1), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                                2 => rsx! { h2 { key: "{index}", class: heading_class(2), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                                3 => rsx! { h3 { key: "{index}", class: heading_class(3), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                                4 => rsx! { h4 { key: "{index}", class: heading_class(4), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                                5 => rsx! { h5 { key: "{index}", class: heading_class(5), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                                _ => rsx! { h6 { key: "{index}", class: heading_class(6), style: heading_style(*color), "data-lum": heading_lum(*color), "{text}" } },
                            },
                            DocBlock::Paragraph { runs } => rsx! {
                                p { key: "{index}", {runs_view(runs, None)} }
                            },
                            DocBlock::ListItem { level, runs, .. } => rsx! {
                                div { key: "{index}", class: "office-item", style: "margin-left: {indent(*level)}px",
                                    span { class: "office-marker", "{markers[index].clone().unwrap_or_default()}" }
                                    span { {runs_view(runs, None)} }
                                }
                            },
                            DocBlock::Table { rows } => rsx! {
                                div { key: "{index}", class: "office-table",
                                    table {
                                        tbody {
                                            for (row_index, row) in rows.iter().enumerate() {
                                                tr { key: "{row_index}",
                                                    for (cell_index, cell) in row.iter().enumerate() {
                                                        td { key: "{cell_index}", style: cell_style(cell), {runs_view(&cell.runs, cell.fill)} }
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
}

/// The body of a pptx tab: a toolbar and one card per slide.
#[component]
pub fn SlideViewer(deck: SlideDeck, path: String, on_open: EventHandler<()>) -> Element {
    let deck = use_memo(use_reactive!(|deck| Rc::new(deck)));
    let deck = deck();
    let mut mode = use_signal(|| if deck.source.is_some() { OfficeMode::Pages } else { OfficeMode::Reading });
    let mut failure = use_signal(|| None::<String>);
    rsx! {
        div { class: "office-view", "aria-label": "{path}",
            OfficeToolbar { path: path.clone(), on_open, modes: deck.source.is_some().then(|| (OfficeMode::Pages, tr!("Slides"))), mode: mode(), on_mode: move |next| mode.set(next) }
            if let (OfficeMode::Pages, Some(source)) = (mode(), deck.source.clone()) {
                OfficePages {
                    key: "{path}",
                    docx: false,
                    source,
                    on_failed: move |why: String| {
                        failure.set(Some(why));
                        mode.set(OfficeMode::Reading);
                    },
                }
            } else {
            div { class: "office-scroll",
                if let Some(why) = failure() {
                    p { class: "office-note", {tr!("The slide view could not be drawn ({why}). This is the reading view.", why = why)} }
                }
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
}

/// The slim bar above a document or a deck, with the default program's button on the right.
#[component]
fn OfficeToolbar(
    path: String,
    on_open: EventHandler<()>,
    /// The drawn view's value and label; `None` when there is no drawn view (a toolbar of one button).
    modes: Option<(OfficeMode, &'static str)>,
    mode: OfficeMode,
    on_mode: EventHandler<OfficeMode>,
) -> Element {
    rsx! {
        div { class: "office-toolbar",
            if let Some((drawn, label)) = modes {
                Segmented {
                    label: tr!("View"),
                    value: mode.value().to_string(),
                    options: vec![(drawn.value().to_string(), label.to_string()), (OfficeMode::Reading.value().to_string(), tr!("Reading|view").to_string())],
                    onchange: move |value: String| on_mode.call(OfficeMode::from_value(&value)),
                }
                span { class: "spacer" }
            }
            if offers_default_program(&path) {
                button { class: "files-kind-button", r#type: "button", onclick: move |_| on_open.call(()), {tr!("Open in the default program")} }
            }
        }
    }
}

/// How a docx or pptx is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfficeMode {
    /// Pages or slides drawn like the program draws them.
    Pages,
    /// The text, in a reading column.
    Reading,
}

impl OfficeMode {
    fn value(self) -> &'static str {
        match self {
            OfficeMode::Pages => "pages",
            OfficeMode::Reading => "reading",
        }
    }

    fn from_value(value: &str) -> Self {
        if value == "reading" { OfficeMode::Reading } else { OfficeMode::Pages }
    }
}

static NEXT_HOST: AtomicU32 = AtomicU32::new(1);

/// The drawn view of a docx (`docx`) or a pptx: the file goes to the page's own drawing code, which
/// answers `ok` or why it could not.
#[component]
fn OfficePages(docx: bool, source: std::sync::Arc<[u8]>, on_failed: EventHandler<String>) -> Element {
    let id = use_hook(|| format!("office-pages-{}", NEXT_HOST.fetch_add(1, Ordering::Relaxed)));
    let host = id.clone();
    rsx! {
        div {
            id: "{id}",
            class: if docx { "office-pages office-docx" } else { "office-pages office-pptx" },
            onmounted: move |_| {
                let host = host.clone();
                let source = source.clone();
                spawn(async move {
                    let encoded = base64::engine::general_purpose::STANDARD.encode(&*source);
                    if !docx {
                        // The slide library is big: it is loaded the first time a deck is drawn.
                        if let Err(error) = document::eval(&format!("{} return 'ok';", forge_shared::pptx_library())).join::<String>().await {
                            on_failed.call(error.to_string());
                            return;
                        }
                    }
                    let call = if docx { "forgeOffice.docx" } else { "forgeOffice.pptx" };
                    let script = format!("return await {call}('{host}', '{encoded}');");
                    match document::eval(&script).join::<String>().await {
                        Ok(answer) if answer == "ok" => {}
                        Ok(answer) => on_failed.call(answer.trim_start_matches("error: ").to_string()),
                        Err(error) => on_failed.call(error.to_string()),
                    }
                });
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_round_trips_through_its_value() {
        assert_eq!(OfficeMode::from_value(OfficeMode::Pages.value()), OfficeMode::Pages);
        assert_eq!(OfficeMode::from_value(OfficeMode::Reading.value()), OfficeMode::Reading);
        assert_eq!(OfficeMode::from_value("anything else"), OfficeMode::Pages);
    }

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

    fn run(text: &str) -> TextRun {
        TextRun { text: text.to_string(), ..TextRun::default() }
    }

    #[test]
    fn luminance_of_black_white_and_red() {
        assert!(luminance(Rgb(0, 0, 0)).abs() < 1e-6);
        assert!((luminance(Rgb(255, 255, 255)) - 1.0).abs() < 1e-4);
        assert!((luminance(Rgb(255, 0, 0)) - 0.2126).abs() < 1e-3);
    }

    #[test]
    fn luminance_class_boundaries() {
        assert_eq!(luminance_class(Rgb(0, 0, 0)), LumClass::Dark);
        assert_eq!(luminance_class(Rgb(255, 255, 255)), LumClass::Light);
        assert_eq!(luminance_class(Rgb(255, 0, 0)), LumClass::Mid);
        // Just either side of 0.12: sRGB 100 is about 0.127, 90 about 0.102.
        assert_eq!(luminance_class(Rgb(100, 100, 100)), LumClass::Mid);
        assert_eq!(luminance_class(Rgb(90, 90, 90)), LumClass::Dark);
        assert_eq!(luminance_class(Rgb(245, 245, 245)), LumClass::Light);
        assert_eq!(luminance_class(Rgb(235, 235, 235)), LumClass::Mid);
    }

    #[test]
    fn text_on_a_light_fill_is_black_and_on_a_dark_fill_is_white() {
        assert_eq!(text_on(Rgb(255, 255, 0)), Rgb(0, 0, 0));
        assert_eq!(text_on(Rgb(0, 0, 128)), Rgb(255, 255, 255));
        assert_eq!(text_on(Rgb(255, 255, 255)), Rgb(0, 0, 0));
    }

    #[test]
    fn run_style_is_none_for_a_plain_run() {
        assert_eq!(run_style(&run("x")), None);
        assert_eq!(run_style(&TextRun { bold: true, italic: true, ..run("x") }), None);
        assert_eq!(lum_attr(&run("x")), None);
    }

    #[test]
    fn run_style_builds_only_from_validated_values() {
        let red = TextRun { color: Some(Rgb(255, 0, 0)), underline: true, strike: true, ..run("x") };
        assert_eq!(run_style(&red).as_deref(), Some("color:#ff0000;text-decoration:underline line-through"));
        assert_eq!(lum_attr(&red), Some("mid"));
        let underline = TextRun { underline: true, ..run("x") };
        assert_eq!(run_style(&underline).as_deref(), Some("text-decoration:underline"));
        // Whatever the three bytes are, the style has no way to carry anything but the fixed shape.
        for byte in 0..=255u8 {
            let styled = TextRun { color: Some(Rgb(byte, 255 - byte, byte)), highlight: Some(Rgb(byte, byte, 1)), ..run("x") };
            let style = run_style(&styled).unwrap();
            assert!(!style.contains("url(") && !style.contains('"') && !style.contains('<') && !style.contains('\\'));
            assert_eq!(style.matches(';').count(), 1);
            assert!(style.split(';').all(|part| part.starts_with("color:#") || part.starts_with("background-color:#")));
        }
    }

    #[test]
    fn a_highlighted_run_gets_a_readable_text_colour() {
        let yellow = Rgb(255, 255, 0);
        let plain = TextRun { highlight: Some(yellow), ..run("x") };
        assert_eq!(run_style(&plain).as_deref(), Some("color:#000000;background-color:#ffff00"));
        // A white own colour on yellow has no contrast: the readable one wins.
        let white = TextRun { color: Some(Rgb(255, 255, 255)), highlight: Some(yellow), ..run("x") };
        assert_eq!(run_style(&white).as_deref(), Some("color:#000000;background-color:#ffff00"));
        // A dark blue on yellow has contrast and is kept; it needs no theme reset.
        let blue = TextRun { color: Some(Rgb(0, 0, 128)), highlight: Some(yellow), ..run("x") };
        assert_eq!(run_style(&blue).as_deref(), Some("color:#000080;background-color:#ffff00"));
        assert_eq!(lum_attr(&blue), None);
    }

    #[test]
    fn a_cell_fill_style_and_its_text_colour() {
        let none = TableCell { runs: vec![run("a")], fill: None };
        assert_eq!(cell_style(&none), None);
        let navy = TableCell { runs: vec![run("a")], fill: Some(Rgb(0, 0, 128)) };
        assert_eq!(cell_style(&navy).as_deref(), Some("background-color:#000080;color:#ffffff"));
        // A run keeps its colour only if it reads on the fill.
        let on_navy = TextRun { color: Some(Rgb(255, 255, 0)), ..run("a") };
        assert_eq!(run_style_on(&on_navy, navy.fill).as_deref(), Some("color:#ffff00"));
        assert_eq!(lum_attr_on(&on_navy, navy.fill), None);
        let black_on_navy = TextRun { color: Some(Rgb(0, 0, 0)), ..run("a") };
        assert_eq!(run_style_on(&black_on_navy, navy.fill), None);
    }

    #[test]
    fn a_heading_colour_is_a_fixed_style() {
        assert_eq!(heading_style(None), None);
        assert_eq!(heading_style(Some(Rgb(1, 2, 3))).as_deref(), Some("color:#010203"));
    }

    #[test]
    fn a_paragraph_mixing_plain_and_styled_runs_renders_twice_without_a_key_mismatch() {
        fn mixed() -> Element {
            let runs = [run("a"), TextRun { bold: true, ..run("b") }, run("c"), TextRun { color: Some(Rgb(1, 2, 3)), ..run("d") }];
            rsx! { p { {runs_view(&runs, None)} } }
        }
        let mut dom = VirtualDom::new(mixed);
        dom.rebuild_in_place();
        dom.mark_dirty(ScopeId::APP);
        // Dioxus debug-asserts that siblings are all keyed or all unkeyed while diffing.
        dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
    }
}
