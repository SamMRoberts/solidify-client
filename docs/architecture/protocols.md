# Protocol support and requirements

## Status

Byte-level Telnet framing is implemented. Transport, negotiation policy, text decoding, and extension interpretation remain planned. The references below establish framing syntax and future requirements, not a claim of complete Telnet or MUD compatibility.

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

This is an initial internal project API, not a stable plugin contract. NVT newline conversion, character decoding, outgoing encoding, option acceptance/refusal and loop prevention, command actions/TCP urgent handling, transport, ANSI/MXP, GMCP/ATCP/MSP, and rendering are not implemented. The parser's strict rejection policy is a local choice, not a universal server recovery rule.

## Planned coverage

| Protocol | Intended capability | Reference |
|---|---|---|
| Telnet | Stream framing, commands, negotiation, and subnegotiation | [RFC 854](https://www.rfc-editor.org/rfc/rfc854), [RFC 855](https://www.rfc-editor.org/rfc/rfc855) |
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
