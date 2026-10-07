//! Bounded TCP sessions on a caller-owned Tokio runtime with I/O and time enabled.
//!
//! Connections use numeric addresses and default-deny Telnet negotiation. Queued
//! sends are not delivery acknowledgments. Drop signals cancellation; await
//! [`Session::disconnect`] or [`Session::closed`] to verify worker cleanup.

mod driver;

pub use crate::protocols::options::{OptionSnapshot, SessionOptions, TerminalSize};
use crate::protocols::telnet::{DecodeError, TelnetEvent};
use std::{fmt, io, net::SocketAddr, time::Duration};
use tokio::{
    net::TcpStream,
    sync::{mpsc, watch},
    task::JoinHandle,
};

/// Maximum application payload before Telnet escaping.
pub const MAX_SEND_BYTES: usize = 16 * 1024;
/// Maximum queued application sends.
pub const COMMAND_CAPACITY: usize = 32;
/// Maximum encoded frames waiting for the writer.
pub const WRITER_CAPACITY: usize = 32;
/// Maximum decoded events waiting for the consumer.
pub const EVENT_CAPACITY: usize = 128;

/// Caller-assigned identity. Do not reuse for concurrent or replacement sessions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionId(pub u64);

/// Timeouts must be positive. There is deliberately no idle/read timeout.
#[derive(Debug, Clone, Copy)]
pub struct SessionConfig {
    pub connect_timeout: Duration,
    pub write_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            write_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectError {
    InvalidConnectTimeout,
    InvalidWriteTimeout,
    TimedOut,
    Io { kind: io::ErrorKind },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SendError {
    Closed,
    QueueFull,
    TooLarge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IoOperation {
    Read,
    Write,
}

/// Terminal reasons contain no addresses, transcripts, or raw OS error strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    Disconnected,
    OwnerDropped,
    ConsumerDropped,
    PeerEof,
    Protocol(DecodeError),
    Io {
        operation: IoOperation,
        kind: io::ErrorKind,
    },
    WriteTimedOut,
    WorkerFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Connected,
    Closed(CloseReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionStatus {
    pub id: SessionId,
    pub state: SessionState,
}

/// A received event, including uninterpreted negotiation and subnegotiation data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionEvent {
    pub id: SessionId,
    pub event: TelnetEvent,
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConnectTimeout => f.write_str("connection timeout must be positive"),
            Self::InvalidWriteTimeout => f.write_str("write timeout must be positive"),
            Self::TimedOut => f.write_str("connection timed out"),
            Self::Io { kind } => write!(f, "connection failed: {kind:?}"),
        }
    }
}
impl std::error::Error for ConnectError {}

impl fmt::Display for SendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Closed => "session is closed",
            Self::QueueFull => "session command queue is full",
            Self::TooLarge => "send payload exceeds 16 KiB",
        })
    }
}
impl std::error::Error for SendError {}

/// Connects using the current Tokio runtime. No session tasks exist until success.
/// Dropping this future cancels the attempt. No DNS, TLS, or reconnect is performed.
///
/// # Errors
/// Returns an invalid-timeout, connection-timeout, or sanitized I/O error.
pub async fn connect(
    id: SessionId,
    address: SocketAddr,
    config: SessionConfig,
) -> Result<(Session, SessionEvents), ConnectError> {
    connect_with_options(id, address, config, SessionOptions::DenyAll).await
}

/// Opts into implemented Telnet behavior without changing the default connection API.
/// Uses the same timeouts, runtime, bounds and cancellation as [`connect`].
///
/// # Errors
/// Returns the same sanitized errors as [`connect`].
pub async fn connect_with_options(
    id: SessionId,
    address: SocketAddr,
    config: SessionConfig,
    options: SessionOptions,
) -> Result<(Session, SessionEvents), ConnectError> {
    if config.connect_timeout.is_zero() {
        return Err(ConnectError::InvalidConnectTimeout);
    }
    if config.write_timeout.is_zero() {
        return Err(ConnectError::InvalidWriteTimeout);
    }
    let stream = driver::connect_with(TcpStream::connect(address), config.connect_timeout).await?;
    Ok(driver::start_with_options(id, stream, config, options))
}

