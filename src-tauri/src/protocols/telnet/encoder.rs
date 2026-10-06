use super::{
    DO, DONT, IAC, MAX_SUBNEGOTIATION_BYTES, NegotiationVerb, SB, SE, TelnetEvent, WILL, WONT,
};
use std::fmt;

/// Maximum wire bytes delivered in one encoder callback.
pub const MAX_ENCODED_CHUNK_BYTES: usize = 4 * 1024;

/// Invalid outgoing framing; errors never retain event payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    ReservedCommand { command: u8 },
    SubnegotiationTooLarge,
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReservedCommand { command } => {
                write!(f, "command {command} requires structured framing")
            }
            Self::SubnegotiationTooLarge => f.write_str("subnegotiation payload exceeds 64 KiB"),
        }
    }
}

impl std::error::Error for EncodeError {}

/// Encodes one event using a fixed 4 KiB scratch buffer and synchronous callbacks.
///
/// Only IAC escaping and Telnet framing are applied. Empty data emits nothing;
/// empty subnegotiations still emit their option and delimiters. Callback chunks
/// may split control sequences and are borrowed only for the duration of the call.
/// Emit all chunks in order before encoding later events. The caller owns output
/// retention, backpressure, and transport error handling; this function does no I/O.
/// Encoding does not check whether a subnegotiation's option has been enabled.
///
/// # Errors
///
/// Reserved standalone commands and oversized subnegotiations are rejected before
/// any callback is invoked. Errors contain only categories and control metadata.
pub fn encode(event: &TelnetEvent, mut emit: impl FnMut(&[u8])) -> Result<(), EncodeError> {
    match event {
        TelnetEvent::Command(command)
            if matches!(*command, SE | SB | WILL | WONT | DO | DONT | IAC) =>
        {
            return Err(EncodeError::ReservedCommand { command: *command });
        }
        TelnetEvent::Subnegotiation { payload, .. } if payload.len() > MAX_SUBNEGOTIATION_BYTES => {
            return Err(EncodeError::SubnegotiationTooLarge);
        }
        _ => {}
    }

    let mut buffer = [0; MAX_ENCODED_CHUNK_BYTES];
    let mut length = 0;
    {
        let mut push = |byte| {
            buffer[length] = byte;
            length += 1;
            if length == buffer.len() {
                emit(&buffer);
                length = 0;
            }
        };
        match event {
            TelnetEvent::Data(data) => escape(data, &mut push),
            TelnetEvent::Command(command) => {
                push(IAC);
                push(*command);
            }
            TelnetEvent::Negotiation { verb, option } => {
                push(IAC);
                push(match verb {
                    NegotiationVerb::Will => WILL,
                    NegotiationVerb::Wont => WONT,
                    NegotiationVerb::Do => DO,
                    NegotiationVerb::Dont => DONT,
                });
                push(*option);
            }
            TelnetEvent::Subnegotiation { option, payload } => {
                push(IAC);
                push(SB);
                push(*option);
                escape(payload, &mut push);
                push(IAC);
                push(SE);
            }
        }
    }
    if length != 0 {
        emit(&buffer[..length]);
    }
    Ok(())
}

fn escape(bytes: &[u8], push: &mut impl FnMut(u8)) {
    for &byte in bytes {
        push(byte);
        if byte == IAC {
            push(IAC);
        }
    }
}

#[cfg(test)]
#[path = "encoder_tests.rs"]
mod tests;
