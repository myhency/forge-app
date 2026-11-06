use std::path::{Path, PathBuf};

use async_trait::async_trait;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// Desktop extensions (`.mcpb`, `.dxt`): unpacks one where Forge keeps them.
#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait ExtensionBundlePort: Send + Sync {
    /// Unpacks `archive` into the extensions folder under the name its manifest gives, in place of an
    /// earlier copy. Gives the folder and the text of its `manifest.json`; an error is for the person.
    async fn unpack(&self, archive: &Path) -> Result<(PathBuf, String), String>;
    /// Forge's own Node program, for an extension whose command is `node`.
    fn node_program(&self) -> Option<PathBuf>;
}
