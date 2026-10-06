# Protocol fixtures

## Scope

Own sanitized inputs and expected outcomes for future tests. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Record origin, permission, sanitization, protocol context, byte notation, and expected behavior.
- Distinguish synthetic examples from captured traffic. Never include credentials, private chat, or identifiable profiles.
- Preserve parsing-relevant byte boundaries while redacting; document any transformation that changes lengths or framing.

## Verification

Check exact byte interpretation, provenance, and expected output; do not claim a fixture ran until a consuming test exists.
