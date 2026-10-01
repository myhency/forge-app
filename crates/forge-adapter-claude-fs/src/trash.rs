use std::path::Path;

/// Moves a path to the operating system's trash. A trait so tests never touch a real trash.
pub trait TrashBin: Send + Sync {
    fn trash(&self, path: &Path) -> Result<(), String>;
}

/// The real trash (Finder's Trash, the Windows Recycle Bin, the freedesktop trash).
pub struct SystemTrash;

impl TrashBin for SystemTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        // On macOS the default method drives Finder through AppleScript: it needs the Automation
        // permission to control Finder (awkward in a bundled app) and takes seconds. NSFileManager
        // needs neither.
        #[cfg(target_os = "macos")]
        let context = {
            use ::trash::macos::{DeleteMethod, TrashContextExtMacos};
            let mut context = ::trash::TrashContext::new();
            context.set_delete_method(DeleteMethod::NsFileManager);
            context
        };
        #[cfg(not(target_os = "macos"))]
        let context = ::trash::TrashContext::new();
        context.delete(path).map_err(|error| error.to_string())
    }
}
