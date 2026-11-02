#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

/// One note Claude keeps for a folder: a file in its `memory` directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryEntry {
    /// The file name inside the memory directory, such as `user-role.md`.
    pub file: String,
    /// The `name` from the file's header; the file name without `.md` when it has none.
    pub name: String,
    /// The one-line `description` from the header (blank when there is none).
    pub description: String,
    /// The `type` from the header (`user`, `feedback`, `project`, `reference`), blank when there is none.
    pub kind: String,
    /// When the file last changed, in seconds since 1970.
    pub updated: i64,
}

/// The notes of one folder.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MemoryProject {
    /// The name of the folder under `~/.claude/projects`; it identifies the project in the calls below.
    pub key: String,
    /// The last part of the folder's path, for a title.
    pub label: String,
    /// The folder's full path, as far as it is known.
    pub path: String,
    /// Newest first.
    pub entries: Vec<MemoryEntry>,
}

/// Reads and edits what Claude Code's auto memory keeps on this computer
/// (`~/.claude/projects/<folder>/memory/*.md`, with `MEMORY.md` as the index).
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait MemoryPort: Send + Sync {
    /// Every folder that has at least one note, the one changed last first.
    fn list(&self) -> Result<Vec<MemoryProject>, String>;
    /// The text of one note.
    fn read(&self, project: &str, file: &str) -> Result<String, String>;
    /// Replaces the text of one note.
    fn write(&self, project: &str, file: &str, text: &str) -> Result<(), String>;
    /// Removes one note and its line in the index.
    fn delete(&self, project: &str, file: &str) -> Result<(), String>;
}
