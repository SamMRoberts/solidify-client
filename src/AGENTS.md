# Frontend

## Scope

Own application presentation and client-side interaction; the framework is not selected. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Route native calls and subscriptions through the bridge; keep protocol decoding and credential storage in the backend.
- Treat server and plugin output as data, never executable HTML or JavaScript.
- Keep session state explicit and clean up subscriptions when views close.

## Verification

Review keyboard navigation, session switching, rendering safety, and listener cleanup; use the frontend checks once configured.
