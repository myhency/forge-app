use std::path::Path;

/// Moves a path to the operating system's trash. A trait so tests never touch a real trash.
pub trait TrashBin: Send + Sync {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// The real trash (Finder's Trash, the Windows Recycle Bin, the freedesktop trash).
pub struct SystemTrash;

impl TrashBin for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        ::trash::delete(path).map_err(|error| error.to_string())
    }
}
