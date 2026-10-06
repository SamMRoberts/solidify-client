# Native commands

## Scope

Own the validated entrypoints called through the frontend bridge. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Validate payloads and session identity before acting; do not trust frontend validation.
- Delegate transport, parsing, plugin, and storage work to their owning subsystems.
- Expose only task-specific capabilities and return bounded, non-sensitive errors.

## Verification

Exercise malformed inputs, missing or stale sessions, denied operations, and normal success paths through the command boundary.
