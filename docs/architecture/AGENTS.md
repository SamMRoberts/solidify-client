# Architecture documentation

## Scope

Own subsystem boundaries, data flow, and protocol requirements. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Keep diagrams and responsibilities aligned with actual code as implementation develops.
- Document trust boundaries, ownership, and failure behavior without prematurely inventing wire contracts.
- Cite protocol sources and identify server-specific behavior explicitly.

## Verification

Review for contradictions between transport, parser, frontend, storage, and plugin responsibilities.
