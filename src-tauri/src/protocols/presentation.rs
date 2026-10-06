//! Bounded UTF-8 and a small seven-bit ANSI subset, independent of transport.
//!
//! Feed only Telnet data bytes. Other Telnet events remain the caller's concern
//! and must not interrupt this decoder. Text is untrusted literal content.
//! See the repository's presentation contract for unsupported syntax and local
//! recovery policy; this is not a terminal emulator or an encoding negotiator.
//!
//! UTF-8 validity follows [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629).
//! ESC/CSI syntax and supported SGR values follow
//! [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/).
//! Invalid UTF-8 uses Rust's lossy replacement grouping. Only seven-bit ESC
//! forms are recognized; unsupported C0/DEL and decoded C1 controls are dropped.
//! Any unsupported SGR field makes that complete SGR a no-op. OSC/DCS/SOS/PM/APC
//! payloads are discarded through ST (or BEL for OSC), subject to the string
//! limit. Malformed syntax, excess limits, and truncation latch failure.

use std::fmt;

/// Maximum UTF-8 bytes in one text event, including replacement characters.
pub const MAX_TEXT_BYTES: usize = 4096;
/// Maximum retained incomplete UTF-8 prefix between calls.
pub const MAX_INCOMPLETE_UTF8_BYTES: usize = 3;
/// Maximum ordinary ESC/CSI sequence bytes, including framing and controls.
pub const MAX_ESCAPE_BYTES: usize = 128;
/// Maximum semicolon-separated SGR fields, including empty fields.
pub const MAX_SGR_PARAMETERS: usize = 16;
/// Maximum discarded control-string bytes, including both delimiters.
pub const MAX_CONTROL_STRING_BYTES: usize = 4096;

/// A semantic color; palette/RGB selection belongs to the future renderer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TextColor {
    #[default]
    Default,
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
}

/// Full effective style. Bold does not imply a bright palette color.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub foreground: TextColor,
    pub background: TextColor,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
}

/// Ordered controls, reported without execution or newline normalization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextControl {
    CarriageReturn,
    LineFeed,
    Tab,
    Backspace,
    Bell,
}

/// Owned callback output. Consumers are responsible for bounding retention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PresentationEvent {
    /// Nonempty valid UTF-8, no larger than [`MAX_TEXT_BYTES`].
    Text(String),
    /// Complete style, emitted only on an effective change.
    StyleChanged(TextStyle),
    Control(TextControl),
}

/// Incomplete framing at end of input, without any transcript contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncompleteSequence {
    Escape,
    Csi,
    ControlString,
    ControlStringTerminator,
}

/// Payload-free failures. Malformed/oversized/truncated framing latches failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationError {
    MalformedSequence,
    EscapeTooLong,
    ControlStringTooLong,
    TooManySgrParameters,
    Truncated { sequence: IncompleteSequence },
    DecoderFailed,
    DecoderFinished,
}

impl fmt::Display for PresentationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedSequence => f.write_str("malformed presentation escape sequence"),
            Self::EscapeTooLong => f.write_str("escape sequence exceeds 128 bytes"),
            Self::ControlStringTooLong => f.write_str("control string exceeds 4096 bytes"),
            Self::TooManySgrParameters => f.write_str("SGR exceeds 16 parameters"),
            Self::Truncated { sequence } => {
                write!(f, "incomplete presentation sequence: {sequence:?}")
            }
            Self::DecoderFailed => f.write_str("decoder failed; reset before feeding more input"),
            Self::DecoderFinished => {
                f.write_str("decoder finished; reset before feeding more input")
            }
        }
    }
}

impl std::error::Error for PresentationError {}

#[derive(Clone, Copy, Default)]
enum State {
    #[default]
    Ground,
    Escape {
        intermediate: bool,
    },
    Csi {
        intermediate: bool,
    },
    ControlString {
        osc: bool,
        escaped: bool,
    },
    Failed,
    Finished,
}

/// Connection-local presentation state with fixed partial-input storage.
///
/// No runtime, networking, actions, or internal output queue. Each feed flushes
/// available text. Resetting also requires the consumer to reset its own style.
pub struct PresentationDecoder {
    state: State,
    style: TextStyle,
    utf8: [u8; MAX_INCOMPLETE_UTF8_BYTES],
    utf8_len: usize,
    sequence_len: usize,
    parameters: [u8; MAX_ESCAPE_BYTES],
    parameters_len: usize,
}

impl Default for PresentationDecoder {
    fn default() -> Self {
        Self::new()
    }
}

