# JavaScript plugin examples

## Scope

Own examples for a proposed JavaScript plugin engine. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Do not assume Node.js, browser globals, eval permissions, or direct Tauri access.
- Keep plugin execution isolated from the trusted application frontend; use explicitly granted host functions.
- Handle asynchronous errors and release subscriptions and tasks when disabled; prevent late results from modifying a replacement instance.

## Verification

Verify rejected capabilities, failed promises, bounded execution, and disable/reload cleanup in the selected runtime.
