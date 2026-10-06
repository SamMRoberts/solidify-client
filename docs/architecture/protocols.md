# Protocol support and requirements

## Status

Byte-level Telnet decoding/encoding and configurable Q-method negotiation are implemented. Bounded TCP transport is implemented separately in [sessions](sessions.md); option-specific behavior, text decoding, and extension interpretation remain planned. The references below establish wire syntax and negotiation behavior, not a claim of complete Telnet or MUD compatibility.

## Implemented Telnet framing

The internal Rust interface is `solidify_client::protocols::telnet`. `TelnetDecoder::new()` creates independent state; `feed(&[u8], callback)` delivers owned `TelnetEvent` values synchronously and returns `Result<(), DecodeError>`. There is no networking, runtime dependency, or internal event queue.

| Event | Behavior |
|---|---|
| `Data(Vec<u8>)` | Nonempty raw bytes, at most 4 KiB per event; available bytes flush at feed boundaries without waiting for a newline |
| `Command(u8)` | Standalone command code, including unknown codes; no action is performed |
| `Negotiation { verb, option }` | `NegotiationVerb::{Will, Wont, Do, Dont}` and raw option byte, including repeats and unknown options; no reply is generated |
| `Subnegotiation { option, payload }` | Complete payload, including empty payloads, capped at 64 KiB of decoded bytes excluding the option byte |

