//! Streaming framing from [RFC 854](https://www.rfc-editor.org/rfc/rfc854)
//! and [RFC 855](https://www.rfc-editor.org/rfc/rfc855).
//!
//! Events preserve stream order. Only doubled IAC bytes are unescaped; text
//! encodings, NVT newline rules, option policy, and command semantics are left
//! to later layers. Unknown standalone command and option codes remain visible.

use std::fmt;

/// Maximum number of bytes in a single [`TelnetEvent::Data`] event.
pub const MAX_DATA_BYTES: usize = 4 * 1024;
/// Maximum decoded subnegotiation payload size, excluding its option byte.
pub const MAX_SUBNEGOTIATION_BYTES: usize = 64 * 1024;

const SE: u8 = 240;
const SB: u8 = 250;
const WILL: u8 = 251;
const WONT: u8 = 252;
const DO: u8 = 253;
const DONT: u8 = 254;
const IAC: u8 = 255;

/// The received negotiation verb, without any decision about accepting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationVerb {
    Will,
    Wont,
    Do,
    Dont,
}

/// One ordered framing event. Payloads are untrusted bytes, not display markup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelnetEvent {
    /// Available data, without waiting for a newline. Never empty.
    Data(Vec<u8>),
    /// A standalone command code, including unknown codes.
    Command(u8),
    /// A complete option request or acknowledgment. No reply is generated.
    Negotiation { verb: NegotiationVerb, option: u8 },
    /// A complete payload with doubled IAC bytes unescaped.
    Subnegotiation { option: u8, payload: Vec<u8> },
}

/// The framing boundary that was incomplete when the input ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompleteFrame {
    Command,
    NegotiationOption,
    SubnegotiationOption,
    SubnegotiationPayload,
    SubnegotiationEscape,
}

