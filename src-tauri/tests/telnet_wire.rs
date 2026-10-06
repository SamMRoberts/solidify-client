//! Synthetic peers exercise only the public API, without sockets or live services.

use solidify_client::protocols::telnet::{
    NegotiationState, OptionDirection, OptionPolicy, TelnetDecoder, TelnetEvent, TelnetNegotiator,
    encode,
};
use std::collections::VecDeque;

struct Peer {
    decoder: TelnetDecoder,
    negotiator: TelnetNegotiator,
    data: Vec<u8>,
    controls: Vec<TelnetEvent>,
}

impl Peer {
    fn new(policy: OptionPolicy) -> Self {
        Self {
            decoder: TelnetDecoder::new(),
            negotiator: TelnetNegotiator::new(policy),
            data: vec![],
            controls: vec![],
        }
    }
}

struct Link {
    peers: [Peer; 2],
    messages: VecDeque<(usize, Vec<u8>)>,
    chunk_size: usize,
}

impl Link {
    fn new(first: OptionPolicy, second: OptionPolicy, chunk_size: usize) -> Self {
        Self {
            peers: [Peer::new(first), Peer::new(second)],
            messages: VecDeque::new(),
            chunk_size,
        }
    }

    fn send(&mut self, source: usize, event: TelnetEvent) {
        let mut bytes = Vec::new();
        encode(&event, |chunk| bytes.extend_from_slice(chunk)).unwrap();
        assert!(
            self.messages.len() < 32,
            "peer output queue exceeded test bound"
        );
        self.messages.push_back((1 - source, bytes));
    }

    fn request(&mut self, source: usize, direction: OptionDirection, option: u8, enabled: bool) {
        if let Some(command) = self.peers[source]
            .negotiator
            .request(direction, option, enabled)
            .unwrap()
        {
            self.send(source, command.into());
        }
    }

    fn settle(&mut self) -> usize {
        let mut exchanges = 0;
        while let Some((target, bytes)) = self.messages.pop_front() {
            exchanges += 1;
            assert!(exchanges <= 64, "negotiation failed to quiesce");
            let mut replies = Vec::new();
            let Peer {
                decoder,
                negotiator,
                data,
                controls,
            } = &mut self.peers[target];
            for chunk in bytes.chunks(self.chunk_size) {
                decoder
                    .feed(chunk, |event| match event {
                        TelnetEvent::Negotiation { verb, option } => {
                            if let Some(reply) = negotiator.receive(verb, option) {
                                replies.push(reply);
                            }
                        }
                        TelnetEvent::Data(bytes) => data.extend(bytes),
                        event => controls.push(event),
                    })
                    .unwrap();
            }
            for reply in replies {
                self.send(target, reply.into());
            }
        }
        exchanges
    }

    fn assert_pair(&self, performing_peer: usize, option: u8, enabled: bool) {
        let state = if enabled {
            NegotiationState::Yes
        } else {
            NegotiationState::No
        };
        assert_eq!(
            self.peers[performing_peer]
                .negotiator
                .state(OptionDirection::Local, option),
            state
        );
        assert_eq!(
            self.peers[1 - performing_peer]
                .negotiator
                .state(OptionDirection::Remote, option),
            state
        );
    }

    fn finish(mut self) {
        assert!(self.messages.is_empty());
        for peer in &mut self.peers {
            peer.decoder.finish().unwrap();
            for option in 0..=255 {
                for direction in [OptionDirection::Local, OptionDirection::Remote] {
                    assert!(
                        matches!(
                            peer.negotiator.state(direction, option),
                            NegotiationState::Yes | NegotiationState::No
                        ),
                        "pending negotiation after quiescence"
                    );
                }
            }
        }
    }
}

#[test]
fn mutually_allowed_options_settle_and_leave_other_events_with_the_caller() {
    for chunk in 1..=8 {
        let mut link = Link::new(
            OptionPolicy::new(&[42], &[43]),
            OptionPolicy::new(&[43], &[42]),
            chunk,
        );
        link.request(0, OptionDirection::Local, 42, true);
        link.request(0, OptionDirection::Remote, 43, true);
        assert_eq!(link.settle(), 4);
        link.assert_pair(0, 42, true);
        link.assert_pair(1, 43, true);
        link.request(0, OptionDirection::Local, 42, true);
        assert_eq!(link.settle(), 0);

        let payload = TelnetEvent::Subnegotiation {
            option: 42,
            payload: vec![b'x', 255],
        };
        link.send(0, TelnetEvent::Data(b"raw\r\n\xff".to_vec()));
        link.send(0, payload.clone());
        link.send(0, TelnetEvent::Command(249));
        assert_eq!(link.settle(), 3);
        assert_eq!(link.peers[1].data, b"raw\r\n\xff");
        assert_eq!(
            link.peers[1].controls,
            vec![payload, TelnetEvent::Command(249)]
        );
        link.finish();
    }
}

#[test]
fn asymmetric_policy_refuses_one_direction_without_retry_or_acknowledgment_loops() {
    for chunk in 1..=3 {
        let mut link = Link::new(
            OptionPolicy::new(&[42], &[43]),
            OptionPolicy::new(&[], &[42]),
            chunk,
        );
        link.request(0, OptionDirection::Local, 42, true);
        link.request(0, OptionDirection::Remote, 43, true);
        assert_eq!(link.settle(), 4);
        link.assert_pair(0, 42, true);
        link.assert_pair(1, 43, false);
        assert_eq!(link.settle(), 0);
        link.finish();
    }
}

#[test]
fn simultaneous_complementary_requests_do_not_generate_extra_replies() {
    for chunk in 1..=3 {
        let policy = OptionPolicy::new(&[42], &[42]);
        let mut link = Link::new(policy.clone(), policy, chunk);
        for enabled in [true, false] {
            link.request(0, OptionDirection::Local, 42, enabled);
            link.request(1, OptionDirection::Remote, 42, enabled);
            assert_eq!(link.settle(), 2);
            link.assert_pair(0, 42, enabled);
        }
        link.finish();
    }
}

#[test]
fn queued_reversals_and_cancellations_settle_at_the_last_requested_state() {
    for chunk in 1..=3 {
        for direction in [OptionDirection::Local, OptionDirection::Remote] {
            for initially_enabled in [false, true] {
                for reverse_back in [false, true] {
                    let policy = OptionPolicy::new(&[42], &[42]);
                    let mut link = Link::new(policy.clone(), policy, chunk);
                    if initially_enabled {
                        link.request(0, direction, 42, true);
                        assert_eq!(link.settle(), 2);
                    }
                    link.request(0, direction, 42, !initially_enabled);
                    link.request(0, direction, 42, initially_enabled);
                    if reverse_back {
                        link.request(0, direction, 42, !initially_enabled);
                    }
                    assert_eq!(link.messages.len(), 1, "queued requests leaked onto wire");
                    assert_eq!(link.settle(), if reverse_back { 2 } else { 4 });
                    let performing_peer = if direction == OptionDirection::Local {
                        0
                    } else {
                        1
                    };
                    link.assert_pair(
                        performing_peer,
                        42,
                        if reverse_back {
                            !initially_enabled
                        } else {
                            initially_enabled
                        },
                    );
                    link.finish();
                }
            }
        }
    }
}
