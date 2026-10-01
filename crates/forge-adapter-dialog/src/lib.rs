use std::path::PathBuf;

use async_trait::async_trait;
use forge_ports::FolderPickerPort;

/// The operating system's folder dialog, through `rfd`. The dialog itself cannot be tested
/// without a person; spec §19.6 leaves it to the manual checklist.
#[derive(Default)]
pub struct RfdFolderPicker;

impl RfdFolderPicker {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl FolderPickerPort for RfdFolderPicker {
    async fn pick_folder(&self) -> Option<PathBuf> {
        rfd::AsyncFileDialog::new()
            .set_title("Open folder")
            .pick_folder()
            .await
            .map(|handle| handle.path().to_path_buf())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_picker_is_usable_as_a_port_object() {
        let _picker: std::sync::Arc<dyn FolderPickerPort> = std::sync::Arc::new(RfdFolderPicker::new());
    }
}