impl PresentationDecoder {
    /// Starts at default style. Construction emits nothing.
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            style: TextStyle::default(),
            utf8: [0; MAX_INCOMPLETE_UTF8_BYTES],
            utf8_len: 0,
            sequence_len: 0,
            parameters: [0; MAX_ESCAPE_BYTES],
            parameters_len: 0,
        }
    }

    /// Emits available text and complete events synchronously in stream order.
    /// Adjacent text-event boundaries may vary with chunking; contents do not.
    ///
    /// # Errors
    /// Framing errors flush preceding text, discard partial state, and latch
    /// failure. Remaining input is not processed. Even empty input checks the
    /// lifecycle; reset is required after failure or successful finish.
    pub fn feed(
        &mut self,
        input: &[u8],
        mut emit: impl FnMut(PresentationEvent),
    ) -> Result<(), PresentationError> {
        match self.state {
            State::Failed => return Err(PresentationError::DecoderFailed),
            State::Finished => return Err(PresentationError::DecoderFinished),
            _ => {}
        }
        let mut text = String::new();
        for &byte in input {
            if let Err(error) = self.decode_byte(byte, &mut text, &mut emit) {
                flush_text(&mut text, &mut emit);
                self.fail();
                return Err(error);
            }
        }
        flush_text(&mut text, &mut emit);
        Ok(())
    }

    /// Replaces an incomplete UTF-8 suffix and marks end of input. Repeated
    /// successful calls emit nothing and succeed. Does not reset the style.
    ///
    /// # Errors
    /// Truncated escape framing latches failure. A failed decoder remains failed
    /// until reset. No incomplete escape payload is emitted.
    pub fn finish(
        &mut self,
        mut emit: impl FnMut(PresentationEvent),
    ) -> Result<(), PresentationError> {
        let sequence = match self.state {
            State::Ground | State::Finished => {
                let mut text = String::new();
                self.flush_utf8(&mut text, &mut emit);
                flush_text(&mut text, &mut emit);
                self.state = State::Finished;
                return Ok(());
            }
            State::Failed => return Err(PresentationError::DecoderFailed),
            State::Escape { .. } => IncompleteSequence::Escape,
            State::Csi { .. } => IncompleteSequence::Csi,
            State::ControlString { escaped: false, .. } => IncompleteSequence::ControlString,
            State::ControlString { escaped: true, .. } => {
                IncompleteSequence::ControlStringTerminator
            }
        };
        self.fail();
        Err(PresentationError::Truncated { sequence })
    }

    /// Silently discards partial state and restores default style and lifecycle.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn fail(&mut self) {
        self.reset();
        self.state = State::Failed;
    }

    fn decode_byte(
        &mut self,
        byte: u8,
        text: &mut String,
        emit: &mut impl FnMut(PresentationEvent),
    ) -> Result<(), PresentationError> {
        if let State::ControlString { osc, escaped } = self.state {
            self.sequence_len += 1;
            if self.sequence_len > MAX_CONTROL_STRING_BYTES {
                return Err(PresentationError::ControlStringTooLong);
            }
            self.state = if (escaped && byte == b'\\') || (osc && byte == 7) {
                State::Ground
            } else {
                State::ControlString {
                    osc,
                    escaped: byte == 0x1b,
                }
            };
            return Ok(());
        }

        if matches!(self.state, State::Ground) {
            if byte < 0x20 || byte == 0x7f {
                self.flush_utf8(text, emit);
                if byte == 0x1b {
                    self.sequence_len = 1;
                    self.parameters_len = 0;
                    self.state = State::Escape {
                        intermediate: false,
                    };
                } else {
                    emit_control(byte, text, emit);
                }
            } else {
                self.decode_text_byte(byte, text, emit);
            }
            return Ok(());
        }

        self.sequence_len += 1;
        if self.sequence_len > MAX_ESCAPE_BYTES {
            return Err(PresentationError::EscapeTooLong);
        }
        if matches!(byte, 0x1b | 0x18 | 0x1a) {
            return Err(PresentationError::MalformedSequence);
        }
        if byte < 0x20 || byte == 0x7f {
            emit_control(byte, text, emit);
            return Ok(());
        }

        self.state = match self.state {
            State::Escape {
                intermediate: false,
            } if byte == b'[' => State::Csi {
                intermediate: false,
            },
            State::Escape {
                intermediate: false,
            } if matches!(byte, b']' | b'P' | b'X' | b'^' | b'_') => State::ControlString {
                osc: byte == b']',
                escaped: false,
            },
            State::Escape { .. } if (0x20..=0x2f).contains(&byte) => {
                State::Escape { intermediate: true }
            }
            State::Escape { .. } if (0x30..=0x7e).contains(&byte) => State::Ground,
            State::Csi {
                intermediate: false,
            } if (0x30..=0x3f).contains(&byte) => {
                self.parameters[self.parameters_len] = byte;
                self.parameters_len += 1;
                State::Csi {
                    intermediate: false,
                }
            }
            State::Csi { .. } if (0x20..=0x2f).contains(&byte) => State::Csi { intermediate: true },
            State::Csi { intermediate } if (0x40..=0x7e).contains(&byte) => {
                if byte == b'm' {
                    self.sgr(intermediate, text, emit)?;
                }
                State::Ground
            }
            _ => return Err(PresentationError::MalformedSequence),
        };
        Ok(())
    }

    fn sgr(
        &mut self,
        intermediate: bool,
        text: &mut String,
        emit: &mut impl FnMut(PresentationEvent),
    ) -> Result<(), PresentationError> {
        let parameters = &self.parameters[..self.parameters_len];
        if parameters.iter().filter(|&&byte| byte == b';').count() + 1 > MAX_SGR_PARAMETERS {
            return Err(PresentationError::TooManySgrParameters);
        }
        if intermediate
            || parameters
                .iter()
                .any(|byte| !byte.is_ascii_digit() && *byte != b';')
        {
            return Ok(());
        }
        let mut next = self.style;
        for field in parameters.split(|&byte| byte == b';') {
            // Values above the supported range are unknown, not integer errors.
            let value = field.iter().fold(0_u16, |value, byte| {
                (value * 10 + u16::from(byte - b'0')).min(100)
            });
            match value {
                0 => next = TextStyle::default(),
                1 => next.bold = true,
                3 => next.italic = true,
                4 => next.underline = true,
                7 => next.inverse = true,
                22 => next.bold = false,
                23 => next.italic = false,
                24 => next.underline = false,
                27 => next.inverse = false,
                30..=37 => next.foreground = basic_color(value - 30),
                39 => next.foreground = TextColor::Default,
                40..=47 => next.background = basic_color(value - 40),
                49 => next.background = TextColor::Default,
                _ => return Ok(()),
            }
        }
        if next != self.style {
            flush_text(text, emit);
            self.style = next;
            emit(PresentationEvent::StyleChanged(next));
        }
        Ok(())
    }

    fn flush_utf8(&mut self, text: &mut String, emit: &mut impl FnMut(PresentationEvent)) {
        if self.utf8_len != 0 {
            push_char('\u{fffd}', text, emit);
            self.utf8_len = 0;
        }
    }

    fn decode_text_byte(
        &mut self,
        byte: u8,
        text: &mut String,
        emit: &mut impl FnMut(PresentationEvent),
    ) {
        let mut candidate = [0; 4];
        candidate[..self.utf8_len].copy_from_slice(&self.utf8[..self.utf8_len]);
        candidate[self.utf8_len] = byte;
        let len = self.utf8_len + 1;
        match std::str::from_utf8(&candidate[..len]) {
            Ok(value) => {
                self.utf8_len = 0;
                for character in value.chars() {
                    if !('\u{80}'..='\u{9f}').contains(&character) {
                        push_char(character, text, emit);
                    }
                }
            }
            Err(error) => match error.error_len() {
                None => {
                    self.utf8[..len].copy_from_slice(&candidate[..len]);
                    self.utf8_len = len;
                }
                Some(invalid_len) => {
                    self.utf8_len = 0;
                    push_char('\u{fffd}', text, emit);
                    if invalid_len < len {
                        self.decode_text_byte(byte, text, emit);
                    }
                }
            },
        }
    }
}

