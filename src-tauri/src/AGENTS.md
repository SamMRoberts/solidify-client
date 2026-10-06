# Backend subsystems

## Scope

Own backend composition across commands, sessions, protocols, plugins, and storage. These instructions supplement ancestor `AGENTS.md` files for this subtree. The library entrypoint, Telnet decoding/encoding/negotiation under `protocols/`, and bounded Tokio TCP sessions under `sessions/` are implemented; other subsystems remain guidance only.

## Working guidance

- Keep protocol parsing independent of UI and network execution so it can be exercised deterministically.
- Make ownership of session tasks, buffers, subscriptions, and shutdown explicit; avoid process-global mutable session state.
- Keep errors meaningful across boundaries and redact sensitive data before logging.

## Verification

Check subsystem boundaries and failure cleanup; place integration tests in the sibling tests directory.
