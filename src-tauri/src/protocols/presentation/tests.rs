use super::*;

fn coalesce(events: Vec<PresentationEvent>) -> Vec<PresentationEvent> {
    let mut normalized = Vec::new();
    for event in events {
        if let PresentationEvent::Text(text) = &event {
            assert!(!text.is_empty());
            assert!(text.len() <= MAX_TEXT_BYTES);
            if let Some(PresentationEvent::Text(previous)) = normalized.last_mut() {
                previous.push_str(text);
                continue;
            }
        }
        normalized.push(event);
    }
    normalized
}

fn decode(chunks: &[&[u8]]) -> (Vec<PresentationEvent>, Result<(), PresentationError>) {
    let mut decoder = PresentationDecoder::new();
    let mut events = Vec::new();
    for chunk in chunks {
        if let Err(error) = decoder.feed(chunk, |event| events.push(event)) {
            return (coalesce(events), Err(error));
        }
    }
    let result = decoder.finish(|event| events.push(event));
    (coalesce(events), result)
}

fn invariant(input: &[u8]) -> (Vec<PresentationEvent>, Result<(), PresentationError>) {
    let expected = decode(&[input]);
    for split in 0..=input.len() {
        assert_eq!(
            decode(&[&input[..split], &[], &input[split..]]),
            expected,
            "split {split}"
        );
    }
    assert_eq!(decode(&input.chunks(1).collect::<Vec<_>>()), expected);
    expected
}

fn text(value: &str) -> PresentationEvent {
    PresentationEvent::Text(value.to_owned())
}

#[test]
fn prompts_flush_without_newline_and_hold_only_incomplete_unicode() {
    let mut decoder = PresentationDecoder::new();
    let mut events = Vec::new();
    decoder
        .feed(b"Name: \xf0\x9f\x8c", |e| events.push(e))
        .unwrap();
    assert_eq!(events, [text("Name: ")]);
    decoder.feed(b"\x8d> ", |e| events.push(e)).unwrap();
    assert_eq!(events, [text("Name: "), text("🌍> ")]);
    decoder
        .feed(&[], |_| panic!("empty input emitted"))
        .unwrap();
}

#[test]
fn unicode_and_replacements_match_lossy_decoding_at_all_splits() {
    for input in [
        "\u{feff}ASCII é 中 🌍 e\u{301}".as_bytes(),
        b"\xc0\xaf\xc1\xbf",
        b"\xed\xa0\x80",
        b"\xf4\x90\x80\x80",
        b"\xe0\x80\x80",
        b"\xf0\x80\x80\x80",
        b"\xf5\xff\xfe",
        b"\xe2\x82X\xe2X\xf0\x90\x80X",
        b"\xc2",
        b"\xe2\x82",
        b"\xf0\x90\x80",
        b"\xc2\xc2\xa0",
        b"\x80\x9b[31mtext",
        b"\xf4\x8f\xbf\xbf",
    ] {
        assert_eq!(
            invariant(input),
            (vec![text(&String::from_utf8_lossy(input))], Ok(()))
        );
    }
}

#[test]
fn all_two_byte_ordinary_runs_match_the_standard_library_oracle() {
    for first in 0x20..=255_u8 {
        for second in 0x20..=255_u8 {
            if first == 0x7f || second == 0x7f {
                continue;
            }
            let input = [first, second];
            let expected: String = String::from_utf8_lossy(&input)
                .chars()
                .filter(|c| !('\u{80}'..='\u{9f}').contains(c))
                .collect();
            let events = if expected.is_empty() {
                vec![]
            } else {
                vec![text(&expected)]
            };
            assert_eq!(decode(&[&input]), (events.clone(), Ok(())), "{input:?}");
            assert_eq!(
                decode(&[&input[..1], &input[1..]]),
                (events, Ok(())),
                "{input:?}"
            );
        }
    }
}

