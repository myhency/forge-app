use std::path::{Path, PathBuf};

use forge_domain::{Attachment, AttachmentKind};

/// The API's limit for one image, counted on the base64 data the sidecar sends.
pub const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;

/// The largest image file whose base64 form stays within `MAX_IMAGE_BYTES` (base64 is 4 bytes for
/// every 3).
pub const MAX_IMAGE_FILE_BYTES: u64 = MAX_IMAGE_BYTES / 4 * 3;

const IMAGE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "gif", "webp"];

pub fn kind_for_path(path: &Path) -> AttachmentKind {
    let image = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| IMAGE_EXTENSIONS.contains(&extension.to_lowercase().as_str()));
    if image {
        AttachmentKind::Image
    } else {
        AttachmentKind::File
    }
}

/// Adds the picked files as chips, skipping one that is already there.
pub fn add_attachments(list: &mut Vec<Attachment>, picked: Vec<PathBuf>) {
    for path in picked {
        if list.iter().any(|existing| existing.path == path) {
            continue;
        }
        let kind = kind_for_path(&path);
        list.push(Attachment { path, kind });
    }
}

/// Looks at every chip before a send. `size_of` is the file's size, `None` when it cannot be
/// read. Only images have a size limit: a text file is read by the model's own tool.
pub fn check_attachments(list: &[Attachment], size_of: impl Fn(&Path) -> Option<u64>) -> Result<(), String> {
    for attachment in list {
        let Some(size) = size_of(&attachment.path) else {
            return Err(format!("{} cannot be read.", attachment.file_name()));
        };
        if attachment.kind == AttachmentKind::Image && size > MAX_IMAGE_FILE_BYTES {
            return Err(format!(
                "{} is too large to send as an image: the limit is 5 MB once encoded, about 3.7 MB of file.",
                attachment.file_name()
            ));
        }
    }
    Ok(())
}

/// The text with a line `@folder` for each folder, so the model knows it may use them.
pub fn with_folder_hints(text: &str, folders: &[PathBuf]) -> String {
    let mut lines: Vec<String> = Vec::new();
    if !text.is_empty() {
        lines.push(text.to_string());
    }
    lines.extend(folders.iter().map(|folder| format!("@{}", folder.display())));
    lines.join("\n")
}

/// The name a folder chip shows: the last component, or the whole path for `/`.
pub fn folder_label(folder: &Path) -> String {
    folder.file_name().map_or_else(|| folder.display().to_string(), |name| name.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn att(path: &str) -> Attachment {
        Attachment { path: path.into(), kind: kind_for_path(Path::new(path)) }
    }

    #[test]
    fn the_extension_decides_the_kind_whatever_its_case() {
        assert_eq!(kind_for_path(Path::new("/a/b.PNG")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.jpeg")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.webp")), AttachmentKind::Image);
        assert_eq!(kind_for_path(Path::new("/a/b.svg")), AttachmentKind::File);
        assert_eq!(kind_for_path(Path::new("/a/README")), AttachmentKind::File);
    }

    #[test]
    fn the_same_file_twice_is_one_chip_and_order_is_kept() {
        let mut list = Vec::new();
        add_attachments(&mut list, vec!["/a/1.png".into(), "/a/2.txt".into()]);
        add_attachments(&mut list, vec!["/a/1.png".into(), "/a/3.gif".into()]);
        let names: Vec<String> = list.iter().map(Attachment::file_name).collect();
        assert_eq!(names, ["1.png", "2.txt", "3.gif"]);
    }

    #[test]
    fn an_image_over_the_limit_is_refused_by_name() {
        let list = vec![att("/a/big photo.png")];
        let error = check_attachments(&list, |_| Some(MAX_IMAGE_FILE_BYTES + 1)).unwrap_err();
        assert!(error.contains("big photo.png"), "{error}");
        assert!(error.contains("5 MB"), "{error}");
    }

    #[test]
    fn an_image_that_is_over_the_limit_once_base64_encoded_is_refused() {
        // 4.5 MB of file is about 6 MB of base64, which is what the API counts.
        let list = vec![att("/a/photo.png")];
        let error = check_attachments(&list, |_| Some(4_500_000)).unwrap_err();
        assert!(error.contains("photo.png"), "{error}");
    }

    #[test]
    fn an_image_that_fits_once_encoded_and_a_huge_text_file_are_fine() {
        let list = vec![att("/a/ok.png"), att("/a/huge.log")];
        assert!(check_attachments(&list, |path| {
            Some(if path.ends_with("ok.png") { MAX_IMAGE_FILE_BYTES } else { u64::MAX })
        })
        .is_ok());
        assert!(check_attachments(&list[..1], |_| Some(MAX_IMAGE_FILE_BYTES + 1)).is_err());
    }

    #[test]
    fn a_file_that_is_gone_is_refused_by_name() {
        let list = vec![att("/a/gone.txt")];
        let error = check_attachments(&list, |_| None).unwrap_err();
        assert!(error.contains("gone.txt"), "{error}");
    }

    #[test]
    fn folders_become_at_lines_after_the_text_and_a_path_with_spaces_is_kept() {
        let text = with_folder_hints("fix it", &[PathBuf::from("/내 문서/proj a")]);
        assert_eq!(text, "fix it\n@/내 문서/proj a");
        assert_eq!(with_folder_hints("", &[PathBuf::from("/x")]), "@/x");
        assert_eq!(with_folder_hints("same", &[]), "same");
    }

    #[test]
    fn a_folder_chip_shows_the_last_component_and_root_shows_in_full() {
        assert_eq!(folder_label(Path::new("/내 문서/proj a")), "proj a");
        assert_eq!(folder_label(Path::new("/")), "/");
    }
}
