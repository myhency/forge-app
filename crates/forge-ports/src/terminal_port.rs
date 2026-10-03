use std::path::{Path, PathBuf};

use thiserror::Error;
use tokio::sync::mpsc::UnboundedReceiver;

#[cfg(any(test, feature = "mocks"))]
use mockall::automock;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TerminalId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEvent {
    /// Raw bytes from the shell. A multi-byte character can be split between two events.
    Output(Vec<u8>),
    /// The shell ended; the exit code when it is known. Always the last event.
    Exited(Option<u32>),
}

#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("Working directory \"{}\" does not exist", .0.display())]
    MissingDirectory(PathBuf),
    #[error("{0}")]
    Failed(String),
}

/// A shell that was started: its id and the stream of what it printed.
pub struct TerminalSession {
    pub id: TerminalId,
    pub events: UnboundedReceiver<TerminalEvent>,
}

/// Runs shells in a pseudo-terminal. Calls return at once (a write to a pty does not wait).
#[cfg_attr(any(test, feature = "mocks"), automock)]
pub trait TerminalPort: Send + Sync {
    fn open(&self, cwd: &Path, size: TerminalSize) -> Result<TerminalSession, TerminalError>;
    fn write(&self, id: TerminalId, data: &[u8]) -> Result<(), TerminalError>;
    fn resize(&self, id: TerminalId, size: TerminalSize) -> Result<(), TerminalError>;
    /// Ends the shell. A terminal that is already closed is ignored.
    fn close(&self, id: TerminalId);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_directory_reads_as_the_message_the_panel_shows() {
        let error = TerminalError::MissingDirectory(PathBuf::from("/work/gone"));
        assert_eq!(error.to_string(), "Working directory \"/work/gone\" does not exist");
    }

    #[test]
    fn the_mock_opens_a_session_whose_events_can_be_read() {
        let mut mock = MockTerminalPort::new();
        mock.expect_open().returning(|_, _| {
            let (sender, events) = tokio::sync::mpsc::unbounded_channel();
            sender.send(TerminalEvent::Output(b"hi".to_vec())).unwrap();
            Ok(TerminalSession { id: TerminalId(7), events })
        });
        let mut session = mock.open(Path::new("/work/a"), TerminalSize { cols: 80, rows: 24 }).unwrap();
        assert_eq!(session.id, TerminalId(7));
        assert_eq!(session.events.try_recv().unwrap(), TerminalEvent::Output(b"hi".to_vec()));
    }
}
