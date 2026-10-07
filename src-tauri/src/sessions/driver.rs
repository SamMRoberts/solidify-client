use super::*;
use crate::protocols::options::TerminalOptions;
use crate::protocols::telnet::{TelnetDecoder, encode};
use std::future::Future;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const READ_BYTES: usize = 4 * 1024;

pub(super) async fn connect_with<T>(
    connecting: impl Future<Output = io::Result<T>>,
    limit: Duration,
) -> Result<T, ConnectError> {
    tokio::time::timeout(limit, connecting)
        .await
        .map_err(|_| ConnectError::TimedOut)?
        .map_err(|error| ConnectError::Io { kind: error.kind() })
}

/// Aborting a coordinator must not detach its writer; Session retains its join handle.
struct WriterTask {
    completion: tokio::sync::oneshot::Receiver<CloseReason>,
    abort: tokio::task::AbortHandle,
    completed: bool,
}

impl Drop for WriterTask {
    fn drop(&mut self) {
        self.abort.abort();
    }
}

struct StatusPublisher {
    sender: watch::Sender<SessionStatus>,
    id: SessionId,
    published: bool,
}

impl StatusPublisher {
    fn close(&mut self, reason: CloseReason) {
        self.sender.send_replace(SessionStatus {
            id: self.id,
            state: SessionState::Closed(reason),
        });
        self.published = true;
    }
}

impl Drop for StatusPublisher {
    fn drop(&mut self) {
        if !self.published {
            self.close(CloseReason::WorkerFailed);
        }
    }
}

#[cfg(test)]
pub(super) fn start<S>(id: SessionId, stream: S, config: SessionConfig) -> (Session, SessionEvents)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    start_with_options(id, stream, config, SessionOptions::DenyAll)
}

pub(super) fn start_with_options<S>(
    id: SessionId,
    stream: S,
    config: SessionConfig,
    profile: SessionOptions,
) -> (Session, SessionEvents)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let initial_size = match profile {
        SessionOptions::DenyAll => TerminalSize::default(),
        SessionOptions::MudClient { size } => size,
    };
    let (viewport, viewport_rx) = watch::channel(initial_size);
    let (options_tx, options) = watch::channel(OptionSnapshot::default());
    let (commands, command_rx) = mpsc::channel(COMMAND_CAPACITY);
    let (events, event_rx) = mpsc::channel(EVENT_CAPACITY);
    let (stop, mut stop_rx) = watch::channel(None);
    let (status_tx, status) = watch::channel(SessionStatus {
        id,
        state: SessionState::Connected,
    });
    let mut publisher = StatusPublisher {
        sender: status_tx,
        id,
        published: false,
    };
    let task_stop = stop.clone();
    let writer_stop = stop_rx.clone();
    let (mut reader, writer) = tokio::io::split(stream);
    let (frames, frame_rx) = mpsc::channel(WRITER_CAPACITY);
    let (completion_tx, completion) = tokio::sync::oneshot::channel();
    let writer_task = tokio::spawn(async move {
        let reason = write_frames(writer, frame_rx, writer_stop, config.write_timeout).await;
        let _ = completion_tx.send(reason);
    });
    let mut writer = WriterTask {
        completion,
        abort: writer_task.abort_handle(),
        completed: false,
    };
    let task = tokio::spawn(async move {
        let reason = coordinate(
            &mut reader,
            command_rx,
            &events,
            &frames,
            &mut stop_rx,
            &mut writer,
            ProtocolState {
                id,
                decoder: TelnetDecoder::new(),
                handler: TerminalOptions::new(profile),
                viewport: viewport_rx,
                published: options_tx,
            },
        )
        .await;
        request_stop(&task_stop, reason);
        drop(frames);
        if !writer.completed {
            let _ = (&mut writer.completion).await;
        }
        drop(reader);
        drop(events);
        publisher.close(reason);
    });
    (
        Session {
            commands,
            viewport,
            options,
            stop: stop.clone(),
            status,
            task: Some(task),
            writer_task: Some(writer_task),
        },
        SessionEvents {
            events: event_rx,
            stop,
        },
    )
}

async fn interruptible<T>(
    work: impl Future<Output = T>,
    stop: &mut watch::Receiver<Option<CloseReason>>,
    writer: &mut WriterTask,
) -> Result<T, CloseReason> {
    tokio::select! {
        biased;
        reason = cancelled(stop) => Err(reason),
        result = &mut writer.completion => {
            writer.completed = true;
            Err(result.unwrap_or(CloseReason::WorkerFailed))
        }
        result = work => Ok(result),
    }
}

enum Input {
    Read(io::Result<usize>),
    Command(Option<Vec<u8>>),
    Viewport(TerminalSize),
}

struct ProtocolState {
    id: SessionId,
    decoder: TelnetDecoder,
    handler: TerminalOptions,
    viewport: watch::Receiver<TerminalSize>,
    published: watch::Sender<OptionSnapshot>,
}

