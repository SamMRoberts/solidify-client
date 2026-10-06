# Architecture overview

## Status and direction

This is the intended organization for a Rust/Tauri MUD client. The Rust Telnet wire core, bounded Tokio TCP session library, and independent UTF-8/basic ANSI presentation decoder are implemented in the single `src-tauri/` package. Target platforms remain Windows, Linux, and macOS; current library verification does not establish desktop support. Tauri, the frontend, IPC, and persistence are not implemented. No external API or serialization format is defined yet.

## Implemented data flow

Caller-provided byte slices pass through `protocols::telnet::TelnetDecoder` to a synchronous callback receiving ordered raw-data and control events. Each instance owns its partial framing state. Data events and incomplete subnegotiations are bounded; the parser has no event queue. A consumer retaining events must bound its own queue. See the [framing contract](protocols.md#implemented-telnet-framing) for limits and lifecycle semantics.

The caller may pass negotiation events to a separate `TelnetNegotiator`, which owns fixed local/remote option state and immutable allowlists. Its returned commands convert into existing events for `encode`, which emits bounded borrowed wire chunks. Direct users of the wire core preserve command/chunk order, own delivery failures, and handle non-negotiation events. See the [negotiation contract](protocols.md#implemented-option-negotiation) before integrating option behavior.

The `sessions` module connects numeric socket addresses using the caller's Tokio runtime. A coordinator owns reading, protocol state, command processing, and bounded event delivery; a writer serializes encoded frames. Connected sessions use default-deny negotiation and forward every decoded event as untrusted bytes. Cancellation and terminal status bypass data queues; awaited shutdown joins both workers. See the [session contract](sessions.md) for bounds and lifecycle details.

Callers may pass only received `TelnetEvent::Data` into a connection-local
`protocols::presentation::PresentationDecoder`. It synchronously emits bounded
UTF-8 text, typed style snapshots, and nonexecuting control events. Other Telnet
events remain separate and do not interrupt partial presentation state. After
draining session events at end of input, callers finish the presentation decoder;
presentation failure does not automatically close the session. See the
[presentation contract](protocols.md#presentation-decoder-contract) for limits
and strict recovery. No terminal rendering or scrollback model exists.

## Responsibilities

| Area | Intended ownership |
|---|---|
| Frontend components | Accessible controls, panels, notifications, and layout |
| Terminal | Output rendering, selection, scrollback, prompts, and command entry |
| Frontend bridge | Centralized native calls, event subscriptions, and error presentation |
| Backend commands | Validated application entrypoints with narrow capabilities |
| Sessions | Implemented numeric TCP connections, task ownership, default-deny negotiation, bounded queues, and teardown |
| Protocols | Telnet decoding, encoding, generic negotiation, and UTF-8/basic ANSI presentation decoding implemented independently of UI/networking; option-specific behavior and other extensions remain planned |
| Plugins | Compatibility, capabilities, callbacks, isolation, and lifecycle |
| Storage | User settings, profiles, plugin namespaces, and safe persistence |

## Intended data flow

User input passes from the terminal through the frontend bridge to validated backend commands. The session owns transport and outbound ordering. Incoming bytes pass through Telnet framing and negotiated extension processing before display text is interpreted for styling or supported markup. The backend delivers session-scoped display updates and structured events to the frontend.

This is conceptual ordering, not a mandated parser API: Telnet subnegotiations and embedded display extensions need different handling. The implementation must retain stream order and partial sequence state.

Plugin hooks may observe or transform approved stages through a documented host interface. Raw network access is a privileged extension point requiring explicit ordering and size limits; it must not bypass the host's transport ownership.

## Ownership and failure boundaries

Each connection owns its parser state, asynchronous tasks, and subscriptions. Disconnecting or replacing a session must invalidate stale work and release resources. Parsing should remain deterministic and testable without opening a socket.

The frontend cannot authorize its own native privileges. MUD text, markup, imported files, and plugin output remain untrusted at every boundary. Persistent user data belongs in application data locations; test state belongs in disposable directories.

Before implementing IPC or persistence, document the minimum contracts needed by that feature, including errors, limits, and compatibility. The scaffold deliberately does not select framework, engine, database, or wire-format details.

See [protocol requirements](protocols.md), [plugin design](../plugins/design.md), and [testing strategy](../development/testing.md).
