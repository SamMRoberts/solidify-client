# Persistent storage

## Scope

Own user settings, profiles, and plugin-scoped persistence. These instructions supplement ancestor `AGENTS.md` files for this subtree. The optional `profiles` feature implements bounded versioned saved connections and transcript appearance; desktop enables it. Follow the [canonical storage contract](../../../docs/architecture/storage.md).

## Working guidance

- Use backend-resolved application configuration locations rather than the repository; separate secrets from ordinary settings and diagnostics.
- Validate imported data and paths. Isolate plugin namespaces and handle corrupt, missing, or unwritable state deliberately.
- Design atomic updates and explicit schema migrations when a format is selected; preserve existing user data and recover from partial failures.

## Verification

Test with temporary directories: first run, round-trip persistence, corrupt input, permission failures, migration, and interrupted writes.
