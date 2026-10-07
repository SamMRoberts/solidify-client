use super::*;
use crate::{
    protocols::{presentation::PresentationDecoder, telnet::TelnetEvent},
    sessions::{self, CloseReason, Session, SessionConfig, SessionId, SessionState},
};
use std::time::Duration;

/// Returns ownership to the supervisor even if decoding or the worker panics.
struct SessionGuard {
    session: Session,
    cleanup: Option<oneshot::Sender<Session>>,
}
// The Option form permits transferring Session out from Drop without unsafe code.
struct OwnedSession {
    session: Option<SessionGuard>,
}
impl OwnedSession {
    fn get(&mut self) -> &mut Session {
        &mut self.session.as_mut().expect("session owned").session
    }
}
impl Drop for OwnedSession {
    fn drop(&mut self) {
        if let Some(mut owned) = self.session.take()
            && let Some(cleanup) = owned.cleanup.take()
        {
            let _ = cleanup.send(owned.session);
        }
    }
}

pub(super) fn spawn(
    connection: Arc<Connection>,
    endpoint: Endpoint,
    resolver: Resolver,
    stop: watch::Receiver<bool>,
    requests: mpsc::Receiver<SendRequest>,
) {
    tokio::spawn(async move {
        let (cleanup, receive) = oneshot::channel();
        let task = tokio::spawn(run(
            connection.clone(),
            endpoint,
            resolver,
            stop,
            requests,
            cleanup,
        ));
        let result = task.await;
        if let Ok(mut session) = receive.await {
            session.disconnect().await;
        }
        let message = match result {
            Ok(message) => message,
            Err(_) => "Connection worker stopped unexpectedly.".into(),
        };
        {
            let mut buffer = lock(&connection.buffer);
            buffer.status = Status {
                phase: Phase::Closed,
                message,
            };
            buffer.finished = true;
        }
        connection.ready.notify_one();
        connection.done.notify_waiters();
    });
}

async fn cancelled(stop: &mut watch::Receiver<bool>) {
    loop {
        if *stop.borrow_and_update() {
            return;
        }
        if stop.changed().await.is_err() {
            return;
        }
    }
}

