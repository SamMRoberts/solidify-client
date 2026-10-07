# Rust/Tauri application

## Scope

Own the native application boundary, platform integration, and backend test package. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains a Rust library for Telnet decoding, encoding, Q-method negotiation, bounded TCP sessions using pinned Tokio 1.53.2, and independent UTF-8/basic ANSI presentation decoding. The application coordinator adds bounded DNS/connect attempts and pull delivery; a `desktop` feature enables Tauri 2 commands and the native window. The `profiles` feature adds bounded JSON profile storage without native dependencies; desktop enables it. Unit, in-memory peer, loopback, coordinator, and temporary-directory storage tests exist.

## Working guidance

- Keep Tauri-specific initialization and permissions separate from protocol and domain logic.
- Use narrow capabilities; avoid exposing arbitrary filesystem, shell, or network access through frontend commands.
- Keep platform-specific code localized and document platform differences. Do not select a crate layout beyond this single backend without a concrete need.

## Verification

Run the [library commands](../docs/development/setup.md#library-commands) from the repository root using this directory's Cargo manifest. Keep the lockfile tracked; fetch locked dependencies before offline checks. Do not introduce Tokio into pure protocol modules. Verify platform claims on the corresponding OS; library checks do not establish native application acceptance.
