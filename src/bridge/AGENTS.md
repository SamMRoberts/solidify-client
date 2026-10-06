# Tauri bridge

## Scope

Own the frontend boundary for native commands and event subscriptions. These instructions supplement ancestor `AGENTS.md` files for this subtree. Four native invocations and a single cancellable poll chain are implemented; mocked bridges belong only in tests.

## Working guidance

- Centralize invocation, payload conversion, and subscription cleanup; do not duplicate command names across components.
- Carry session identity through asynchronous work and prevent stale responses from updating a replacement session.
- Map expected failures to useful UI states without leaking credentials. Backend validation remains authoritative.

## Verification

Verify rejected payloads, cancellation or stale responses, disconnects, and repeated subscribe/unsubscribe cycles using the configured frontend tests and separate native acceptance.
