use super::*;
use std::{
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};

#[derive(Default)]
struct Probe {
    written: Mutex<Vec<u8>>,
    reads: AtomicUsize,
    dropped: AtomicBool,
}

enum Writes {
    Short(usize),
    StallAfter(usize),
    Fail(io::ErrorKind),
    FailAfter(usize),
    Zero,
    Panic,
}

struct TestIo {
    input: Vec<u8>,
    position: usize,
    read_error: Option<io::ErrorKind>,
    eof: bool,
    writes: Writes,
    probe: Arc<Probe>,
}

impl TestIo {
    fn new(input: Vec<u8>, writes: Writes) -> (Self, Arc<Probe>) {
        let probe = Arc::new(Probe::default());
        (
            Self {
                input,
                position: 0,
                read_error: None,
                eof: false,
                writes,
                probe: probe.clone(),
            },
            probe,
        )
    }
}

impl AsyncRead for TestIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if let Some(kind) = self.read_error.take() {
            return Poll::Ready(Err(kind.into()));
        }
        if self.position < self.input.len() {
            let end = self.input.len().min(self.position + buffer.remaining());
            buffer.put_slice(&self.input[self.position..end]);
            self.position = end;
            self.probe.reads.fetch_add(1, Ordering::SeqCst);
            Poll::Ready(Ok(()))
        } else if self.eof {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

impl AsyncWrite for TestIo {
    fn poll_write(
        self: Pin<&mut Self>,
        _: &mut Context<'_>,
        data: &[u8],
    ) -> Poll<io::Result<usize>> {
        let mut written = self.probe.written.lock().unwrap();
        let count = match self.writes {
            Writes::Short(max) => data.len().min(max),
            Writes::StallAfter(max) if written.len() >= max => return Poll::Pending,
            Writes::StallAfter(max) => data.len().min(max - written.len()),
            Writes::Fail(kind) => return Poll::Ready(Err(kind.into())),
            Writes::FailAfter(max) if written.len() >= max => {
                return Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()));
            }
            Writes::FailAfter(max) => data.len().min(max - written.len()),
            Writes::Zero => return Poll::Ready(Ok(0)),
            Writes::Panic => panic!("synthetic writer panic"),
        };
        written.extend_from_slice(&data[..count]);
        Poll::Ready(Ok(count))
    }
    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
    fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

impl Drop for TestIo {
    fn drop(&mut self) {
        self.probe.dropped.store(true, Ordering::SeqCst);
    }
}

async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("bounded test wait expired")
}

