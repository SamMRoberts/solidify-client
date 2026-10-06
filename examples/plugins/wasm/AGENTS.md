# WebAssembly plugin examples

## Scope

Own examples for a proposed WebAssembly runtime. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Use only explicitly granted host imports and WASI capabilities; do not assume ambient filesystem or network access.
- Document the supported target, interface version, memory and execution limits when selected.
- Handle traps and cleanup; hot reload requires host lifecycle support and is not guaranteed by WASM.

## Verification

Verify capability denial, resource limits, trap handling, and unload behavior using the implemented host.