#[test]
fn every_partition_of_short_mixed_streams_is_equivalent() {
    for input in [
        b"a\x1b[1mb".as_slice(),
        b"\xe2\x82\x1b[m\t",
        b"\x1b]x\x1b\\z",
        b"a\x1b[ \x80z",
    ] {
        let expected = decode(&[input]);
        for mask in 0..(1_usize << (input.len() - 1)) {
            let mut chunks = Vec::new();
            let mut start = 0;
            for end in 1..input.len() {
                if mask & (1 << (end - 1)) != 0 {
                    chunks.push(&input[start..end]);
                    start = end;
                }
            }
            chunks.push(&input[start..]);
            assert_eq!(decode(&chunks), expected);
        }
    }
}

#[test]
fn controls_preserve_order_and_interrupt_unicode_without_normalization() {
    assert_eq!(
        invariant(b"x\xe2\x82\r\n\t\x08\x07y"),
        (
            vec![
                text("x�"),
                PresentationEvent::Control(TextControl::CarriageReturn),
                PresentationEvent::Control(TextControl::LineFeed),
                PresentationEvent::Control(TextControl::Tab),
                PresentationEvent::Control(TextControl::Backspace),
                PresentationEvent::Control(TextControl::Bell),
                text("y"),
            ],
            Ok(())
        )
    );
    assert_eq!(
        invariant(b"a\xe2\x1b[31mb"),
        (
            vec![
                text("a�"),
                PresentationEvent::StyleChanged(TextStyle {
                    foreground: TextColor::Red,
                    ..TextStyle::default()
                }),
                text("b")
            ],
            Ok(())
        )
    );
    let mut input = vec![b'a'];
    input.extend((0..=0x1f).filter(|v| !matches!(v, 7..=10 | 13 | 27)));
    input.extend([0x7f, 0xc2, 0x80, 0xc2, 0x9f, b'b']);
    assert_eq!(invariant(&input), (vec![text("ab")], Ok(())));
}

#[test]
fn every_basic_foreground_background_and_color_reset_is_typed() {
    let colors = [
        TextColor::Black,
        TextColor::Red,
        TextColor::Green,
        TextColor::Yellow,
        TextColor::Blue,
        TextColor::Magenta,
        TextColor::Cyan,
        TextColor::White,
    ];
    for (index, color) in colors.into_iter().enumerate() {
        let input = format!("\x1b[{};{}mX\x1b[39mY\x1b[49mZ", 30 + index, 40 + index);
        assert_eq!(
            invariant(input.as_bytes()),
            (
                vec![
                    PresentationEvent::StyleChanged(TextStyle {
                        foreground: color,
                        background: color,
                        ..TextStyle::default()
                    }),
                    text("X"),
                    PresentationEvent::StyleChanged(TextStyle {
                        background: color,
                        ..TextStyle::default()
                    }),
                    text("Y"),
                    PresentationEvent::StyleChanged(TextStyle::default()),
                    text("Z")
                ],
                Ok(())
            )
        );
    }
}

#[test]
fn flags_reset_independently_and_sgr_commits_one_snapshot() {
    let mut style = TextStyle {
        bold: true,
        italic: true,
        underline: true,
        inverse: true,
        ..TextStyle::default()
    };
    let mut expected = vec![PresentationEvent::StyleChanged(style), text("a")];
    style.bold = false;
    expected.extend([PresentationEvent::StyleChanged(style), text("b")]);
    style.italic = false;
    expected.extend([PresentationEvent::StyleChanged(style), text("c")]);
    style.underline = false;
    expected.extend([PresentationEvent::StyleChanged(style), text("d")]);
    style.inverse = false;
    expected.extend([PresentationEvent::StyleChanged(style), text("e")]);
    assert_eq!(
        invariant(b"\x1b[1;3;4;7ma\x1b[22mb\x1b[23mc\x1b[24md\x1b[27me"),
        (expected, Ok(()))
    );
    assert_eq!(
        invariant(b"\x1b[1;22;31;39;0mplain\x1b[m"),
        (vec![text("plain")], Ok(()))
    );
}

