# Fuzzing workspace

## Scope

Own future fuzzing configuration, sanitized corpora, and reproduction guidance. These instructions supplement ancestor `AGENTS.md` files for this subtree. The directory currently contains documentation only.

## Working guidance

- Choose and document the fuzzing tool when implementation begins; no fuzz harness or dependency is configured now.
- Run parsers in isolation with bounded memory and execution time, without network or user profile access.
- Minimize failures and retain sanitized regression inputs with provenance; keep generated bulk output out of version control.

## Verification

Record target, toolchain, seed or corpus, limits, and reproduction command for each result.
