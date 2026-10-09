use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use forge_domain::{Connection, NewConnector, NewTransport};
use forge_ports::{McpManagerError, McpManagerPort};
use tokio::sync::Mutex;

use crate::run::{run, Invocation};

const QUICK: Duration = Duration::from_secs(20);
/// The spec's limit for "Check connection".
const CHECK: Duration = Duration::from_secs(15);

/// Adds, removes and checks MCP servers by running the `claude mcp ...` commands of the Claude Code that
/// ships with the SDK, so the state is the one the terminal's Claude Code uses. User scope only.
pub struct ClaudeMcpCli {
    binary: PathBuf,
    /// Passed as `CLAUDE_CONFIG_DIR`; `None` for the real home.
    config_dir: Option<PathBuf>,
    /// Writes run one at a time: two of them would write `.claude.json` together.
    write_lock: Mutex<()>,
}

impl ClaudeMcpCli {
    pub fn new(binary: PathBuf, config_dir: Option<PathBuf>) -> Self {
        Self { binary, config_dir, write_lock: Mutex::new(()) }
    }

    async fn exec(&self, args: Vec<String>, timeout: Duration) -> Result<String, McpManagerError> {
        let binary = self.binary.clone();
        let config_dir = self.config_dir.clone();
        tokio::task::spawn_blocking(move || {
            let refs: Vec<&str> = args.iter().map(String::as_str).collect();
            run(&Invocation { program: &binary, config_dir: config_dir.as_deref(), args: &refs, timeout })
        })
        .await
        .map_err(|error| McpManagerError::Cli(error.to_string()))?
        .map_err(McpManagerError::from)
    }
}

