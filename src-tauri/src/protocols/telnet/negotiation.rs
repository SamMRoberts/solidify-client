//! Symmetric Q-method transitions from RFC 1143, section 7.

use super::{NegotiationVerb, TelnetEvent};
use std::fmt;

/// Which endpoint performs the option, relative to this negotiator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionDirection {
    Local,
    Remote,
}

impl OptionDirection {
    fn index(self) -> usize {
        match self {
            Self::Local => 0,
            Self::Remote => 1,
        }
    }

    fn command(self, option: u8, enable: bool) -> NegotiationCommand {
        let verb = match (self, enable) {
            (Self::Local, true) => NegotiationVerb::Will,
            (Self::Local, false) => NegotiationVerb::Wont,
            (Self::Remote, true) => NegotiationVerb::Do,
            (Self::Remote, false) => NegotiationVerb::Dont,
        };
        NegotiationCommand { verb, option }
    }
}

/// Immutable allowlists. Permission alone does not implement an option's behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionPolicy {
    local: [bool; 256],
    remote: [bool; 256],
}

impl Default for OptionPolicy {
    fn default() -> Self {
        Self {
            local: [false; 256],
            remote: [false; 256],
        }
    }
}

impl OptionPolicy {
    /// Allows only the listed codes in each direction. Duplicate codes are harmless.
    /// Configure real options only when the caller implements their semantics.
    pub fn new(local: &[u8], remote: &[u8]) -> Self {
        let mut policy = Self::default();
        for &option in local {
            policy.local[usize::from(option)] = true;
        }
        for &option in remote {
            policy.remote[usize::from(option)] = true;
        }
        policy
    }

    /// Whether a code may be enabled in the specified direction.
    pub fn allows(&self, direction: OptionDirection, option: u8) -> bool {
        match direction {
            OptionDirection::Local => self.local[usize::from(option)],
            OptionDirection::Remote => self.remote[usize::from(option)],
        }
    }
}

/// The six Q states, including one queued opposite request during negotiation.
/// Only [`Self::Yes`] means enabled; pending states must not activate option behavior.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum NegotiationState {
    #[default]
    No,
    Yes,
    WantNo,
    WantNoOpposite,
    WantYes,
    WantYesOpposite,
}

/// A single outbound request or acknowledgment, to be sent in operation order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NegotiationCommand {
    pub verb: NegotiationVerb,
    pub option: u8,
}

impl From<NegotiationCommand> for TelnetEvent {
    fn from(command: NegotiationCommand) -> Self {
        Self::Negotiation {
            verb: command.verb,
            option: command.option,
        }
    }
}

/// Local policy rejects an explicit request; no state changes or output occur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NegotiationError {
    OptionNotAllowed {
        direction: OptionDirection,
        option: u8,
    },
}

impl fmt::Display for NegotiationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OptionNotAllowed { direction, option } => {
                write!(
                    f,
                    "option {option} is not allowed in the {direction:?} direction"
                )
            }
        }
    }
}

impl std::error::Error for NegotiationError {}

/// Connection-local negotiation with fixed state for both sides of every option.
///
/// This implements [RFC 1143's Q method](https://www.rfc-editor.org/rfc/rfc1143),
/// including its recovery transitions for unexpected acknowledgments. Default
/// policy denies all options. Construction and reset generate no startup traffic.
///
/// Process decoded negotiation events in stream order, then encode and deliver
/// returned commands in that same order. State advances when a command is returned,
/// not when it is delivered. Do not discard commands and continue the connection;
/// a future session owner must tear down/reset on output failure. There is no I/O,
/// retry, timeout, or reply queue here. Other decoded events stay with the caller.
#[derive(Debug)]
pub struct TelnetNegotiator {
    policy: OptionPolicy,
    states: [[NegotiationState; 256]; 2],
}

impl Default for TelnetNegotiator {
    fn default() -> Self {
        Self::new(OptionPolicy::default())
    }
}

impl TelnetNegotiator {
    /// Starts with every option disabled and the supplied immutable policy.
    pub fn new(policy: OptionPolicy) -> Self {
        Self {
            policy,
            states: [[NegotiationState::No; 256]; 2],
        }
    }

    /// Current Q state for one direction of an option.
    pub fn state(&self, direction: OptionDirection, option: u8) -> NegotiationState {
        self.states[direction.index()][usize::from(option)]
    }

    /// True only in the settled YES state.
    pub fn is_enabled(&self, direction: OptionDirection, option: u8) -> bool {
        self.state(direction, option) == NegotiationState::Yes
    }

    /// Requests a state, returning at most one command. Repeats are idempotent;
    /// reversals while negotiating update the queued opposite request.
    ///
    /// # Errors
    ///
    /// Enabling a disallowed option returns [`NegotiationError::OptionNotAllowed`]
    /// without changing state. Disabling is always permitted.
    pub fn request(
        &mut self,
        direction: OptionDirection,
        option: u8,
        enabled: bool,
    ) -> Result<Option<NegotiationCommand>, NegotiationError> {
        if enabled && !self.policy.allows(direction, option) {
            return Err(NegotiationError::OptionNotAllowed { direction, option });
        }
        use NegotiationState::*;
        let state = &mut self.states[direction.index()][usize::from(option)];
        let (next, send) = match (*state, enabled) {
            (No, true) => (WantYes, Some(true)),
            (Yes, false) => (WantNo, Some(false)),
            (WantNo, true) => (WantNoOpposite, None),
            (WantNoOpposite, false) => (WantNo, None),
            (WantYes, false) => (WantYesOpposite, None),
            (WantYesOpposite, true) => (WantYes, None),
            _ => (*state, None),
        };
        *state = next;
        Ok(send.map(|enable| direction.command(option, enable)))
    }

    /// Handles one received verb and returns at most one ordered reply.
    /// Unsupported enables are refused; duplicate acknowledgments do not loop.
    /// Protocol anomalies use the RFC 1143 recovery transitions without logging.
    pub fn receive(&mut self, verb: NegotiationVerb, option: u8) -> Option<NegotiationCommand> {
        let (direction, positive) = match verb {
            NegotiationVerb::Do => (OptionDirection::Local, true),
            NegotiationVerb::Dont => (OptionDirection::Local, false),
            NegotiationVerb::Will => (OptionDirection::Remote, true),
            NegotiationVerb::Wont => (OptionDirection::Remote, false),
        };
        let allowed = self.policy.allows(direction, option);
        use NegotiationState::*;
        let state = &mut self.states[direction.index()][usize::from(option)];
        let (next, send) = match (*state, positive) {
            (No, true) if allowed => (Yes, Some(true)),
            (No, true) => (No, Some(false)),
            (No, false) | (Yes, true) => (*state, None),
            (Yes, false) => (No, Some(false)),
            // An enable answering our disable is anomalous: settle without
            // generating further traffic, as specified by RFC 1143 section 7.
            (WantNo, _) => (No, None),
            (WantNoOpposite, true) => (Yes, None),
            (WantNoOpposite, false) => (WantYes, Some(true)),
            (WantYes, true) => (Yes, None),
            (WantYes, false) | (WantYesOpposite, false) => (No, None),
            (WantYesOpposite, true) => (WantNo, Some(false)),
        };
        *state = next;
        send.map(|enable| direction.command(option, enable))
    }

    /// Clears all states and queued requests while preserving policy. Emits nothing.
    pub fn reset(&mut self) {
        self.states = [[NegotiationState::No; 256]; 2];
    }
}

#[cfg(test)]
#[path = "negotiation_tests.rs"]
mod tests;
