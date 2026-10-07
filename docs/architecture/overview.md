# Architecture overview

## Status and direction

This is the intended organization for a Rust/Tauri MUD client. The Rust Telnet wire core, bounded Tokio TCP session library, and independent UTF-8/basic ANSI presentation decoder are implemented in the single `src-tauri/` package. The feature-gated Tauri 2 application and React/TypeScript frontend now implement a single-connection desktop slice. Target platforms remain Windows, Linux, and macOS; native acceptance must be reported per OS. Named saved connections and appearance are implemented; other persistence and external plugin APIs remain deferred. Internal IPC DTOs are documented in the [desktop contracts](desktop.md).

## Implemented data flow

Caller-provided byte slices pass through `protocols::telnet::TelnetDecoder` to a synchronous callback receiving ordered raw-data and control events. Each instance owns its partial framing state. Data events and incomplete subnegotiations are bounded; the parser has no event queue. A consumer retaining events must bound its own queue. See the [framing contract](protocols.md#implemented-telnet-framing) for limits and lifecycle semantics.

The caller may pass negotiation events to a separate `TelnetNegotiator`, which owns fixed local/remote option state and immutable allowlists. Its returned commands convert into existing events for `encode`, which emits bounded borrowed wire chunks. Direct users of the wire core preserve command/chunk order, own delivery failures, and handle non-negotiation events. See the [negotiation contract](protocols.md#implemented-option-negotiation) before integrating option behavior.

The `sessions` module connects numeric socket addresses using the caller's Tokio runtime. A coordinator owns reading, protocol state, command processing, and bounded event delivery; a writer serializes encoded frames. The original connect API uses default-deny negotiation; desktop sessions opt into passive TTYPE/NAWS, remote ECHO, and SGA and forward every decoded event as untrusted bytes. Cancellation and terminal status bypass data queues; awaited shutdown joins both workers. See the [session contract](sessions.md) for bounds and lifecycle details.

Callers may pass only received `TelnetEvent::Data` into a connection-local
`protocols::presentation::PresentationDecoder`. It synchronously emits bounded
UTF-8 text, typed style snapshots, and nonexecuting control events. Other Telnet
events remain separate and do not interrupt partial presentation state. After
draining session events at end of input, callers finish the presentation decoder;
presentation failure does not automatically close the session. See the
[presentation contract](protocols.md#presentation-decoder-contract) for limits
and strict recovery. The desktop application adds its own coordinator and bounded
transcript renderer above these unchanged library APIs.

## Responsibilities

| Area | Intended ownership |
|---|---|
| Frontend components | Accessible controls, panels, notifications, and layout |
| Terminal | Output rendering, selection, scrollback, prompts, and command entry |
| Frontend bridge | Centralized native calls, event subscriptions, and error presentation |
| Backend commands | Validated application entrypoints with narrow capabilities |
| Application coordinator | Implemented hostname resolution, connection IDs, decoder composition, bounded polling, command acceptance, and supervised cleanup |
| Sessions | Implemented numeric TCP connections, task ownership, default-deny and opt-in MUD negotiation, bounded queues, and teardown |
| Protocols | Telnet decoding, encoding, generic negotiation, and UTF-8/basic ANSI presentation decoding implemented independently of UI/networking; TTYPE/NAWS/ECHO/SGA are implemented; other extensions remain planned |
| Plugins | Compatibility, capabilities, callbacks, isolation, and lifecycle |
| Storage | Implemented bounded saved connections and appearance; other settings and plugin namespaces remain planned |

## Application data flow

User input passes from the terminal through the frontend bridge to validated backend commands. The session owns transport and outbound ordering. Incoming bytes pass through Telnet framing and the supported option profile; only data enters UTF-8/basic ANSI decoding. The frontend polls bounded structured output and renders literal text with typed style classes. Other option extensions and markup interpretation remain deferred.

This is conceptual ordering, not a mandated parser API: Telnet subnegotiations and embedded display extensions need different handling. The implementation must retain stream order and partial sequence state.

Plugin hooks may observe or transform approved stages through a documented host interface. Raw network access is a privileged extension point requiring explicit ordering and size limits; it must not bypass the host's transport ownership.

## Ownership and failure boundaries

Each connection owns its parser state, asynchronous tasks, and subscriptions. Disconnecting or replacing a session must invalidate stale work and release resources. Parsing should remain deterministic and testable without opening a socket.

The frontend cannot authorize its own native privileges. MUD text, markup, imported files, and plugin output remain untrusted at every boundary. Persistent user data belongs in application data locations; test state belongs in disposable directories.

The desktop contracts define the implemented IPC boundary, including errors, limits, and lifecycle. The [storage contracts](storage.md) define saved profiles, locking and recovery. Future persistence and plugin features must define their own contracts before implementation; no database or plugin engine is selected.

See [protocol requirements](protocols.md), [plugin design](../plugins/design.md), and [testing strategy](../development/testing.md).
