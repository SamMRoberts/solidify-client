use super::*;
use serde_json::json;

fn message(data: Option<Value>) -> GmcpMessage {
    GmcpMessage {
        package: "Vendor.Package".into(),
        data,
    }
}

fn payload(message: &GmcpMessage) -> Vec<u8> {
    let TelnetEvent::Subnegotiation { option, payload } = to_subnegotiation(message).unwrap()
    else {
        panic!("expected subnegotiation");
    };
    assert_eq!(option, GMCP);
    payload
}

#[test]
fn absent_data_null_and_all_json_types_round_trip() {
    for data in [
        None,
        Some(Value::Null),
        Some(json!(true)),
        Some(json!(false)),
        Some(json!(-123)),
        Some(json!(u64::MAX)),
        Some(json!(1.25)),
        Some(json!("🌍 café \" \\ \n\t\0 <script>")),
        Some(json!([])),
        Some(json!({})),
        Some(json!([1, "two", {"HP": 3, "hp": 4}])),
    ] {
        let expected = message(data);
        assert_eq!(decode(&payload(&expected)), Ok(expected));
    }
    assert_eq!(payload(&message(None)), b"Vendor.Package");
    assert_eq!(payload(&message(Some(Value::Null))), b"Vendor.Package null");
    assert_eq!(
        payload(&message(Some(json!({"n": [1, 2]})))),
        br#"Vendor.Package {"n":[1,2]}"#
    );
}

#[test]
fn opaque_names_whitespace_and_standard_json_semantics() {
    for name in ["Unknown", "cHaR.ViTaLs", "MSDP", "msdp", "Vendor-v2:_!"] {
        let bytes = format!("{name} \t\r\n {{\"HP\":1,\"hp\":2,\"HP\":3}} \n");
        let decoded = decode(bytes.as_bytes()).unwrap();
        assert_eq!(decoded.package, name);
        assert_eq!(decoded.data, Some(json!({"HP": 3, "hp": 2})));
    }
    assert_eq!(decode(b"Number 1e2").unwrap().data, Some(json!(100.0)));
    assert_eq!(decode(b"Number 1e999"), Err(GmcpError::InvalidJson));
}

#[test]
fn malformed_messages_are_sanitized_and_do_not_poison_later_calls() {
    for (input, error) in [
        (&b""[..], GmcpError::InvalidPackage),
        (&b" null"[..], GmcpError::InvalidPackage),
        (&b"Package\t{}"[..], GmcpError::InvalidPackage),
        (&b"Package\0"[..], GmcpError::InvalidPackage),
        (&b"Package\x7f"[..], GmcpError::InvalidPackage),
        ("Päckage".as_bytes(), GmcpError::InvalidPackage),
        (&b"Package\xff"[..], GmcpError::InvalidUtf8),
        (&b"Package \"\xff\""[..], GmcpError::InvalidUtf8),
        (&b"Package "[..], GmcpError::InvalidJson),
        (&b"Package \t\n"[..], GmcpError::InvalidJson),
        (&b"Package true false"[..], GmcpError::InvalidJson),
        (&b"Package nulljunk"[..], GmcpError::InvalidJson),
        (&b"Package {\"secret\":"[..], GmcpError::InvalidJson),
        (&b"Package \"secret\n\""[..], GmcpError::InvalidJson),
        (&b"Package [}"[..], GmcpError::InvalidJson),
        (&b"Package ]"[..], GmcpError::InvalidJson),
        (&b"Package \"\\uD800\""[..], GmcpError::InvalidJson),
        (&b"Package True"[..], GmcpError::InvalidJson),
    ] {
        let actual = decode(input).unwrap_err();
        assert_eq!(actual, error);
        assert!(!format!("{actual} {actual:?}").contains("secret"));
        assert!(std::error::Error::source(&actual).is_none());
        assert_eq!(decode(b"Valid").unwrap().data, None);
    }
}