#[test]
fn full_reset_empty_fields_leading_zeroes_and_redundant_styles() {
    let bold = TextStyle {
        bold: true,
        ..TextStyle::default()
    };
    assert_eq!(
        invariant(b"\x1b[0001m\x1b[1mA\x1b[31;;1mB\x1b[0mC\x1b[mD"),
        (
            vec![
                PresentationEvent::StyleChanged(bold),
                text("AB"),
                PresentationEvent::StyleChanged(TextStyle::default()),
                text("CD")
            ],
            Ok(())
        )
    );
    let style = TextStyle {
        foreground: TextColor::Red,
        background: TextColor::Blue,
        bold: true,
        italic: true,
        underline: true,
        inverse: true,
    };
    for reset in ["0", "", ";"] {
        let input = format!("\x1b[31;44;1;3;4;7m\x1b[{reset}m");
        assert_eq!(
            invariant(input.as_bytes()),
            (
                vec![
                    PresentationEvent::StyleChanged(style),
                    PresentationEvent::StyleChanged(TextStyle::default())
                ],
                Ok(())
            )
        );
    }
}

#[test]
fn unsupported_sgr_is_atomic_including_extended_color_operands() {
    for parameters in [
        "31;2;1",
        "0;38;2;1;3;4",
        "48;5;1;0",
        "31;91",
        "31;101",
        "31;99999999999999999999999",
        "31:1",
        "?31",
        ">1",
        "31 ",
        "21",
    ] {
        let input = format!("\x1b[1mA\x1b[{parameters}mB");
        assert_eq!(
            invariant(input.as_bytes()),
            (
                vec![
                    PresentationEvent::StyleChanged(TextStyle {
                        bold: true,
                        ..TextStyle::default()
                    }),
                    text("AB")
                ],
                Ok(())
            ),
            "{parameters}"
        );
    }
}

#[test]
fn unsupported_esc_and_csi_sequences_are_consumed() {
    assert_eq!(
        invariant(b"a\x1b[2Jb\x1b[1;2Hc\x1b[?25ld\x1b(B\x1bc\x1b\\e\x1b[1 qf"),
        (vec![text("abcdef")], Ok(()))
    );
}

#[test]
fn strings_discard_opaque_payload_and_recognize_only_their_terminators() {
    for introducer in *b"]PX^_" {
        let mut input = vec![b'a', 0x1b, introducer];
        input.extend(b"private\xff\x00\r\n\x18\x1a\x1b[31m\x1bQ\x1b\x1b\\b");
        assert_eq!(invariant(&input), (vec![text("ab")], Ok(())));
        let bel = [b'a', 0x1b, introducer, b'x', 7, b'y', 0x1b, b'\\', b'b'];
        assert_eq!(
            invariant(&bel),
            (
                vec![text(if introducer == b']' { "ayb" } else { "ab" })],
                Ok(())
            )
        );
    }
    assert_eq!(
        invariant(b"a\x1b]8;;https://example.invalid\x07link\x1b]8;;\x07z"),
        (vec![text("alinkz")], Ok(()))
    );
    assert_eq!(
        invariant(b"a\x1b]52;c;private\x1b\\b"),
        (vec![text("ab")], Ok(()))
    );
}

#[test]
fn embedded_controls_are_emitted_and_count_toward_sequence_length() {
    assert_eq!(
        invariant(b"a\x1b\t[3\r1\x00\x7fmz"),
        (
            vec![
                text("a"),
                PresentationEvent::Control(TextControl::Tab),
                PresentationEvent::Control(TextControl::CarriageReturn),
                PresentationEvent::StyleChanged(TextStyle {
                    foreground: TextColor::Red,
                    ..TextStyle::default()
                }),
                text("z")
            ],
            Ok(())
        )
    );
    let mut input = b"\x1b[".to_vec();
    input.extend([0; MAX_ESCAPE_BYTES - 3]);
    input.push(b'm');
    assert_eq!(invariant(&input), (vec![], Ok(())));
    input.insert(2, 0);
    assert_eq!(invariant(&input).1, Err(PresentationError::EscapeTooLong));
}

