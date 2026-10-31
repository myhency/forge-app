use std::path::PathBuf;

/// What kind of thing a chip stands for. Decided by the extension, not by reading the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    pub path: PathBuf,
    pub kind: AttachmentKind,
}

impl Attachment {
    /// The last component of the path, for a chip. A path with none shows in full.
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map_or_else(|| self.path.to_string_lossy().to_string(), |name| name.to_string_lossy().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_name_is_the_last_component_even_with_spaces_and_hangul() {
        let attachment = Attachment { path: PathBuf::from("/내 문서/사진 1.png"), kind: AttachmentKind::Image };
        assert_eq!(attachment.file_name(), "사진 1.png");
    }

    #[test]
    fn a_path_with_no_file_name_shows_in_full() {
        let attachment = Attachment { path: PathBuf::from("/"), kind: AttachmentKind::File };
        assert_eq!(attachment.file_name(), "/");
    }
}