#[test]
fn package_boundaries_are_validated_in_both_directions() {
    for length in [1, MAX_PACKAGE_BYTES] {
        let expected = GmcpMessage {
            package: "p".repeat(length),
            data: None,
        };
        assert_eq!(decode(&payload(&expected)), Ok(expected));
    }
    for package in [
        String::new(),
        "p".repeat(MAX_PACKAGE_BYTES + 1),
        "not a name".into(),
        "bad\tname".into(),
        "🌍".into(),
    ] {
        assert_eq!(
            to_subnegotiation(&GmcpMessage {
                package,
                data: None
            }),
            Err(GmcpError::InvalidPackage)
        );
    }
    assert_eq!(
        decode("p".repeat(MAX_PACKAGE_BYTES + 1).as_bytes()),
        Err(GmcpError::InvalidPackage)
    );
}

#[test]
fn complete_payload_limit_includes_package_separator_and_json_escapes() {
    // P + space + quotes leave four fewer bytes for string content.
    let mut expected = GmcpMessage {
        package: "P".into(),
        data: Some(json!("x".repeat(MAX_PAYLOAD_BYTES - 4))),
    };
    let exact = payload(&expected);
    assert_eq!(exact.len(), MAX_PAYLOAD_BYTES);
    assert_eq!(decode(&exact), Ok(expected.clone()));
    let mut oversized = exact;
    oversized.push(b' ');
    assert_eq!(decode(&oversized), Err(GmcpError::PayloadTooLarge));
    expected.data = Some(json!("x".repeat(MAX_PAYLOAD_BYTES - 3)));
    assert_eq!(
        to_subnegotiation(&expected),
        Err(GmcpError::PayloadTooLarge)
    );
    expected.data = Some(json!("\0".repeat(MAX_PAYLOAD_BYTES / 6)));
    assert_eq!(payload(&expected).len(), MAX_PAYLOAD_BYTES);
    expected.data = Some(json!("\0".repeat(MAX_PAYLOAD_BYTES / 6 + 1)));
    assert_eq!(
        to_subnegotiation(&expected),
        Err(GmcpError::PayloadTooLarge)
    );
    expected.data = Some(json!({"x".repeat(MAX_PAYLOAD_BYTES): 0}));
    assert_eq!(
        to_subnegotiation(&expected),
        Err(GmcpError::PayloadTooLarge)
    );
    expected.data = Some(json!(vec![0; MAX_PAYLOAD_BYTES]));
    assert_eq!(
        to_subnegotiation(&expected),
        Err(GmcpError::PayloadTooLarge)
    );
}

fn nested(depth: usize) -> Value {
    let mut value = Value::Null;
    for level in 0..depth {
        value = if level % 2 == 0 {
            json!([value])
        } else {
            json!({"child": value})
        };
    }
    value
}

#[test]
fn nesting_is_bounded_before_parsing_and_serialization() {
    for depth in [0, 1, MAX_JSON_DEPTH] {
        let expected = message(Some(nested(depth)));
        assert_eq!(decode(&payload(&expected)), Ok(expected));
    }
    for depth in [MAX_JSON_DEPTH + 1, 256] {
        let input = format!("P {}null{}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(decode(input.as_bytes()), Err(GmcpError::NestingTooDeep));
        assert_eq!(
            to_subnegotiation(&message(Some(nested(depth)))),
            Err(GmcpError::NestingTooDeep)
        );
    }
    // Empty containers count too; sibling containers do not accumulate depth.
    let input = format!("P {}{}", "[".repeat(64), "]".repeat(64));
    assert!(decode(input.as_bytes()).is_ok());
    let expected = message(Some(json!([nested(63), nested(63)])));
    assert_eq!(decode(&payload(&expected)), Ok(expected));
}

#[test]
fn depth_preflight_respects_quotes_escapes_and_object_keys() {
    let brackets = "[{}]".repeat(100);
    let expected = message(Some(json!({
        brackets.clone(): [brackets, "\\\"[{", "trailing slash\\", "\\\\\"}"]
    })));
    assert_eq!(decode(&payload(&expected)), Ok(expected));
    assert!(decode(br#"P "\u005b\u007b""#).is_ok());
}
