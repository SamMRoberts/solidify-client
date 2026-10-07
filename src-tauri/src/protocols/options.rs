//! Passive, bounded TTYPE/NAWS/ECHO/SGA behavior over the existing Q method.
//!
//! Each receive emits at most two responses (a negotiation acknowledgment and
//! initial NAWS); updates emit at most one. Deliver callbacks in order or close
//! the session. Unsupported option payloads are ignored; framing belongs to Telnet.

use super::telnet::{OptionDirection, OptionPolicy, TelnetEvent, TelnetNegotiator};

pub const ECHO: u8 = 1;
pub const SGA: u8 = 3;
pub const TTYPE: u8 = 24;
pub const NAWS: u8 = 31;

/// Approximate character dimensions, not terminal cell geometry. Zero is unknown
/// as permitted by RFC 1073; the desktop uses nonzero dimensions instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub columns: u16,
    pub rows: u16,
}
impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            columns: 80,
            rows: 24,
        }
    }
}

/// Only implemented profiles are exposed; arbitrary allowlists are insufficient.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SessionOptions {
    #[default]
    DenyAll,
    MudClient {
        size: TerminalSize,
    },
}

/// Fixed snapshot independent of text delivery. Generation remembers transient
/// remote ECHO enables even when a subscriber misses intermediate snapshots.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OptionSnapshot {
    pub terminal_type: bool,
    pub window_size: bool,
    pub remote_echo: bool,
    pub local_sga: bool,
    pub remote_sga: bool,
    pub masking_generation: u64,
}