/// Owning, non-cloneable session control. Dropping it signals asynchronous teardown.
pub struct Session {
    commands: mpsc::Sender<Vec<u8>>,
    viewport: watch::Sender<TerminalSize>,
    options: watch::Receiver<OptionSnapshot>,
    stop: watch::Sender<Option<CloseReason>>,
    status: watch::Receiver<SessionStatus>,
    task: Option<JoinHandle<()>>,
    writer_task: Option<JoinHandle<()>>,
}

impl Session {
    /// Subscribes to effective option state without consuming raw Telnet events.
    pub fn subscribe_options(&self) -> watch::Receiver<OptionSnapshot> {
        self.options.clone()
    }
    /// Replaces the pending viewport; intermediate sizes need not reach the wire.
    ///
    /// # Errors
    /// Returns Closed once cancellation or closure has been observed.
    pub fn update_viewport(&self, size: TerminalSize) -> Result<(), SendError> {
        if self.stop.borrow().is_some() || matches!(self.status().state, SessionState::Closed(_)) {
            return Err(SendError::Closed);
        }
        self.viewport.send_if_modified(|old| {
            if *old == size {
                false
            } else {
                *old = size;
                true
            }
        });
        Ok(())
    }

    /// Enqueues raw data, without newline conversion. Success means accepted only.
    ///
    /// # Errors
    /// Reports closure, input over 16 KiB, or a full command queue without copying
    /// the payload. Empty data is an accepted no-op when the session is open.
    pub fn try_send_data(&self, data: &[u8]) -> Result<(), SendError> {
        if self.stop.borrow().is_some() || matches!(self.status().state, SessionState::Closed(_)) {
            return Err(SendError::Closed);
        }
        if data.len() > MAX_SEND_BYTES {
            return Err(SendError::TooLarge);
        }
        if data.is_empty() {
            return Ok(());
        }
        let permit = self.commands.try_reserve().map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => SendError::QueueFull,
            mpsc::error::TrySendError::Closed(_) => SendError::Closed,
        })?;
        permit.send(data.to_vec());
        Ok(())
    }

    /// Snapshot independent of received-event queue capacity.
    pub fn status(&self) -> SessionStatus {
        *self.status.borrow()
    }

    /// Waits for closure and joins the coordinator and its owned writer.
    /// Cancelling this wait retains ownership, allowing a later wait or disconnect.
    pub async fn closed(&mut self) -> SessionStatus {
        if let Some(task) = self.task.as_mut() {
            // The coordinator's publication guard also reports unexpected exits.
            let _ = task.await;
            self.task = None;
        }
        if let Some(task) = self.writer_task.as_mut() {
            let _ = task.await;
            self.writer_task = None;
        }
        self.status()
    }

    /// Cancels unsent work and joins workers. Idempotent; does not promise flushing.
    pub async fn disconnect(&mut self) -> SessionStatus {
        request_stop(&self.stop, CloseReason::Disconnected);
        self.closed().await
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        request_stop(&self.stop, CloseReason::OwnerDropped);
    }
}

/// Separate event consumer. Dropping it cancels the session even under backpressure.
pub struct SessionEvents {
    events: mpsc::Receiver<SessionEvent>,
    stop: watch::Sender<Option<CloseReason>>,
}

impl SessionEvents {
    /// Receives the next event. Queued events remain drainable after closure.
    /// Returns `None` once the session has stopped and its queue is drained.
    pub async fn recv(&mut self) -> Option<SessionEvent> {
        self.events.recv().await
    }
}

impl Drop for SessionEvents {
    fn drop(&mut self) {
        request_stop(&self.stop, CloseReason::ConsumerDropped);
    }
}

fn request_stop(sender: &watch::Sender<Option<CloseReason>>, reason: CloseReason) {
    sender.send_if_modified(|current| {
        if current.is_some() {
            false
        } else {
            *current = Some(reason);
            true
        }
    });
}

async fn cancelled(receiver: &mut watch::Receiver<Option<CloseReason>>) -> CloseReason {
    loop {
        if let Some(reason) = *receiver.borrow_and_update() {
            return reason;
        }
        if receiver.changed().await.is_err() {
            return CloseReason::OwnerDropped;
        }
    }
}

#[cfg(test)]
mod tests;