#[test]
fn escape_limits_include_introducer_and_final_byte() {
    for prefix in [b"\x1b[".as_slice(), b"\x1b"] {
        let mut input = prefix.to_vec();
        input.resize(MAX_ESCAPE_BYTES - 1, b' ');
        input.push(b'm');
        assert_eq!(invariant(&input), (vec![], Ok(())));
        input.insert(input.len() - 1, b' ');
        assert_eq!(invariant(&input).1, Err(PresentationError::EscapeTooLong));
    }
}

#[test]
fn sgr_field_limit_counts_empty_and_unsupported_fields() {
    let valid = format!("\x1b[{}m", [""; MAX_SGR_PARAMETERS].join(";"));
    assert_eq!(invariant(valid.as_bytes()), (vec![], Ok(())));
    for field in ["", "1", "999", "?"] {
        let input = format!(
            "before\x1b[{}mafter",
            [field; MAX_SGR_PARAMETERS + 1].join(";")
        );
        assert_eq!(
            invariant(input.as_bytes()),
            (
                vec![text("before")],
                Err(PresentationError::TooManySgrParameters)
            )
        );
    }
    let supported = format!("\x1b[{}m", ["1"; MAX_SGR_PARAMETERS].join(";"));
    assert_eq!(
        invariant(supported.as_bytes()),
        (
            vec![PresentationEvent::StyleChanged(TextStyle {
                bold: true,
                ..TextStyle::default()
            })],
            Ok(())
        )
    );
}

#[test]
fn string_limits_include_both_delimiters_and_apply_to_every_kind() {
    for introducer in *b"]PX^_" {
        for terminator in [b"\x1b\\".as_slice(), b"\x07"] {
            if terminator == b"\x07" && introducer != b']' {
                continue;
            }
            let mut input = vec![0x1b, introducer];
            input.resize(MAX_CONTROL_STRING_BYTES - terminator.len(), b'x');
            input.extend(terminator);
            for size in [1, 2, 127, 128, 4095, 4096] {
                assert_eq!(
                    decode(&input.chunks(size).collect::<Vec<_>>()),
                    (vec![], Ok(()))
                );
            }
            input.insert(2, b'x');
            for split in [0, 2, 4094, 4095, 4096, 4097] {
                assert_eq!(
                    decode(&[&input[..split], &input[split..]]),
                    (vec![], Err(PresentationError::ControlStringTooLong))
                );
            }
            let no_terminator = vec![b'x'; MAX_CONTROL_STRING_BYTES];
            let mut decoder = PresentationDecoder::new();
            decoder
                .feed(&[0x1b, introducer], |_| panic!("string output"))
                .unwrap();
            assert_eq!(
                decoder.feed(&no_terminator, |_| panic!("string output")),
                Err(PresentationError::ControlStringTooLong)
            );
        }
    }
}

#[test]
fn malformed_sequences_flush_prior_text_and_stop_before_trailing_data() {
    for malformed in [
        b"\x1b\x80".as_slice(),
        b"\x1b[ \x31",
        b"\x1b[\xff",
        b"\x1b\x1b",
        b"\x1b[\x18",
        b"\x1b \x1a",
    ] {
        let mut input = b"before".to_vec();
        input.extend(malformed);
        input.extend(b"after");
        assert_eq!(
            invariant(&input),
            (
                vec![text("before")],
                Err(PresentationError::MalformedSequence)
            )
        );
    }
}

