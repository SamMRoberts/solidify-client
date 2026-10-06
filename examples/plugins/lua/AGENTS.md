# Lua plugin examples

## Scope

Own examples for a proposed embedded Lua engine. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Do not assume unrestricted standard libraries, filesystem access, process execution, or dynamic module loading.
- Use the eventual documented host API and bounded callback execution.
- Release event handlers and timers on disable/unload; keep state scoped to the plugin and session.

## Verification

Exercise missing capabilities, script errors, execution limits, and repeated lifecycle transitions.
