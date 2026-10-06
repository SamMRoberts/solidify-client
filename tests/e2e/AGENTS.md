# End-to-end acceptance

## Scope

Own full application workflows once the application and a suitable driver exist. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Test connection, command entry, output, session switching, persistence, and plugin lifecycle through user-visible flows.
- Use explicit readiness conditions and bounded timeouts instead of arbitrary sleeps.
- Capture only redacted artifacts and restore disposable state after failures. Do not assume one driver works on every target platform.

## Verification

Verify keyboard access, reconnect and error handling, restart persistence, and each supported native platform; report unsupported coverage.
