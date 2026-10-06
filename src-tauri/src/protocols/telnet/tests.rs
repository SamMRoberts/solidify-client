use super::*;

fn decode(chunks: &[&[u8]]) -> (Vec<TelnetEvent>, Result<(), DecodeError>) {
    let mut decoder = TelnetDecoder::new();
    let mut events = Vec::new();
    for chunk in chunks {
        if let Err(error) = decoder.feed(chunk, |event| events.push(event)) {
            return (coalesce(events), Err(error));
        }
    }
    (coalesce(events), decoder.finish())
}

fn coalesce(events: Vec<TelnetEvent>) -> Vec<TelnetEvent> {
    let mut result = Vec::new();
    for event in events {
        match (result.last_mut(), event) {
            (Some(TelnetEvent::Data(previous)), TelnetEvent::Data(next)) => previous.extend(next),
            (_, event) => result.push(event),
        }
    }
    result
}

fn assert_all_splits(input: &[u8], expected: (Vec<TelnetEvent>, Result<(), DecodeError>)) {
    assert_eq!(decode(&[input]), expected);
    assert_eq!(decode(&input.chunks(1).collect::<Vec<_>>()), expected);
    for split in 0..=input.len() {
        assert_eq!(
            decode(&[&input[..split], &input[split..]]),
            expected,
            "split {split}"
        );
    }
}

#[test]
fn empty_input_and_finish_emit_nothing() {
    assert_eq!(decode(&[b"", b""]), (vec![], Ok(())));
}

#[test]
fn ordinary_bytes_are_not_decoded_or_normalized() {
    let input = b"Snow: \xe2\x98\x83\r\n\r\0\x1b[31m<untrusted>\x80\xfe";
    assert_all_splits(input, (vec![TelnetEvent::Data(input.to_vec())], Ok(())));
}

#[test]
fn every_byte_value_can_be_carried_as_data() {
    let expected: Vec<u8> = (0..=255).collect();
    let mut encoded = expected.clone();
    encoded.push(IAC);
    assert_all_splits(&encoded, (vec![TelnetEvent::Data(expected)], Ok(())));
}

#[test]
fn partial_prompts_are_emitted_at_each_feed_boundary() {
    let mut decoder = TelnetDecoder::new();
    let mut events = Vec::new();
    decoder.feed(b"Pass", |event| events.push(event)).unwrap();
    assert_eq!(events, vec![TelnetEvent::Data(b"Pass".to_vec())]);
    events.clear();
    decoder.feed(b"word: ", |event| events.push(event)).unwrap();
    assert_eq!(events, vec![TelnetEvent::Data(b"word: ".to_vec())]);
    decoder.finish().unwrap();
}

#[test]
fn escaped_iac_remains_data() {
    assert_all_splits(
        &[b'a', IAC, IAC, IAC, IAC, b'b'],
        (vec![TelnetEvent::Data(vec![b'a', IAC, IAC, b'b'])], Ok(())),
    );
}

#[test]
fn standalone_commands_including_unknown_codes_remain_observable() {
    for command in 0..=255 {
        if matches!(command, SE | SB | WILL | WONT | DO | DONT | IAC) {
            continue;
        }
        assert_all_splits(
            &[IAC, command],
            (vec![TelnetEvent::Command(command)], Ok(())),
        );
    }
}

#[test]
fn all_negotiation_verbs_and_option_bytes_are_preserved() {
    for (code, verb) in [
        (WILL, NegotiationVerb::Will),
        (WONT, NegotiationVerb::Wont),
        (DO, NegotiationVerb::Do),
        (DONT, NegotiationVerb::Dont),
    ] {
        for option in 0..=255 {
            assert_all_splits(
                &[IAC, code, option],
                (vec![TelnetEvent::Negotiation { verb, option }], Ok(())),
            );
        }
    }
}

#[test]
fn repeated_negotiations_are_events_without_policy_or_replies() {
    assert_all_splits(
        &[IAC, WILL, 42, IAC, WILL, 42, IAC, WONT, 42],
        (
            vec![
                TelnetEvent::Negotiation {
                    verb: NegotiationVerb::Will,
                    option: 42,
                },
                TelnetEvent::Negotiation {
                    verb: NegotiationVerb::Will,
                    option: 42,
                },
                TelnetEvent::Negotiation {
                    verb: NegotiationVerb::Wont,
                    option: 42,
                },
            ],
            Ok(()),
        ),
    );
}

#[test]
fn empty_subnegotiations_preserve_every_option_byte() {
    for option in 0..=255 {
        assert_all_splits(
            &[IAC, SB, option, IAC, SE],
            (
                vec![TelnetEvent::Subnegotiation {
                    option,
                    payload: vec![],
                }],
                Ok(()),
            ),
        );
    }
}

#[test]
fn subnegotiation_escapes_and_bare_control_bytes_are_preserved() {
    assert_all_splits(
        &[IAC, SB, 201, SE, SB, WILL, 0, IAC, IAC, IAC, SE],
        (
            vec![TelnetEvent::Subnegotiation {
                option: 201,
                payload: vec![SE, SB, WILL, 0, IAC],
            }],
            Ok(()),
        ),
    );
}

