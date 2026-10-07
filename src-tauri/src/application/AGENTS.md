# Application coordinator

Own one connection attempt/session, DNS admission, presentation composition,
bounded output polling, and awaited cleanup. Follow ancestor instructions.

Keep this module independent of Tauri and serialization. Preserve the numeric
session API and protocol APIs. Never release the DNS slot merely because its
caller stopped waiting. Cancellation and status must bypass output pressure.
Use sanitized errors and fresh IDs, and supervise session ownership across task
failure. Keep tests local, waits bounded, and server sockets owned by test futures.

Run focused `application` tests, then the configured library checks. See the
[desktop contracts](../../../docs/architecture/desktop.md) before changing limits.