async fn scheduled_until(mut condition: impl FnMut() -> bool) {
    for _ in 0..1000 {
        if condition() {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("worker did not reach expected state within scheduling bound");
}

fn reason(status: SessionStatus) -> CloseReason {
    match status.state {
        SessionState::Closed(reason) => reason,
        other => panic!("expected closure, got {other:?}"),
    }
}

#[tokio::test]
async fn validates_configuration_before_connecting() {
    let address = "127.0.0.1:1".parse().unwrap();
    for (config, expected) in [
        (
            SessionConfig {
                connect_timeout: Duration::ZERO,
                ..SessionConfig::default()
            },
            ConnectError::InvalidConnectTimeout,
        ),
        (
            SessionConfig {
                write_timeout: Duration::ZERO,
                ..SessionConfig::default()
            },
            ConnectError::InvalidWriteTimeout,
        ),
    ] {
        assert!(
            matches!(connect(SessionId(1), address, config).await, Err(error) if error == expected)
        );
    }
}

struct PendingConnect(Arc<AtomicBool>);
impl Future for PendingConnect {
    type Output = io::Result<()>;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}
impl Drop for PendingConnect {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[tokio::test(start_paused = true)]
async fn connection_deadline_drops_the_attempt() {
    let dropped = Arc::new(AtomicBool::new(false));
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(11),
            driver::connect_with(PendingConnect(dropped.clone()), Duration::from_secs(10))
        )
        .await
        .expect("connection test deadline"),
        Err(ConnectError::TimedOut)
    );
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn cancelling_connection_future_drops_the_attempt_without_workers() {
    let dropped = Arc::new(AtomicBool::new(false));
    let mut attempt = Box::pin(driver::connect_with(
        PendingConnect(dropped.clone()),
        Duration::from_secs(10),
    ));
    tokio::select! { biased; _ = &mut attempt => panic!("unexpected completion"), _ = tokio::task::yield_now() => {} }
    drop(attempt);
    assert!(dropped.load(Ordering::SeqCst));
    assert_eq!(
        driver::connect_with(
            async {
                Err::<(), _>(io::Error::new(
                    io::ErrorKind::ConnectionRefused,
                    "private endpoint",
                ))
            },
            Duration::from_secs(1)
        )
        .await,
        Err(ConnectError::Io {
            kind: io::ErrorKind::ConnectionRefused
        })
    );
}

#[tokio::test]
async fn short_writes_preserve_frame_order_and_escaping_then_join_cleanly() {
    let (io, probe) = TestIo::new(vec![], Writes::Short(1));
    let (mut session, _events) = driver::start(SessionId(7), io, SessionConfig::default());
    session.try_send_data(&[b'a', 255, b'b']).unwrap();
    session.try_send_data(b"\r\nnext").unwrap();
    let expected = b"a\xff\xffb\r\nnext";
    scheduled_until(|| probe.written.lock().unwrap().len() == expected.len()).await;
    assert_eq!(*probe.written.lock().unwrap(), expected);
    let status = bounded(session.disconnect()).await;
    assert_eq!(status.id, SessionId(7));
    assert_eq!(reason(status), CloseReason::Disconnected);
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert_eq!(session.disconnect().await, status);
    assert_eq!(session.try_send_data(b"late"), Err(SendError::Closed));
}

#[tokio::test]
async fn application_queue_and_payload_are_bounded_before_copying() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(0));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    assert_eq!(
        session.try_send_data(&vec![0; MAX_SEND_BYTES + 1]),
        Err(SendError::TooLarge)
    );
    session.try_send_data(b"").unwrap();
    for _ in 0..COMMAND_CAPACITY {
        session.try_send_data(&vec![255; MAX_SEND_BYTES]).unwrap();
    }
    assert_eq!(session.try_send_data(b"full"), Err(SendError::QueueFull));
    assert_eq!(
        reason(bounded(session.disconnect()).await),
        CloseReason::Disconnected
    );
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn full_event_queue_pauses_reads_then_recovers_without_loss() {
    // Each two-byte command emits an event, ensuring one read fills the event queue.
    let input = [255, 241, 255, 242].repeat(1500);
    let (io, probe) = TestIo::new(input, Writes::Short(16));
    let (mut session, mut events) = driver::start(SessionId(8), io, SessionConfig::default());
    scheduled_until(|| events.events.len() == EVENT_CAPACITY).await;
    assert_eq!(probe.reads.load(Ordering::SeqCst), 1);
    for _ in 0..20 {
        tokio::task::yield_now().await;
    }
    assert_eq!(probe.reads.load(Ordering::SeqCst), 1);
    for index in 0..3000 {
        assert_eq!(
            bounded(events.recv()).await.unwrap(),
            SessionEvent {
                id: SessionId(8),
                event: TelnetEvent::Command(241 + (index % 2) as u8)
            }
        );
    }
    assert_eq!(probe.reads.load(Ordering::SeqCst), 2);
    bounded(session.disconnect()).await;
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert!(bounded(events.recv()).await.is_none());
}

#[tokio::test]
async fn disconnect_and_consumer_drop_interrupt_full_event_queue() {
    for drop_consumer in [false, true] {
        let (io, probe) = TestIo::new([255, 241].repeat(300), Writes::Short(16));
        let (mut session, events) = driver::start(SessionId(1), io, SessionConfig::default());
        scheduled_until(|| events.events.len() == EVENT_CAPACITY).await;
        if drop_consumer {
            drop(events);
            assert_eq!(
                reason(bounded(session.closed()).await),
                CloseReason::ConsumerDropped
            );
        } else {
            assert_eq!(
                reason(bounded(session.disconnect()).await),
                CloseReason::Disconnected
            );
            let mut events = events;
            let mut count = 0;
            while bounded(events.recv()).await.is_some() {
                count += 1;
            }
            assert_eq!(count, EVENT_CAPACITY);
        }
        assert!(probe.dropped.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn owner_drop_closes_workers_and_preserves_terminal_status() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(0));
    let (session, mut events) = driver::start(SessionId(3), io, SessionConfig::default());
    let mut status = session.status.clone();
    drop(session);
    bounded(async {
        while status.borrow().state == SessionState::Connected {
            status.changed().await.unwrap();
        }
    })
    .await;
    assert_eq!(reason(*status.borrow()), CloseReason::OwnerDropped);
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert!(bounded(events.recv()).await.is_none());
}

#[tokio::test]
async fn cancelling_closed_wait_does_not_detach_the_coordinator() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(0));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    tokio::select! { biased; _ = session.closed() => panic!("unexpected closure"), _ = tokio::task::yield_now() => {} }
    assert!(session.task.is_some());
    bounded(session.disconnect()).await;
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn unexpected_coordinator_exit_aborts_and_joins_its_writer() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(1));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    session.try_send_data(b"partial").unwrap();
    scheduled_until(|| probe.written.lock().unwrap().len() == 1).await;
    session.task.as_ref().unwrap().abort();
    assert_eq!(
        reason(bounded(session.closed()).await),
        CloseReason::WorkerFailed
    );
    assert!(probe.dropped.load(Ordering::SeqCst));
    assert!(session.task.is_none());
    assert!(session.writer_task.is_none());
}

