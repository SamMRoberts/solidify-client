use super::*;
use crate::protocols::telnet::TelnetDecoder;

fn wire(event: &TelnetEvent) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode(event, |chunk| {
        assert!(!chunk.is_empty() && chunk.len() <= MAX_ENCODED_CHUNK_BYTES);
        bytes.extend_from_slice(chunk);
    })
    .unwrap();
    bytes
}

fn decode(chunks: impl IntoIterator<Item = impl AsRef<[u8]>>) -> Vec<TelnetEvent> {
    let mut decoder = TelnetDecoder::new();
    let mut result = Vec::new();
    for chunk in chunks {
        decoder
            .feed(chunk.as_ref(), |event| match (result.last_mut(), event) {
                (Some(TelnetEvent::Data(previous)), TelnetEvent::Data(next)) => {
                    previous.extend(next)
                }
                (_, event) => result.push(event),
            })
            .unwrap();
    }
    decoder.finish().unwrap();
    result
}

#[test]
fn exact_wire_bytes_preserve_data_and_frame_each_event_kind() {
    let cases = [
        (
            TelnetEvent::Data(b"\r\n\0\x1b[31m\xc3\xa9\xff".to_vec()),
            b"\r\n\0\x1b[31m\xc3\xa9\xff\xff".to_vec(),
        ),
        (TelnetEvent::Command(249), vec![255, 249]),
        (
            TelnetEvent::Subnegotiation {
                option: 42,
                payload: vec![1, 255, 2],
            },
            vec![255, 250, 42, 1, 255, 255, 2, 255, 240],
        ),
    ];
    for (event, expected) in cases {
        assert_eq!(wire(&event), expected);
    }
    for (verb, byte) in [
        (NegotiationVerb::Will, 251),
        (NegotiationVerb::Wont, 252),
        (NegotiationVerb::Do, 253),
        (NegotiationVerb::Dont, 254),
    ] {
        for option in 0..=255 {
            assert_eq!(
                wire(&TelnetEvent::Negotiation { verb, option }),
                vec![255, byte, option]
            );
        }
    }
}

#[test]
fn every_data_byte_is_preserved_with_only_iac_doubled() {
    let data: Vec<u8> = (0..=255).collect();
    let mut expected = data.clone();
    expected.push(255);
    assert_eq!(wire(&TelnetEvent::Data(data.clone())), expected);
    let mut framed = vec![255, 250, 255];
    framed.extend(&expected);
    framed.extend([255, 240]);
    assert_eq!(
        wire(&TelnetEvent::Subnegotiation {
            option: 255,
            payload: data
        }),
        framed
    );
}

#[test]
fn empty_data_is_silent_but_empty_subnegotiations_have_framing() {
    encode(&TelnetEvent::Data(vec![]), |_| panic!("empty data emitted")).unwrap();
    for option in 0..=255 {
        assert_eq!(
            wire(&TelnetEvent::Subnegotiation {
                option,
                payload: vec![]
            }),
            vec![255, 250, option, 255, 240]
        );
    }
}

#[test]
fn reserved_commands_fail_before_output_and_other_codes_are_preserved() {
    for command in 0..=255 {
        if matches!(command, 240 | 250..=255) {
            assert_eq!(
                encode(&TelnetEvent::Command(command), |_| panic!(
                    "invalid event emitted"
                )),
                Err(EncodeError::ReservedCommand { command })
            );
        } else {
            assert_eq!(wire(&TelnetEvent::Command(command)), vec![255, command]);
        }
    }
}

#[test]
fn subnegotiation_limit_is_checked_before_any_framing_is_emitted() {
    for byte in [b'x', 255] {
        for length in [MAX_SUBNEGOTIATION_BYTES - 1, MAX_SUBNEGOTIATION_BYTES] {
            let event = TelnetEvent::Subnegotiation {
                option: 42,
                payload: vec![byte; length],
            };
            let bytes = wire(&event);
            assert_eq!(bytes.len(), 5 + length * if byte == 255 { 2 } else { 1 });
            assert_eq!(decode(bytes.chunks(1)), vec![event]);
        }
        let event = TelnetEvent::Subnegotiation {
            option: 42,
            payload: vec![byte; MAX_SUBNEGOTIATION_BYTES + 1],
        };
        assert_eq!(
            encode(&event, |_| panic!("oversized event emitted")),
            Err(EncodeError::SubnegotiationTooLarge)
        );
    }
}

#[test]
fn chunks_remain_bounded_for_large_streams_and_escapes_at_the_boundary() {
    for length in [
        MAX_ENCODED_CHUNK_BYTES - 1,
        MAX_ENCODED_CHUNK_BYTES,
        MAX_ENCODED_CHUNK_BYTES + 1,
        2 * 1024 * 1024,
    ] {
        for byte in [b'x', 255] {
            let mut count = 0;
            encode(&TelnetEvent::Data(vec![byte; length]), |chunk| {
                assert!(!chunk.is_empty() && chunk.len() <= MAX_ENCODED_CHUNK_BYTES);
                assert!(chunk.iter().all(|&value| value == byte));
                count += chunk.len();
            })
            .unwrap();
            assert_eq!(count, length * if byte == 255 { 2 } else { 1 });
        }
    }
    let mut data = vec![b'x'; MAX_ENCODED_CHUNK_BYTES - 1];
    data.extend([255, b'y']);
    let event = TelnetEvent::Data(data);
    let mut chunks = Vec::new();
    encode(&event, |chunk| chunks.push(chunk.to_vec())).unwrap();
    assert_eq!(chunks[0].len(), MAX_ENCODED_CHUNK_BYTES);
    assert_eq!(chunks[0].last(), Some(&255));
    assert_eq!(chunks[1], vec![255, b'y']);
    assert_eq!(decode(chunks), vec![event]);
}

#[test]
fn mixed_events_round_trip_across_all_split_points() {
    let events = vec![
        TelnetEvent::Data(vec![0, b'a', 255]),
        TelnetEvent::Negotiation {
            verb: NegotiationVerb::Will,
            option: 255,
        },
        TelnetEvent::Command(239),
        TelnetEvent::Subnegotiation {
            option: 42,
            payload: vec![240, 250, 255],
        },
        TelnetEvent::Data(b"prompt> ".to_vec()),
    ];
    let bytes: Vec<_> = events.iter().flat_map(wire).collect();
    assert_eq!(decode([&bytes]), events);
    assert_eq!(decode(bytes.chunks(1)), events);
    for split in 0..=bytes.len() {
        assert_eq!(decode([&bytes[..split], &bytes[split..]]), events);
    }
}
