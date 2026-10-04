use base64::Engine;
use dioxus::prelude::*;
use forge_ports::FileContent;

/// What an open tab shows.
#[derive(Debug, Clone, PartialEq)]
pub enum TabKind {
    Loading,
    /// `lossy`: the bytes were not valid UTF-8. `anyway`: "Show as text anyway" on a binary file.
    Text { lossy: bool, anyway: bool },
    Image { data_url: String },
    Binary,
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
            TabKind::Text { lossy: true, .. } => Some("This file is not valid UTF-8, so it is shown read only."),
            TabKind::Text { anyway: true, .. } => Some("Shown as text. This file cannot be edited."),
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
        FileContent::Binary => TabKind::Binary,
        FileContent::TooLarge => TabKind::TooLarge,
    }
}

/// The body of a tab that has no editor: a picture, a binary file, a file that is too large.
#[component]
pub fn FileKindView(kind: TabKind, path: String, on_anyway: EventHandler<()>, on_reveal: EventHandler<()>) -> Element {
    match kind {
        TabKind::Image { data_url } => rsx! {
            div { class: "files-kind",
                img { src: "{data_url}", alt: "{path}" }
            }
        },
        TabKind::Binary => rsx! {
            div { class: "files-kind",
                p { "This file looks like binary data and cannot be previewed as text." }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "Show in Finder" }
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_anyway.call(()), "Show as text anyway" }
                }
            }
        },
        TabKind::TooLarge => rsx! {
            div { class: "files-kind",
                p { class: "files-kind-title", "This file cannot be previewed" }
                p { class: "files-kind-sub", "It is too large to preview here." }
                p { class: "files-kind-sub", "{path}" }
                div { class: "files-kind-buttons",
                    button { class: "files-kind-button", r#type: "button", onclick: move |_| on_reveal.call(()), "Show in Finder" }
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

    fn version() -> FileVersion {
        FileVersion { modified_ns: 1, len: 1 }
    }

    #[test]
    fn text_is_editable_unless_it_was_read_lossily_or_forced() {
        let plain = kind_of(FileContent::Text { text: "a".to_string(), version: version(), lossy: false });
        assert_eq!(plain, TabKind::Text { lossy: false, anyway: false });
        assert!(!plain.read_only());
        assert_eq!(plain.notice(), None);

        let lossy = kind_of(FileContent::Text { text: "a\u{fffd}".to_string(), version: version(), lossy: true });
        assert!(lossy.read_only(), "a file that is not valid UTF-8 must never be written back");
        assert_eq!(lossy.notice(), Some("This file is not valid UTF-8, so it is shown read only."));

        let forced = TabKind::Text { lossy: false, anyway: true };
        assert!(forced.read_only(), "a binary file shown as text must never be written back");
        assert_eq!(forced.notice(), Some("Shown as text. This file cannot be edited."));
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
    fn only_text_has_an_editor() {
        assert!(TabKind::Text { lossy: false, anyway: false }.shows_editor());
        for other in [TabKind::Loading, TabKind::Binary, TabKind::TooLarge, TabKind::Failed("x".to_string())] {
            assert!(!other.shows_editor(), "{other:?}");
        }
    }
}
