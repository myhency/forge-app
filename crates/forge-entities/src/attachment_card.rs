//! The card of an attached file: a thumbnail for an image, the name (up to three lines), a type tag.
//! One card for the composer (removable) and the sent message (read only). Reading the image is not
//! done here: the card takes the `data:` address, and the feature layer provides the loader.

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use dioxus::prelude::*;
use forge_domain::{Attachment, AttachmentKind};

use crate::mention::{shorten_middle, type_tag};

/// The characters a name may have on a card: three lines of about 24.
const NAME_CHARS: usize = 72;

/// Gives a card its thumbnail: the `data:` address of an attached image, `None` when it has none.
/// The feature layer provides it as a context value; the sent message looks it up.
#[derive(Clone)]
pub struct ThumbnailLoader(pub Rc<dyn Fn(Attachment) -> Pin<Box<dyn Future<Output = Option<String>>>>>);

/// The name shown on a card and its tag.
pub fn card_label(attachment: &Attachment) -> (String, String) {
    (shorten_middle(&attachment.file_name(), NAME_CHARS), type_tag(&attachment.path.to_string_lossy()))
}

/// An attached file as a card. With `on_remove` it has a remove button (the composer); without, it
/// only shows (the sent message).
#[component]
pub fn AttachmentCard(attachment: Attachment, thumbnail: Option<String>, on_remove: Option<EventHandler<()>>) -> Element {
    let (name, tag) = card_label(&attachment);
    rsx! {
        div { class: "attach-card", title: "{attachment.path.display()}",
            if let Some(src) = thumbnail {
                img { class: "attach-thumb", src: "{src}", alt: "" }
            }
            span { class: "attach-name", "{name}" }
            span { class: "attach-tag", "{tag}" }
            if let Some(on_remove) = on_remove {
                button { class: "attach-remove", r#type: "button", "aria-label": "Remove", onclick: move |_| on_remove.call(()), "×" }
            }
        }
    }
}

/// A card on a sent message: it asks the provided loader for the thumbnail, if there is one.
#[component]
pub fn SentAttachment(attachment: Attachment) -> Element {
    let loader = try_use_context::<ThumbnailLoader>();
    let for_load = attachment.clone();
    // The answer carries its path so a card never shows the thumbnail of another file.
    let loaded = use_resource(use_reactive!(|(for_load)| {
        let loader = loader.clone();
        async move {
            let thumbnail = match loader {
                Some(loader) => (loader.0)(for_load.clone()).await,
                None => None,
            };
            (for_load.path, thumbnail)
        }
    }));
    let thumbnail = loaded.read().clone().and_then(|(path, thumbnail)| if path == attachment.path { thumbnail } else { None });
    rsx! { AttachmentCard { attachment, thumbnail, on_remove: None } }
}

/// The small picture at the end of a folded line that read one image; nothing until the loader
/// has the image, and nothing for a file it cannot show.
#[component]
pub fn ReadThumbnail(path: String) -> Element {
    let loader = try_use_context::<ThumbnailLoader>();
    let attachment = Attachment { path: std::path::PathBuf::from(&path), kind: AttachmentKind::Image };
    let for_load = attachment.clone();
    let loaded = use_resource(use_reactive!(|(for_load)| {
        let loader = loader.clone();
        async move {
            let thumbnail = match loader {
                Some(loader) => (loader.0)(for_load.clone()).await,
                None => None,
            };
            (for_load.path, thumbnail)
        }
    }));
    let thumbnail = loaded.read().clone().and_then(|(loaded_path, thumbnail)| if loaded_path == attachment.path { thumbnail } else { None });
    match thumbnail {
        Some(src) => rsx! { img { class: "read-thumb", src: "{src}", alt: "" } },
        None => rsx! {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forge_domain::AttachmentKind;
    use std::path::PathBuf;

    fn attachment(path: &str, kind: AttachmentKind) -> Attachment {
        Attachment { path: PathBuf::from(path), kind }
    }

    #[test]
    fn the_label_is_the_shortened_name_and_the_tag() {
        assert_eq!(card_label(&attachment("/a/사진 1.png", AttachmentKind::Image)), ("사진 1.png".to_string(), "PNG".to_string()));
        assert_eq!(card_label(&attachment("/a/Makefile", AttachmentKind::File)).1, "FILE");
        let long = format!("/a/{}.docx", "가".repeat(100));
        let (name, tag) = card_label(&attachment(&long, AttachmentKind::File));
        assert_eq!(name.chars().count(), NAME_CHARS);
        assert!(name.ends_with("종.docx") || name.ends_with("가.docx"));
        assert_eq!(tag, "DOCX");
    }
}
