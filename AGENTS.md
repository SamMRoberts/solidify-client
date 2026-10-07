# solidify repository instructions

## Current state and purpose

solidify is a planned cross-platform MUD client using Rust and Tauri, targeting Windows, Linux, and macOS. The implemented core in `src-tauri/` provides Telnet decoding/encoding, configurable Q-method negotiation, bounded TCP sessions using pinned Tokio 1.53.2 on a caller-owned runtime, and independent bounded UTF-8/ANSI presentation decoding. Protocol modules remain independent of Tokio. A stateless GMCP envelope codec provides bounded JSON parsing/encoding; explicit opt-in TCP sessions add ordered GMCP interpretation and bounded sends. Desktop GMCP and package semantics remain deferred. Unit, in-memory peer, and loopback TCP tests exist. A feature-gated Tauri 2 desktop application and React/TypeScript frontend implement one connection, bounded styled transcripts, command entry, and a loopback demo. The desktop opts into passive TTYPE/NAWS, remote ECHO masking, and bidirectional SGA. Named connection profiles and per-profile transcript appearance use bounded, versioned backend storage through the optional `profiles` feature. Other option behavior, credentials, other persistence, plugins, installers, and automation remain deferred.

Treat capabilities labeled planned in the design documents as requirements or proposals, not working features. Rust 1.99 is the library baseline; Tauri 2.12.1, React 19.3.0, TypeScript 7.0.2, Vite 8.3.3, and npm are selected and locked. Plugin engines and external plugin API contracts remain unselected. The library's Rust interfaces are internal project contracts. Do not infer implementation from directory names.

Presentation supports basic flags plus bright, indexed 256-color and direct RGB SGR, preserving bounded atomic decoding.

## Instruction scope

Read this file and the applicable nested `AGENTS.md` files before editing a directory. Nested instructions specialize the parent guidance within their subtrees; do not assume sibling instructions apply. When starting at the repository root, explicitly inspect guides below it before making nested edits.

Keep shared engineering rules here and directory-specific rules close to their files. Use ordinary `AGENTS.md` files; no override files or Codex configuration changes are needed. See [instruction discovery](docs/development/agent-instructions.md).

## Repository map

| Location | Responsibility |
|---|---|
| [src](src/AGENTS.md) | Framework-neutral frontend: components, terminal, Tauri bridge |
| [src-tauri](src-tauri/AGENTS.md) | Rust/Tauri application, backend subsystems, integration tests |
| [tests](tests/AGENTS.md) | Application-level acceptance and end-to-end testing |
| [fuzz](fuzz/AGENTS.md) | Bounded protocol fuzzing |
| [examples](examples/AGENTS.md) | Plugin author examples, separated by proposed runtime |
| [scripts](scripts/AGENTS.md) | Development and maintenance tooling |
| [.github](.github/AGENTS.md) | Contribution templates and future GitHub automation |
| [.devcontainer](.devcontainer/AGENTS.md) | Future reproducible development environment |
| [docs](docs/README.md) | Architecture, development, plugin, and operations guidance |
| [resources](resources/README.md) | Primary references, sanitized fixtures, and asset provenance |

## Working agreements

- Inspect existing files, Git status, relevant architecture, and actual configuration before changes. Preserve unrelated and uncommitted work.
- Make the smallest coherent change. Follow established patterns; avoid speculative abstractions, unrelated refactors, and dependency changes.
- Keep code readable, testable, and maintainable. Preserve compatibility unless the task requires a breaking change; document any such change.
- Treat network traffic, markup, IPC payloads, plugin input, and imported files as untrusted. Validate at boundaries and keep credentials out of logs and fixtures.
- Handle credible failures deliberately: cancellation, disconnects, partial writes, resource cleanup, bounded buffers, and plugin failures belong to their owning subsystems.
- Preserve user profile and map data. Use disposable state for verification and do not connect to live MUDs as an implicit test step.
- Do not run destructive Git operations, rewrite history, commit, or push unless explicitly requested. Automation design documents do not grant publishing or external messaging authority.
- Remove debug artifacts and scaffolding introduced accidentally. Explain constraints with focused comments rather than restating code.

## Verification and completion

For Markdown changes, inspect relative links, headings, instruction consistency, whitespace, and the complete diff including untracked files. Do not install an application toolchain just to validate documentation or the library.

Use the [configured Cargo commands](docs/development/setup.md#library-commands): focused behavioral tests first, followed by formatting, lint, library build, and documentation checks. Desktop/frontend changes also require the configured desktop-feature and npm checks plus separate browser/native acceptance. Do not invent frontend/package scripts or report hypothetical commands as executed. See the [testing strategy](docs/development/testing.md).

Report what changed, what was verified, and what remains unverified. Separate documentation/static checks, automated tests, native application acceptance, and live-server evidence. A build does not prove runtime behavior.

## Code review rules

Flag unsafe rendering of server content, excessive IPC or plugin privileges, unbounded stream processing, leaked session resources, and destructive persistence changes. Require meaningful regression evidence for changed behavior without weakening existing checks. Confirm docs distinguish implemented behavior from planned work.
