//! Local servers run as test-owned futures/sockets, never detached tasks. Every
//! wait is bounded; unwinding drops listeners and sockets and cancels sessions.
use solidify_client::{
    protocols::telnet::{DecodeError, IncompleteFrame, NegotiationVerb, TelnetEvent},
    sessions::{
        CloseReason, ConnectError, MAX_SEND_BYTES, SendError, Session, SessionConfig,
        SessionEvents, SessionId, SessionState, connect,
    },
};
use std::{future::Future, io, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("loopback wait expired")
}

async fn accept_session(listener: &TcpListener, id: u64) -> (Session, SessionEvents, TcpStream) {
    let (client, server) = bounded(async {
        tokio::join!(
            connect(
                SessionId(id),
                listener.local_addr().unwrap(),
                SessionConfig::default()
            ),
            listener.accept()
        )
    })
    .await;
    let (session, events) = client.unwrap();
    (session, events, server.unwrap().0)
}

async fn pair(id: u64) -> (Session, SessionEvents, TcpStream) {
    let listener = bounded(TcpListener::bind("127.0.0.1:0")).await.unwrap();
    accept_session(&listener, id).await
}

async fn bytes(server: &mut TcpStream, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    bounded(server.read_exact(&mut actual)).await.unwrap();
    assert_eq!(actual, expected);
}

async fn event(events: &mut SessionEvents, id: u64, expected: TelnetEvent) {
    let actual = bounded(events.recv()).await.unwrap();
    assert_eq!(actual.id, SessionId(id));
    assert_eq!(actual.event, expected);
}

