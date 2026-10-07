# Protocol support and requirements

## Status

Byte-level Telnet decoding/encoding, configurable Q-method negotiation, and independent bounded UTF-8/ANSI presentation decoding are implemented. Bounded TCP transport is implemented separately in [sessions](sessions.md); a separate [desktop layer](desktop.md) renders basic styled text and line controls. Passive TTYPE/NAWS, remote ECHO, and SGA behavior is implemented in the separate option handler below; a stateless GMCP envelope codec is also implemented below, with explicit opt-in session integration. Desktop GMCP and other extensions remain planned. The references below establish wire syntax and negotiation behavior, not a claim of complete Telnet or MUD compatibility.

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

These are initial internal project APIs, not stable plugin contracts. NVT newline conversion, option-specific behavior, command actions/TCP urgent handling, MXP, GMCP/ATCP/MSP, and rendering are not implemented by this wire module. Character decoding and basic ANSI styling are implemented separately below. The Telnet decoder's strict rejection policy is a local choice, not a universal server recovery rule.

## Presentation decoder contract

The `protocols::presentation` module is independent of transport and Tokio.
Callers pass only `TelnetEvent::Data` bytes to one `PresentationDecoder` per
session. Handle all other Telnet events separately without flushing or resetting
presentation state. This layer does not change any Telnet or session API.

`new()` starts with default colors and disabled style flags. `feed(bytes, callback)`
and `finish(callback)` synchronously deliver owned `PresentationEvent` values and
return `Result<(), PresentationError>`. Events are `Text(String)`,
`StyleChanged(TextStyle)`, and `Control(TextControl)`. There is no event queue;
callers must bound retained output. Text is untrusted literal content, not markup.

### Text and styles

UTF-8 follows [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629). Invalid ordinary
text runs have the replacement behavior of Rust's `String::from_utf8_lossy`,
independent of chunking. Retain at most three incomplete bytes between calls and
reprocess a byte that invalidates a pending prefix. An ASCII control interrupting
that prefix, or EOF, first emits one replacement. Preserve BOM and ordinary
Unicode without normalization. Raw eight-bit C1 bytes undergo UTF-8 validation;
they are never ANSI introducers. Decoded U+0080–U+009F controls are discarded.

Text events are nonempty, valid UTF-8, and at most 4,096 encoded bytes including
replacements, with no split scalars. Flush available text at feed boundaries,
before emitted controls/style changes, and before errors. Chunk invariance permits
different adjacent text-event boundaries only. CR, LF, tab, backspace, and bell
are distinct control events, without execution or newline conversion. Other
ground-state C0 controls and DEL are discarded.

`TextStyle` contains typed default/basic, indexed, and RGB foreground/background
values and boolean bold, italic, underline, and inverse flags. Support SGR 0,
1/22, 3/23, 4/24, 7/27, 30–37/39, 40–47/49, bright 90–97/100–107, and the
[xterm indexed/direct color forms](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html):

- Indexed: `38;5;n` / `48;5;n` and `38:5:n` / `48:5:n`.
- RGB: `38;2;r;g;b`, `38:2:r:g:b`, `38:2::r:g:b`, or `38:2:0:r:g:b`,
  with corresponding `48` background forms. Explicit color-space slots accept
  only empty or zero; other color spaces and additional subparameters are unsupported.

Color operands must be nonempty decimal integers from 0 through 255. Indices 0–7
canonicalize to existing basic variants; bright codes canonicalize to indices 8–15.
`TextColor::Indexed(u8)` and `TextColor::Rgb { red, green, blue }` are additive
internal enum variants; exhaustive consumers must handle them. Bold never implies
bright color. Ordinary empty top-level fields mean reset. Apply complete groups
left to right and commit atomically, emitting one snapshot only when style changes.

