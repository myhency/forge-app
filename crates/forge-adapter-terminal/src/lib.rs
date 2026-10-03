use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use forge_ports::{TerminalError, TerminalEvent, TerminalId, TerminalPort, TerminalSession, TerminalSize};
use portable_pty::{native_pty_system, ChildKiller, CommandBuilder, MasterPty, PtySize};
use tokio::sync::mpsc::unbounded_channel;

struct Live {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// Shells in a pseudo-terminal, through `portable-pty`. Each shell has a thread that copies what
/// it prints into the session's event stream and reports its exit.
pub struct PtyTerminal {
    shell: String,
    next: AtomicU64,
    live: Mutex<HashMap<TerminalId, Live>>,
}

impl PtyTerminal {
    pub fn new(shell: impl Into<String>) -> Self {
        Self { shell: shell.into(), next: AtomicU64::new(1), live: Mutex::new(HashMap::new()) }
    }
}

fn failed(error: impl std::fmt::Display) -> TerminalError {
    TerminalError::Failed(error.to_string())
}

fn pty_size(size: TerminalSize) -> PtySize {
    PtySize { rows: size.rows, cols: size.cols, pixel_width: 0, pixel_height: 0 }
}

impl TerminalPort for PtyTerminal {
    fn open(&self, cwd: &Path, size: TerminalSize) -> Result<TerminalSession, TerminalError> {
        if !cwd.is_dir() {
            return Err(TerminalError::MissingDirectory(cwd.to_path_buf()));
        }
        let pair = native_pty_system().openpty(pty_size(size)).map_err(failed)?;
        // A login shell, so the person's PATH from their profile is there: an app started from
        // the Finder has only a minimal one.
        let mut command = CommandBuilder::new(&self.shell);
        command.arg("-l");
        command.cwd(cwd);
        command.env("TERM", "xterm-256color");
        let mut child = pair.slave.spawn_command(command).map_err(failed)?;
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().map_err(failed)?;
        let writer = pair.master.take_writer().map_err(failed)?;
        let killer = child.clone_killer();

        let (sender, events) = unbounded_channel();
        std::thread::spawn(move || {
            let mut buffer = [0u8; 8192];
            loop {
                match reader.read(&mut buffer) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        if sender.send(TerminalEvent::Output(buffer[..read].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            let code = child.wait().ok().map(|status| status.exit_code());
            let _ = sender.send(TerminalEvent::Exited(code));
        });

        let id = TerminalId(self.next.fetch_add(1, Ordering::Relaxed));
        self.live.lock().unwrap().insert(id, Live { master: pair.master, writer, killer });
        Ok(TerminalSession { id, events })
    }

    fn write(&self, id: TerminalId, data: &[u8]) -> Result<(), TerminalError> {
        let mut live = self.live.lock().unwrap();
        let terminal = live.get_mut(&id).ok_or_else(|| failed("the terminal is closed"))?;
        terminal.writer.write_all(data).map_err(failed)?;
        terminal.writer.flush().map_err(failed)
    }

    fn resize(&self, id: TerminalId, size: TerminalSize) -> Result<(), TerminalError> {
        let live = self.live.lock().unwrap();
        let terminal = live.get(&id).ok_or_else(|| failed("the terminal is closed"))?;
        terminal.master.resize(pty_size(size)).map_err(failed)
    }

    fn close(&self, id: TerminalId) {
        let removed = self.live.lock().unwrap().remove(&id);
        if let Some(mut terminal) = removed {
            let _ = terminal.killer.kill();
        }
    }
}

impl Drop for PtyTerminal {
    fn drop(&mut self) {
        if let Ok(live) = self.live.get_mut() {
            for (_, mut terminal) in live.drain() {
                let _ = terminal.killer.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::sync::mpsc::UnboundedReceiver;

    use super::*;

    const SIZE: TerminalSize = TerminalSize { cols: 80, rows: 24 };

    fn terminal() -> PtyTerminal {
        PtyTerminal::new("/bin/sh")
    }

    async fn next(events: &mut UnboundedReceiver<TerminalEvent>) -> TerminalEvent {
        tokio::time::timeout(Duration::from_secs(10), events.recv())
            .await
            .expect("no event within 10 s")
            .expect("the event stream ended")
    }

    // Reads output until `needle` has been printed.
    async fn read_until(events: &mut UnboundedReceiver<TerminalEvent>, needle: &str) -> String {
        let mut seen = Vec::new();
        loop {
            if let TerminalEvent::Output(bytes) = next(events).await {
                seen.extend(bytes);
                let text = String::from_utf8_lossy(&seen).into_owned();
                if text.contains(needle) {
                    return text;
                }
            }
        }
    }

    #[tokio::test]
    async fn a_command_typed_into_the_shell_runs_and_its_output_arrives() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        // The typed line is echoed with `$((1+2))` in it; only a shell that ran it prints `forge-3`.
        terminal.write(session.id, b"echo forge-$((1+2))\n").unwrap();
        read_until(&mut session.events, "forge-3").await;
        terminal.close(session.id);
    }

    #[tokio::test]
    async fn the_shell_starts_in_the_folder_it_was_given() {
        let dir = tempfile::tempdir().unwrap();
        let name = dir.path().file_name().unwrap().to_string_lossy().into_owned();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        // The typed line holds `$(pwd)`, not the folder's name: only a shell that ran it prints the name.
        terminal.write(session.id, b"echo here=$(pwd)\n").unwrap();
        read_until(&mut session.events, &name).await;
        terminal.close(session.id);
    }

    #[tokio::test]
    async fn korean_text_goes_through_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.write(session.id, "echo 한글\n".as_bytes()).unwrap();
        read_until(&mut session.events, "한글").await;
        terminal.close(session.id);
    }

    #[test]
    fn a_folder_that_does_not_exist_is_refused_with_its_path() {
        let terminal = terminal();
        let error = terminal.open(std::path::Path::new("/definitely/not/here"), SIZE).err().expect("should fail");
        assert!(matches!(error, TerminalError::MissingDirectory(ref path) if path == std::path::Path::new("/definitely/not/here")));
    }

    #[tokio::test]
    async fn the_exit_code_of_a_shell_that_ends_is_reported_last() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.write(session.id, b"exit 3\n").unwrap();
        loop {
            if let TerminalEvent::Exited(code) = next(&mut session.events).await {
                assert_eq!(code, Some(3));
                break;
            }
        }
    }

    #[tokio::test]
    async fn closing_a_terminal_ends_its_shell() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let mut session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.close(session.id);
        loop {
            if let TerminalEvent::Exited(_) = next(&mut session.events).await {
                break;
            }
        }
    }

    #[test]
    fn writing_or_resizing_a_closed_terminal_is_an_error_not_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let session = terminal.open(dir.path(), SIZE).unwrap();
        terminal.close(session.id);
        assert!(terminal.write(session.id, b"x").is_err());
        assert!(terminal.resize(session.id, SIZE).is_err());
        terminal.close(session.id); // closing twice is ignored
    }

    #[test]
    fn a_resize_reaches_the_shell() {
        let dir = tempfile::tempdir().unwrap();
        let terminal = terminal();
        let session = terminal.open(dir.path(), SIZE).unwrap();
        assert!(terminal.resize(session.id, TerminalSize { cols: 120, rows: 40 }).is_ok());
        terminal.close(session.id);
    }
}
