use std::path::{Path, PathBuf};
use std::time::SystemTime;

use forge_ports::McpLogPort;

/// Finds the logs Claude Code keeps for each MCP server: `<cache>/<project>/mcp-logs-<server>/<time>.jsonl`.
pub struct ClaudeMcpLogAdapter {
    cache_root: Option<PathBuf>,
    claude_json_path: PathBuf,
}

impl ClaudeMcpLogAdapter {
    pub fn new(cache_root: Option<PathBuf>, claude_json_path: PathBuf) -> Self {
        Self { cache_root, claude_json_path }
    }
}

/// How Claude Code spells a server name in a folder name: every character that is not a letter or a digit is `-`.
fn folder_name(server: &str) -> String {
    let spelled: String = server.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '-' }).collect();
    format!("mcp-logs-{spelled}")
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok()?.modified().ok()
}

impl McpLogPort for ClaudeMcpLogAdapter {
    fn latest_log(&self, server: &str) -> Option<PathBuf> {
        let wanted = folder_name(server);
        let mut newest: Option<(SystemTime, PathBuf)> = None;
        for project in std::fs::read_dir(self.cache_root.as_ref()?).ok()?.flatten() {
            let Ok(logs) = std::fs::read_dir(project.path().join(&wanted)) else { continue };
            for log in logs.flatten() {
                let path = log.path();
                if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                    continue;
                }
                let Some(time) = modified(&path) else { continue };
                if newest.as_ref().is_none_or(|(best, _)| time > *best) {
                    newest = Some((time, path));
                }
            }
        }
        newest.map(|(_, path)| path)
    }

    fn config_file(&self) -> PathBuf {
        self.claude_json_path.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    fn write(root: &Path, project: &str, folder: &str, file: &str, age_secs: u64) -> PathBuf {
        let dir = root.join(project).join(folder);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join(file);
        fs::write(&path, "{}\n").unwrap();
        let time = SystemTime::now() - Duration::from_secs(age_secs);
        fs::File::options().write(true).open(&path).unwrap().set_modified(time).unwrap();
        path
    }

    #[test]
    fn the_newest_log_over_every_project_wins() {
        let cache = tempfile::tempdir().unwrap();
        write(cache.path(), "-a", "mcp-logs-fs", "old.jsonl", 500);
        let newest = write(cache.path(), "-b", "mcp-logs-fs", "new.jsonl", 5);
        write(cache.path(), "-b", "mcp-logs-other", "newer.jsonl", 1);
        let adapter = ClaudeMcpLogAdapter::new(Some(cache.path().to_path_buf()), PathBuf::from("/x/.claude.json"));
        assert_eq!(adapter.latest_log("fs"), Some(newest));
    }

    #[test]
    fn a_name_is_spelled_the_way_claude_code_spells_it() {
        let cache = tempfile::tempdir().unwrap();
        let log = write(cache.path(), "-a", "mcp-logs-claude-ai-Claude-Docs", "t.jsonl", 5);
        let adapter = ClaudeMcpLogAdapter::new(Some(cache.path().to_path_buf()), PathBuf::from("/x/.claude.json"));
        assert_eq!(adapter.latest_log("claude.ai Claude Docs"), Some(log));
    }

    #[test]
    fn files_that_are_not_logs_are_skipped() {
        let cache = tempfile::tempdir().unwrap();
        write(cache.path(), "-a", "mcp-logs-fs", "notes.txt", 5);
        let adapter = ClaudeMcpLogAdapter::new(Some(cache.path().to_path_buf()), PathBuf::from("/x/.claude.json"));
        assert_eq!(adapter.latest_log("fs"), None);
    }

    #[test]
    fn no_cache_folder_means_no_log() {
        assert_eq!(ClaudeMcpLogAdapter::new(None, PathBuf::from("/x")).latest_log("fs"), None);
        let adapter = ClaudeMcpLogAdapter::new(Some(PathBuf::from("/definitely/not/here")), PathBuf::from("/x"));
        assert_eq!(adapter.latest_log("fs"), None);
    }

    #[test]
    fn the_config_file_is_the_one_given() {
        let adapter = ClaudeMcpLogAdapter::new(None, PathBuf::from("/home/me/.claude.json"));
        assert_eq!(adapter.config_file(), PathBuf::from("/home/me/.claude.json"));
    }
}
