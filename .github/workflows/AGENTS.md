# Workflow automation

## Scope

Own future CI, packaging, auditing, documentation, and release workflow definitions. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Separate untrusted PR checks from privileged signing, publishing, and deployment jobs.
- Pin third-party actions to reviewed immutable revisions when adding workflows and set explicit minimal permissions.
- Use repository-defined commands, bounded jobs, and platform-specific build evidence; avoid automatic commits or publishing from ordinary checks.
- Scope secrets to trusted jobs and never interpolate untrusted event text directly into shell code.

## Verification

Review fork behavior, trigger scope, artifact provenance, and the Windows/Linux/macOS matrix before enabling automation.
