use std::path::PathBuf;

mod fork;
mod sessions;
mod skills;
mod plugins;
mod mcp;
mod projects;
mod transcript_cwd;
mod trash;
mod usage;

pub use sessions::ClaudeSessionAdapter;
pub use skills::ClaudeSkillAdapter;
pub use plugins::ClaudePluginAdapter;
pub use mcp::ClaudeMcpAdapter;
pub use projects::ClaudeProjectAdapter;
pub use trash::{SystemTrash, TrashBin};

pub fn default_claude_home() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude"))
}

pub fn default_claude_json_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| dirs.home_dir().join(".claude.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_claude_home_ends_with_dot_claude() {
        let path = default_claude_home().expect("a home directory should be resolvable on this platform");
        assert_eq!(path.file_name().unwrap(), ".claude");
    }

    #[test]
    fn default_claude_json_path_is_a_sibling_of_the_claude_directory_not_inside_it() {
        let json_path = default_claude_json_path().expect("a home directory should be resolvable on this platform");
        let claude_home = default_claude_home().expect("a home directory should be resolvable on this platform");
        assert_eq!(json_path.file_name().unwrap(), ".claude.json");
        assert_eq!(json_path.parent(), claude_home.parent());
    }
}
