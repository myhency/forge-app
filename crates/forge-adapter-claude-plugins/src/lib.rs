mod binary;
mod mcp;
mod parse;
mod run;

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::PluginSnapshot;
use forge_ports::{PluginManagerError, PluginManagerPort};
use tokio::sync::Mutex;

pub use binary::claude_binary_for;
pub use mcp::ClaudeMcpCli;
use run::{run, Invocation};

const QUICK: Duration = Duration::from_secs(20);
const SLOW: Duration = Duration::from_secs(120);

/// Manages plugins and marketplaces by running the `claude plugin ...` commands of the Claude Code
/// that ships with the SDK, so the state is the one the terminal's Claude Code uses.
pub struct ClaudePluginCli {
    binary: PathBuf,
    /// Where `plugins/known_marketplaces.json` is read from.
    claude_home: PathBuf,
    /// Passed as `CLAUDE_CONFIG_DIR`; `None` for the real home (see the plan's Global Constraints).
    config_dir: Option<PathBuf>,
    /// Write commands run one at a time: two of them would write the settings file together.
    write_lock: Mutex<()>,
}

impl ClaudePluginCli {
    pub fn new(binary: PathBuf, claude_home: PathBuf, config_dir: Option<PathBuf>) -> Self {
        Self { binary, claude_home, config_dir, write_lock: Mutex::new(()) }
    }

    async fn exec(&self, args: Vec<String>, timeout: Duration) -> Result<String, PluginManagerError> {
        let binary = self.binary.clone();
        let config_dir = self.config_dir.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run(&Invocation { program: &binary, config_dir: config_dir.as_deref(), args: &refs, timeout })
        })
        .await
        .map_err(|error| PluginManagerError::Cli(error.to_string()))?
    }

    async fn write(&self, args: &[&str], user_text: &str, timeout: Duration) -> Result<(), PluginManagerError> {
        refuse_flag_like(user_text)?;
        let _one_at_a_time = self.write_lock.lock().await;
        self.exec(args.iter().map(|arg| arg.to_string()).collect(), timeout).await.map(|_| ())
    }
}

/// A plugin id, marketplace name or source that starts with `-` would be read as an option.
fn refuse_flag_like(text: &str) -> Result<(), PluginManagerError> {
    if text.trim_start().starts_with('-') {
        return Err(PluginManagerError::Cli(format!("\"{text}\" cannot start with a dash.")));
    }
    Ok(())
}

fn read_manifest_description(install_path: &Path) -> Option<String> {
    let text = std::fs::read_to_string(install_path.join(".claude-plugin").join("plugin.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(value.get("description")?.as_str()?.to_string())
}

#[async_trait]
impl PluginManagerPort for ClaudePluginCli {
    async fn snapshot(&self) -> Result<PluginSnapshot, PluginManagerError> {
        let list = self.exec(["plugin", "list", "--available", "--json"].map(String::from).to_vec(), QUICK).await?;
        let markets = self.exec(["plugin", "marketplace", "list", "--json"].map(String::from).to_vec(), QUICK).await?;
        let known_file = self.claude_home.join("plugins").join("known_marketplaces.json");
        tokio::task::spawn_blocking(move || {
            let dates = parse::parse_last_updated(&std::fs::read_to_string(known_file).unwrap_or_default());
            let mut snapshot = parse::parse_snapshot(&list, &markets, &dates)?;
            parse::fill_descriptions(&mut snapshot, read_manifest_description);
            Ok(snapshot)
        })
        .await
        .map_err(|error| PluginManagerError::Cli(error.to_string()))?
    }

    async fn add_marketplace(&self, source: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "add", source], source, SLOW).await
    }

    async fn remove_marketplace(&self, name: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "remove", name], name, QUICK).await
    }

    async fn update_marketplace(&self, name: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "marketplace", "update", name], name, SLOW).await
    }

    async fn install(&self, id: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "install", id], id, SLOW).await
    }

    async fn uninstall(&self, id: &str) -> Result<(), PluginManagerError> {
        self.write(&["plugin", "uninstall", id], id, QUICK).await
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    // A stand-in `claude` that writes its arguments to a file, so a test can see what was run.
    fn recording_cli(dir: &Path) -> (ClaudePluginCli, PathBuf) {
        let log = dir.join("calls.log");
        let program = dir.join("fake-claude");
        std::fs::write(&program, format!("#!/bin/sh\necho \"$@\" >> {}\n", log.display())).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        (ClaudePluginCli::new(program, dir.join(".claude"), None), log)
    }

    #[tokio::test]
    async fn install_runs_plugin_install_with_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        cli.install("hello@market").await.unwrap();
        assert_eq!(std::fs::read_to_string(log).unwrap().trim(), "plugin install hello@market");
    }

    #[tokio::test]
    async fn a_source_or_id_that_starts_with_a_dash_is_refused_and_nothing_runs() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        for text in ["--help", "-x", "  --version"] {
            assert!(matches!(cli.add_marketplace(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.install(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.uninstall(text).await, Err(PluginManagerError::Cli(_))), "{text}");
            assert!(matches!(cli.remove_marketplace(text).await, Err(PluginManagerError::Cli(_))), "{text}");
        }
        assert!(!log.exists(), "the program must not have been started");
    }

    #[tokio::test]
    async fn a_source_with_spaces_is_one_argument() {
        let dir = tempfile::tempdir().unwrap();
        let (cli, log) = recording_cli(dir.path());
        cli.add_marketplace("/tmp/my market").await.unwrap();
        // The recorder joins arguments with spaces; the point is that no shell was involved.
        assert_eq!(std::fs::read_to_string(log).unwrap().trim(), "plugin marketplace add /tmp/my market");
    }

    #[tokio::test]
    async fn the_manifest_description_is_read_from_the_install_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude-plugin")).unwrap();
        std::fs::write(dir.path().join(".claude-plugin/plugin.json"), r#"{"name":"x","description":"does x"}"#).unwrap();
        assert_eq!(read_manifest_description(dir.path()).as_deref(), Some("does x"));
        assert_eq!(read_manifest_description(&dir.path().join("nope")), None);
    }
}
