# Frontend

## Scope

Own application presentation and client-side interaction using React, TypeScript, and Vite. These instructions supplement ancestor `AGENTS.md` files for this subtree. The single-connection desktop UI, bridge, bounded transcript model, profile selection/editor with live transcript appearance, and colocated Vitest tests are implemented. Preserve late-startup guards and acceptance-based saves; no frontend filesystem persistence.

## Working guidance

- Route native calls and subscriptions through the bridge; keep protocol decoding and credential storage in the backend.
- Treat server and plugin output as data, never executable HTML or JavaScript.
- Keep session state explicit and clean up subscriptions when views close.

## Verification

Review keyboard navigation, session switching, rendering safety, and listener cleanup; run the configured typecheck, tests, formatting, and build commands. Keep browser evidence separate from native IPC acceptance.