#[test]
fn truncation_identifies_every_incomplete_escape_state_without_payloads() {
    for (input, sequence) in [
        (b"\x1b".as_slice(), IncompleteSequence::Escape),
        (b"\x1b ", IncompleteSequence::Escape),
        (b"\x1b[", IncompleteSequence::Csi),
        (b"\x1b[31", IncompleteSequence::Csi),
        (b"\x1b[31 ", IncompleteSequence::Csi),
        (b"\x1b]private", IncompleteSequence::ControlString),
        (
            b"\x1bPprivate\x1b",
            IncompleteSequence::ControlStringTerminator,
        ),
    ] {
        let (_, result) = invariant(input);
        assert_eq!(result, Err(PresentationError::Truncated { sequence }));
        let error = result.unwrap_err();
        assert!(!format!("{error:?} {error}").contains("private"));
    }
}

#[test]
fn finish_is_idempotent_and_failure_latches_until_reset() {
    let mut decoder = PresentationDecoder::new();
    let mut events = Vec::new();
    decoder.feed(b"\xf0\x90\x80", |e| events.push(e)).unwrap();
    assert!(events.is_empty());
    decoder.finish(|e| events.push(e)).unwrap();
    assert_eq!(events, [text("�")]);
    decoder.finish(|_| panic!("second finish emitted")).unwrap();
    for input in [b"".as_slice(), b"text"] {
        assert_eq!(
            decoder.feed(input, |_| panic!("finished output")),
            Err(PresentationError::DecoderFinished)
        );
    }
    decoder.reset();
    assert_eq!(
        decoder.feed(b"\x1b[\xff", |_| {}),
        Err(PresentationError::MalformedSequence)
    );
    for input in [b"".as_slice(), b"text"] {
        assert_eq!(
            decoder.feed(input, |_| panic!("failed output")),
            Err(PresentationError::DecoderFailed)
        );
    }
    assert_eq!(
        decoder.finish(|_| panic!("failed output")),
        Err(PresentationError::DecoderFailed)
    );
    decoder.reset();
    decoder.feed(b"fresh", |e| events.push(e)).unwrap();
    assert_eq!(events.last(), Some(&text("fresh")));
}

#[test]
fn every_framing_failure_latches_and_reset_restores_default_style() {
    let mut long_escape = b"\x1b[".to_vec();
    long_escape.extend([b'0'; MAX_ESCAPE_BYTES]);
    let mut long_string = b"\x1b]".to_vec();
    long_string.extend([b'x'; MAX_CONTROL_STRING_BYTES]);
    let fields = format!("\x1b[{}m", ["1"; MAX_SGR_PARAMETERS + 1].join(";"));
    for (input, expected) in [
        (b"\x1b[\xff".to_vec(), PresentationError::MalformedSequence),
        (long_escape, PresentationError::EscapeTooLong),
        (long_string, PresentationError::ControlStringTooLong),
        (fields.into_bytes(), PresentationError::TooManySgrParameters),
        (
            b"\x1b[31".to_vec(),
            PresentationError::Truncated {
                sequence: IncompleteSequence::Csi,
            },
        ),
    ] {
        let mut decoder = PresentationDecoder::new();
        decoder.feed(b"\x1b[1m", |_| {}).unwrap();
        let result = decoder
            .feed(&input, |_| panic!("partial frame emitted"))
            .and_then(|()| decoder.finish(|_| panic!("partial frame emitted")));
        assert_eq!(result, Err(expected));
        assert_eq!(
            decoder.feed(&[], |_| panic!("failed output")),
            Err(PresentationError::DecoderFailed)
        );
        assert_eq!(
            decoder.finish(|_| panic!("failed output")),
            Err(PresentationError::DecoderFailed)
        );
        decoder.reset();
        decoder
            .feed(b"\x1b[m", |_| panic!("reset retained old style"))
            .unwrap();
        decoder
            .finish(|_| panic!("reset retained partial input"))
            .unwrap();
    }
}