#[test]
fn interleaved_events_preserve_stream_order() {
    let input = [
        b'a', IAC, WILL, 201, b'b', IAC, 249, IAC, SB, 201, b'x', IAC, SE, b'c',
    ];
    assert_all_splits(
        &input,
        (
            vec![
                TelnetEvent::Data(vec![b'a']),
                TelnetEvent::Negotiation {
                    verb: NegotiationVerb::Will,
                    option: 201,
                },
                TelnetEvent::Data(vec![b'b']),
                TelnetEvent::Command(249),
                TelnetEvent::Subnegotiation {
                    option: 201,
                    payload: vec![b'x'],
                },
                TelnetEvent::Data(vec![b'c']),
            ],
            Ok(()),
        ),
    );
}

#[test]
fn short_streams_are_equivalent_across_every_partition() {
    let streams: &[&[u8]] = &[
        &[b'a', IAC, IAC, b'b', IAC, WILL, 42, IAC, 249],
        &[b'a', IAC, SB, 42, b'x', IAC, IAC, IAC, SE, b'b'],
        &[IAC, SB, 42, IAC, SE, IAC, SB, 43, IAC, SE],
        &[b'a', IAC, SB, 42, b'x', IAC, DO, b'b'],
        &[b'a', IAC, SE, b'b'],
        &[b'a', IAC, SB, 42, b'x', IAC],
    ];
    for &input in streams {
        let expected = decode(&[input]);
        for mask in 0..(1usize << (input.len() - 1)) {
            let mut chunks = Vec::new();
            let mut start = 0;
            for boundary in 1..input.len() {
                if mask & (1 << (boundary - 1)) != 0 {
                    chunks.push(&input[start..boundary]);
                    start = boundary;
                }
            }
            chunks.push(&input[start..]);
            assert_eq!(
                decode(&chunks),
                expected,
                "input {input:?}, partition {mask}"
            );
        }
    }
}

#[test]
fn data_event_sizes_are_bounded_at_and_around_the_limit() {
    for length in [
        MAX_DATA_BYTES - 1,
        MAX_DATA_BYTES,
        MAX_DATA_BYTES + 1,
        2 * MAX_DATA_BYTES,
    ] {
        let mut decoder = TelnetDecoder::new();
        let mut sizes = Vec::new();
        decoder
            .feed(&vec![b'x'; length], |event| match event {
                TelnetEvent::Data(data) => {
                    assert!(data.iter().all(|&byte| byte == b'x'));
                    assert!(!data.is_empty() && data.len() <= MAX_DATA_BYTES);
                    sizes.push(data.len());
                }
                other => panic!("unexpected event: {other:?}"),
            })
            .unwrap();
        assert_eq!(sizes.iter().sum::<usize>(), length);
        assert_eq!(sizes.len(), length.div_ceil(MAX_DATA_BYTES));
        decoder.finish().unwrap();
    }
}

#[test]
fn large_data_stream_is_delivered_incrementally_without_retention() {
    let mut decoder = TelnetDecoder::new();
    let mut count = 0;
    decoder
        .feed(&vec![b'x'; 2 * 1024 * 1024], |event| match event {
            TelnetEvent::Data(data) => {
                assert!(data.len() <= MAX_DATA_BYTES);
                count += data.len();
            }
            other => panic!("unexpected event: {other:?}"),
        })
        .unwrap();
    assert_eq!(count, 2 * 1024 * 1024);
    decoder.finish().unwrap();
}

fn subnegotiation(payload_length: usize, escaped: bool) -> Vec<u8> {
    let mut input = vec![IAC, SB, 42];
    if escaped {
        input.extend(std::iter::repeat_n(IAC, payload_length * 2));
    } else {
        input.extend(std::iter::repeat_n(b'x', payload_length));
    }
    input.extend([IAC, SE]);
    input
}

#[test]
fn payload_limit_counts_decoded_bytes_and_excludes_framing() {
    for escaped in [false, true] {
        for length in [MAX_SUBNEGOTIATION_BYTES - 1, MAX_SUBNEGOTIATION_BYTES] {
            let input = subnegotiation(length, escaped);
            let expected = (
                vec![TelnetEvent::Subnegotiation {
                    option: 42,
                    payload: vec![if escaped { IAC } else { b'x' }; length],
                }],
                Ok(()),
            );
            assert_eq!(decode(&[&input]), expected);
            assert_eq!(decode(&input.chunks(1).collect::<Vec<_>>()), expected);
        }
    }
}

#[test]
fn oversized_payload_fails_without_emitting_partial_payload_or_trailing_data() {
    for escaped in [false, true] {
        let mut input = b"before".to_vec();
        input.extend(subnegotiation(MAX_SUBNEGOTIATION_BYTES + 1, escaped));
        input.extend(b"after");
        let expected = (
            vec![TelnetEvent::Data(b"before".to_vec())],
            Err(DecodeError::SubnegotiationTooLarge),
        );
        assert_eq!(decode(&[&input]), expected);
        assert_eq!(decode(&input.chunks(1).collect::<Vec<_>>()), expected);
    }
}

