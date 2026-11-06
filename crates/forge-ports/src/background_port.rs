use std::path::Path;

/// Programs Forge starts in the background with their output kept: a dev server the Preview starts
/// where the Terminal panel cannot run one (Windows).
pub trait BackgroundPort: Send + Sync {
    /// Starts `command` (a command line for the system shell) in `cwd`; gives its id.
    fn start(&self, cwd: &Path, command: &str) -> Result<u64, String>;
    /// The output so far (standard output and error together), the last 100,000 bytes.
    fn output(&self, id: u64) -> Option<String>;
    /// Whether it still runs; `Some(code)` once it ended (`-1` when killed).
    fn exit_code(&self, id: u64) -> Option<Option<i32>>;
    /// Stops it. False when there is no such program or it already ended.
    fn stop(&self, id: u64) -> bool;
}
