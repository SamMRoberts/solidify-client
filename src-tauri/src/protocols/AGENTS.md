# Protocol processing

## Scope

Own streaming Telnet, ANSI, MXP, GMCP, ATCP, and MSP interpretation as those protocols are implemented. These instructions supplement ancestor `AGENTS.md` files for this subtree. `telnet.rs` implements byte framing with colocated tests in `telnet/tests.rs`; negotiation policy and all extension interpretation remain planned.

## Working guidance

- Accept arbitrary input chunking; retain only bounded incomplete sequences and preserve text ordering.
- Separate protocol framing from decoded display/events. Never execute server markup or treat it as trusted frontend content.
- Document unsupported variants and malformed-input recovery against primary sources; do not claim compliance from happy-path samples.
- Preserve the [decoder contract](../../../docs/architecture/protocols.md#implemented-telnet-framing): bounded callback events, raw bytes, strict failure latching, and explicit finish/reset. Keep changes to these semantics documented and tested.

## Verification

Test every meaningful split point, escaped control bytes, repeated negotiation, malformed or oversized payloads, and interleaved text. Keep parsers fuzzable without network access. Run the configured [Cargo checks](../../../docs/development/setup.md#library-commands); a fuzz harness is not yet configured.
