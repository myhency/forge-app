use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use forge_ports::PluginManagerError;

use crate::parse::clean_message;

/// How long to wait for the pipes to close after the program has exited. A process the program left
/// behind can keep a pipe open for as long as it likes; what was read by then is what counts.
const PIPE_GRACE: Duration = Duration::from_secs(1);

/// A pipe read on its own thread into a shared buffer, so the text read so far can be taken even if the
/// pipe never closes.
struct Drain {
    text: Arc<Mutex<Vec<u8>>>,
    closed: Receiver<()>,
}

impl Drain {
    fn start(mut pipe: impl Read + Send + 'static) -> Drain {
        let text = Arc::new(Mutex::new(Vec::new()));
        let (done, closed) = mpsc::channel();
        let sink = text.clone();
        thread::spawn(move || {
            let mut chunk = [0u8; 4096];
            while let Ok(read) = pipe.read(&mut chunk) {
                if read == 0 {
                    break;
                }
                sink.lock().unwrap().extend_from_slice(&chunk[..read]);
            }
            let _ = done.send(());
        });
        Drain { text, closed }
    }

    /// What was read, once the pipe has closed or `wait` has passed.
    fn finish(self, wait: Duration) -> String {
        let _ = self.closed.recv_timeout(wait);
        String::from_utf8_lossy(&self.text.lock().unwrap()).into_owned()
    }
}

pub(crate) struct Invocation<'a> {
    pub program: &'a Path,
    /// `Some` only when Forge runs against another Claude home (`FORGE_CLAUDE_HOME`).
    pub config_dir: Option<&'a Path>,
    pub args: &'a [&'a str],
    pub timeout: Duration,
}

/// Stops the program and the processes it started.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // A negative pid is the whole process group, which `process_group(0)` made the child's own.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill").args(["/PID", &child.id().to_string(), "/T", "/F"]).output();
    }
    let _ = child.kill();
    let _ = child.wait();
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
    // Its own process group, so a timeout can stop everything the program started (a `git clone`, say).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    match invocation.config_dir {
        Some(dir) => command.env("CLAUDE_CONFIG_DIR", dir),
        None => command.env_remove("CLAUDE_CONFIG_DIR"),
    };
    let mut child = command.spawn().map_err(|error| PluginManagerError::Cli(error.to_string()))?;
    // Both pipes are drained on their own threads so a large output cannot fill a pipe and stop the program.
    let stdout = Drain::start(child.stdout.take().expect("piped"));
    let stderr = Drain::start(child.stderr.take().expect("piped"));
    let deadline = Instant::now() + invocation.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            // The reader threads are left to finish on their own: a grandchild may hold the pipes open.
            Ok(None) if Instant::now() >= deadline => {
                kill_tree(&mut child);
                return Err(PluginManagerError::TimedOut);
            }
            Ok(None) => thread::sleep(Duration::from_millis(10)),
            Err(error) => return Err(PluginManagerError::Cli(error.to_string())),
        }
    };
    let stdout = stdout.finish(PIPE_GRACE);
    let stderr = stderr.finish(PIPE_GRACE);
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

    #[test]
    fn a_process_left_behind_holding_the_pipe_does_not_hang_the_command() {
        let dir = tempfile::tempdir().unwrap();
        // The script exits at once but leaves `sleep` running with stdout and stderr still open.
        let program = script(dir.path(), "sleep 5 &\necho done");
        let started = Instant::now();
        let out = run(&invocation(&program, None, &[], 10)).unwrap();
        assert!(started.elapsed() < Duration::from_secs(4), "took {:?}", started.elapsed());
        assert_eq!(out.trim(), "done");
    }

    #[test]
    fn a_command_that_times_out_takes_the_processes_it_started_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let program = script(dir.path(), &format!("sleep 30 &\necho $! > {}/pid\nwait", dir.path().display()));
        let error = run(&Invocation { program: &program, config_dir: None, args: &[], timeout: Duration::from_millis(2000) }).unwrap_err();
        assert_eq!(error, PluginManagerError::TimedOut);
        let pid = std::fs::read_to_string(dir.path().join("pid")).unwrap().trim().to_string();
        // The kill is asynchronous: give the process a moment to go, under load too.
        let is_alive = || Command::new("kill").args(["-0", &pid]).stderr(Stdio::null()).status().unwrap().success();
        let until = Instant::now() + Duration::from_secs(3);
        while is_alive() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(!is_alive(), "the sleep started by the program (pid {pid}) is still running");
        let _ = Command::new("kill").args(["-9", &pid]).stderr(Stdio::null()).status();
    }
}
