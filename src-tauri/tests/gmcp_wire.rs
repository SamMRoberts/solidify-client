//! Synthetic, bounded composition: production sessions still refuse GMCP.
use solidify_client::protocols::{
    gmcp::{self, GMCP, GmcpError, GmcpMessage},
    telnet::{
        NegotiationState, NegotiationVerb, OptionDirection, OptionPolicy, TelnetDecoder,
        TelnetEvent, TelnetNegotiator, encode,
    },
};
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
enum Received {
    Text(Vec<u8>),
    Message(GmcpMessage),
    Rejected(GmcpError),
}

struct Peer {
    decoder: TelnetDecoder,
    negotiator: TelnetNegotiator,
    direction: OptionDirection,
    received: Vec<Received>,
}
impl Peer {
    fn client(allow: bool) -> Self {
        Self::new(
            OptionPolicy::new(&[], if allow { &[GMCP] } else { &[] }),
            OptionDirection::Remote,
        )
    }
    fn server() -> Self {
        Self::new(OptionPolicy::new(&[GMCP], &[]), OptionDirection::Local)
    }
    fn new(policy: OptionPolicy, direction: OptionDirection) -> Self {
        Self {
            decoder: TelnetDecoder::new(),
            negotiator: TelnetNegotiator::new(policy),
            direction,
            received: vec![],
        }
    }
    fn enabled(&self) -> bool {
        self.negotiator.is_enabled(self.direction, GMCP)
    }
    fn send_gmcp(&self, message: &GmcpMessage) -> Option<Vec<u8>> {
        self.enabled()
            .then(|| wire(&gmcp::to_subnegotiation(message).unwrap()))
    }
    fn feed(&mut self, bytes: &[u8]) -> Vec<Vec<u8>> {
        let mut replies = Vec::new();
        let Self {
            decoder,
            negotiator,
            direction,
            received,
        } = self;
        decoder
            .feed(bytes, |event| {
                match event {
                    TelnetEvent::Negotiation { verb, option } => {
                        if let Some(reply) = negotiator.receive(verb, option) {
                            assert!(replies.len() < 32);
                            replies.push(wire(&reply.into()));
                        }
                    }
                    TelnetEvent::Subnegotiation {
                        option: GMCP,
                        payload,
                    } if negotiator.is_enabled(*direction, GMCP) => {
                        received.push(match gmcp::decode(&payload) {
                            Ok(message) => Received::Message(message),
                            Err(error) => Received::Rejected(error),
                        });
                    }
                    TelnetEvent::Data(bytes) => {
                        if let Some(Received::Text(previous)) = received.last_mut() {
                            assert!(previous.len() + bytes.len() <= 4096);
                            previous.extend(bytes);
                        } else {
                            received.push(Received::Text(bytes));
                        }
                    }
                    _ => {}
                }
                assert!(received.len() <= 32);
            })
            .unwrap();
        replies
    }
}

fn wire(event: &TelnetEvent) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode(event, |chunk| bytes.extend_from_slice(chunk)).unwrap();
    bytes
}
fn negotiation(verb: NegotiationVerb) -> TelnetEvent {
    TelnetEvent::Negotiation { verb, option: GMCP }
}
fn envelope(payload: &[u8]) -> TelnetEvent {
    TelnetEvent::Subnegotiation {
        option: GMCP,
        payload: payload.to_vec(),
    }
}

#[test]
fn every_split_preserves_text_messages_errors_and_enablement_order() {
    use NegotiationVerb::{Will, Wont};
    let mut stream = Vec::new();
    for event in [
        envelope(b"Ignored invalid-json"),
        TelnetEvent::Data("before 🌍".as_bytes().to_vec()),
        negotiation(Will),
        negotiation(Will),
        envelope("cHaR.Vitals {\"hp\":42,\"name\":\"café\"}".as_bytes()),
        TelnetEvent::Data(b"middle".to_vec()),
        envelope(b"Broken {"),
        envelope(b"Binary \"\xff\""),
        envelope(b"Valid null"),
        negotiation(Wont),
        envelope(b"Ignored {}"),
        negotiation(Will),
        envelope(b"MSDP {\"LIST\":\"COMMANDS\"}"),
        TelnetEvent::Data(b"prompt> ".to_vec()),
    ] {
        stream.extend(wire(&event));
    }
    assert!(stream.windows(2).any(|pair| pair == [255, 255]));
    let run = |chunks: Vec<&[u8]>| {
        let mut peer = Peer::client(true);
        let mut replies = Vec::new();
        for chunk in chunks {
            replies.extend(peer.feed(chunk));
            assert!(replies.len() <= 3);
        }
        peer.decoder.finish().unwrap();
        assert!(peer.enabled());
        (peer.received, replies)
    };
    let expected = run(vec![&stream]);
    assert_eq!(
        expected.1,
        [
            vec![255, 253, GMCP],
            vec![255, 254, GMCP],
            vec![255, 253, GMCP]
        ]
    );
    assert_eq!(
        expected.0,
        [
            Received::Text("before 🌍".as_bytes().to_vec()),
            Received::Message(
                gmcp::decode("cHaR.Vitals {\"hp\":42,\"name\":\"café\"}".as_bytes()).unwrap()
            ),
            Received::Text(b"middle".to_vec()),
            Received::Rejected(GmcpError::InvalidJson),
            Received::Rejected(GmcpError::InvalidUtf8),
            Received::Message(gmcp::decode(b"Valid null").unwrap()),
            Received::Message(gmcp::decode(b"MSDP {\"LIST\":\"COMMANDS\"}").unwrap()),
            Received::Text(b"prompt> ".to_vec()),
        ]
    );
    for split in 0..=stream.len() {
        assert_eq!(run(vec![&stream[..split], &stream[split..]]), expected);
    }
    assert_eq!(run(stream.chunks(1).collect()), expected);
}

