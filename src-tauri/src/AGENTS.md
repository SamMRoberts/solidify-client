# Backend subsystems

## Scope

Own backend composition across commands, sessions, protocols, plugins, and storage. These instructions supplement ancestor `AGENTS.md` files for this subtree. The library entrypoint, Telnet decoding/encoding/negotiation and UTF-8/ANSI presentation decoding under `protocols/`, and bounded Tokio TCP sessions under `sessions/` are implemented. `application/` owns the single-connection coordinator; `main.rs` and `commands/` provide feature-gated Tauri composition and DTOs. `storage/` implements feature-gated saved connections and appearance with bounded admission, strict validation, locking and atomic replacement. Plugins remain guidance only.

## Working guidance

- Keep protocol parsing independent of UI and network execution so it can be exercised deterministically.
- Make ownership of session tasks, buffers, subscriptions, and shutdown explicit; avoid process-global mutable session state.
- Keep errors meaningful across boundaries and redact sensitive data before logging.

## Verification

Check subsystem boundaries and failure cleanup; place integration tests in the sibling tests directory.