Framing follows the command structure in [RFC 854](https://www.rfc-editor.org/rfc/rfc854) and subnegotiation syntax in [RFC 855](https://www.rfc-editor.org/rfc/rfc855). Doubled `IAC` bytes become one data byte in ordinary data and subnegotiation payloads. Option bytes are consumed literally. Other data bytes, including CR/LF, nulls, ANSI sequences, and encoded text, are preserved. Control-event ordering and data bytes are invariant under arbitrary chunking; adjacent data-event boundaries may differ. Consumers must bound any retained events themselves.

`finish()` succeeds at a complete boundary and is idempotent after success. EOF during a command, negotiation option, subnegotiation option, payload, or escape reports `Truncated` with an `IncompleteFrame` category. Feeding a finished decoder, even with empty input, returns `DecoderFinished`.

The local malformed-input policy is strict: stray `IAC SE`, any command other than `IAC` or `SE` after `IAC` inside a payload, and exceeding the decoded payload limit fail immediately. Incomplete payloads are discarded, the remaining input is not processed, and subsequent feed/finish calls return `DecoderFailed`. Previously delivered events remain valid. Errors contain categories/control metadata, never transcript payloads. `reset()` discards partial state and allows reuse after partial input, failure, or successful finish; it emits nothing.

These are initial internal project APIs, not stable plugin contracts. NVT newline conversion, character decoding, option-specific behavior, command actions/TCP urgent handling, ANSI/MXP, GMCP/ATCP/MSP, and rendering are not implemented. The decoder's strict rejection policy is a local choice, not a universal server recovery rule.

## Implemented outbound encoding

`encode(&TelnetEvent, callback) -> Result<(), EncodeError>` uses a fixed 4 KiB scratch buffer and synchronous borrowed byte-slice callbacks. Output chunks are nonempty and no larger than `MAX_ENCODED_CHUNK_BYTES` (4 KiB); they may split control sequences. The caller must deliver all chunks in order before a later event, and own output retention, backpressure, and transport failure handling.

Data bytes are preserved except for doubled IAC escaping. Empty data emits nothing. Negotiations encode as IAC, verb, and literal option byte. Subnegotiations encode their option, escaped payload, and delimiters, including for empty payloads. No text conversion or newline/command terminator is added. Subnegotiations are limited to the decoder's 64 KiB decoded-payload bound; the encoder does not check option enablement.

Standalone `Command` values reserved for framing (`SE`, `SB`, `WILL`, `WONT`, `DO`, `DONT`, and `IAC`) are rejected; all other command codes remain representable. Reserved commands and oversized payloads fail before any callback is invoked. `EncodeError` contains only categories/control metadata, not payload bytes.

## Implemented option negotiation

`TelnetNegotiator::new(OptionPolicy::new(local_allowlist, remote_allowlist))` starts every option disabled. `TelnetNegotiator::default()` denies every option. Policy is immutable within a negotiator and uses independent allowlists: `Local` means this endpoint performs an option, while `Remote` means its peer performs it. Allowing a code is permission only; a future caller must implement that option's semantics before allowing it in a real connection. No options are configured automatically.

The state machine follows the symmetric Q method in [RFC 1143, section 7](https://www.rfc-editor.org/rfc/rfc1143), using fixed state for all 256 codes in both directions. `NegotiationState` exposes `No`, `Yes`, `WantNo`, `WantNoOpposite`, `WantYes`, and `WantYesOpposite`. `is_enabled` is true only in `Yes`; pending states do not authorize option behavior.

| Operation | Contract |
|---|---|
| `request(direction, option, enabled)` | Returns `Result<Option<NegotiationCommand>, NegotiationError>`; disallowed enables fail without mutation, disables are permitted, duplicates are idempotent, and pending reversals update the queued opposite request |
| `receive(verb, option)` | Returns at most one command; DO/DONT affect local state, WILL/WONT affect remote state, unsupported enables receive refusals, and repeated negative acknowledgments produce no reply loops |
| `state(direction, option)` / `is_enabled(direction, option)` | Inspect one side of one option without mutation |
| `reset()` | Clears all states and queued requests while retaining policy, without output |

`NegotiationCommand { verb, option }` converts to `TelnetEvent` using `Into`/`From`, then passes to `encode`. Construction and reset emit no startup negotiation. There are no reply queues, timers, retries, or automatic re-requests after refusals. Unexpected acknowledgments follow RFC 1143's recovery transitions, without logging transcripts or failing the decoder.

Process received negotiation events in order and deliver returned commands in operation order alongside other outgoing events. Negotiator state advances when a command is returned, not on successful delivery. Dropping a command and continuing can desynchronize the peers: the session owner must tear down/reset on output failure and manage bounded queues. The implemented TCP session layer provides these guarantees using default-deny policy. Other decoded events remain the caller's responsibility; these modules do not activate subnegotiation handlers or execute commands.

## Protocol roadmap

| Protocol | Intended capability | Reference |
|---|---|---|
| Telnet | Decoding, encoding, generic negotiation, and separate TCP sessions implemented; option-specific behavior planned | [RFC 854](https://www.rfc-editor.org/rfc/rfc854), [RFC 855](https://www.rfc-editor.org/rfc/rfc855), [RFC 1143](https://www.rfc-editor.org/rfc/rfc1143) |
| ANSI controls | Text styling and a deliberately supported control subset | [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/) |
| MXP | Supported markup converted into safe client display/actions | [Zugg Software MXP specification](https://www.zuggsoft.com/zmud/mxp.htm) |
| GMCP | Negotiated structured messages and documented package handling | [Aardwolf GMCP documentation](https://www.aardwolf.com/wiki/index.php/Clients/GMCP), a server-specific reference |
| ATCP | Negotiated structured extension messages | [Iron Realms ATCP reference](https://www.ironrealms.com/rapture/manual/files/FeatATCP-txt.html) |
| MSP | Sound directives subject to local resource and playback policy | [Zugg Software MSP specification](https://www.zuggsoft.com/zmud/msp.htm) |

## Parsing requirements

- Accept arbitrary transport chunk boundaries, including splits within control sequences, text encoding, and subnegotiations.
- Preserve ordinary text, ordering, and partial prompts while routing structured protocol data separately.
- Maintain negotiation state per connection and avoid repeated negotiation loops. Document unsupported options and recovery behavior.
- Bound incomplete sequences, payload sizes, nesting, and buffered output; reject or recover from malformed input deliberately.
- Keep Telnet framing distinct from text decoding. Select and document encoding behavior during implementation.
- Interpret supported markup as data. Never pass server-provided HTML, scripts, URLs, or commands directly into privileged execution.
- Gate media retrieval, links, and command actions through explicit application policies. A server request is not user authorization for arbitrary downloads or execution.

## Acceptance evidence

For each implemented feature, use deterministic inputs covering valid, unknown, truncated, repeated, interleaved, and oversized messages. Feed equivalent streams with different chunk boundaries and compare observable results. Reset state across disconnects and confirm no state crosses sessions.

Document unsupported variants and capture provenance in the [fixture guide](../../resources/fixtures/README.md). Public-server acceptance, when explicitly requested, supplements local tests; it does not establish universal compatibility. See [testing](../development/testing.md).
