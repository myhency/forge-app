use base64::Engine;
use dioxus::prelude::*;
use forge_entities::{parse_csv, preview_kind, render_markdown, CsvTable, PreviewKind, CSV_ROW_LIMIT};
use forge_ports::{DocumentView, FileContent, SheetBook, SlideDeck};

use crate::files_office::{DocumentViewer, SlideViewer};
use crate::files_sheet::{binary_offers_open, SheetView};
use forge_shared::tr;

/// What an open tab shows.
#[derive(Debug, Clone, PartialEq)]
pub enum TabKind {
    Loading,
    /// `lossy`: the bytes were not valid UTF-8. `anyway`: "Show as text anyway" on a binary file.
    Text { lossy: bool, anyway: bool },
    Image { data_url: String },
    Pdf { data_url: String },
    Binary,
    Sheet(SheetBook),
    Document(DocumentView),
    Slides(SlideDeck),
    TooLarge,
    Failed(String),
}

impl TabKind {
    /// Text that must never be written back: it was read lossily, or it is binary shown as text.
    pub fn read_only(&self) -> bool {
        matches!(self, TabKind::Text { lossy: true, .. } | TabKind::Text { anyway: true, .. })
    }

    /// Whether the editor (not a note or a picture) is what this tab shows.
    pub fn shows_editor(&self) -> bool {
        matches!(self, TabKind::Text { .. })
    }

    /// A line above the editor when the text cannot be edited.
    pub fn notice(&self) -> Option<&'static str> {
        match self {
            TabKind::Text { lossy: true, .. } => Some(tr!("This file is not valid UTF-8, so it is shown read only.")),
            TabKind::Text { anyway: true, .. } => Some(tr!("Shown as text. This file cannot be edited.")),
            _ => None,
        }
    }
}

