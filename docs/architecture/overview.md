# Architecture overview

## Status and direction

This is the intended organization for a Rust/Tauri MUD client, not an implemented architecture. Target platforms are Windows, Linux, and macOS. The frontend remains framework-neutral, and the backend occupies one Tauri application directory. No public API or serialization format is defined yet.

## Responsibilities

| Area | Intended ownership |
|---|---|
| Frontend components | Accessible controls, panels, notifications, and layout |
| Terminal | Output rendering, selection, scrollback, prompts, and command entry |
| Frontend bridge | Centralized native calls, event subscriptions, and error presentation |
| Backend commands | Validated application entrypoints with narrow capabilities |
| Sessions | Connections, task ownership, negotiation state, queues, and teardown |
| Protocols | Streaming framing and interpretation independent of UI or live networking |
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
