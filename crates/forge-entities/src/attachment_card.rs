//! The card of an attached file: a thumbnail for an image, the name (up to three lines), a type tag.
//! One card for the composer (removable) and the sent message (read only).

use std::path::Path;

use base64::Engine;
use dioxus::prelude::*;
use forge_domain::{Attachment, AttachmentKind};

use crate::mention::{shorten_middle, type_tag};

/// The biggest image that gets a thumbnail; a bigger one is not read at all.
pub const THUMBNAIL_LIMIT: u64 = 1_000_000;
/// The characters a name may have on a card: three lines of about 24.
const NAME_CHARS: usize = 72;

/// Whether a card may show a thumbnail: an image of known size within the limit.
pub fn thumbnail_allowed(kind: AttachmentKind, size: Option<u64>) -> bool {
    kind == AttachmentKind::Image && size.is_some_and(|size| size <= THUMBNAIL_LIMIT)
}

/// The name shown on a card and its tag.
pub fn card_label(attachment: &Attachment) -> (String, String) {
    (shorten_middle(&attachment.file_name(), NAME_CHARS), type_tag(&attachment.path.to_string_lossy()))
}

fn image_mime(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_string_lossy().to_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        _ => None,
    }
}

/// The `data:` address of an image attachment, or `None` when it is no image, too big, or unreadable.
/// Reads the file, so call it off the UI thread.
pub fn thumbnail_data_url(attachment: &Attachment) -> Option<String> {
    let size = std::fs::metadata(&attachment.path).ok().map(|meta| meta.len());
    if !thumbnail_allowed(attachment.kind, size) {
        return None;
    }
    let mime = image_mime(&attachment.path)?;
    let bytes = std::fs::read(&attachment.path).ok()?;
    // The file may have grown since the size was read.
    if bytes.len() as u64 > THUMBNAIL_LIMIT {
        return None;
    }
    Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

/// An attached file as a card. With `on_remove` it has a remove button (the composer); without, it
/// only shows (the sent message). The thumbnail of an image is read in the background.
#[component]
pub fn AttachmentCard(attachment: Attachment, on_remove: Option<EventHandler<()>>) -> Element {
    let for_thumbnail = attachment.clone();
    let thumbnail = use_resource(use_reactive!(|(for_thumbnail)| async move {
        tokio::task::spawn_blocking(move || thumbnail_data_url(&for_thumbnail)).await.ok().flatten()
    }));
    let (name, tag) = card_label(&attachment);
    let src = thumbnail.read().clone().flatten();
    rsx! {
        div { class: "attach-card", title: "{attachment.path.display()}",
            if let Some(src) = src {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn attachment(path: &str, kind: AttachmentKind) -> Attachment {
        Attachment { path: PathBuf::from(path), kind }
    }

    #[test]
    fn a_thumbnail_needs_an_image_of_known_size_within_a_megabyte() {
        assert!(thumbnail_allowed(AttachmentKind::Image, Some(0)));
        assert!(thumbnail_allowed(AttachmentKind::Image, Some(THUMBNAIL_LIMIT)));
        assert!(!thumbnail_allowed(AttachmentKind::Image, Some(THUMBNAIL_LIMIT + 1)));
        assert!(!thumbnail_allowed(AttachmentKind::Image, None));
        assert!(!thumbnail_allowed(AttachmentKind::File, Some(10)));
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

    #[test]
    fn a_thumbnail_is_a_data_url_and_a_missing_or_big_file_has_none() {
        let dir = std::env::temp_dir().join(format!("forge-card-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let small = dir.join("a.png");
        std::fs::write(&small, [1u8, 2, 3]).unwrap();
        let big = dir.join("big.png");
        std::fs::write(&big, vec![0u8; THUMBNAIL_LIMIT as usize + 1]).unwrap();
        let text = dir.join("t.txt");
        std::fs::write(&text, "x").unwrap();
        let url = thumbnail_data_url(&Attachment { path: small, kind: AttachmentKind::Image });
        assert_eq!(url.as_deref(), Some("data:image/png;base64,AQID"));
        assert_eq!(thumbnail_data_url(&Attachment { path: big, kind: AttachmentKind::Image }), None);
        assert_eq!(thumbnail_data_url(&Attachment { path: text, kind: AttachmentKind::File }), None);
        assert_eq!(thumbnail_data_url(&attachment("/nope/missing.png", AttachmentKind::Image)), None);
        let _ = std::fs::remove_dir_all(dir);
    }
}