#[test]
fn every_c0_control_has_the_same_policy_inside_escape_and_csi() {
    for prefix in [b"\x1b".as_slice(), b"\x1b[", b"\x1b ", b"\x1b[ "] {
        for byte in (0..=0x1f).chain(std::iter::once(0x7f)) {
            let mut input = prefix.to_vec();
            input.extend([byte, b'm']);
            let (events, result) = invariant(&input);
            if matches!(byte, 0x18 | 0x1a | 0x1b) {
                assert_eq!(result, Err(PresentationError::MalformedSequence));
                assert!(events.is_empty());
            } else {
                assert_eq!(result, Ok(()));
                let control = match byte {
                    7 => Some(TextControl::Bell),
                    8 => Some(TextControl::Backspace),
                    9 => Some(TextControl::Tab),
                    10 => Some(TextControl::LineFeed),
                    13 => Some(TextControl::CarriageReturn),
                    _ => None,
                };
                assert_eq!(
                    events,
                    control
                        .into_iter()
                        .map(PresentationEvent::Control)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn reset_clears_all_partial_states_and_style_without_output() {
    for partial in [
        b"\xf0\x90\x80".as_slice(),
        b"\x1b",
        b"\x1b ",
        b"\x1b[31",
        b"\x1b[31 ",
        b"\x1b]private",
        b"\x1bPprivate\x1b",
    ] {
        let mut decoder = PresentationDecoder::new();
        decoder.feed(b"\x1b[1m", |_| {}).unwrap();
        decoder.feed(partial, |_| {}).unwrap();
        decoder.reset();
        let mut events = Vec::new();
        decoder
            .feed(b"\x1b[mplain\x1b[1m", |e| events.push(e))
            .unwrap();
        assert_eq!(
            events,
            [
                text("plain"),
                PresentationEvent::StyleChanged(TextStyle {
                    bold: true,
                    ..TextStyle::default()
                })
            ]
        );
        decoder
            .finish(|_| panic!("reset left partial state"))
            .unwrap();
    }
}

#[test]
fn instances_isolate_styles_unicode_framing_and_failure() {
    let mut first = PresentationDecoder::new();
    let mut second = PresentationDecoder::new();
    first.feed(b"\x1b[1m\xc3", |_| {}).unwrap();
    let mut events = Vec::new();
    second.feed(b"\x1b[mB", |e| events.push(e)).unwrap();
    first.feed(b"\xa9\x1b[", |e| events.push(e)).unwrap();
    first.finish(|_| {}).unwrap_err();
    second.feed(b"C", |e| events.push(e)).unwrap();
    assert_eq!(events, [text("B"), text("é"), text("C")]);
    second.finish(|_| {}).unwrap();
}

#[test]
fn text_boundaries_preserve_scalars_and_count_replacement_expansion() {
    for prefix_len in [4092, 4093, 4094, 4095, 4096, 4097] {
        for suffix in ["é".as_bytes(), "🌍".as_bytes(), b"\xff"] {
            let mut input = vec![b'a'; prefix_len];
            input.extend(suffix);
            let expected = String::from_utf8_lossy(&input).into_owned();
            assert_eq!(decode(&[&input]), (vec![text(&expected)], Ok(())));
        }
    }
    let input = vec![0xff; 4096];
    assert_eq!(decode(&[&input]), (vec![text(&"�".repeat(4096))], Ok(())));
    let mut decoder = PresentationDecoder::new();
    let mut lengths = Vec::new();
    decoder
        .feed(&vec![b'a'; 8193], |event| {
            let PresentationEvent::Text(value) = event else {
                panic!("unexpected event");
            };
            lengths.push(value.len());
        })
        .unwrap();
    assert_eq!(lengths, [4096, 4096, 1]);
}

#[test]
fn large_streams_emit_bounded_events_without_output_retention() {
    let input = vec![0xff; 2 * 1024 * 1024];
    let mut decoder = PresentationDecoder::new();
    let mut bytes = 0;
    decoder
        .feed(&input, |event| {
            let PresentationEvent::Text(value) = event else {
                panic!("unexpected event");
            };
            assert!(!value.is_empty() && value.len() <= MAX_TEXT_BYTES);
            assert!(value.chars().all(|c| c == '\u{fffd}'));
            bytes += value.len();
        })
        .unwrap();
    assert_eq!(bytes, 3 * input.len());
}