async fn eof(server: &mut TcpStream) {
    assert_eq!(bounded(server.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn connects_preserves_partial_prompts_and_escapes_outbound_data() {
    let (mut session, mut events, mut server) = pair(10).await;
    assert_eq!(session.status().state, SessionState::Connected);
    assert_eq!(session.status().id, SessionId(10));
    bounded(server.write_all(b"Name: ")).await.unwrap();
    let mut prompt = Vec::new();
    while prompt.len() < 6 {
        let received = bounded(events.recv()).await.unwrap();
        assert_eq!(received.id, SessionId(10));
        let TelnetEvent::Data(data) = received.event else {
            panic!("expected prompt")
        };
        prompt.extend(data);
    }
    assert_eq!(prompt, b"Name: ");
    assert_eq!(
        session.try_send_data(&vec![0; MAX_SEND_BYTES + 1]),
        Err(SendError::TooLarge)
    );
    session
        .try_send_data(b"\0\r\n\x1b[31m\xc3\xa9\xff")
        .unwrap();
    bytes(&mut server, b"\0\r\n\x1b[31m\xc3\xa9\xff\xff").await;
    session.try_send_data(&vec![255; MAX_SEND_BYTES]).unwrap();
    bytes(&mut server, &vec![255; 2 * MAX_SEND_BYTES]).await;
    assert_eq!(
        bounded(session.disconnect()).await.state,
        SessionState::Closed(CloseReason::Disconnected)
    );
    eof(&mut server).await;
    assert!(bounded(events.recv()).await.is_none());
}

#[tokio::test]
async fn fragmented_frames_forward_untrusted_events_and_order_refusals_with_data() {
    let (mut session, mut events, mut server) = pair(11).await;
    session.try_send_data(b"before").unwrap();
    bytes(&mut server, b"before").await;
    for byte in [
        255, 251, 42, 255, 253, 43, 255, 250, 99, b'x', 255, 255, 255, 240,
    ] {
        bounded(server.write_all(&[byte])).await.unwrap();
        tokio::task::yield_now().await;
    }
    event(
        &mut events,
        11,
        TelnetEvent::Negotiation {
            verb: NegotiationVerb::Will,
            option: 42,
        },
    )
    .await;
    event(
        &mut events,
        11,
        TelnetEvent::Negotiation {
            verb: NegotiationVerb::Do,
            option: 43,
        },
    )
    .await;
    event(
        &mut events,
        11,
        TelnetEvent::Subnegotiation {
            option: 99,
            payload: vec![b'x', 255],
        },
    )
    .await;
    // Receipt of the events establishes that both refusals entered the shared FIFO.
    session.try_send_data(b"after").unwrap();
    bytes(
        &mut server,
        &[255, 254, 42, 255, 252, 43, b'a', b'f', b't', b'e', b'r'],
    )
    .await;
    bounded(server.write_all(&[255, 252, 42, 255, 254, 43]))
        .await
        .unwrap();
    event(
        &mut events,
        11,
        TelnetEvent::Negotiation {
            verb: NegotiationVerb::Wont,
            option: 42,
        },
    )
    .await;
    event(
        &mut events,
        11,
        TelnetEvent::Negotiation {
            verb: NegotiationVerb::Dont,
            option: 43,
        },
    )
    .await;
    session.try_send_data(b"marker").unwrap();
    bytes(&mut server, b"marker").await; // Negative acknowledgments generated no replies.
    bounded(session.disconnect()).await;
    eof(&mut server).await;
}

#[tokio::test]
async fn clean_eof_and_all_truncated_states_close_both_halves() {
    for (input, incomplete) in [
        (vec![], None),
        (vec![255], Some(IncompleteFrame::Command)),
        (vec![255, 251], Some(IncompleteFrame::NegotiationOption)),
        (vec![255, 250], Some(IncompleteFrame::SubnegotiationOption)),
        (
            vec![255, 250, 42],
            Some(IncompleteFrame::SubnegotiationPayload),
        ),
        (
            vec![255, 250, 42, 255],
            Some(IncompleteFrame::SubnegotiationEscape),
        ),
    ] {
        let (mut session, mut events, mut server) = pair(12).await;
        bounded(server.write_all(&input)).await.unwrap();
        bounded(server.shutdown()).await.unwrap();
        let expected = incomplete.map_or(CloseReason::PeerEof, |frame| {
            CloseReason::Protocol(DecodeError::Truncated { frame })
        });
        assert_eq!(
            bounded(session.closed()).await.state,
            SessionState::Closed(expected)
        );
        assert!(bounded(events.recv()).await.is_none());
        eof(&mut server).await;
    }
}

#[tokio::test]
async fn malformed_frames_and_payload_overflow_are_terminal() {
    let mut oversized = vec![255, 250, 42];
    oversized.extend(vec![b'x'; 65537]);
    for (input, expected) in [
        (vec![255, 240], DecodeError::UnexpectedSubnegotiationEnd),
        (
            vec![255, 250, 42, 255, 241],
            DecodeError::UnexpectedSubnegotiationCommand { command: 241 },
        ),
        (oversized, DecodeError::SubnegotiationTooLarge),
    ] {
        let (mut session, mut events, mut server) = pair(13).await;
        bounded(server.write_all(&input)).await.unwrap();
        assert_eq!(
            bounded(session.closed()).await.state,
            SessionState::Closed(CloseReason::Protocol(expected))
        );
        assert_eq!(session.try_send_data(b"late"), Err(SendError::Closed));
        assert!(bounded(events.recv()).await.is_none());
        eof(&mut server).await;
    }
}

#[tokio::test]
async fn simultaneous_and_replacement_sessions_have_fresh_state_and_identity() {
    let listener = bounded(TcpListener::bind("127.0.0.1:0")).await.unwrap();
    let (mut old, mut old_events, mut old_server) = accept_session(&listener, 20).await;
    let (mut other, mut other_events, mut other_server) = accept_session(&listener, 21).await;
    bounded(old_server.write_all(&[255, 241, 255]))
        .await
        .unwrap();
    bounded(old_server.shutdown()).await.unwrap();
    assert_eq!(
        bounded(old.closed()).await.state,
        SessionState::Closed(CloseReason::Protocol(DecodeError::Truncated {
            frame: IncompleteFrame::Command
        }))
    );
    let (mut replacement, mut new_events, mut new_server) = accept_session(&listener, 22).await;
    bounded(new_server.write_all(&[255, 241])).await.unwrap();
    bounded(other_server.write_all(&[255, 242])).await.unwrap();
    event(&mut new_events, 22, TelnetEvent::Command(241)).await;
    event(&mut old_events, 20, TelnetEvent::Command(241)).await;
    event(&mut other_events, 21, TelnetEvent::Command(242)).await;
    assert!(bounded(old_events.recv()).await.is_none());
    bounded(replacement.disconnect()).await;
    assert_eq!(other.status().state, SessionState::Connected);
    other.try_send_data(b"independent").unwrap();
    bytes(&mut other_server, b"independent").await;
    bounded(other.disconnect()).await;
    eof(&mut old_server).await;
    eof(&mut new_server).await;
    eof(&mut other_server).await;
}

#[tokio::test]
async fn dropping_either_owner_closes_the_socket() {
    let (session, mut events, mut server) = pair(30).await;
    drop(session);
    assert!(bounded(events.recv()).await.is_none());
    eof(&mut server).await;
    let (mut session, events, mut server) = pair(31).await;
    drop(events);
    assert_eq!(
        bounded(session.closed()).await.state,
        SessionState::Closed(CloseReason::ConsumerDropped)
    );
    eof(&mut server).await;
}

#[tokio::test]
async fn refused_connection_returns_sanitized_error() {
    let listener = bounded(TcpListener::bind("127.0.0.1:0")).await.unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    let result = bounded(connect(SessionId(40), address, SessionConfig::default())).await;
    assert!(matches!(
        result,
        Err(ConnectError::Io {
            kind: io::ErrorKind::ConnectionRefused
        })
    ));
}
