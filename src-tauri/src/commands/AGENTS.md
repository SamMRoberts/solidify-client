# Native commands

## Scope

Own the validated entrypoints called through the frontend bridge. These instructions supplement ancestor `AGENTS.md` files for this subtree. Four main-window commands implement start, poll, send-line, and disconnect. Their DTOs are separate from protocol types; all commands verify window identity and application origin.

## Working guidance

- Validate payloads and session identity before acting; do not trust frontend validation.
- Delegate transport, parsing, plugin, and storage work to their owning subsystems.
- Expose only task-specific capabilities and return bounded, non-sensitive errors.

## Verification

Exercise malformed inputs, missing or stale sessions, denied operations, and normal success paths through the command boundary.
