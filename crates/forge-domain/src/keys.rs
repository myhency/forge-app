//! The labels of the command-key shortcuts as the person sees them: `⌘K` on macOS, `Ctrl+K` elsewhere.

macro_rules! command_label {
    ($key:literal) => {
        if cfg!(target_os = "macos") { concat!("⌘", $key) } else { concat!("Ctrl+", $key) }
    };
}

/// The menu line that shows a file in the system's file manager.
pub const SHOW_IN_FILE_MANAGER: &str = if cfg!(windows) { "Show in Explorer" } else { "Show in Finder" };
pub const KEY_FIND: &str = command_label!("F");
pub const KEY_ADD_FILES: &str = command_label!("U");
pub const KEY_SEARCH: &str = command_label!("K");
pub const KEY_SETTINGS: &str = command_label!(",");
pub const KEY_CLOSE_SESSION: &str = command_label!("W");
pub const KEY_ANSWER: &str = command_label!("↩");
pub const KEY_ALLOW_ALWAYS: &str = if cfg!(target_os = "macos") { "⇧⌘↩" } else { "Ctrl+Shift+↩" };

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_label_names_the_command_key_of_the_platform() {
        let expected = if cfg!(target_os = "macos") { "⌘K" } else { "Ctrl+K" };
        assert_eq!(KEY_SEARCH, expected);
    }
}
