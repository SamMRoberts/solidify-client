# Development environment

## Scope

Own future reproducible container-based development guidance and configuration. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Keep versions aligned with actual project manifests once they exist.
- Avoid privileged containers, host credentials, and broad host mounts unless a concrete development requirement justifies them.
- Document which native platform checks remain necessary outside a container; a Linux environment cannot prove macOS or Windows acceptance.

## Verification

Verify a clean setup and documented prerequisites when configuration is introduced; there is no container to launch now.
