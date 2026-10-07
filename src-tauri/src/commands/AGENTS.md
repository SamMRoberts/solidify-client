# Native commands

## Scope

Own the validated entrypoints called through the frontend bridge. These instructions supplement ancestor `AGENTS.md` files for this subtree. Connection commands implement start, poll, send-line, viewport updates, and disconnect. Six profile commands implement load/retry, create, update, appearance-only update, delete and selection; they expose no paths and never open sockets. Their DTOs are separate from protocol types; all commands verify window identity and application origin.

## Working guidance

- Validate payloads and session identity before acting; do not trust frontend validation.
- Delegate transport, parsing, plugin, and storage work to their owning subsystems.
- Expose only task-specific capabilities and return bounded, non-sensitive errors.

Map indexed/RGB colors to dedicated numeric DTOs while retaining existing named strings.

## Verification

Exercise malformed inputs, missing or stale sessions, denied operations, and normal success paths through the command boundary.
