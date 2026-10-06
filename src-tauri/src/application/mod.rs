//! Single-connection application ownership, independent of Tauri and rendering.
//! Polling provides bounded delivery; status and cancellation bypass output pressure.

mod endpoint;
mod worker;

use crate::{protocols::presentation::PresentationEvent, sessions::MAX_SEND_BYTES};
pub use endpoint::Endpoint;
use endpoint::Resolver;
use std::{
    collections::VecDeque,
    fmt,
    sync::{Arc, Mutex, MutexGuard},
};
use tokio::sync::{Notify, mpsc, oneshot, watch};

pub const OUTPUT_EVENTS: usize = 1024;
pub const OUTPUT_BYTES: usize = 256 * 1024;
pub const POLL_EVENTS: usize = 256;
pub const POLL_BYTES: usize = 64 * 1024;

/// Sanitized application errors; never retain hostnames, commands, or transcripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppError {
    InvalidHost,
    InvalidPort,
    Busy,
    StaleSession,
    Closed,
    InvalidLine,
    TooLarge,
    QueueFull,
    PollBusy,
    ShuttingDown,
}
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidHost => {
                "Enter an IP address or ASCII hostname, without a URL or whitespace."
            }
            Self::InvalidPort => "Port must be between 1 and 65535.",
            Self::Busy => "A connection is already active or closing.",
            Self::StaleSession => "This connection has been replaced.",
            Self::Closed => "The connection is not open.",
            Self::InvalidLine => "Commands must be one line without control characters.",
            Self::TooLarge => "Command exceeds 16 KiB including its line ending.",
            Self::QueueFull => "The send queue is full. Your draft has been retained.",
            Self::PollBusy => "An output poll is already pending.",
            Self::ShuttingDown => "The application is shutting down.",
        })
    }
}
impl std::error::Error for AppError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Resolving,
    Connecting,
    Connected,
    Disconnecting,
    Closed,
}

/// Fixed application messages; peer input is only present in presentation events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Status {
    pub phase: Phase,
    pub message: String,
}
#[derive(Debug)]
pub struct Poll {
    pub id: u64,
    pub status: Status,
    pub events: Vec<PresentationEvent>,
    pub finished: bool,
}

pub(super) struct SendRequest {
    bytes: Vec<u8>,
    reply: oneshot::Sender<Result<(), AppError>>,
}
pub(super) struct Buffer {
    status: Status,
    events: VecDeque<PresentationEvent>,
    bytes: usize,
    finished: bool,
}
pub(super) struct Connection {
    id: u64,
    buffer: Mutex<Buffer>,
    ready: Notify,
    space: Notify,
    done: Notify,
    cancel: watch::Sender<bool>,
    sends: mpsc::Sender<SendRequest>,
    polling: Arc<tokio::sync::Semaphore>,
}

pub(super) fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(|p| p.into_inner())
}
pub(super) fn event_bytes(event: &PresentationEvent) -> usize {
    if let PresentationEvent::Text(text) = event {
        text.len()
    } else {
        0
    }
}
impl Connection {
    fn set_status(&self, phase: Phase, message: impl Into<String>) {
        lock(&self.buffer).status = Status {
            phase,
            message: message.into(),
        };
        self.ready.notify_one();
    }
    fn cancel(&self) {
        if !lock(&self.buffer).finished {
            self.set_status(Phase::Disconnecting, "Disconnecting…");
            self.cancel.send_replace(true);
        }
    }
    async fn wait(&self) {
        loop {
            let notified = self.done.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if lock(&self.buffer).finished {
                return;
            }
            notified.await;
        }
    }
}

struct Registry {
    next: u64,
    current: Option<Arc<Connection>>,
    shutting_down: bool,
}
/// Owns one connection or attempt. Use `shutdown` before destroying the runtime.
pub struct Application {
    registry: Mutex<Registry>,
    resolver: Resolver,
}
impl Default for Application {
    fn default() -> Self {
        Self::new()
    }
}
impl Application {
    pub fn new() -> Self {
        Self {
            registry: Mutex::new(Registry {
                next: 1,
                current: None,
                shutting_down: false,
            }),
            resolver: Resolver::new(),
        }
    }

