# Session lifecycle

## Scope

Own connections, read/write tasks, session identity, and teardown. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory implements the public session API in `mod.rs`, private generic I/O coordination in `driver.rs`, and deterministic unit tests in `tests.rs`. See the [session contract](../../../docs/architecture/sessions.md).

## Working guidance

- Maintain isolated connection and negotiation state per session.
- Handle partial reads/writes, connection errors, cancellation, and disconnects without blocking the UI or leaking tasks.
- Preserve fixed read, send, frame, and queue bounds. Pause reads under event pressure; never drop events during normal backpressure.
- Keep cancellation and writer failure separate from data queues. Every blocking queue operation must observe termination; awaited shutdown must join both workers. Guard the writer against unexpected coordinator exits.
- Use the caller's runtime, numeric addresses, fresh protocol state, and caller-assigned unique IDs. Connected sessions deny every option; do not expose option enablement or silently reconnect/replay commands.
- A send result acknowledges queue acceptance only. Preserve raw bytes and processing order; terminate after a cancelled partial write rather than retrying it.

## Verification

Use ephemeral loopback servers for the public API and private generic I/O adapters for short writes, errors, and stalls. Use a controlled clock for deadlines. Bound test waits and clean up all sockets/tasks, including on failure; no public MUD fixtures.
