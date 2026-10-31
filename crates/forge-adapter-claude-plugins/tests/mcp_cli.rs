//! The `claude mcp` calls, against a stand-in `claude` that writes each argument on its own line.
//! These start processes, so they live here: this binary runs after the library tests, not beside the
//! timing tests of `run.rs`.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use forge_adapter_claude_plugins::ClaudeMcpCli;
use forge_domain::{Connection, NewConnector, NewTransport};
use forge_ports::{McpManagerError, McpManagerPort};

fn cli_with(dir: &Path, body: &str) -> (ClaudeMcpCli, PathBuf) {
    let log = dir.join("calls.log");
    let program = dir.join("fake-claude");
    std::fs::write(&program, format!("#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n' \"$a\" >> {}; done\n{body}\n", log.display())).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    (ClaudeMcpCli::new(program, None), log)
}

fn lines(log: &Path) -> Vec<String> {
    std::fs::read_to_string(log).unwrap().lines().map(String::from).collect()
}

fn remote() -> NewConnector {
    NewConnector {
        name: "web".into(),
        transport: NewTransport::Http {
            url: "https://a.example/mcp".into(),
            headers: vec![("X-One".into(), "a b:c".into()), ("X-Two".into(), "한글 'q' \"d\" -x".into())],
        },
    }
}

#[tokio::test]
async fn add_passes_every_value_as_exactly_one_argument() {
    let dir = tempfile::tempdir().unwrap();
    let (cli, log) = cli_with(dir.path(), "");
    cli.add(&remote()).await.unwrap();
    assert_eq!(
        lines(&log),
        ["mcp", "add", "-s", "user", "-t", "http", "web", "https://a.example/mcp", "-H", "X-One: a b:c", "-H", "X-Two: 한글 'q' \"d\" -x"]
    );
}

#[tokio::test]
async fn remove_runs_mcp_remove_for_the_user_scope() {
    let dir = tempfile::tempdir().unwrap();
    let (cli, log) = cli_with(dir.path(), "");
    cli.remove("web").await.unwrap();
    assert_eq!(lines(&log), ["mcp", "remove", "-s", "user", "web"]);
}

#[tokio::test]
async fn a_name_that_starts_with_a_dash_is_refused_and_nothing_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (cli, log) = cli_with(dir.path(), "");
    assert!(matches!(cli.remove("--help").await, Err(McpManagerError::Cli(_))));
    assert!(matches!(cli.check(" -x").await, Err(McpManagerError::Cli(_))));
    let mut connector = remote();
    connector.name = "-x".into();
    assert!(matches!(cli.add(&connector).await, Err(McpManagerError::Cli(_))));
    assert!(!log.exists());
}

#[tokio::test]
async fn a_failure_that_echoes_a_value_does_not_show_it() {
    let dir = tempfile::tempdir().unwrap();
    let (cli, _) = cli_with(dir.path(), "echo \"bad header Bearer TOPSECRET and SECRET2 rejected\" >&2\nexit 1");
    let connector = NewConnector {
        name: "web".into(),
        transport: NewTransport::Http {
            url: "https://a".into(),
            headers: vec![("Authorization".into(), "Bearer TOPSECRET".into()), ("X".into(), "SECRET2".into())],
        },
    };
    let McpManagerError::Cli(message) = cli.add(&connector).await.unwrap_err() else { panic!("expected Cli") };
    assert!(!message.contains("TOPSECRET") && !message.contains("SECRET2"), "{message}");
    assert!(message.contains("rejected"), "{message}");
}

#[tokio::test]
async fn check_reads_only_the_status_line_of_the_output() {
    let dir = tempfile::tempdir().unwrap();
    let body = "cat <<'EOF'\nweb:\n  Scope: User config\n  Status: ! Needs authentication\n  Type: http\n  Headers:\n    Authorization: Bearer SECRET\nEOF";
    let (cli, log) = cli_with(dir.path(), body);
    assert_eq!(cli.check("web").await.unwrap(), Connection::NeedsSignIn);
    assert_eq!(lines(&log), ["mcp", "get", "web"]);
}

#[test]
fn the_launch_line_quotes_the_program_and_carries_the_config_folder() {
    let dir = tempfile::tempdir().unwrap();
    let program = dir.path().join("it's claude");
    std::fs::write(&program, "").unwrap();
    let plain = ClaudeMcpCli::new(program.clone(), None);
    assert_eq!(plain.launch_line().unwrap(), format!("'{}'", program.display().to_string().replace('\'', r"'\''")));
    let with_dir = ClaudeMcpCli::new(program.clone(), Some(PathBuf::from("/tmp/a b")));
    assert!(with_dir.launch_line().unwrap().starts_with("CLAUDE_CONFIG_DIR='/tmp/a b' '"));
}

#[test]
fn the_launch_line_says_so_when_the_program_is_missing() {
    let cli = ClaudeMcpCli::new(PathBuf::from("/no/such/claude"), None);
    assert!(matches!(cli.launch_line(), Err(McpManagerError::BinaryMissing(_))));
}