pub struct TerminalOptions {
    negotiator: TelnetNegotiator,
    size: TerminalSize,
    masking_generation: u64,
}
impl TerminalOptions {
    pub fn new(profile: SessionOptions) -> Self {
        let (policy, size) = match profile {
            SessionOptions::DenyAll => (OptionPolicy::default(), TerminalSize::default()),
            SessionOptions::MudClient { size } => {
                (OptionPolicy::new(&[TTYPE, NAWS, SGA], &[ECHO, SGA]), size)
            }
        };
        Self {
            negotiator: TelnetNegotiator::new(policy),
            size,
            masking_generation: 0,
        }
    }
    pub fn snapshot(&self) -> OptionSnapshot {
        let local = |option| self.negotiator.is_enabled(OptionDirection::Local, option);
        let remote = |option| self.negotiator.is_enabled(OptionDirection::Remote, option);
        OptionSnapshot {
            terminal_type: local(TTYPE),
            window_size: local(NAWS),
            remote_echo: remote(ECHO),
            local_sga: local(SGA),
            remote_sga: remote(SGA),
            masking_generation: self.masking_generation,
        }
    }
    pub fn receive(&mut self, event: &TelnetEvent, mut emit: impl FnMut(TelnetEvent)) {
        let before = self.snapshot();
        match event {
            TelnetEvent::Negotiation { verb, option } => {
                if let Some(reply) = self.negotiator.receive(*verb, *option) {
                    emit(reply.into());
                }
                let after = self.snapshot();
                if !before.remote_echo && after.remote_echo {
                    self.masking_generation = self.masking_generation.saturating_add(1);
                }
                if !before.window_size && after.window_size {
                    emit(self.naws());
                }
            }
            TelnetEvent::Subnegotiation {
                option: TTYPE,
                payload,
            } if before.terminal_type && payload == &[1] => {
                let mut payload = Vec::with_capacity(9);
                payload.push(0);
                payload.extend_from_slice(b"SOLIDIFY");
                emit(TelnetEvent::Subnegotiation {
                    option: TTYPE,
                    payload,
                });
            }
            _ => {}
        }
    }
    pub fn set_size(&mut self, size: TerminalSize, mut emit: impl FnMut(TelnetEvent)) {
        if self.size != size {
            self.size = size;
            if self.snapshot().window_size {
                emit(self.naws());
            }
        }
    }
    fn naws(&self) -> TelnetEvent {
        let mut payload = Vec::with_capacity(4);
        payload.extend_from_slice(&self.size.columns.to_be_bytes());
        payload.extend_from_slice(&self.size.rows.to_be_bytes());
        TelnetEvent::Subnegotiation {
            option: NAWS,
            payload,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocols::telnet::{NegotiationVerb::*, TelnetDecoder, encode};

    fn handler() -> TerminalOptions {
        TerminalOptions::new(SessionOptions::MudClient {
            size: TerminalSize::default(),
        })
    }
    fn negotiation(verb: super::super::telnet::NegotiationVerb, option: u8) -> TelnetEvent {
        TelnetEvent::Negotiation { verb, option }
    }
    fn receive(handler: &mut TerminalOptions, event: TelnetEvent) -> Vec<TelnetEvent> {
        let mut result = Vec::new();
        handler.receive(&event, |e| result.push(e));
        result
    }
    #[test]
    fn passive_directions_duplicates_and_isolation() {
        let mut a = handler();
        let b = handler();
        assert_eq!(a.snapshot(), OptionSnapshot::default());
        for (verb, option, reply) in [
            (Do, TTYPE, Will),
            (Do, NAWS, Will),
            (Do, SGA, Will),
            (Will, SGA, Do),
            (Will, ECHO, Do),
        ] {
            assert_eq!(
                receive(&mut a, negotiation(verb, option))[0],
                negotiation(reply, option)
            );
            assert!(receive(&mut a, negotiation(verb, option)).is_empty());
        }
        assert_eq!(
            a.snapshot(),
            OptionSnapshot {
                terminal_type: true,
                window_size: true,
                remote_echo: true,
                local_sga: true,
                remote_sga: true,
                masking_generation: 1
            }
        );
        assert_eq!(b.snapshot(), OptionSnapshot::default());
        for (verb, option, reply) in [
            (Do, ECHO, Wont),
            (Will, TTYPE, Dont),
            (Will, NAWS, Dont),
            (Do, 0, Wont),
            (Will, 201, Dont),
        ] {
            assert_eq!(
                receive(&mut a, negotiation(verb, option)),
                [negotiation(reply, option)]
            );
        }
        receive(&mut a, negotiation(Wont, ECHO));
        assert!(!a.snapshot().remote_echo);
        receive(&mut a, negotiation(Will, ECHO));
        assert_eq!(a.snapshot().masking_generation, 2);
        for (verb, option) in [
            (Dont, TTYPE),
            (Dont, NAWS),
            (Dont, SGA),
            (Wont, SGA),
            (Wont, ECHO),
        ] {
            receive(&mut a, negotiation(verb, option));
        }
        assert_eq!(
            a.snapshot(),
            OptionSnapshot {
                masking_generation: 2,
                ..Default::default()
            }
        );
    }
    #[test]
    fn terminal_identity_requires_exact_request_and_enable() {
        let mut a = handler();
        let request = TelnetEvent::Subnegotiation {
            option: TTYPE,
            payload: vec![1],
        };
        assert!(receive(&mut a, request.clone()).is_empty());
        receive(&mut a, negotiation(Do, TTYPE));
        for _ in 0..3 {
            assert_eq!(
                receive(&mut a, request.clone()),
                [TelnetEvent::Subnegotiation {
                    option: TTYPE,
                    payload: b"\0SOLIDIFY".to_vec()
                }]
            );
        }
        for payload in [vec![], vec![0], vec![1, 0], vec![255], vec![1; 65536]] {
            assert!(
                receive(
                    &mut a,
                    TelnetEvent::Subnegotiation {
                        option: TTYPE,
                        payload
                    }
                )
                .is_empty()
            );
        }
        receive(&mut a, negotiation(Dont, TTYPE));
        assert!(receive(&mut a, request).is_empty());
    }
    #[test]
    fn naws_caches_disabled_sizes_and_escapes_exact_wire() {
        let mut a = handler();
        let size = TerminalSize {
            columns: 255,
            rows: 65535,
        };
        a.set_size(size, |_| panic!("disabled"));
        let replies = receive(&mut a, negotiation(Do, NAWS));
        let mut wire = Vec::new();
        for reply in replies {
            encode(&reply, |bytes| wire.extend_from_slice(bytes)).unwrap();
        }
        assert_eq!(
            wire,
            [
                255, 251, 31, 255, 250, 31, 0, 255, 255, 255, 255, 255, 255, 255, 240
            ]
        );
        a.set_size(size, |_| panic!("unchanged"));
        let mut count = 0;
        a.set_size(
            TerminalSize {
                columns: 1,
                rows: 1,
            },
            |_| count += 1,
        );
        assert_eq!(count, 1);
        receive(&mut a, negotiation(Dont, NAWS));
        a.set_size(TerminalSize::default(), |_| panic!("disabled"));
        assert_eq!(receive(&mut a, negotiation(Do, NAWS)).len(), 2);
    }
    #[test]
    fn all_splits_and_bytewise_exchanges_match() {
        let wire = [
            255, 253, 24, 255, 250, 24, 1, 255, 240, 255, 251, 1, 255, 252, 1, 255, 253, 31,
        ];
        let run = |chunks: Vec<&[u8]>| {
            let mut decoder = TelnetDecoder::new();
            let mut options = handler();
            let mut result = Vec::new();
            for chunk in chunks {
                decoder
                    .feed(chunk, |event| {
                        options.receive(&event, |reply| result.push(reply))
                    })
                    .unwrap();
            }
            decoder.finish().unwrap();
            (result, options.snapshot())
        };
        let expected = run(vec![&wire]);
        for split in 0..=wire.len() {
            assert_eq!(run(vec![&wire[..split], &wire[split..]]), expected);
        }
        assert_eq!(run(wire.chunks(1).collect()), expected);
    }
    #[test]
    fn default_profile_denies_every_option() {
        let mut a = TerminalOptions::new(SessionOptions::DenyAll);
        for option in 0..=255 {
            assert_eq!(
                receive(&mut a, negotiation(Do, option)),
                [negotiation(Wont, option)]
            );
            assert_eq!(
                receive(&mut a, negotiation(Will, option)),
                [negotiation(Dont, option)]
            );
        }
        assert_eq!(a.snapshot(), OptionSnapshot::default());
    }
}
