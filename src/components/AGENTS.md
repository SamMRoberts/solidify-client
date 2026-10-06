# UI components

## Scope

Own reusable controls, panels, notifications, and layout. These instructions supplement ancestor `AGENTS.md` files for this subtree. Connection controls and command entry are implemented with explicit props and acceptance-based draft clearing.

## Working guidance

- Use accessible labels, predictable focus, and keyboard-operable actions; do not rely on color alone.
- Keep components focused on presentation and explicit inputs/events; use the bridge for native interactions.
- Avoid embedding protocol interpretation or plugin privilege decisions in views.

## Verification

Verify empty, loading, disconnected, and error states along with resizing and focus behavior.
