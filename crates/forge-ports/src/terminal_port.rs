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

/// The most output `TerminalSession::recv_batch` puts in one event.
pub const OUTPUT_BATCH_LIMIT: usize = 64 * 1024;

/// A shell that was started: its id and the stream of what it printed.
pub struct TerminalSession {
    pub id: TerminalId,
    pub events: UnboundedReceiver<TerminalEvent>,
    // An event that was read while merging output and has not been handed out yet.
    pending: Option<TerminalEvent>,
}

impl TerminalSession {
    pub fn new(id: TerminalId, events: UnboundedReceiver<TerminalEvent>) -> Self {
        Self { id, events, pending: None }
    }

    /// Waits for the next event. Output that is already queued is merged into it, up to
    /// `OUTPUT_BATCH_LIMIT`, so a program that prints fast costs one screen update per batch and
    /// not one per read. An exit that was met while merging comes next, after that output.
    pub async fn recv_batch(&mut self) -> Option<TerminalEvent> {
        let first = match self.pending.take() {
            Some(event) => event,
            None => self.events.recv().await?,
        };
        let TerminalEvent::Output(mut bytes) = first else {
            return Some(first);
        };
        while bytes.len() < OUTPUT_BATCH_LIMIT {
            match self.events.try_recv() {
                Ok(TerminalEvent::Output(more)) => bytes.extend(more),
                Ok(other) => {
                    self.pending = Some(other);
                    break;
                }
                Err(_) => break,
            }
        }
        Some(TerminalEvent::Output(bytes))
    }
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

    fn session_with(events: Vec<TerminalEvent>) -> TerminalSession {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        for event in events {
            sender.send(event).unwrap();
        }
        // The sender is dropped here: the stream ends after the queued events.
        TerminalSession::new(TerminalId(1), receiver)
    }

    fn output(bytes: &[u8]) -> TerminalEvent {
        TerminalEvent::Output(bytes.to_vec())
    }

    #[tokio::test]
    async fn queued_output_comes_as_one_batch_and_the_exit_after_it() {
        let mut session = session_with(vec![output(b"a"), output(b"b"), output(b"c"), TerminalEvent::Exited(Some(3))]);
        assert_eq!(session.recv_batch().await, Some(output(b"abc")));
        assert_eq!(session.recv_batch().await, Some(TerminalEvent::Exited(Some(3))));
        assert_eq!(session.recv_batch().await, None);
    }

    #[tokio::test]
    async fn a_batch_stops_growing_at_the_limit() {
        let chunk = vec![b'x'; OUTPUT_BATCH_LIMIT / 2 + 1];
        let mut session = session_with(vec![
            TerminalEvent::Output(chunk.clone()),
            TerminalEvent::Output(chunk.clone()),
            TerminalEvent::Output(chunk.clone()),
        ]);
        let Some(TerminalEvent::Output(first)) = session.recv_batch().await else { panic!("expected output") };
        assert_eq!(first.len(), 2 * chunk.len());
        let Some(TerminalEvent::Output(second)) = session.recv_batch().await else { panic!("expected output") };
        assert_eq!(second.len(), chunk.len());
    }

    #[tokio::test]
    async fn an_exit_that_comes_first_is_returned_as_it_is() {
        let mut session = session_with(vec![TerminalEvent::Exited(None)]);
        assert_eq!(session.recv_batch().await, Some(TerminalEvent::Exited(None)));
    }

    #[test]
    fn the_mock_opens_a_session_whose_events_can_be_read() {
        let mut mock = MockTerminalPort::new();
        mock.expect_open().returning(|_, _| {
            let (sender, events) = tokio::sync::mpsc::unbounded_channel();
            sender.send(TerminalEvent::Output(b"hi".to_vec())).unwrap();
            Ok(TerminalSession::new(TerminalId(7), events))
        });
        let mut session = mock.open(Path::new("/work/a"), TerminalSize { cols: 80, rows: 24 }).unwrap();
        assert_eq!(session.id, TerminalId(7));
        assert_eq!(session.events.try_recv().unwrap(), TerminalEvent::Output(b"hi".to_vec()));
    }
}