Any unsupported parameter, private/intermediate form, incomplete color group,
empty/out-of-range operand, unsupported mode, or mixed separators inside a group
makes the entire SGR a no-op. Rejected operands never become independent flags or
resets. Large decimal values are rejected without overflow. All operands count
toward the existing 16 semicolon-field limit; colon groups occupy one field and
remain subject to the 128-byte sequence limit. No output queue is introduced.

### Escape syntax and limits

Use seven-bit ESC forms and ESC/CSI byte classes from
[ECMA-48, sections 5 and 8.3.117](https://ecma-international.org/wp-content/uploads/ECMA-48_5th_edition_june_1991.pdf).
Consume syntactically valid unsupported ESC/CSI sequences silently. Ordinary
sequences are capped at 128 bytes, including introducer, embedded controls, and
final byte. A completed SGR is capped at 16 semicolon-separated fields, including
empty fields, even when unsupported. Check sequence length before processing a
byte, then syntax; check the SGR field count at its final byte.

Discard OSC, DCS, SOS, PM, and APC strings with a 4,096-byte total limit including
introducer and terminator. Retain no payload: only kind, length, and terminator
state. Payload is opaque, including nested introducers and invalid UTF-8. End at
`ESC \`; OSC additionally accepts BEL per the
[xterm convention](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html).
The terminating BEL emits no bell event. A nonterminating ESC remains discarded
payload; consecutive ESC bytes allow the last to begin the terminator.

Within ESC/CSI, emit the five supported C0 controls without ending the sequence.
Ignore other C0/DEL except ESC, CAN, and SUB, which are malformed interruptions.
This strict interruption policy is local, not a claim of terminal emulation.

### Failure and lifecycle

Malformed syntax, excess sequence/string length, excess SGR fields, and truncated
escape framing return typed errors without transcript bytes. Errors distinguish
these categories and truncated escape, CSI, string, and string-terminator states.
Flush preceding text, discard incomplete state, and latch failure without
processing the remainder of the input. Further feed/finish calls fail until
`reset()`. No automatic resynchronization or session closure occurs.

`finish(callback)` replaces a pending UTF-8 suffix; truncated escape framing
instead fails. Successful finish is idempotent and feeding afterward, even empty
input, returns `DecoderFinished`. `reset()` silently discards partial state and
restores default style, including after finish/failure; consumers resetting a
decoder must also reset their own tracked style. Construction/reset emit nothing.

Legacy encodings, charset negotiation, cursor movement,
screen editing, hyperlinks, clipboard actions, rendering, and terminal emulation
remain deferred. Unsupported controls never execute actions.

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

Process received negotiation events in order and deliver returned commands in operation order alongside other outgoing events. Negotiator state advances when a command is returned, not on successful delivery. Dropping a command and continuing can desynchronize the peers: the session owner must tear down/reset on output failure and manage bounded queues. The implemented TCP session layer provides these guarantees using default-deny or the explicit implemented-options profile. Other decoded events remain the caller's responsibility; these modules do not activate subnegotiation handlers or execute commands.

## Implemented GMCP envelope codec

`protocols::gmcp` is a stateless, transport-independent codec following the
[TinTin++ GMCP guide](https://tintin.mudhalla.net/protocols/gmcp/). It consumes one
complete, already-unescaped option-201 subnegotiation payload from the Telnet
decoder. It has no streaming state, queue, package cache, networking, or rendering.
The explicit [GMCP session profile](sessions.md#opt-in-gmcp-profile) integrates
this codec with TCP. Default and desktop profiles still refuse GMCP; the desktop
does not consume these messages.

| API | Contract |
|---|---|
| `GMCP` | Telnet option code 201 |
| `GmcpMessage { package, data }` | Case-preserved `String` and `Option<serde_json::Value>`; public fields are validated on encoding |
| `decode(&[u8])` | Returns one complete message or a payload-free `GmcpError` |
| `to_subnegotiation(&GmcpMessage)` | Returns a validated `TelnetEvent::Subnegotiation`; pass it to the existing Telnet encoder for framing and IAC escaping |

The first ASCII space separates the package from JSON. Without a separator there
is no data; with one, exactly one JSON value is required, with ordinary JSON
whitespace allowed. Absent data and explicit `null` are distinct. Objects, arrays,
and scalars are accepted. Encoding emits compact JSON and exactly one separator
when data exists, otherwise just the package name.

Names remain opaque, including unknown packages and `MSDP`; no case folding,
namespace dispatch, Core handshake, subscriptions, or game-specific semantics are
implemented. JSON keys preserve case. Standard `serde_json::Value` semantics
apply: duplicate object keys retain their last value, numbers use the library's
integer/floating-point representation, and original spelling, whitespace and
object-key order are not preserved. Out-of-range numbers rejected by serde_json
are invalid JSON for this codec. Parsed strings remain untrusted, including
escaped controls and markup.

### Local bounds and errors

The complete payload, including package and separator, is at most 64 KiB before
Telnet escaping, reusing the wire module's limit. Names must contain 1–256 visible
ASCII bytes (`0x21` through `0x7e`, without whitespace). JSON permits at most 64
nested arrays/objects, counting the root container; scalar roots have depth zero.
The name and depth restrictions are local policies, not additional GMCP syntax
requirements.

Inbound byte limits are checked before allocation; an allocation-free, string-
and escape-aware depth preflight runs before JSON deserialization. Outbound
values undergo depth-bounded traversal and serialization into a size-limited
writer, including JSON escape expansion. No partial event is returned on failure.
Allocated parser/value overhead and caller-retained messages are outside the
payload-byte bound; callers must bound their queues and retained state.

`GmcpError` distinguishes invalid names, UTF-8, JSON, payload size, nesting depth,
and serialization failure. Errors contain no payloads or underlying parser error
strings. Each failed call rejects only its message; subsequent calls work without
`reset` or `finish`. Telnet framing errors retain their existing fatal/latching
contract. The GMCP codec does not close a connection or flush presentation state.

### Negotiation composition

The caller owns negotiation and must process events in order. A passive client
allows remote option 201 in the existing Q-method negotiator, then responds to a
server's `WILL` with `DO`. Effective remote enablement authorizes both receiving
and sending GMCP; the client does not also need local GMCP enablement. Do not
initiate client negotiation or act on premature/disabled payloads. Deliver every
negotiation response and encoded frame in order, or terminate the connection.

On `WONT`, disable GMCP and clear any caller-owned package state. A later `WILL`
starts a fresh exchange, including after server copyover; reconnection starts
with fresh negotiator state. No package state exists inside this codec.
`src-tauri/tests/gmcp_wire.rs` provides bounded in-memory composition examples,
including send gating and quiescent refusal. The explicit GMCP session profile
provides production integration; in-memory tests do not establish live-server
compatibility.

## Protocol roadmap

| Protocol | Intended capability | Reference |
|---|---|---|
| Telnet | Decoding, encoding, generic negotiation, and separate TCP sessions implemented; TTYPE/NAWS/ECHO/SGA implemented in an opt-in profile | [RFC 854](https://www.rfc-editor.org/rfc/rfc854), [RFC 855](https://www.rfc-editor.org/rfc/rfc855), [RFC 1143](https://www.rfc-editor.org/rfc/rfc1143) |
| UTF-8 and ANSI controls | Bounded text decoding, basic flags plus bright/indexed/RGB SGR, and nonexecuting control events implemented; basic desktop rendering implemented separately; other styles deferred | [RFC 3629](https://www.rfc-editor.org/rfc/rfc3629), [ECMA-48](https://ecma-international.org/publications-and-standards/standards/ecma-48/) |
| MXP | Supported markup converted into safe client display/actions | [Zugg Software MXP specification](https://www.zuggsoft.com/zmud/mxp.htm) |
| GMCP | Bounded envelope codec and explicit TCP profile implemented; desktop integration, Core setup and package handling deferred | [TinTin++ GMCP guide](https://tintin.mudhalla.net/protocols/gmcp/); [Aardwolf packages](https://www.aardwolf.com/wiki/index.php/Clients/GMCP), a server-specific reference |
| ATCP | Negotiated structured extension messages | [Iron Realms ATCP reference](https://www.ironrealms.com/rapture/manual/files/FeatATCP-txt.html) |
| MSP | Sound directives subject to local resource and playback policy | [Zugg Software MSP specification](https://www.zuggsoft.com/zmud/msp.htm) |

## Parsing requirements

- Accept arbitrary transport chunk boundaries, including splits within control sequences, text encoding, and subnegotiations.
- Preserve ordinary text, ordering, and partial prompts while routing structured protocol data separately.
- Maintain negotiation state per connection and avoid repeated negotiation loops. Document unsupported options and recovery behavior.
- Bound incomplete sequences, payload sizes, nesting, and buffered output; reject or recover from malformed input deliberately.
- Keep Telnet framing distinct from text decoding. UTF-8 is the initial presentation encoding; legacy encodings and charset negotiation remain deferred.
- Interpret supported markup as data. Never pass server-provided HTML, scripts, URLs, or commands directly into privileged execution.
- Gate media retrieval, links, and command actions through explicit application policies. A server request is not user authorization for arbitrary downloads or execution.

## Acceptance evidence

For each implemented feature, use deterministic inputs covering valid, unknown, truncated, repeated, interleaved, and oversized messages. Feed equivalent streams with different chunk boundaries and compare observable results. Reset state across disconnects and confirm no state crosses sessions.

Document unsupported variants and capture provenance in the [fixture guide](../../resources/fixtures/README.md). Public-server acceptance, when explicitly requested, supplements local tests; it does not establish universal compatibility. See [testing](../development/testing.md).

## Implemented terminal options

`protocols::options::TerminalOptions` owns one existing Q-method negotiator,
a `TerminalSize`, and a masking generation. It is independent of Tokio and has
no output queue. `SessionOptions::DenyAll` preserves the original refusal policy;
`MudClient { size }` permits local TTYPE/NAWS, remote ECHO, and SGA both ways.
Construction emits nothing. `receive` emits at most two ordered owned events;
`set_size` emits at most one. Responses must be delivered or the connection closed.

- TTYPE responds only to exactly `SEND` (`[1]`) while enabled, with `IS SOLIDIFY`.
  Repeated requests return the same single identity. No terminal emulation, MTTS,
  or extended-color support is advertised. See [RFC 1091](https://www.rfc-editor.org/info/rfc1091/).
- NAWS emits four network-order bytes (columns, rows) when enabled and after
  changed dimensions. The encoder escapes IAC. Disabled updates replace the cached
  size; re-enabling sends the latest value. The protocol type accepts zero as
  unknown; the desktop validates nonzero dimensions. See [RFC 1073](https://www.rfc-editor.org/info/rfc1073/).
- Effective remote ECHO increments a `u64` generation on every disabled-to-enabled
  transition (saturating at its maximum). This preserves transient enables for
  polling consumers. ECHO itself specifies echo ownership, not passwords; the
  desktop adopts the [MUD masking convention](https://wiki.mudlet.org/w/Manual%3ATechnical_Manual/en#ECHO_.28Password_Masking.29).
  See [RFC 857](https://www.rfc-editor.org/info/rfc857/).
- SGA is tracked independently in both directions. Neither ECHO nor SGA changes
  line-based sends, newline handling, or local echo. See [RFC 858](https://www.rfc-editor.org/info/rfc858/).

Unsupported directions/options are refused through the Q method. Unsupported,
malformed, or premature option payloads are ignored without retaining their
contents. Telnet framing errors still follow the decoder's strict failure policy.
Each session creates fresh option state; subscribers never interpret raw payloads
as frontend actions.
