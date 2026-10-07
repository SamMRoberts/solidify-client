//! Public API acceptance uses test-owned loopback sockets, with bounded waits.
use solidify_client::{
    protocols::{
        gmcp::{self, GmcpError, GmcpMessage},
        telnet::{self, TelnetEvent},
    },
    sessions::{
        self, CloseReason, GmcpSendError, Session, SessionConfig, SessionEvents, SessionId,
        SessionOptions, SessionState, TerminalSize,
    },
};
use std::{future::Future, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

async fn bounded<T>(future: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(3), future)
        .await
        .expect("GMCP loopback deadline")
}
async fn pair(id: u64, profile: SessionOptions) -> (Session, SessionEvents, TcpStream) {
    let listener = bounded(TcpListener::bind("127.0.0.1:0")).await.unwrap();
    let (client, server) = bounded(async {
        tokio::join!(
            sessions::connect_with_options(
                SessionId(id),
                listener.local_addr().unwrap(),
                SessionConfig::default(),
                profile
            ),
            listener.accept()
        )
    })
    .await;
    let (session, events) = client.unwrap();
    (session, events, server.unwrap().0)
}
fn profile() -> SessionOptions {
    SessionOptions::MudClientGmcp {
        size: TerminalSize::default(),
    }
}
fn wire(event: TelnetEvent) -> Vec<u8> {
    let mut bytes = vec![];
    telnet::encode(&event, |chunk| bytes.extend_from_slice(chunk)).unwrap();
    bytes
}
fn message(payload: &[u8]) -> Vec<u8> {
    wire(TelnetEvent::Subnegotiation {
        option: gmcp::GMCP,
        payload: payload.to_vec(),
    })
}
async fn bytes(server: &mut TcpStream, expected: &[u8]) {
    let mut actual = vec![0; expected.len()];
    bounded(server.read_exact(&mut actual)).await.unwrap();
    assert_eq!(actual, expected);
}
async fn negotiate(server: &mut TcpStream, events: &mut SessionEvents, enabled: bool) {
    bounded(server.write_all(&[255, if enabled { 251 } else { 252 }, 201]))
        .await
        .unwrap();
    bytes(server, &[255, if enabled { 253 } else { 254 }, 201]).await;
    let event = bounded(events.recv()).await.unwrap();
    assert!(matches!(
        event.event,
        TelnetEvent::Negotiation { option: 201, .. }
    ));
    assert!(event.gmcp.is_none());
}
async fn disconnect(session: &mut Session, server: &mut TcpStream) {
    assert_eq!(
        bounded(session.disconnect()).await.state,
        SessionState::Closed(CloseReason::Disconnected)
    );
    assert_eq!(bounded(server.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn gmcp_fragmented_stream_preserves_raw_parsed_and_text_order() {
    let (mut session, mut events, mut server) = pair(301, profile()).await;
    let outbound = gmcp::decode(b"Vendor.Test [1,true]").unwrap();
    assert_eq!(
        bounded(session.send_gmcp(&outbound)).await,
        Err(GmcpSendError::Disabled)
    );
    let disabled = message(b"Ignored invalid-json");
    bounded(server.write_all(&disabled)).await.unwrap();
    assert!(bounded(events.recv()).await.unwrap().gmcp.is_none());
    negotiate(&mut server, &mut events, true).await;
    assert!(session.subscribe_options().borrow().gmcp);
    assert_eq!(session.subscribe_options().borrow().gmcp_generation, 1);
    let frames = [
        TelnetEvent::Data("before 🌍".as_bytes().to_vec()),
        TelnetEvent::Subnegotiation {
            option: 201,
            payload: "Char.Vitals {\"name\":\"café\",\"hp\":42}"
                .as_bytes()
                .to_vec(),
        },
        TelnetEvent::Subnegotiation {
            option: 201,
            payload: b"Bad {".to_vec(),
        },
        TelnetEvent::Subnegotiation {
            option: 201,
            payload: b"Bad \"\xff\"".to_vec(),
        },
        TelnetEvent::Subnegotiation {
            option: 201,
            payload: b"Vendor.Done null".to_vec(),
        },
        TelnetEvent::Data(b"prompt> ".to_vec()),
    ];
    for expected in frames {
        for byte in wire(expected.clone()) {
            bounded(server.write_all(&[byte])).await.unwrap();
            tokio::task::yield_now().await;
        }
        let first = bounded(events.recv()).await.unwrap();
        assert_eq!(first.id, SessionId(301));
        match &expected {
            TelnetEvent::Data(wanted) => {
                assert!(first.gmcp.is_none());
                let TelnetEvent::Data(mut actual) = first.event else {
                    panic!("data expected")
                };
                while actual.len() < wanted.len() {
                    let next = bounded(events.recv()).await.unwrap();
                    assert!(next.gmcp.is_none());
                    let TelnetEvent::Data(bytes) = next.event else {
                        panic!("data expected")
                    };
                    actual.extend(bytes);
                }
                assert_eq!(&actual, wanted);
            }
            TelnetEvent::Subnegotiation { payload, .. } => {
                assert_eq!(first.event, expected);
                assert_eq!(first.gmcp, Some(gmcp::decode(payload)));
            }
            _ => unreachable!(),
        }
    }
    assert_eq!(bounded(session.send_gmcp(&outbound)).await, Ok(()));
    let expected_wire = message(b"Vendor.Test [1,true]");
    bytes(&mut server, &expected_wire).await;
    negotiate(&mut server, &mut events, false).await;
    assert_eq!(
        bounded(session.send_gmcp(&outbound)).await,
        Err(GmcpSendError::Disabled)
    );
    bounded(server.write_all(&message(b"Ignored {}")))
        .await
        .unwrap();
    assert!(bounded(events.recv()).await.unwrap().gmcp.is_none());
    negotiate(&mut server, &mut events, true).await;
    assert_eq!(session.subscribe_options().borrow().gmcp_generation, 2);
    assert_eq!(bounded(session.send_gmcp(&outbound)).await, Ok(()));
    bytes(&mut server, &expected_wire).await;
    disconnect(&mut session, &mut server).await;
    assert_eq!(
        bounded(session.send_gmcp(&outbound)).await,
        Err(GmcpSendError::Closed)
    );
}

#[tokio::test]
async fn gmcp_maximum_payload_and_invalid_sends_use_bounded_existing_writer() {
    let (mut session, mut events, mut server) = pair(302, profile()).await;
    negotiate(&mut server, &mut events, true).await;
    let payload = format!("P \"{}\"", "x".repeat(gmcp::MAX_PAYLOAD_BYTES - 4));
    let expected = gmcp::decode(payload.as_bytes()).unwrap();
    let encoded = message(payload.as_bytes());
    assert_eq!(encoded.len(), gmcp::MAX_PAYLOAD_BYTES + 5);
    let (sent, ()) =
        bounded(async { tokio::join!(session.send_gmcp(&expected), bytes(&mut server, &encoded)) })
            .await;
    assert_eq!(sent, Ok(()));
    bounded(server.write_all(&encoded)).await.unwrap();
    let received = bounded(events.recv()).await.unwrap();
    assert_eq!(received.gmcp, Some(Ok(expected)));
    assert!(
        matches!(received.event, TelnetEvent::Subnegotiation { payload: p, .. } if p == payload.as_bytes())
    );
    for (bad, error) in [
        (
            GmcpMessage {
                package: "invalid name".into(),
                data: None,
            },
            GmcpError::InvalidPackage,
        ),
        (
            GmcpMessage {
                package: "P".into(),
                data: Some(serde_json::json!("x".repeat(gmcp::MAX_PAYLOAD_BYTES))),
            },
            GmcpError::PayloadTooLarge,
        ),
    ] {
        assert_eq!(
            bounded(session.send_gmcp(&bad)).await,
            Err(GmcpSendError::Invalid(error))
        );
    }
    session.try_send_data(b"after").unwrap();
    bytes(&mut server, b"after").await;
    bounded(server.shutdown()).await.unwrap();
    assert_eq!(
        bounded(session.closed()).await.state,
        SessionState::Closed(CloseReason::PeerEof)
    );
    assert!(bounded(events.recv()).await.is_none());
    assert_eq!(bounded(server.read(&mut [0])).await.unwrap(), 0);
}

#[tokio::test]
async fn gmcp_profiles_and_session_generations_are_isolated() {
    let (mut active, mut active_events, mut active_server) = pair(303, profile()).await;
    negotiate(&mut active_server, &mut active_events, true).await;
    for (id, options) in [
        (304, SessionOptions::DenyAll),
        (
            305,
            SessionOptions::MudClient {
                size: TerminalSize::default(),
            },
        ),
    ] {
        let (mut session, mut events, mut server) = pair(id, options).await;
        bounded(server.write_all(&[255, 251, 201])).await.unwrap();
        bytes(&mut server, &[255, 254, 201]).await;
        bounded(events.recv()).await.unwrap();
        assert!(!session.subscribe_options().borrow().gmcp);
        assert_eq!(session.subscribe_options().borrow().gmcp_generation, 0);
        assert!(active.subscribe_options().borrow().gmcp);
        disconnect(&mut session, &mut server).await;
    }
    disconnect(&mut active, &mut active_server).await;
    let (mut fresh, _events, mut server) = pair(306, profile()).await;
    assert!(!fresh.subscribe_options().borrow().gmcp);
    assert_eq!(fresh.subscribe_options().borrow().gmcp_generation, 0);
    disconnect(&mut fresh, &mut server).await;
}
