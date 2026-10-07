//! Local servers run as test-owned futures/sockets, never detached tasks. Every
//! wait is bounded; unwinding drops listeners and sockets and cancels sessions.
use solidify_client::{
    protocols::presentation::{
        IncompleteSequence, MAX_CONTROL_STRING_BYTES, MAX_TEXT_BYTES, PresentationDecoder,
        PresentationError, PresentationEvent, TextColor, TextControl, TextStyle,
    },
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

fn collect_presentation(events: &mut Vec<PresentationEvent>, event: PresentationEvent) {
    if let PresentationEvent::Text(text) = &event {
        assert!(!text.is_empty() && text.len() <= MAX_TEXT_BYTES);
        if let Some(PresentationEvent::Text(previous)) = events.last_mut() {
            previous.push_str(text);
            return;
        }
    }
    events.push(event);
}

#[tokio::test]
async fn presentation_streams_prompts_unicode_and_styles_across_telnet_controls() {
    let (mut session, mut events, mut server) = pair(40).await;
    let mut decoder = PresentationDecoder::new();
    let mut output = Vec::new();
    let mut controls = Vec::new();
    for (index, fragment) in [
        b"Name: \xf0\x9f".as_slice(),
        b"\xff\xfb\x2a\x8c\x8d \x1b[3",
        b"\xff\xfa\x63private\xff\xf0\xff\xfd\x2b1m> ",
        b"\r\n\x1b[0m",
    ]
    .into_iter()
    .enumerate()
    {
        bounded(server.write_all(fragment)).await.unwrap();
        // A Telnet NOP provides an observed boundary without assuming TCP packetization.
        bounded(server.write_all(&[255, 241])).await.unwrap();
        bounded(async {
            loop {
                let received = events.recv().await.unwrap();
                assert_eq!(received.id, SessionId(40));
                match received.event {
                    TelnetEvent::Data(data) => decoder
                        .feed(&data, |e| collect_presentation(&mut output, e))
                        .unwrap(),
                    TelnetEvent::Command(241) => break,
                    other => controls.push(other),
                }
            }
        })
        .await;
        if index == 0 {
            assert_eq!(output, [PresentationEvent::Text("Name: ".into())]);
            assert_eq!(session.status().state, SessionState::Connected);
        }
    }
    assert_eq!(
        output,
        [
            PresentationEvent::Text("Name: 🌍 ".into()),
            PresentationEvent::StyleChanged(TextStyle {
                foreground: TextColor::Red,
                ..TextStyle::default()
            }),
            PresentationEvent::Text("> ".into()),
            PresentationEvent::Control(TextControl::CarriageReturn),
            PresentationEvent::Control(TextControl::LineFeed),
            PresentationEvent::StyleChanged(TextStyle::default()),
        ]
    );
    assert_eq!(
        controls,
        [
            TelnetEvent::Negotiation {
                verb: NegotiationVerb::Will,
                option: 42
            },
            TelnetEvent::Subnegotiation {
                option: 99,
                payload: b"private".to_vec()
            },
            TelnetEvent::Negotiation {
                verb: NegotiationVerb::Do,
                option: 43
            },
        ]
    );
    bytes(&mut server, &[255, 254, 42, 255, 252, 43]).await;
    bounded(server.shutdown()).await.unwrap();
    assert!(bounded(events.recv()).await.is_none());
    decoder
        .finish(|_| panic!("unexpected finish output"))
        .unwrap();
    assert_eq!(
        bounded(session.closed()).await.state,
        SessionState::Closed(CloseReason::PeerEof)
    );
    eof(&mut server).await;
}

#[tokio::test]
async fn presentation_finish_handles_unicode_and_escape_eof_separately_from_session_eof() {
    for (input, expected_text, expected_error) in [
        (b"prompt \xe2\x82".as_slice(), "prompt �", None),
        (b"prompt \x1b", "prompt ", Some(IncompleteSequence::Escape)),
        (b"prompt \x1b[31", "prompt ", Some(IncompleteSequence::Csi)),
        (
            b"prompt \x1b]private",
            "prompt ",
            Some(IncompleteSequence::ControlString),
        ),
        (
            b"prompt \x1bPprivate\x1b",
            "prompt ",
            Some(IncompleteSequence::ControlStringTerminator),
        ),
    ] {
        let (mut session, mut events, mut server) = pair(41).await;
        let mut decoder = PresentationDecoder::new();
        let mut output = Vec::new();
        bounded(server.write_all(input)).await.unwrap();
        bounded(server.shutdown()).await.unwrap();
        bounded(async {
            while let Some(received) = events.recv().await {
                let TelnetEvent::Data(data) = received.event else {
                    panic!("unexpected Telnet control");
                };
                decoder
                    .feed(&data, |e| collect_presentation(&mut output, e))
                    .unwrap();
            }
        })
        .await;
        assert_eq!(
            decoder.finish(|e| collect_presentation(&mut output, e)),
            expected_error.map_or(Ok(()), |sequence| Err(PresentationError::Truncated {
                sequence
            }))
        );
        assert_eq!(output, [PresentationEvent::Text(expected_text.into())]);
        assert_eq!(
            bounded(session.closed()).await.state,
            SessionState::Closed(CloseReason::PeerEof)
        );
        eof(&mut server).await;
    }
}

#[tokio::test]
async fn presentation_failure_does_not_implicitly_close_transport() {
    let (mut session, mut events, mut server) = pair(42).await;
    let mut decoder = PresentationDecoder::new();
    let mut input = b"before\x1b]".to_vec();
    input.extend(vec![b'x'; MAX_CONTROL_STRING_BYTES]);
    input.extend(b"\x07after");
    bounded(server.write_all(&input)).await.unwrap();
    let mut output = Vec::new();
    let failure = bounded(async {
        loop {
            let received = events.recv().await.unwrap();
            let TelnetEvent::Data(data) = received.event else {
                panic!("unexpected Telnet control");
            };
            if let Err(error) = decoder.feed(&data, |e| collect_presentation(&mut output, e)) {
                break error;
            }
        }
    })
    .await;
    assert_eq!(failure, PresentationError::ControlStringTooLong);
    assert_eq!(output, [PresentationEvent::Text("before".into())]);
    assert_eq!(session.status().state, SessionState::Connected);
    session.try_send_data(b"still connected").unwrap();
    bytes(&mut server, b"still connected").await;
    assert_eq!(
        decoder.feed(b"late", |_| panic!("failed decoder emitted")),
        Err(PresentationError::DecoderFailed)
    );
    assert_eq!(
        bounded(session.disconnect()).await.state,
        SessionState::Closed(CloseReason::Disconnected)
    );
    eof(&mut server).await;
    bounded(async { while events.recv().await.is_some() {} }).await;
}

#[tokio::test]
async fn presentation_instances_remain_isolated_across_concurrent_sessions() {
    let (mut first, mut first_events, mut first_server) = pair(43).await;
    let (mut second, mut second_events, mut second_server) = pair(44).await;
    let mut first_decoder = PresentationDecoder::new();
    let mut second_decoder = PresentationDecoder::new();
    let mut first_output = Vec::new();
    let mut second_output = Vec::new();
    bounded(first_server.write_all(b"\x1b[1m\xc3\xff\xf1"))
        .await
        .unwrap();
    bounded(async {
        loop {
            let received = first_events.recv().await.unwrap();
            assert_eq!(received.id, SessionId(43));
            match received.event {
                TelnetEvent::Data(data) => first_decoder
                    .feed(&data, |e| collect_presentation(&mut first_output, e))
                    .unwrap(),
                TelnetEvent::Command(241) => break,
                _ => panic!("unexpected Telnet event"),
            }
        }
    })
    .await;
    bounded(second_server.write_all(b"\x1b[mplain"))
        .await
        .unwrap();
    bounded(second_server.shutdown()).await.unwrap();
    bounded(async {
        while let Some(received) = second_events.recv().await {
            assert_eq!(received.id, SessionId(44));
            let TelnetEvent::Data(data) = received.event else {
                panic!("unexpected Telnet control");
            };
            second_decoder
                .feed(&data, |e| collect_presentation(&mut second_output, e))
                .unwrap();
        }
    })
    .await;
    second_decoder
        .finish(|_| panic!("unexpected finish output"))
        .unwrap();
    bounded(first_server.write_all(b"\xa9")).await.unwrap();
    bounded(first_server.shutdown()).await.unwrap();
    bounded(async {
        while let Some(received) = first_events.recv().await {
            let TelnetEvent::Data(data) = received.event else {
                panic!("unexpected Telnet control");
            };
            first_decoder
                .feed(&data, |e| collect_presentation(&mut first_output, e))
                .unwrap();
        }
    })
    .await;
    first_decoder
        .finish(|_| panic!("unexpected finish output"))
        .unwrap();
    assert_eq!(
        first_output,
        [
            PresentationEvent::StyleChanged(TextStyle {
                bold: true,
                ..TextStyle::default()
            }),
            PresentationEvent::Text("é".into())
        ]
    );
    assert_eq!(second_output, [PresentationEvent::Text("plain".into())]);
    for (session, server) in [
        (&mut first, &mut first_server),
        (&mut second, &mut second_server),
    ] {
        assert_eq!(
            bounded(session.closed()).await.state,
            SessionState::Closed(CloseReason::PeerEof)
        );
        eof(server).await;
    }
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

#[tokio::test]
async fn opt_in_options_preserve_default_sessions_and_fragmented_presentation() {
    use solidify_client::sessions::{SessionOptions, TerminalSize, connect_with_options};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let (mut enabled, mut events) = bounded(connect_with_options(
        SessionId(100),
        listener.local_addr().unwrap(),
        SessionConfig::default(),
        SessionOptions::MudClient {
            size: TerminalSize::default(),
        },
    ))
    .await
    .unwrap();
    let (mut peer, _) = bounded(listener.accept()).await.unwrap();
    let (mut original, _original_events, mut original_peer) = accept_session(&listener, 101).await;
    let mut decoder = PresentationDecoder::new();
    let mut output = Vec::new();
    for fragment in [
        b"\xf0\x9f\xff".as_slice(),
        b"\xfd\x18\xff\xfa\x18",
        b"\x01\xff\xf0\x8c\x8d> ",
    ] {
        bounded(peer.write_all(fragment)).await.unwrap();
        // No incomplete Telnet framing is allowed at a NOP boundary; this split
        // remains deliberately unobserved until the complete exchange below.
    }
    bounded(peer.write_all(&[255, 241])).await.unwrap();
    bounded(async {
        loop {
            match events.recv().await.unwrap().event {
                TelnetEvent::Data(data) => decoder
                    .feed(&data, |event| collect_presentation(&mut output, event))
                    .unwrap(),
                TelnetEvent::Command(241) => break,
                _ => {}
            }
        }
    })
    .await;
    assert_eq!(output, [PresentationEvent::Text("🌍> ".into())]);
    bytes(&mut peer, b"\xff\xfb\x18\xff\xfa\x18\0SOLIDIFY\xff\xf0").await;
    bounded(original_peer.write_all(b"\xff\xfd\x18\xff\xfb\x01"))
        .await
        .unwrap();
    bytes(&mut original_peer, b"\xff\xfc\x18\xff\xfe\x01").await;
    assert!(enabled.subscribe_options().borrow().terminal_type);
    assert!(!original.subscribe_options().borrow().terminal_type);
    assert!(!original.subscribe_options().borrow().remote_echo);
    bounded(enabled.disconnect()).await;
    bounded(original.disconnect()).await;
    eof(&mut peer).await;
    eof(&mut original_peer).await;
}

#[tokio::test]
async fn extended_colors_survive_fragmentation_and_interleaved_negotiation() {
    let (mut session, mut events, mut server) = pair(80).await;
    let mut decoder = PresentationDecoder::new();
    let mut output = Vec::new();
    for fragment in [
        b"\x1b[38;2;25".as_slice(),
        b"\xff\xfb\x2a5;128;0mRGB \x1b[48:5:",
        b"255mindexed\x1b[0m> ",
    ] {
        bounded(server.write_all(fragment)).await.unwrap();
        bounded(server.write_all(&[255, 241])).await.unwrap();
        bounded(async {
            loop {
                match events.recv().await.unwrap().event {
                    TelnetEvent::Data(data) => decoder
                        .feed(&data, |e| collect_presentation(&mut output, e))
                        .unwrap(),
                    TelnetEvent::Command(241) => break,
                    TelnetEvent::Negotiation { .. } => {}
                    _ => panic!("unexpected event"),
                }
            }
        })
        .await;
    }
    assert_eq!(
        output,
        [
            PresentationEvent::StyleChanged(TextStyle {
                foreground: TextColor::Rgb {
                    red: 255,
                    green: 128,
                    blue: 0
                },
                ..TextStyle::default()
            }),
            PresentationEvent::Text("RGB ".into()),
            PresentationEvent::StyleChanged(TextStyle {
                foreground: TextColor::Rgb {
                    red: 255,
                    green: 128,
                    blue: 0
                },
                background: TextColor::Indexed(255),
                ..TextStyle::default()
            }),
            PresentationEvent::Text("indexed".into()),
            PresentationEvent::StyleChanged(TextStyle::default()),
            PresentationEvent::Text("> ".into()),
        ]
    );
    bytes(&mut server, &[255, 254, 42]).await;
    bounded(server.shutdown()).await.unwrap();
    assert!(bounded(events.recv()).await.is_none());
    decoder
        .finish(|_| panic!("unexpected finish output"))
        .unwrap();
    assert_eq!(
        bounded(session.closed()).await.state,
        SessionState::Closed(CloseReason::PeerEof)
    );
    eof(&mut server).await;
}
