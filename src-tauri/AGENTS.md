# Rust/Tauri application

## Scope

Own the native application boundary, platform integration, and backend test package. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Keep Tauri-specific initialization and permissions separate from protocol and domain logic.
- Use narrow capabilities; avoid exposing arbitrary filesystem, shell, or network access through frontend commands.
- Keep platform-specific code localized and document platform differences. Do not select a crate layout beyond this single backend without a concrete need.

## Verification

Use the backend manifest and configured formatting/lint/build checks once present; verify platform claims on the corresponding OS.