#[tokio::test(start_paused = true)]
async fn partial_write_deadline_terminates_without_replay() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(2));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    session.try_send_data(b"abcdef").unwrap();
    assert_eq!(
        reason(
            tokio::time::timeout(Duration::from_secs(6), session.closed())
                .await
                .expect("shutdown deadline")
        ),
        CloseReason::WriteTimedOut
    );
    assert_eq!(*probe.written.lock().unwrap(), b"ab");
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancellation_interrupts_a_stalled_partial_write_and_saturated_output() {
    let (io, probe) = TestIo::new(vec![], Writes::StallAfter(1));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    session.try_send_data(b"first").unwrap();
    scheduled_until(|| probe.written.lock().unwrap().len() == 1).await;
    for _ in 0..200 {
        let _ = session.try_send_data(&vec![0; MAX_SEND_BYTES]);
        tokio::task::yield_now().await;
    }
    assert_eq!(session.commands.capacity(), 0);
    assert_eq!(
        reason(bounded(session.disconnect()).await),
        CloseReason::Disconnected
    );
    assert_eq!(*probe.written.lock().unwrap(), b"f");
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn writer_failure_is_reported_while_event_delivery_is_blocked() {
    let (client, mut server) = tokio::io::duplex(1024);
    let (mut session, events) = driver::start(SessionId(1), client, SessionConfig::default());
    session.try_send_data(&vec![b'x'; MAX_SEND_BYTES]).unwrap();
    // Queue acceptance alone does not prove coordinator processing. Observe the
    // first wire byte before filling the inbound queue and suspending commands.
    let mut first = [0];
    bounded(server.read_exact(&mut first)).await.unwrap();
    assert_eq!(first, [b'x']);
    bounded(server.write_all(&[255, 241].repeat(300)))
        .await
        .unwrap();
    scheduled_until(|| events.events.len() == EVENT_CAPACITY).await;
    assert_eq!(
        reason(
            tokio::time::timeout(Duration::from_secs(6), session.closed())
                .await
                .expect("shutdown deadline")
        ),
        CloseReason::WriteTimedOut
    );
    assert_eq!(events.events.len(), EVENT_CAPACITY);
}

#[tokio::test]
async fn io_failures_and_writer_panics_are_sanitized_and_joined() {
    for (writes, expected) in [
        (
            Writes::Fail(io::ErrorKind::BrokenPipe),
            CloseReason::Io {
                operation: IoOperation::Write,
                kind: io::ErrorKind::BrokenPipe,
            },
        ),
        (
            Writes::Zero,
            CloseReason::Io {
                operation: IoOperation::Write,
                kind: io::ErrorKind::WriteZero,
            },
        ),
        (Writes::Panic, CloseReason::WorkerFailed),
    ] {
        let (io, probe) = TestIo::new(vec![], writes);
        let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
        session.try_send_data(b"test").unwrap();
        assert_eq!(reason(bounded(session.closed()).await), expected);
        assert!(probe.dropped.load(Ordering::SeqCst));
    }
    let (mut io, probe) = TestIo::new(vec![], Writes::Short(1));
    io.read_error = Some(io::ErrorKind::ConnectionReset);
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    assert_eq!(
        reason(bounded(session.closed()).await),
        CloseReason::Io {
            operation: IoOperation::Read,
            kind: io::ErrorKind::ConnectionReset
        }
    );
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn saturated_output_recovers_in_order_without_replaying_or_losing_frames() {
    let (client, mut server) = tokio::io::duplex(1024);
    let (mut session, _events) = driver::start(SessionId(1), client, SessionConfig::default());
    let mut expected = Vec::new();
    for sequence in 0..100_u8 {
        let mut payload = vec![sequence; MAX_SEND_BYTES];
        payload[MAX_SEND_BYTES - 1] = 255;
        match session.try_send_data(&payload) {
            Ok(()) => {
                expected.extend(payload);
                expected.push(255);
            }
            Err(SendError::QueueFull) => {}
            other => panic!("unexpected send result: {other:?}"),
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(session.commands.capacity(), 0);
    let mut received = vec![0; expected.len()];
    bounded(server.read_exact(&mut received)).await.unwrap();
    assert_eq!(received, expected);
    session.try_send_data(b"recovered").unwrap();
    let mut marker = [0; 9];
    bounded(server.read_exact(&mut marker)).await.unwrap();
    assert_eq!(&marker, b"recovered");
    bounded(session.disconnect()).await;
    assert_eq!(bounded(server.read(&mut [0])).await.unwrap(), 0);
    assert!(session.task.is_none());
    assert!(session.writer_task.is_none());
}

#[tokio::test]
async fn partial_write_error_discards_remaining_frames_without_replay() {
    let (io, probe) = TestIo::new(vec![], Writes::FailAfter(2));
    let (mut session, _events) = driver::start(SessionId(1), io, SessionConfig::default());
    session.try_send_data(b"first").unwrap();
    session.try_send_data(b"second").unwrap();
    assert_eq!(
        reason(bounded(session.closed()).await),
        CloseReason::Io {
            operation: IoOperation::Write,
            kind: io::ErrorKind::BrokenPipe,
        }
    );
    assert_eq!(*probe.written.lock().unwrap(), b"fi");
    assert!(probe.dropped.load(Ordering::SeqCst));
}

#[tokio::test(start_paused = true)]
async fn write_deadline_covers_the_whole_frame_despite_partial_progress() {
    let (client, mut server) = tokio::io::duplex(1);
    let config = SessionConfig {
        write_timeout: Duration::from_secs(3),
        ..SessionConfig::default()
    };
    let (mut session, _events) = driver::start(SessionId(1), client, config);
    session.try_send_data(b"abcdef").unwrap();
    let start = tokio::time::Instant::now();
    tokio::task::yield_now().await;
    for expected in b"ab" {
        tokio::time::advance(Duration::from_secs(1)).await;
        let mut byte = [0];
        bounded(server.read_exact(&mut byte)).await.unwrap();
        assert_eq!(byte[0], *expected);
        tokio::task::yield_now().await;
    }
    assert_eq!(
        reason(bounded(session.closed()).await),
        CloseReason::WriteTimedOut
    );
    assert_eq!(start.elapsed(), Duration::from_secs(3));
    let mut tail = Vec::new();
    bounded(server.read_to_end(&mut tail)).await.unwrap();
    assert_eq!(tail, b"c");
}

#[tokio::test]
async fn terminal_status_is_published_once_and_does_not_wait_for_event_drain() {
    let (mut io, probe) = TestIo::new(b"prompt".to_vec(), Writes::Short(1));
    io.eof = true;
    let (mut session, mut events) = driver::start(SessionId(6), io, SessionConfig::default());
    let mut statuses = session.status.clone();
    assert_eq!(
        reason(bounded(session.closed()).await),
        CloseReason::PeerEof
    );
    assert!(probe.dropped.load(Ordering::SeqCst));
    bounded(statuses.changed()).await.unwrap();
    assert_eq!(reason(*statuses.borrow_and_update()), CloseReason::PeerEof);
    assert!(bounded(statuses.changed()).await.is_err());
    assert_eq!(
        bounded(events.recv()).await.unwrap().event,
        TelnetEvent::Data(b"prompt".to_vec())
    );
    assert!(bounded(events.recv()).await.is_none());
    assert_eq!(reason(session.disconnect().await), CloseReason::PeerEof);
}