pub fn kind_of(content: FileContent) -> TabKind {
    match content {
        FileContent::Text { lossy, .. } => TabKind::Text { lossy, anyway: false },
        FileContent::Image { bytes, mime } => {
            TabKind::Image { data_url: format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
        }
        FileContent::Pdf { bytes } => {
            TabKind::Pdf { data_url: format!("data:application/pdf;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)) }
        }
        FileContent::Binary => TabKind::Binary,
        FileContent::Sheet(book) => TabKind::Sheet(book),
        FileContent::Document(doc) => TabKind::Document(doc),
        FileContent::Slides(deck) => TabKind::Slides(deck),
        FileContent::TooLarge => TabKind::TooLarge,
    }
}

/// What a file with a rendered view shows when it is not in source mode.
#[derive(Debug, Clone, PartialEq)]
pub enum Rendered {
    /// The markdown as HTML.
    Markdown(String),
    Csv(CsvTable),
    /// The file's own text, drawn in a sandboxed frame.
    Html(String),
    /// A `data:` address of the svg.
    Svg(String),
}

/// The rendered view of `text` for the file at `path`; `None` for a file with no rendered view.
pub fn render_for(path: &str, text: &str) -> Option<Rendered> {
    match preview_kind(path) {
        PreviewKind::Markdown => Some(Rendered::Markdown(render_markdown(text))),
        PreviewKind::Csv => Some(Rendered::Csv(parse_csv(text, CSV_ROW_LIMIT))),
        PreviewKind::Html => Some(Rendered::Html(text.to_string())),
        PreviewKind::Svg => Some(Rendered::Svg(format!("data:image/svg+xml;base64,{}", base64::engine::general_purpose::STANDARD.encode(text)))),
        _ => None,
    }
}

const ZOOM_STEP: f64 = 1.25;
const ZOOM_MIN: f64 = 0.1;
const ZOOM_MAX: f64 = 8.0;

/// The zoom after one step: `1` in, `-1` out, any other direction leaves it.
pub fn zoom_step(current: f64, direction: i32) -> f64 {
    match direction {
        1 => (current * ZOOM_STEP).min(ZOOM_MAX),
        -1 => (current / ZOOM_STEP).max(ZOOM_MIN),
        _ => current,
    }
}

/// The body of a tab that is shown rendered. Keyed by the path by the caller, so a tab keeps its own zoom.
#[component]
pub fn RenderedView(rendered: Rendered, path: String) -> Element {
    match rendered {
        Rendered::Markdown(html) => rsx! {
            div { class: "changes-diff",
                div { class: "file-viewer file-markdown markdown", "aria-label": "{path}", dangerous_inner_html: "{html}" }
            }
        },
        Rendered::Csv(table) => rsx! { CsvView { table, path } },
        // An empty sandbox: no scripts, no same-origin, popups, forms or top navigation. A link in the
        // page goes nowhere.
        Rendered::Html(source) => rsx! {
            iframe { class: "files-html", title: "{path}", srcdoc: "{source}", "sandbox": HTML_SANDBOX }
        },
        Rendered::Svg(data_url) => rsx! { SvgView { data_url, path } },
    }
}

/// The html frame's sandbox: nothing allowed. Scripts are off because every frame of the web view can reach the app's IPC handler.
pub const HTML_SANDBOX: &str = "";

#[component]
fn CsvView(table: CsvTable, path: String) -> Element {
    rsx! {
        div { class: "changes-diff",
            if table.truncated {
                p { class: "files-csv-note", {tr!("Only the first {count} rows are shown. Switch to the source to see the rest.", count = CSV_ROW_LIMIT)} }
            }
            div { class: "file-viewer file-markdown markdown files-csv", "aria-label": "{path}",
                table {
                    thead {
                        tr {
                            for (index, cell) in table.header.iter().enumerate() {
                                th { key: "{index}", "{cell}" }
                            }
                        }
                    }
                    tbody {
                        for (row_index, row) in table.rows.iter().enumerate() {
                            tr { key: "{row_index}",
                                for (index, cell) in row.iter().enumerate() {
                                    td { key: "{index}", "{cell}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SvgView(data_url: String, path: String) -> Element {
    let mut scale = use_signal(|| 1.0_f64);
    rsx! {
        div { class: "files-svg",
            div { class: "files-svg-stage",
                img { src: "{data_url}", alt: "{path}", style: "zoom: {scale}" }
            }
            div { class: "files-svg-bar",
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Zoom out"),
                    "aria-label": "Zoom out",
                    onclick: move |_| scale.set(zoom_step(scale(), -1)),
                    "−"
                }
                button {
                    class: "icon-button",
                    r#type: "button",
                    title: tr!("Zoom in"),
                    "aria-label": "Zoom in",
                    onclick: move |_| scale.set(zoom_step(scale(), 1)),
                    "+"
                }
                button { class: "icon-button", r#type: "button", title: tr!("Fit"), "aria-label": tr!("Fit"), onclick: move |_| scale.set(1.0), "⤢" }
            }
        }
    }
}

/// The body of a tab that has no editor: a picture, a binary file, a file that is too large.
#[component]
pub fn FileKindView(kind: TabKind, path: String, on_anyway: EventHandler<()>, on_reveal: EventHandler<()>, on_open: EventHandler<()>) -> Element {
    match kind {
        TabKind::Image { data_url } => rsx! {
            div { class: "files-kind",
                img { src: "{data_url}", alt: "{path}" }
            }
        },
        TabKind::Pdf { data_url } => rsx! {
            embed { class: "files-pdf", src: "{data_url}", r#type: "application/pdf" }
        },
        TabKind::Binary => rsx! {
            div { class: "files-kind",
                p { {tr!("This file looks like binary data and cannot be previewed as text.")} }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "{forge_features::show_in_file_manager()}" }
                    if binary_offers_open(&path) {
                        button { class: "files-kind-button", r#type: "button", onclick: move |_| on_open.call(()), {tr!("Open in the default program")} }
                    }
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_anyway.call(()), {tr!("Show as text anyway")} }
                }
            }
        },
        TabKind::Sheet(book) => rsx! { SheetView { key: "{path}", book, path, on_open } },
        TabKind::Document(doc) => rsx! { DocumentViewer { key: "{path}", doc, path, on_open } },
        TabKind::Slides(deck) => rsx! { SlideViewer { key: "{path}", deck, path, on_open } },
        TabKind::TooLarge => rsx! {
            div { class: "files-kind",
                p { class: "files-kind-title", {tr!("This file cannot be previewed")} }
                p { class: "files-kind-sub", {tr!("It is too large to preview here.")} }
                p { class: "files-kind-sub", "{path}" }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "{forge_features::show_in_file_manager()}" }
                }
            }
        },
        TabKind::Loading | TabKind::Text { .. } | TabKind::Failed(_) => rsx! {},
    }
}

#[cfg(test)]
mod tests {
    use forge_ports::FileVersion;

    use super::*;

    #[test]
    fn kind_of_gives_a_document_tab_and_a_slides_tab() {
        let doc = DocumentView { blocks: vec![], truncated: false };
        let deck = SlideDeck { slides: vec![], truncated: true };
        assert_eq!(kind_of(FileContent::Document(doc.clone())), TabKind::Document(doc));
        assert_eq!(kind_of(FileContent::Slides(deck.clone())), TabKind::Slides(deck));
    }

    #[test]
    fn the_html_frame_allows_nothing() {
        assert_eq!(HTML_SANDBOX, "");
    }

    fn version() -> FileVersion {
        FileVersion { modified_ns: 1, len: 1 }
    }

    #[test]
    fn each_rendered_kind_is_made_from_its_text_and_other_files_have_no_rendered_view() {
        assert!(matches!(render_for("a.md", "# Hi"), Some(Rendered::Markdown(html)) if html.contains("<h1>")));
        match render_for("a.csv", "a,b\n1,2\n") {
            Some(Rendered::Csv(table)) => assert_eq!(table.rows.len(), 1),
            other => panic!("{other:?}"),
        }
        assert_eq!(render_for("a.html", "<p>x</p>"), Some(Rendered::Html("<p>x</p>".to_string())));
        assert_eq!(
            render_for("a.svg", "<svg/>"),
            Some(Rendered::Svg("data:image/svg+xml;base64,PHN2Zy8+".to_string()))
        );
        for plain in ["a.rs", "a.json", "a.png", "a.pdf", "a"] {
            assert_eq!(render_for(plain, "x"), None, "{plain}");
        }
    }

    #[test]
    fn the_csv_view_is_cut_at_the_row_limit() {
        let text: String = std::iter::once("n\n".to_string()).chain((0..CSV_ROW_LIMIT + 3).map(|n| format!("{n}\n"))).collect();
        let Some(Rendered::Csv(table)) = render_for("big.csv", &text) else { panic!("not a table") };
        assert_eq!(table.rows.len(), CSV_ROW_LIMIT);
        assert!(table.truncated);
    }

    #[test]
    fn zooming_steps_by_a_quarter_and_stays_between_a_tenth_and_eight() {
        assert_eq!(zoom_step(1.0, 1), 1.25);
        assert_eq!(zoom_step(1.25, -1), 1.0);
        assert_eq!(zoom_step(7.0, 1), 8.0);
        assert_eq!(zoom_step(8.0, 1), 8.0);
        assert_eq!(zoom_step(0.11, -1), 0.1);
        assert_eq!(zoom_step(0.1, -1), 0.1);
        assert_eq!(zoom_step(2.0, 0), 2.0);
    }

    #[test]
    fn a_pdf_becomes_a_data_address_with_its_type_and_has_no_editor() {
        let kind = kind_of(FileContent::Pdf { bytes: vec![1, 2, 3] });
        assert_eq!(kind, TabKind::Pdf { data_url: "data:application/pdf;base64,AQID".to_string() });
        assert!(!kind.shows_editor());
        assert_eq!(kind.notice(), None);
    }

    #[test]
    fn text_is_editable_unless_it_was_read_lossily_or_forced() {
        let plain = kind_of(FileContent::Text { text: "a".to_string(), version: version(), lossy: false });
        assert_eq!(plain, TabKind::Text { lossy: false, anyway: false });
        assert!(!plain.read_only());
        assert_eq!(plain.notice(), None);

        let lossy = kind_of(FileContent::Text { text: "a\u{fffd}".to_string(), version: version(), lossy: true });
        assert!(lossy.read_only(), "a file that is not valid UTF-8 must never be written back");
        assert_eq!(lossy.notice(), Some(tr!("This file is not valid UTF-8, so it is shown read only.")));

        let forced = TabKind::Text { lossy: false, anyway: true };
        assert!(forced.read_only(), "a binary file shown as text must never be written back");
        assert_eq!(forced.notice(), Some(tr!("Shown as text. This file cannot be edited.")));
    }

    #[test]
    fn an_image_becomes_a_data_address_with_its_type() {
        let kind = kind_of(FileContent::Image { bytes: vec![1, 2, 3], mime: "image/png".to_string() });
        assert_eq!(kind, TabKind::Image { data_url: "data:image/png;base64,AQID".to_string() });
    }

    #[test]
    fn binary_and_too_large_map_straight_across() {
        assert_eq!(kind_of(FileContent::Binary), TabKind::Binary);
        assert_eq!(kind_of(FileContent::TooLarge), TabKind::TooLarge);
    }

    #[test]
    fn a_sheet_keeps_its_book_and_has_no_editor() {
        let book = SheetBook { sheets: vec![] };
        let kind = kind_of(FileContent::Sheet(book.clone()));
        assert_eq!(kind, TabKind::Sheet(book));
        assert!(!kind.shows_editor());
    }

    #[test]
    fn only_text_has_an_editor() {
        assert!(TabKind::Text { lossy: false, anyway: false }.shows_editor());
        for other in [TabKind::Loading, TabKind::Binary, TabKind::TooLarge, TabKind::Failed("x".to_string())] {
            assert!(!other.shows_editor(), "{other:?}");
        }
    }
}
