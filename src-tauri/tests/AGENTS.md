# Backend integration tests

## Scope

Own tests spanning backend subsystem boundaries. These instructions supplement ancestor `AGENTS.md` files for this subtree. `telnet_wire.rs` exercises the public encoder, decoder, and negotiator APIs between in-memory peers; `tcp_sessions.rs` exercises real TCP sessions using ephemeral loopback listeners. Other subsystem integration remains planned.

## Working guidance

- Exercise public behavior with local protocol emulators and disposable state.
- Keep pure parser unit tests close to their modules and application UI acceptance in the root tests/e2e directory.
- Bound waits and clean up listeners, sockets, tasks, and temporary files on success and failure.
- Prefer test-owned server futures/sockets so unwinding cleans them up; explicitly stop and join any spawned server tasks. Await session closure and check peer socket EOF.
- Bound in-memory message queues and exchange counts; assert final negotiation states and eventual quiescence rather than relying on a timeout to stop reply loops.

## Verification

Cover session isolation, ordering, refusal, EOF/framing failures, and cleanup without public MUD dependencies. Command, persistence, and plugin host coverage remains planned.
