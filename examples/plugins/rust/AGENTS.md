# Native Rust plugin examples

## Scope

Own examples for the proposed native dynamic-library option. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Document that native plugins execute with host-process privileges and cannot be made safe by a manifest alone.
- Use only the eventual documented ABI and ownership contract; do not export Rust types across an assumed stable ABI.
- Document platform, architecture, panic handling, allocation ownership, and unload restrictions when implemented.

## Verification

Build against the supported compatibility matrix and test rejection of incompatible libraries in a disposable host.