#[test]
fn stray_subnegotiation_end_is_fatal_after_preserving_prior_data() {
    assert_all_splits(
        &[b'a', IAC, SE, b'b'],
        (
            vec![TelnetEvent::Data(vec![b'a'])],
            Err(DecodeError::UnexpectedSubnegotiationEnd),
        ),
    );
}

#[test]
fn every_unexpected_subnegotiation_command_is_fatal() {
    for command in 0..=255 {
        if matches!(command, SE | IAC) {
            continue;
        }
        assert_all_splits(
            &[IAC, SB, 42, b'x', IAC, command, b'b'],
            (
                vec![],
                Err(DecodeError::UnexpectedSubnegotiationCommand { command }),
            ),
        );
    }
}

#[test]
fn eof_reports_every_incomplete_framing_state() {
    let cases: &[(&[u8], IncompleteFrame)] = &[
        (&[IAC], IncompleteFrame::Command),
        (&[IAC, WILL], IncompleteFrame::NegotiationOption),
        (&[IAC, WONT], IncompleteFrame::NegotiationOption),
        (&[IAC, DO], IncompleteFrame::NegotiationOption),
        (&[IAC, DONT], IncompleteFrame::NegotiationOption),
        (&[IAC, SB], IncompleteFrame::SubnegotiationOption),
        (&[IAC, SB, 42], IncompleteFrame::SubnegotiationPayload),
        (&[IAC, SB, 42, b'x'], IncompleteFrame::SubnegotiationPayload),
        (&[IAC, SB, 42, IAC], IncompleteFrame::SubnegotiationEscape),
    ];
    for &(input, frame) in cases {
        assert_all_splits(input, (vec![], Err(DecodeError::Truncated { frame })));
    }
}

#[test]
fn failures_remain_latched_until_reset() {
    let failures = [
        vec![IAC, SE],
        vec![IAC, SB, 42, IAC, SB],
        subnegotiation(MAX_SUBNEGOTIATION_BYTES + 1, false),
        vec![IAC],
    ];
    for input in failures {
        let mut decoder = TelnetDecoder::new();
        let result = decoder.feed(&input, |_| panic!("no event expected"));
        if result.is_ok() {
            assert!(matches!(
                decoder.finish(),
                Err(DecodeError::Truncated { .. })
            ));
        }
        for more in [&b""[..], &b"ignored"[..]] {
            assert_eq!(
                decoder.feed(more, |_| panic!("failed decoder emitted")),
                Err(DecodeError::DecoderFailed)
            );
        }
        assert_eq!(decoder.finish(), Err(DecodeError::DecoderFailed));
        decoder.reset();
        let mut events = Vec::new();
        decoder.feed(b"fresh", |event| events.push(event)).unwrap();
        assert_eq!(events, vec![TelnetEvent::Data(b"fresh".to_vec())]);
        decoder.finish().unwrap();
    }
}

#[test]
fn successful_finish_is_idempotent_and_blocks_even_empty_feeds_until_reset() {
    let mut decoder = TelnetDecoder::new();
    decoder.finish().unwrap();
    decoder.finish().unwrap();
    for input in [&b""[..], &b"ignored"[..]] {
        assert_eq!(
            decoder.feed(input, |_| panic!("finished decoder emitted")),
            Err(DecodeError::DecoderFinished)
        );
    }
    decoder.reset();
    decoder
        .feed(b"", |_| panic!("empty input emitted"))
        .unwrap();
    decoder.finish().unwrap();
}

#[test]
fn reset_discards_every_partial_frame_without_leaking_payload() {
    for input in [
        vec![IAC],
        vec![IAC, WILL],
        vec![IAC, SB],
        vec![IAC, SB, 42, b'x'],
        vec![IAC, SB, 42, IAC],
    ] {
        let mut decoder = TelnetDecoder::new();
        decoder
            .feed(&input, |_| panic!("partial frame emitted"))
            .unwrap();
        decoder.reset();
        let mut events = Vec::new();
        decoder
            .feed(&[IAC, SB, 43, IAC, SE], |event| events.push(event))
            .unwrap();
        assert_eq!(
            events,
            vec![TelnetEvent::Subnegotiation {
                option: 43,
                payload: vec![]
            }]
        );
        decoder.finish().unwrap();
    }
}

#[test]
fn decoder_instances_have_independent_state() {
    let mut first = TelnetDecoder::new();
    let mut second = TelnetDecoder::new();
    first
        .feed(&[IAC, SB, 42, b'x'], |_| panic!("partial frame emitted"))
        .unwrap();
    let mut events = Vec::new();
    second
        .feed(b"independent", |event| events.push(event))
        .unwrap();
    second.finish().unwrap();
    first.feed(&[IAC, SE], |event| events.push(event)).unwrap();
    first.finish().unwrap();
    assert_eq!(
        events,
        vec![
            TelnetEvent::Data(b"independent".to_vec()),
            TelnetEvent::Subnegotiation {
                option: 42,
                payload: vec![b'x']
            },
        ]
    );
}
