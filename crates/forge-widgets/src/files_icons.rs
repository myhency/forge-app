use dioxus::prelude::*;
use forge_entities::FileIcon;

/// The small picture before a file name in the tree: a table for csv, a globe for html, a picture
/// for images and svg, `</>` for code, a page with lines for markdown, a plain page for the rest.
#[component]
pub fn FileTypeIcon(icon: FileIcon) -> Element {
    rsx! {
        svg {
            class: "files-icon",
            view_box: "0 0 16 16",
            width: "14",
            height: "14",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "1.3",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            "aria-hidden": "true",
            match icon {
                FileIcon::Table => rsx! {
                    rect { x: "2", y: "3", width: "12", height: "10", rx: "1.5" }
                    path { d: "M2 7h12M6 3v10" }
                },
                FileIcon::Globe => rsx! {
                    circle { cx: "8", cy: "8", r: "6" }
                    path { d: "M2 8h12M8 2c2 2 2 10 0 12M8 2c-2 2-2 10 0 12" }
                },
                FileIcon::Picture => rsx! {
                    rect { x: "2", y: "3", width: "12", height: "10", rx: "1.5" }
                    circle { cx: "6", cy: "7", r: "1" }
                    path { d: "M2.5 12l3.5-3 3 2.5 2-1.5 2.5 2" }
                },
                FileIcon::Code => rsx! { path { d: "M6 5L3 8l3 3M10 5l3 3-3 3" } },
                FileIcon::Doc => rsx! { path { d: "M4 2h5l3 3v9H4zM9 2v3h3M6 8h4M6 10.5h4" } },
                FileIcon::File => rsx! { path { d: "M4 2h5l3 3v9H4zM9 2v3h3" } },
            }
        }
    }
}
