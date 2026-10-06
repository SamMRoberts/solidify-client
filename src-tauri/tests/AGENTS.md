# Backend integration tests

## Scope

Own tests spanning backend subsystem boundaries. These instructions supplement ancestor `AGENTS.md` files for this subtree. `telnet_wire.rs` exercises the public encoder, decoder, and negotiator APIs between in-memory peers; transport and other subsystem integration remain planned.

## Working guidance

- Exercise public behavior with local protocol emulators and disposable state.
- Keep pure parser unit tests close to their modules and application UI acceptance in the root tests/e2e directory.
- Bound waits and clean up listeners, sockets, tasks, and temporary files on success and failure.
- Bound in-memory message queues and exchange counts; assert final negotiation states and eventual quiescence rather than relying on a timeout to stop reply loops.

## Verification

Cover command-to-session behavior, protocol events, persistence, and plugin host integration without public MUD dependencies.
