# Rust/Tauri application

## Scope

Own the native application boundary, platform integration, and backend test package. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains a dependency-free Rust library for Telnet decoding, encoding, and Q-method negotiation, with unit and in-memory integration tests; Tauri and transport are not initialized.

## Working guidance

- Keep Tauri-specific initialization and permissions separate from protocol and domain logic.
- Use narrow capabilities; avoid exposing arbitrary filesystem, shell, or network access through frontend commands.
- Keep platform-specific code localized and document platform differences. Do not select a crate layout beyond this single backend without a concrete need.

## Verification

Run the [library commands](../docs/development/setup.md#library-commands) from the repository root using this directory's Cargo manifest. Keep the lockfile tracked. Verify platform claims on the corresponding OS; library checks do not establish native application acceptance.
