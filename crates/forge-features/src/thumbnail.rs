//! Thumbnails for attachment cards: the image is read off the UI thread and handed to the card as a
//! `data:` address. The cards themselves (forge-entities) do no reading.

use std::path::Path;
use std::rc::Rc;

use base64::Engine;
use dioxus::prelude::*;
use forge_domain::{Attachment, AttachmentKind};
use forge_entities::{AttachmentCard, ThumbnailLoader};

use crate::blocking::run_blocking;

/// The biggest image that gets a thumbnail; a bigger one is not read at all.
pub const THUMBNAIL_LIMIT: u64 = 1_000_000;

/// Whether a card may show a thumbnail: an image of known size within the limit.
pub fn thumbnail_allowed(kind: AttachmentKind, size: Option<u64>) -> bool {
    kind == AttachmentKind::Image && size.is_some_and(|size| size <= THUMBNAIL_LIMIT)
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

/// Reads the thumbnail in the background; no work at all for a file that is not an image.
async fn load_thumbnail(attachment: Attachment) -> Option<String> {
    if attachment.kind != AttachmentKind::Image {
        return None;
    }
    run_blocking(move || thumbnail_data_url(&attachment)).await.flatten()
}

/// Makes the loader the sent messages' cards ask for their thumbnails.
pub fn provide_thumbnail_loader() {
    use_context_provider(|| ThumbnailLoader(Rc::new(|attachment| Box::pin(load_thumbnail(attachment)))));
}

/// A removable card in the composer, with its thumbnail loaded in the background.
#[component]
pub fn ComposerAttachment(attachment: Attachment, on_remove: EventHandler<()>) -> Element {
    let for_load = attachment.clone();
    // The answer carries its path so a card never shows the thumbnail of another file.
    let loaded = use_resource(use_reactive!(|(for_load)| async move {
        let thumbnail = load_thumbnail(for_load.clone()).await;
        (for_load.path, thumbnail)
    }));
    let thumbnail = loaded.read().clone().and_then(|(path, thumbnail)| if path == attachment.path { thumbnail } else { None });
    rsx! { AttachmentCard { attachment, thumbnail, on_remove: Some(on_remove) } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn a_thumbnail_needs_an_image_of_known_size_within_a_megabyte() {
        assert!(thumbnail_allowed(AttachmentKind::Image, Some(0)));
        assert!(thumbnail_allowed(AttachmentKind::Image, Some(THUMBNAIL_LIMIT)));
        assert!(!thumbnail_allowed(AttachmentKind::Image, Some(THUMBNAIL_LIMIT + 1)));
        assert!(!thumbnail_allowed(AttachmentKind::Image, None));
        assert!(!thumbnail_allowed(AttachmentKind::File, Some(10)));
    }

    #[test]
    fn a_thumbnail_is_a_data_url_and_a_missing_or_big_file_has_none() {
        let dir = tempfile::tempdir().unwrap();
        let small = dir.path().join("a.png");
        std::fs::write(&small, [1u8, 2, 3]).unwrap();
        let big = dir.path().join("big.png");
        std::fs::write(&big, vec![0u8; THUMBNAIL_LIMIT as usize + 1]).unwrap();
        let text = dir.path().join("t.txt");
        std::fs::write(&text, "x").unwrap();
        let url = thumbnail_data_url(&Attachment { path: small, kind: AttachmentKind::Image });
        assert_eq!(url.as_deref(), Some("data:image/png;base64,AQID"));
        assert_eq!(thumbnail_data_url(&Attachment { path: big, kind: AttachmentKind::Image }), None);
        assert_eq!(thumbnail_data_url(&Attachment { path: text, kind: AttachmentKind::File }), None);
        let missing = Attachment { path: PathBuf::from("/nope/missing.png"), kind: AttachmentKind::Image };
        assert_eq!(thumbnail_data_url(&missing), None);
    }
}
