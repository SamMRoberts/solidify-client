# Protocol processing

## Scope

Own streaming Telnet, ANSI, MXP, GMCP, ATCP, and MSP interpretation as those protocols are implemented. These instructions supplement ancestor `AGENTS.md` files for this subtree. `telnet.rs` implements decoding; its `telnet/` modules add encoding and Q-method negotiation with colocated tests. `presentation.rs` implements bounded UTF-8/basic ANSI decoding with tests under `presentation/`. Option-specific behavior, other extension interpretation, and rendering remain planned.

## Working guidance

- Accept arbitrary input chunking; retain only bounded incomplete sequences and preserve text ordering.
- Separate protocol framing from decoded display/events. Never execute server markup or treat it as trusted frontend content.
- Document unsupported variants and malformed-input recovery against primary sources; do not claim compliance from happy-path samples.
- Preserve the [decoder contract](../../../docs/architecture/protocols.md#implemented-telnet-framing): bounded callback events, raw bytes, strict failure latching, and explicit finish/reset. Keep changes to these semantics documented and tested.
- Keep encoding and negotiation separate from decoding. Preserve default-deny policy, independent option directions, fixed state, and ordered command delivery; allowlists are not option implementations.
- Preserve the [presentation contract](../../../docs/architecture/protocols.md#presentation-decoder-contract): only Telnet data is input, bounded text/escape state, atomic SGR, discarded string payloads, payload-free latched failures, and caller-owned rendering/session actions.

## Verification

Test every meaningful split point, escaped control bytes, malformed or oversized payloads, interleaved text, exact encoded bytes, and every Q transition in both directions. Public-API peer tests belong in `src-tauri/tests/` and must bound message exchanges and require quiescence. Keep protocol processing testable without network access. Run the configured [Cargo checks](../../../docs/development/setup.md#library-commands); a fuzz harness is not yet configured.