async fn run(
    connection: Arc<Connection>,
    endpoint: Endpoint,
    resolver: Resolver,
    mut stop: watch::Receiver<bool>,
    mut requests: mpsc::Receiver<SendRequest>,
    cleanup: oneshot::Sender<Session>,
) -> String {
    let attempt = async {
        let addresses = resolver.resolve(endpoint).await?;
        connection.set_status(Phase::Connecting, "Connecting…");
        for address in addresses {
            let config = SessionConfig {
                connect_timeout: Duration::from_secs(2),
                ..SessionConfig::default()
            };
            let size = *connection.viewport.borrow();
            if let Ok(pair) = sessions::connect_with_options(
                SessionId(connection.id),
                address,
                config,
                sessions::SessionOptions::MudClient { size },
            )
            .await
            {
                return Ok(pair);
            }
        }
        Err("Could not connect to any resolved address.")
    };
    let (session, mut events) = tokio::select! {
        biased;
        _ = cancelled(&mut stop) => return "Disconnected.".into(),
        result = tokio::time::timeout(Duration::from_secs(10), attempt) => match result {
            Ok(Ok(pair)) => pair,
            Ok(Err(error)) => return error.into(),
            Err(_) => return "Connection timed out after 10 seconds.".into(),
        }
    };
    let mut session = OwnedSession {
        session: Some(SessionGuard {
            session,
            cleanup: Some(cleanup),
        }),
    };
    connection.set_status(Phase::Connected, "Connected.");
    let mut option_state = session.get().subscribe_options();
    let mut options_open = true;
    let mut viewport = connection.viewport.subscribe();
    let _ = session.get().update_viewport(*viewport.borrow_and_update());
    let mut decoder = PresentationDecoder::new();
    let mut pending = VecDeque::new();
    let mut terminal_message = None;
    let mut input_done = false;
    let mut transport_closed = false;
    loop {
        if *stop.borrow() {
            return "Disconnected.".into();
        }
        let snapshot = *option_state.borrow_and_update();
        {
            let mut buffer = lock(&connection.buffer);
            if buffer.options != snapshot {
                buffer.options = snapshot;
                connection.ready.notify_one();
            }
        }
        let space = connection.space.notified();
        tokio::pin!(space);
        space.as_mut().enable();
        if let Some(event) = pending.front() {
            let size = event_bytes(event);
            let enqueued = {
                let mut buffer = lock(&connection.buffer);
                if buffer.events.len() < OUTPUT_EVENTS && buffer.bytes + size <= OUTPUT_BYTES {
                    buffer
                        .events
                        .push_back(pending.pop_front().expect("pending event"));
                    buffer.bytes += size;
                    true
                } else {
                    false
                }
            };
            if enqueued {
                connection.ready.notify_one();
                tokio::task::yield_now().await;
                continue;
            }
        } else if input_done {
            return terminal_message.unwrap_or_else(|| "Connection closed.".into());
        }
        tokio::select! {
            biased;
            _ = cancelled(&mut stop) => return "Disconnected.".into(),
            status = session.get().closed(), if !transport_closed => {
                transport_closed = true;
                let message = match status.state {
                    SessionState::Closed(CloseReason::PeerEof) => "Server closed the connection.",
                    SessionState::Closed(CloseReason::Protocol(_)) => "Connection closed: invalid Telnet stream.",
                    SessionState::Closed(CloseReason::Io { .. }) => "Connection closed: network I/O failed.",
                    SessionState::Closed(CloseReason::WriteTimedOut) => "Connection closed: sending timed out.",
                    SessionState::Closed(CloseReason::WorkerFailed) => "Connection worker stopped unexpectedly.",
                    _ => "Connection closed.",
                }.to_owned();
                if terminal_message.is_none() { terminal_message = Some(message.clone()); }
                connection.set_status(Phase::Closed, message);
            },
            changed = option_state.changed(), if options_open => { options_open = changed.is_ok(); },
            _ = viewport.changed() => {
                let size = *viewport.borrow_and_update();
                let _ = session.get().update_viewport(size);
            },
            request = requests.recv() => if let Some(request) = request {
                let result = session.get().try_send_data(&request.bytes).map_err(|error| match error {
                    sessions::SendError::Closed => AppError::Closed,
                    sessions::SendError::QueueFull => AppError::QueueFull,
                    sessions::SendError::TooLarge => AppError::TooLarge,
                });
                let _ = request.reply.send(result);
            },
            _ = space, if !pending.is_empty() => {},
            received = events.recv(), if pending.is_empty() && !input_done => {
                let result = match received {
                    Some(received) => if let TelnetEvent::Data(data) = received.event {
                        decoder.feed(&data, |event| pending.push_back(event))
                    } else { Ok(()) },
                    None => { input_done = true; decoder.finish(|event| pending.push_back(event)) },
                };
                if let Err(error) = result {
                    let message = format!("Presentation error: {error}");
                    terminal_message = Some(message.clone());
                    connection.set_status(Phase::Closed, message);
                    session.get().disconnect().await;
                    transport_closed = true;
                    input_done = true;
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{io::AsyncReadExt, net::TcpListener};

    #[tokio::test]
    async fn unexpected_task_exit_returns_session_for_awaited_cleanup() {
        tokio::time::timeout(Duration::from_secs(3), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (session, _events) = sessions::connect(
                SessionId(1),
                listener.local_addr().unwrap(),
                SessionConfig::default(),
            )
            .await
            .unwrap();
            let (mut peer, _) = listener.accept().await.unwrap();
            let (cleanup, receive) = oneshot::channel();
            let task = tokio::spawn(async move {
                let _owned = OwnedSession {
                    session: Some(SessionGuard {
                        session,
                        cleanup: Some(cleanup),
                    }),
                };
                panic!("synthetic worker failure");
            });
            assert!(task.await.unwrap_err().is_panic());
            let mut session = receive.await.unwrap();
            session.disconnect().await;
            assert_eq!(peer.read(&mut [0]).await.unwrap(), 0);
        })
        .await
        .expect("cleanup deadline");
    }
}
