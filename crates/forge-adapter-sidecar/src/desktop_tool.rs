use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use forge_ports::{DesktopToolPort, DesktopToolStatus};
use serde::Deserialize;
use tokio::process::Command;

const STATUS_TIMEOUT: Duration = Duration::from_secs(30);
const INSTALL_TIMEOUT: Duration = Duration::from_secs(240);

/// Runs the sidecar's `install_agent_desktop.js` with the Node the sidecar runs on, so installing needs no npm.
pub struct NodeDesktopTool {
    node_binary: String,
    script_path: PathBuf,
    tools_dir: PathBuf,
}

impl NodeDesktopTool {
    /// `sidecar_script` is the sidecar's `index.js`; the installer sits beside it.
    pub fn new(node_binary: String, sidecar_script: &Path, tools_dir: PathBuf) -> Self {
        Self { node_binary, script_path: sidecar_script.with_file_name("install_agent_desktop.js"), tools_dir }
    }

    async fn run(&self, command: &str, timeout: Duration) -> Result<DesktopToolStatus, String> {
        let child = Command::new(&self.node_binary)
            .arg(&self.script_path)
            .arg(command)
            .arg(&self.tools_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| format!("Could not start the installer: {e}"))?;
        let output = tokio::time::timeout(timeout, child.wait_with_output())
            .await
            .map_err(|_| "The installer took too long and was stopped.".to_string())?
            .map_err(|e| format!("The installer failed: {e}"))?;
        parse_reply(&String::from_utf8_lossy(&output.stdout))
    }
}

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default)]
    installed: bool,
    version: Option<String>,
    message: Option<String>,
}

/// The installer prints one JSON line; the last non-empty line is the answer.
pub(crate) fn parse_reply(stdout: &str) -> Result<DesktopToolStatus, String> {
    let line = stdout.lines().map(str::trim).rev().find(|line| !line.is_empty()).ok_or("The installer printed nothing.")?;
    let reply: Reply = serde_json::from_str(line).map_err(|_| "The installer's answer could not be read.".to_string())?;
    if !reply.ok {
        return Err(reply.message.unwrap_or_else(|| "The installer failed.".to_string()));
    }
    match (reply.installed, reply.version) {
        (true, Some(version)) => Ok(DesktopToolStatus::Installed { version }),
        _ => Ok(DesktopToolStatus::Missing),
    }
}

#[async_trait]
impl DesktopToolPort for NodeDesktopTool {
    async fn status(&self) -> Result<DesktopToolStatus, String> {
        self.run("status", STATUS_TIMEOUT).await
    }

    async fn install(&self) -> Result<DesktopToolStatus, String> {
        self.run("install", INSTALL_TIMEOUT).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_installed_reply_carries_the_version() {
        assert_eq!(
            parse_reply("{\"ok\":true,\"installed\":true,\"path\":\"/x\",\"version\":\"1.0.2\"}\n"),
            Ok(DesktopToolStatus::Installed { version: "1.0.2".into() })
        );
    }

    #[test]
    fn a_reply_without_the_program_is_missing() {
        assert_eq!(parse_reply("{\"ok\":true,\"installed\":false}"), Ok(DesktopToolStatus::Missing));
    }

    #[test]
    fn a_failed_reply_gives_its_message_and_noise_before_it_is_ignored() {
        assert_eq!(parse_reply("warning\n{\"ok\":false,\"message\":\"offline\"}\n"), Err("offline".to_string()));
        assert!(parse_reply("").is_err());
        assert!(parse_reply("not json").is_err());
    }

    #[tokio::test]
    async fn a_missing_node_is_an_error_not_a_panic() {
        let tool = NodeDesktopTool::new("/no/such/node".into(), Path::new("/s/index.js"), PathBuf::from("/t"));
        assert!(tool.status().await.unwrap_err().contains("Could not start"));
        assert_eq!(tool.script_path, PathBuf::from("/s/install_agent_desktop.js"));
    }
}