async fn coordinate<R: AsyncRead + Unpin>(
    reader: &mut R,
    mut commands: mpsc::Receiver<Vec<u8>>,
    events: &mpsc::Sender<SessionEvent>,
    frames: &mpsc::Sender<Vec<u8>>,
    stop: &mut watch::Receiver<Option<CloseReason>>,
    writer: &mut WriterTask,
    mut options: ProtocolState,
) -> CloseReason {
    let mut buffer = [0; READ_BYTES];
    loop {
        let input = interruptible(
            async {
                tokio::select! {
                    read = reader.read(&mut buffer) => Input::Read(read),
                    command = commands.recv() => Input::Command(command),
                    _ = options.viewport.changed() => Input::Viewport(*options.viewport.borrow_and_update()),
                }
            },
            stop,
            writer,
        )
        .await;
        match input {
            Err(reason) => return reason,
            Ok(Input::Viewport(size)) => {
                let mut response = None;
                options
                    .handler
                    .set_size(size, |event| response = Some(event));
                if let Some(event) = response
                    && let Err(reason) = send_frame(event, frames, stop, writer).await
                {
                    return reason;
                }
            }
            Ok(Input::Command(None)) => return CloseReason::OwnerDropped,
            Ok(Input::Command(Some(data))) => {
                if let Err(reason) = send_frame(TelnetEvent::Data(data), frames, stop, writer).await
                {
                    return reason;
                }
            }
            Ok(Input::Read(Err(error))) if error.kind() == io::ErrorKind::Interrupted => continue,
            Ok(Input::Read(Err(error))) => {
                return CloseReason::Io {
                    operation: IoOperation::Read,
                    kind: error.kind(),
                };
            }
            Ok(Input::Read(Ok(0))) => {
                return match options.decoder.finish() {
                    Ok(()) => CloseReason::PeerEof,
                    Err(error) => CloseReason::Protocol(error),
                };
            }
            Ok(Input::Read(Ok(length))) => {
                // At most one 4 KiB read's events plus a completed retained payload.
                // No further read occurs until this finite batch has been delivered.
                let mut batch = Vec::new();
                if let Err(error) = options
                    .decoder
                    .feed(&buffer[..length], |event| batch.push(event))
                {
                    return CloseReason::Protocol(error);
                }
                for event in batch {
                    // At most two bounded responses. Publish before awaiting writer/event capacity.
                    let mut responses = Vec::with_capacity(2);
                    options
                        .handler
                        .receive(&event, |reply| responses.push(reply));
                    let snapshot = options.handler.snapshot();
                    options.published.send_if_modified(|old| {
                        if *old == snapshot {
                            false
                        } else {
                            *old = snapshot;
                            true
                        }
                    });
                    for reply in responses {
                        if let Err(reason) = send_frame(reply, frames, stop, writer).await {
                            return reason;
                        }
                    }
                    match interruptible(
                        events.send(SessionEvent {
                            id: options.id,
                            event,
                        }),
                        stop,
                        writer,
                    )
                    .await
                    {
                        Err(reason) => return reason,
                        Ok(Err(_)) => return CloseReason::ConsumerDropped,
                        Ok(Ok(())) => {}
                    }
                }
            }
        }
    }
}

async fn send_frame(
    event: TelnetEvent,
    frames: &mpsc::Sender<Vec<u8>>,
    stop: &mut watch::Receiver<Option<CloseReason>>,
    writer: &mut WriterTask,
) -> Result<(), CloseReason> {
    // Only bounded Data, negotiation replies, and fixed-size TTYPE/NAWS originate here.
    let mut frame = Vec::new();
    encode(&event, |bytes| frame.extend_from_slice(bytes))
        .expect("session produces valid Telnet events");
    match interruptible(frames.send(frame), stop, writer).await? {
        Ok(()) => Ok(()),
        Err(_) => {
            let reason = (&mut writer.completion)
                .await
                .unwrap_or(CloseReason::WorkerFailed);
            writer.completed = true;
            Err(reason)
        }
    }
}

async fn write_frames<W: AsyncWrite + Unpin>(
    mut writer: W,
    mut frames: mpsc::Receiver<Vec<u8>>,
    mut stop: watch::Receiver<Option<CloseReason>>,
    limit: Duration,
) -> CloseReason {
    loop {
        let frame = tokio::select! {
            biased;
            reason = cancelled(&mut stop) => return reason,
            frame = frames.recv() => match frame {
                Some(frame) => frame,
                None => return CloseReason::OwnerDropped,
            },
        };
        // Cancelling this operation always ends the connection; partially written
        // frames are never restarted or replayed. The deadline covers the full frame.
        let result = tokio::select! {
            biased;
            reason = cancelled(&mut stop) => return reason,
            result = tokio::time::timeout(limit, write_frame(&mut writer, &frame)) => result,
        };
        match result {
            Err(_) => return CloseReason::WriteTimedOut,
            Ok(Err(error)) => {
                return CloseReason::Io {
                    operation: IoOperation::Write,
                    kind: error.kind(),
                };
            }
            Ok(Ok(())) => {}
        }
    }
}

async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, frame: &[u8]) -> io::Result<()> {
    let mut offset = 0;
    while offset < frame.len() {
        match writer.write(&frame[offset..]).await {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => offset += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