fn basic_color(index: u16) -> TextColor {
    [
        TextColor::Black,
        TextColor::Red,
        TextColor::Green,
        TextColor::Yellow,
        TextColor::Blue,
        TextColor::Magenta,
        TextColor::Cyan,
        TextColor::White,
    ][usize::from(index)]
}

fn emit_control(byte: u8, text: &mut String, emit: &mut impl FnMut(PresentationEvent)) {
    let control = match byte {
        b'\r' => TextControl::CarriageReturn,
        b'\n' => TextControl::LineFeed,
        b'\t' => TextControl::Tab,
        8 => TextControl::Backspace,
        7 => TextControl::Bell,
        _ => return,
    };
    flush_text(text, emit);
    emit(PresentationEvent::Control(control));
}

fn push_char(character: char, text: &mut String, emit: &mut impl FnMut(PresentationEvent)) {
    if text.len() + character.len_utf8() > MAX_TEXT_BYTES {
        flush_text(text, emit);
    }
    if text.capacity() == 0 {
        text.reserve_exact(MAX_TEXT_BYTES);
    }
    text.push(character);
}

fn flush_text(text: &mut String, emit: &mut impl FnMut(PresentationEvent)) {
    if !text.is_empty() {
        emit(PresentationEvent::Text(std::mem::take(text)));
    }
}

#[cfg(test)]
mod tests;
