use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use forge_ports::PluginManagerError;

use crate::parse::clean_message;

pub(crate) struct Invocation<'a> {
    pub program: &'a Path,
    /// `Some` only when Forge runs against another Claude home (`FORGE_CLAUDE_HOME`).
    pub config_dir: Option<&'a Path>,
    pub args: &'a [&'a str],
    pub timeout: Duration,
}

/// Runs the program, waits at most `timeout`, and returns what it wrote to stdout. A non-zero exit
/// is `Cli` with the message from `clean_message`.
pub(crate) fn run(invocation: &Invocation) -> Result<String, PluginManagerError> {
    if !invocation.program.is_file() {
        return Err(PluginManagerError::BinaryMissing(invocation.program.display().to_string()));
    }
    let mut command = Command::new(invocation.program);
    command
        .args(invocation.args)
        // A private Git server must fail with a message, not wait for a password nobody can type.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match invocation.config_dir {
        Some(dir) => command.env("CLAUDE_CONFIG_DIR", dir),
        None => command.env_remove("CLAUDE_CONFIG_DIR"),
    };
    let mut child = command.spawn().map_err(|error| PluginManagerError::Cli(error.to_string()))?;
    // Both pipes are drained on their own threads so a large output cannot fill a pipe and stop the program.
    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_thread = thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let err_thread = thread::spawn(move || {
        let mut text = String::new();
        let _ = stderr.read_to_string(&mut text);
        text
    });
    let deadline = Instant::now() + invocation.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            // The reader threads are left to finish on their own: a grandchild may hold the pipes open.
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(PluginManagerError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(PluginManagerError::Cli(error.to_string())),
        }
    };
    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    if status.success() {
        Ok(stdout)
    } else {
        Err(PluginManagerError::Cli(clean_message(&stderr, &stdout, status.code())))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("fake-claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn invocation<'a>(program: &'a Path, config_dir: Option<&'a Path>, args: &'a [&'a str], seconds: u64) -> Invocation<'a> {
        Invocation { program, config_dir, args, timeout: Duration::from_secs(seconds) }
    }

    #[test]
    fn a_successful_command_returns_its_stdout() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"args: $@\"");
        let out = run(&invocation(&program, None, &["plugin", "list"], 5)).unwrap();
        assert_eq!(out.trim(), "args: plugin list");
    }

    #[test]
    fn a_failed_command_returns_its_stderr_without_the_cross() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo '✘ Path does not exist: /x' >&2\nexit 1");
        let error = run(&invocation(&program, None, &["x"], 5)).unwrap_err();
        assert_eq!(error, PluginManagerError::Cli("Path does not exist: /x".into()));
    }

    #[test]
    fn a_command_that_runs_too_long_is_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "exec sleep 5");
        let started = Instant::now();
        let error = run(&Invocation { program: &program, config_dir: None, args: &[], timeout: Duration::from_millis(200) }).unwrap_err();
        assert_eq!(error, PluginManagerError::TimedOut);
        assert!(started.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn a_missing_program_is_reported_with_its_path() {
        let error = run(&invocation(Path::new("/no/such/claude"), None, &[], 5)).unwrap_err();
        assert_eq!(error, PluginManagerError::BinaryMissing("/no/such/claude".into()));
    }

    #[test]
    fn the_config_dir_is_passed_only_when_one_is_given() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"${CLAUDE_CONFIG_DIR:-unset}\"");
        let with = run(&invocation(&program, Some(Path::new("/fixture/.claude")), &[], 5)).unwrap();
        assert_eq!(with.trim(), "/fixture/.claude");
        // The variable of the test process itself must not leak through either.
        let without = run(&invocation(&program, None, &[], 5)).unwrap();
        assert_eq!(without.trim(), "unset");
    }

    #[test]
    fn git_is_told_never_to_ask_for_a_password() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), "echo \"$GIT_TERMINAL_PROMPT\"");
        assert_eq!(run(&invocation(&program, None, &[], 5)).unwrap().trim(), "0");
    }
}
