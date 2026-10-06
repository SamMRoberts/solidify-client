# Tauri bridge

## Scope

Own the frontend boundary for native commands and event subscriptions. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Centralize invocation, payload conversion, and subscription cleanup; do not duplicate command names across components.
- Carry session identity through asynchronous work and prevent stale responses from updating a replacement session.
- Map expected failures to useful UI states without leaking credentials. Backend validation remains authoritative.

## Verification

Verify rejected payloads, cancellation or stale responses, disconnects, and repeated subscribe/unsubscribe cycles once IPC exists.