struct Link {
    peers: [Peer; 2],
    queue: VecDeque<(usize, Vec<u8>)>,
}
impl Link {
    fn new(allow: bool) -> Self {
        Self {
            peers: [Peer::client(allow), Peer::server()],
            queue: VecDeque::new(),
        }
    }
    fn enqueue(&mut self, source: usize, bytes: Vec<u8>) {
        assert!(self.queue.len() < 32);
        assert!(bytes.len() <= 2 * gmcp::MAX_PAYLOAD_BYTES + 5);
        self.queue.push_back((1 - source, bytes));
    }
    fn server_request(&mut self, enabled: bool) {
        if let Some(command) = self.peers[1]
            .negotiator
            .request(OptionDirection::Local, GMCP, enabled)
            .unwrap()
        {
            self.enqueue(1, wire(&command.into()));
        }
    }
    fn settle(&mut self) -> usize {
        let mut exchanges = 0;
        while let Some((target, bytes)) = self.queue.pop_front() {
            exchanges += 1;
            assert!(exchanges <= 64, "negotiation must quiesce");
            for chunk in bytes.chunks(1) {
                for reply in self.peers[target].feed(chunk) {
                    self.enqueue(target, reply);
                }
            }
        }
        exchanges
    }
    fn assert_state(&self, enabled: bool) {
        for peer in &self.peers {
            assert_eq!(
                peer.negotiator.state(peer.direction, GMCP),
                if enabled {
                    NegotiationState::Yes
                } else {
                    NegotiationState::No
                }
            );
        }
    }
    fn finish(&mut self) {
        assert_eq!(self.settle(), 0);
        for peer in &mut self.peers {
            peer.decoder.finish().unwrap();
        }
    }
}

#[test]
fn passive_handshake_authorizes_both_directions_and_copyover_reenables() {
    let mut link = Link::new(true);
    let message = gmcp::decode(b"Vendor.Send [1,true]").unwrap();
    assert_eq!(link.settle(), 0, "client emits no startup negotiation");
    assert!(link.peers[0].send_gmcp(&message).is_none());
    for _ in 0..2 {
        link.server_request(true);
        assert_eq!(link.settle(), 2);
        link.assert_state(true);
        // The client never enables its local GMCP direction.
        assert!(
            !link.peers[0]
                .negotiator
                .is_enabled(OptionDirection::Local, GMCP)
        );
        link.server_request(true);
        assert_eq!(link.settle(), 0);
        for source in 0..2 {
            let bytes = link.peers[source].send_gmcp(&message).unwrap();
            let mut expected = vec![255, 250, GMCP];
            expected.extend_from_slice(b"Vendor.Send [1,true]");
            expected.extend_from_slice(&[255, 240]);
            assert_eq!(bytes, expected);
            link.enqueue(source, bytes);
        }
        assert_eq!(link.settle(), 2);
        link.server_request(false);
        assert_eq!(link.settle(), 2);
        link.assert_state(false);
        assert!(link.peers[0].send_gmcp(&message).is_none());
    }
    for peer in &link.peers {
        assert_eq!(
            peer.received,
            [
                Received::Message(message.clone()),
                Received::Message(message.clone())
            ]
        );
    }
    link.finish();
    let fresh = Peer::client(true);
    assert!(!fresh.enabled());
    assert!(fresh.received.is_empty());
}

#[test]
fn refusal_is_quiescent_and_peers_are_independent() {
    let mut denied = Link::new(false);
    let mut allowed = Link::new(true);
    denied.server_request(true);
    assert_eq!(denied.settle(), 2);
    denied.assert_state(false);
    allowed.server_request(true);
    assert_eq!(allowed.settle(), 2);
    allowed.assert_state(true);
    denied.assert_state(false);
    denied.enqueue(1, wire(&envelope(b"Ignored {}")));
    assert_eq!(denied.settle(), 1);
    assert!(denied.peers[0].received.is_empty());
    // A server asking the client to perform GMCP is the unsupported direction.
    allowed.enqueue(1, wire(&negotiation(NegotiationVerb::Do)));
    assert_eq!(allowed.settle(), 2);
    allowed.assert_state(true);
    denied.finish();
    allowed.finish();
}