/// A name that starts with `-` would be read as an option (the CLI itself would take it after `--`, but a
/// connector like that cannot be addressed by later commands).
fn refuse_flag_like(name: &str) -> Result<(), McpManagerError> {
    if name.trim_start().starts_with('-') {
        return Err(McpManagerError::Cli(format!("\"{name}\" cannot start with a dash.")));
    }
    Ok(())
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

/// The arguments of `claude mcp add`, one array element per argument, never joined into a shell line.
fn add_args(connector: &NewConnector) -> Vec<String> {
    let mut args = strings(&["mcp", "add", "-s", "user"]);
    match &connector.transport {
        NewTransport::Http { url, headers } | NewTransport::Sse { url, headers } => {
            let kind = if matches!(connector.transport, NewTransport::Sse { .. }) { "sse" } else { "http" };
            args.extend(strings(&["-t", kind]));
            args.push(connector.name.clone());
            args.push(url.clone());
            for (name, value) in headers {
                args.push("-H".to_string());
                args.push(format!("{name}: {value}"));
            }
        }
        NewTransport::Stdio { command, args: command_args, env } => {
            args.push(connector.name.clone());
            for (key, value) in env {
                args.push("-e".to_string());
                args.push(format!("{key}={value}"));
            }
            args.push("--".to_string());
            args.push(command.clone());
            args.extend(command_args.iter().cloned());
        }
    }
    args
}

/// Takes the connector's secret values out of a CLI message. Longest first, so a short value does not cut a long one.
fn redact(message: String, secrets: &[String]) -> String {
    let mut sorted: Vec<&String> = secrets.iter().filter(|secret| !secret.is_empty()).collect();
    sorted.sort_by_key(|secret| std::cmp::Reverse(secret.len()));
    sorted.into_iter().fold(message, |text, secret| text.replace(secret.as_str(), "[hidden]"))
}

/// Reads the `Status:` (and `Issue:`) line of `claude mcp get`. Nothing else of the output is used or kept:
/// the rest has header and env values in clear.
fn parse_connection(output: &str) -> Connection {
    let field = |label: &str| {
        output
            .lines()
            .map(str::trim)
            .find_map(|line| line.strip_prefix(label))
            .map(|rest| rest.trim().trim_start_matches(|c: char| !c.is_alphanumeric()).trim().to_string())
    };
    let Some(status) = field("Status:") else {
        return Connection::Unknown("No status line in the answer of claude.".to_string());
    };
    let lower = status.to_lowercase();
    if lower.contains("needs auth") {
        Connection::NeedsSignIn
    } else if lower.contains("failed") {
        let issue = field("Issue:").filter(|text| !text.is_empty()).unwrap_or(status);
        Connection::Failed(issue.chars().take(200).collect())
    } else if lower.starts_with("connected") {
        Connection::Connected
    } else {
        Connection::Unknown(status.chars().take(200).collect())
    }
}

#[async_trait]
impl McpManagerPort for ClaudeMcpCli {
    async fn add(&self, connector: &NewConnector) -> Result<(), McpManagerError> {
        refuse_flag_like(&connector.name)?;
        let _one_at_a_time = self.write_lock.lock().await;
        match self.exec(add_args(connector), QUICK).await {
            Ok(_) => Ok(()),
            Err(McpManagerError::Cli(message)) => Err(McpManagerError::Cli(redact(message, &connector.secrets()))),
            Err(other) => Err(other),
        }
    }

    async fn remove(&self, name: &str) -> Result<(), McpManagerError> {
        refuse_flag_like(name)?;
        let _one_at_a_time = self.write_lock.lock().await;
        self.exec(strings(&["mcp", "remove", "-s", "user", name]), QUICK).await.map(|_| ())
    }

    async fn check(&self, name: &str) -> Result<Connection, McpManagerError> {
        refuse_flag_like(name)?;
        let output = self.exec(strings(&["mcp", "get", name]), CHECK).await?;
        Ok(parse_connection(&output))
    }

    fn launch_line(&self) -> Result<String, McpManagerError> {
        if !self.binary.exists() {
            return Err(McpManagerError::BinaryMissing(self.binary.display().to_string()));
        }
        let program = shell_quote(&self.binary.to_string_lossy());
        Ok(match &self.config_dir {
            Some(dir) => format!("CLAUDE_CONFIG_DIR={} {program}", shell_quote(&dir.to_string_lossy())),
            None => program,
        })
    }
}

/// Single quotes around the text; a quote inside it closes, escapes and opens again.
fn shell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remote(sse: bool) -> NewConnector {
        let headers = vec![("X-One".to_string(), "a b:c".to_string()), ("X-Two".to_string(), "한글 'q' \"d\" -x".to_string())];
        let url = "https://a.example/mcp".to_string();
        NewConnector { name: "web".into(), transport: if sse { NewTransport::Sse { url, headers } } else { NewTransport::Http { url, headers } } }
    }

    #[test]
    fn a_remote_connector_is_added_with_name_and_url_first_and_each_header_as_one_argument() {
        assert_eq!(
            add_args(&remote(false)),
            ["mcp", "add", "-s", "user", "-t", "http", "web", "https://a.example/mcp", "-H", "X-One: a b:c", "-H", "X-Two: 한글 'q' \"d\" -x"]
        );
        assert_eq!(add_args(&remote(true))[5], "sse");
    }

    #[test]
    fn a_local_connector_puts_env_before_the_double_dash_and_the_command_after_it() {
        let connector = NewConnector {
            name: "files".into(),
            transport: NewTransport::Stdio {
                command: "npx".into(),
                args: vec!["-y".into(), "two words".into(), "--flag".into()],
                env: vec![("API_KEY".into(), "k 1".into()), ("B".into(), String::new())],
            },
        };
        assert_eq!(
            add_args(&connector),
            ["mcp", "add", "-s", "user", "files", "-e", "API_KEY=k 1", "-e", "B=", "--", "npx", "-y", "two words", "--flag"]
        );
    }

    #[test]
    fn redact_takes_out_the_longest_value_first() {
        assert_eq!(redact("a abc b".to_string(), &["abc".to_string(), "a".to_string()]), "[hidden] [hidden] b");
    }

    #[test]
    fn parse_connection_knows_the_real_outputs() {
        let failed = "web:\n  Scope: User config (available in all your projects)\n  Status: ✘ Failed to connect\n  Issue: ECONNREFUSED: Unable to connect.\n  Type: http\n";
        assert_eq!(parse_connection(failed), Connection::Failed("ECONNREFUSED: Unable to connect.".into()));
        let auth = "auth:\n  Status: ! Needs authentication\n  Type: http\n";
        assert_eq!(parse_connection(auth), Connection::NeedsSignIn);
        assert_eq!(parse_connection("x:\n  Status: ✓ Connected\n"), Connection::Connected);
        assert_eq!(parse_connection("x:\n  Status: ⏸ Pending approval\n"), Connection::Unknown("Pending approval".into()));
        assert_eq!(parse_connection("no status here"), Connection::Unknown("No status line in the answer of claude.".into()));
        assert_eq!(parse_connection("x:\n  Status: ✘ Failed to connect\n"), Connection::Failed("Failed to connect".into()));
    }

    #[test]
    fn a_long_issue_is_cut() {
        let text = format!("x:\n  Status: ✘ Failed to connect\n  Issue: {}\n", "e".repeat(500));
        let Connection::Failed(issue) = parse_connection(&text) else { panic!() };
        assert!(issue.chars().count() <= 200, "{}", issue.chars().count());
    }
}
