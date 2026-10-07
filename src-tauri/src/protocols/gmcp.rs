//! Stateless GMCP envelopes over complete, already-unescaped Telnet payloads.
//!
//! Negotiation, framing, delivery, and package semantics belong to the caller.
//! A rejected message does not affect later calls. Names and JSON are untrusted
//! data, never display markup or executable actions. See the
//! [GMCP guide](https://tintin.mudhalla.net/protocols/gmcp/).

use super::telnet::{MAX_SUBNEGOTIATION_BYTES, TelnetEvent};
use serde_json::Value;
use std::{fmt, io};

pub const GMCP: u8 = 201;
/// Complete payload, including package and optional separator, before escaping.
pub const MAX_PAYLOAD_BYTES: usize = MAX_SUBNEGOTIATION_BYTES;
/// Local interoperability limit, not a GMCP wire requirement.
pub const MAX_PACKAGE_BYTES: usize = 256;
/// Maximum number of enclosing JSON arrays/objects, including the root container.
pub const MAX_JSON_DEPTH: usize = 64;

/// An opaque, case-preserved package and optional JSON value.
/// `None` is distinct from `Some(Value::Null)`. Encoding validates public fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmcpMessage {
    pub package: String,
    pub data: Option<Value>,
}

/// Categories only: no payloads or underlying JSON error strings are retained.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GmcpError {
    InvalidPackage,
    InvalidUtf8,
    InvalidJson,
    PayloadTooLarge,
    NestingTooDeep,
    Serialization,
}

impl fmt::Display for GmcpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidPackage => "invalid GMCP package name",
            Self::InvalidUtf8 => "invalid GMCP UTF-8",
            Self::InvalidJson => "invalid GMCP JSON",
            Self::PayloadTooLarge => "GMCP payload exceeds 64 KiB",
            Self::NestingTooDeep => "GMCP JSON exceeds 64 nested containers",
            Self::Serialization => "GMCP JSON serialization failed",
        })
    }
}
impl std::error::Error for GmcpError {}

/// Decodes one complete option-201 payload, without framing or negotiation.
/// The first ASCII space separates the package from one required JSON value.
/// Without a space there is no data. JSON whitespace is otherwise permitted.
/// Standard `serde_json::Value` number and duplicate-key semantics apply.
///
/// # Errors
/// Rejects invalid package names, UTF-8, JSON, and exceeded byte/depth limits.
/// Failure is per message; no state or partially decoded output is retained.
pub fn decode(payload: &[u8]) -> Result<GmcpMessage, GmcpError> {
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err(GmcpError::PayloadTooLarge);
    }
    let text = std::str::from_utf8(payload).map_err(|_| GmcpError::InvalidUtf8)?;
    let (package, json) = match text.split_once(' ') {
        Some((package, json)) => (package, Some(json)),
        None => (text, None),
    };
    validate_package(package)?;
    let data = json
        .map(|json| {
            check_json_depth(json.as_bytes())?;
            serde_json::from_str(json).map_err(|_| GmcpError::InvalidJson)
        })
        .transpose()?;
    Ok(GmcpMessage {
        package: package.to_owned(),
        data,
    })
}

/// Builds a complete subnegotiation for the existing Telnet encoder.
/// JSON is compact, with exactly one separator when data exists. No partial
/// event escapes on failure. The caller must gate both sends and receives on
/// effective remote GMCP negotiation and deliver encoded bytes in order.
///
/// # Errors
/// Rejects invalid names and exceeded byte/depth limits, or serialization failure.
pub fn to_subnegotiation(message: &GmcpMessage) -> Result<TelnetEvent, GmcpError> {
    validate_package(&message.package)?;
    if let Some(data) = &message.data {
        check_value_depth(data, 0)?;
    }
    let mut writer = PayloadWriter {
        bytes: message.package.as_bytes().to_vec(),
        exceeded: false,
    };
    if let Some(data) = &message.data {
        writer.bytes.push(b' ');
        if serde_json::to_writer(&mut writer, data).is_err() {
            return Err(if writer.exceeded {
                GmcpError::PayloadTooLarge
            } else {
                GmcpError::Serialization
            });
        }
    }
    Ok(TelnetEvent::Subnegotiation {
        option: GMCP,
        payload: writer.bytes,
    })
}

fn validate_package(package: &str) -> Result<(), GmcpError> {
    if package.is_empty()
        || package.len() > MAX_PACKAGE_BYTES
        || !package.bytes().all(|b| b.is_ascii_graphic())
    {
        return Err(GmcpError::InvalidPackage);
    }
    Ok(())
}

// This allocation-free preflight bounds the parser's recursion. Only nesting
// outside strings matters; serde_json remains responsible for syntax validation.
fn check_json_depth(json: &[u8]) -> Result<(), GmcpError> {
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in json {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'[' | b'{' => {
                    depth += 1;
                    if depth > MAX_JSON_DEPTH {
                        return Err(GmcpError::NestingTooDeep);
                    }
                }
                b']' | b'}' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    Ok(())
}

fn check_value_depth(value: &Value, depth: usize) -> Result<(), GmcpError> {
    if matches!(value, Value::Array(_) | Value::Object(_)) && depth == MAX_JSON_DEPTH {
        return Err(GmcpError::NestingTooDeep);
    }
    match value {
        Value::Array(values) => {
            for value in values {
                check_value_depth(value, depth + 1)?;
            }
        }
        Value::Object(values) => {
            for value in values.values() {
                check_value_depth(value, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

struct PayloadWriter {
    bytes: Vec<u8>,
    exceeded: bool,
}
impl io::Write for PayloadWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_PAYLOAD_BYTES - self.bytes.len() {
            self.exceeded = true;
            return Err(io::Error::other("GMCP payload limit"));
        }
        // Grow geometrically, but cap requested capacity even when an escape
        // crosses a growth boundary. Allocator overhead is outside this bound.
        let needed = self.bytes.len() + bytes.len();
        if needed > self.bytes.capacity() {
            let capacity = needed.max(self.bytes.capacity() * 2).min(MAX_PAYLOAD_BYTES);
            self.bytes.reserve_exact(capacity - self.bytes.len());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
#[path = "gmcp/tests.rs"]
mod tests;