/// Framing failures contain no transcript bytes or subnegotiation payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedSubnegotiationEnd,
    UnexpectedSubnegotiationCommand { command: u8 },
    SubnegotiationTooLarge,
    Truncated { frame: IncompleteFrame },
    DecoderFailed,
    DecoderFinished,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedSubnegotiationEnd => {
                f.write_str("subnegotiation end outside a subnegotiation")
            }
            Self::UnexpectedSubnegotiationCommand { command } => {
                write!(f, "unexpected command {command} inside a subnegotiation")
            }
            Self::SubnegotiationTooLarge => f.write_str("subnegotiation payload exceeds 64 KiB"),
            Self::Truncated { frame } => write!(f, "incomplete Telnet frame: {frame:?}"),
            Self::DecoderFailed => f.write_str("decoder failed; reset before feeding more input"),
            Self::DecoderFinished => {
                f.write_str("decoder finished; reset before feeding more input")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

#[derive(Clone, Copy, Default)]
enum State {
    #[default]
    Data,
    Iac,
    Negotiation(NegotiationVerb),
    SubnegotiationOption,
    SubnegotiationData(u8),
    SubnegotiationIac(u8),
    Failed,
    Finished,
}

/// A connection-local byte decoder with no internal event queue.
///
/// Retained partial payloads are limited to [`MAX_SUBNEGOTIATION_BYTES`]. Data
/// is delivered synchronously in chunks of at most [`MAX_DATA_BYTES`]. A caller
/// that retains events is responsible for bounding its own queue.
#[derive(Default)]
pub struct TelnetDecoder {
    state: State,
    payload: Vec<u8>,
}

impl TelnetDecoder {
    /// Creates an empty decoder. Each connection should own a separate instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Delivers complete events in stream order, retaining only incomplete framing.
    ///
    /// Adjacent data-event boundaries may differ with input chunking; their bytes
    /// and order do not. Available data is flushed before this method returns.
    /// Empty input emits nothing but still checks the decoder's lifecycle state.
    ///
    /// # Errors
    ///
    /// Malformed framing or an oversized payload latches failure and discards the
    /// incomplete payload. Events already delivered remain valid. The remaining
    /// input is not processed; no further input is accepted until [`Self::reset`].
    pub fn feed(
        &mut self,
        input: &[u8],
        mut emit: impl FnMut(TelnetEvent),
    ) -> Result<(), DecodeError> {
        match self.state {
            State::Failed => return Err(DecodeError::DecoderFailed),
            State::Finished => return Err(DecodeError::DecoderFinished),
            _ => {}
        }

        let mut data = Vec::new();
        for &byte in input {
            if let Err(error) = self.decode_byte(byte, &mut data, &mut emit) {
                self.payload = Vec::new();
                self.state = State::Failed;
                return Err(error);
            }
        }
        flush_data(&mut data, &mut emit);
        Ok(())
    }

    /// Marks end of input. Repeated successful calls are harmless.
    ///
    /// # Errors
    ///
    /// An incomplete frame reports truncation and latches failure. A previously
    /// failed decoder returns [`DecodeError::DecoderFailed`]. No partial payload
    /// is emitted. Feeding a finished decoder requires [`Self::reset`].
    pub fn finish(&mut self) -> Result<(), DecodeError> {
        let frame = match self.state {
            State::Data | State::Finished => {
                self.state = State::Finished;
                return Ok(());
            }
            State::Failed => return Err(DecodeError::DecoderFailed),
            State::Iac => IncompleteFrame::Command,
            State::Negotiation(_) => IncompleteFrame::NegotiationOption,
            State::SubnegotiationOption => IncompleteFrame::SubnegotiationOption,
            State::SubnegotiationData(_) => IncompleteFrame::SubnegotiationPayload,
            State::SubnegotiationIac(_) => IncompleteFrame::SubnegotiationEscape,
        };
        self.payload = Vec::new();
        self.state = State::Failed;
        Err(DecodeError::Truncated { frame })
    }

    /// Discards all partial input and lifecycle state without emitting events.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn decode_byte(
        &mut self,
        byte: u8,
        data: &mut Vec<u8>,
        emit: &mut impl FnMut(TelnetEvent),
    ) -> Result<(), DecodeError> {
        match self.state {
            State::Data if byte == IAC => {
                flush_data(data, emit);
                self.state = State::Iac;
            }
            State::Data => push_data(byte, data, emit),
            State::Iac => {
                self.state = match byte {
                    IAC => {
                        push_data(IAC, data, emit);
                        State::Data
                    }
                    WILL => State::Negotiation(NegotiationVerb::Will),
                    WONT => State::Negotiation(NegotiationVerb::Wont),
                    DO => State::Negotiation(NegotiationVerb::Do),
                    DONT => State::Negotiation(NegotiationVerb::Dont),
                    SB => State::SubnegotiationOption,
                    SE => return Err(DecodeError::UnexpectedSubnegotiationEnd),
                    command => {
                        emit(TelnetEvent::Command(command));
                        State::Data
                    }
                };
            }
            State::Negotiation(verb) => {
                emit(TelnetEvent::Negotiation { verb, option: byte });
                self.state = State::Data;
            }
            State::SubnegotiationOption => self.state = State::SubnegotiationData(byte),
            State::SubnegotiationData(option) if byte == IAC => {
                self.state = State::SubnegotiationIac(option);
            }
            State::SubnegotiationData(_) => self.push_payload(byte)?,
            State::SubnegotiationIac(option) => match byte {
                IAC => {
                    self.push_payload(IAC)?;
                    self.state = State::SubnegotiationData(option);
                }
                SE => {
                    emit(TelnetEvent::Subnegotiation {
                        option,
                        payload: std::mem::take(&mut self.payload),
                    });
                    self.state = State::Data;
                }
                command => {
                    return Err(DecodeError::UnexpectedSubnegotiationCommand { command });
                }
            },
            State::Failed | State::Finished => unreachable!("feed checks lifecycle state"),
        }
        Ok(())
    }

    fn push_payload(&mut self, byte: u8) -> Result<(), DecodeError> {
        if self.payload.len() == MAX_SUBNEGOTIATION_BYTES {
            return Err(DecodeError::SubnegotiationTooLarge);
        }
        self.payload.push(byte);
        Ok(())
    }
}

fn push_data(byte: u8, data: &mut Vec<u8>, emit: &mut impl FnMut(TelnetEvent)) {
    data.push(byte);
    if data.len() == MAX_DATA_BYTES {
        flush_data(data, emit);
    }
}

fn flush_data(data: &mut Vec<u8>, emit: &mut impl FnMut(TelnetEvent)) {
    if !data.is_empty() {
        emit(TelnetEvent::Data(std::mem::take(data)));
    }
}

#[cfg(test)]
mod tests;
