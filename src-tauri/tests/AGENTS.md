# Backend integration tests

## Scope

Own tests spanning backend subsystem boundaries. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Exercise public behavior with local protocol emulators and disposable state.
- Keep pure parser unit tests close to their modules and application UI acceptance in the root tests/e2e directory.
- Bound waits and clean up listeners, sockets, tasks, and temporary files on success and failure.

## Verification

Cover command-to-session behavior, protocol events, persistence, and plugin host integration without public MUD dependencies.
