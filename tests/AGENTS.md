# Application verification

## Scope

Own cross-subsystem acceptance specifications and shared application test guidance. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Keep backend integration tests under src-tauri/tests and parser fuzzing under fuzz.
- Use deterministic inputs, local services, and disposable profiles; never use a developer's live profile as a fixture.
- Record the OS, runtime, test scope, and limitations with results. Browser checks do not prove native Tauri behavior.

## Verification

Check behavior visible to users and report automated, native, and live-server evidence separately.