    /// Starts background work on the caller's Tokio runtime and returns immediately.
    pub fn start(&self, host: &str, port: u32) -> Result<u64, AppError> {
        let endpoint = Endpoint::parse(host, port)?;
        let mut registry = lock(&self.registry);
        if registry.shutting_down {
            return Err(AppError::ShuttingDown);
        }
        if registry
            .current
            .as_ref()
            .is_some_and(|c| !lock(&c.buffer).finished)
        {
            return Err(AppError::Busy);
        }
        let id = registry.next;
        registry.next = id.checked_add(1).ok_or(AppError::Busy)?;
        let (cancel, stop) = watch::channel(false);
        let (sends, requests) = mpsc::channel(8);
        let connection = Arc::new(Connection {
            id,
            buffer: Mutex::new(Buffer {
                status: Status {
                    phase: Phase::Resolving,
                    message: "Resolving…".into(),
                },
                events: VecDeque::new(),
                bytes: 0,
                finished: false,
            }),
            ready: Notify::new(),
            space: Notify::new(),
            done: Notify::new(),
            cancel,
            sends,
            polling: Arc::new(tokio::sync::Semaphore::new(1)),
        });
        registry.current = Some(connection.clone());
        worker::spawn(connection, endpoint, self.resolver.clone(), stop, requests);
        Ok(id)
    }
    fn connection(&self, id: u64) -> Result<Arc<Connection>, AppError> {
        lock(&self.registry)
            .current
            .as_ref()
            .filter(|c| c.id == id)
            .cloned()
            .ok_or(AppError::StaleSession)
    }
    /// At most one poll per connection; no output is removed until it can return.
    pub async fn poll(&self, id: u64) -> Result<Poll, AppError> {
        let connection = self.connection(id)?;
        let _permit = connection
            .polling
            .clone()
            .try_acquire_owned()
            .map_err(|_| AppError::PollBusy)?;
        let notified = connection.ready.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        let empty = {
            let buffer = lock(&connection.buffer);
            buffer.events.is_empty() && !buffer.finished
        };
        if empty {
            let _ = tokio::time::timeout(std::time::Duration::from_millis(100), notified).await;
        }
        self.connection(id)?;
        let mut buffer = lock(&connection.buffer);
        let mut events = Vec::new();
        let mut bytes = 0;
        while events.len() < POLL_EVENTS {
            let Some(event) = buffer.events.front() else {
                break;
            };
            let size = event_bytes(event);
            if bytes + size > POLL_BYTES {
                break;
            }
            bytes += size;
            events.push(buffer.events.pop_front().expect("front exists"));
        }
        buffer.bytes -= bytes;
        let result = Poll {
            id,
            status: buffer.status.clone(),
            events,
            finished: buffer.finished && buffer.events.is_empty(),
        };
        drop(buffer);
        connection.space.notify_one();
        Ok(result)
    }
    pub async fn send_line(&self, id: u64, text: &str) -> Result<(), AppError> {
        if text.len() > MAX_SEND_BYTES - 2 {
            return Err(AppError::TooLarge);
        }
        if text.chars().any(char::is_control) {
            return Err(AppError::InvalidLine);
        }
        let connection = self.connection(id)?;
        if lock(&connection.buffer).status.phase != Phase::Connected {
            return Err(AppError::Closed);
        }
        let mut bytes = text.as_bytes().to_vec();
        bytes.extend(b"\r\n");
        let (reply, receive) = oneshot::channel();
        connection
            .sends
            .try_send(SendRequest { bytes, reply })
            .map_err(|e| match e {
                mpsc::error::TrySendError::Full(_) => AppError::QueueFull,
                mpsc::error::TrySendError::Closed(_) => AppError::Closed,
            })?;
        receive.await.unwrap_or(Err(AppError::Closed))
    }
    pub async fn disconnect(&self, id: u64) -> Result<(), AppError> {
        let connection = self.connection(id)?;
        connection.cancel();
        connection.wait().await;
        Ok(())
    }
    pub async fn shutdown(&self) {
        let connection = {
            let mut registry = lock(&self.registry);
            registry.shutting_down = true;
            registry.current.clone()
        };
        if let Some(connection) = connection {
            connection.cancel();
            connection.wait().await;
        }
    }
}
impl Drop for Application {
    fn drop(&mut self) {
        if let Some(connection) = &lock(&self.registry).current {
            connection.cancel();
        }
    }
}

#[cfg(test)]
mod tests;
