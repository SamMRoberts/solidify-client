# Session lifecycle

## Scope

Own connections, read/write tasks, session identity, and teardown. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Maintain isolated connection and negotiation state per session.
- Handle partial reads/writes, connection errors, cancellation, and disconnects without blocking the UI or leaking tasks.
- Bound queues and define backpressure when implementing transport. Do not silently reconnect and replay user commands.

## Verification

Use local emulators to cover concurrent sessions, rapid reconnects, in-flight disconnects, and cleanup.
