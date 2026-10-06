# Developer tooling

## Scope

Own future repeatable local maintenance and developer utilities. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Keep tooling task-specific, cross-platform where practical, and explicit about working directories and prerequisites.
- Fail with actionable errors; avoid logging secrets or silently downloading, installing, publishing, or changing user profiles.
- Offer dry-run behavior for destructive or external changes when such tooling is introduced.

## Verification

Verify documented inputs, exit status, failure cleanup, and repeatability; do not invent scripts in setup instructions.
