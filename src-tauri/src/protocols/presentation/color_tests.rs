use super::{
    tests::{decode, invariant, text},
    *,
};
fn foreground(color: TextColor) -> PresentationEvent {
    PresentationEvent::StyleChanged(TextStyle {
        foreground: color,
        ..TextStyle::default()
    })
}
#[test]
fn every_bright_and_indexed_color_is_typed_and_canonical_across_splits() {
    for index in 0..=255_u8 {
        let color = if index < 8 {
            basic_color(u16::from(index))
        } else {
            TextColor::Indexed(index)
        };
        for parameters in [format!("38;5;{index}"), format!("38:5:{index}")] {
            assert_eq!(
                invariant(format!("\x1b[{parameters}mX").as_bytes()),
                (vec![foreground(color), text("X")], Ok(()))
            );
        }
        let background = TextStyle {
            background: color,
            ..TextStyle::default()
        };
        assert_eq!(
            invariant(format!("\x1b[48;5;{index}mX\x1b[48:5:{index}mY").as_bytes()),
            (
                vec![PresentationEvent::StyleChanged(background), text("XY")],
                Ok(())
            )
        );
    }
    for index in 8..16 {
        assert_eq!(
            invariant(format!("\x1b[{}mA\x1b[38;5;{index}mB", 90 + index - 8).as_bytes()),
            (
                vec![foreground(TextColor::Indexed(index)), text("AB")],
                Ok(())
            )
        );
        assert_eq!(
            invariant(format!("\x1b[{}mA\x1b[48;5;{index}mB", 100 + index - 8).as_bytes()),
            (
                vec![
                    PresentationEvent::StyleChanged(TextStyle {
                        background: TextColor::Indexed(index),
                        ..TextStyle::default()
                    }),
                    text("AB")
                ],
                Ok(())
            )
        );
    }
}
#[test]
fn direct_rgb_variants_channels_and_following_style_fields_are_atomic() {
    for red in [0, 1, 127, 255] {
        for green in [0, 1, 127, 255] {
            for blue in [0, 1, 127, 255] {
                let color = TextColor::Rgb { red, green, blue };
                for target in [38, 48] {
                    let mut style = TextStyle {
                        bold: true,
                        underline: true,
                        ..TextStyle::default()
                    };
                    if target == 38 {
                        style.foreground = color;
                    } else {
                        style.background = color;
                    }
                    for params in [
                        format!("{target};2;{red};{green};{blue}"),
                        format!("{target}:2:{red}:{green}:{blue}"),
                        format!("{target}:2::{red}:{green}:{blue}"),
                        format!("{target}:2:0:{red}:{green}:{blue}"),
                    ] {
                        assert_eq!(
                            invariant(format!("\x1b[1;{params};4m🌍").as_bytes()),
                            (
                                vec![PresentationEvent::StyleChanged(style), text("🌍")],
                                Ok(())
                            )
                        );
                    }
                }
            }
        }
    }
}
#[test]
fn invalid_operands_modes_and_subparameters_never_leak_partial_styles() {
    for params in [
        "38",
        "38;5",
        "38;5;",
        "38;5;256",
        "38;2;1;3",
        "38;2;;3;4",
        "48;2;1;2;9999999999999999999",
        "38;3;1;3;4",
        "38:5:",
        "38:5:1:2",
        "48:2::1:2",
        "38:2:1:2:3:4",
        "38:2::1:2:3:4",
        "38:2::1:2:256",
        "38:2:0:1::3",
        "38;2:1:2:3",
        "38:2;1;2;3",
        "1:2",
        "?38;5;1",
        "38;5;1 ",
        "38;5;1;999",
    ] {
        assert_eq!(
            invariant(format!("\x1b[1mA\x1b[0;{params}mB").as_bytes()),
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
            "{params}"
        );
    }
}
#[test]
fn extended_colors_resets_controls_and_instances_preserve_lifecycle() {
    let rgb = TextColor::Rgb {
        red: 1,
        green: 2,
        blue: 3,
    };
    assert_eq!(
        invariant(b"\x1b[38;2;1;2;3;48:5:17;1mA\x1b[39;49;22mB"),
        (
            vec![
                PresentationEvent::StyleChanged(TextStyle {
                    foreground: rgb,
                    background: TextColor::Indexed(17),
                    bold: true,
                    ..TextStyle::default()
                }),
                text("A"),
                PresentationEvent::StyleChanged(TextStyle::default()),
                text("B")
            ],
            Ok(())
        )
    );
    assert_eq!(
        invariant(b"\x1b[38;2;1;\x072;3mA"),
        (
            vec![
                PresentationEvent::Control(TextControl::Bell),
                foreground(rgb),
                text("A")
            ],
            Ok(())
        )
    );
    let mut first = PresentationDecoder::new();
    let mut second = PresentationDecoder::new();
    first
        .feed(b"\x1b[38;2;1;", |_| panic!("partial style emitted"))
        .unwrap();
    let mut events = vec![];
    second.feed(b"plain", |e| events.push(e)).unwrap();
    assert_eq!(events, [text("plain")]);
    first.reset();
    events.clear();
    first.feed(b"\x1b[38;5;9m", |e| events.push(e)).unwrap();
    assert_eq!(events, [foreground(TextColor::Indexed(9))]);
    assert_eq!(
        decode(&[b"\x1b[38;2;1;"]).1,
        Err(PresentationError::Truncated {
            sequence: IncompleteSequence::Csi
        })
    );
}
#[test]
fn extended_sgr_keeps_exact_byte_and_semicolon_limits() {
    let base = "\x1b[38:2::255:0:1m";
    let exact = format!(
        "\x1b[38:2::{}255:0:1m",
        "0".repeat(MAX_ESCAPE_BYTES - base.len())
    );
    assert_eq!(exact.len(), MAX_ESCAPE_BYTES);
    assert_eq!(invariant(exact.as_bytes()).1, Ok(()));
    let overflow = exact.replacen("255", "0255", 1);
    assert_eq!(
        invariant(overflow.as_bytes()).1,
        Err(PresentationError::EscapeTooLong)
    );
    let parameters = "38;2;1;2;3;48;2;4;5;6;1;3;4;7;22;23";
    assert_eq!(parameters.split(';').count(), MAX_SGR_PARAMETERS);
    assert_eq!(
        invariant(format!("\x1b[{parameters}m").as_bytes()).1,
        Ok(())
    );
    assert_eq!(
        invariant(format!("\x1b[{parameters};0m").as_bytes()).1,
        Err(PresentationError::TooManySgrParameters)
    );
}
#[test]
fn short_extended_color_samples_match_every_partition() {
    for bytes in [b"\x1b[91mX".as_slice(), b"\x1b[38:5:9m".as_slice()] {
        let expected = decode(&[bytes]);
        for mask in 0..(1_usize << (bytes.len() - 1)) {
            let mut chunks = vec![];
            let mut start = 0;
            for boundary in 1..bytes.len() {
                if mask & (1 << (boundary - 1)) != 0 {
                    chunks.push(&bytes[start..boundary]);
                    start = boundary;
                }
            }
            chunks.push(&bytes[start..]);
            assert_eq!(decode(&chunks), expected);
        }
    }
}
